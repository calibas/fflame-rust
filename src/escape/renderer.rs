//! The escape-time GPU stage.
//!
//! A single compute pass per render: iterate every pixel, classify,
//! color, write an `Rgba32Float` image the flame tonemap tail consumes
//! via `tonemap_pass_with_input`. No accumulation loop — one dispatch
//! is the whole image at the configured `max_iter`.
//!
//! WebGPU discipline: buffer/texture `Drop` frees nothing on wasm, so
//! everything created here is destroyed in [`EscapeRenderer::destroy`]
//! (mirrors `EffectChainRunner`). Pipelines/bind-group layouts carry
//! no memory and are left to `Drop`.

use std::collections::HashMap;

use egui_wgpu::wgpu::{
    self, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, Buffer, BufferBindingType,
    BufferDescriptor, BufferUsages, CommandEncoder, ComputePassDescriptor, ComputePipeline,
    ComputePipelineDescriptor, Device, Extent3d, PipelineLayoutDescriptor, Queue,
    SamplerBindingType, SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages,
    StorageTextureAccess, Texture, TextureDescriptor, TextureDimension, TextureFormat,
    TextureSampleType, TextureUsages, TextureView, TextureViewDescriptor, TextureViewDimension,
};

use crate::config::escape::EscapeConfig;

use super::assembler::{self, PARAM_VEC4S};
use super::reference::OrbitCache;

/// Above this zoom the direct path's f32 pixel mapping visibly
/// pixelates: the center's f32 ulp (~6e-8 near |c| = 1) stops
/// resolving pixel spacing a couple of octaves before it equals it —
/// field-observed at zoom 16, so the switch sits at 14. The scaled
/// f32 delta pipeline holds to roughly zoom 54 (w-squared overflow);
/// the floatexp rung takes over beyond [`PERTURB_FLOATEXP_ZOOM`].
pub const PERTURB_MIN_ZOOM: f64 = 14.0;

/// Above this zoom the scaled-f32 delta rung approaches its w-squared
/// overflow (~zoom 54) and the floatexp rung takes over. Below it the
/// scaled rung is preferred: same images, several times faster.
pub const PERTURB_FLOATEXP_ZOOM: f64 = 48.0;

/// Iteration budget per perturbed dispatch, in pixel-iterations
/// (pixels x iterations). One unbounded dispatch at high max_iter is
/// a Windows TDR (driver reset; observed in the field as a 0xc0000409
/// abort at 200k iterations deep, reproduced as "Parent device is
/// lost" at 1080p). The budget targets a fraction of the 2-second TDR
/// window on a mid-range GPU; the floatexp rung's iterations cost
/// several times the scaled rung's, so its budget is smaller.
pub const PERTURB_CHUNK_BUDGET: u64 = 8_000_000_000;
// DF mantissas cost ~3-5x per iteration vs plain f32 CFe.
pub const PERTURB_CHUNK_BUDGET_FE: u64 = 600_000_000;

/// Uniform block — must match `EscapeParams` in the WGSL template
/// (std140: vec2 pairs pack the head, the vec4 arrays start at a
/// 16-byte boundary, total 192 bytes).
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct EscapeParamsGpu {
    center: [f32; 2],
    julia_c: [f32; 2],
    span: [f32; 2],
    rot_cs: [f32; 2],
    width: u32,
    height: u32,
    max_iter: u32,
    flags: u32,
    bailout: f32,
    _pad0: f32,
    /// Mann α (re, im); the shader reads it only in damped pipelines.
    damping: [f32; 2],
    fparams: [[f32; 4]; PARAM_VEC4S],
    cparams: [[f32; 4]; PARAM_VEC4S],
}

/// Uniform for the perturbed pipeline — must match `PerturbParams`
/// in the WGSL template.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct PerturbParamsGpu {
    s: f32,
    inv_s: f32,
    orbit_len: u32,
    /// bit 0: skip quadratic term; bit 1: Julia mode.
    flags: u32,
    /// Pixel spacing as mantissa * 2^exponent (floatexp rung) —
    /// computed symbolically from zoom_log2, valid at any depth.
    s_m: f32,
    s_e: i32,
    /// (view - reference) in pixel units (nucleus relocation).
    ref_offset: [f32; 2],
    /// Chunk window [iter_start, iter_end) for this dispatch.
    iter_start: u32,
    iter_end: u32,
    _pad_c0: u32,
    _pad_c1: u32,
}

pub struct EscapeRenderer {
    width: u32,
    height: u32,
    output_texture: Texture,
    output_view: TextureView,
    params_buffer: Buffer,
    /// Linear-filtering sampler for the palette (the flame buffers'
    /// shared sampler is non-filtering, chosen for the Rgba32Float
    /// accumulator; the palette is Rgba8Unorm and filters fine).
    palette_sampler: wgpu::Sampler,
    bind_group_layout: BindGroupLayout,
    /// Compiled pipelines keyed `"formula|coloring"` — tiny shaders,
    /// but a live panel flips combinations and recompiles add up.
    pipelines: HashMap<String, ComputePipeline>,
    /// Test-only: force the perturbed path regardless of zoom so the
    /// direct/perturbed agreement test can render the SAME shallow
    /// view both ways.
    #[cfg(test)]
    pub(crate) force_perturbed: bool,
    /// Test-only: with `force_perturbed`, use the floatexp rung.
    #[cfg(test)]
    pub(crate) force_floatexp: bool,
    /// Deep-zoom state: CPU reference-orbit cache (append-on-deepen),
    /// its GPU mirror, and the perturbed pipeline's own layout (two
    /// extra bindings: the orbit buffer and the perturb uniform).
    orbit_cache: OrbitCache,
    /// Progressive mode (the app sets this): reference orbits come
    /// from the worker thread and frames render with whatever prefix
    /// has landed — `render` reports whether the image is final.
    /// Off (the default), orbit compute is synchronous and exports
    /// stay deterministic.
    #[cfg(not(target_arch = "wasm32"))]
    pub progressive: bool,
    #[cfg(not(target_arch = "wasm32"))]
    orbit_worker: Option<super::reference::OrbitWorker>,
    /// Epoch of the worker data currently uploaded (progressive).
    #[cfg(not(target_arch = "wasm32"))]
    uploaded_epoch: u64,
    orbit_buffer: Option<Buffer>,
    /// DF residuals, parallel to orbit_buffer (binding 8).
    orbit_lo_buffer: Option<Buffer>,
    /// Per-entry reference exponents (binding 9), parallel to the
    /// orbit buffer.
    orbit_e_buffer: Option<Buffer>,
    orbit_capacity: u32,
    orbit_uploaded: u32,
    perturb_params_buffer: Buffer,
    perturb_bind_group_layout: BindGroupLayout,
    /// Relocation of the current reference (pixel units); fed into
    /// the perturb uniform each render.
    current_ref_offset: [f32; 2],
    /// Per-pixel iteration state for chunked perturbed dispatches
    /// (48 B/px, created on demand, explicit destroy).
    iter_state_buffer: Option<Buffer>,
    iter_state_px: u32,
    /// Next chunk's starting iteration and the render it belongs to;
    /// a key change restarts from chunk 0.
    chunk_next: u32,
    chunk_key: Option<String>,
    /// Test hook: shrink the chunk to force multi-chunk renders.
    #[cfg(test)]
    pub(crate) chunk_override: Option<u32>,
    /// BLA iteration-skip table (binding 7): GPU buffer + what it was
    /// built for. A zeroed dummy (n_levels = 0) is bound whenever the
    /// table is inapplicable, so there is no pipeline permutation.
    bla_buffer: Option<Buffer>,
    bla_dummy: Option<Buffer>,
    bla_built: Option<BlaBuilt>,
    /// Test hook: force per-step iteration to compare against skips.
    #[cfg(test)]
    pub(crate) disable_bla: bool,
    /// Display (output) size. `width`/`height` are the RENDER size =
    /// display × supersample; everything internal keys off those, so
    /// the whole pipeline (pixel spacing, iteration state, chunk
    /// keys, nucleus offsets) is supersampling-consistent for free.
    out_width: u32,
    out_height: u32,
    supersample: u32,
    /// Display-size target of the box downsample (None at 1×:
    /// `output_view` then serves the render texture directly).
    final_texture: Option<Texture>,
    final_view: Option<TextureView>,
    /// (factor, pipeline, layout) — the factor is spliced into the
    /// WGSL, so a factor change recompiles (rare).
    downsample: Option<(u32, wgpu::ComputePipeline, BindGroupLayout)>,
}

