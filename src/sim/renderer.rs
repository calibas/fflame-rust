//! `SimRenderer` — the stateful grid, its passes, and the step loop.
//!
//! Owns a **ping-pong pair** of `Rgba32Float` textures at the
//! simulation grid size, plus one output image at the display size in
//! the flame accumulator's layout, so the shared tonemap → effects →
//! readback tail consumes it exactly as it consumes escape's.
//!
//! Why a pair rather than one texture: wgpu rejects read-write storage
//! access on `rgba32float`, so a step reads `field[i]` as a sampled
//! texture and writes `field[1 - i]` as a write-only storage texture.
//! That is the only portable shape, and everything else here follows
//! from it.
//!
//! Three things this renderer is responsible for that the escape one
//! is not:
//!
//! * **State across frames.** `step_index` and the field survive the
//!   frame; a still is "the state at step N from this seed". Reseeding
//!   is explicit.
//! * **The grid is not the output.** Colouring happens at grid
//!   resolution and the same pass resolves to the output size, so a
//!   256-cell model shown at 1080p stays 256 cells of information.
//! * **Batching against the watchdog.** A 10,000-step export cannot be
//!   one submission. [`Self::run_steps`] splits by a measured budget;
//!   phase 0 established that per-submit overhead is 0.8% across a
//!   256x range, so batching is purely a watchdog and pacing device.

use crate::config::sim::{SimConfig, SimGrid};
use crate::sim::{assembler, coloring_or_default, model_or_default, ModelDef, SimColoringDef};
#[allow(unused_imports)]
use crate::sim::ColoringFeature;
use wgpu::util::DeviceExt;
use wgpu::*;

/// Most steps in one submission, and the size of the uniform ring.
///
/// This used to be THE batch size, justified by phase 0's numbers
/// (a 1080p stencil step at 0.5 ms, so 256 steps is ~0.1 s). Those
/// numbers were for a 3×3 stencil. Cyclic CA at range 5 is 121 reads
/// a cell and 9.7 ms a step at 1080p, so one 256-step submit is 2.5 s
/// — past Windows' 2 s GPU watchdog, which resets the device. The
/// fence signals anyway, the run reports a fictional cost, and the
/// process aborts at teardown; the shipped binary's `export` of that
/// config failed with "Parent device is lost". Pinned between 192
/// steps (1.8 s, clean) and 224 (2.3 s, lost). So the batch is now
/// sized from measured cost, and this is only its ceiling.
const MAX_STEPS_PER_SUBMIT: u32 = 256;

/// Steps in the first submission after a pipeline or grid change,
/// before anything has been measured. Small enough that even a kernel
/// an order of magnitude slower than range-5 cyclic CA (phase 3's
/// large-kernel models) stays well inside the watchdog: 8 steps at
/// 60 ms is half a second.
const FIRST_SUBMIT: u32 = 8;

/// Wall-clock budget for one submission. An eighth of the watchdog,
/// so a card half as fast as the one measured on, or a frame that
/// shares the GPU with something else, still has margin; and long
/// enough that at 0.3 ms a step the ceiling is what binds.
const SUBMIT_BUDGET_MS: f64 = 250.0;

/// Cap on a single dimension of the grid. The real limit is the
/// device's `max_texture_dimension_2d`, which [`SimRenderer::allocation_error`]
/// checks; this keeps arithmetic sane before a device is in scope.
const MAX_GRID_DIM: u32 = 8192;

/// Ring slots for per-step uniforms. One per step in a submission, so
/// each step reads its own step index.
const RING_SLOTS: u32 = MAX_STEPS_PER_SUBMIT;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct SimParamsGpu {
    grid: [u32; 2],
    out_size: [u32; 2],
    step_index: u32,
    seed_lo: u32,
    seed_hi: u32,
    dt: f32,
    init_p0: f32,
    init_p1: f32,
    _pad0: f32,
    _pad1: f32,
}

