use std::collections::{HashMap, BTreeMap};
use std::sync::atomic::{AtomicU64, Ordering};
use serde::{Deserialize, Serialize, Deserializer, Serializer};
use serde::de::{self, Visitor, MapAccess};
use crate::variations::VariationRegistry;

/// Process-global monotonic ID counter used to assign stable, session-local
/// identities to Transforms and Flames (subflames). The counter starts at 1
/// so `0` can act as the "needs assignment" sentinel — `Default::default()`
/// produces `id = 0`, and `fixup_ids` (in `config::fractal_config`) walks
/// freshly-deserialized configs and assigns a real ID to anything that's
/// still `0`.
///
/// IDs are runtime-only (`#[serde(skip)]`) — they never appear in saved
/// `.fflame` / `.anim` files. The animation system uses them to bind tracks
/// to specific Transforms / subflames so that adding, deleting, or reordering
/// items mid-session keeps tracks pointing at the same thing.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// Allocate a fresh ID from the process-global counter.
pub fn next_id() -> u64 {
    NEXT_ID.fetch_add(1, Ordering::Relaxed)
}

/// Maximum number of variations that can be active in a single flame.
/// This is the cap on the GPU-side `xform.variations` array and the
/// `variation_params` array layout (100 variations × 12 params = 1200 floats).
/// The variation registry itself is unbounded — this only limits the
/// per-flame active set.
pub const MAX_VARIATIONS_PER_FLAME: usize = 100;

/// Compute a per-flame local index map for the given active variation set.
///
/// Active variation names are sorted by their order in the registry (which is
/// append-only, so this is stable across runs and across registry growth) and
/// assigned sequential local indices `0..N`. If the active set exceeds
/// `MAX_VARIATIONS_PER_FLAME`, a warning is logged and the overflow is dropped.
///
/// Both the shader builder and the GPU buffer populator must use the same
/// mapping to keep buffer slots and shader code in agreement.
pub fn compute_local_index_map<I, S>(
    active_names: I,
) -> HashMap<String, u32>
where
    I: IntoIterator<Item = S>,
    S: AsRef<str>,
{
    // Assign local indices in the ORDER the names arrive (deduped, first
    // occurrence wins). Callers pass a flame-ordered list
    // (`Flame::active_variation_names_ordered`) so the dispatch emission
    // order matches JWildfire's per-xform variation order — which matters
    // for `NeedsAccum` / `Replace` variations that read/clobber the
    // running accumulator. The GPU buffer populator uses this same map, so
    // weight/param slots stay consistent with the shader.
    let mut active_in_order: Vec<String> = Vec::new();
    let mut seen = std::collections::HashSet::new();
    for s in active_names {
        let name = s.as_ref().to_string();
        if seen.insert(name.clone()) {
            active_in_order.push(name);
        }
    }
    if active_in_order.len() > MAX_VARIATIONS_PER_FLAME {
        log::warn!(
            "Flame has {} active variations; truncating to {}. Dropped: {:?}",
            active_in_order.len(),
            MAX_VARIATIONS_PER_FLAME,
            &active_in_order[MAX_VARIATIONS_PER_FLAME..],
        );
        active_in_order.truncate(MAX_VARIATIONS_PER_FLAME);
    }
    active_in_order
        .into_iter()
        .enumerate()
        .map(|(local, name)| (name, local as u32))
        .collect()
}

/// One entry in a flame's packed parameter layout: a variation, its
/// per-flame local index, the offset where its slots start in the
/// packed buffer, and how many slots it owns.
///
/// Returned by [`compute_packed_layout`] in local-index order, so the
/// total slot count of a flame is `entries.last().map(|e| e.offset +
/// e.slot_count).unwrap_or(0)`.
#[derive(Debug, Clone)]
pub struct PackedParamEntry {
    pub name: String,
    pub local_idx: u32,
    pub offset: u32,
    pub slot_count: u32,
}

/// Compute the packed parameter layout for a flame's active variation set.
///
/// Walks the local index map in order of `local_idx` (which matches
/// registry order, see [`compute_local_index_map`]) and assigns each
/// variation a contiguous slot range in the packed buffer:
///
/// ```text
///   variation A (local_idx=0, slot_count=3): offset 0, slots [0..3)
///   variation B (local_idx=1, slot_count=8): offset 3, slots [3..11)
///   variation C (local_idx=2, slot_count=2): offset 11, slots [11..13)
/// ```
///
/// Both the shader builder (for its generated `get_param` switch) and
/// the host packer ([`crate::gpu::buffers::GpuVariationParams`]) must
/// use this layout consistently — they're keyed by local_idx through
/// the same registry-order assignment.
///
/// Variations not found in the registry are skipped (this is unusual —
/// it would mean the flame references a variation that's been
/// unregistered).
pub fn compute_packed_layout(
    local_map: &HashMap<String, u32>,
    registry: &VariationRegistry,
) -> Vec<PackedParamEntry> {
    let mut entries: Vec<(&String, u32)> =
        local_map.iter().map(|(n, &i)| (n, i)).collect();
    entries.sort_by_key(|&(_, i)| i);

    let mut out = Vec::with_capacity(entries.len());
    let mut cursor: u32 = 0;
    for (name, local_idx) in entries {
        let slot_count = match registry.get(name) {
            Some(info) => info.slot_count() as u32,
            None => continue,
        };
        out.push(PackedParamEntry {
            name: name.clone(),
            local_idx,
            offset: cursor,
            slot_count,
        });
        cursor += slot_count;
    }
    out
}

/// Total number of slots needed to pack a flame's active variations.
///
/// Convenience wrapper that returns just the cursor value after walking
/// [`compute_packed_layout`].
pub fn total_packed_slots(
    local_map: &HashMap<String, u32>,
    registry: &VariationRegistry,
) -> u32 {
    compute_packed_layout(local_map, registry)
        .last()
        .map(|e| e.offset + e.slot_count)
        .unwrap_or(0)
}

/// One entry in a flame's per-thread state layout. Records which (xform,
/// variation) pair this entry belongs to, the offset in the flame's
/// `thread_state` array where its slots start, and how many slots it owns.
///
/// Returned by [`compute_state_layout`]. Unlike
/// [`PackedParamEntry`], state is keyed on `(xform_idx,
/// variation_local_id)` rather than just `variation_local_id` — the
/// same variation in different transforms gets independent state.
///
/// See [`docs/projects/intra-iteration-state-and-accum.md`](../../../docs/projects/intra-iteration-state-and-accum.md).
#[derive(Debug, Clone)]
pub struct PackedStateEntry {
    pub xform_idx: u32,
    pub variation_local_id: u32,
    pub variation_name: String,
    pub offset: u32,
    pub state_count: u32,
}

/// Soft cap on the total `var<private> thread_state` allocation per flame.
/// 1024 f32 = 4 KB per thread, well within the typical 32 KB per-thread
/// stack on desktop / mobile GPUs. Bumped here if we encounter flames that
/// legitimately need more.
pub const MAX_STATE_SLOTS_PER_FLAME: u32 = 1024;

/// Synthetic `xform_id` base for subflame xforms in the unified
/// xform_id space.
///
/// Subflame xforms occupy `[SUBFLAME_XFORM_ID_BASE,
/// SUBFLAME_XFORM_ID_BASE + subflame_total)` in every per-xform
/// buffer (variation_params, per-thread state, eventually transforms).
/// The subflame iteration shader computes
/// `xform_id = xform_id_base + normals_offset + picked` to land on
/// the right slot — `xform_id_base` on `SubflameMeta` matches this
/// constant.
///
/// MUST match `crate::gpu::buffers::MAX_TRANSFORMS`. Hardcoded here
/// instead of imported to keep `scene` from depending on `gpu`. If
/// MAX_TRANSFORMS changes, update this too (and the matching
/// `xform_id_base` initializer in `update_subflames`).
pub const SUBFLAME_XFORM_ID_BASE: u32 = 128;

/// Walk a flame's active variations and assign each `(xform_idx,
/// variation_local_id)` pair with `state_count > 0` a contiguous offset
/// in the per-thread state array.
///
/// Walk order:
///   1. Each transform in declaration order (`flame.transforms`).
///   2. The final transform last (if present), at index `transforms.len()`.
///   3. Within each transform, active variations sorted by local_idx so
///      the layout matches the shader builder's emit order.
///
/// Variations not in the active set, with weight ≈ 0, or with
/// `state_count == 0` are skipped.
pub fn compute_state_layout(
    flame: &Flame,
    local_map: &HashMap<String, u32>,
    registry: &VariationRegistry,
) -> Vec<PackedStateEntry> {
    let mut out: Vec<PackedStateEntry> = Vec::new();
    let mut cursor: u32 = 0;

    let mut emit_xform = |xform_idx: u32, xform: &Transform, cursor: &mut u32| {
        let mut active: Vec<(&String, u32)> = xform
            .variations
            .iter()
            .filter(|(_, &w)| w.abs() > 1e-6)
            .filter_map(|(name, _)| local_map.get(name).map(|&id| (name, id)))
            .collect();
        active.sort_by_key(|&(_, id)| id);
        for (name, local_id) in active {
            let info = match registry.get(name) {
                Some(i) => i,
                None => continue,
            };
            if info.state_count == 0 {
                continue;
            }
            let state_count = info.state_count as u32;
            out.push(PackedStateEntry {
                xform_idx,
                variation_local_id: local_id,
                variation_name: name.clone(),
                offset: *cursor,
                state_count,
            });
            *cursor += state_count;
        }
    };

    // Emit state slots in the same global xform_id order used by the GPU
    // transform buffer: normals, then linkeds, then finals.
    let mut next_idx: u32 = 0;
    for xform in flame.transforms.iter() {
        emit_xform(next_idx, xform, &mut cursor);
        next_idx += 1;
    }
    for xform in flame.linked_transforms.iter() {
        emit_xform(next_idx, xform, &mut cursor);
        next_idx += 1;
    }
    for xform in flame.final_transforms.iter() {
        emit_xform(next_idx, xform, &mut cursor);
        next_idx += 1;
    }

    // Subflame xforms land in the unified xform_id range
    // `[SUBFLAME_XFORM_ID_BASE, SUBFLAME_XFORM_ID_BASE + subflame_total)`
    // matching the variation_params buffer layout and the synthetic
    // xform_id computed by `subflame_iterate`. Per-subflame order
    // is normals first, then finals — same as `update_subflames`
    // packs the subflame_transforms_buffer. Without this, stateful
    // variations in subflames (klein_group, etc.) read OOB on
    // `thread_state` and lose their state every iteration.
    let mut sub_offset: u32 = 0;
    for sf in flame.subflames.iter() {
        for xform in sf.transforms.iter() {
            emit_xform(SUBFLAME_XFORM_ID_BASE + sub_offset, xform, &mut cursor);
            sub_offset += 1;
        }
        for xform in sf.final_transforms.iter() {
            emit_xform(SUBFLAME_XFORM_ID_BASE + sub_offset, xform, &mut cursor);
            sub_offset += 1;
        }
    }

    if cursor > MAX_STATE_SLOTS_PER_FLAME {
        log::warn!(
            "Flame '{}' needs {} state slots; soft cap is {}. Consider raising MAX_STATE_SLOTS_PER_FLAME.",
            flame.name,
            cursor,
            MAX_STATE_SLOTS_PER_FLAME,
        );
    }

    out
}

/// Total number of state slots needed for a flame's active variations.
/// Returns 0 if no active variation declares state.
pub fn total_state_slots(
    flame: &Flame,
    local_map: &HashMap<String, u32>,
    registry: &VariationRegistry,
) -> u32 {
    compute_state_layout(flame, local_map, registry)
        .last()
        .map(|e| e.offset + e.state_count)
        .unwrap_or(0)
}

/// IFS Transform with named variations (V2)
///
/// This struct is used for both regular transforms AND the final transform.
/// When used as the final transform, only these fields are used:
/// - Affine matrix (a, b, c, d, e, f, g)
/// - Variations and variation_params
///
/// The following fields are IGNORED for final transforms (color is computed
/// during the iteration loop before the final transform is applied):
/// - weight (final transform is always applied, not selected by probability)
/// - color (final transform doesn't affect color index)
/// - color_speed (final transform doesn't blend colors)
/// - opacity (final transform doesn't affect visibility)
#[derive(Debug, Clone)]
pub struct Transform {
    /// Session-local identity used by the animation system to bind tracks
    /// stably across structural changes (add/delete/reorder). Never
    /// serialized — see module-level `next_id()` docs.
    pub id: u64,