/// What the current BLA table was built for (rebuild trigger).
struct BlaBuilt {
    orbit_len: u32,
    power: u32,
    julia: bool,
    /// log2 of the |δc| bound the radii used (finite at ANY zoom).
    /// Zooming IN shrinks the actual bound below it (still
    /// conservative — no rebuild); widening the view past it forces
    /// one. Julia renders carry −∞ (δc = 0 exactly).
    dc_log2: f64,
}

impl EscapeRenderer {
    pub fn new(device: &Device, width: u32, height: u32) -> Self {
        let (output_texture, output_view) = Self::create_output(device, width, height);

        let params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Escape Params"),
            size: std::mem::size_of::<EscapeParamsGpu>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let palette_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Escape Palette Sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let perturb_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Escape Perturb Params"),
            size: std::mem::size_of::<PerturbParamsGpu>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let perturb_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Escape Perturbed Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba32Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 6,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 7,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 8,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 9,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Escape Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba32Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        Self {
            width,
            height,
            output_texture,
            output_view,
            params_buffer,
            palette_sampler,
            bind_group_layout,
            pipelines: HashMap::new(),
            #[cfg(test)]
            force_perturbed: false,
            #[cfg(test)]
            force_floatexp: false,
            orbit_cache: OrbitCache::default(),
            #[cfg(not(target_arch = "wasm32"))]
            progressive: false,
            #[cfg(not(target_arch = "wasm32"))]
            orbit_worker: None,
            #[cfg(not(target_arch = "wasm32"))]
            uploaded_epoch: 0,
            orbit_buffer: None,
            orbit_lo_buffer: None,
            orbit_e_buffer: None,
            orbit_capacity: 0,
            orbit_uploaded: 0,
            perturb_params_buffer,
            perturb_bind_group_layout,
            current_ref_offset: [0.0, 0.0],
            iter_state_buffer: None,
            iter_state_px: 0,
            chunk_next: 0,
            chunk_key: None,
            #[cfg(test)]
            chunk_override: None,
            bla_buffer: None,
            bla_dummy: None,
            bla_built: None,
            #[cfg(test)]
            disable_bla: false,
            out_width: width,
            out_height: height,
            supersample: 1,
            final_texture: None,
            final_view: None,
            downsample: None,
        }
    }

