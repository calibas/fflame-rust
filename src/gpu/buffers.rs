use egui_wgpu::wgpu::*;
use egui_wgpu::wgpu::util::DeviceExt;
use crate::scene::transforms::{Transform, Flame};
use crate::scene::palette::Palette;

/// Maximum total number of transforms per flame, summed across the
/// normal / linked / final pools. The GPU `transforms` buffer is a
/// single concatenated array of this size: normals occupy the first
/// `flame.transforms.len()` slots, linkeds the next
/// `flame.linked_transforms.len()`, finals the next
/// `flame.final_transforms.len()`. Per-pool CPU-side indexes are
/// translated to global xform_ids during GPU packing.
///
/// Bumped from the legacy 32 to 128 in the per-transform-linked-and-final
/// project. Drives the transforms buffer (~60 KB), the variation_params
/// buffer (~800 KB), and the attachments buffer (~33 KB) — ~893 KB of
/// always-allocated GPU memory per flame.
///
/// Tried 64 as a perf optimization; benchmarks were identical to 128
/// (per-iteration cost is dominated by the in-loop `attachments[xform_idx]`
/// load, not by overall buffer size). Kept at 128 for the headroom
/// (Apophysis-style flames with 30+ normals + auto-attached finals).
/// `MAX_ATTACHMENTS_PER_TRANSFORM` is the actually-impactful knob for
/// per-iteration cost — see its doc comment.
///
/// Soft cap: panics with a clear message if a flame exceeds it.
/// See `docs/projects/per-transform-linked-and-final.md`.
pub const MAX_TRANSFORMS: usize = 128;

/// Max number of per-transform analytic-blur mean-splat buffers (v1 cap).
/// A flame with more eligible blur transforms renders the excess
/// stochastically (their `analytic_blur_slot` stays -1). Each buffer costs
/// `width×height×4×u32`, so this bounds the analytic-blur memory.
pub const MAX_BLUR_BUFFERS: u32 = 4;

/// Maximum number of subflames in a single FractalConfig.
///
/// Each subflame is a complete `Flame` definition referenced by index
/// from a `subflame_wf` variation's `subflame_id` parameter. The cap
/// is intentionally small for v1 (most real-world flames use 1–2
/// subflames; the JWildfire ecosystem rarely exceeds 4). Bumping this
/// later costs proportional GPU memory in the subflame transform
/// buffer + metadata uniform.
pub const MAX_SUBFLAMES: usize = 8;

/// Maximum combined transform count across all subflames in one config.
///
/// Subflame transforms are stored back-to-back in a single GPU buffer
/// (binding 11 once Phase 4 wires it). This cap bounds that buffer's
/// size: `MAX_SUBFLAME_TRANSFORMS_TOTAL × sizeof(GpuTransform)` ≈
/// 139 KB at 256 × 544 B/transform. Validation at config load fails
/// if a config's subflames collectively exceed it.
///
/// A single subflame can have up to `MAX_TRANSFORMS` (128) transforms
/// in principle, but in practice subflames are small (3–10 transforms);
/// this cap is set so 8 typical subflames fit comfortably.
pub const MAX_SUBFLAME_TRANSFORMS_TOTAL: usize = 256;

/// Total xform_id space covering both parent and subflame xforms.
///
/// Parent xforms live in `[0, MAX_TRANSFORMS)` (their xform_id is the
/// pool position, unchanged from v1). Subflame xforms live in
/// `[MAX_TRANSFORMS, MAX_TRANSFORMS_UNIFIED)`, packed back-to-back in
/// the same order as `subflame_transforms` packs them. This is the
/// size of every per-xform GPU buffer that needs to address both
/// parent and subflame xforms (currently `variation_params`; later
/// `transforms` once the separate `subflame_transforms_buffer` is
/// merged in Phase 5).
pub const MAX_TRANSFORMS_UNIFIED: usize = MAX_TRANSFORMS + MAX_SUBFLAME_TRANSFORMS_TOTAL;

/// Maximum number of attachments (linked + final) per normal transform.
/// Per-normal chain length cap — independent of the total transform
/// budget. Sized to keep `GpuAttachmentList` small (264 bytes per entry
/// at 32) so the per-iteration `attachments[xform_idx]` load stays
/// cheap; was briefly bumped to 100 but reverted after benchmark
/// regressions traced to the larger struct's bandwidth cost.
pub const MAX_ATTACHMENTS_PER_TRANSFORM: usize = 32;

/// Bit flags packed into `GpuTransform::plane_flags`. Each bit
/// indicates whether the corresponding JWildfire-extension plane
/// affine is non-identity (and therefore needs the per-step linear
/// math in `apply_affine` / `apply_post_affine`). When all four bits
/// are zero, the shader takes the existing flat 2D-affine path —
/// the equivalent of JWildfire's `TransformationAffineFlatStep`.
/// Apophysis flames always land here.
pub const PLANE_FLAG_YZ: u32 = 1 << 0;
pub const PLANE_FLAG_ZX: u32 = 1 << 1;
pub const PLANE_FLAG_YZ_POST: u32 = 1 << 2;
pub const PLANE_FLAG_ZX_POST: u32 = 1 << 3;

/// GPU representation of Transform (must match WGSL struct layout)
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct GpuTransform {
    // Pre-affine matrix
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
    pub g: f32, // Z offset for 3D mode
    pub weight: f32,

    // Post-affine matrix (applied after variations)
    pub post_a: f32,
    pub post_b: f32,
    pub post_c: f32,
    pub post_d: f32,
    pub post_e: f32,
    pub post_f: f32,
    pub post_g: f32,
    pub post_enabled: f32, // 0.0 = disabled, 1.0 = enabled (f32 for GPU alignment)

    // Variations (100 floats: supports all Apophysis 7X + future expansion)
    pub variations: [f32; 100],

    // Color (palette position) + color_speed + opacity + direct_color (forms vec4 for alignment)
    pub color: f32,
    pub color_speed: f32,
    pub opacity: f32,
    pub direct_color: f32,

    // JWildfire-extension plane affines (positional `[a, c, b, d, e, f]`
    // matching the XML write order). Read in the shader as
    // `array<f32, 6>` with JWildfire-style 00/01/10/11/20/21 indexing.
    // See `docs/projects/jwf-features.md` for the composition rule.
    pub yz_coefs: [f32; 6],
    pub zx_coefs: [f32; 6],
    pub yz_post_coefs: [f32; 6],
    pub zx_post_coefs: [f32; 6],

    // Bit flags (see `PLANE_FLAG_*` constants). When 0, the shader's
    // existing flat 2D-affine path runs — same math as before this
    // extension. When non-zero, the gated YZ → ZX steps run too.
    // Computed host-side on upload by comparing each plane to identity.
    pub plane_flags: u32,

    // Analytic-blur routing: index of this transform's mean-splat blur
    // buffer, or -1 when the transform isn't analytic-blur-eligible. The
    // plot section routes a blur-terminated iteration to `blur_buffers[slot]`
    // instead of the main histogram. Repurposed from one `_plane_pad` slot,
    // so the struct size/alignment is unchanged. See
    // `docs/projects/analytic-blur-buffer.md`.
    pub analytic_blur_slot: i32,

    // Analytic-blur routing knobs (carved from the former 2-u32 pad, so the
    // struct size/alignment is unchanged). `strength` scales the mean-splat
    // density (artistic dominance); `residual` keeps routing the next N plots
    // through the blur buffer to smooth propagated fuzz. Both default to the
    // no-op (1.0 / 0) and are only read when analytic_blur_slot >= 0.
    pub analytic_blur_strength: f32,
    pub analytic_blur_residual: u32,
    // (former `_plane_pad: [u32; 2]` fully consumed — size still 592 = 37×16.)
}

// Manual implementation for bytemuck (arrays of size 50 not auto-derived)
unsafe impl bytemuck::Pod for GpuTransform {}
unsafe impl bytemuck::Zeroable for GpuTransform {}

/// GPU representation of a single subflame's transform-pool offsets.
///
/// One `SubflameMeta` per subflame, indexed by `subflame_id` (the
/// variation parameter). The fields describe where this subflame's
/// transforms live inside the concatenated `subflame_transforms`
/// storage buffer. The shader's `subflame_iterate(id, ...)` helper
/// reads from `subflame_transforms[normals_offset .. normals_offset
/// + normals_count]` for the chaos-game step + `final_offset`/`final_count`
/// for the optional final-xform chain.
///
/// Linked transforms (the "attachment" mechanism the parent flame
/// uses) are intentionally **not** part of subflames in v1 — the
/// JWildfire reference port doesn't use them, and they'd add another
/// shader-codegen path for a feature that nobody serializes today.
/// Future v2 can add linked support by extending this struct.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default)]
pub struct SubflameMeta {
    /// Index into `subflame_transforms` where this subflame's normal
    /// (regular, non-final) transforms begin.
    pub normals_offset: u32,
    /// Number of normal transforms for this subflame.
    pub normals_count: u32,
    /// Index into `subflame_transforms` where this subflame's final
    /// transforms begin. `final_count = 0` means the subflame has no
    /// final-xform chain.
    pub finals_offset: u32,
    /// Number of final transforms for this subflame.
    pub finals_count: u32,
    /// Reserved. Was `render_mode` (0=2D/1=3D), removed in config v3 — render
    /// mode is scene-global now and was never read by any shader. Kept as a
    /// pad so the 32-byte GPU layout and every field offset stay stable.
    pub _reserved_render_mode: u32,
    /// Base value added to (normals_offset/finals_offset + picked) to
    /// produce the synthetic `xform_id` the variation system sees for
    /// this subflame's xforms. v1: always 128 — the constant prefix that
    /// keeps subflame xform_ids outside the parent's [0, MAX_TRANSFORMS)
    /// range. v2 will set this per-subflame to the unified-array
    /// position where this subflame's xforms start, so the variation
    /// system's per-xform buffers can hold real entries for subflame
    /// xforms too.
    pub xform_id_base: u32,
    pub _pad0: u32,
    pub _pad1: u32,
}

unsafe impl bytemuck::Pod for SubflameMeta {}
unsafe impl bytemuck::Zeroable for SubflameMeta {}

/// Pack a flame's transforms into the unified GPU layout:
/// parent xforms (normals, linkeds, finals) → zero-pad to `MAX_TRANSFORMS` →
/// subflame xforms (per subflame: normals then finals) → zero-pad to
/// `MAX_TRANSFORMS_UNIFIED`. Shared by `Buffers::update_transforms` and the
/// `HighResExporter` so both render engines lay out transforms (incl.
/// subflames) identically. The subflame local index map matches
/// `pack_gpu_variation_params`.
pub fn pack_gpu_transforms(flame: &Flame, render_mode: crate::scene::transforms::RenderMode) -> Vec<GpuTransform> {
    let registry = crate::variations::global_registry();
    let mut gpu_transforms = GpuTransform::from_flame(flame, &registry, render_mode);

    // Pad parent slack to MAX_TRANSFORMS so subflame slots start at the
    // synthetic-id base (matches variation_params layout).
    while gpu_transforms.len() < MAX_TRANSFORMS {
        gpu_transforms.push(bytemuck::Zeroable::zeroed());
    }

    let local_map = crate::scene::transforms::compute_local_index_map(
        flame.active_variation_names_ordered(&registry),
    );
    let mut subflame_xforms_packed = 0usize;
    for sf in &flame.subflames {
        for xform in &sf.transforms {
            gpu_transforms.push(GpuTransform::from_transform(xform, &local_map));
            subflame_xforms_packed += 1;
        }
        for xform in &sf.final_transforms {
            gpu_transforms.push(GpuTransform::from_transform(xform, &local_map));
            subflame_xforms_packed += 1;
        }
    }
    if subflame_xforms_packed > MAX_SUBFLAME_TRANSFORMS_TOTAL {
        panic!(
            "Subflames have {} total transforms but MAX_SUBFLAME_TRANSFORMS_TOTAL is {}",
            subflame_xforms_packed, MAX_SUBFLAME_TRANSFORMS_TOTAL,
        );
    }
    while gpu_transforms.len() < MAX_TRANSFORMS_UNIFIED {
        gpu_transforms.push(bytemuck::Zeroable::zeroed());
    }
    gpu_transforms
}

/// Pack a flame's per-transform variation parameters into the unified GPU
/// layout, in lockstep with `pack_gpu_transforms` (same parent → pad → subflame
/// → pad ordering, same shared local index map). Shared by
/// `Buffers::update_variation_params` and `HighResExporter`.
pub fn pack_gpu_variation_params(flame: &Flame) -> Vec<GpuVariationParams> {
    let registry = crate::variations::global_registry();
    let local_map = crate::scene::transforms::compute_local_index_map(
        flame.active_variation_names_ordered(&registry),
    );

    let mut gpu_params: Vec<GpuVariationParams> = Vec::with_capacity(MAX_TRANSFORMS_UNIFIED);
    for xform in &flame.transforms {
        gpu_params.push(GpuVariationParams::from_transform(xform, &local_map, &registry));
    }
    for xform in &flame.linked_transforms {
        gpu_params.push(GpuVariationParams::from_transform(xform, &local_map, &registry));
    }
    for xform in &flame.final_transforms {
        gpu_params.push(GpuVariationParams::from_transform(xform, &local_map, &registry));
    }
    while gpu_params.len() < MAX_TRANSFORMS {
        gpu_params.push(bytemuck::Zeroable::zeroed());
    }

    let mut subflame_xforms_packed = 0usize;
    for sf in &flame.subflames {
        for xform in &sf.transforms {
            gpu_params.push(GpuVariationParams::from_transform(xform, &local_map, &registry));
            subflame_xforms_packed += 1;
        }
        for xform in &sf.final_transforms {
            gpu_params.push(GpuVariationParams::from_transform(xform, &local_map, &registry));
            subflame_xforms_packed += 1;
        }
    }
    if subflame_xforms_packed > MAX_SUBFLAME_TRANSFORMS_TOTAL {
        panic!(
            "Subflames have {} total transforms but MAX_SUBFLAME_TRANSFORMS_TOTAL is {}",
            subflame_xforms_packed, MAX_SUBFLAME_TRANSFORMS_TOTAL,
        );
    }
    while gpu_params.len() < MAX_TRANSFORMS_UNIFIED {
        gpu_params.push(bytemuck::Zeroable::zeroed());
    }
    gpu_params
}