    // Affine transformation matrix: x' = ax + by + e, y' = cx + dy + f
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,

    /// Z offset for 3D mode (z' = z + g)
    pub g: f32,

    /// Probability weight for selecting this transform.
    /// NOTE: Ignored for final transforms.
    pub weight: f32,

    /// Weights for each variation function (named)
    pub variations: HashMap<String, f32>,

    /// Variation parameters (key format: "variation_name.param_name")
    /// Example: "julian.power" -> 3.0
    pub variation_params: HashMap<String, f32>,

    /// Canonical-name order of this transform's variations — a *hint* used
    /// to drive the per-flame dispatch order so it matches JWildfire's
    /// per-xform variation list order. This matters because the running
    /// accumulator (`pVarTP`) is read by `NeedsAccum` variations
    /// (roundspher3D, …) and clobbered by `Replace` variations, so the
    /// order they're applied in changes the result. `variations` is an
    /// unordered map, so the order is captured here on import (XML
    /// attribute order) and on UI add/remove.
    ///
    /// Best-effort: any active variation NOT listed here is appended in
    /// registry order (the pre-feature behavior), so old `.fflame` files
    /// (no `variation_order`) render exactly as before. Entries that are
    /// no longer active are ignored. One canonical name per transform
    /// (no duplicates — see `docs/projects/jwf-features.md`). Serialized
    /// only when non-empty (see the manual `Serialize`/`Deserialize`).
    pub variation_order: Vec<String>,

    /// Per-variation phase override (JWildfire `<var>_fx_priority`), keyed
    /// by canonical variation name — same key space as `variations`. The
    /// `i32` is the raw JWF priority (`<0` pre, `0` normal, `>0` post;
    /// `±2` = the prepost-"inv" family); dispatch buckets by sign at
    /// shader-build time. **Sparse override:** an entry exists only when
    /// the value differs from the variation def's natural-phase priority
    /// (`Pre`→−1 / `Normal`→0 / `Post`→1), and is honored only for
    /// variations whose def phase is `VariationPhase::Any`. Empty for
    /// nearly every transform. See `docs/projects/jwf-features.md`.
    pub variation_priorities: HashMap<String, i32>,

    /// Color palette position (0.0 to 1.0)
    /// Represents position in the palette for color coordinate evolution.
    /// NOTE: Ignored for final transforms.
    pub color: f32,

    /// Color speed / symmetry (-1.0 to 1.0, Apophysis compatibility)
    /// -1.0 = full transform color replacement
    ///  0.0 = 50/50 blend
    ///  1.0 = full inheritance (transform has no color influence)
    /// NOTE: Ignored for final transforms.
    pub color_speed: f32,

    /// Opacity / visibility (0.0 to 1.0, Apophysis compatibility)
    /// Controls probability of plotting points from this transform
    /// 1.0 = always plot (default), 0.0 = never plot (invisible)
    /// NOTE: Ignored for final transforms.
    pub opacity: f32,

    /// Direct-color blend strength (0.0 to 1.0, Apophysis `pluginColor`).
    /// 0.0 = standard color evolution; 1.0 = direct-color variations fully
    /// override the iteration color. No-op when no direct-color variations
    /// are active in the flame, so the default is **1.0** — when a user
    /// adds a DC variation it just works without hunting for an extra
    /// slider. Apophysis defaults this to 0.0; we deviate intentionally
    /// because our model has no other DC-enable toggle and the cost of
    /// being on for a non-DC transform is exactly zero.
    pub direct_color: f32,

    // Post-affine transformation matrix (optional, applied after variations)
    // Same formula as pre-affine: x' = ax + by + e, y' = cx + dy + f, z' = z + g
    // When disabled, post-affine is skipped entirely (zero shader cost).
    /// Whether post-affine is enabled for this transform
    pub post_affine_enabled: bool,
    /// Post-affine matrix coefficient a (default: 1.0 = identity)
    pub post_a: f32,
    /// Post-affine matrix coefficient b (default: 0.0 = identity)
    pub post_b: f32,
    /// Post-affine matrix coefficient c (default: 0.0 = identity)
    pub post_c: f32,
    /// Post-affine matrix coefficient d (default: 1.0 = identity)
    pub post_d: f32,
    /// Post-affine translation X (default: 0.0 = identity)
    pub post_e: f32,
    /// Post-affine translation Y (default: 0.0 = identity)
    pub post_f: f32,
    /// Post-affine Z offset for 3D mode (default: 0.0 = identity)
    pub post_g: f32,

    /// JWildfire-style YZ-plane pre-affine coefficients in the same
    /// `[a, c, b, d, e, f]` positional order as the standard `coefs`
    /// XML attribute. Acts on `(y, z)`: `y' = a·y + b·z + e`,
    /// `z' = c·y + d·z + f` (using the JWF naming convention where
    /// position 00 = input-Y to output-Y = YY, etc.). Identity is
    /// `[1, 0, 0, 1, 0, 0]` — when this is identity (and `zx_coefs`
    /// is also identity), the affine dispatch picks the "flat" path
    /// matching Apophysis math byte-for-byte. See
    /// `docs/projects/jwf-features.md` ("zxCoefs / yzCoefs") for the
    /// full composition rule.
    pub yz_coefs: [f32; 6],

    /// JWildfire-style ZX-plane pre-affine coefficients. Same positional
    /// layout as `yz_coefs`. Acts on `(x, z)` per JWildfire's convention
    /// where position 00 = input-X to output-X = XX, etc. — note the
    /// "first axis" in JWF's ZX plane is X, not Z (despite the name).
    /// Identity is `[1, 0, 0, 1, 0, 0]`.
    pub zx_coefs: [f32; 6],

    /// JWildfire-style YZ-plane post-affine coefficients. Same layout as
    /// `yz_coefs` but applied after the variation chain. Identity is
    /// `[1, 0, 0, 1, 0, 0]`.
    pub yz_post_coefs: [f32; 6],

    /// JWildfire-style ZX-plane post-affine coefficients. Same layout as
    /// `zx_coefs` but applied after the variation chain. Identity is
    /// `[1, 0, 0, 1, 0, 0]`.
    pub zx_post_coefs: [f32; 6],

    /// Indexes into `flame.linked_transforms` — Linked transforms that
    /// run sequentially after this normal transform's variations.
    /// Linked transforms are part of dynamics: their output feeds the
    /// next iteration. Empty for transforms in the linked/final pools
    /// themselves. See `docs/projects/per-transform-linked-and-final.md`.
    pub linked_attachments: Vec<usize>,

    /// Indexes into `flame.final_transforms` — Final transforms that
    /// run sequentially after the Linked chain to produce the plotted
    /// point. Output is NOT fed forward (filter only). Empty for
    /// transforms in the linked/final pools themselves.
    pub final_attachments: Vec<usize>,
}

impl Default for Transform {
    fn default() -> Self {
        Self {
            // `id = 0` is the "needs assignment" sentinel: `fixup_ids`
            // (after deserialize) replaces zeros with fresh counter
            // values. `Transform::new()` allocates eagerly so editor-
            // created transforms have an ID immediately.
            id: 0,
            a: 1.0,
            b: 0.0,
            c: 0.0,
            d: 1.0,
            e: 0.0,
            f: 0.0,
            g: 0.0,
            weight: 1.0,
            variations: HashMap::new(),
            variation_params: HashMap::new(),
            variation_order: Vec::new(),
            variation_priorities: HashMap::new(),
            color: 0.5,        // Mid-palette position (neutral default)
            color_speed: 0.0,  // Apophysis default: 50/50 blend
            opacity: 1.0,      // Apophysis default: always visible
            direct_color: 1.0, // DC on by default; no-op when no DC variation is active. See field docs.
            post_affine_enabled: false,
            post_a: 1.0,
            post_b: 0.0,
            post_c: 0.0,
            post_d: 1.0,
            post_e: 0.0,
            post_f: 0.0,
            post_g: 0.0,
            yz_coefs: IDENTITY_PLANE_COEFS,
            zx_coefs: IDENTITY_PLANE_COEFS,
            yz_post_coefs: IDENTITY_PLANE_COEFS,
            zx_post_coefs: IDENTITY_PLANE_COEFS,
            linked_attachments: Vec::new(),
            final_attachments: Vec::new(),
        }
    }
}

/// Identity 2D affine for a single plane (XY, YZ, or ZX), in the JWF
/// XML positional order `[a, c, b, d, e, f]` = `[1, 0, 0, 1, 0, 0]`.
/// Used as the default for all four JWildfire-extension plane fields
/// (`yz_coefs`, `zx_coefs`, plus their post-affine siblings) so any
/// `Transform` constructed without explicitly setting them behaves as
/// an Apophysis flame.
pub const IDENTITY_PLANE_COEFS: [f32; 6] = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

impl Transform {
    /// True when `yz_coefs` equals the identity 2D affine and therefore
    /// the YZ plane has no effect. Drives both the conditional XML
    /// export (we don't write the attribute when identity, matching
    /// JWildfire's `if (xForm.isHasYZCoeffs())` output) and the GPU
    /// flag bit that picks the flat-vs-full affine path in the shader.
    pub fn is_yz_identity(&self) -> bool {
        self.yz_coefs == IDENTITY_PLANE_COEFS
    }

    /// True when `zx_coefs` is the identity 2D affine. See
    /// [`Self::is_yz_identity`] for what this gates.
    pub fn is_zx_identity(&self) -> bool {
        self.zx_coefs == IDENTITY_PLANE_COEFS
    }

    /// True when `yz_post_coefs` is the identity 2D affine. Same
    /// dispatch semantics as the pre-affine variant but for the
    /// post-affine chain.
    pub fn is_yz_post_identity(&self) -> bool {
        self.yz_post_coefs == IDENTITY_PLANE_COEFS
    }

    /// True when `zx_post_coefs` is the identity 2D affine.
    pub fn is_zx_post_identity(&self) -> bool {
        self.zx_post_coefs == IDENTITY_PLANE_COEFS
    }

    /// True when this transform's post-affine step must run: either the
    /// XY post is enabled (`post_affine_enabled`, set by the `post=`
    /// XML attribute / UI toggle) or a YZ/ZX post plane is non-identity.
    ///
    /// JWildfire gates the three post planes INDEPENDENTLY
    /// (`hasXYPostCoeffs` / `hasYZPostCoeffs` / `hasZXPostCoeffs` in
    /// XForm.java) — a flame can carry `zxPost` with no `post=`
    /// attribute at all, and the ZX post still applies. Gating the
    /// whole step on `post_affine_enabled` alone silently dropped such
    /// planes (observed as a ~44° Y-axis rotation missing on
    /// JWF-rando4-rotated). When this fires with the XY post disabled,
    /// the XY coefficients are at their identity defaults so the XY
    /// part of the step is a no-op.
    pub fn has_post_step(&self) -> bool {
        self.post_affine_enabled || !self.is_yz_post_identity() || !self.is_zx_post_identity()
    }

    /// Create a new transform with identity affine matrix and a fresh
    /// session-local ID. Use this from editor code paths; use
    /// `Transform::default()` (which leaves `id == 0`) for code that
    /// expects `fixup_ids` to allocate later (deserialize / preset
    /// loaders).
    pub fn new() -> Self {
        Self {
            id: next_id(),
            ..Self::default()
        }
    }

    /// Set a variation weight by name
    pub fn set_variation(&mut self, name: &str, weight: f32) {
        // Always insert/update the weight - don't auto-remove at zero
        // This allows variations to remain visible in UI at weight 0
        // Use remove_variation() to explicitly remove a variation
        if self.variations.insert(name.to_string(), weight).is_none() {
            // First time this variation is added — record its order so the
            // dispatch matches add order (see `variation_order`).
            self.variation_order.push(name.to_string());
        }
    }