/// The shader set for one (model, colouring, boundary, resolve)
/// combination. Rebuilt when any of those change, which is rare —
/// parameter edits do not touch it.
struct Pipelines {
    seed: ComputePipeline,
    step: ComputePipeline,
    color: ComputePipeline,
    seed_layout: BindGroupLayout,
    step_layout: BindGroupLayout,
    color_layout: BindGroupLayout,
    /// What the pipelines were built for, so the renderer knows when
    /// they are stale.
    key: PipelineKey,
}

#[derive(Clone, PartialEq, Eq)]
struct PipelineKey {
    model: &'static str,
    coloring: &'static str,
    boundary: crate::config::sim::SimBoundary,
    upscale: crate::config::sim::SimUpscale,
    downscale: crate::config::sim::SimDownscale,
    // `&'static`: `SimInit::kind_name` returns one, and this key is
    // rebuilt three times per frame -- a String here was three
    // allocations per frame for nothing.
    init_kind: &'static str,
    magnifying: bool,
}

pub struct SimRenderer {
    /// Simulation grid, in cells.
    grid_w: u32,
    grid_h: u32,
    /// Output image, in pixels.
    out_w: u32,
    out_h: u32,

    field: [Texture; 2],
    field_view: [TextureView; 2],
    /// Which of the pair currently holds the live state.
    current: usize,

    output_texture: Texture,
    output_view: TextureView,

    /// A RING of per-step uniforms, addressed by dynamic offset.
    ///
    /// Not one uniform rewritten per step: `Queue::write_buffer` is
    /// staged and applied before the command buffer executes, so every
    /// step in a submission would read the LAST step's values. Measured,
    /// that silently corrupted the age channel in every batched run
    /// (93 texels of 4,096 differed between one batch of 300 and 300
    /// batches of one) while the concentration channels looked right --
    /// and it cost a `write_buffer` per step, which is what made 4
    /// steps at 1080p take 39.8 ms instead of ~2.
    params_buffer: Buffer,
    /// Distance between ring slots, `min_uniform_buffer_offset_alignment`
    /// rounded up from the struct size.
    params_stride: u64,
    model_params_buffer: Buffer,
    coloring_params_buffer: Buffer,

    pipelines: Option<Pipelines>,
    /// One bind group per ping-pong direction, built with the
    /// pipelines and reused for every step in every batch.
    step_bind_groups: Option<[BindGroup; 2]>,

    /// Steps applied since the last reseed. The state's identity.
    step_index: u32,
    /// Steps in the next submission: adapted from the measured cost
    /// of the previous one, reset to [`FIRST_SUBMIT`] whenever the
    /// pipeline or the grid changes and the old measurement no longer
    /// describes the kernel.
    steps_per_submit: u32,
    /// Set when the field has not been seeded yet, or the config
    /// changed in a way that invalidates it.
    needs_seed: bool,
}