/// Build the per-subflame metadata array (offsets/counts into the unified
/// transform layout produced by `pack_gpu_transforms`). Shared by
/// `Buffers::update_subflames` and `HighResExporter`.
pub fn build_subflame_metas(subflames: &[Flame]) -> Result<[SubflameMeta; MAX_SUBFLAMES], String> {
    if subflames.len() > MAX_SUBFLAMES {
        return Err(format!(
            "Config has {} subflames but MAX_SUBFLAMES is {}",
            subflames.len(),
            MAX_SUBFLAMES,
        ));
    }
    let mut metas: [SubflameMeta; MAX_SUBFLAMES] = Default::default();
    let mut cursor: u32 = 0;
    for (sf_idx, sf) in subflames.iter().enumerate() {
        let normals_offset = cursor;
        cursor += sf.transforms.len() as u32;
        let normals_count = cursor - normals_offset;
        let finals_offset = cursor;
        cursor += sf.final_transforms.len() as u32;
        let finals_count = cursor - finals_offset;
        metas[sf_idx] = SubflameMeta {
            normals_offset,
            normals_count,
            finals_offset,
            finals_count,
            _reserved_render_mode: 0,
            xform_id_base: MAX_TRANSFORMS as u32,
            _pad0: 0,
            _pad1: 0,
        };
    }
    if cursor as usize > MAX_SUBFLAME_TRANSFORMS_TOTAL {
        return Err(format!(
            "Subflames have {} total transforms but MAX_SUBFLAME_TRANSFORMS_TOTAL is {}",
            cursor, MAX_SUBFLAME_TRANSFORMS_TOTAL,
        ));
    }
    Ok(metas)
}

impl GpuTransform {
    /// Create from Transform using a per-flame local index map.
    /// See `crate::scene::transforms::compute_local_index_map` for context.
    pub fn from_transform(
        xform: &Transform,
        local_map: &std::collections::HashMap<String, u32>,
    ) -> Self {
        Self::from_transform_with_opacity(xform, xform.opacity, local_map)
    }

    /// Create from Transform with an explicit effective opacity (for solo mode)
    pub fn from_transform_with_opacity(
        xform: &Transform,
        effective_opacity: f32,
        local_map: &std::collections::HashMap<String, u32>,
    ) -> Self {
        // Host-computed plane-flag word. Comparing the [f32; 6] arrays
        // to the identity constant via the Transform helpers — the
        // bit gates the corresponding shader step entirely. JWildfire
        // does the equivalent decision once at xform-build time via
        // `isHasYZCoeffs()` etc.
        let mut plane_flags: u32 = 0;
        if !xform.is_yz_identity() { plane_flags |= PLANE_FLAG_YZ; }
        if !xform.is_zx_identity() { plane_flags |= PLANE_FLAG_ZX; }
        if !xform.is_yz_post_identity() { plane_flags |= PLANE_FLAG_YZ_POST; }
        if !xform.is_zx_post_identity() { plane_flags |= PLANE_FLAG_ZX_POST; }

        Self {
            a: xform.a,
            b: xform.b,
            c: xform.c,
            d: xform.d,
            e: xform.e,
            f: xform.f,
            g: xform.g,
            weight: xform.weight,
            post_a: xform.post_a,
            post_b: xform.post_b,
            post_c: xform.post_c,
            post_d: xform.post_d,
            post_e: xform.post_e,
            post_f: xform.post_f,
            post_g: xform.post_g,
            // has_post_step, not post_affine_enabled: JWF gates the
            // YZ/ZX post planes independently of the XY post, so a
            // transform with only `zxPost` must still run the step
            // (the XY part is identity then).
            post_enabled: if xform.has_post_step() { 1.0 } else { 0.0 },
            variations: xform.to_fixed_array(local_map),
            color: xform.color,
            color_speed: xform.color_speed,
            opacity: effective_opacity,
            direct_color: xform.direct_color,
            yz_coefs: xform.yz_coefs,
            zx_coefs: xform.zx_coefs,
            yz_post_coefs: xform.yz_post_coefs,
            zx_post_coefs: xform.zx_post_coefs,
            plane_flags,
            // -1 = not analytic-blur-eligible. The host overwrites this per
            // eligible transform after blur buffers are assigned (see
            // update_transforms / the analytic-blur slot pass).
            analytic_blur_slot: -1,
            analytic_blur_strength: 1.0,
            analytic_blur_residual: 0,
        }
    }

    /// Create GPU transforms from a Flame, handling solo mode.
    /// Computes the per-flame local index map once, then populates each
    /// transform's variation slot array against that map.
    /// Pack the flame's three transform pools (normals, linkeds, finals)
    /// into a single concatenated GPU array. Layout:
    ///   slots 0..N                      — normals (N = flame.transforms.len())
    ///   slots N..N+L                    — linkeds (L = flame.linked_transforms.len())
    ///   slots N+L..N+L+F                — finals  (F = flame.final_transforms.len())
    ///
    /// `xform_id` in the shader is a global index into this array.
    /// Per-pool CPU indexes (in `Transform::linked_attachments` /
    /// `final_attachments`) get translated to global xform_ids during
    /// attachment-list packing — see `GpuAttachmentList::from_transform`.
    ///
    /// The legacy `flame.final_transform` field is migrated into
    /// `flame.final_transforms[0]` at deserialize time and re-synced
    /// on every UI edit, so chain renderers see it without needing a
    /// separate GPU slot. See
    /// `docs/projects/per-transform-linked-and-final.md`.
    ///
    /// Solo mode (a normal-only feature) sets effective_opacity to 0
    /// for non-solo normals; linkeds and finals always run with their
    /// own opacity (currently irrelevant for them since plot opacity
    /// inherits from the normal that fired).
    /// Eligible transforms are assigned analytic-blur slots `0..MAX_BLUR_BUFFERS`
    /// optimistically; the actual allocated count can be smaller (memory cap in
    /// `ensure_lowres_blur_buffers`), and the shader gates `slot < count`, so a
    /// transform whose slot wasn't allocated falls back to the stochastic path.
    pub fn from_flame(
        flame: &Flame,
        registry: &crate::variations::VariationRegistry,
        render_mode: crate::scene::transforms::RenderMode,
    ) -> Vec<Self> {
        let local_map = crate::scene::transforms::compute_local_index_map(
            flame.active_variation_names_ordered(registry),
        );
        let solo_idx = flame.solo_transform;

        let mut gpu_transforms: Vec<Self> = Vec::with_capacity(
            flame.transforms.len()
                + flame.linked_transforms.len()
                + flame.final_transforms.len(),
        );

        for (i, xform) in flame.transforms.iter().enumerate() {
            let effective_opacity = match solo_idx {
                Some(solo) if i != solo => 0.0,
                _ => xform.opacity,
            };
            gpu_transforms.push(Self::from_transform_with_opacity(xform, effective_opacity, &local_map));
        }
        for xform in &flame.linked_transforms {
            gpu_transforms.push(Self::from_transform(xform, &local_map));
        }
        for xform in &flame.final_transforms {
            gpu_transforms.push(Self::from_transform(xform, &local_map));
        }

        // Analytic-blur slot assignment. Only when the whole-flame gate is
        // satisfied; otherwise every slot stays -1 and the shader routes
        // these transforms' plots through the normal (stochastic) histogram.
        // Slots 0..MAX_BLUR_BUFFERS map to the first eligible normals; any
        // eligible transforms beyond the cap keep -1 (stochastic fallback).
        if flame.analytic_blur_active(registry, render_mode) {
            for (slot, (xform_idx, name, _w)) in flame
                .analytic_blur_transforms(registry)
                .into_iter()
                .take(MAX_BLUR_BUFFERS as usize)
                .enumerate()
            {
                let t = &flame.transforms[xform_idx];
                gpu_transforms[xform_idx].analytic_blur_slot = slot as i32;
                gpu_transforms[xform_idx].analytic_blur_strength =
                    t.get_variation_param(&name, "strength").unwrap_or(1.0).max(0.0);
                gpu_transforms[xform_idx].analytic_blur_residual =
                    t.get_variation_param(&name, "residual").unwrap_or(0.0).max(0.0) as u32;
            }
        }

        gpu_transforms
    }
}

/// Maximum total variation-parameter slots per transform.
///
/// This is the packed buffer's capacity. With the packed layout, each
/// variation occupies exactly `parameters.len() + init_param_count`
/// slots, so the cap applies to `sum(active_variation_slot_counts)`,
/// not to any single variation. Even pathological flames stay well
/// below this — typical flames use 30-150 slots.
pub const MAX_VARIATION_PARAM_SLOTS: usize = 1600;

/// GPU-side per-normal-transform list of attached Linked and Final
/// indexes. Buffer is `array<GpuAttachmentList, MAX_NORMAL_TRANSFORMS>`
/// — one entry per normal transform slot, indexed by `xform_id`.
///
/// Layout: each list is a fixed-capacity `[u32; 32]` plus a count.
/// Unused tail entries are zero (the count gates how many are read).
/// Indices in `linked` reference `linked_transforms[]`; indices in
/// `final_` reference `final_transforms[]`.
///
/// std430 alignment: `array<u32, N>` is tightly packed, `u32` is
/// 4-byte aligned. Total size: 32·4 + 4 + 32·4 + 4 = 264 bytes per
/// entry, or 33 KB for the whole buffer at MAX_TRANSFORMS=128.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct GpuAttachmentList {
    pub linked: [u32; MAX_ATTACHMENTS_PER_TRANSFORM],
    pub linked_count: u32,
    pub final_: [u32; MAX_ATTACHMENTS_PER_TRANSFORM],
    pub final_count: u32,
}

unsafe impl bytemuck::Pod for GpuAttachmentList {}
unsafe impl bytemuck::Zeroable for GpuAttachmentList {}

impl GpuAttachmentList {
    /// Pack a normal transform's attachment lists into the GPU layout,
    /// translating per-pool CPU indexes into the GLOBAL xform_id space
    /// of the concatenated transforms buffer.
    ///
    /// Layout convention (set by host packing — see `GpuTransform::from_flame`):
    ///   slots 0..normal_count                                      — normals
    ///   slots normal_count..normal_count+linked_count               — linkeds
    ///   slots normal_count+linked_count..total                      — finals
    ///
    /// `linked_offset = flame.transforms.len()`,
    /// `final_offset  = flame.transforms.len() + flame.linked_transforms.len()`.
    ///
    /// Truncates to `MAX_ATTACHMENTS_PER_TRANSFORM` with a warning. Pool
    /// indexes that overflow their pool's actual size are dropped with a
    /// warning (host-side validation should catch this earlier).
    pub fn from_transform(
        t: &crate::scene::transforms::Transform,
        linked_offset: usize,
        linked_count_total: usize,
        final_offset: usize,
        final_count_total: usize,
    ) -> Self {
        let mut linked = [0u32; MAX_ATTACHMENTS_PER_TRANSFORM];
        let mut final_ = [0u32; MAX_ATTACHMENTS_PER_TRANSFORM];

        let mut linked_count = 0u32;
        for &idx in t.linked_attachments.iter() {
            if linked_count as usize >= MAX_ATTACHMENTS_PER_TRANSFORM {
                log::warn!(
                    "Transform has more than {} linked attachments; truncating",
                    MAX_ATTACHMENTS_PER_TRANSFORM
                );
                break;
            }
            if idx >= linked_count_total {
                log::warn!("Linked attachment index {} exceeds linked pool size {}; dropped", idx, linked_count_total);
                continue;
            }
            linked[linked_count as usize] = (linked_offset + idx) as u32;
            linked_count += 1;
        }

        let mut final_count = 0u32;
        for &idx in t.final_attachments.iter() {
            if final_count as usize >= MAX_ATTACHMENTS_PER_TRANSFORM {
                log::warn!(
                    "Transform has more than {} final attachments; truncating",
                    MAX_ATTACHMENTS_PER_TRANSFORM
                );
                break;
            }
            if idx >= final_count_total {
                log::warn!("Final attachment index {} exceeds final pool size {}; dropped", idx, final_count_total);
                continue;
            }
            final_[final_count as usize] = (final_offset + idx) as u32;
            final_count += 1;
        }

        Self { linked, linked_count, final_, final_count }
    }

    pub fn empty() -> Self {
        Self {
            linked: [0u32; MAX_ATTACHMENTS_PER_TRANSFORM],
            linked_count: 0,
            final_: [0u32; MAX_ATTACHMENTS_PER_TRANSFORM],
            final_count: 0,
        }
    }
}

/// Byte stride of one AttachmentList entry at the given per-side cap.
/// Layout matches the WGSL struct (with the same `{{ATTACHMENT_CAP}}`
/// substituted): `array<u32, cap> + u32 + array<u32, cap> + u32`.
/// std430 packs u32 arrays tightly, so this is just `(cap * 4 + 4) * 2`.
#[inline]
pub fn attachment_stride_bytes(cap: usize) -> usize {
    (cap * 4 + 4) * 2
}