    /// Build/refresh the BLA table when skipping applies to this
    /// render; returns whether `bla_buffer` is current. Inapplicable
    /// (Ship tier, per-iteration colorings, unreachable CPU orbit):
    /// false, and the caller binds the zeroed dummy.
    fn ensure_bla(
        &mut self,
        device: &Device,
        queue: &Queue,
        escape: &EscapeConfig,
        orbit_len: u32,
        tier: assembler::PerturbTier,
        progressive: bool,
    ) -> bool {
        #[cfg(test)]
        if self.disable_bla {
            return false;
        }
        // Diagnostic escape hatch (all builds): isolate BLA-dependent
        // differences from per-step rendering.
        if std::env::var("ESCAPE_DISABLE_BLA").is_ok() {
            return false;
        }
        let power = match tier {
            assembler::PerturbTier::Power(p) => p.clamp(2, 12),
            assembler::PerturbTier::Ship(_) => return false,
        };
        // Skipped iterations never run the accumulator/period updates,
        // so those colorings keep the per-step path.
        let coloring = super::get_coloring(&escape.coloring);
        if coloring.has_feature(super::ColoringFeature::NeedsOrbitAccum)
            || coloring.has_feature(super::ColoringFeature::NeedsPeriod)
        {
            return false;
        }
        // |δc| bound over the viewport: half-diagonal plus the nucleus
        // relocation offset, in PIXEL units, carried against the pixel
        // spacing in LOG SPACE — an f64 absolute bound underflows past
        // ~zoom 1000, which used to disable BLA exactly where deep
        // renders need the skips most.
        let h = self.height.max(1) as f64;
        let dc_log2 = if escape.julia {
            f64::NEG_INFINITY
        } else {
            let half_diag = 0.5 * (self.width as f64).hypot(h);
            let off = (self.current_ref_offset[0] as f64).hypot(self.current_ref_offset[1] as f64);
            (half_diag + off).max(1e-30).log2() + (2.0 - escape.zoom_log2 - h.log2())
        };
        if let Some(b) = &self.bla_built {
            if b.orbit_len == orbit_len
                && b.power == power
                && b.julia == escape.julia
                && dc_log2 <= b.dc_log2 + 1e-9
            {
                return self.bla_buffer.is_some();
            }
        }
        // The CPU copy of the orbit the GPU mirror holds.
        // BLA takes PLAIN values: a near-nucleus iterate reads as
        // zero here exactly as it did before the exponent existed,
        // which zeroes that step's radius and simply makes the table
        // refuse to skip across the dip (conservative, never wrong).
        let with_exp = |hi: &[[f32; 2]], e: &[i32]| -> Vec<[f32; 2]> {
            hi.iter()
                .enumerate()
                .map(|(i, z)| {
                    super::reference::entry_value(*z, e.get(i).copied().unwrap_or(0))
                })
                .collect()
        };
        #[cfg(not(target_arch = "wasm32"))]
        let orbit_data: Option<Vec<[f32; 2]>> = if progressive {
            self.orbit_worker.as_ref().map(|wk| {
                let p = wk.progress.lock().unwrap();
                with_exp(&p.orbit, &p.orbit_e)
            })
        } else {
            self.orbit_cache
                .peek()
                .map(|o| with_exp(&o.orbit, &o.orbit_e))
        };
        #[cfg(target_arch = "wasm32")]
        let orbit_data: Option<Vec<[f32; 2]>> = {
            let _ = progressive;
            self.orbit_cache
                .peek()
                .map(|o| with_exp(&o.orbit, &o.orbit_e))
        };
        let Some(orbit_data) = orbit_data else {
            return false;
        };
        let usable = (orbit_len as usize).min(orbit_data.len());
        if usable < 3 {
            return false;
        }
        // Truncate at the reference's own escape: a skip riding the
        // escaped tail would overshoot the pixel's escape iteration
        // by up to the span.
        let bail = escape.bailout.max(1e-6) as f64;
        let mut prefix = usable;
        for (i, z) in orbit_data[..usable].iter().enumerate() {
            let q = (z[0] as f64) * (z[0] as f64) + (z[1] as f64) * (z[1] as f64);
            if q > bail {
                prefix = (i + 1).max(2);
                break;
            }
        }
        let dc = if dc_log2 == f64::NEG_INFINITY {
            super::bla::MagFe::zero()
        } else {
            let e = dc_log2.floor();
            super::bla::MagFe { m: 2f64.powf(dc_log2 - e), e: e as i64 }
        };
        let table =
            super::bla::BlaTable::build_with_dc(&orbit_data[..prefix], power, dc, dc_log2);
        let n_levels = table.levels.len().min(30);
        let total: usize = table.levels[..n_levels].iter().map(|l| l.len()).sum();
        if total == 0 {
            self.bla_built = None;
            return false;
        }
        let mut bytes = Vec::with_capacity(144 + total * 32);
        let mut offsets = [0u32; 32];
        let mut acc = 0u32;
        for (l, lev) in table.levels[..n_levels].iter().enumerate() {
            offsets[l] = acc;
            acc += lev.len() as u32;
        }
        for o in offsets {
            bytes.extend_from_slice(&o.to_le_bytes());
        }
        bytes.extend_from_slice(&(n_levels as u32).to_le_bytes());
        bytes.extend_from_slice(&((prefix - 1) as u32).to_le_bytes());
        bytes.extend_from_slice(&[0u8; 8]);
        let clamp_e = |e: i64| -> i32 { e.clamp(-1_000_000_000, 1_000_000_000) as i32 };
        for lev in &table.levels[..n_levels] {
            for ent in lev {
                bytes.extend_from_slice(&(ent.a.re as f32).to_le_bytes());
                bytes.extend_from_slice(&(ent.a.im as f32).to_le_bytes());
                bytes.extend_from_slice(&(ent.b.re as f32).to_le_bytes());
                bytes.extend_from_slice(&(ent.b.im as f32).to_le_bytes());
                bytes.extend_from_slice(&clamp_e(ent.a.e).to_le_bytes());
                bytes.extend_from_slice(&clamp_e(ent.b.e).to_le_bytes());
                let (rm, re) = if ent.r.m > 0.0 {
                    (ent.r.m as f32, clamp_e(ent.r.e))
                } else {
                    (0.0f32, 0)
                };
                bytes.extend_from_slice(&rm.to_le_bytes());
                bytes.extend_from_slice(&re.to_le_bytes());
            }
        }
        let size = bytes.len() as u64;
        let recreate = match &self.bla_buffer {
            Some(b) => b.size() < size,
            None => true,
        };
        if recreate {
            if let Some(old) = self.bla_buffer.take() {
                old.destroy();
            }
            // Headroom so progressive deepening doesn't recreate
            // every rebuild.
            self.bla_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape BLA Table"),
                size: (size + size / 2).max(176),
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
        }
        queue.write_buffer(self.bla_buffer.as_ref().unwrap(), 0, &bytes);
        self.bla_built = Some(BlaBuilt {
            orbit_len,
            power,
            julia: escape.julia,
            dc_log2,
        });
        true
    }

    fn chunk_size(&self, floatexp: bool) -> u32 {
        #[cfg(test)]
        if let Some(c) = self.chunk_override {
            return c;
        }
        let budget = if floatexp {
            PERTURB_CHUNK_BUDGET_FE
        } else {
            PERTURB_CHUNK_BUDGET
        };
        let px = (self.width as u64 * self.height as u64).max(1);
        // Floor 16, not 256: at supersampled resolutions (or plain
        // 4K+), 256 iterations x tens of megapixels is a multi-second
        // dispatch — the TDR class of crash the budget exists to
        // prevent. 16 still makes visible progress every frame.
        (budget / px).clamp(16, 65_536) as u32
    }

    /// Everything that invalidates in-flight chunk state.
    fn chunk_key_for(&self, escape: &EscapeConfig, orbit_len: u32) -> String {
        format!(
            "{}|{:?}|{}|{}|{}|{}|{}|{}|{:?}|{}x{}|{}",
            escape.formula,
            escape.formula_params,
            escape.coloring,
            escape.center_re,
            escape.center_im,
            escape.zoom_log2,
            escape.max_iter,
            escape.julia,
            escape.coloring_params,
            self.width,
            self.height,
            orbit_len,
        )
    }

