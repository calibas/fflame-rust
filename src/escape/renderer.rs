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
pub const PERTURB_CHUNK_BUDGET_FE: u64 = 2_000_000_000;

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
        }
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
        (budget / px).clamp(256, 65_536) as u32
    }

    /// Everything that invalidates in-flight chunk state.
    fn chunk_key_for(&self, escape: &EscapeConfig, orbit_len: u32) -> String {
        format!(
            "{}|{}|{}|{}|{}|{}|{}|{:?}|{}x{}|{}",
            escape.formula,
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
                if variant.abs() < 1e-6 && escape.zoom_log2 <= PERTURB_FLOATEXP_ZOOM {
                    Some(assembler::PerturbTier::Ship)
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
        let (power, ship) = match tier {
            assembler::PerturbTier::Power(p) => (p, false),
            assembler::PerturbTier::Ship => (2, true),
        };
        let height_px = self.height.max(1) as f64;
        let worker = self.orbit_worker.get_or_insert_with(OrbitWorker::new);
        let epoch = worker.request(OrbitRequest {
            center_re: escape.center_re.clone(),
            center_im: escape.center_im.clone(),
            n_limbs: super::fixedpoint::limbs_for_zoom(escape.zoom_log2),
            max_iter: escape.max_iter,
            julia_c,
            power,
            ship,
            zoom_log2: escape.zoom_log2,
            height_px,
        });
        let (len, done, data) = {
            let p = worker.progress.lock().unwrap();
            if p.epoch == epoch {
                self.current_ref_offset = p.ref_offset;
            }
            if p.epoch != epoch {
                (0u32, false, Vec::new())
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
            let capacity = (len + len / 2).max(1024);
            self.orbit_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Reference Orbit"),
                size: (capacity as u64) * 8,
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
            self.orbit_uploaded = p.orbit.len() as u32;
            self.uploaded_epoch = epoch;
        } else if !data.is_empty() {
            queue.write_buffer(
                self.orbit_buffer.as_ref().unwrap(),
                (self.orbit_uploaded as u64) * 8,
                bytemuck::cast_slice(&data),
            );
            self.orbit_uploaded += data.len() as u32;
        }
        (self.orbit_uploaded, done)
    }

    /// Compute/extend the reference orbit and mirror it to the GPU.
    /// Returns the usable orbit length, or None if the center failed
    /// to parse (caller falls back to the direct path).
    fn ensure_orbit(&mut self, device: &Device, queue: &Queue, escape: &EscapeConfig) -> Option<u32> {
        let julia_c = if escape.julia {
            Some((escape.julia_re, escape.julia_im))
        } else {
            None
        };
        let tier = Self::perturb_tier(escape)
            .unwrap_or(assembler::PerturbTier::Power(2));
        let (power, ship) = match tier {
            assembler::PerturbTier::Power(p) => (p, false),
            assembler::PerturbTier::Ship => (2, true),
        };
        self.orbit_cache.set_height(self.height.max(1) as f64);
        let orbit = self.orbit_cache.get(
            &escape.center_re,
            &escape.center_im,
            escape.zoom_log2,
            escape.max_iter,
            julia_c,
            power,
            ship,
        )?;
        self.current_ref_offset = orbit.ref_offset;
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
            // Grow with headroom so deepening doesn't recreate every
            // frame.
            let capacity = (len + len / 2).max(1024);
            self.orbit_buffer = Some(device.create_buffer(&BufferDescriptor {
                label: Some("Escape Reference Orbit"),
                size: (capacity as u64) * 8,
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
            self.orbit_uploaded = len;
        }
        Some(len)
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
    /// tail expects.
    pub fn output_view(&self) -> &TextureView {
        &self.output_view
    }

    /// Recreate the output for new dimensions. Cheap relative to a
    /// render; pipelines and params survive. Returns true when the size
    /// actually changed — the output is stale until the next `render`.
    pub fn resize(&mut self, device: &Device, width: u32, height: u32) -> bool {
        if width == self.width && height == self.height {
            return false;
        }
        self.output_texture.destroy();
        let (texture, view) = Self::create_output(device, width, height);
        self.output_texture = texture;
        self.output_view = view;
        self.width = width;
        self.height = height;
        true
    }

    /// Compile (or fetch from cache) the pipeline for this config's
    /// (formula, coloring) pair; returns its cache key.
    fn ensure_pipeline(&mut self, device: &Device, escape: &EscapeConfig) -> String {
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
        let formula = super::get_formula(&escape.formula);
        let coloring = super::get_coloring(&escape.coloring);

        // zoom_log2 = 0 is the home view: vertical span 4 complex
        // units (the EscapeConfig doc contract); width follows aspect.
        let span_y = 4.0 / escape.zoom_factor();
        let span_x = span_y * (self.width as f64 / self.height.max(1) as f64);
        let (cx, cy) = escape.center_f64();

        let mut fparams = [[0.0f32; 4]; PARAM_VEC4S];
        let mut cparams = [[0.0f32; 4]; PARAM_VEC4S];
        super::pack_params(formula.parameters, &escape.formula_params, fparams.as_flattened_mut());
        super::pack_params(coloring.parameters, &escape.coloring_params, cparams.as_flattened_mut());

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
                self.ensure_orbit(device, queue, escape).map(|l| (l, true))
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
        if let Some(b) = &self.iter_state_buffer {
            b.destroy();
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
            "plain Ship is in the diffabs tier (scaled rung)"
        );
        esc.zoom_log2 = 60.0;
        assert!(
            !EscapeRenderer::wants_perturbation(&esc),
            "Ship past the floatexp threshold stays direct (no floatexp diffabs yet)"
        );
        esc.zoom_log2 = 30.0;
        esc.formula_params.insert("variant".to_string(), 3.0);
        assert!(
            !EscapeRenderer::wants_perturbation(&esc),
            "non-plain Ship variants stay direct"
        );
        esc.formula_params.clear();
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