/// Pack one normal transform's linked + final attachment lists into a
/// pre-zeroed `dst` slice sized exactly `attachment_stride_bytes(cap)`.
/// Translates per-pool CPU indexes into GLOBAL xform_ids in the
/// concatenated transforms buffer (see `GpuTransform::from_flame` for
/// the layout convention).
///
/// `cap` matches the shader's `array<u32, cap>` length — both the
/// shader stride and this packing must agree, which is what
/// `Flame::attachment_cap()` guarantees when the shader is rebuilt on
/// cap changes.
pub fn pack_attachment_entry(
    dst: &mut [u8],
    t: &crate::scene::transforms::Transform,
    cap: usize,
    linked_offset: usize,
    linked_pool_size: usize,
    final_offset: usize,
    final_pool_size: usize,
) {
    debug_assert_eq!(dst.len(), attachment_stride_bytes(cap));

    // Linked array (cap × u32) followed by linked_count (u32).
    let mut linked_count = 0u32;
    for &idx in t.linked_attachments.iter() {
        if (linked_count as usize) >= cap {
            log::warn!(
                "Transform has more than {} linked attachments; truncating",
                cap
            );
            break;
        }
        if idx >= linked_pool_size {
            log::warn!(
                "Linked attachment index {} exceeds linked pool size {}; dropped",
                idx, linked_pool_size
            );
            continue;
        }
        let global_id = (linked_offset + idx) as u32;
        let off = (linked_count as usize) * 4;
        dst[off..off + 4].copy_from_slice(&global_id.to_le_bytes());
        linked_count += 1;
    }
    let linked_count_off = cap * 4;
    dst[linked_count_off..linked_count_off + 4].copy_from_slice(&linked_count.to_le_bytes());

    // Final array (cap × u32) followed by final_count (u32).
    let final_array_off = cap * 4 + 4;
    let mut final_count = 0u32;
    for &idx in t.final_attachments.iter() {
        if (final_count as usize) >= cap {
            log::warn!(
                "Transform has more than {} final attachments; truncating",
                cap
            );
            break;
        }
        if idx >= final_pool_size {
            log::warn!(
                "Final attachment index {} exceeds final pool size {}; dropped",
                idx, final_pool_size
            );
            continue;
        }
        let global_id = (final_offset + idx) as u32;
        let off = final_array_off + (final_count as usize) * 4;
        dst[off..off + 4].copy_from_slice(&global_id.to_le_bytes());
        final_count += 1;
    }
    let final_count_off = final_array_off + cap * 4;
    dst[final_count_off..final_count_off + 4].copy_from_slice(&final_count.to_le_bytes());
}

/// GPU representation of variation parameters for ONE transform.
///
/// Layout is **packed**: each active variation gets exactly
/// `parameters.len() + init_param_count` consecutive slots starting
/// at its packed offset (computed by
/// [`crate::scene::transforms::compute_packed_layout`]). The shader
/// builder generates a per-flame `get_param` that maps `variation_id`
/// (per-flame local index) to its packed offset, so host and shader
/// stay in sync.
///
/// The buffer is sized to a worst-case ceiling
/// (`MAX_VARIATION_PARAM_SLOTS = 1600` floats = 6400 bytes); flames
/// only fill the prefix they need, leaving the tail zeroed.
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct GpuVariationParams {
    pub params: [f32; MAX_VARIATION_PARAM_SLOTS],
}

// Manual implementation for bytemuck (arrays > 128 not auto-derived)
unsafe impl bytemuck::Pod for GpuVariationParams {}
unsafe impl bytemuck::Zeroable for GpuVariationParams {}

impl GpuVariationParams {
    /// Create the packed param-array for a single transform.
    ///
    /// `local_map` and `registry` together yield each active variation's
    /// packed offset (via `compute_packed_layout`), and we write each
    /// variation's user-visible parameter values starting at that
    /// offset. Init-derived slots are left at 0.0; the GPU init compute
    /// dispatch fills them in before the main render pass reads them.
    pub fn from_transform(
        xform: &Transform,
        local_map: &std::collections::HashMap<String, u32>,
        registry: &crate::variations::VariationRegistry,
    ) -> Self {
        let mut params = [0.0f32; MAX_VARIATION_PARAM_SLOTS];

        // Build name → offset lookup once per transform. Cheap; could be
        // hoisted to caller for very large flames.
        let layout = crate::scene::transforms::compute_packed_layout(local_map, registry);
        let offset_for: std::collections::HashMap<&str, u32> = layout
            .iter()
            .map(|e| (e.name.as_str(), e.offset))
            .collect();

        for (var_name, _weight) in &xform.variations {
            let offset = match offset_for.get(var_name.as_str()) {
                Some(&o) => o as usize,
                None => continue, // not active or dropped past the cap
            };
            let info = match registry.get(var_name) {
                Some(i) => i,
                None => continue,
            };

            for (param_idx, param_def) in info.parameters.iter().enumerate() {
                let value = xform.get_variation_param_or_default(
                    var_name,
                    &param_def.name,
                    registry,
                );
                let buffer_idx = offset + param_idx;
                if buffer_idx < params.len() {
                    params[buffer_idx] = value;
                }
            }
        }

        Self { params }
    }

    /// Create the full per-flame param-array vector, computing the local index
    /// map once and applying it consistently to all transforms.
    pub fn from_flame(
        flame: &Flame,
        registry: &crate::variations::VariationRegistry,
    ) -> Vec<Self> {
        let local_map = crate::scene::transforms::compute_local_index_map(
            flame.active_variation_names_ordered(registry),
        );
        // Soft cap check: warn if total slot footprint approaches the buffer.
        let total_slots = crate::scene::transforms::total_packed_slots(&local_map, registry);
        if total_slots as usize > MAX_VARIATION_PARAM_SLOTS {
            log::error!(
                "Flame has {} packed variation slots, exceeds buffer capacity {}. \
                 Some parameters will be lost. Active variations: {:?}",
                total_slots,
                MAX_VARIATION_PARAM_SLOTS,
                local_map.keys().collect::<Vec<_>>(),
            );
        }
        // Concatenated layout: normals, then linkeds, then finals.
        // Mirror of `GpuTransform::from_flame`. xform_id in the shader
        // indexes into both the transforms array and this params array
        // simultaneously, so the orderings must match.
        let mut result: Vec<Self> = Vec::with_capacity(
            flame.transforms.len()
                + flame.linked_transforms.len()
                + flame.final_transforms.len(),
        );
        for xform in &flame.transforms {
            result.push(Self::from_transform(xform, &local_map, registry));
        }
        for xform in &flame.linked_transforms {
            result.push(Self::from_transform(xform, &local_map, registry));
        }
        for xform in &flame.final_transforms {
            result.push(Self::from_transform(xform, &local_map, registry));
        }
        result
    }
}

/// Calculate bits needed per (normal) transform index based on the
/// normal-transform count. Used by the PathMap color mode's per-pixel
/// path-hash packing (`xform_idx` is the NORMAL transform's index, not
/// a global xform_id, since chaos-game selection is among normals only).
///
/// - 1-2 transforms: 1 bit
/// - 3-4 transforms: 2 bits
/// - 5-8 transforms: 3 bits
/// - 9-16 transforms: 4 bits
/// - 17-32 transforms: 5 bits
/// - 33-64 transforms: 6 bits
/// - 65-128 transforms: 7 bits
pub fn bits_per_transform(num_transforms: u32) -> u32 {
    if num_transforms <= 2 {
        1
    } else if num_transforms <= 4 {
        2
    } else if num_transforms <= 8 {
        3
    } else if num_transforms <= 16 {
        4
    } else if num_transforms <= 32 {
        5
    } else if num_transforms <= 64 {
        6
    } else {
        7  // Up to 128 transforms
    }
}

/// Dispatch parameters for compute shader
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuParams {
    pub num_transforms: u32,
    pub iterations_per_thread: u32,
    pub burn_in: u32,
    pub width: u32,
    pub height: u32,
    pub seed: u32,
    pub color_mode: u32, // 0 = palette, 1 = speed, 2 = path_map
    pub render_mode: u32, // 0 = 2D, 1 = 3D
    pub splat_size: f32,
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub rotation: f32, // Rotation in radians (2D rotation around Z)
    pub speed_factor: f32, // Blend factor for speed-based coloring
    pub perspective_strength: f32, // Strength for perspective projection
    // Depth-density compensation strength s: 3D samples weighted by
    // zr^(-2s) so apparent brightness is depth-invariant (radiance-
    // preserving splats). 0 = off. See Flame field docs.
    pub depth_density_compensation: f32,
    // Far density fade: sample density weighted by
    // exp(-(far_density_fade_start - camera_z)^2 * far_density_fade)
    // beyond the start depth. 0 = off.
    pub far_density_fade: f32,
    pub far_density_fade_start: f32,
    pub camera_rotation_x: f32, // 3D camera pitch (rotation around X axis)
    pub camera_rotation_y: f32, // 3D camera yaw (rotation around Z axis — Apo ZXY Euler)
    // JWildfire/Apophysis bank — Y-axis rotation, applied as the 3rd
    // angle in the 4-input camera matrix per
    // `createProjectionMatrix(yaw, pitch, bank, roll)`. In radians.
    pub camera_bank: f32,
    pub camera_x: f32, // 3D camera X position (world space)
    pub camera_y: f32, // 3D camera Y position (world space)
    pub camera_z: f32, // 3D camera Z position (height / world space)
    pub dof_focus_distance: f32, // Depth of field: distance where image is sharpest (default: 1.0)
    pub dof_blur_strength: f32, // Depth of field: blur amount (0.0 = disabled, default: 0.0)
    pub fog_strength: f32, // Depth fog: exponential fog density (0.0 = disabled)
    pub fog_start: f32, // Depth fog: distance where fog begins
    pub bits_per_transform: u32, // Bits needed per transform index (1-4 based on num_transforms)
    pub path_map_style: u32, // 0=Prefix, 1=Suffix, 2=PrefixDistinct, 3=SuffixDistinct
    pub path_capture_mode: u32, // 0=FirstHit, 1=FirstAfterBurnIn, 2=DeepestHit
    pub path_tracking_mode: u32, // 0=First (first 32 iterations), 1=Recent (rolling window of 32)
    pub num_path_filters: u32, // Number of active path filters (0 = disabled)
    pub min_suffix_filter_length: u32, // Minimum length among depth=0 filters (for optimization)
    pub background_r: f32, // Background color R (for depth fog)
    pub background_g: f32, // Background color G (for depth fog)
    pub background_b: f32, // Background color B (for depth fog)

    // Solid rendering (Phase 0). These three fields occupy what used to
    // be the 12-byte std140 pad before `post_symmetry` (37 scalars × 4 =
    // 148 bytes; the struct must start on a 16-byte boundary = 160), so
    // the layout is unchanged. Only read when the SOLID shader-builder
    // flag is set. Mirror in shaders/core/header.wgsl.
    pub solid_strength: f32,    // Occlusion strength: 0 = off, 1 = hard surface
    pub surface_thickness: f32, // Depth shell accepted as "the surface" (world units)
    pub depth_prime: u32,       // 1 = depth-priming batch (record depth, plot nothing)

    // Post-symmetry — plot-time density replication. Each chaos-game
    // sample also deposits at K−1 mirrored/rotated positions before the
    // camera transform. Gated by the HAS_POST_SYMMETRY shader-builder
    // flag, so when `post_symmetry.kind == 0` the symmetry block
    // doesn't compile in and these fields aren't read.
    pub post_symmetry: GpuPostSymmetry,

    // Light-space shadow maps (solid rendering Stage 2). The maps cover
    // the sphere of `shadow_radius` around `shadow_center` (frozen per
    // accumulation run from measured bounds). shadow_count = 0 disables
    // the splat at runtime (cheap branch inside the SOLID block).
    pub shadow_center_x: f32,
    pub shadow_center_y: f32,
    pub shadow_center_z: f32,
    pub shadow_radius: f32,
    pub shadow_count: u32,
    pub _pad_shadow: [u32; 3],
    // xyz = world-space direction TO each light, w unused.
    pub shadow_dirs: [[f32; 4]; 4],
}

/// Plot-time symmetry params packed for the GPU uniform. Mirrors the
/// WGSL `PostSymmetry` struct in `header.wgsl`. Built from a
/// `scene::transforms::PostSymmetry` via `From` — rotation converts
/// to radians at the boundary, order clamps to [1, 32].
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuPostSymmetry {
    /// 0 = None, 1 = XAxis, 2 = YAxis, 3 = Point.
    pub kind: u32,
    /// K for Point mode; ignored by axis modes.
    pub order: u32,
    pub center_x: f32,
    pub center_y: f32,
    /// Pan along the symmetry axis (axis modes only).
    pub distance: f32,
    /// Pre-rotation in radians (axis modes only).
    pub rotation: f32,
    /// std140 padding so the struct's tail aligns to 16 bytes — keeps
    /// it embeddable inside `GpuParams` without surprising the next
    /// field. 6 × 4 = 24 bytes → pad to 32.
    pub _pad_a: f32,
    pub _pad_b: f32,
}

impl From<&crate::scene::transforms::PostSymmetry> for GpuPostSymmetry {
    fn from(s: &crate::scene::transforms::PostSymmetry) -> Self {
        Self {
            kind: s.ty.as_u32(),
            order: s.order.clamp(1, 32),
            center_x: s.center_x,
            center_y: s.center_y,
            distance: s.distance,
            rotation: s.rotation_deg.to_radians(),
            _pad_a: 0.0,
            _pad_b: 0.0,
        }
    }
}

impl GpuPostSymmetry {
    /// Identity-ish default — `kind = 0` so the shader's gate
    /// short-circuits, all geometry fields zero.
    pub fn none() -> Self {
        Self {
            kind: 0,
            order: 1,
            center_x: 0.0,
            center_y: 0.0,
            distance: 0.0,
            rotation: 0.0,
            _pad_a: 0.0,
            _pad_b: 0.0,
        }
    }
}

/// Maximum number of path filters supported
pub const MAX_PATH_FILTERS: usize = 64;

/// GPU representation of a path filter (must match WGSL PathFilter struct)
/// Used to block specific transform sequences during iteration
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuPathFilter {
    /// Packed pattern (up to 8 iterations at 4 bits each, LSB = first)
    pub pattern: u32,
    /// Number of iterations in pattern (1-8)
    pub length: u32,
    /// 0 = suffix match (any depth), >0 = match at this exact depth
    pub depth: u32,
    /// Padding for 16-byte alignment
    pub _padding: u32,
}

impl GpuPathFilter {
    /// Create an empty (unused) filter
    pub fn empty() -> Self {
        Self {
            pattern: 0,
            length: 0,
            depth: 0,
            _padding: 0,
        }
    }

    /// Create a suffix filter (matches at any depth)
    /// pattern: array of transform indices (0-15), up to 8 elements
    pub fn suffix(pattern: &[u32]) -> Self {
        assert!(pattern.len() <= 8, "Pattern can have at most 8 elements");
        let mut packed = 0u32;
        for (i, &idx) in pattern.iter().enumerate() {
            packed |= (idx & 0xF) << (i * 4);
        }
        Self {
            pattern: packed,
            length: pattern.len() as u32,
            depth: 0, // 0 = suffix match
            _padding: 0,
        }
    }