    /// Remove a variation from this transform, scrubbing all of its
    /// metadata: the weight, the `variation_order` entry, every
    /// `"<name>.<param>"` value in `variation_params`, and any
    /// `variation_priorities` (fx_priority) override. Leaving those behind
    /// orphans them — e.g. a stale `variation_priorities` entry still feeds
    /// the phase bucketing and can mis-place the variation on other pools.
    pub fn remove_variation(&mut self, name: &str) {
        self.variations.remove(name);
        self.variation_order.retain(|n| n != name);
        let prefix = format!("{}.", name);
        self.variation_params.retain(|k, _| !k.starts_with(&prefix));
        self.variation_priorities.remove(name);
    }

    /// This transform's active variation names in dispatch order: the
    /// `variation_order` hint first (only entries still active), then any
    /// remaining active variations in registry order. The registry-order
    /// tail is the stable fallback for flames/edits that didn't record an
    /// order, so old `.fflame` files render exactly as before. See
    /// [`Self::variation_order`].
    /// Value of `variation.param` on this transform, falling back to the
    /// variation DEFINITION's declared default when the key is absent.
    ///
    /// Use this anywhere the CPU has to predict what the GPU will see —
    /// above all in the per-flame shader specializers, which bake data
    /// derived from these values into the WGSL. The GPU fills absent
    /// params from the definition, so a hardcoded literal here silently
    /// diverges the moment a default is edited: the specializer then
    /// bakes tables for a flame that is not the one being drawn.
    pub fn variation_param_or_default(&self, variation: &str, param: &str) -> f32 {
        if let Some(v) = self.variation_params.get(&format!("{variation}.{param}")) {
            return *v;
        }
        crate::variations::global_registry()
            .get(variation)
            .and_then(|info| info.get_param_default(param))
            .unwrap_or(0.0)
    }

    pub fn ordered_variation_names(&self, registry: &VariationRegistry) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for name in &self.variation_order {
            if self.variations.contains_key(name) && seen.insert(name.clone()) {
                out.push(name.clone());
            }
        }
        for name in registry.names() {
            if self.variations.contains_key(name) && seen.insert(name.clone()) {
                out.push(name.clone());
            }
        }
        out
    }

    /// Resolution-independent half of the analytic-blur gate. If this
    /// transform is eligible, returns its single analytic-blur variation's
    /// `(name, weight)`. Eligible iff it has **exactly one** active
    /// (`|w| > eps`) `AnalyticBlur` variation, that variation sits in the
    /// **normal phase** (no `fx_priority` override moving it to pre/post),
    /// and **no other** active variation uses RNG. Non-blur companions may be
    /// nonlinear — they're the deterministic structure the mean-splat
    /// captures. The plot-path-linearity check (post-affine/finals/projection
    /// linear) is applied separately by the renderer. See
    /// `docs/projects/analytic-blur-buffer.md`.
    pub fn analytic_blur(&self, registry: &VariationRegistry) -> Option<(String, f32)> {
        use crate::variations::{Feature, analytic_blur::is_analytic_blur};
        let mut found: Option<(String, f32)> = None;
        for (name, &w) in &self.variations {
            if w.abs() < 1e-6 {
                continue;
            }
            let Some(info) = registry.get(name) else { continue };
            if is_analytic_blur(name) {
                // Normal phase only: a default (0) priority, not moved to
                // pre/post via fx_priority. A moved blur would route its
                // offset through other variations and break linearity.
                if self.variation_priorities.get(name).copied().unwrap_or(0) != 0 {
                    return None;
                }
                if found.is_some() {
                    return None; // >1 analytic blur — not eligible in v1
                }
                found = Some((name.clone(), w));
            } else if info.has_feature(Feature::NeedsRng) {
                return None; // another stochastic variation breaks input-independence
            }
        }
        found
    }

    /// Get a variation weight by name
    pub fn get_variation(&self, name: &str) -> f32 {
        self.variations.get(name).copied().unwrap_or(0.0)
    }

    /// Get all active variation names
    pub fn active_variations(&self) -> Vec<String> {
        self.variations.keys().cloned().collect()
    }

    // === VARIATION PARAMETER METHODS ===

    /// Set a parameter for a specific variation
    /// Key format: "variation_name.param_name" (e.g., "julian.power")
    pub fn set_variation_param(&mut self, variation: &str, param: &str, value: f32) {
        let key = format!("{}.{}", variation, param);
        self.variation_params.insert(key, value);
    }

    /// Get a parameter value for a specific variation
    /// Returns None if not set
    pub fn get_variation_param(&self, variation: &str, param: &str) -> Option<f32> {
        let key = format!("{}.{}", variation, param);
        self.variation_params.get(&key).copied()
    }

    /// Get a parameter value with fallback to default from registry
    pub fn get_variation_param_or_default(
        &self,
        variation: &str,
        param: &str,
        registry: &VariationRegistry,
    ) -> f32 {
        self.get_variation_param(variation, param)
            .or_else(|| {
                registry.get(variation)
                    .and_then(|info| info.get_param_default(param))
            })
            .unwrap_or(0.0)
    }

    /// Convert from legacy array format to HashMap
    pub fn from_array(
        array: &[f32],
        registry: &VariationRegistry,
    ) -> HashMap<String, f32> {
        let mut map = HashMap::new();
        let names = registry.names();

        for (i, &weight) in array.iter().enumerate() {
            if weight.abs() > 1e-6 {
                if let Some(name) = names.get(i) {
                    map.insert(name.clone(), weight);
                }
            }
        }

        map
    }

    /// Convert to GPU array format with runtime ID mapping
    pub fn to_gpu_array(
        &self,
        id_map: &HashMap<String, u32>,
        max_variations: usize,
    ) -> Vec<f32> {
        let mut array = vec![0.0; max_variations];

        for (name, &weight) in &self.variations {
            if let Some(&id) = id_map.get(name) {
                if (id as usize) < max_variations {
                    array[id as usize] = weight;
                }
            }
        }

        array
    }

    // === TRIANGLE EDITOR METHODS ===

    /// Convert affine coefficients to triangle representation (O, X, Y points)
    /// Returns (Origin, X-axis endpoint, Y-axis endpoint)
    ///
    /// Note: Apophysis uses Y-down coordinate system for triangle display.
    /// This matches Apophysis behavior by negating f, b, and c appropriately.
    pub fn to_triangle(&self) -> ([f32; 2], [f32; 2], [f32; 2]) {
        let o = [self.e, -self.f];
        let x = [self.e + self.a, -self.f - self.b];
        let y = [self.e - self.c, -self.f + self.d];
        (o, x, y)
    }

    /// Update affine coefficients from triangle representation
    /// Takes (Origin, X-axis endpoint, Y-axis endpoint)
    ///
    /// Note: Inverse of to_triangle(), accounts for Apophysis Y-down coordinate system.
    pub fn from_triangle(&mut self, o: [f32; 2], x: [f32; 2], y: [f32; 2]) {
        self.a = x[0] - o[0];
        self.b = -(x[1] - o[1]);
        self.c = -(y[0] - o[0]);
        self.d = y[1] - o[1];
        self.e = o[0];
        self.f = -o[1];
    }

    /// Convert affine coefficients to triangle using Apophysis sign convention
    /// Matches Apophysis triangle editor exactly
    pub fn to_triangle_apophysis(&self) -> ([f32; 2], [f32; 2], [f32; 2]) {
        // Apophysis displays b, c, f with opposite sign from our internal representation
        let display_f = -self.f;

        // Apophysis formulas (verified to match exactly):
        // O = (e, -f)
        // X = (e + a, -f - c)
        // Y = (e - b, -f + d)
        let o = [self.e, display_f];
        let x = [self.e + self.a, display_f - self.c];
        let y = [self.e - self.b, display_f + self.d];
        (o, x, y)
    }

    /// Update affine coefficients from triangle using Apophysis sign convention
    /// Inverse of to_triangle_apophysis()
    pub fn from_triangle_apophysis(&mut self, o: [f32; 2], x: [f32; 2], y: [f32; 2]) {
        // Inverse of:
        // O = (e, -f)
        // X = (e + a, -f - c)
        // Y = (e - b, -f + d)
        //
        // Solve for coefficients:
        // e = O[0]
        // f = -O[1]
        // a = X[0] - O[0]
        // c = O[1] - X[1]  (since X[1] = O[1] - c)
        // b = O[0] - Y[0]  (since Y[0] = O[0] - b)
        // d = Y[1] - O[1]

        self.e = o[0];
        self.f = -o[1];
        self.a = x[0] - o[0];
        self.c = o[1] - x[1];
        self.b = o[0] - y[0];
        self.d = y[1] - o[1];
    }

    // === JWF PLANE-AFFINE TRIANGLE EDITOR METHODS ===

    /// Convert a JWildfire plane-affine coefficient array (`yz_coefs`,
    /// `zx_coefs`, or their post variants — positional order
    /// `[00, 01, 10, 11, 20, 21]` = our `[a, c, b, d, e, f]`) to an
    /// Apophysis-convention editor triangle. Same formulas as
    /// [`Self::to_triangle_apophysis`], so the triangle editor behaves
    /// identically on every plane.
    pub fn plane_to_triangle_apophysis(coefs: &[f32; 6]) -> ([f32; 2], [f32; 2], [f32; 2]) {
        let (a, c, b, d, e, f) = (coefs[0], coefs[1], coefs[2], coefs[3], coefs[4], coefs[5]);
        let display_f = -f;
        let o = [e, display_f];
        let x = [e + a, display_f - c];
        let y = [e - b, display_f + d];
        (o, x, y)
    }

    /// Inverse of [`Self::plane_to_triangle_apophysis`] — write an
    /// editor triangle back into a plane-affine coefficient array.
    pub fn plane_from_triangle_apophysis(coefs: &mut [f32; 6], o: [f32; 2], x: [f32; 2], y: [f32; 2]) {
        coefs[4] = o[0];                // e (20)
        coefs[5] = -o[1];               // f (21)
        coefs[0] = x[0] - o[0];         // a (00)
        coefs[1] = o[1] - x[1];         // c (01)
        coefs[2] = o[0] - y[0];         // b (10)
        coefs[3] = y[1] - o[1];         // d (11)
    }

    // === POST-AFFINE TRIANGLE EDITOR METHODS ===

    /// Convert post-affine coefficients to triangle using Apophysis sign convention
    pub fn post_to_triangle_apophysis(&self) -> ([f32; 2], [f32; 2], [f32; 2]) {
        let display_f = -self.post_f;
        let o = [self.post_e, display_f];
        let x = [self.post_e + self.post_a, display_f - self.post_c];
        let y = [self.post_e - self.post_b, display_f + self.post_d];
        (o, x, y)
    }

    /// Update post-affine coefficients from triangle using Apophysis sign convention
    pub fn post_from_triangle_apophysis(&mut self, o: [f32; 2], x: [f32; 2], y: [f32; 2]) {
        self.post_e = o[0];
        self.post_f = -o[1];
        self.post_a = x[0] - o[0];
        self.post_c = o[1] - x[1];
        self.post_b = o[0] - y[0];
        self.post_d = y[1] - o[1];
    }

    /// Reset post-affine to identity (no-op transform)
    pub fn reset_post_affine_to_identity(&mut self) {
        self.post_a = 1.0;
        self.post_b = 0.0;
        self.post_c = 0.0;
        self.post_d = 1.0;
        self.post_e = 0.0;
        self.post_f = 0.0;
        self.post_g = 0.0;
    }

    /// Reset transform to identity (unit triangle at origin)
    pub fn reset_to_identity(&mut self) {
        self.a = 1.0;
        self.b = 0.0;
        self.c = 0.0;
        self.d = 1.0;
        self.e = 0.0;
        self.f = 0.0;
    }

    // === TRANSFORM OPERATIONS (for animation) ===

    /// Get the origin X position (translation component)
    pub fn origin_x(&self) -> f32 {
        self.e
    }

    /// Get the origin Y position (translation component, Apophysis convention)
    pub fn origin_y(&self) -> f32 {
        -self.f
    }

    /// Set the origin X position (translation component)
    pub fn set_origin_x(&mut self, x: f32) {
        // Get current triangle
        let (mut o, mut x_pt, mut y_pt) = self.to_triangle_apophysis();
        let dx = x - o[0];
        // Translate all points
        o[0] = x;
        x_pt[0] += dx;
        y_pt[0] += dx;
        self.from_triangle_apophysis(o, x_pt, y_pt);
    }

    /// Set the origin Y position (translation component, Apophysis convention)
    pub fn set_origin_y(&mut self, y: f32) {
        // Get current triangle
        let (mut o, mut x_pt, mut y_pt) = self.to_triangle_apophysis();
        let dy = y - o[1];
        // Translate all points
        o[1] = y;
        x_pt[1] += dy;
        y_pt[1] += dy;
        self.from_triangle_apophysis(o, x_pt, y_pt);
    }

    /// Get the rotation angle in radians (from the X-axis arm)
    pub fn rotation(&self) -> f32 {
        let (o, x_pt, _) = self.to_triangle_apophysis();
        let dx = x_pt[0] - o[0];
        let dy = x_pt[1] - o[1];
        dy.atan2(dx)
    }

    /// Set the rotation angle in radians (rotates around origin, preserving scale)
    pub fn set_rotation(&mut self, angle: f32) {
        let (o, x_pt, y_pt) = self.to_triangle_apophysis();

        // Get current vectors from origin
        let x_vec = [x_pt[0] - o[0], x_pt[1] - o[1]];
        let y_vec = [y_pt[0] - o[0], y_pt[1] - o[1]];

        // Get current lengths (scales)
        let x_len = (x_vec[0] * x_vec[0] + x_vec[1] * x_vec[1]).sqrt();
        let y_len = (y_vec[0] * y_vec[0] + y_vec[1] * y_vec[1]).sqrt();

        // Get current angle between X and Y arms (to preserve shear)
        let current_x_angle = x_vec[1].atan2(x_vec[0]);
        let current_y_angle = y_vec[1].atan2(y_vec[0]);
        let angle_diff = current_y_angle - current_x_angle;

        // New X arm at the target angle
        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let new_x = [o[0] + x_len * cos_a, o[1] + x_len * sin_a];

        // New Y arm at target angle + preserved angle difference
        let y_angle = angle + angle_diff;
        let new_y = [o[0] + y_len * y_angle.cos(), o[1] + y_len * y_angle.sin()];

        self.from_triangle_apophysis(o, new_x, new_y);
    }

    /// Get the uniform scale factor (average of X and Y arm lengths)
    pub fn scale(&self) -> f32 {
        let (o, x_pt, y_pt) = self.to_triangle_apophysis();
        let x_vec = [x_pt[0] - o[0], x_pt[1] - o[1]];
        let y_vec = [y_pt[0] - o[0], y_pt[1] - o[1]];
        let x_len = (x_vec[0] * x_vec[0] + x_vec[1] * x_vec[1]).sqrt();
        let y_len = (y_vec[0] * y_vec[0] + y_vec[1] * y_vec[1]).sqrt();
        (x_len + y_len) / 2.0
    }

    /// Set uniform scale (scales both arms equally, preserving rotation)
    pub fn set_scale(&mut self, scale: f32) {
        let (o, x_pt, y_pt) = self.to_triangle_apophysis();

        // Get current vectors from origin
        let x_vec = [x_pt[0] - o[0], x_pt[1] - o[1]];
        let y_vec = [y_pt[0] - o[0], y_pt[1] - o[1]];

        // Get current lengths
        let x_len = (x_vec[0] * x_vec[0] + x_vec[1] * x_vec[1]).sqrt();
        let y_len = (y_vec[0] * y_vec[0] + y_vec[1] * y_vec[1]).sqrt();

        // Avoid division by zero
        if x_len < 1e-6 || y_len < 1e-6 {
            return;
        }

        // Scale both arms to the target scale
        let x_scale = scale / x_len;
        let y_scale = scale / y_len;

        let new_x = [o[0] + x_vec[0] * x_scale, o[1] + x_vec[1] * x_scale];
        let new_y = [o[0] + y_vec[0] * y_scale, o[1] + y_vec[1] * y_scale];

        self.from_triangle_apophysis(o, new_x, new_y);
    }

    // === POST-AFFINE TRANSFORM OPERATIONS (mirror of pre-affine) ===

    /// Get the post-affine origin X position (translation component).
    pub fn post_origin_x(&self) -> f32 {
        self.post_e
    }

    /// Get the post-affine origin Y position (Apophysis convention).
    pub fn post_origin_y(&self) -> f32 {
        -self.post_f
    }

    /// Set the post-affine origin X position (translates triangle by dx).
    pub fn set_post_origin_x(&mut self, x: f32) {
        let (mut o, mut x_pt, mut y_pt) = self.post_to_triangle_apophysis();
        let dx = x - o[0];
        o[0] = x;
        x_pt[0] += dx;
        y_pt[0] += dx;
        self.post_from_triangle_apophysis(o, x_pt, y_pt);
    }

    /// Set the post-affine origin Y position (Apophysis convention).
    pub fn set_post_origin_y(&mut self, y: f32) {
        let (mut o, mut x_pt, mut y_pt) = self.post_to_triangle_apophysis();
        let dy = y - o[1];
        o[1] = y;
        x_pt[1] += dy;
        y_pt[1] += dy;
        self.post_from_triangle_apophysis(o, x_pt, y_pt);
    }

    /// Get the post-affine rotation angle in radians (from the X-axis arm).
    pub fn post_rotation(&self) -> f32 {
        let (o, x_pt, _) = self.post_to_triangle_apophysis();
        let dx = x_pt[0] - o[0];
        let dy = x_pt[1] - o[1];
        dy.atan2(dx)
    }

    /// Set the post-affine rotation angle in radians. Rotates the
    /// X arm to `angle`, preserves the X/Y arm length ratio and the
    /// angle difference between arms (so any pre-existing shear is
    /// kept).
    pub fn set_post_rotation(&mut self, angle: f32) {
        let (o, x_pt, y_pt) = self.post_to_triangle_apophysis();

        let x_vec = [x_pt[0] - o[0], x_pt[1] - o[1]];
        let y_vec = [y_pt[0] - o[0], y_pt[1] - o[1]];

        let x_len = (x_vec[0] * x_vec[0] + x_vec[1] * x_vec[1]).sqrt();
        let y_len = (y_vec[0] * y_vec[0] + y_vec[1] * y_vec[1]).sqrt();

        let current_x_angle = x_vec[1].atan2(x_vec[0]);
        let current_y_angle = y_vec[1].atan2(y_vec[0]);
        let angle_diff = current_y_angle - current_x_angle;

        let cos_a = angle.cos();
        let sin_a = angle.sin();
        let new_x = [o[0] + x_len * cos_a, o[1] + x_len * sin_a];

        let y_angle = angle + angle_diff;
        let new_y = [o[0] + y_len * y_angle.cos(), o[1] + y_len * y_angle.sin()];

        self.post_from_triangle_apophysis(o, new_x, new_y);
    }

    /// Get the post-affine uniform scale factor (average of X and Y arm lengths).
    pub fn post_scale(&self) -> f32 {
        let (o, x_pt, y_pt) = self.post_to_triangle_apophysis();
        let x_vec = [x_pt[0] - o[0], x_pt[1] - o[1]];
        let y_vec = [y_pt[0] - o[0], y_pt[1] - o[1]];
        let x_len = (x_vec[0] * x_vec[0] + x_vec[1] * x_vec[1]).sqrt();
        let y_len = (y_vec[0] * y_vec[0] + y_vec[1] * y_vec[1]).sqrt();
        (x_len + y_len) / 2.0
    }

    /// Set uniform post-affine scale (scales both arms equally,
    /// preserves rotation). No-op if either arm has near-zero length.
    pub fn set_post_scale(&mut self, scale: f32) {
        let (o, x_pt, y_pt) = self.post_to_triangle_apophysis();

        let x_vec = [x_pt[0] - o[0], x_pt[1] - o[1]];
        let y_vec = [y_pt[0] - o[0], y_pt[1] - o[1]];

        let x_len = (x_vec[0] * x_vec[0] + x_vec[1] * x_vec[1]).sqrt();
        let y_len = (y_vec[0] * y_vec[0] + y_vec[1] * y_vec[1]).sqrt();

        if x_len < 1e-6 || y_len < 1e-6 {
            return;
        }

        let x_scale = scale / x_len;
        let y_scale = scale / y_len;

        let new_x = [o[0] + x_vec[0] * x_scale, o[1] + x_vec[1] * x_scale];
        let new_y = [o[0] + y_vec[0] * y_scale, o[1] + y_vec[1] * y_scale];

        self.post_from_triangle_apophysis(o, new_x, new_y);
    }

    // === COMPATIBILITY METHODS (for gradual migration) ===

    /// COMPATIBILITY: Set variation by index (for old code)
    pub fn set_variation_by_index(&mut self, index: usize, weight: f32, registry: &VariationRegistry) {
        if let Some(name) = registry.names().get(index) {
            self.set_variation(name, weight);
        }
    }

    /// COMPATIBILITY: Get variation by index
    pub fn get_variation_by_index(&self, index: usize, registry: &VariationRegistry) -> f32 {
        if let Some(name) = registry.names().get(index) {
            self.get_variation(name)
        } else {
            0.0
        }
    }

    /// Convert this transform's variation weights into the GPU's fixed-size
    /// `[f32; 100]` slot array, using the supplied per-flame local index map.
    /// Variations not present in `local_map` (either not active anywhere in the
    /// flame, or dropped past the cap) contribute zero.
    pub fn to_fixed_array(&self, local_map: &HashMap<String, u32>) -> [f32; MAX_VARIATIONS_PER_FLAME] {
        let mut array = [0.0; MAX_VARIATIONS_PER_FLAME];
        for (name, weight) in &self.variations {
            if let Some(&local_idx) = local_map.get(name) {
                let slot = local_idx as usize;
                if slot < MAX_VARIATIONS_PER_FLAME {
                    array[slot] = *weight;
                }
            }
        }
        array
    }
}