impl SimRenderer {
    pub fn new(device: &Device, cfg: &SimConfig, out_w: u32, out_h: u32) -> Self {
        let (grid_w, grid_h) = Self::allocatable_grid(cfg, out_w, out_h);
        let (field, field_view) = Self::create_field_pair(device, grid_w, grid_h);
        let (output_texture, output_view) = Self::create_output(device, out_w, out_h);

        let align = device.limits().min_uniform_buffer_offset_alignment as u64;
        let params_stride = (std::mem::size_of::<SimParamsGpu>() as u64).div_ceil(align) * align;
        let params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Sim Params Ring"),
            size: params_stride * RING_SLOTS as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        // Storage buffers, not uniforms: the arrays are runtime-sized
        // and a uniform would need a fixed maximum. They are tiny and
        // read once per invocation.
        let model_params_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Sim Model Params"),
            contents: bytemuck::cast_slice(&[0.0f32; 16]),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });
        let coloring_params_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Sim Coloring Params"),
            contents: bytemuck::cast_slice(&[0.0f32; 16]),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        Self {
            grid_w,
            grid_h,
            out_w,
            out_h,
            field,
            field_view,
            current: 0,
            output_texture,
            output_view,
            params_buffer,
            params_stride,
            model_params_buffer,
            coloring_params_buffer,
            pipelines: None,
            step_bind_groups: None,
            step_index: 0,
            steps_per_submit: FIRST_SUBMIT,
            needs_seed: true,
        }
    }

    /// Grid size for a config at a given output size, as REQUESTED.
    ///
    /// Deliberately not clamped to anything a device can hold: an
    /// earlier version clamped here, which silently shrank an
    /// over-large `Fixed` grid to 8192 and made
    /// [`Self::allocation_error`]'s dimension check unreachable — the
    /// user typed a size and got a different one with no message. The
    /// clamp belongs at the two ends instead: the config manager
    /// bounds what can be entered, and the constructor bounds what can
    /// be allocated if a caller skipped the check.
    pub fn grid_for(cfg: &SimConfig, out_w: u32, out_h: u32) -> (u32, u32) {
        cfg.grid.cells_for(out_w.max(1), out_h.max(1))
    }

    /// What will actually be allocated: the requested grid, bounded so
    /// a caller that never asked [`Self::allocation_error`] gets a
    /// smaller texture rather than an aborted process.
    fn allocatable_grid(cfg: &SimConfig, out_w: u32, out_h: u32) -> (u32, u32) {
        let (w, h) = Self::grid_for(cfg, out_w, out_h);
        (w.clamp(1, MAX_GRID_DIM), h.clamp(1, MAX_GRID_DIM))
    }

    /// Whether this config can be rendered on this device, and why not
    /// if it cannot. Mirrors `EscapeRenderer::allocation_error`: the
    /// caller refuses the job rather than letting wgpu abort the
    /// process on an allocation failure.
    pub fn allocation_error(
        device: &Device,
        cfg: &SimConfig,
        out_w: u32,
        out_h: u32,
    ) -> Option<String> {
        let limits = device.limits();
        let (gw, gh) = Self::grid_for(cfg, out_w, out_h);
        let max_dim = limits.max_texture_dimension_2d;
        if gw > max_dim || gh > max_dim {
            return Some(format!(
                "simulation grid {gw}x{gh} exceeds this device's maximum texture size ({max_dim}). \
                 Use a Fixed grid, or a viewport scale below 1."
            ));
        }
        if out_w > max_dim || out_h > max_dim {
            return Some(format!(
                "output {out_w}x{out_h} exceeds this device's maximum texture size ({max_dim})."
            ));
        }
        // Two field textures plus the output, 16 bytes per texel.
        let field_bytes = 2u64 * gw as u64 * gh as u64 * 16;
        let out_bytes = out_w as u64 * out_h as u64 * 16;
        let total = field_bytes + out_bytes;
        // Not a device limit but a sanity bound: past this the machine
        // is thrashing rather than rendering, and refusing with a
        // message beats an out-of-memory abort.
        const BUDGET: u64 = 4 << 30;
        if total > BUDGET {
            return Some(format!(
                "simulation needs {:.1} GiB of GPU memory ({gw}x{gh} grid, {out_w}x{out_h} output) \
                 which is over the {:.0} GiB budget. Reduce the grid or the output size.",
                total as f64 / (1 << 30) as f64,
                BUDGET as f64 / (1 << 30) as f64
            ));
        }
        None
    }

    fn create_field_pair(device: &Device, w: u32, h: u32) -> ([Texture; 2], [TextureView; 2]) {
        let desc = TextureDescriptor {
            label: Some("Sim Field"),
            size: Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            // Both usages on both textures: they swap roles every step.
            // COPY_SRC so a test can read the field back and compare
            // against a CPU mirror of the rule.
            usage: TextureUsages::STORAGE_BINDING
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC,
            view_formats: &[],
        };
        let a = device.create_texture(&desc);
        let b = device.create_texture(&desc);
        let va = a.create_view(&TextureViewDescriptor::default());
        let vb = b.create_view(&TextureViewDescriptor::default());
        ([a, b], [va, vb])
    }

    fn create_output(device: &Device, w: u32, h: u32) -> (Texture, TextureView) {
        let texture = device.create_texture(&TextureDescriptor {
            label: Some("Sim Output"),
            size: Extent3d {
                width: w.max(1),
                height: h.max(1),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            usage: TextureUsages::STORAGE_BINDING
                | TextureUsages::TEXTURE_BINDING
                | TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&TextureViewDescriptor::default());
        (texture, view)
    }

    /// The rendered image, in the flame-accumulator format the tonemap
    /// pass expects.
    pub fn output_view(&self) -> &TextureView {
        &self.output_view
    }

    pub fn output_texture(&self) -> &Texture {
        &self.output_texture
    }

    /// The texture currently holding live state. Test-only: reading the
    /// field back is how the CPU-mirror and determinism tests check the
    /// rule rather than the picture.
    #[cfg(test)]
    pub(crate) fn field_texture(&self) -> &Texture {
        &self.field[self.current]
    }

    pub fn step_index(&self) -> u32 {
        self.step_index
    }

    pub fn grid_size(&self) -> (u32, u32) {
        (self.grid_w, self.grid_h)
    }

    /// Mark the field stale so the next render reseeds it.
    pub fn request_seed(&mut self) {
        self.needs_seed = true;
    }

    /// Resize the OUTPUT, and the grid with it when the grid is bound
    /// to the viewport. Returns whether anything was recreated.
    ///
    /// A bound grid changing size discards the run — the field cannot
    /// be carried across a resolution change without interpolating
    /// state, which is a phase-3 concern (`resample_into`). Saying so
    /// by reseeding is honest; silently continuing on a stretched
    /// field would not be.
    pub fn resize(&mut self, device: &Device, cfg: &SimConfig, out_w: u32, out_h: u32) -> bool {
        let (gw, gh) = Self::allocatable_grid(cfg, out_w, out_h);
        let out_changed = out_w != self.out_w || out_h != self.out_h;
        let grid_changed = gw != self.grid_w || gh != self.grid_h;
        if !out_changed && !grid_changed {
            return false;
        }
        if out_changed {
            let (t, v) = Self::create_output(device, out_w, out_h);
            self.output_texture = t;
            self.output_view = v;
            self.out_w = out_w;
            self.out_h = out_h;
        }
        if grid_changed {
            let (f, fv) = Self::create_field_pair(device, gw, gh);
            self.field = f;
            self.field_view = fv;
            self.grid_w = gw;
            self.grid_h = gh;
            self.current = 0;
            self.needs_seed = true;
        }
        // Do NOT clear `pipelines` here. The resolve direction is part
        // of the pipeline key, so `ensure_pipelines` already rebuilds
        // exactly when a resize flips it -- clearing unconditionally
        // recompiled all three shaders on EVERY resize event, which is
        // once per frame during a window drag. The step bind groups do
        // reference the field views, so they go with the field.
        if grid_changed {
            self.step_bind_groups = None;
            self.steps_per_submit = FIRST_SUBMIT;
        }
        true
    }

    fn pipeline_key(&self, cfg: &SimConfig) -> PipelineKey {
        PipelineKey {
            model: model_or_default(&cfg.model).name,
            coloring: coloring_or_default(&cfg.coloring).name,
            boundary: cfg.boundary,
            upscale: cfg.upscale,
            downscale: cfg.downscale,
            init_kind: cfg.init.kind_name(),
            magnifying: self.magnifying(),
        }
    }

    /// Whether the output is larger than the grid, which decides which
    /// resolve filter the colour pass compiles.
    fn magnifying(&self) -> bool {
        self.out_w >= self.grid_w && self.out_h >= self.grid_h
    }

    fn ensure_pipelines(&mut self, device: &Device, cfg: &SimConfig) {
        let key = self.pipeline_key(cfg);
        if self.pipelines.as_ref().is_some_and(|p| p.key == key) {
            return;
        }
        let model = model_or_default(&cfg.model);
        let coloring = coloring_or_default(&cfg.coloring);

        let seed_src = assembler::assemble_seed(model, cfg.init.kind_name());
        let step_src = assembler::assemble_step(model, cfg.boundary);
        let color_src = assembler::assemble_color(
            coloring,
            cfg.boundary,
            cfg.upscale,
            cfg.downscale,
            key.magnifying,
        );

        let make = |label: &str, src: &str| {
            device.create_shader_module(ShaderModuleDescriptor {
                label: Some(label),
                source: ShaderSource::Wgsl(src.into()),
            })
        };
        let seed_mod = make("Sim Seed", &seed_src);
        let step_mod = make("Sim Step", &step_src);
        let color_mod = make("Sim Color", &color_src);

        // Bind group layouts, written out rather than derived from the
        // shader: `layout: None` would infer a fresh layout per module
        // and the bind groups could not be shared between passes.
        let uniform_entry = |binding: u32| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Uniform,
                // Dynamic: the step loop selects its ring slot by
                // offset rather than rewriting the buffer.
                has_dynamic_offset: true,
                min_binding_size: None,
            },
            count: None,
        };
        let storage_ro = |binding: u32| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Buffer {
                ty: BufferBindingType::Storage { read_only: true },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let storage_tex = |binding: u32| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::StorageTexture {
                access: StorageTextureAccess::WriteOnly,
                format: TextureFormat::Rgba32Float,
                view_dimension: TextureViewDimension::D2,
            },
            count: None,
        };
        let sampled_tex = |binding: u32, float32: bool| BindGroupLayoutEntry {
            binding,
            visibility: ShaderStages::COMPUTE,
            ty: BindingType::Texture {
                // Non-filterable: FLOAT32_FILTERABLE is an optional
                // feature, and every read here is a textureLoad anyway.
                sample_type: TextureSampleType::Float { filterable: !float32 },
                view_dimension: TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };

        let seed_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Sim Seed Layout"),
            entries: &[
                uniform_entry(0),
                storage_ro(1),
                storage_ro(2),
                storage_tex(3),
            ],
        });
        let step_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Sim Step Layout"),
            entries: &[
                uniform_entry(0),
                storage_ro(1),
                storage_ro(2),
                storage_tex(3),
                sampled_tex(4, true),
            ],
        });
        let color_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Sim Color Layout"),
            entries: &[
                uniform_entry(0),
                storage_ro(1),
                storage_ro(2),
                storage_tex(3),
                sampled_tex(4, true),
                sampled_tex(5, false),
            ],
        });

        let pipeline = |label: &str, layout: &BindGroupLayout, module: &ShaderModule| {
            let pl = device.create_pipeline_layout(&PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            });
            device.create_compute_pipeline(&ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&pl),
                module,
                entry_point: Some("main"),
                compilation_options: Default::default(),
                cache: None,
            })
        };

        // New layouts and modules: any cached bind group refers to the
        // old ones.
        self.step_bind_groups = None;
        self.steps_per_submit = FIRST_SUBMIT;
        self.pipelines = Some(Pipelines {
            seed: pipeline("Sim Seed", &seed_layout, &seed_mod),
            step: pipeline("Sim Step", &step_layout, &step_mod),
            color: pipeline("Sim Color", &color_layout, &color_mod),
            seed_layout,
            step_layout,
            color_layout,
            key,
        });
    }

    /// The uniform for one step index.
    fn params_for(&self, cfg: &SimConfig, step_index: u32) -> SimParamsGpu {
        let (p0, p1) = match cfg.init {
            crate::config::sim::SimInit::Noise { amplitude } => (amplitude, 0.0),
            crate::config::sim::SimInit::Blob { radius } => (radius as f32, 0.0),
            crate::config::sim::SimInit::Blobs { count, radius } => {
                (count.min(64) as f32, radius as f32)
            }
            crate::config::sim::SimInit::Ring { radius } => (radius as f32, 0.0),
            crate::config::sim::SimInit::Line
            | crate::config::sim::SimInit::Center
            | crate::config::sim::SimInit::BrokenWave => (0.0, 0.0),
        };
        SimParamsGpu {
            grid: [self.grid_w, self.grid_h],
            out_size: [self.out_w, self.out_h],
            step_index,
            seed_lo: cfg.seed as u32,
            seed_hi: (cfg.seed >> 32) as u32,
            // Capped at the model's stability bound here as well as at
            // the config manager: a hand-edited file can carry any value,
            // and past the bound the [0,1] clamp turns divergence into
            // plausible-looking garbage rather than NaN. The bound
            // depends on the diffusion rates in force, not just the
            // model, which is why it is computed from the params.
            dt: {
                let max_dt = model_or_default(&cfg.model).max_dt_for(&cfg.model_params);
                if cfg.dt.is_finite() { cfg.dt.clamp(1e-4, max_dt) } else { 1.0 }
            },
            init_p0: p0,
            init_p1: p1,
            _pad0: 0.0,
            _pad1: 0.0,
        }
    }

    /// Write ring slot 0, for the passes that run once (seed, colour).
    fn write_params_slot0(&self, queue: &Queue, cfg: &SimConfig) {
        let p = self.params_for(cfg, self.step_index);
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&p));
    }

    /// Fill `count` ring slots for steps `start..start + count`, in ONE
    /// write. Each step then reads its own slot by dynamic offset,
    /// which is the only way a batched submission can see per-step
    /// values at all -- `write_buffer` is staged before the command
    /// buffer runs, so rewriting one uniform per step gives every step
    /// in the batch the last step's index.
    fn write_params_ring(&self, queue: &Queue, cfg: &SimConfig, start: u32, count: u32) {
        let stride = self.params_stride as usize;
        let mut bytes = vec![0u8; stride * count as usize];
        for i in 0..count {
            let p = self.params_for(cfg, start + i);
            let at = i as usize * stride;
            bytes[at..at + std::mem::size_of::<SimParamsGpu>()]
                .copy_from_slice(bytemuck::bytes_of(&p));
        }
        queue.write_buffer(&self.params_buffer, 0, &bytes);
    }

    /// Build the two step bind groups once. They depend only on the
    /// pipeline layout and the field views, both of which are replaced
    /// together, so a rebuild is driven by `pipelines` being cleared.
    fn ensure_step_bind_groups(&mut self, device: &Device) {
        if self.step_bind_groups.is_some() {
            return;
        }
        let p = self.pipelines.as_ref().expect("pipelines built before bind groups");
        let make = |src: usize| {
            device.create_bind_group(&BindGroupDescriptor {
                label: Some("Sim Step BG"),
                layout: &p.step_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::Buffer(BufferBinding {
                            buffer: &self.params_buffer,
                            offset: 0,
                            size: std::num::NonZeroU64::new(
                                std::mem::size_of::<SimParamsGpu>() as u64,
                            ),
                        }),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: self.model_params_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: self.coloring_params_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: BindingResource::TextureView(&self.field_view[1 - src]),
                    },
                    BindGroupEntry {
                        binding: 4,
                        resource: BindingResource::TextureView(&self.field_view[src]),
                    },
                ],
            })
        };
        self.step_bind_groups = Some([make(0), make(1)]);
    }

    fn write_param_arrays(&self, queue: &Queue, model: &ModelDef, coloring: &SimColoringDef, cfg: &SimConfig) {
        // Padded to a fixed length so the buffer never needs resizing;
        // the shader only ever indexes as far as the definition
        // declares.
        let mut mp = model.pack_params(cfg);
        mp.resize(16, 0.0);
        let mut cp = coloring.pack_params(cfg);
        cp.resize(16, 0.0);
        queue.write_buffer(&self.model_params_buffer, 0, bytemuck::cast_slice(&mp));
        queue.write_buffer(&self.coloring_params_buffer, 0, bytemuck::cast_slice(&cp));
    }

    fn dispatch_size(w: u32, h: u32) -> (u32, u32) {
        (w.div_ceil(8), h.div_ceil(8))
    }

    /// Seed the field from the config. Resets the step counter: the
    /// pair (seed, step_index) is the state's identity, and a reseed
    /// starts a new run.
    pub fn seed(&mut self, device: &Device, queue: &Queue, cfg: &SimConfig) {
        self.ensure_pipelines(device, cfg);
        self.step_index = 0;
        self.write_params_slot0(queue, cfg);
        let model = model_or_default(&cfg.model);
        let coloring = coloring_or_default(&cfg.coloring);
        self.write_param_arrays(queue, model, coloring, cfg);

        let p = self.pipelines.as_ref().expect("pipelines built above");
        let bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Sim Seed BG"),
            layout: &p.seed_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &self.params_buffer,
                        offset: 0,
                        size: std::num::NonZeroU64::new(
                            std::mem::size_of::<SimParamsGpu>() as u64,
                        ),
                    }),
                },
                BindGroupEntry { binding: 1, resource: self.model_params_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: self.coloring_params_buffer.as_entire_binding() },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(&self.field_view[self.current]),
                },
            ],
        });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Sim Seed"),
        });
        {
            let mut pass = enc.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Sim Seed"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&p.seed);
            pass.set_bind_group(0, &bg, &[0]);
            let (gx, gy) = Self::dispatch_size(self.grid_w, self.grid_h);
            pass.dispatch_workgroups(gx, gy, 1);
        }
        queue.submit(std::iter::once(enc.finish()));
        self.needs_seed = false;
    }

    /// Advance exactly `count` steps, in watchdog-sized submissions.
    ///
    /// Batching does not change the sequence: each step reads the
    /// previous step's output, so a batch of 256 and 256 batches of one
    /// produce identical fields. There is a test for that, because it
    /// is the property that lets an export batch freely while a still
    /// stays reproducible.
    ///
    /// "Watchdog-sized" is measured, not assumed: each submission is
    /// waited on and timed, and the next is sized to
    /// [`SUBMIT_BUDGET_MS`] from that cost. The wait means the CPU
    /// stalls for the simulation's GPU time inside this call -- for an
    /// interactive frame that is a few milliseconds and for an export
    /// it was already the case. On wasm the wait returns at once (no
    /// blocking poll in WebGPU), the measured cost is ~0, and the
    /// batch sits at the ceiling; browsers have no 2 s reset to avoid.
    pub fn run_steps(&mut self, device: &Device, queue: &Queue, cfg: &SimConfig, count: u32) {
        if count == 0 {
            return;
        }
        self.ensure_pipelines(device, cfg);
        let model = model_or_default(&cfg.model);
        let coloring = coloring_or_default(&cfg.coloring);
        self.write_param_arrays(queue, model, coloring, cfg);

        self.ensure_step_bind_groups(device);
        let (gx, gy) = Self::dispatch_size(self.grid_w, self.grid_h);
        let stride = self.params_stride as u32;

        let mut done = 0;
        while done < count {
            let batch = self.steps_per_submit.clamp(1, MAX_STEPS_PER_SUBMIT).min(count - done);
            // One write for the whole batch, and one compute pass: the
            // dispatches inside it are ordered against each other, and
            // each reads its own ring slot by dynamic offset.
            self.write_params_ring(queue, cfg, self.step_index, batch);
            let mut enc = device.create_command_encoder(&CommandEncoderDescriptor {
                label: Some("Sim Steps"),
            });
            {
                let p = self.pipelines.as_ref().expect("pipelines built above");
                let groups = self
                    .step_bind_groups
                    .as_ref()
                    .expect("built by ensure_step_bind_groups");
                let mut pass = enc.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Sim Steps"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&p.step);
                for i in 0..batch {
                    // groups[src] reads field[src] and writes
                    // field[1 - src], so alternating the index IS the
                    // ping-pong.
                    pass.set_bind_group(0, &groups[self.current], &[i * stride]);
                    pass.dispatch_workgroups(gx, gy, 1);
                    self.current = 1 - self.current;
                    self.step_index += 1;
                }
            }
            queue.submit(std::iter::once(enc.finish()));
            done += batch;

            // Time this submission and size the next one from it. The
            // measurement can include unrelated work still in flight
            // (the seed pass, a previous frame's tonemap), which only
            // overestimates the cost and shrinks the next batch: the
            // safe direction.
            let started = web_time::Instant::now();
            let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
            let ms = started.elapsed().as_secs_f64() * 1e3;
            let per_step = ms / batch as f64;
            self.steps_per_submit = if per_step > 0.0 {
                (SUBMIT_BUDGET_MS / per_step).floor().clamp(1.0, MAX_STEPS_PER_SUBMIT as f64) as u32
            } else {
                MAX_STEPS_PER_SUBMIT
            };
        }
    }

    /// Colour the live field into the output image at the display size.
    ///
    /// Always run, even on a frame that took no steps: a parameter or
    /// palette edit has to be visible without advancing the
    /// simulation, and stepping to show an edit would make the picture
    /// depend on how long the user looked at it.
    pub fn color(
        &mut self,
        device: &Device,
        queue: &Queue,
        cfg: &SimConfig,
        palette_view: &TextureView,
    ) {
        self.ensure_pipelines(device, cfg);
        self.write_params_slot0(queue, cfg);
        let model = model_or_default(&cfg.model);
        let coloring = coloring_or_default(&cfg.coloring);
        self.write_param_arrays(queue, model, coloring, cfg);

        let p = self.pipelines.as_ref().expect("pipelines built above");
        let bg = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Sim Color BG"),
            layout: &p.color_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: &self.params_buffer,
                        offset: 0,
                        size: std::num::NonZeroU64::new(
                            std::mem::size_of::<SimParamsGpu>() as u64,
                        ),
                    }),
                },
                BindGroupEntry { binding: 1, resource: self.model_params_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: self.coloring_params_buffer.as_entire_binding() },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(&self.output_view),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::TextureView(&self.field_view[self.current]),
                },
                BindGroupEntry { binding: 5, resource: BindingResource::TextureView(palette_view) },
            ],
        });
        let mut enc = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Sim Color"),
        });
        {
            let mut pass = enc.begin_compute_pass(&ComputePassDescriptor {
                label: Some("Sim Color"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&p.color);
            pass.set_bind_group(0, &bg, &[0]);
            let (gx, gy) = Self::dispatch_size(self.out_w, self.out_h);
            pass.dispatch_workgroups(gx, gy, 1);
        }
        queue.submit(std::iter::once(enc.finish()));
    }

    /// One frame of interactive rendering: seed if needed, advance if
    /// running, and always colour.
    pub fn render_frame(
        &mut self,
        device: &Device,
        queue: &Queue,
        cfg: &SimConfig,
        palette_view: &TextureView,
        steps: u32,
    ) {
        if self.needs_seed {
            self.seed(device, queue, cfg);
        }
        if steps > 0 {
            self.run_steps(device, queue, cfg, steps);
        }
        self.color(device, queue, cfg, palette_view);
    }

    /// A complete still: seed, run exactly `cfg.steps`, colour. The
    /// export contract.
    pub fn render_still(
        &mut self,
        device: &Device,
        queue: &Queue,
        cfg: &SimConfig,
        palette_view: &TextureView,
    ) {
        self.seed(device, queue, cfg);
        self.run_steps(device, queue, cfg, cfg.steps);
        self.color(device, queue, cfg, palette_view);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bound_grid_follows_the_output_and_a_fixed_one_does_not() {
        let mut cfg = SimConfig::default();
        cfg.grid = SimGrid::Viewport { scale: 0.5 };
        assert_eq!(SimRenderer::grid_for(&cfg, 1920, 1080), (960, 540));
        cfg.grid = SimGrid::Fixed { width: 256, height: 256 };
        assert_eq!(SimRenderer::grid_for(&cfg, 1920, 1080), (256, 256));
    }

    /// `grid_for` must report what was ASKED for, so the refusal path
    /// can see it; only the allocation is bounded. Collapsing these two
    /// silently resized the user's grid and made the refusal
    /// unreachable.
    #[test]
    fn an_absurd_grid_is_reported_honestly_and_allocated_safely() {
        let mut cfg = SimConfig::default();
        cfg.grid = SimGrid::Fixed { width: 100_000, height: 100_000 };
        assert_eq!(
            SimRenderer::grid_for(&cfg, 1920, 1080),
            (100_000, 100_000),
            "the requested size must survive so allocation_error can refuse it"
        );
        assert_eq!(
            SimRenderer::allocatable_grid(&cfg, 1920, 1080),
            (MAX_GRID_DIM, MAX_GRID_DIM),
            "but nothing that large is ever handed to wgpu"
        );
    }

    #[test]
    fn dispatch_covers_every_cell_including_a_partial_workgroup() {
        assert_eq!(SimRenderer::dispatch_size(256, 256), (32, 32));
        // 257 needs a 33rd group whose upper rows the shader's bounds
        // check discards.
        assert_eq!(SimRenderer::dispatch_size(257, 1), (33, 1));
        assert_eq!(SimRenderer::dispatch_size(1, 1), (1, 1));
    }
}