    /// Create an exact depth filter (only matches at specific iteration depth)
    /// pattern: array of transform indices (0-15), up to 8 elements
    /// depth: the iteration count at which this pattern should match
    pub fn at_depth(pattern: &[u32], depth: u32) -> Self {
        assert!(pattern.len() <= 8, "Pattern can have at most 8 elements");
        assert!(depth >= pattern.len() as u32, "Depth must be >= pattern length");
        let mut packed = 0u32;
        for (i, &idx) in pattern.iter().enumerate() {
            packed |= (idx & 0xF) << (i * 4);
        }
        Self {
            pattern: packed,
            length: pattern.len() as u32,
            depth, // >0 = exact depth match
            _padding: 0,
        }
    }
}

/// Tonemap parameters
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TonemapParams {
    pub exposure: f32,
    pub gamma: f32,
    pub density_scale: f32, // Controls how density maps to alpha
    pub tonemap_mode: u32,  // 0 = Linear, 1 = Logarithmic
    pub background_color: [f32; 3],
    pub _pad_bg: f32,  // Padding to align vec3 to 16 bytes (std140 rule)
    pub use_curve: u32,  // 0 = disabled, 1 = enabled
    pub vibrancy: f32,  // Blend between old and new color algorithms (0.0-30.0)
    pub brightness: f32,  // Logarithmic brightness scaling (0.0-5.0, default 1.0)
    pub white_level: f32,  // Apophysis white_level constant (default 200.0)
    pub prefilter_white: f32,  // Apophysis PREFILTER_WHITE constant (67108864.0)
    pub bright_adjust: f32,  // Apophysis BRIGHT_ADJUST constant (2.3)
    pub area: f32,  // Render area (width * height)
    pub sample_density: f32,  // Iterations per pixel
    pub saturation: f32,  // Color saturation boost (1.0 = no change, >1.0 = more saturated)
    pub hue_shift: f32,  // Hue rotation in degrees (-180.0 to 180.0)
    pub gamma_threshold: f32,  // Smooths gamma curve at low densities (see DEFAULT_GAMMA_THRESHOLD)
    pub alpha_blend_low: f32,  // Start blending toward linear alpha at this gamma-corrected value
    pub alpha_blend_high: f32,  // Full linear alpha above this value
    pub transparent_mode: u32,  // 0 = normal (blend with background), 1 = transparent export
    pub color_mode: u32,  // 0 = palette, 1 = speed, 2 = path_map
    pub width: u32,  // Texture width for path buffer indexing
    pub height: u32,  // Texture height for path buffer indexing
    pub path_map_style: u32,  // 0=Prefix, 1=Suffix, 2=PrefixDistinct, 3=SuffixDistinct, 4=Depth, 5=OriginRadial, 6=OriginHorizontal, 7=OriginVertical
    pub burn_in: u32,  // Burn-in iterations (for Depth gradient: start depth)
    pub num_transforms: u32,  // Number of transforms (for path coloring entropy)
    pub palette_size: u32,  // Palette texture size (256-4096), for shader index calculations
    // Levels controls (histogram-based density remapping).
    // levels_low / levels_high are in "× mean density" units
    // (i.e., multiples of sample_density), making the slider effect
    // invariant to total iteration count.
    pub levels_low: f32,  // Density (× mean) below this → fully transparent
    pub levels_high: f32,  // Density (× mean) above this → fully opaque
    pub levels_gamma: f32,  // Gamma/midpoint for density curve (1.0 = linear)
    pub highlight_mode: u32,  // 0 = Clip (per-channel clamp, Apophysis), 1 = MaxNorm (hue-preserving)
    pub levels_enabled: u32,  // 0 = Levels off (Apo-matching), 1 = on
    pub _pad_levels: [u32; 2],  // Pad trailing chunk to 16 bytes for std140 alignment
}

impl Default for TonemapParams {
    fn default() -> Self {
        use crate::config::defaults::*;
        Self {
            exposure: DEFAULT_EXPOSURE,
            gamma: DEFAULT_GAMMA,
            density_scale: DEFAULT_DENSITY_SCALE,
            tonemap_mode: 1,  // Default to Logarithmic
            background_color: [0.0, 0.0, 0.0],
            _pad_bg: 0.0,
            use_curve: 0,  // Curves disabled by default
            vibrancy: 1.0,  // Modern vibrant colors by default
            brightness: DEFAULT_BRIGHTNESS,
            white_level: DEFAULT_WHITE_LEVEL,
            prefilter_white: PREFILTER_WHITE,
            bright_adjust: BRIGHT_ADJUST,
            area: 800.0 * 600.0,  // Default resolution
            sample_density: 1.0,  // Will be updated per frame
            saturation: DEFAULT_SATURATION,
            hue_shift: DEFAULT_HUE_SHIFT,
            gamma_threshold: DEFAULT_GAMMA_THRESHOLD,
            alpha_blend_low: DEFAULT_ALPHA_BLEND_LOW,
            alpha_blend_high: DEFAULT_ALPHA_BLEND_HIGH,
            transparent_mode: 0,  // Normal display mode
            color_mode: 0,  // Palette mode by default
            width: 800,  // Default width (will be updated per frame)
            height: 600,  // Default height (will be updated per frame)
            path_map_style: 0,  // Prefix mode by default
            burn_in: 20,  // Default burn-in
            num_transforms: 3,  // Default 3 transforms
            palette_size: 256,  // Default palette size
            levels_low: 0.0,  // No low clipping by default
            levels_high: crate::config::defaults::DEFAULT_LEVELS_HIGH,
            levels_gamma: crate::config::defaults::DEFAULT_LEVELS_GAMMA,
            highlight_mode: 0,  // Clip (Apophysis-compatible)
            levels_enabled: 0,  // Levels off — Apo-matching default
            _pad_levels: [0; 2],
        }
    }
}

/// Histogram-blur parameters (matches `BlurParams` in histogram_blur.wgsl).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct HistogramBlurParams {
    pub width: u32,
    pub height: u32,
    pub radius_pixels: f32,
    pub direction: u32,  // 0 = horizontal, 1 = vertical
    /// Bilateral weighting `σ_d` in u32-histogram density units. Taps
    /// whose density differs from the center by ≳ `density_sigma` get
    /// weight ≈ 0, preserving edges. Large value → uniform Gaussian.
    pub density_sigma: f32,
    pub _pad: [u32; 3],  // pad to 32 bytes for std140 alignment
}

/// Analytic-blur convolution-stage parameters (shared by the downsample,
/// convolve, and upscale shaders). The convolution runs at REDUCED resolution
/// (`lowres = ceil(full / downscale)`) because the blur is low-frequency — this
/// is what bounds the kernel size (and thus cost). `meta[slot]` packs
/// (lowres_half, weight_offset, _, _): the kernel half-extent at low-res and
/// the slot's first weight index into `blur_kernel_weights_buffer`. Only the
/// first `count` slots are read.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct BlurConvolveParams {
    pub full_width: u32,
    pub full_height: u32,
    pub lowres_width: u32,
    pub lowres_height: u32,
    pub downscale: u32,
    pub count: u32,
    /// Per-frame dither seed (offset 24 B). The upscale stochastically rounds
    /// its `density/D²` add using a hash of (pixel, seed); varying the seed
    /// each frame makes the rounding error average out across accumulation
    /// frames → smooth, band-free, no residual noise. Written every frame.
    pub frame_seed: u32,
    pub _pad1: u32,
    /// Per-slot (lowres_half, weight_offset, _, _) — std140 array stride is
    /// 16 B, so each entry is a `vec4<u32>`.
    pub meta: [[u32; 4]; MAX_BLUR_BUFFERS as usize],
}

/// Accumulation parameters
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AccumulateParams {
    pub width: u32,
    pub height: u32,
    pub blend_factor: f32,
    /// Mode flag — 0 = cumulative-mean (algorithmic ideal,
    /// precision-limited around 10^9 iters/pixel on f32), 1 = fixed
    /// EMA at `blend_factor` (precision-stable, dim early frames as
    /// the EMA bootstraps from 0). Mirrors `accumulate.wgsl`'s
    /// `use_fixed_ema` field.
    pub use_fixed_ema: u32,
    pub background_r: f32,  // Background color RGB (for blending when no samples)
    pub background_g: f32,
    pub background_b: f32,
    pub _pad1: f32,
    /// Solid rendering depth-tightening reset (see accumulate.wgsl):
    /// discard a pixel's accumulated history when its nearest depth
    /// moves closer by more than ~1.5× this.
    pub surface_thickness: f32,
    /// 1 when the histogram carries the depth region AND accum_depth
    /// is a real buffer.
    pub has_depth: u32,
    pub _pad2: [u32; 2],
}

/// Manages GPU buffers and textures for fractal flame rendering
pub struct FlameBuffers {
    pub transform_buffer: Buffer,
    pub variation_params_buffer: Buffer,  // Parameter buffer for variations
    /// Per-normal-transform attachment list buffer:
    /// `array<AttachmentList, MAX_TRANSFORMS>`. Indexed by the normal's
    /// xform_id (0..flame.transforms.len()). Each entry holds up to
    /// `flame.attachment_cap()` linked + cap final GLOBAL xform_ids
    /// (plus counts) — the per-flame cap is substituted into the
    /// shader struct via `{{ATTACHMENT_CAP}}` and matched by
    /// `update_attachments`'s packing stride. Backing storage is
    /// always allocated at the worst-case `MAX_ATTACHMENTS_PER_TRANSFORM`
    /// stride; smaller per-flame caps just write fewer bytes per slot.
    /// See `docs/projects/per-transform-linked-and-final.md`.
    pub attachments_buffer: Buffer,

    /// `array<SubflameMeta, MAX_SUBFLAMES>` uniform with per-subflame
    /// transform-pool offsets + counts. Indexed by `subflame_id` (the
    /// variation parameter). Unused slots remain zero.
    pub subflame_metadata_buffer: Buffer,

    pub params_buffer: Buffer,
    pub tonemap_params_buffer: Buffer,
    pub accumulate_params_buffer: Buffer,
    // Two uniform buffers, one per direction. `queue.write_buffer` orders
    // are flattened before any encoded dispatch executes, so a single
    // buffer with rewrites between dispatches would race — both passes
    // would see the latest value.
    pub histogram_blur_params_buffer_h: Buffer,
    pub histogram_blur_params_buffer_v: Buffer,

    // Dual textures for ping-pong accumulation
    pub accumulation_texture_a: Texture,
    pub accumulation_texture_b: Texture,
    pub accumulation_view_a: TextureView,
    pub accumulation_view_b: TextureView,

    // Histogram storage buffer for atomic color accumulation (within-frame)
    // Layout: [r, g, b, density] × (width × height) as u32 array.
    // When `solid_depth_region` is set, one extra u32 per pixel follows at
    // offset W*H*4 — the solid-rendering nearest-depth region (inverted
    // ordered-float encoding, 0 = no sample; see main_template.wgsl SOLID).
    pub histogram_buffer: Buffer,
    // Second histogram buffer used as the spatial-filter scratch — when
    // `filter_radius > 0`, the histogram-blur pass writes the horizontal
    // Gaussian pass here and the vertical pass writes back to the primary
    // buffer that accumulate.wgsl reads. Always RGBD-sized (4 words/px):
    // the blur never touches the depth region.
    pub histogram_buffer_scratch: Buffer,
    // Whether histogram_buffer carries the solid-rendering depth region.
    pub solid_depth_region: bool,
    // Solid rendering: per-pixel encoded depth the ACCUMULATOR's data
    // belongs to (accumulate.wgsl's depth-tightening reset). Allocated
    // with the depth region; 0-filled = "no data yet".
    pub accum_depth_buffer: Option<Buffer>,

    // Per-pixel iteration count buffer for convergence tracking
    // Per-pixel path buffer for PathMap color mode (OPTIONAL)
    // Layout: 7× u32 per pixel (PathEntry struct)
    // Path is packed MSB-first: transform indices stored from high bits down
    // Last-write-wins semantics (not accumulated)
    // None when path features are disabled to save ~58MB at 1920×1080
    pub path_buffer: Option<Buffer>,

    // Path filter buffer for blocking specific transform sequences (OPTIONAL)
    // Layout: MAX_PATH_FILTERS × GpuPathFilter (16 bytes each)
    // None when path features are disabled
    pub path_filter_buffer: Option<Buffer>,

    // Dummy buffers for binding when path features are disabled
    // WebGPU requires all bindings to be present, so we bind minimal buffers when disabled
    pub dummy_path_buffer: Buffer,
    pub dummy_filter_buffer: Buffer,

    // Xaos (chaos) transition weights buffer (OPTIONAL)
    // Layout: N × N matrix where N = num_transforms
    // Index: xaos_weights[src * num_transforms + dst]
    // None when all xaos weights are 1.0 (default behavior)
    pub xaos_buffer: Option<Buffer>,
    pub dummy_xaos_buffer: Buffer,

    // Analytic-blur per-transform mean-splat buffers, allocated at LOW
    // resolution (`ceil(W/D)×ceil(H/D) × 4 × u32 × slots`). The chaos game
    // splats means straight to low res (mean ÷ D) — there is no full-res blur
    // buffer or downsample pass. Reallocated when the downscale D changes
    // (rare, discrete steps; never on exports). None when inactive (the dummy
    // is bound). See `ensure_lowres_blur_buffers` and
    // docs/projects/analytic-blur-buffer.md.
    //   blur_splat_buffer     — low-res atomic splat target + convolve input
    //   blur_convolved_buffer — convolve output / upscale input
    pub blur_splat_buffer: Option<Buffer>,
    pub blur_convolved_buffer: Option<Buffer>,
    pub dummy_blur_buffer: Buffer,
    // Number of blur slices actually ALLOCATED (after the memory cap). The
    // shader gates `slot < count`, so an eligible transform beyond this falls
    // back to the stochastic path. 0 when inactive.
    pub blur_buffer_count: u32,
    // Low-res dims the current blur buffers were allocated for (so a D change
    // triggers a realloc). 0 when no buffers.
    pub blur_lowres_w: u32,
    pub blur_lowres_h: u32,
    // Concatenated per-slot convolution-kernel weights (host-built each time
    // the view/flame changes; see FlameRenderer::maybe_rebuild_blur_kernels).
    // Sized for the worst case so it never reallocates. Always present (a
    // 1-float dummy when the feature is off) so the convolve bind group is
    // stable.
    pub blur_kernel_weights_buffer: Buffer,
    // Convolve params uniform (width, height, slot count, per-slot kernel
    // half-extent + weight offset). Always present.
    pub blur_convolve_params_buffer: Buffer,
    // Device's max single storage-buffer binding size, captured at creation.
    // Bounds how many full-res analytic-blur slices (each `W·H·16` bytes ×3
    // buffers) we can allocate before exceeding a binding / OOMing — see
    // `max_blur_slots`.
    pub max_binding_size: u64,