/// Custom serialization - saves as sorted map for deterministic output
impl Serialize for Transform {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        use serde::ser::SerializeStruct;

        // Convert HashMaps to BTreeMaps for deterministic key ordering
        // (HashMap iteration order is random, breaking content-addressable caching)
        let variations_sorted: BTreeMap<_, _> = self.variations.iter().collect();
        let params_sorted: BTreeMap<_, _> = self.variation_params.iter().collect();
        let priorities_sorted: BTreeMap<_, _> = self.variation_priorities.iter().collect();

        // Count fields: 13 base + 1 if direct_color != 0 + up to 8 post-affine
        // + up to 4 plane-affine arrays (yz/zx pre and post, only when
        // non-identity) + up to 2 attachment lists (only when non-empty)
        let has_post = self.post_affine_enabled;
        let has_direct_color = self.direct_color.abs() > 1e-6;
        let has_yz = !self.is_yz_identity();
        let has_zx = !self.is_zx_identity();
        let has_yz_post = !self.is_yz_post_identity();
        let has_zx_post = !self.is_zx_post_identity();
        let has_linked = !self.linked_attachments.is_empty();
        let has_final = !self.final_attachments.is_empty();
        let has_priorities = !self.variation_priorities.is_empty();
        let has_var_order = !self.variation_order.is_empty();
        let field_count = 13
            + if has_direct_color { 1 } else { 0 }
            + if has_post { 8 } else { 0 }
            + if has_yz { 1 } else { 0 }
            + if has_zx { 1 } else { 0 }
            + if has_yz_post { 1 } else { 0 }
            + if has_zx_post { 1 } else { 0 }
            + if has_linked { 1 } else { 0 }
            + if has_final { 1 } else { 0 }
            + if has_priorities { 1 } else { 0 }
            + if has_var_order { 1 } else { 0 };