    fn ensure_iter_state(&mut self, device: &Device) {
        let px = self.width * self.height;
        if self.iter_state_px != px || self.iter_state_buffer.is_none() {
            if let Some(old) = self.iter_state_buffer.take() {
                old.destroy();
            }
            self.iter_state_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Iter State"),
                size: (px as u64) * 48,
                usage: BufferUsages::STORAGE,
                mapped_at_creation: false,
            }));
            self.iter_state_px = px;
        }
    }

    /// Whether this view renders through the perturbation path:
    /// Mandelbrot parameter plane, plain iteration, past the direct
    /// path's f32 ceiling. Everything else stays direct (and deep
    /// zooms of unsupported combinations render the direct path's
    /// f32 mush honestly rather than wrong perturbation math).
    fn wants_perturbation(escape: &EscapeConfig) -> bool {
        escape.zoom_log2 > PERTURB_MIN_ZOOM
            && Self::perturb_tier(escape).is_some()
            && !escape.is_damped()
            && escape.biomorph == crate::config::escape::BiomorphMode::Off
    }

    /// The delta tier this view can use, if any: Mandelbrot (p = 2)
    /// and integer-power Multibrot (the binomial expansion needs an
    /// integer exponent), plus the plain Burning Ship variant via
    /// diffabs — SCALED RUNG ONLY (a floatexp diffabs is deferred, so
    /// Ship past the floatexp threshold falls back to the direct
    /// path's honest mush rather than wrong math).
    fn perturb_tier(escape: &EscapeConfig) -> Option<assembler::PerturbTier> {
        match escape.formula.as_str() {
            "mandelbrot" => Some(assembler::PerturbTier::Power(2)),
            "multibrot" => {
                let p = escape.formula_params.get("power").copied().unwrap_or(3.0);
                let rounded = p.round();
                if (p - rounded).abs() < 1e-6 && (2.0..=12.0).contains(&rounded) {
                    Some(assembler::PerturbTier::Power(rounded as u32))
                } else {
                    None
                }
            }
            "burning_ship" => {
                let variant = escape.formula_params.get("variant").copied().unwrap_or(0.0);
                let v = variant.round();
                if (variant - v).abs() < 1e-6 && (0.0..=5.0).contains(&v) {
                    Some(assembler::PerturbTier::Ship(v as u32))
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Ensure the perturbed pipeline for this coloring exists.
    fn ensure_perturbed_pipeline(
        &mut self,
        device: &Device,
        escape: &EscapeConfig,
        floatexp: bool,
    ) -> String {
        let coloring = super::get_coloring(&escape.coloring);
        let tier = Self::perturb_tier(escape)
            .unwrap_or(assembler::PerturbTier::Power(2));
        let key = format!("perturbed|{}|{}|{:?}", coloring.name, floatexp, tier);
        if !self.pipelines.contains_key(&key) {
            let source = assembler::assemble_perturbed(coloring, floatexp, tier);
            let module = device.create_shader_module(ShaderModuleDescriptor {
                label: Some(&format!("Escape Shader {key}")),
                source: ShaderSource::Wgsl(source.into()),
            });
            let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Escape Perturbed Pipeline Layout"),
                bind_group_layouts: &[Some(&self.perturb_bind_group_layout)],
                immediate_size: 0,
            });
            let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some(&format!("Escape Pipeline {key}")),
                layout: Some(&layout),
                module: &module,
                entry_point: Some("escape_main"),
                compilation_options: Default::default(),
                cache: None,
            });
            self.pipelines.insert(key.clone(), pipeline);
        }
        key
    }

    /// Progressive orbit acquisition: post the request to the worker,
    /// upload whatever prefix has landed. Returns
    /// (usable_len, complete); (0, _) means nothing to render yet.
    #[cfg(not(target_arch = "wasm32"))]
    fn ensure_orbit_progressive(
        &mut self,
        device: &Device,
        queue: &Queue,
        escape: &EscapeConfig,
    ) -> (u32, bool) {
        use super::reference::{OrbitRequest, OrbitWorker};
        let julia_c = if escape.julia {
            Some((escape.julia_re, escape.julia_im))
        } else {
            None
        };
        let tier = Self::perturb_tier(escape)
            .unwrap_or(assembler::PerturbTier::Power(2));
        let (power, ship, ship_variant) = match tier {
            assembler::PerturbTier::Power(p) => (p, false, 0),
            assembler::PerturbTier::Ship(v) => (2, true, v),
        };
        let height_px = self.height.max(1) as f64;
        let worker = self.orbit_worker.get_or_insert_with(OrbitWorker::new);
        let epoch = worker.request(OrbitRequest {
            center_re: escape.center_re.clone(),
            center_im: escape.center_im.clone(),
            n_limbs: super::fixedpoint::limbs_for_view(
                &escape.center_re,
                &escape.center_im,
                escape.zoom_log2,
            ),
            max_iter: escape.max_iter,
            julia_c,
            power,
            ship,
            ship_variant,
            reference_period: if julia_c.is_none() && !ship {
                escape.reference_period.filter(|&p| p > 0)
            } else {
                None
            },
            zoom_log2: escape.zoom_log2,
            height_px,
        });
        let (len, done, data, data_lo, data_e) = {
            let p = worker.progress.lock().unwrap();
            if p.epoch == epoch {
                // Rescale to this view (see the blocking path). The
                // reuse guard retires orbits whose relocation can't
                // rescale, so a None here is at most one transitional
                // frame — render it centered rather than displaced.
                self.current_ref_offset = super::reference::rescale_offset(
                    p.ref_offset,
                    p.off_zoom_log2,
                    p.off_height_px,
                    escape.zoom_log2,
                    self.height.max(1) as f64,
                )
                .unwrap_or([0.0, 0.0]);
            }
            if p.epoch == epoch {
                super::reference::set_live_reference_period(p.detected_period);
            }
            if p.epoch != epoch {
                (0u32, false, Vec::new(), Vec::new(), Vec::new())
            } else {
                let start = if self.uploaded_epoch == epoch {
                    self.orbit_uploaded as usize
                } else {
                    0
                };
                (
                    p.orbit.len() as u32,
                    p.done,
                    p.orbit[start.min(p.orbit.len())..].to_vec(),
                    p.orbit_lo[start.min(p.orbit_lo.len())..].to_vec(),
                    p.orbit_e[start.min(p.orbit_e.len())..].to_vec(),
                )
            }
        };
        if len < 2 {
            return (0, false);
        }
        // (Re)create the buffer as needed, then append the new tail.
        let fresh = self.uploaded_epoch != epoch;
        let recreate = self.orbit_buffer.is_none() || len > self.orbit_capacity || fresh && len > self.orbit_capacity;
        if recreate {
            if let Some(old) = self.orbit_buffer.take() {
                old.destroy();
            }
            if let Some(old) = self.orbit_lo_buffer.take() {
                old.destroy();
            }
            if let Some(old) = self.orbit_e_buffer.take() {
                old.destroy();
            }
            let capacity = (len + len / 2).max(1024);
            self.orbit_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Reference Orbit"),
                size: (capacity as u64) * 8,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.orbit_lo_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Reference Orbit Lo"),
                size: (capacity as u64) * 8,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.orbit_e_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Reference Orbit Exp"),
                size: (capacity as u64) * 4,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.orbit_capacity = capacity;
            self.orbit_uploaded = 0;
        }
        if fresh {
            // Epoch changed: upload from scratch (the worker republished
            // the full prefix under the new epoch).
            let worker = self.orbit_worker.as_ref().unwrap();
            let p = worker.progress.lock().unwrap();
            queue.write_buffer(
                self.orbit_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&p.orbit),
            );
            queue.write_buffer(
                self.orbit_lo_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&p.orbit_lo),
            );
            queue.write_buffer(
                self.orbit_e_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&p.orbit_e),
            );
            self.orbit_uploaded = p.orbit.len() as u32;
            self.uploaded_epoch = epoch;
        } else if !data.is_empty() {
            queue.write_buffer(
                self.orbit_buffer.as_ref().unwrap(),
                (self.orbit_uploaded as u64) * 8,
                bytemuck::cast_slice(&data),
            );
            queue.write_buffer(
                self.orbit_lo_buffer.as_ref().unwrap(),
                (self.orbit_uploaded as u64) * 8,
                bytemuck::cast_slice(&data_lo),
            );
            queue.write_buffer(
                self.orbit_e_buffer.as_ref().unwrap(),
                (self.orbit_uploaded as u64) * 4,
                bytemuck::cast_slice(&data_e),
            );
            self.orbit_uploaded += data.len() as u32;
        }
        (self.orbit_uploaded, done)
    }

    /// Compute/extend the reference orbit and mirror it to the GPU.
    /// Returns the usable orbit length, or None if the center failed
    /// to parse (caller falls back to the direct path).
    fn ensure_orbit(&mut self, device: &Device, queue: &Queue, escape: &EscapeConfig) -> Option<u32> {
        self.ensure_orbit_with(device, queue, escape, None).map(|(len, _)| len)
    }

    /// Blocking (`budget` None) or time-sliced (`budget` Some) orbit
    /// acquisition + GPU mirror. The sliced form is the WASM path's
    /// per-frame call — no worker thread exists there, so the orbit
    /// grows a bounded amount each frame and partial-orbit renders
    /// refine via rebasing, exactly like the desktop worker's
    /// progressive prefixes.
    fn ensure_orbit_with(
        &mut self,
        device: &Device,
        queue: &Queue,
        escape: &EscapeConfig,
        budget: Option<u32>,
    ) -> Option<(u32, bool)> {
        let julia_c = if escape.julia {
            Some((escape.julia_re, escape.julia_im))
        } else {
            None
        };
        let tier = Self::perturb_tier(escape)
            .unwrap_or(assembler::PerturbTier::Power(2));
        let (power, ship, ship_variant) = match tier {
            assembler::PerturbTier::Power(p) => (p, false, 0),
            assembler::PerturbTier::Ship(v) => (2, true, v),
        };
        let period_hint = if julia_c.is_none() && !ship {
            escape.reference_period.filter(|&p| p > 0)
        } else {
            None
        };
        self.orbit_cache.set_reference_period(period_hint);
        self.orbit_cache.set_height(self.height.max(1) as f64);
        // Retire a cached relocation this view can't express (zoomed
        // far out from where its nucleus was found) BEFORE borrowing
        // the slot — the recompute then happens inside get() at the
        // current view.
        if self
            .orbit_cache
            .peek()
            .is_some_and(|o| !o.relocation_serves(escape.zoom_log2, self.height.max(1) as f64))
        {
            self.orbit_cache.clear();
        }
        let (orbit, done) = match budget {
            None => (
                self.orbit_cache.get(
                    &escape.center_re,
                    &escape.center_im,
                    escape.zoom_log2,
                    escape.max_iter,
                    julia_c,
                    power,
                    ship,
                    ship_variant,
                )?,
                true,
            ),
            Some(b) => self.orbit_cache.get_budgeted(
                &escape.center_re,
                &escape.center_im,
                escape.zoom_log2,
                escape.max_iter,
                julia_c,
                power,
                ship,
                ship_variant,
                b,
            )?,
        };
        // The offset's pixel units are rescaled to THIS view (zoom
        // drags and supersampling change the units under a reused
        // orbit). A fresh orbit is always at the current view, so the
        // rescale is the identity there and the fallback never fires
        // after the pre-borrow retirement above.
        let h_px = self.height.max(1) as f64;
        self.current_ref_offset = orbit
            .offset_for_view(escape.zoom_log2, h_px)
            .unwrap_or([0.0, 0.0]);
        super::reference::set_live_reference_period(orbit.periodic);
        let len = orbit.len();
        let needed_bytes = (len as u64) * 8;
        let recreate = match &self.orbit_buffer {
            Some(_) => len > self.orbit_capacity,
            None => true,
        };
        if recreate {
            if let Some(old) = self.orbit_buffer.take() {
                old.destroy();
            }
            if let Some(old) = self.orbit_lo_buffer.take() {
                old.destroy();
            }
            if let Some(old) = self.orbit_e_buffer.take() {
                old.destroy();
            }
            // Grow with headroom so deepening doesn't recreate every
            // frame.
            let capacity = (len + len / 2).max(1024);
            self.orbit_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Reference Orbit"),
                size: (capacity as u64) * 8,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.orbit_lo_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Reference Orbit Lo"),
                size: (capacity as u64) * 8,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.orbit_e_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Reference Orbit Exp"),
                size: (capacity as u64) * 4,
                usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }));
            self.orbit_capacity = capacity;
            self.orbit_uploaded = 0;
        }
        if self.orbit_uploaded != len {
            // Upload the whole orbit (append-only uploads are a later
            // optimization; a full orbit at max_iter 100k is 800 KB).
            queue.write_buffer(
                self.orbit_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&orbit.orbit),
            );
            queue.write_buffer(
                self.orbit_lo_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&orbit.orbit_lo),
            );
            queue.write_buffer(
                self.orbit_e_buffer.as_ref().unwrap(),
                0,
                bytemuck::cast_slice(&orbit.orbit_e),
            );
            self.orbit_uploaded = len;
        }
        Some((len, done))
    }

    fn create_output(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Escape Output"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            // Storage write from the compute pass; read (textureLoad)
            // by the tonemap pass and the density-effect chain.
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        (texture, view)
    }

    /// The rendered image, in the flame-accumulator format the tonemap
    /// tail expects — display-sized (the downsampled target when
    /// supersampling is on).
    pub fn output_view(&self) -> &TextureView {
        self.final_view.as_ref().unwrap_or(&self.output_view)
    }

    /// Recreate the output for new DISPLAY dimensions and
    /// supersampling factor (render size = display × factor). Cheap
    /// relative to a render; pipelines and params survive. Returns
    /// true when anything changed — the output is stale until the
    /// next `render`.
    pub fn resize(&mut self, device: &Device, width: u32, height: u32, supersample: u32) -> bool {
        let mut ss = supersample.clamp(1, 3);
        // Cap the RENDER pixel count: the perturbed path carries
        // 48 B/px of iteration state (plus the 16 B/px accumulator),
        // and an unbounded supersample x display product is a device
        // OOM (observed as a device-loss abort). 32 Mpx ~ 1.5 GB of
        // state — reduce the factor until it fits rather than crash.
        const MAX_RENDER_PX: u64 = 32 * 1024 * 1024;
        // The perturbed path binds 48 B/px of iteration state as ONE
        // storage buffer; the device's binding limit (browsers often
        // grant far less than desktop adapters) caps render pixels
        // harder than the fixed ceiling.
        let device_px_cap = device.limits().max_storage_buffer_binding_size as u64 / 48;
        let px_cap = MAX_RENDER_PX.min(device_px_cap.max(1))
            .min({
                let d = device.limits().max_texture_dimension_2d as u64;
                d * d
            });
        while ss > 1
            && (width as u64 * ss as u64) * (height as u64 * ss as u64) > px_cap
        {
            ss -= 1;
        }
        if ss != supersample.clamp(1, 3) {
            log::warn!(
                "Escape supersample clamped to {ss}x at {width}x{height} (render-pixel budget)"
            );
        }
        if width == self.out_width && height == self.out_height && ss == self.supersample {
            return false;
        }
        self.out_width = width;
        self.out_height = height;
        self.supersample = ss;
        let (rw, rh) = (width.saturating_mul(ss).max(1), height.saturating_mul(ss).max(1));
        self.output_texture.destroy();
        let (texture, view) = Self::create_output(device, rw, rh);
        self.output_texture = texture;
        self.output_view = view;
        self.width = rw;
        self.height = rh;
        if let Some(t) = self.final_texture.take() {
            t.destroy();
        }
        self.final_view = None;
        if ss > 1 {
            let (t, v) = Self::create_output(device, width, height);
            self.final_texture = Some(t);
            self.final_view = Some(v);
        }
        true
    }

    /// The box-downsample pass: render texture → display texture,
    /// factor² samples averaged per output pixel. Linear-space (the
    /// texture is pre-tonemap accumulator format), so the average is
    /// the radiometrically correct one.
    fn run_downsample(&mut self, device: &Device, encoder: &mut CommandEncoder) {
        if self.supersample <= 1 {
            return;
        }
        let factor = self.supersample;
        let rebuild = match &self.downsample {
            Some((f, _, _)) => *f != factor,
            None => true,
        };
        if rebuild {
            let src = format!(
                r#"
@group(0) @binding(0) var src_tex: texture_2d<f32>;
@group(0) @binding(1) var dst_tex: texture_storage_2d<rgba32float, write>;

@compute @workgroup_size(8, 8, 1)
fn downsample_main(@builtin(global_invocation_id) gid: vec3<u32>) {{
    let dims = textureDimensions(dst_tex);
    if (gid.x >= dims.x || gid.y >= dims.y) {{
        return;
    }}
    var sum = vec4<f32>(0.0);
    for (var dy = 0u; dy < {factor}u; dy = dy + 1u) {{
        for (var dx = 0u; dx < {factor}u; dx = dx + 1u) {{
            sum = sum + textureLoad(
                src_tex,
                vec2<i32>(i32(gid.x * {factor}u + dx), i32(gid.y * {factor}u + dy)),
                0,
            );
        }}
    }}
    textureStore(
        dst_tex,
        vec2<i32>(i32(gid.x), i32(gid.y)),
        sum / f32({factor}u * {factor}u),
    );
}}
"#
            );
            let module = device.create_shader_module(ShaderModuleDescriptor {
                label: Some("Escape Downsample Shader"),
                source: ShaderSource::Wgsl(src.into()),
            });
            let layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
                label: Some("Escape Downsample Layout"),
                entries: &[
                    BindGroupLayoutEntry {
                        binding: 0,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::Texture {
                            sample_type: wgpu::TextureSampleType::Float { filterable: false },
                            view_dimension: wgpu::TextureViewDimension::D2,
                            multisampled: false,
                        },
                        count: None,
                    },
                    BindGroupLayoutEntry {
                        binding: 1,
                        visibility: ShaderStages::COMPUTE,
                        ty: BindingType::StorageTexture {
                            access: wgpu::StorageTextureAccess::WriteOnly,
                            format: TextureFormat::Rgba32Float,
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                ],
            });
            let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Escape Downsample Pipeline Layout"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });
            let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some("Escape Downsample Pipeline"),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some("downsample_main"),
                compilation_options: Default::default(),
                cache: None,
            });
            self.downsample = Some((factor, pipeline, layout));
        }
        let (_, pipeline, layout) = self.downsample.as_ref().unwrap();
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Escape Downsample Bind Group"),
            layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(&self.output_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(
                        self.final_view.as_ref().expect("final texture at ss > 1"),
                    ),
                },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Escape Downsample Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(self.out_width.div_ceil(8), self.out_height.div_ceil(8), 1);
    }

    /// Compile (or fetch from cache) the pipeline for this config's
    /// (formula, coloring) pair; returns its cache key.
    fn ensure_pipeline(&mut self, device: &Device, escape: &EscapeConfig) -> String {
        // Mode B routing: a formula name resolving in the FIELD
        // registry compiles the field template instead. Same bind
        // group layout, same dispatch — only the shader differs.
        let (key, source_for) = if let Some(field) = super::fields::get_field(&escape.formula) {
            let coloring = super::fields::get_field_coloring(&escape.coloring, field);
            (
                format!("field|{}|{}", field.name, coloring.name),
                Some(assembler::assemble_field(field, coloring)),
            )
        } else {
            (String::new(), None)
        };
        if let Some(source) = source_for {
            if !self.pipelines.contains_key(&key) {
                let module = device.create_shader_module(ShaderModuleDescriptor {
                    label: Some(&format!("Escape Shader {key}")),
                    source: ShaderSource::Wgsl(source.into()),
                });
                let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                    label: Some("Escape Pipeline Layout"),
                    bind_group_layouts: &[Some(&self.bind_group_layout)],
                    immediate_size: 0,
                });
                let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                    label: Some(&format!("Escape Pipeline {key}")),
                    layout: Some(&layout),
                    module: &module,
                    entry_point: Some("escape_main"),
                    compilation_options: Default::default(),
                    cache: None,
                });
                self.pipelines.insert(key.clone(), pipeline);
            }
            return key;
        }
        let formula = super::get_formula(&escape.formula);
        let coloring = super::get_coloring(&escape.coloring);
        let damped = escape.is_damped();
        let key = format!("{}|{}|{}", formula.name, coloring.name, damped);
        if !self.pipelines.contains_key(&key) {
            let source = assembler::assemble(formula, coloring, damped);
            let module = device.create_shader_module(ShaderModuleDescriptor {
                label: Some(&format!("Escape Shader {key}")),
                source: ShaderSource::Wgsl(source.into()),
            });
            let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some("Escape Pipeline Layout"),
                bind_group_layouts: &[Some(&self.bind_group_layout)],
                immediate_size: 0,
            });
            let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some(&format!("Escape Pipeline {key}")),
                layout: Some(&layout),
                module: &module,
                entry_point: Some("escape_main"),
                compilation_options: Default::default(),
                cache: None,
            });
            self.pipelines.insert(key.clone(), pipeline);
        }
        key
    }

    fn params_for(&self, escape: &EscapeConfig) -> EscapeParamsGpu {
        // zoom_log2 = 0 is the home view: vertical span 4 complex
        // units (the EscapeConfig doc contract); width follows aspect.
        let span_y = 4.0 / escape.zoom_factor();
        let span_x = span_y * (self.width as f64 / self.height.max(1) as f64);
        let (cx, cy) = escape.center_f64();

        let mut fparams = [[0.0f32; 4]; PARAM_VEC4S];
        let mut cparams = [[0.0f32; 4]; PARAM_VEC4S];
        if let Some(field) = super::fields::get_field(&escape.formula) {
            // Mode B: pack the field's params + its resolved coloring's.
            let coloring = super::fields::get_field_coloring(&escape.coloring, field);
            super::pack_params(field.parameters, &escape.formula_params, fparams.as_flattened_mut());
            super::pack_params(coloring.parameters, &escape.coloring_params, cparams.as_flattened_mut());
        } else {
            let formula = super::get_formula(&escape.formula);
            let coloring = super::get_coloring(&escape.coloring);
            super::pack_params(formula.parameters, &escape.formula_params, fparams.as_flattened_mut());
            super::pack_params(coloring.parameters, &escape.coloring_params, cparams.as_flattened_mut());
        }

        EscapeParamsGpu {
            center: [cx as f32, cy as f32],
            julia_c: [escape.julia_re, escape.julia_im],
            span: [span_x as f32, span_y as f32],
            rot_cs: [escape.rotation.cos(), escape.rotation.sin()],
            width: self.width,
            height: self.height,
            max_iter: escape.max_iter.max(1),
            flags: {
                // bit 0 = Julia; bits 1-2 = biomorph classification axis.
                let bio = match escape.biomorph {
                    crate::config::escape::BiomorphMode::Off => 0u32,
                    crate::config::escape::BiomorphMode::Re => 1,
                    crate::config::escape::BiomorphMode::Im => 2,
                };
                (if escape.julia { 1 } else { 0 }) | (bio << 1)
            },
            bailout: escape.bailout.max(1e-6),
            _pad0: 0.0,
            damping: [escape.damping_re, escape.damping_im],
            fparams,
            cparams,
        }
    }

    /// One full-image escape pass into the output texture.
    ///
    /// `palette_view` is the flame renderer's palette texture (already
    /// carrying rotation/squeeze from `update_palette`), so escape mode
    /// inherits the whole palette pipeline for free.
    /// Returns whether the rendered image is FINAL (false only in
    /// progressive mode while the reference orbit is still growing —
    /// the caller should keep the escape image marked dirty).
    pub fn render(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        escape: &EscapeConfig,
        palette_view: &TextureView,
    ) -> bool {
        let params = self.params_for(escape);
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Deep zoom: the perturbation path. Falls back to direct on a
        // center-parse failure (matching center_f64's fallback view).
        #[cfg(test)]
        let use_perturbed = Self::wants_perturbation(escape) || self.force_perturbed;
        #[cfg(not(test))]
        let use_perturbed = Self::wants_perturbation(escape);
        if use_perturbed {
            #[cfg(not(target_arch = "wasm32"))]
            let progressive = self.progressive;
            #[cfg(target_arch = "wasm32")]
            let progressive = false;
            let orbit_state: Option<(u32, bool)> = if progressive {
                #[cfg(not(target_arch = "wasm32"))]
                {
                    let (len, done) = self.ensure_orbit_progressive(device, queue, escape);
                    if len == 0 {
                        // Nothing to render yet: keep the previous
                        // frame's texture, come back next frame.
                        return false;
                    }
                    Some((len, done))
                }
                #[cfg(target_arch = "wasm32")]
                None
            } else {
                // WASM has no worker thread: slice the fixed-point
                // compute per frame (budget shrinks with limb count so
                // a slice stays in tens of milliseconds) and let the
                // partial orbit render progressively via rebasing.
                // Desktop non-progressive (CLI, tests) stays blocking
                // and deterministic.
                #[cfg(target_arch = "wasm32")]
                {
                    let limbs =
                        super::fixedpoint::limbs_for_zoom(escape.zoom_log2).max(1) as u32;
                    let budget = (1_000_000 / limbs).clamp(256, 50_000);
                    match self.ensure_orbit_with(device, queue, escape, Some(budget)) {
                        Some((len, done)) if len >= 2 => Some((len, done)),
                        Some(_) => return false,
                        None => None,
                    }
                }
                #[cfg(not(target_arch = "wasm32"))]
                {
                    self.ensure_orbit(device, queue, escape).map(|l| (l, true))
                }
            };
            if let Some((orbit_len, orbit_done)) = orbit_state {
                #[cfg(test)]
                let floatexp = escape.zoom_log2 > PERTURB_FLOATEXP_ZOOM || self.force_floatexp;
                #[cfg(not(test))]
                let floatexp = escape.zoom_log2 > PERTURB_FLOATEXP_ZOOM;
                // Pixel spacing S = 2^(2 - zoom) / height. The scaled
                // rung takes it as f64->f32 (normal down to ~zoom 119,
                // past its own ceiling); the floatexp rung takes it
                // SYMBOLICALLY as mantissa * 2^exponent so no float
                // ever underflows however deep the zoom goes.
                let h = self.height.max(1) as f64;
                let s_f64 = if escape.zoom_log2 < 1000.0 {
                    4.0 / escape.zoom_factor() / h
                } else {
                    0.0
                };
                let x = 2.0 - escape.zoom_log2 - h.log2();
                let s_e = x.floor();
                let s_m = 2f64.powf(x - s_e);
                // Chunk window: restart on any render-state change,
                // else continue where the last dispatch stopped.
                let chunk = self.chunk_size(floatexp);
                let key = self.chunk_key_for(escape, orbit_len);
                if self.chunk_key.as_deref() != Some(key.as_str()) {
                    self.chunk_key = Some(key);
                    self.chunk_next = 0;
                }
                let iter_start = self.chunk_next.min(escape.max_iter);
                let iter_end = iter_start.saturating_add(chunk).min(escape.max_iter);
                self.ensure_iter_state(device);
                // BLA table: build/refresh when skipping applies,
                // else bind the zeroed dummy (n_levels = 0).
                let tier = Self::perturb_tier(escape)
                    .unwrap_or(assembler::PerturbTier::Power(2));
                let bla_ready =
                    self.ensure_bla(device, queue, escape, orbit_len, tier, progressive);
                if self.bla_dummy.is_none() {
                    self.bla_dummy = Some(device.create_buffer(&BufferDescriptor {
                        label: Some("Escape BLA Dummy"),
                        size: 176,
                        usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
                        mapped_at_creation: false,
                    }));
                }
                let pp = PerturbParamsGpu {
                    s: s_f64 as f32,
                    inv_s: if s_f64 > 0.0 { (1.0 / s_f64) as f32 } else { 0.0 },
                    orbit_len: orbit_len.max(2),
                    flags: if escape.julia { 2 } else { 0 },
                    s_m: s_m as f32,
                    s_e: s_e as i32,
                    ref_offset: self.current_ref_offset,
                    iter_start,
                    iter_end,
                    _pad_c0: 0,
                    _pad_c1: 0,
                };
                queue.write_buffer(&self.perturb_params_buffer, 0, bytemuck::bytes_of(&pp));

                let key = self.ensure_perturbed_pipeline(device, escape, floatexp);
                let bind_group = device.create_bind_group(&BindGroupDescriptor {
                    label: Some("Escape Perturbed Bind Group"),
                    layout: &self.perturb_bind_group_layout,
                    entries: &[
                        BindGroupEntry {
                            binding: 0,
                            resource: self.params_buffer.as_entire_binding(),
                        },
                        BindGroupEntry {
                            binding: 1,
                            resource: BindingResource::TextureView(&self.output_view),
                        },
                        BindGroupEntry {
                            binding: 2,
                            resource: BindingResource::TextureView(palette_view),
                        },
                        BindGroupEntry {
                            binding: 3,
                            resource: BindingResource::Sampler(&self.palette_sampler),
                        },
                        BindGroupEntry {
                            binding: 4,
                            resource: self.orbit_buffer.as_ref().unwrap().as_entire_binding(),
                        },
                        BindGroupEntry {
                            binding: 5,
                            resource: self.perturb_params_buffer.as_entire_binding(),
                        },
                        BindGroupEntry {
                            binding: 6,
                            resource: self
                                .iter_state_buffer
                                .as_ref()
                                .unwrap()
                                .as_entire_binding(),
                        },
                        BindGroupEntry {
                            binding: 7,
                            resource: if bla_ready {
                                self.bla_buffer.as_ref().unwrap().as_entire_binding()
                            } else {
                                self.bla_dummy.as_ref().unwrap().as_entire_binding()
                            },
                        },
                        BindGroupEntry {
                            binding: 8,
                            resource: self
                                .orbit_lo_buffer
                                .as_ref()
                                .unwrap()
                                .as_entire_binding(),
                        },
                        BindGroupEntry {
                            binding: 9,
                            resource: self
                                .orbit_e_buffer
                                .as_ref()
                                .unwrap()
                                .as_entire_binding(),
                        },
                    ],
                });
                let pipeline = &self.pipelines[&key];
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Escape Perturbed Pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
                drop(pass);
                // Every chunk refreshes the display image, so
                // progressive refinement stays visible under AA.
                self.run_downsample(device, encoder);
                let iterations_done = iter_end >= escape.max_iter;
                self.chunk_next = if iterations_done { 0 } else { iter_end };
                if iterations_done {
                    // A repeat of the same render starts fresh.
                    self.chunk_key = None;
                }
                return orbit_done && iterations_done;
            }
            log::warn!("Deep zoom requested but the center failed to parse; rendering direct");
        }

        // Built per pass: the palette view can be recreated under us
        // (palette-size changes), and one bind group per render is
        // noise next to the dispatch itself.
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Escape Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: self.params_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::TextureView(&self.output_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(palette_view),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::Sampler(&self.palette_sampler),
                },
            ],
        });

        let key = self.ensure_pipeline(device, escape);
        let pipeline = &self.pipelines[&key];

        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Escape Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(self.width.div_ceil(8), self.height.div_ceil(8), 1);
        drop(pass);
        self.run_downsample(device, encoder);
        true
    }

    /// Free GPU memory explicitly — on WebGPU `Drop` frees nothing.
    /// Idempotent; safe once no submitted work references the texture.
    pub fn destroy(&self) {
        self.output_texture.destroy();
        self.params_buffer.destroy();
        self.perturb_params_buffer.destroy();
        if let Some(b) = &self.orbit_buffer {
            b.destroy();
        }
        if let Some(b) = &self.orbit_e_buffer {
            b.destroy();
        }
        if let Some(b) = &self.orbit_lo_buffer {
            b.destroy();
        }
        if let Some(b) = &self.iter_state_buffer {
            b.destroy();
        }
        if let Some(b) = &self.bla_buffer {
            b.destroy();
        }
        if let Some(b) = &self.bla_dummy {
            b.destroy();
        }
        if let Some(t) = &self.final_texture {
            t.destroy();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn perturbation_gate_is_tight() {
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.zoom_log2 = 30.0;
        assert!(EscapeRenderer::wants_perturbation(&esc));
        esc.zoom_log2 = 10.0;
        assert!(!EscapeRenderer::wants_perturbation(&esc), "shallow stays direct");
        esc.zoom_log2 = 30.0;
        esc.julia = true;
        assert!(EscapeRenderer::wants_perturbation(&esc), "julia is in the trivial tier");
        esc.julia = false;
        esc.formula = "burning_ship".to_string();
        assert!(
            EscapeRenderer::wants_perturbation(&esc),
            "the Ship family is in the diffabs tier"
        );
        esc.zoom_log2 = 60.0;
        assert!(
            EscapeRenderer::wants_perturbation(&esc),
            "deep Ship rides the floatexp diffabs rung"
        );
        esc.zoom_log2 = 30.0;
        esc.formula_params.insert("variant".to_string(), 3.0);
        assert!(
            EscapeRenderer::wants_perturbation(&esc),
            "every fold variant has its own delta algebra now"
        );
        esc.formula_params.clear();
        esc.formula = "weierstrass".to_string();
        assert!(
            !EscapeRenderer::wants_perturbation(&esc),
            "mode B fields never perturb"
        );
        esc.formula = "burning_ship".to_string();
        esc.formula = "multibrot".to_string();
        assert!(EscapeRenderer::wants_perturbation(&esc), "integer multibrot is in the tier");
        esc.formula_params.insert("power".to_string(), 3.5);
        assert!(
            !EscapeRenderer::wants_perturbation(&esc),
            "non-integer power stays direct (binomial needs an integer exponent)"
        );
        esc.formula_params.clear();
        esc.formula = "mandelbrot".to_string();
        esc.damping_re = 0.5;
        assert!(!EscapeRenderer::wants_perturbation(&esc), "damped not in v1");
    }

    #[test]
    fn params_struct_matches_wgsl_layout() {
        // 4 vec2 (32) + 4 u32 (16) + f32 + 3 pad (16) + 2 param arrays
        // (128) = 192, and the arrays must start 16-byte aligned.
        assert_eq!(std::mem::size_of::<EscapeParamsGpu>(), 192);
        assert_eq!(std::mem::offset_of!(EscapeParamsGpu, fparams), 64);
        assert_eq!(std::mem::offset_of!(EscapeParamsGpu, cparams), 128);
    }
}