    // Per-pixel scale buffer for adaptive histogram scaling
    // Note: scale_buffer removed - now using params.histogram_color_scale (global uniform)

    // Palette texture (1D)
    pub palette_texture: Texture,
    pub palette_view: TextureView,

    // Tone curve LUT texture (1D, 256 samples)
    pub curve_lut_texture: Texture,
    pub curve_lut_view: TextureView,

    pub sampler: Sampler,
    pub curve_lut_sampler: Sampler,  // Separate sampler for curve LUT (nearest neighbor)

    // Track which texture is current for display
    pub current_is_a: bool,

    // Track current dimensions for path buffer recreation
    width: u32,
    height: u32,

    // Track current palette size for dynamic palette texture
    palette_size: u32,
}

/// Default palette size (256 colors for backward compatibility)
pub const DEFAULT_PALETTE_SIZE: u32 = 256;

/// Maximum supported palette size
pub const MAX_PALETTE_SIZE: u32 = 4096;

impl FlameBuffers {
    /// Explicitly release every GPU buffer and texture this owns.
    ///
    /// On WebGPU, dropping the Rust handles only marks the underlying
    /// `GPUBuffer`/`GPUTexture` objects for JS garbage collection — the GPU
    /// memory stays allocated until the browser GCs them, which lags well
    /// behind back-to-back work. A throwaway export renderer at 8000² holds
    /// ~4 GB (two Rgba32Float accumulation textures + two u32 histogram
    /// buffers, plus the rest), so repeated large WASM exports pile gigabytes
    /// onto the live device until allocation fails — silently, producing
    /// all-black output. `destroy()` frees each resource synchronously.
    ///
    /// Call once when the buffers are no longer in use (all GPU work that
    /// reads them has completed); the owning `FlameBuffers` should be dropped
    /// afterward.
    pub fn destroy(&self) {
        // Large consumers first.
        self.accumulation_texture_a.destroy();
        self.accumulation_texture_b.destroy();
        self.histogram_buffer.destroy();
        self.histogram_buffer_scratch.destroy();
        // Optional / feature buffers.
        if let Some(b) = &self.path_buffer { b.destroy(); }
        if let Some(b) = &self.path_filter_buffer { b.destroy(); }
        if let Some(b) = &self.xaos_buffer { b.destroy(); }
        if let Some(b) = &self.accum_depth_buffer { b.destroy(); }
        if let Some(b) = &self.blur_splat_buffer { b.destroy(); }
        if let Some(b) = &self.blur_convolved_buffer { b.destroy(); }
        // Small uniform/param/dummy buffers + 1D LUT textures (KB each, but
        // tidy them too so nothing is left dangling on the device).
        self.transform_buffer.destroy();
        self.variation_params_buffer.destroy();
        self.attachments_buffer.destroy();
        self.subflame_metadata_buffer.destroy();
        self.params_buffer.destroy();
        self.tonemap_params_buffer.destroy();
        self.accumulate_params_buffer.destroy();
        self.histogram_blur_params_buffer_h.destroy();
        self.histogram_blur_params_buffer_v.destroy();
        self.dummy_path_buffer.destroy();
        self.dummy_filter_buffer.destroy();
        self.dummy_xaos_buffer.destroy();
        self.dummy_blur_buffer.destroy();
        self.blur_kernel_weights_buffer.destroy();
        self.blur_convolve_params_buffer.destroy();
        self.palette_texture.destroy();
        self.curve_lut_texture.destroy();
    }

    /// Free only the large per-iteration GPU resources — the histogram buffers,
    /// the accumulation ping-pong, the sample/path/blur buffers — while keeping
    /// the small palette/curve/param resources. ~3-4 GB at 8000². Used after
    /// tonemap on memory-constrained (WASM) exports so a following full-res
    /// color-effect ping-pong has VRAM headroom: these buffers aren't needed
    /// once tonemap has written the renderer's fractal texture (which lives on
    /// FlameRenderer and is left intact). Call ONLY after the tonemap's GPU work
    /// has completed; the renderer must not iterate or tonemap again after this.
    pub fn free_iteration_buffers(&self) {
        self.histogram_buffer.destroy();
        self.histogram_buffer_scratch.destroy();
        self.accumulation_texture_a.destroy();
        self.accumulation_texture_b.destroy();
        if let Some(b) = &self.path_buffer { b.destroy(); }
        if let Some(b) = &self.blur_splat_buffer { b.destroy(); }
        if let Some(b) = &self.blur_convolved_buffer { b.destroy(); }
    }

    /// Create new FlameBuffers with default palette size (256)
    pub fn new(device: &Device, queue: &Queue, width: u32, height: u32, flame: &Flame) -> Self {
        Self::with_palette_size(device, queue, width, height, flame, DEFAULT_PALETTE_SIZE)
    }