        let mut state = serializer.serialize_struct("Transform", field_count)?;
        state.serialize_field("a", &self.a)?;
        state.serialize_field("b", &self.b)?;
        state.serialize_field("c", &self.c)?;
        state.serialize_field("d", &self.d)?;
        state.serialize_field("e", &self.e)?;
        state.serialize_field("f", &self.f)?;
        state.serialize_field("g", &self.g)?;
        state.serialize_field("weight", &self.weight)?;
        state.serialize_field("variations", &variations_sorted)?;
        state.serialize_field("variation_params", &params_sorted)?;
        // Only serialize fx_priority overrides when present (keeps .fflame clean)
        if has_priorities {
            state.serialize_field("variation_priorities", &priorities_sorted)?;
        }
        // Variation order hint — preserved as a list (NOT sorted), since the
        // order is the whole point. Only when non-empty.
        if has_var_order {
            state.serialize_field("variation_order", &self.variation_order)?;
        }
        state.serialize_field("color", &self.color)?;
        state.serialize_field("color_speed", &self.color_speed)?;
        state.serialize_field("opacity", &self.opacity)?;
        // Only serialize direct_color when non-zero (keeps .fflame files clean)
        if has_direct_color {
            state.serialize_field("direct_color", &self.direct_color)?;
        }
        // Only serialize post-affine fields when enabled (keeps .fflame files clean)
        if has_post {
            state.serialize_field("post_affine_enabled", &self.post_affine_enabled)?;
            state.serialize_field("post_a", &self.post_a)?;
            state.serialize_field("post_b", &self.post_b)?;
            state.serialize_field("post_c", &self.post_c)?;
            state.serialize_field("post_d", &self.post_d)?;
            state.serialize_field("post_e", &self.post_e)?;
            state.serialize_field("post_f", &self.post_f)?;
            state.serialize_field("post_g", &self.post_g)?;
        }
        // JWildfire-extension plane affines. Each is a 6-float array
        // serialized only when non-identity, so existing .fflame files
        // remain unchanged and Apophysis-imported flames produce no
        // extra noise. JSON shape: `"yz_coefs": [a, c, b, d, e, f]`.
        if has_yz {
            state.serialize_field("yz_coefs", &self.yz_coefs)?;
        }
        if has_zx {
            state.serialize_field("zx_coefs", &self.zx_coefs)?;
        }
        if has_yz_post {
            state.serialize_field("yz_post_coefs", &self.yz_post_coefs)?;
        }
        if has_zx_post {
            state.serialize_field("zx_post_coefs", &self.zx_post_coefs)?;
        }
        if has_linked {
            state.serialize_field("linked_attachments", &self.linked_attachments)?;
        }
        if has_final {
            state.serialize_field("final_attachments", &self.final_attachments)?;
        }
        state.end()
    }
}

/// Custom deserialization - supports both HashMap and array formats
impl<'de> Deserialize<'de> for Transform {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            A, B, C, D, E, F, G, Weight, Variations, VariationParams, VariationPriorities,
            VariationOrder,
            Color, ColorSpeed, Opacity,
            DirectColor,
            PostAffineEnabled, PostA, PostB, PostC, PostD, PostE, PostF, PostG,
            YzCoefs, ZxCoefs, YzPostCoefs, ZxPostCoefs,
            LinkedAttachments, FinalAttachments,
        }

        struct TransformVisitor;

        impl<'de> Visitor<'de> for TransformVisitor {
            type Value = Transform;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Transform")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Transform, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut a = None;
                let mut b = None;
                let mut c = None;
                let mut d = None;
                let mut e = None;
                let mut f = None;
                let mut g = None;
                let mut weight = None;
                let mut variations = None;
                let mut variation_params = None;
                let mut variation_priorities: Option<HashMap<String, i32>> = None;
                let mut variation_order: Option<Vec<String>> = None;
                let mut color = None;
                let mut color_speed = None;
                let mut opacity = None;
                let mut direct_color = None;
                let mut post_affine_enabled = None;
                let mut post_a = None;
                let mut post_b = None;
                let mut post_c = None;
                let mut post_d = None;
                let mut post_e = None;
                let mut post_f = None;
                let mut post_g = None;
                let mut yz_coefs: Option<[f32; 6]> = None;
                let mut zx_coefs: Option<[f32; 6]> = None;
                let mut yz_post_coefs: Option<[f32; 6]> = None;
                let mut zx_post_coefs: Option<[f32; 6]> = None;
                let mut linked_attachments: Option<Vec<usize>> = None;
                let mut final_attachments: Option<Vec<usize>> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::A => a = Some(map.next_value()?),
                        Field::B => b = Some(map.next_value()?),
                        Field::C => c = Some(map.next_value()?),
                        Field::D => d = Some(map.next_value()?),
                        Field::E => e = Some(map.next_value()?),
                        Field::F => f = Some(map.next_value()?),
                        Field::G => g = Some(map.next_value()?),
                        Field::Weight => weight = Some(map.next_value()?),
                        Field::Variations => {
                            // Try to deserialize as HashMap first
                            let value: serde_json::Value = map.next_value()?;

                            let var_map = match value {
                                // New format: HashMap
                                serde_json::Value::Object(obj) => {
                                    let mut map = HashMap::new();
                                    for (k, v) in obj {
                                        if let serde_json::Value::Number(num) = v {
                                            if let Some(f) = num.as_f64() {
                                                map.insert(k, f as f32);
                                            }
                                        }
                                    }
                                    map
                                }
                                // Old format: Array - convert using global registry
                                serde_json::Value::Array(arr) => {
                                    let mut map = HashMap::new();
                                    let registry = crate::variations::global_registry();
                                    let names = registry.names();

                                    for (i, val) in arr.iter().enumerate() {
                                        if let serde_json::Value::Number(num) = val {
                                            if let Some(weight) = num.as_f64() {
                                                let weight = weight as f32;
                                                if weight.abs() > 1e-6 {
                                                    if let Some(name) = names.get(i) {
                                                        map.insert(name.clone(), weight);
                                                    }
                                                }
                                            }
                                        }
                                    }
                                    map
                                }
                                _ => {
                                    return Err(de::Error::custom(
                                        "variations must be an object (new format) or array (legacy format)"
                                    ));
                                }
                            };

                            variations = Some(var_map);
                        }
                        Field::VariationParams => {
                            variation_params = Some(map.next_value()?);
                        }
                        Field::VariationPriorities => {
                            variation_priorities = Some(map.next_value()?);
                        }
                        Field::VariationOrder => {
                            variation_order = Some(map.next_value()?);
                        }
                        Field::Color => {
                            // Handle both old format [f32; 3] and new format f32
                            let value: serde_json::Value = map.next_value()?;
                            let color_value = match value {
                                // New format: single float (palette position)
                                serde_json::Value::Number(num) => {
                                    num.as_f64().map(|f| f as f32).unwrap_or(0.5)
                                }
                                // Old format: RGB array → average to single value
                                serde_json::Value::Array(arr) if arr.len() >= 3 => {
                                    let r = arr[0].as_f64().unwrap_or(0.0) as f32;
                                    let g = arr[1].as_f64().unwrap_or(0.0) as f32;
                                    let b = arr[2].as_f64().unwrap_or(0.0) as f32;
                                    (r + g + b) / 3.0
                                }
                                _ => 0.5  // Default to mid-palette
                            };
                            color = Some(color_value);
                        }
                        Field::ColorSpeed => color_speed = Some(map.next_value()?),
                        Field::Opacity => opacity = Some(map.next_value()?),
                        Field::DirectColor => direct_color = Some(map.next_value()?),
                        Field::PostAffineEnabled => post_affine_enabled = Some(map.next_value()?),
                        Field::PostA => post_a = Some(map.next_value()?),
                        Field::PostB => post_b = Some(map.next_value()?),
                        Field::PostC => post_c = Some(map.next_value()?),
                        Field::PostD => post_d = Some(map.next_value()?),
                        Field::PostE => post_e = Some(map.next_value()?),
                        Field::PostF => post_f = Some(map.next_value()?),
                        Field::PostG => post_g = Some(map.next_value()?),
                        Field::YzCoefs => yz_coefs = Some(map.next_value()?),
                        Field::ZxCoefs => zx_coefs = Some(map.next_value()?),
                        Field::YzPostCoefs => yz_post_coefs = Some(map.next_value()?),
                        Field::ZxPostCoefs => zx_post_coefs = Some(map.next_value()?),
                        Field::LinkedAttachments => linked_attachments = Some(map.next_value()?),
                        Field::FinalAttachments => final_attachments = Some(map.next_value()?),
                    }
                }

                // Canonicalize aliased variation names (e.g. `su3_mobius` →
                // `su_mobius`, `jacobian_cubic` → `jacobian_counterexample`)
                // across every name-keyed field, so `.fflame` configs saved
                // under a pre-rename name keep their weights, params,
                // priorities, and ordering. XML import canonicalizes at parse
                // time; this is the JSON-config equivalent. Without the
                // param-key rewrite an aliased flame would still COMPILE the
                // right variation (registry `get` resolves aliases) but its
                // packed param slots would read as all zeros.
                let registry = crate::variations::global_registry();
                let variations: HashMap<String, f32> = variations
                    .ok_or_else(|| de::Error::missing_field("variations"))?
                    .into_iter()
                    .map(|(k, v)| (registry.resolve_alias(&k).to_string(), v))
                    .collect();
                let variation_params: HashMap<String, f32> = variation_params
                    .unwrap_or_else(HashMap::<String, f32>::new)
                    .into_iter()
                    .map(|(k, v)| match k.split_once('.') {
                        Some((name, param)) => {
                            (format!("{}.{}", registry.resolve_alias(name), param), v)
                        }
                        None => (k, v),
                    })
                    .collect();
                let variation_priorities: HashMap<String, i32> = variation_priorities
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(k, v)| (registry.resolve_alias(&k).to_string(), v))
                    .collect();
                let variation_order: Vec<String> = variation_order
                    .unwrap_or_default()
                    .into_iter()
                    .map(|k| registry.resolve_alias(&k).to_string())
                    .collect();

                Ok(Transform {
                    // ID is assigned by the post-deserialize `fixup_ids`
                    // pass (see `config::fractal_config`). Leaving it 0
                    // here keeps the file format unchanged.
                    id: 0,
                    a: a.ok_or_else(|| de::Error::missing_field("a"))?,
                    b: b.ok_or_else(|| de::Error::missing_field("b"))?,
                    c: c.ok_or_else(|| de::Error::missing_field("c"))?,
                    d: d.ok_or_else(|| de::Error::missing_field("d"))?,
                    e: e.ok_or_else(|| de::Error::missing_field("e"))?,
                    f: f.ok_or_else(|| de::Error::missing_field("f"))?,
                    g: g.unwrap_or(0.0),
                    weight: weight.ok_or_else(|| de::Error::missing_field("weight"))?,
                    variations,
                    variation_params,
                    variation_priorities,
                    variation_order,
                    color: color.ok_or_else(|| de::Error::missing_field("color"))?,
                    color_speed: color_speed.unwrap_or(0.0), // Default to 0.0 for backward compatibility
                    opacity: opacity.unwrap_or(1.0), // Default to 1.0 for backward compatibility
                    // Deserialize fallback is 0.0 even though Transform::default() is now
                    // 1.0 — `.fflame` files written before the default flip omit this field
                    // when it was at the old default of 0.0 (see the `has_direct_color` skip
                    // in serialize), and we want those files to keep their old appearance.
                    // New flames serialize 1.0 explicitly, so they round-trip fine.
                    direct_color: direct_color.unwrap_or(0.0),
                    // Post-affine defaults to disabled + identity (backward compatible)
                    post_affine_enabled: post_affine_enabled.unwrap_or(false),
                    post_a: post_a.unwrap_or(1.0),
                    post_b: post_b.unwrap_or(0.0),
                    post_c: post_c.unwrap_or(0.0),
                    post_d: post_d.unwrap_or(1.0),
                    post_e: post_e.unwrap_or(0.0),
                    post_f: post_f.unwrap_or(0.0),
                    post_g: post_g.unwrap_or(0.0),
                    yz_coefs: yz_coefs.unwrap_or(IDENTITY_PLANE_COEFS),
                    zx_coefs: zx_coefs.unwrap_or(IDENTITY_PLANE_COEFS),
                    yz_post_coefs: yz_post_coefs.unwrap_or(IDENTITY_PLANE_COEFS),
                    zx_post_coefs: zx_post_coefs.unwrap_or(IDENTITY_PLANE_COEFS),
                    linked_attachments: linked_attachments.unwrap_or_default(),
                    final_attachments: final_attachments.unwrap_or_default(),
                })
            }
        }

        const FIELDS: &[&str] = &["a", "b", "c", "d", "e", "f", "g", "weight", "variations", "variation_params", "variation_priorities", "variation_order", "color", "color_speed", "opacity", "direct_color", "post_affine_enabled", "post_a", "post_b", "post_c", "post_d", "post_e", "post_f", "post_g", "yz_coefs", "zx_coefs", "yz_post_coefs", "zx_post_coefs", "linked_attachments", "final_attachments"];
        deserializer.deserialize_struct("Transform", FIELDS, TransformVisitor)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `.fflame` JSON with a pre-rename variation name must canonicalize
    /// across every name-keyed field at deserialize time — without the
    /// param-key rewrite the variation compiles but its packed param
    /// slots read as zeros.
    #[test]
    fn test_deserialize_canonicalizes_variation_aliases() {
        let json = r#"{
            "a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "e": 0.0, "f": 0.0,
            "weight": 1.0, "color": 0.5,
            "variations": {"jacobian_cubic": 1.0, "su3_mobius": 0.5},
            "variation_params": {"jacobian_cubic.scale": 2.0, "su3_mobius.group": 1.0},
            "variation_order": ["jacobian_cubic", "su3_mobius"],
            "variation_priorities": {"jacobian_cubic": 1}
        }"#;
        let t: Transform = serde_json::from_str(json).unwrap();
        assert_eq!(t.get_variation("jacobian_counterexample"), 1.0);
        assert_eq!(t.get_variation("su_mobius"), 0.5);
        assert!(!t.variations.contains_key("jacobian_cubic"));
        assert_eq!(t.variation_params["jacobian_counterexample.scale"], 2.0);
        assert_eq!(t.variation_params["su_mobius.group"], 1.0);
        assert_eq!(t.variation_order, vec!["jacobian_counterexample", "su_mobius"]);
        assert_eq!(t.variation_priorities["jacobian_counterexample"], 1);
    }

    #[test]
    fn test_named_variations() {
        let mut xform = Transform::new();
        xform.set_variation("linear", 0.5);
        xform.set_variation("swirl", 0.3);

        assert_eq!(xform.get_variation("linear"), 0.5);
        assert_eq!(xform.get_variation("swirl"), 0.3);
        assert_eq!(xform.get_variation("nonexistent"), 0.0);
    }

    #[test]
    fn test_compute_local_index_map_preserves_order() {
        // Indices follow the input order (deduped, first-occurrence), so the
        // dispatch emission order matches the flame's variation order.
        let m = compute_local_index_map(
            ["roundspher3D", "waves2", "linear", "waves2"].iter().copied(),
        );
        assert_eq!(m["roundspher3D"], 0);
        assert_eq!(m["waves2"], 1);
        assert_eq!(m["linear"], 2);
        assert_eq!(m.len(), 3, "duplicate name deduped");
    }

    #[test]
    fn test_set_remove_variation_maintains_order() {
        let mut xform = Transform::new();
        xform.set_variation("spherical", 1.0);
        xform.set_variation("linear", 0.5);
        xform.set_variation("spherical", 0.7); // re-set must NOT duplicate
        assert_eq!(xform.variation_order, vec!["spherical".to_string(), "linear".to_string()]);
        xform.remove_variation("spherical");
        assert_eq!(xform.variation_order, vec!["linear".to_string()]);
    }

    #[test]
    fn test_remove_variation_scrubs_params_and_priorities() {
        let mut xform = Transform::new();
        xform.set_variation("squish", 1.0);
        xform.set_variation("linear", 1.0);
        xform.variation_params.insert("squish.power".to_string(), 5.0);
        xform.variation_params.insert("linear.foo".to_string(), 1.0);
        xform.variation_priorities.insert("squish".to_string(), 1);
        xform.variation_priorities.insert("linear".to_string(), -1);

        xform.remove_variation("squish");

        // squish's metadata is gone; linear's is untouched.
        assert!(!xform.variations.contains_key("squish"));
        assert!(!xform.variation_params.contains_key("squish.power"));
        assert!(!xform.variation_priorities.contains_key("squish"));
        assert!(!xform.variation_order.contains(&"squish".to_string()));
        assert_eq!(xform.variation_params.get("linear.foo"), Some(&1.0));
        assert_eq!(xform.variation_priorities.get("linear"), Some(&-1));
    }

    #[test]
    fn test_analytic_blur_gate() {
        let registry = crate::variations::global_registry();

        // Eligible: one analytic blur + a (possibly nonlinear) deterministic
        // companion, no other RNG, normal phase.
        let mut t = Transform::new();
        t.set_variation("spherical", 1.0); // nonlinear, deterministic — OK
        t.set_variation("analytic_blur", 0.2);
        assert_eq!(
            t.analytic_blur(&registry),
            Some(("analytic_blur".to_string(), 0.2)),
        );

        // Another stochastic variation (the original `blur` uses RNG) → out.
        let mut t = Transform::new();
        t.set_variation("analytic_blur", 0.2);
        t.set_variation("blur", 0.1);
        assert_eq!(t.analytic_blur(&registry), None);

        // Two analytic blurs → out (v1).
        let mut t = Transform::new();
        t.set_variation("analytic_blur", 0.2);
        t.set_variation("analytic_gaussian_blur", 0.2);
        assert_eq!(t.analytic_blur(&registry), None);

        // Moved off normal phase (fx_priority) → out.
        let mut t = Transform::new();
        t.set_variation("linear", 1.0);
        t.set_variation("analytic_blur", 0.2);
        t.variation_priorities.insert("analytic_blur".to_string(), 1);
        assert_eq!(t.analytic_blur(&registry), None);

        // No analytic blur → out.
        let mut t = Transform::new();
        t.set_variation("linear", 1.0);
        assert_eq!(t.analytic_blur(&registry), None);

        // Flame collector picks out only the eligible normals.
        let mut flame = Flame::new();
        let mut a = Transform::new();
        a.set_variation("linear", 1.0);
        a.set_variation("analytic_blur", 0.3);
        let mut b = Transform::new();
        b.set_variation("linear", 1.0); // not eligible
        flame.transforms = vec![a, b];
        let elig = flame.analytic_blur_transforms(&registry);
        assert_eq!(elig, vec![(0, "analytic_blur".to_string(), 0.3)]);

        // Full activation gate: orthographic 2D, no attachments → active.
        // (render_mode is scene-level since config v3, passed explicitly.)
        assert!(flame.analytic_blur_active(&registry, RenderMode::TwoD));
        // A Final transform makes the plot path non-trivial → inactive (v1).
        let mut f = Transform::new();
        f.set_variation("linear", 1.0);
        flame.final_transforms = vec![f];
        assert!(!flame.analytic_blur_active(&registry, RenderMode::TwoD));
        flame.final_transforms.clear();
        // Post-symmetry fans one sample into multiple copies → inactive (v1).
        flame.post_symmetry.ty = PostSymmetryType::Point;
        flame.post_symmetry.order = 3;
        assert!(!flame.analytic_blur_active(&registry, RenderMode::TwoD));
        flame.post_symmetry = PostSymmetry::default();
        assert!(flame.analytic_blur_active(&registry, RenderMode::TwoD));
        // 3D — even orthographic — is deferred in v1.
        assert!(!flame.analytic_blur_active(&registry, RenderMode::ThreeD));
    }

    #[test]
    fn test_ordered_variation_names_hint_then_registry_fallback() {
        // The variation_order hint comes first; any active variation not in
        // the hint is appended in registry order (stable fallback).
        let registry = crate::variations::global_registry();
        let mut xform = Transform::new();
        // Insert directly so variation_order stays empty for "spherical".
        xform.variations.insert("spherical".to_string(), 1.0);
        xform.set_variation("waves2", 0.9); // tracked in variation_order
        let ordered = xform.ordered_variation_names(&registry);
        assert_eq!(ordered.first().map(String::as_str), Some("waves2"),
            "hinted variation comes first");
        assert!(ordered.contains(&"spherical".to_string()),
            "untracked active variation still included via registry fallback");
        assert_eq!(ordered.len(), 2);
    }

    #[test]
    fn test_legacy_final_migration_attaches_to_every_normal() {
        let mut flame = Flame::new();
        flame.transforms.push(Transform::new());
        flame.transforms.push(Transform::new());
        flame.transforms.push(Transform::new());
        let legacy = {
            let mut t = Transform::new();
            t.set_variation("spherical", 0.5);
            t
        };

        flame.migrate_legacy_final(Some(legacy));

        // Pool should now contain the legacy final at index 0.
        assert_eq!(flame.final_transforms.len(), 1);
        assert_eq!(flame.final_transforms[0].get_variation("spherical"), 0.5);
        // Every normal transform should reference final_transforms[0].
        for t in &flame.transforms {
            assert_eq!(t.final_attachments, vec![0]);
            assert_eq!(t.linked_attachments, Vec::<usize>::new());
        }
    }

    #[test]
    fn test_migration_appends_to_existing_pool() {
        let mut flame = Flame::new();
        flame.transforms.push(Transform::new());

        flame.migrate_legacy_final(Some(Transform::new()));
        let first_pool_len = flame.final_transforms.len();
        let first_attach = flame.transforms[0].final_attachments.clone();

        // Calling again with another legacy final appends to the pool and
        // adds the new pool index to the attachment list.
        flame.migrate_legacy_final(Some(Transform::new()));
        assert_eq!(flame.final_transforms.len(), first_pool_len + 1);
        assert_eq!(flame.transforms[0].final_attachments.len(), first_attach.len() + 1);
    }

    #[test]
    fn test_migration_no_legacy_final_is_noop() {
        let mut flame = Flame::new();
        flame.transforms.push(Transform::new());
        flame.migrate_legacy_final(None);
        assert!(flame.final_transforms.is_empty());
        assert!(flame.transforms[0].final_attachments.is_empty());
    }

    #[test]
    fn test_array_conversion() {
        let array = [0.5, 0.0, 0.0, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0];
        let registry = crate::variations::global_registry();

        let map = Transform::from_array(&array, &registry);

        assert_eq!(map.get("linear"), Some(&0.5));
        assert_eq!(map.get("swirl"), Some(&0.3));
    }

    #[test]
    fn test_gpu_array_conversion() {
        let mut xform = Transform::new();
        xform.set_variation("linear", 0.5);
        xform.set_variation("swirl", 0.3);

        let mut id_map = HashMap::new();
        id_map.insert("linear".to_string(), 0);
        id_map.insert("swirl".to_string(), 1);

        let gpu_array = xform.to_gpu_array(&id_map, 10);

        assert_eq!(gpu_array[0], 0.5);
        assert_eq!(gpu_array[1], 0.3);
        assert_eq!(gpu_array[2], 0.0);
    }

    #[test]
    fn test_serialize_deserialize() {
        let mut xform = Transform::new();
        xform.set_variation("linear", 0.5);
        xform.set_variation("swirl", 0.3);

        let json = serde_json::to_string(&xform).unwrap();
        let deserialized: Transform = serde_json::from_str(&json).unwrap();

        assert_eq!(deserialized.get_variation("linear"), 0.5);
        assert_eq!(deserialized.get_variation("swirl"), 0.3);
    }

    #[test]
    fn test_deserialize_legacy_array() {
        let json = r#"{
            "a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "e": 0.0, "f": 0.0, "g": 0.0,
            "weight": 1.0,
            "variations": [0.5, 0.0, 0.0, 0.3, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            "color": [1.0, 1.0, 1.0],
            "color_speed": 0.5
        }"#;

        let xform: Transform = serde_json::from_str(json).unwrap();

        assert_eq!(xform.get_variation("linear"), 0.5);
        assert_eq!(xform.get_variation("swirl"), 0.3);
    }

    #[test]
    fn test_variation_params_set_get() {
        let mut xform = Transform::new();

        // Set parameters
        xform.set_variation_param("julian", "power", 5.0);
        xform.set_variation_param("julian", "dist", 1.5);

        // Get parameters
        assert_eq!(xform.get_variation_param("julian", "power"), Some(5.0));
        assert_eq!(xform.get_variation_param("julian", "dist"), Some(1.5));

        // Non-existent parameter
        assert_eq!(xform.get_variation_param("julian", "nonexistent"), None);
        assert_eq!(xform.get_variation_param("nonexistent", "power"), None);
    }

    #[test]
    fn test_variation_params_serialize() {
        let mut xform = Transform::new();
        xform.set_variation("julian", 0.8);
        xform.set_variation_param("julian", "power", 3.0);
        xform.set_variation_param("julian", "dist", 1.0);

        let json = serde_json::to_string(&xform).unwrap();
        let deserialized: Transform = serde_json::from_str(&json).unwrap();

        // Verify variation weight
        assert_eq!(deserialized.get_variation("julian"), 0.8);

        // Verify parameters
        assert_eq!(deserialized.get_variation_param("julian", "power"), Some(3.0));
        assert_eq!(deserialized.get_variation_param("julian", "dist"), Some(1.0));
    }

    #[test]
    fn test_variation_params_backward_compat() {
        // Old config without variation_params field
        let json = r#"{
            "a": 1.0, "b": 0.0, "c": 0.0, "d": 1.0, "e": 0.0, "f": 0.0, "g": 0.0,
            "weight": 1.0,
            "variations": {"julian": 0.8},
            "color": [1.0, 1.0, 1.0],
            "color_speed": 0.5
        }"#;

        let xform: Transform = serde_json::from_str(json).unwrap();

        // Should deserialize successfully
        assert_eq!(xform.get_variation("julian"), 0.8);

        // variation_params should be empty (defaults to empty HashMap)
        assert_eq!(xform.get_variation_param("julian", "power"), None);
        assert!(xform.variation_params.is_empty());
    }

}
// === Additional code from legacy transforms.rs ===