    /// Create new FlameBuffers with specified palette size
    pub fn with_palette_size(device: &Device, queue: &Queue, width: u32, height: u32, flame: &Flame, palette_size: u32) -> Self {
        // Clamp palette size to valid range
        let palette_size = palette_size.clamp(DEFAULT_PALETTE_SIZE, MAX_PALETTE_SIZE);
        // Create transform storage buffer sized for the unified
        // xform_id space: parent xforms in [0, MAX_TRANSFORMS),
        // subflame xforms in [MAX_TRANSFORMS, MAX_TRANSFORMS_UNIFIED).
        // The subflame iteration shader reads xform data via
        // `transforms[xform_id]` (where xform_id is the unified id),
        // and per-xform variation lookups (transforms[xform_id].
        // variations[var_id]) now resolve for subflame xforms too.
        let buffer_size = (MAX_TRANSFORMS_UNIFIED * std::mem::size_of::<GpuTransform>()) as u64;
        let transform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Transform Buffer"),
            size: buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create variation parameters storage buffer sized for
        // `MAX_TRANSFORMS_UNIFIED` — the unified xform_id space
        // covering parent xforms in [0, MAX_TRANSFORMS) plus subflame
        // xforms in [MAX_TRANSFORMS, MAX_TRANSFORMS_UNIFIED). The
        // buffer is populated below via update_variation_params, which
        // walks parent xforms then subflame xforms in the same
        // contiguous order.
        let params_buffer_size = (MAX_TRANSFORMS_UNIFIED * std::mem::size_of::<GpuVariationParams>()) as u64;
        let variation_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Variation Params Buffer"),
            size: params_buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Per-normal-transform attachment list buffer (binding 10).
        // One GpuAttachmentList entry per normal transform slot;
        // MAX_TRANSFORMS entries pre-allocated, populated via
        // update_attachments. See per-transform-linked-and-final.md.
        let attachments_buffer_size = (MAX_TRANSFORMS * std::mem::size_of::<GpuAttachmentList>()) as u64;
        let attachments_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Attachments Buffer"),
            size: attachments_buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create params uniform buffer
        let params = GpuParams {
            num_transforms: flame.transforms.len() as u32,
            iterations_per_thread: 2048, // More iterations for better coverage per frame
            burn_in: 20,
            width,
            height,
            seed: 12345,
            color_mode: 0, // Default to transform colors
            render_mode: 0, // Default to 2D
            splat_size: 1.0,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            rotation: 0.0,
            speed_factor: 0.5,
            perspective_strength: 2.0,
            depth_density_compensation: 0.0,
            far_density_fade: 0.0,
            far_density_fade_start: 0.0,
            camera_rotation_x: 0.0,
            camera_rotation_y: 0.0,
            camera_bank: 0.0,

            camera_x: 0.0,

            camera_y: 0.0,
            camera_z: 0.0,
            dof_focus_distance: crate::config::DEFAULT_DOF_FOCUS_DISTANCE,
            dof_blur_strength: crate::config::DEFAULT_DOF_BLUR_STRENGTH,
            fog_strength: crate::config::DEFAULT_FOG_STRENGTH,
            fog_start: crate::config::DEFAULT_FOG_START,
            bits_per_transform: bits_per_transform(flame.transforms.len() as u32),
            path_map_style: 0,
            path_capture_mode: 0, // FirstHit by default
            path_tracking_mode: 0, // First (first 32 iterations) by default
            num_path_filters: 0, // No filters by default
            min_suffix_filter_length: 0, // No filters by default
            background_r: 0.0,
            background_g: 0.0,
            background_b: 0.0,
            solid_strength: 0.0,
            surface_thickness: crate::config::DEFAULT_SURFACE_THICKNESS,
            depth_prime: 0,
            post_symmetry: GpuPostSymmetry::none(),
            shadow_center_x: 0.0,
            shadow_center_y: 0.0,
            shadow_center_z: 0.0,
            shadow_radius: 1.0,
            shadow_count: 0,
            _pad_shadow: [0; 3],
            shadow_dirs: [[0.0; 4]; 4],
        };

        let params_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Params Buffer"),
            contents: bytemuck::cast_slice(&[params]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        // Create tonemap params buffer
        let tonemap_params = TonemapParams::default();
        let tonemap_params_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Tonemap Params Buffer"),
            contents: bytemuck::cast_slice(&[tonemap_params]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        // Create accumulate params buffer
        let accumulate_params = AccumulateParams {
            width,
            height,
            blend_factor: 1.0,
            use_fixed_ema: 0, // Default: cumulative-mean
            background_r: 0.0,  // Default black background
            background_g: 0.0,
            background_b: 0.0,
            _pad1: 0.0,
            surface_thickness: 0.0,
            has_depth: 0,
            _pad2: [0; 2],
        };
        let accumulate_params_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Accumulate Params Buffer"),
            contents: bytemuck::cast_slice(&[accumulate_params]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        // Histogram-blur params, one per direction. `direction` is set
        // once at construction; only width/height/radius_pixels get
        // rewritten between batches via `update_histogram_blur_params`.
        let histogram_blur_params_buffer_h = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Histogram Blur Params Buffer (H)"),
            contents: bytemuck::cast_slice(&[HistogramBlurParams { width, height, radius_pixels: 0.0, direction: 0, density_sigma: f32::INFINITY, _pad: [0; 3] }]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });
        let histogram_blur_params_buffer_v = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Histogram Blur Params Buffer (V)"),
            contents: bytemuck::cast_slice(&[HistogramBlurParams { width, height, radius_pixels: 0.0, direction: 1, density_sigma: f32::INFINITY, _pad: [0; 3] }]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        // Helper function to create accumulation texture
        let create_accum_texture = |label: &str| {
            // WASM needs RENDER_ATTACHMENT for clear_texture_wasm() to work
            // Desktop uses CLEAR_TEXTURE feature instead
            #[cfg(target_arch = "wasm32")]
            let usage = TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC | TextureUsages::RENDER_ATTACHMENT;

            #[cfg(not(target_arch = "wasm32"))]
            let usage = TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC;

            let texture = device.create_texture(&TextureDescriptor {
                label: Some(label),
                size: Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                // Rgba32Float (was Rgba16Float pre-Phase-8c). f16's
                // ~11-bit mantissa caps cumulative-mean precision: as
                // sample count grows, each new batch's per-pixel
                // contribution shrinks below f16's smallest
                // representable change and gets quantized to zero, so
                // the image stops improving past a precision floor
                // independent of statistical noise. f32's 23-bit
                // mantissa pushes that floor far enough out that
                // statistical noise (Monte Carlo 1/√N) dominates
                // through any practical iteration count. Costs 2× the
                // accumulation buffer's GPU memory.
                format: TextureFormat::Rgba32Float,
                usage,
                view_formats: &[],
            });
            let view = texture.create_view(&TextureViewDescriptor::default());
            (texture, view)
        };

        // Create dual accumulation textures for ping-pong
        let (accumulation_texture_a, accumulation_view_a) = create_accum_texture("Accumulation Texture A");
        let (accumulation_texture_b, accumulation_view_b) = create_accum_texture("Accumulation Texture B");

        // Create histogram storage buffer for atomic color accumulation
        // Buffer layout: 4× u32 per pixel (unpacked, no bit manipulation needed)
        //   u32[0]: R (full u32)
        //   u32[1]: G (full u32)
        //   u32[2]: B (full u32)
        //   u32[3]: Density (full u32)
        // Size: width × height × 4 × sizeof(u32)
        // Memory: ~7.7MB @ 800×600 (was ~5.8MB with packed format)
        let histogram_buffer_size = (width * height * 4 * std::mem::size_of::<u32>() as u32) as u64;
        let histogram_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Histogram Buffer"),
            size: histogram_buffer_size,
            // COPY_SRC to match the reallocating path below, which has
            // carried it since the solid-rendering bounds tail needed
            // reading back. Without it here, whether the histogram can
            // be read depends on whether the window has been resized
            // yet — and the numerical probe reads it straight after
            // construction. A usage flag costs nothing at runtime.
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let histogram_buffer_scratch = device.create_buffer(&BufferDescriptor {
            label: Some("Histogram Buffer (scratch for spatial filter)"),
            size: histogram_buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Path buffers are optional - only created when path features are enabled
        // See create_path_buffers() and drop_path_buffers() methods
        // Initially None to save memory (~58MB at 1920×1080)
        let path_buffer: Option<Buffer> = None;
        let path_filter_buffer: Option<Buffer> = None;

        // Create minimal dummy buffers for binding when path features are disabled
        // WebGPU requires all declared bindings to be bound, even if unused
        // Path buffer: 28 bytes minimum (PathEntry = 7 × u32)
        // Filter buffer: 16 bytes minimum (GpuPathFilter = 4 × u32)
        let dummy_path_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Dummy Path Buffer"),
            size: 28,  // PathEntry size: 7 × sizeof(u32) = 28 bytes
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let dummy_filter_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Dummy Filter Buffer"),
            size: 16,  // GpuPathFilter size: 4 × sizeof(u32) = 16 bytes
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Dummy xaos buffer for binding when xaos is disabled
        // Minimum size: 4 bytes (single f32)
        let dummy_xaos_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Dummy Xaos Buffer"),
            size: 4,  // Single f32
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Bound at binding 13 when the analytic-blur feature is inactive.
        let dummy_blur_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Dummy Blur Histogram Buffer"),
            size: 16,  // 4 atomic<u32>
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Analytic-blur convolution kernel weights — sized for the worst case
        // (every slot at the max half-extent) so the host never reallocates
        // it; only a prefix is uploaded per build. Always present so the
        // convolve bind group is stable even when the feature is off.
        let max_kernel_len = {
            let side = (2 * crate::variations::analytic_blur::MAX_KERNEL_HALF as u64) + 1;
            side * side * MAX_BLUR_BUFFERS as u64
        };
        let blur_kernel_weights_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Blur Kernel Weights Buffer"),
            size: max_kernel_len * std::mem::size_of::<f32>() as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let blur_convolve_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Blur Convolve Params Buffer"),
            size: std::mem::size_of::<BlurConvolveParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Note: scale_buffer removed - now using params.histogram_color_scale (global uniform)

        // Create palette texture (1D, dynamic size: 256-4096 samples)
        // Use Rgba8Unorm for efficient, standard color storage
        let default_palette = Palette::fire(); // Default palette
        let palette_data = default_palette.generate_texture_data(palette_size as usize);

        // Convert f32 [0.0, 1.0] to u8 [0, 255] for Rgba8Unorm
        let palette_data_u8: Vec<u8> = palette_data
            .iter()
            .map(|&v| (v.clamp(0.0, 1.0) * 255.0) as u8)
            .collect();

        let palette_texture = device.create_texture(&TextureDescriptor {
            label: Some("Palette Texture"),
            size: Extent3d {
                width: palette_size,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload palette data
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &palette_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &palette_data_u8,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(palette_size * 4), // N pixels * 4 components * 1 byte
                rows_per_image: None,
            },
            Extent3d {
                width: palette_size,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let palette_view = palette_texture.create_view(&TextureViewDescriptor::default());

        // Create curve LUT texture (1D, 256 samples) - start with linear curve
        let default_curve = crate::scene::tonemap::ToneCurve::linear();
        let curve_lut_data = default_curve.generate_lut();

        let curve_lut_texture = device.create_texture(&TextureDescriptor {
            label: Some("Curve LUT Texture"),
            size: Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba16Float,  // Use 16-bit float for precision (filterable)
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload curve LUT data
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &curve_lut_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &curve_lut_data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: None,  // 1D textures don't have rows
                rows_per_image: None,
            },
            Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let curve_lut_view = curve_lut_texture.create_view(&TextureViewDescriptor::default());

        // Create sampler for tonemap shader (accumulation texture - needs linear filtering)
        // Non-filtering sampler. Pre-Phase-8c this used Linear so the
        // tonemap fragment shader could `textureSample` the f16
        // accumulation texture; the format is now Rgba32Float and
        // f32 textures aren't filterable without the
        // FLOAT32_FILTERABLE feature, so the tonemap shader switched
        // to `textureLoad` instead. The sampler is kept in the bind
        // group for layout compatibility (the binding is declared in
        // tonemap.wgsl) but isn't actually used.
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Accumulation Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Nearest,
            min_filter: FilterMode::Nearest,
            mipmap_filter: MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // Create separate sampler for curve LUT (linear filtering for smooth interpolation)
        let curve_lut_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Curve LUT Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: MipmapFilterMode::Nearest,
            ..Default::default()
        });

        // Subflame metadata: `array<SubflameMeta>` storage buffer with
        // per-subflame offsets + counts. Indexed by `subflame_id` (the
        // variation parameter). All-zero by default. Storage rather
        // than uniform so the WGSL array is runtime-sized — same
        // access pattern as the other bindings in this layout.
        let subflame_metadata_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Subflame Metadata Buffer"),
            size: (MAX_SUBFLAMES * std::mem::size_of::<SubflameMeta>()) as u64,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let buffers = Self {
            transform_buffer,
            variation_params_buffer,
            attachments_buffer,
            subflame_metadata_buffer,
            params_buffer,
            tonemap_params_buffer,
            accumulate_params_buffer,
            histogram_blur_params_buffer_h,
            histogram_blur_params_buffer_v,
            accumulation_texture_a,
            accumulation_texture_b,
            accumulation_view_a,
            accumulation_view_b,
            histogram_buffer,
            histogram_buffer_scratch,
            solid_depth_region: false,
            accum_depth_buffer: None,
            path_buffer,
            path_filter_buffer,
            dummy_path_buffer,
            dummy_filter_buffer,
            xaos_buffer: None,  // Created on demand when xaos is used
            dummy_xaos_buffer,
            blur_splat_buffer: None,  // Created on demand when analytic blur is active
            blur_convolved_buffer: None,
            dummy_blur_buffer,
            blur_buffer_count: 0,
            blur_lowres_w: 0,
            blur_lowres_h: 0,
            blur_kernel_weights_buffer,
            blur_convolve_params_buffer,
            max_binding_size: device.limits().max_storage_buffer_binding_size as u64,
            // scale_buffer removed - using params.histogram_color_scale instead
            palette_texture,
            palette_view,
            curve_lut_texture,
            curve_lut_view,
            sampler,
            curve_lut_sampler,
            current_is_a: true,
            width,
            height,
            palette_size,
        };

        // Populate transform / variation-params buffers via the exact same
        // code path used by every subsequent update (load_config, update_flame).
        // Single source of truth — no risk of the initial write disagreeing
        // with the steady-state write.
        // Bootstrap: render_mode is config-level (v3) and unavailable here;
        // default to 2D. Real mode lands via the renderer's update_flame /
        // config-load path, which repacks if it differs.
        buffers.update_transforms(queue, flame, crate::scene::transforms::RenderMode::TwoD);
        buffers.update_variation_params(queue, flame);
        buffers.update_attachments(queue, flame, flame.attachment_cap());
        // Populate subflame buffers too. Without this, a flame whose
        // parent uses `subflame_wf` reads zeroes from the metadata
        // buffer on the first frame after construction — including
        // after a window resize, which recreates FlameBuffers via this
        // same path. The result was a chaos game stuck at the origin
        // (single bright pixel at center). Errors from this call are
        // logged but non-fatal: subflame data being absent just means
        // subflame_wf renders as zero output, which is the same as
        // the buffer's zero-initialized state.
        if let Err(e) = buffers.update_subflames(queue, &flame.subflames, &flame.get_id_mapping()) {
            log::error!("Failed to initialize subflame buffers: {}", e);
        }

        buffers
    }

    /// Get the current palette size
    pub fn palette_size(&self) -> u32 {
        self.palette_size
    }

    /// Clear all accumulation buffers
    pub fn clear_all(&self, encoder: &mut CommandEncoder, _queue: &Queue) {
        // Use CLEAR_TEXTURE feature if available (desktop usually has it)
        #[cfg(not(target_arch = "wasm32"))]
        {
            let range = ImageSubresourceRange {
                aspect: TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            };
            encoder.clear_texture(&self.accumulation_texture_a, &range);
            encoder.clear_texture(&self.accumulation_texture_b, &range);
        }

        // WASM: Clear textures by rendering black to them
        // This is more compatible than CLEAR_TEXTURE which may not be supported
        #[cfg(target_arch = "wasm32")]
        {
            self.clear_texture_wasm(encoder, &self.accumulation_view_a);
            self.clear_texture_wasm(encoder, &self.accumulation_view_b);
        }

        // Clear histogram buffer (zero out all pixels)
        // Note: This is done via queue.write_buffer to avoid encoder ordering issues
        encoder.clear_buffer(&self.histogram_buffer, 0, None);
        if let Some(ref ad) = self.accum_depth_buffer {
            encoder.clear_buffer(ad, 0, None);
        }

        // Clear path buffer (zero out all paths) - only if enabled
        if let Some(ref path_buffer) = self.path_buffer {
            encoder.clear_buffer(path_buffer, 0, None);
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn clear_texture_wasm(&self, encoder: &mut CommandEncoder, texture_view: &TextureView) {
        // Use a render pass to clear the texture
        // This is more compatible than clear_texture() which may not work in WebGPU
        // Clear to all zeros: (0,0,0,0) - zero RGB color and zero alpha (density)
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Clear Texture Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: texture_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    }),
                    store: StoreOp::Store,
                },
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        drop(render_pass); // End the render pass immediately
    }

    /// Light-space shadow map resolution per axis (4 maps ride the
    /// solid histogram's tail — 16 MB total at 1024).
    pub const SHADOW_MAP_RES: u32 = 1024;

    /// Word offset of the shadow-map region inside the solid histogram
    /// (after RGBD + depth region + 8-word bounds tail).
    pub fn shadow_map_word_offset(&self) -> u32 {
        self.width * self.height * 5 + 8
    }

    /// Bytes of the RGBD portion of the histogram (4 u32 per pixel).
    fn histogram_rgbd_bytes(&self) -> u64 {
        (self.width as u64) * (self.height as u64) * 4 * std::mem::size_of::<u32>() as u64
    }

    /// Ensure the histogram buffer does / does not carry the per-pixel depth
    /// region solid rendering needs (one extra u32 per pixel at offset
    /// W*H*4). Recreates the buffer when the requirement changes — the
    /// CALLER must then recreate every bind group referencing the histogram
    /// (compute, accumulate, histogram-blur H/V) and reset accumulation.
    /// Returns true when the buffer was recreated. No-op (and no cost) when
    /// the state already matches, so non-solid flames keep today's exact
    /// allocation.
    pub fn set_solid_depth_region(&mut self, device: &Device, enabled: bool) -> bool {
        if self.solid_depth_region == enabled {
            return false;
        }
        let words: u64 = if enabled { 5 } else { 4 };
        // Tail regions when the depth region exists:
        //  - 8 words: subsampled attractor-bounds atomics (6 used) for
        //    the shadow-map auto-fit;
        //  - 4 light-space shadow maps (SHADOW_MAP_RES² words each,
        //    ordered-float atomicMax depth toward each light) — shadows
        //    at SPLAT resolution, written by the main pass, read by the
        //    shade pass, cleared with the depth region.
        let tail: u64 = if enabled {
            32 + 4 * (Self::SHADOW_MAP_RES as u64).pow(2) * 4
        } else {
            0
        };
        let size = (self.width as u64) * (self.height as u64) * words * std::mem::size_of::<u32>() as u64 + tail;
        self.histogram_buffer = device.create_buffer(&BufferDescriptor {
            label: Some(if enabled {
                "Histogram Buffer (+solid depth region)"
            } else {
                "Histogram Buffer"
            }),
            size,
            // COPY_SRC: the bounds tail is read back for the shadow-map
            // auto-fit when the depth region exists.
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        // The accumulator's depth-ownership tracker lives and dies with
        // the depth region (accumulate.wgsl depth-tightening reset).
        if let Some(b) = self.accum_depth_buffer.take() {
            b.destroy();
        }
        if enabled {
            self.accum_depth_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Accumulator Depth Buffer"),
                size: (self.width as u64) * (self.height as u64) * 4,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        self.solid_depth_region = enabled;
        true
    }

    /// Clear histogram buffer only (before each batch for proper accumulation math).
    ///
    /// `reset_depth`: also clear the solid depth region. Pass true in
    /// OVERWRITE mode (interactive param drags) — the fractal is changing
    /// frame to frame, and a persistent depth region would occlusion-test
    /// new samples against the OLD shape's surface (dark patches wherever
    /// the new shape sits deeper). Depth is then single-batch, consistent
    /// with the single-batch image overwrite mode displays. Pass false for
    /// progressive accumulation, where depth persists across batches and
    /// the occlusion test tightens as the run converges.
    pub fn clear_histogram(&self, encoder: &mut CommandEncoder, reset_depth: bool) {
        if self.solid_depth_region && !reset_depth {
            // RGBD only — the depth region survives this batch boundary.
            // Full resets go through clear_all / clear_histogram_and_paths,
            // which zero everything — 0 is the depth encoding's "no sample"
            // sentinel, so a plain zero clear is a correct depth reset.
            encoder.clear_buffer(&self.histogram_buffer, 0, Some(self.histogram_rgbd_bytes()));
        } else {
            encoder.clear_buffer(&self.histogram_buffer, 0, None);
        }
        // Analytic-blur low-res splat buffer is per-batch too: the convolution
        // folds it into the main histogram each frame, so it must start each
        // batch empty alongside it.
        if let Some(ref blur) = self.blur_splat_buffer {
            encoder.clear_buffer(blur, 0, None);
        }
        if reset_depth {
            if let Some(ref ad) = self.accum_depth_buffer {
                encoder.clear_buffer(ad, 0, None);
            }
        }
    }

    /// Clear path buffer only (on full reset: view change, flame change, etc.)
    pub fn clear_paths(&self, encoder: &mut CommandEncoder) {
        if let Some(ref path_buffer) = self.path_buffer {
            encoder.clear_buffer(path_buffer, 0, None);
        }
    }

    /// Clear histogram and path buffers (convenience method for full reset)
    pub fn clear_histogram_and_paths(&self, encoder: &mut CommandEncoder) {
        encoder.clear_buffer(&self.histogram_buffer, 0, None);
        if let Some(ref ad) = self.accum_depth_buffer {
            encoder.clear_buffer(ad, 0, None);
        }
        if let Some(ref path_buffer) = self.path_buffer {
            encoder.clear_buffer(path_buffer, 0, None);
        }
    }

    // Note: reset_scale_buffer() removed - scale is now a uniform constant

    /// Get the current accumulation texture view (for display)
    pub fn current_accumulation_view(&self) -> &TextureView {
        if self.current_is_a {
            &self.accumulation_view_a
        } else {
            &self.accumulation_view_b
        }
    }

    /// Get the current accumulation texture (for copy operations)
    pub fn current_accumulation_texture(&self) -> &Texture {
        if self.current_is_a {
            &self.accumulation_texture_a
        } else {
            &self.accumulation_texture_b
        }
    }

    /// Get the previous accumulation texture view (for reading in accumulation shader)
    /// This reads from the CURRENT buffer (what was last displayed/accumulated)
    pub fn previous_accumulation_view(&self) -> &TextureView {
        if self.current_is_a {
            &self.accumulation_view_a
        } else {
            &self.accumulation_view_b
        }
    }

    /// Get the output accumulation texture view (for writing in accumulation shader)
    /// This writes to the BACK buffer (not current), which becomes current after swap
    pub fn output_accumulation_view(&self) -> &TextureView {
        if self.current_is_a {
            &self.accumulation_view_b
        } else {
            &self.accumulation_view_a
        }
    }

    /// Swap which texture is current
    pub fn swap_textures(&mut self) {
        self.current_is_a = !self.current_is_a;
    }

    /// Update parameters (e.g., when changing resolution or settings)
    pub fn update_params(&self, queue: &Queue, params: &GpuParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[*params]));
    }

    /// Update tonemap parameters
    pub fn update_tonemap_params(&self, queue: &Queue, params: &TonemapParams) {
        queue.write_buffer(&self.tonemap_params_buffer, 0, bytemuck::cast_slice(&[*params]));
    }

    /// Update accumulate parameters
    pub fn update_accumulate_params(&self, queue: &Queue, params: &AccumulateParams) {
        queue.write_buffer(&self.accumulate_params_buffer, 0, bytemuck::cast_slice(&[*params]));
    }

    /// Update width/height/radius/density_sigma for both blur-params buffers
    /// in one shot. `direction` is fixed at construction (H=0, V=1) and not
    /// overwritten — each buffer keeps its own direction so the two passes
    /// are race-free.
    pub fn update_histogram_blur_params(&self, queue: &Queue, width: u32, height: u32, radius_pixels: f32, density_sigma: f32) {
        let h = HistogramBlurParams { width, height, radius_pixels, direction: 0, density_sigma, _pad: [0; 3] };
        let v = HistogramBlurParams { width, height, radius_pixels, direction: 1, density_sigma, _pad: [0; 3] };
        queue.write_buffer(&self.histogram_blur_params_buffer_h, 0, bytemuck::cast_slice(&[h]));
        queue.write_buffer(&self.histogram_blur_params_buffer_v, 0, bytemuck::cast_slice(&[v]));
    }

    /// Update transforms.
    ///
    /// Buffer layout (parallel to variation_params):
    ///
    /// ```text
    ///   [0 .. parent_total)               parent normals + linkeds + finals
    ///   [parent_total .. MAX_TRANSFORMS)  zero-padding (parent slack)
    ///   [MAX_TRANSFORMS .. MAX_TRANSFORMS + subflame_total)
    ///                                     subflame xforms, packed in the same
    ///                                     order as `update_variation_params`
    ///                                     (per subflame: normals, then finals)
    ///   [.. MAX_TRANSFORMS_UNIFIED)       zero-padding (subflame slack)
    /// ```
    ///
    /// Subflame xforms in the unified slots let
    /// `transforms[xform_id].variations[var_id]` resolve for them —
    /// klein_group and other `needs_transform: true` variations read
    /// their own weight that way.
    pub fn update_transforms(&self, queue: &Queue, flame: &Flame, render_mode: crate::scene::transforms::RenderMode) {
        let total_transforms = flame.total_gpu_transform_slots();
        if total_transforms > MAX_TRANSFORMS {
            panic!(
                "Flame has {} total transform slots (normals={} + linkeds={} + finals={}) but MAX_TRANSFORMS is {}",
                total_transforms,
                flame.transforms.len(),
                flame.linked_transforms.len(),
                flame.final_transforms.len(),
                MAX_TRANSFORMS,
            );
        }

        // Pack parent + subflame transforms into the unified layout (shared
        // with HighResExporter for cross-engine parity).
        let gpu_transforms = pack_gpu_transforms(flame, render_mode);
        queue.write_buffer(&self.transform_buffer, 0, bytemuck::cast_slice(&gpu_transforms));
    }

    /// Write the per-subflame metadata uniform (offsets / counts /
    /// xform_id_base for each subflame).
    ///
    /// Pre-v2, this function also wrote a separate `subflame_transforms`
    /// buffer that subflame_iterate read xform data from. v2 unified
    /// subflame xforms into the parent's `transforms` buffer (written
    /// by `update_transforms`) at slots
    /// `[MAX_TRANSFORMS, MAX_TRANSFORMS + subflame_total)`, so this
    /// function only writes metadata now. The `local_map` parameter
    /// is kept for call-site stability but is no longer used here.
    ///
    /// Validation: returns `Err(...)` if the subflame count exceeds
    /// `MAX_SUBFLAMES` or the total xform count exceeds
    /// `MAX_SUBFLAME_TRANSFORMS_TOTAL`. The caller surfaces this as a
    /// user-visible config error rather than panic.
    pub fn update_subflames(
        &self,
        queue: &Queue,
        subflames: &[Flame],
        _local_map: &std::collections::HashMap<String, u32>,
    ) -> Result<(), String> {
        // Build the metadata (offsets/counts into the unified transform
        // layout); shared with HighResExporter for cross-engine parity.
        let metas = build_subflame_metas(subflames)?;
        queue.write_buffer(&self.subflame_metadata_buffer, 0, bytemuck::cast_slice(&metas));
        Ok(())
    }

    /// Update the per-normal-transform attachment list buffer using
    /// the per-flame `cap` (matches the shader's `array<u32, cap>` in
    /// the AttachmentList struct). Pads to MAX_TRANSFORMS with empty
    /// lists. Per-pool CPU indexes get translated to global xform_ids
    /// during packing.
    ///
    /// `cap` MUST match the value the shader was compiled with
    /// (`Flame::attachment_cap()` for the same flame). Mismatch =
    /// stride mismatch = garbage reads.
    /// See `docs/projects/per-transform-linked-and-final.md`.
    pub fn update_attachments(&self, queue: &Queue, flame: &Flame, cap: usize) {
        let n = flame.transforms.len();
        let l = flame.linked_transforms.len();
        let f = flame.final_transforms.len();
        let linked_offset = n;
        let final_offset = n + l;

        let stride = attachment_stride_bytes(cap);
        let mut buf = vec![0u8; MAX_TRANSFORMS * stride];

        for (i, t) in flame.transforms.iter().enumerate() {
            pack_attachment_entry(
                &mut buf[i * stride..(i + 1) * stride],
                t, cap,
                linked_offset, l,
                final_offset, f,
            );
        }
        // Slots beyond flame.transforms.len() are left zero (count=0
        // inside the entry implies no chain to walk).

        queue.write_buffer(&self.attachments_buffer, 0, &buf);
    }

    /// Update variation parameters.
    ///
    /// Buffer layout (parallel to xform_id):
    ///
    /// ```text
    ///   [0 .. parent_total)               parent normals + linkeds + finals
    ///   [parent_total .. MAX_TRANSFORMS)  zero-padding (parent slack)
    ///   [MAX_TRANSFORMS .. MAX_TRANSFORMS + subflame_total)
    ///                                     subflame xforms, packed in the same
    ///                                     order as `subflame_transforms_buffer`
    ///                                     (per subflame: normals, then finals)
    ///   [.. MAX_TRANSFORMS_UNIFIED)       zero-padding (subflame slack)
    /// ```
    ///
    /// Subflame xforms use synthetic `xform_id = MAX_TRANSFORMS +
    /// subflame_buffer_offset` (set per-subflame via
    /// `SubflameMeta.xform_id_base`), so `get_param(xform_id, …)`
    /// reads from the subflame portion of this buffer. v1 had all
    /// subflame xforms reading OOB and getting zeros — that's what
    /// broke julian / blob / klein_group inside subflames.
    pub fn update_variation_params(&self, queue: &Queue, flame: &Flame) {
        let total_transforms = flame.total_gpu_transform_slots();
        if total_transforms > MAX_TRANSFORMS {
            panic!(
                "Flame has {} total transform slots (normals={} + linkeds={} + finals={}) but MAX_TRANSFORMS is {}",
                total_transforms,
                flame.transforms.len(),
                flame.linked_transforms.len(),
                flame.final_transforms.len(),
                MAX_TRANSFORMS,
            );
        }

        // Soft cap: warn if the per-xform packed footprint exceeds the fixed
        // slot count (some parameters would be lost).
        let registry = crate::variations::global_registry();
        let local_map = crate::scene::transforms::compute_local_index_map(
            flame.active_variation_names_ordered(&registry),
        );
        let total_slots = crate::scene::transforms::total_packed_slots(&local_map, &registry);
        if total_slots as usize > MAX_VARIATION_PARAM_SLOTS {
            log::error!(
                "Flame has {} packed variation slots, exceeds buffer capacity {}. \
                 Some parameters will be lost. Active variations: {:?}",
                total_slots,
                MAX_VARIATION_PARAM_SLOTS,
                local_map.keys().collect::<Vec<_>>(),
            );
        }

        // Pack parent + subflame variation params into the unified layout
        // (shared with HighResExporter for cross-engine parity).
        let gpu_params = pack_gpu_variation_params(flame);
        queue.write_buffer(&self.variation_params_buffer, 0, bytemuck::cast_slice(&gpu_params));
    }

    /// Update palette texture.
    ///
    /// Uses the palette_size set during FlameBuffers creation. The
    /// rotation/squeeze pipeline lives in
    /// `scene::palette::render_palette_lookup` — both this GPU upload
    /// path and the UI preview call into the same helper so they can't
    /// drift apart.
    pub fn update_palette(
        &self,
        queue: &Queue,
        palette: &Palette,
        palette_rotation: f32,
        palette_squeeze: f32,
        palette_squeeze_mode: crate::scene::palette::SqueezeMode,
        palette_squeeze_falloff: f32,
        palette_log_strength: f32,
        palette_reverse: bool,
    ) {
        use crate::scene::palette::{render_palette_lookup, PaletteTransform, SqueezeMode};

        let size = self.palette_size as usize;
        // For geometric mode, `squeeze_factor` carries the octave ratio
        // (`palette_squeeze_falloff`). For linear mode it carries the
        // existing repeat count (`palette_squeeze`).
        let squeeze_factor = match palette_squeeze_mode {
            SqueezeMode::Linear => palette_squeeze,
            SqueezeMode::Geometric => palette_squeeze_falloff,
        };
        let transform = PaletteTransform {
            squeeze_mode: palette_squeeze_mode,
            squeeze_factor,
            log_strength: palette_log_strength,
            rotation: palette_rotation,
            reverse: palette_reverse,
        };
        let rotated_data = render_palette_lookup(palette, &transform, size);

        // Convert f32 [0.0, 1.0] to u8 [0, 255] for Rgba8Unorm
        let palette_data_u8: Vec<u8> = rotated_data
            .iter()
            .map(|&v| (v.clamp(0.0, 1.0) * 255.0) as u8)
            .collect();

        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.palette_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &palette_data_u8,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.palette_size * 4), // N pixels * 4 components * 1 byte
                rows_per_image: None,
            },
            Extent3d {
                width: self.palette_size,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Resize the palette texture to a new size.
    ///
    /// Only recreates the palette texture and view — all other buffers are untouched.
    /// Returns true if the size actually changed.
    pub fn resize_palette(&mut self, device: &Device, new_size: u32) -> bool {
        let new_size = new_size.clamp(DEFAULT_PALETTE_SIZE, MAX_PALETTE_SIZE);
        if self.palette_size == new_size {
            return false;
        }

        self.palette_size = new_size;

        self.palette_texture = device.create_texture(&TextureDescriptor {
            label: Some("Palette Texture"),
            size: Extent3d {
                width: new_size,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        self.palette_view = self.palette_texture.create_view(&TextureViewDescriptor::default());

        true
    }

    /// Update tone curve LUT texture
    pub fn update_curve_lut(&self, queue: &Queue, curve: &crate::scene::tonemap::ToneCurve) {
        let curve_lut_data = curve.generate_lut();

        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.curve_lut_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &curve_lut_data,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: None,  // 1D textures don't have rows
                rows_per_image: None,
            },
            Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }

    // ============================================================
    // Path buffer management (optional buffers for memory savings)
    // ============================================================

    /// Check if path features are currently enabled (buffers allocated)
    pub fn path_features_enabled(&self) -> bool {
        self.path_buffer.is_some()
    }

    /// Create path buffers if not already created
    /// Call when PathMap color mode is enabled or path filters are added
    /// Returns true if buffers were created (bind groups need rebuilding)
    pub fn create_path_buffers(&mut self, device: &Device, queue: &Queue) -> bool {
        if self.path_buffer.is_some() {
            return false;  // Already created
        }

        log::info!(
            "Creating path buffers: {}×{} ({:.1}MB)",
            self.width,
            self.height,
            (self.width as f64 * self.height as f64 * 7.0 * 4.0) / (1024.0 * 1024.0)
        );

        // Create path buffer (7 × u32 per pixel for PathEntry struct)
        let path_buffer_size = (self.width * self.height * 7 * std::mem::size_of::<u32>() as u32) as u64;
        self.path_buffer = Some(device.create_buffer(&BufferDescriptor {
            label: Some("Path Buffer"),
            size: path_buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));

        // Create path filter buffer (MAX_PATH_FILTERS × 16 bytes each)
        let path_filter_buffer_size = (MAX_PATH_FILTERS * std::mem::size_of::<GpuPathFilter>()) as u64;
        self.path_filter_buffer = Some(device.create_buffer(&BufferDescriptor {
            label: Some("Path Filter Buffer"),
            size: path_filter_buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));

        // Initialize filter buffer with empty filters
        let empty_filters = vec![GpuPathFilter::empty(); MAX_PATH_FILTERS];
        queue.write_buffer(
            self.path_filter_buffer.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(&empty_filters),
        );

        true  // Bind groups need rebuilding
    }

    /// Drop path buffers to free memory
    /// Call when PathMap color mode is disabled AND no path filters are active
    /// Returns true if buffers were dropped (bind groups need rebuilding)
    pub fn drop_path_buffers(&mut self) -> bool {
        if self.path_buffer.is_none() {
            return false;  // Already dropped
        }

        log::info!(
            "Dropping path buffers: {:.1}MB freed",
            (self.width as f64 * self.height as f64 * 7.0 * 4.0) / (1024.0 * 1024.0)
        );

        self.path_buffer = None;
        self.path_filter_buffer = None;

        true  // Bind groups need rebuilding
    }

    /// Get the path buffer for binding (real or dummy)
    /// Use this when creating bind groups
    pub fn get_path_buffer_for_binding(&self) -> &Buffer {
        self.path_buffer.as_ref().unwrap_or(&self.dummy_path_buffer)
    }

    /// Get the path filter buffer for binding (real or dummy)
    /// Use this when creating bind groups
    pub fn get_filter_buffer_for_binding(&self) -> &Buffer {
        self.path_filter_buffer.as_ref().unwrap_or(&self.dummy_filter_buffer)
    }

    /// Write path filters to the GPU buffer
    /// Only writes if path buffers are enabled
    pub fn write_path_filters(&self, queue: &Queue, filters: &[GpuPathFilter]) {
        if let Some(ref filter_buffer) = self.path_filter_buffer {
            // Pad with empty filters if needed
            let mut padded_filters = filters.to_vec();
            padded_filters.resize(MAX_PATH_FILTERS, GpuPathFilter::empty());
            queue.write_buffer(filter_buffer, 0, bytemuck::cast_slice(&padded_filters));
        }
    }

    // ============================================================
    // Xaos buffer management (optional buffer for memory savings)
    // ============================================================

    /// Check if xaos buffer is currently allocated
    pub fn xaos_enabled(&self) -> bool {
        self.xaos_buffer.is_some()
    }

    /// Create xaos buffer for given number of transforms
    /// Call when flame has non-default xaos weights
    /// Returns true if buffer was created (bind groups need rebuilding)
    pub fn create_xaos_buffer(&mut self, device: &Device, num_transforms: u32) -> bool {
        if self.xaos_buffer.is_some() {
            return false;  // Already created
        }

        let buffer_size = (num_transforms * num_transforms * std::mem::size_of::<f32>() as u32) as u64;

        log::info!(
            "Creating xaos buffer: {}×{} transforms ({} bytes)",
            num_transforms,
            num_transforms,
            buffer_size
        );

        self.xaos_buffer = Some(device.create_buffer(&BufferDescriptor {
            label: Some("Xaos Buffer"),
            size: buffer_size.max(4),  // Minimum 4 bytes
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));

        true  // Bind groups need rebuilding
    }

    /// Drop xaos buffer to free memory
    /// Call when flame has all default xaos weights (1.0)
    /// Returns true if buffer was dropped (bind groups need rebuilding)
    pub fn drop_xaos_buffer(&mut self) -> bool {
        if self.xaos_buffer.is_none() {
            return false;  // Already dropped
        }

        log::info!("Dropping xaos buffer");

        self.xaos_buffer = None;

        true  // Bind groups need rebuilding
    }

    /// Get the xaos buffer for binding (real or dummy)
    /// Use this when creating bind groups
    pub fn get_xaos_buffer_for_binding(&self) -> &Buffer {
        self.xaos_buffer.as_ref().unwrap_or(&self.dummy_xaos_buffer)
    }

    /// Get the analytic-blur low-res splat buffer for binding (real or dummy).
    pub fn get_blur_splat_for_binding(&self) -> &Buffer {
        self.blur_splat_buffer.as_ref().unwrap_or(&self.dummy_blur_buffer)
    }

    /// Get the low-res convolved scratch for binding (real or dummy).
    pub fn get_blur_convolved_for_binding(&self) -> &Buffer {
        self.blur_convolved_buffer.as_ref().unwrap_or(&self.dummy_blur_buffer)
    }

    /// Allocate / resize / drop the two LOW-RES analytic-blur buffers (splat +
    /// convolved) for `num_blur` slices at `lowres_w × lowres_h`. Caps the
    /// allocated count so the two buffers together stay within the device's
    /// binding budget (each is `lowres_w·lowres_h·16 × slots`; low-res so this
    /// is tiny except at D=1 / small renders). The shader gates `slot < count`,
    /// so any eligible transform beyond the allocated count falls back to the
    /// stochastic path — no crash. Returns true when the buffer set changed
    /// (bind groups must be rebuilt). `num_blur == 0` frees everything.
    pub fn ensure_lowres_blur_buffers(
        &mut self,
        device: &Device,
        lowres_w: u32,
        lowres_h: u32,
        num_blur: u32,
    ) -> bool {
        let slice = (lowres_w as u64) * (lowres_h as u64) * 4 * std::mem::size_of::<u32>() as u64;
        // Cap so `2 · slots · slice ≤ binding budget` (two low-res buffers).
        let cap = if slice == 0 { 0 } else { (self.max_binding_size / (2 * slice)) as u32 };
        let want = num_blur.min(MAX_BLUR_BUFFERS).min(cap);

        if want == 0 {
            if self.blur_splat_buffer.is_none() {
                return false; // Already dropped.
            }
            self.blur_splat_buffer = None;
            self.blur_convolved_buffer = None;
            self.blur_buffer_count = 0;
            self.blur_lowres_w = 0;
            self.blur_lowres_h = 0;
            log::info!("Dropping analytic-blur buffers");
            return true;
        }

        // No change when count AND low-res dims match.
        if want == self.blur_buffer_count
            && lowres_w == self.blur_lowres_w
            && lowres_h == self.blur_lowres_h
        {
            return false;
        }

        let size = slice * want as u64;
        log::info!(
            "Allocating analytic-blur buffers: {} slice(s) {}×{} low-res ({:.2}MB ×2)",
            want, lowres_w, lowres_h, size as f64 / (1024.0 * 1024.0)
        );
        let mk = |label: &'static str| device.create_buffer(&BufferDescriptor {
            label: Some(label),
            size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        self.blur_splat_buffer = Some(mk("Blur Splat Buffer"));
        self.blur_convolved_buffer = Some(mk("Blur Convolved Buffer"));
        self.blur_buffer_count = want;
        self.blur_lowres_w = lowres_w;
        self.blur_lowres_h = lowres_h;
        true
    }

    /// Update xaos weights from flame
    /// Only writes if xaos buffer is enabled
    pub fn update_xaos(&self, queue: &Queue, flame: &Flame) {
        if let Some(ref xaos_buffer) = self.xaos_buffer {
            if let Some(flat) = flame.xaos_flat() {
                queue.write_buffer(xaos_buffer, 0, bytemuck::cast_slice(&flat));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::transforms::Transform;

    #[test]
    fn gpu_transform_std430_size_invariant() {
        // `array<GpuTransform>` in std430 needs a 16-byte-aligned stride.
        // The analytic_blur_slot field was carved from existing padding, so
        // the size must stay 592 (37 x 16).
        let sz = std::mem::size_of::<GpuTransform>();
        assert_eq!(sz % 16, 0, "GpuTransform size {sz} not 16-byte aligned");
        assert_eq!(sz, 592, "GpuTransform size changed: {sz}");
    }

    /// Reads a u32 (little-endian) from `buf` at offset `off`.
    fn rd_u32(buf: &[u8], off: usize) -> u32 {
        u32::from_le_bytes(buf[off..off + 4].try_into().unwrap())
    }

    #[test]
    fn test_attachment_stride_bytes() {
        // (cap*4 + 4) * 2 — linked array + linked_count + final array + final_count
        assert_eq!(attachment_stride_bytes(1), 16);
        assert_eq!(attachment_stride_bytes(8), 72);
        assert_eq!(attachment_stride_bytes(16), 136);
        assert_eq!(attachment_stride_bytes(32), 264);
    }

    #[test]
    fn test_pack_attachment_entry_layout_cap1() {
        // Smallest case: one slot per side. Verifies all four fields land
        // at the right byte offsets and that the final array starts where
        // the linked count ends.
        let mut t = Transform::new();
        t.linked_attachments = vec![0];
        t.final_attachments = vec![0];

        let cap = 1;
        let stride = attachment_stride_bytes(cap);
        let mut buf = vec![0u8; stride];

        // Pool layout: 2 normals, 3 linkeds, 1 final.
        // linked_offset = 2, final_offset = 5.
        pack_attachment_entry(&mut buf, &t, cap, 2, 3, 5, 1);

        // [0..4]   linked[0] = global id (2 + 0) = 2
        // [4..8]   linked_count = 1
        // [8..12]  final[0]   = global id (5 + 0) = 5
        // [12..16] final_count = 1
        assert_eq!(rd_u32(&buf, 0), 2);
        assert_eq!(rd_u32(&buf, 4), 1);
        assert_eq!(rd_u32(&buf, 8), 5);
        assert_eq!(rd_u32(&buf, 12), 1);
    }

    #[test]
    fn test_pack_attachment_entry_layout_cap8() {
        // cap=8, partial fill: verifies that unused tail slots are zero
        // and that the count field lands after the full cap-sized array
        // (not after the actually-written entries).
        let mut t = Transform::new();
        t.linked_attachments = vec![1, 2];
        t.final_attachments = vec![0];

        let cap = 8;
        let stride = attachment_stride_bytes(cap);
        assert_eq!(stride, 72);
        let mut buf = vec![0u8; stride];

        // Pool layout: 3 normals, 4 linkeds, 2 finals.
        pack_attachment_entry(&mut buf, &t, cap, 3, 4, 7, 2);

        // Linked array (cap*4 = 32 bytes), then linked_count (4 bytes).
        assert_eq!(rd_u32(&buf, 0), 3 + 1, "linked[0]");
        assert_eq!(rd_u32(&buf, 4), 3 + 2, "linked[1]");
        // Slots [2..8] of the linked array stay zero (cap=8, only 2 used).
        for slot in 2..8 {
            assert_eq!(rd_u32(&buf, slot * 4), 0, "linked[{}] should be zero", slot);
        }
        // linked_count at offset cap*4 = 32.
        assert_eq!(rd_u32(&buf, 32), 2, "linked_count");

        // Final array starts at offset cap*4 + 4 = 36.
        assert_eq!(rd_u32(&buf, 36), 7 + 0, "final[0]");
        for slot in 1..8 {
            assert_eq!(rd_u32(&buf, 36 + slot * 4), 0, "final[{}] should be zero", slot);
        }
        // final_count at offset 36 + cap*4 = 68.
        assert_eq!(rd_u32(&buf, 68), 1, "final_count");
    }

    #[test]
    fn test_pack_attachment_entry_layout_cap32() {
        // Worst-case cap (== MAX_ATTACHMENTS_PER_TRANSFORM). Verifies
        // the layout math at the upper bound.
        let mut t = Transform::new();
        t.linked_attachments = (0..32).collect();
        t.final_attachments = (0..16).collect();

        let cap = 32;
        let stride = attachment_stride_bytes(cap);
        assert_eq!(stride, 264);
        let mut buf = vec![0u8; stride];

        // Pool layout: 1 normal, 32 linkeds, 16 finals.
        pack_attachment_entry(&mut buf, &t, cap, 1, 32, 33, 16);

        // Verify a sampling: first, middle, last linked entries.
        assert_eq!(rd_u32(&buf, 0), 1, "linked[0]");
        assert_eq!(rd_u32(&buf, 64), 17, "linked[16]");
        assert_eq!(rd_u32(&buf, 124), 32, "linked[31]");
        // linked_count at cap*4 = 128.
        assert_eq!(rd_u32(&buf, 128), 32, "linked_count");

        // Final starts at cap*4 + 4 = 132.
        assert_eq!(rd_u32(&buf, 132), 33, "final[0]");
        assert_eq!(rd_u32(&buf, 132 + 15 * 4), 48, "final[15]");
        // final_count at 132 + cap*4 = 260.
        assert_eq!(rd_u32(&buf, 260), 16, "final_count");
    }

    #[test]
    fn test_pack_attachment_entry_xform_id_translation() {
        // The most important thing the packer does: translate per-pool
        // CPU indexes into GLOBAL xform_ids in the concatenated GPU
        // transforms buffer.
        //
        // Flame layout: 2 normals + 3 linkeds + 2 finals.
        //   slots 0..1 — normals
        //   slots 2..4 — linkeds (linked_offset = 2)
        //   slots 5..6 — finals  (final_offset  = 5)
        //
        // A normal with linked_attachments=[0, 2] should pack as global
        // ids [2, 4], and final_attachments=[1] as global id [6].
        let mut t = Transform::new();
        t.linked_attachments = vec![0, 2];
        t.final_attachments = vec![1];

        let cap = 4;
        let stride = attachment_stride_bytes(cap);
        let mut buf = vec![0u8; stride];

        pack_attachment_entry(&mut buf, &t, cap, 2, 3, 5, 2);

        // linked[0] = linked_offset(2) + 0 = 2
        // linked[1] = linked_offset(2) + 2 = 4
        assert_eq!(rd_u32(&buf, 0), 2);
        assert_eq!(rd_u32(&buf, 4), 4);
        // linked_count = 2 at offset cap*4 = 16
        assert_eq!(rd_u32(&buf, 16), 2);

        // final[0] = final_offset(5) + 1 = 6
        // Final array starts at cap*4 + 4 = 20.
        assert_eq!(rd_u32(&buf, 20), 6);
        // final_count = 1 at 20 + cap*4 = 36.
        assert_eq!(rd_u32(&buf, 36), 1);
    }

    #[test]
    fn test_pack_attachment_entry_drops_out_of_pool_indices() {
        // Defense in depth: if the CPU-side attachment list contains an
        // index past its pool's actual size (e.g., from a buggy migration),
        // the packer drops it with a warning and the count reflects only
        // the valid entries. Catches accidental buffer writes that would
        // dispatch a non-existent transform on the GPU.
        let mut t = Transform::new();
        t.linked_attachments = vec![0, 99]; // 99 is out of pool range
        t.final_attachments = vec![5, 0];   // 5 is out, 0 is valid

        let cap = 4;
        let stride = attachment_stride_bytes(cap);
        let mut buf = vec![0u8; stride];

        // Linked pool size = 1 (only index 0 is valid).
        // Final pool size = 2 (indices 0, 1 valid; 5 is out).
        pack_attachment_entry(&mut buf, &t, cap, 0, 1, 1, 2);

        // Only the valid linked attachment landed.
        assert_eq!(rd_u32(&buf, 0), 0); // linked_offset(0) + 0
        assert_eq!(rd_u32(&buf, 4), 0); // unused — out-of-range was dropped
        assert_eq!(rd_u32(&buf, 16), 1, "linked_count should reflect drop");

        // Final: 5 dropped, 0 kept (becomes final_offset+0 = 1).
        assert_eq!(rd_u32(&buf, 20), 1);
        assert_eq!(rd_u32(&buf, 36), 1, "final_count should reflect drop");
    }
}