/// Rendering mode for the fractal flame
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RenderMode {
    /// 2D rendering (traditional fractal flames). Wire/cloud-blob form: `"2d"`
    /// (the server casts this to the Postgres `render_mode` enum).
    #[serde(rename = "2d", alias = "TwoD")]
    TwoD,
    /// 3D rendering with pseudo-3D projection. Wire/cloud-blob form: `"3d"`.
    #[serde(rename = "3d", alias = "ThreeD")]
    ThreeD,
}

impl Default for RenderMode {
    fn default() -> Self {
        Self::TwoD
    }
}

// ProjectionType enum removed - now using perspective_strength f32 directly
// 0.0 = orthographic (flat), higher values = increasing perspective distortion

/// A 2D point in fractal space
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Compute radius squared (r²)
    #[inline]
    pub fn r_squared(&self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    /// Compute radius (r)
    #[inline]
    pub fn r(&self) -> f32 {
        self.r_squared().sqrt()
    }

    /// Compute angle (theta)
    #[inline]
    pub fn theta(&self) -> f32 {
        self.y.atan2(self.x)
    }

    /// Compute phi (reciprocal of radius)
    #[inline]
    pub fn phi(&self) -> f32 {
        self.x.atan2(self.y)
    }
}

/// Serde skip helper — omit zero-valued optional f32 fields so
/// existing .fflame files stay byte-stable.
fn _f32_is_zero(v: &f32) -> bool {
    *v == 0.0
}

/// Flame system - collection of transforms
#[derive(Debug, Clone, Serialize)]
pub struct Flame {
    /// Session-local identity used by the animation system to bind tracks
    /// stably across subflame add / delete / reorder. Never serialized;
    /// see module-level `next_id()` docs.
    #[serde(skip)]
    pub id: u64,

    pub name: String,
    pub transforms: Vec<Transform>,
    /// Pool of Linked transforms — referenced by index from each
    /// normal transform's `linked_attachments`. Linked transforms are
    /// part of dynamics (their output feeds the next iteration) and
    /// run in declaration order after the normal transform's variations.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub linked_transforms: Vec<Transform>,
    /// Pool of Final transforms — referenced by index from each
    /// normal transform's `final_attachments`. Final transforms are
    /// pure plot-time filters (output is plotted but NOT fed forward)
    /// and run in declaration order after the Linked chain.
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub final_transforms: Vec<Transform>,
    // render_mode, perspective_strength, depth_density_compensation,
    // far_density_fade, far_density_fade_start moved to `FractalConfig` in
    // config v3 — they were always whole-render (scene) settings, never
    // per-flame. See `FractalConfig` and the v2→v3 migration.
    /// Xaos transition weights: xaos[src][dst] = modifier for src→dst transition
    /// None when all weights are 1.0 (default behavior, no memory allocated)
    /// When Some, outer Vec has len = transforms.len(), inner Vec has len = transforms.len()
    #[serde(skip_serializing_if = "Option::is_none")]
    pub xaos: Option<Vec<Vec<f32>>>,

    /// Solo transform index (0-indexed). When Some(n), only transform n has weight,
    /// all others effectively have weight 0. Used for debugging individual transforms.
    /// Matches Apophysis XML attribute: soloxform="N"
    #[serde(skip_serializing_if = "Option::is_none")]
    pub solo_transform: Option<usize>,

    /// Subflames — additional `Flame` definitions that the
    /// `subflame_wf` variation references by index. A subflame's chaos
    /// game runs as a *nested* IFS during this flame's iteration loop;
    /// it is not a separate render.
    ///
    /// Owned by the parent `Flame` (not by `FractalConfig`) because the
    /// active-variation set + shader-builder local index map need to
    /// include the subflames' variations alongside the parent's. Future
    /// layered rendering (separate `Flame` per layer) naturally gets a
    /// per-layer subflame pool this way.
    ///
    /// See `docs/projects/subflames.md`.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub subflames: Vec<Flame>,

    /// Post-symmetry — JWildfire's per-flame plot-time symmetry. Each
    /// chaos-game sample is also deposited at K−1 symmetric positions
    /// before the camera transform, multiplying density without
    /// advancing the dynamics. K depends on `ty`:
    ///   - `None`: K = 1 (no-op, compile-stripped from the shader)
    ///   - `XAxis` / `YAxis`: K = 2 (original + mirror)
    ///   - `Point`: K = `order` (one per 2π/order rotation around center)
    ///
    /// `distance` and `rotation_deg` are only meaningful for the axis
    /// modes — they offset and pre-rotate the mirror copy. Point mode
    /// ignores them.
    #[serde(default, skip_serializing_if = "PostSymmetry::is_default")]
    pub post_symmetry: PostSymmetry,
    // preserve_z moved to `FractalConfig` in config v3 (scene-global Z
    // semantics, never per-flame). See `FractalConfig::preserve_z` and the
    // v2→v3 migration (absent ⇒ true to preserve pre-flag flames' look).
}

/// What kind of plot-time symmetry to apply (see `Flame.post_symmetry`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PostSymmetryType {
    #[default]
    None,
    XAxis,
    YAxis,
    Point,
}

impl PostSymmetryType {
    /// Single-letter shader encoding so the GPU uniform can be a plain
    /// `u32` instead of an enum import.
    pub fn as_u32(self) -> u32 {
        match self {
            PostSymmetryType::None => 0,
            PostSymmetryType::XAxis => 1,
            PostSymmetryType::YAxis => 2,
            PostSymmetryType::Point => 3,
        }
    }

    /// JWildfire `.flame` XML token (uppercase, underscore-separated).
    ///
    /// **Axis-name swap**: JWildfire's `X_AXIS` actually flips X (a
    /// left/right mirror), and `Y_AXIS` flips Y (top/bottom). We use
    /// the math-class convention internally — `XAxis` = reflect across
    /// the X axis (flips Y). To bridge the two conventions without
    /// surprising users on either side, we swap on the wire: our
    /// `XAxis` ↔ JWF `Y_AXIS`, our `YAxis` ↔ JWF `X_AXIS`. JWF files
    /// round-trip through this swap unchanged, and our UI shows what
    /// most users expect from geometry class.
    pub fn xml_token(self) -> &'static str {
        match self {
            PostSymmetryType::None => "NONE",
            PostSymmetryType::XAxis => "Y_AXIS",
            PostSymmetryType::YAxis => "X_AXIS",
            PostSymmetryType::Point => "POINT",
        }
    }

    /// Inverse of `xml_token` — see the axis-name swap note there.
    /// Unknown values default to None.
    pub fn from_xml_token(s: &str) -> Self {
        // JWildfire emits uppercase tokens. Case-insensitive matching
        // here is defensive against hand-edited files.
        match s.to_ascii_uppercase().as_str() {
            "X_AXIS" => PostSymmetryType::YAxis,
            "Y_AXIS" => PostSymmetryType::XAxis,
            "POINT" => PostSymmetryType::Point,
            _ => PostSymmetryType::None,
        }
    }
}

/// Per-flame post-symmetry settings — see `Flame.post_symmetry`.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PostSymmetry {
    #[serde(default)]
    pub ty: PostSymmetryType,
    /// Number of rotational copies for `Point` mode (ignored for axis
    /// modes). 1 = no-op, 2 = 180° pair, 3 = ternary, etc. Clamped to
    /// [1, 32] at the shader boundary.
    pub order: u32,
    pub center_x: f32,
    pub center_y: f32,
    /// "Pan along the axis" — for `XAxis` shifts the mirror copy by
    /// `(distance, 0)`, for `YAxis` by `(0, distance)`. Applied before
    /// the viewport pan. Ignored in `Point` mode.
    pub distance: f32,
    /// Pre-rotation (degrees) of the mirror copy around `center` for
    /// the axis modes. Ignored in `Point` mode.
    pub rotation_deg: f32,
}

impl Default for PostSymmetry {
    fn default() -> Self {
        // Match JWildfire's default header: NONE, order=3 (the visible
        // default in the UI when type flips to Point), centre=(0,0),
        // distance=1.25, rotation=6° — the per-file defaults JWF writes
        // even when type is NONE.
        Self {
            ty: PostSymmetryType::None,
            order: 3,
            center_x: 0.0,
            center_y: 0.0,
            distance: 1.25,
            rotation_deg: 6.0,
        }
    }
}

impl PostSymmetry {
    /// Serde skip-when-default predicate. Default symmetry serializes
    /// as nothing — keeps `.fflame` files clean.
    pub fn is_default(s: &Self) -> bool {
        *s == Self::default()
    }
}

fn default_flame_name() -> String {
    "Untitled".to_string()
}

impl Default for Flame {
    fn default() -> Self {
        Self {
            // `id = 0` is the "needs assignment" sentinel; `fixup_ids`
            // replaces zeros after deserialize.
            id: 0,
            name: "Untitled".to_string(),
            transforms: Vec::new(),
            linked_transforms: Vec::new(),
            final_transforms: Vec::new(),
            xaos: None,  // Default: no xaos (all weights implicitly 1.0)
            solo_transform: None,  // Default: no solo (all transforms active)
            subflames: Vec::new(),  // Default: no subflames
            post_symmetry: PostSymmetry::default(),
        }
    }
}

impl Flame {
    /// Total GPU transform slot count: normals + linkeds + finals.
    pub fn total_gpu_transform_slots(&self) -> usize {
        self.transforms.len()
            + self.linked_transforms.len()
            + self.final_transforms.len()
    }

    /// Migrate a legacy singular `final_transform` (loaded from JSON
    /// produced by older versions of the app, or pulled from external
    /// sources) into the new `final_transforms` pool with an attachment
    /// on every normal transform. No-op if `legacy` is `None`.
    /// See `docs/projects/per-transform-linked-and-final.md`.
    pub fn migrate_legacy_final(&mut self, legacy: Option<Transform>) {
        let Some(legacy) = legacy else { return };
        let new_idx = self.final_transforms.len();
        self.final_transforms.push(legacy);
        for t in &mut self.transforms {
            if !t.final_attachments.contains(&new_idx) {
                t.final_attachments.push(new_idx);
            }
        }
    }

    /// Create a new flame with a fresh session-local ID. Use this from
    /// editor code paths (e.g. add_subflame); `Flame::default()` leaves
    /// `id == 0` for the `fixup_ids` pass to allocate later.
    pub fn new() -> Self {
        Self {
            id: next_id(),
            ..Self::default()
        }
    }

    pub fn add_transform(&mut self, transform: Transform) {
        self.transforms.push(transform);
    }

    /// Extract all variation names present in any transform across all
    /// three pools. Used by the shader builder to decide which variations
    /// to compile into the flame's shader.
    ///
    /// Includes zero-weight variations so the user can keyframe a
    /// variation from 0 → 1 without triggering a shader recompile when
    /// the weight first crosses the threshold. Adding a variation to
    /// the flame's transform list is the explicit signal to include it
    /// in the shader; if it's truly unused, the user can remove it.
    /// The flame's active variation names in a canonical dispatch order:
    /// walk the transform pools (normal → linked → final → subflames) in
    /// order, and within each transform use its `ordered_variation_names`,
    /// collecting first appearances. Feeds `compute_local_index_map` so the
    /// per-flame shader emits variations in (as close as possible to)
    /// JWildfire's per-xform order — see `Transform::variation_order`.
    pub fn active_variation_names_ordered(&self, registry: &VariationRegistry) -> Vec<String> {
        let mut out: Vec<String> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        let mut absorb = |t: &Transform| {
            for name in t.ordered_variation_names(registry) {
                if seen.insert(name.clone()) {
                    out.push(name);
                }
            }
        };
        for t in &self.transforms { absorb(t); }
        for t in &self.linked_transforms { absorb(t); }
        for t in &self.final_transforms { absorb(t); }
        drop(absorb);
        for sf in &self.subflames {
            for name in sf.active_variation_names_ordered(registry) {
                if seen.insert(name.clone()) {
                    out.push(name);
                }
            }
        }
        out
    }

    pub fn extract_active_variations(&self) -> HashMap<String, f32> {
        let mut all_variations = HashMap::new();
        let absorb = |t: &Transform, all: &mut HashMap<String, f32>| {
            for (name, weight) in &t.variations {
                let existing = all.entry(name.clone()).or_insert(0.0);
                *existing = f32::max(*existing, *weight);
            }
        };

        for t in &self.transforms { absorb(t, &mut all_variations); }
        for t in &self.linked_transforms { absorb(t, &mut all_variations); }
        for t in &self.final_transforms { absorb(t, &mut all_variations); }

        // Include subflames' active variations in the union — the shader
        // builder uses this set to pick which variation functions to
        // include and what the local index map is. Subflames share the
        // parent's variation pool so a subflame transform's variation
        // dispatch lands at the same shader-local index the parent's
        // dispatch would. v1 disallows nested subflames (see config
        // validation in P4), so this recursion only goes one level deep
        // in practice; the `.subflames` field of a subflame is empty.
        for sf in &self.subflames {
            for (name, weight) in sf.extract_active_variations() {
                let existing = all_variations.entry(name).or_insert(0.0);
                *existing = f32::max(*existing, weight);
            }
        }

        all_variations
    }

    /// Check if any transform (in any pool) needs the post-affine step —
    /// XY post enabled OR a non-identity YZ/ZX post plane (JWildfire
    /// gates the three post planes independently; see
    /// `Transform::has_post_step`).
    pub fn has_post_affine(&self) -> bool {
        self.transforms.iter().any(|t| t.has_post_step())
            || self.linked_transforms.iter().any(|t| t.has_post_step())
            || self.final_transforms.iter().any(|t| t.has_post_step())
    }

    /// True when the flame has any Linked or Final pool members.
    /// Drives the `HAS_ATTACHMENTS` shader template flag — when false,
    /// the per-iteration `attachments[xform_idx]` storage load and
    /// both chain loops are stripped from the compiled shader.
    pub fn has_attachments(&self) -> bool {
        !self.linked_transforms.is_empty() || !self.final_transforms.is_empty()
    }

    /// Normal transforms eligible for the analytic blur path (the
    /// resolution-independent gate — see [`Transform::analytic_blur`]), each
    /// as `(normal_index, blur_variation_name, weight)`. **Empty when the
    /// feature is unused**, so callers gate all analytic-blur allocation /
    /// shader codegen on `is_empty()` for zero overhead. v1: normals only;
    /// the renderer applies the plot-path-linearity gate on top.
    pub fn analytic_blur_transforms(
        &self,
        registry: &VariationRegistry,
    ) -> Vec<(usize, String, f32)> {
        self.transforms
            .iter()
            .enumerate()
            .filter_map(|(i, t)| t.analytic_blur(registry).map(|(n, w)| (i, n, w)))
            .collect()
    }

    /// Per-slot kernel inputs (variation name, weight, post-affine linear) for
    /// the first `MAX_BLUR_BUFFERS` eligible normals — the input to
    /// `analytic_blur::compute_blur_setup`. Order matches
    /// `GpuTransform::from_flame`'s slot assignment. Only meaningful when
    /// `analytic_blur_active`; empty otherwise.
    pub fn blur_slots(
        &self,
        registry: &VariationRegistry,
        render_mode: RenderMode,
    ) -> Vec<crate::variations::analytic_blur::BlurSlotInfo> {
        if !self.analytic_blur_active(registry, render_mode) {
            return Vec::new();
        }
        self.analytic_blur_transforms(registry)
            .into_iter()
            .take(crate::gpu::buffers::MAX_BLUR_BUFFERS as usize)
            .map(|(xform_idx, name, weight)| {
                let t = &self.transforms[xform_idx];
                let m_post = if t.post_affine_enabled {
                    [t.post_a, t.post_b, t.post_c, t.post_d]
                } else {
                    [1.0, 0.0, 0.0, 1.0]
                };
                crate::variations::analytic_blur::BlurSlotInfo { name, weight, m_post }
            })
            .collect()
    }

    /// Whole-flame analytic-blur activation gate: are there eligible
    /// transforms **and** is the plot path from a normal transform's output
    /// to the pixel a single linear map? v1 requires the linear tail:
    /// - **2D render mode** (the projection is the affine `world_to_pixel`).
    ///   3D — even orthographic — is deferred: depth-density compensation,
    ///   fog, and the camera projection complicate the mean splat.
    /// - **no Linked/Final chains** (`has_attachments`) — those re-run
    ///   variations after the normal transform, so the tail isn't linear.
    /// - **no subflames** and **no post-symmetry** — both fan one sample out
    ///   into multiple plot copies, which the single mean splat can't model.
    /// When false, the feature is entirely off: no `HAS_ANALYTIC_BLUR`
    /// codegen, no blur buffers, no convolution.
    pub fn analytic_blur_active(&self, registry: &VariationRegistry, render_mode: RenderMode) -> bool {
        matches!(render_mode, RenderMode::TwoD)
            && !self.has_attachments()
            && self.subflames.is_empty()
            && self.post_symmetry.ty == PostSymmetryType::None
            && !self.analytic_blur_transforms(registry).is_empty()
    }

    /// Per-flame cap on the AttachmentList struct's per-side array
    /// length. For each normal transform, takes the larger of its
    /// `linked_attachments` and `final_attachments` lengths, then takes
    /// the max of those values across all normals. Clamped to a minimum
    /// of 1 (WGSL forbids zero-sized arrays even when the struct is
    /// unused).
    ///
    /// The shader builder substitutes this into the `array<u32, N>`
    /// fields of the AttachmentList struct, so a flame whose normals
    /// each carry only one Final attachment loads a 16-byte struct per
    /// iteration instead of a 264-byte one (cap=1 vs the
    /// MAX_ATTACHMENTS_PER_TRANSFORM=32 worst case). Massive win for the
    /// migrated-singular-final case (every normal gets exactly one
    /// final auto-attached) which would otherwise pay the full 32-cap
    /// load every iteration.
    pub fn attachment_cap(&self) -> usize {
        let max_seen = self.transforms.iter()
            .map(|t| t.linked_attachments.len().max(t.final_attachments.len()))
            .max()
            .unwrap_or(0);
        max_seen.max(1)
    }

    /// Get runtime ID mapping for active variations.
    /// Delegates to `compute_local_index_map` using the flame's variation
    /// order (`active_variation_names_ordered`) and the per-flame cap.
    pub fn get_id_mapping(&self) -> HashMap<String, u32> {
        let registry = crate::variations::global_registry();
        compute_local_index_map(self.active_variation_names_ordered(&registry))
    }

    /// Calculate cumulative weights for transform selection
    pub fn cumulative_weights(&self) -> Vec<f32> {
        let mut cumulative = Vec::with_capacity(self.transforms.len());
        let mut sum = 0.0;
        for transform in &self.transforms {
            sum += transform.weight;
            cumulative.push(sum);
        }
        cumulative
    }

    /// Select a transform index based on random value
    pub fn select_transform(&self, cumulative_weights: &[f32], rand_val: f32) -> usize {
        let total = cumulative_weights.last().copied().unwrap_or(1.0);
        let target = rand_val * total;

        for (i, &cum_weight) in cumulative_weights.iter().enumerate() {
            if target <= cum_weight {
                return i;
            }
        }
        self.transforms.len().saturating_sub(1)
    }

}

// Custom deserializer for Flame to handle backward compatibility with old ProjectionType enum
impl<'de> Deserialize<'de> for Flame {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        #[derive(Deserialize)]
        #[serde(field_identifier, rename_all = "snake_case")]
        enum Field {
            Name,
            Transforms,
            FinalTransform,
            LinkedTransforms,
            FinalTransforms,
            Xaos,
            SoloTransform,
            Subflames,
            PostSymmetry,
            // The scene-level render fields (render_mode, perspective_strength,
            // depth_density_compensation, far_density_fade,
            // far_density_fade_start, preserve_z) and the legacy `projection`
            // field moved to `FractalConfig` in config v3 and are lifted by
            // the migration. Any remaining copies — e.g. inside nested
            // subflames of an old blob, where they were always ignored — fall
            // through to `Ignore` so loads don't fail. This also makes `Flame`
            // forward-compatible with unknown future keys.
            #[serde(other)]
            Ignore,
        }

        struct FlameVisitor;

        impl<'de> Visitor<'de> for FlameVisitor {
            type Value = Flame;

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
                formatter.write_str("struct Flame")
            }

            fn visit_map<V>(self, mut map: V) -> Result<Flame, V::Error>
            where
                V: MapAccess<'de>,
            {
                let mut name = None;
                let mut transforms: Option<Vec<Transform>> = None;
                let mut final_transform: Option<Transform> = None;
                let mut linked_transforms: Option<Vec<Transform>> = None;
                let mut final_transforms: Option<Vec<Transform>> = None;
                let mut xaos = None;
                let mut solo_transform = None;
                let mut subflames: Option<Vec<Flame>> = None;
                let mut post_symmetry: Option<PostSymmetry> = None;

                while let Some(key) = map.next_key()? {
                    match key {
                        Field::Name => {
                            name = Some(map.next_value()?);
                        }
                        Field::Transforms => {
                            transforms = Some(map.next_value()?);
                        }
                        Field::FinalTransform => {
                            final_transform = map.next_value()?;
                        }
                        Field::LinkedTransforms => {
                            linked_transforms = Some(map.next_value()?);
                        }
                        Field::FinalTransforms => {
                            final_transforms = Some(map.next_value()?);
                        }
                        Field::Xaos => {
                            xaos = Some(map.next_value()?);
                        }
                        Field::SoloTransform => {
                            solo_transform = Some(map.next_value()?);
                        }
                        Field::Subflames => {
                            subflames = Some(map.next_value()?);
                        }
                        Field::PostSymmetry => {
                            post_symmetry = Some(map.next_value()?);
                        }
                        Field::Ignore => {
                            let _ = map.next_value::<de::IgnoredAny>()?;
                        }
                    }
                }

                let transforms = transforms
                    .ok_or_else(|| de::Error::missing_field("transforms"))?;
                let linked_transforms = linked_transforms.unwrap_or_default();
                let final_transforms = final_transforms.unwrap_or_default();

                let mut flame = Flame {
                    // ID assigned by the post-deserialize `fixup_ids` pass.
                    id: 0,
                    name: name.unwrap_or_else(|| default_flame_name()),
                    transforms,
                    linked_transforms,
                    final_transforms,
                    xaos,
                    solo_transform: solo_transform.unwrap_or(None),
                    subflames: subflames.unwrap_or_default(),
                    post_symmetry: post_symmetry.unwrap_or_default(),
                };
                // Migrate any legacy singular `final_transform` field
                // (consumed locally above into `final_transform`) into the
                // new `final_transforms` pool with auto-attachment on every
                // normal. See
                // `docs/projects/per-transform-linked-and-final.md`
                // §"File format / migration".
                flame.migrate_legacy_final(final_transform);
                Ok(flame)
            }
        }

        const FIELDS: &[&str] = &["name", "transforms", "final_transform", "linked_transforms", "final_transforms", "xaos", "solo_transform", "subflames", "post_symmetry"];
        deserializer.deserialize_struct("Flame", FIELDS, FlameVisitor)
    }
}
