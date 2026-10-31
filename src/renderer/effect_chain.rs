//! Effect Chain Runner
//!
//! Executes shader effects in sequence using ping-pong textures.
//! - Density effects: Run on Rgba32Float before tonemap (`DENSITY_FORMAT`)
//! - Color effects: Run on Rgba8Unorm after tonemap

use std::collections::HashMap;
use egui_wgpu::wgpu;

// ============================================================================
// Shader sources
//
// Every shipped shader is embedded (see `effects::embedded_shaders`).
// Desktop additionally prefers an on-disk copy when there is one, so a
// shader can be edited without a rebuild — but its ABSENCE is no longer
// an error, which it used to be on both counts below.
// ============================================================================

/// The shared blend-mode library that 12 of the 15 shipped effects
/// splice in.
///
/// Same arrangement as an effect's own shader: embedded, with the
/// on-disk copy preferred on desktop when it is there. It used to log an
/// error and return an EMPTY STRING when the file was missing, which
/// meant every effect that spliced it compiled against nothing and
/// failed with a WGSL error naming a function the reader never wrote.
fn load_blend_modes() -> String {
    #[cfg(not(target_arch = "wasm32"))]
    if let Ok(from_disk) = std::fs::read_to_string(crate::resources::resource_path("shaders/effects/common/blend_modes.wgsl")) {
        return from_disk;
    }
    crate::effects::embedded_shaders::BLEND_MODES.to_string()
}

/// Splice include markers, one line at a time.
///
/// The marker is a **directive on its own line**, not a magic substring.
/// The difference is not pedantry: a plain `source.replace` also fires
/// on the marker QUOTED IN PROSE, so a shader whose header comment
/// mentions `// INCLUDE_BLEND_MODES` gets the whole library spliced into
/// the middle of a sentence. It then fails to compile pointing at a line
/// its author never wrote — the same confusing failure the empty-string
/// fallback used to produce.
///
/// No built-in does that, which is why it went unnoticed; the first
/// plugin written to demonstrate the marker hit it immediately.
///
/// A second standalone marker is dropped rather than spliced: two copies
/// of the library is a redefinition error, and a duplicated directive
/// clearly means "include it", not "include it twice".
fn process_shader_includes(source: String) -> String {
    const BLEND_MODES_MARKER: &str = "// INCLUDE_BLEND_MODES";

    if !source.lines().any(|l| l.trim() == BLEND_MODES_MARKER) {
        return source;
    }

    let blend_modes = load_blend_modes();
    let mut spliced = false;
    let mut out = Vec::new();
    for line in source.lines() {
        if line.trim() == BLEND_MODES_MARKER {
            if spliced {
                log::warn!("Ignoring a repeated `{BLEND_MODES_MARKER}` — already included");
                continue;
            }
            spliced = true;
            out.push(blend_modes.clone());
        } else {
            out.push(line.to_string());
        }
    }
    out.join("\n")
}

/// An effect's WGSL, includes spliced.
///
/// The splice happens **here** rather than at rest, which is why the
/// exported corpus keeps the `// INCLUDE_BLEND_MODES` marker intact: a
/// pre-spliced copy would bake the shared library into all twelve
/// effects that use it, and a downloaded effect would arrive carrying a
/// second copy of code the client already has.
fn effect_wgsl(source: &crate::effects::EffectSource) -> String {
    process_shader_includes(source.wgsl())
}
use wgpu::{
    BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Buffer, BufferBindingType,
    BufferDescriptor, BufferUsages, Color, ColorTargetState, ColorWrites, CommandEncoder,
    CommandEncoderDescriptor, Device, Extent3d, FilterMode, FragmentState, LoadOp,
    MultisampleState, MapMode, Operations, Origin3d, PipelineLayoutDescriptor, PollType,
    PrimitiveState, Queue, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
    RenderPipelineDescriptor, Sampler, SamplerBindingType, SamplerDescriptor,
    ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp, Texture, TextureAspect,
    TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureView, TextureViewDescriptor, TextureViewDimension, TexelCopyBufferInfo,
    TexelCopyBufferLayout, TexelCopyTextureInfo, VertexState, COPY_BYTES_PER_ROW_ALIGNMENT,
};

use crate::effects::{global_effect_registry, EffectCategory, EffectInstance};

/// Maximum number of effect parameters per effect.
///
/// This is a HARD limit, not a soft one: the parameters live in a
/// fixed-size uniform, so an effect declaring more has nowhere to put
/// them — the extras are silently dropped. The API's publish-time check
/// must therefore match this number exactly, never exceed it.
///
/// **Why 48 and not more.** Each effect gets one slot in a shared
/// uniform buffer, and the slot stride is set by
/// `UNIFORM_BUFFER_OFFSET_ALIGNMENT` (256 B), not by the struct — so
/// everything up to 240 B of parameters is free, and the buffer starts
/// at 32 × 256 B = 8 KiB (`MAX_EFFECT_SLOTS`). 48 floats (192 B, + 16 B of width/height/pad =
/// 208 B) sits inside that with room spare.
///
/// Going much beyond ~60 stops being free and stops being possible:
/// 512 parameters would need ~72 KiB, over the 64 KiB
/// `max_uniform_buffer_binding_size` of a default device and 4.5× the
/// 16 KiB the WASM build requests. That would mean moving effect
/// parameters to a storage buffer — a real change, not a constant bump.
pub const MAX_EFFECT_PARAMS: usize = 48;

/// The shared params buffer's INITIAL slot count (one slot per effect
/// pass in a frame).
///
/// Not a ceiling: `reserve_slots` grows the buffer to the next power of
/// two when a frame's chains need more. They can: `distance_field` alone
/// takes its jump-flood pass count + 2 slots (13 at 1080p), so three of
/// them and one more effect is 40. The runner used to clamp the 33rd
/// allocation to slot 32 -- an offset equal to the buffer's size -- and
/// the write and bind group there were validation errors that rejected
/// the whole frame (a frozen viewport; a panic on an export device).
/// Published in the engine contract as `effect_slots`; per-effect
/// parameters (`MAX_EFFECT_PARAMS`) are the real fixed limit.
pub const MAX_EFFECT_SLOTS: usize = 32;

/// Alignment for uniform buffer dynamic offsets (256 bytes is typical minimum)
const UNIFORM_BUFFER_OFFSET_ALIGNMENT: u64 = 256;

/// The density chain's ping-pong format: the accumulator's own.
///
/// The chain carries the accumulator's layout, whose alpha is a RAW
/// cumulative hit count (`shaders/accumulate.wgsl`), and the density
/// effects pass it through or average it. It was `Rgba16Float`, whose
/// largest finite value is 65504: a pixel past 65504 hits -- which a
/// filament flame reaches well inside the default budget -- became Inf
/// (black under the Log tone map) or saturated. f32 holds any count
/// the accumulator does.
///
/// `Rgba32Float` is renderable but not blendable (that needs
/// `FLOAT32_BLENDABLE`), so the density pipelines write with
/// `blend: None`; `BlendState::REPLACE` would fail pipeline creation.
/// Sampling it with the chain's filtering sampler needs
/// `FLOAT32_FILTERABLE`, which `run_density_effects` already requires to
/// read the accumulation itself.
const DENSITY_FORMAT: TextureFormat = TextureFormat::Rgba32Float;

/// GPU uniform buffer for effect parameters
///
/// `[[f32; 4]; 12]` to match WGSL `array<vec4<f32>, 12>` — vec4 groups
/// because a uniform array's stride must be 16-byte aligned. The 12
/// must stay in step with `MAX_EFFECT_PARAMS` (48 = 12 × 4) and with
/// the struct declared in every `shaders/effects/**/*.wgsl`; a mismatch
/// is a binding-size validation error at pipeline creation.
///
/// Total size: 208 bytes (params=192 + width=4 + height=4 +
/// density_mean=4 + padding=4 for 16-byte alignment), inside the
/// 256-byte slot stride. `density_mean` sits where an 8-byte padding
/// was, so a shader that still declares `_padding: vec2<f32>` binds the
/// same layout and ignores it.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct EffectParams {
    /// Effect parameters (`MAX_EFFECT_PARAMS` floats, packed as vec4s
    /// for alignment)
    params: [[f32; 4]; MAX_EFFECT_PARAMS / 4],
    /// Texture dimensions
    width: u32,
    height: u32,
    /// For a density effect, the frame's mean density, the unit Levels
    /// measures in (`FlameRenderer::mean_density`): divide a pixel's `a`
    /// by it to get a density that does not grow with the iteration
    /// count. 1 for escape and simulation images, whose `a` is not a
    /// hit count; 0 for colour effects.
    density_mean: f32,
    /// Render pixels per logical viewport point (`set_pixel_scale`, the
    /// viewport's render scale; 1 everywhere else). The effects with sizes
    /// in pixels read it (the density blurs, sharpen, chromatic aberration,
    /// the kaleidoscope's edge offset); the rest declare it as padding.
    pixel_scale: f32,
}

// The uniform packs parameters in vec4 groups, so the count must divide
// by 4, and the whole struct must fit the slot stride. Both are checked
// here rather than discovered as a wgpu validation error at pipeline
// creation — or, worse, as parameters that silently stop arriving.
const _: () = {
    assert!(
        MAX_EFFECT_PARAMS % 4 == 0,
        "MAX_EFFECT_PARAMS must be a multiple of 4 (uniform arrays pack as vec4)"
    );
    assert!(
        std::mem::size_of::<EffectParams>() as u64 <= UNIFORM_BUFFER_OFFSET_ALIGNMENT,
        "EffectParams no longer fits one slot stride — raising MAX_EFFECT_PARAMS \
         past ~60 needs a storage buffer, not a bigger uniform"
    );
};

/// A compiled effect pipeline ready for execution
struct CompiledEffect {
    /// Name of the effect (kept for debugging)
    #[allow(dead_code)]
    name: String,
    /// Render pipeline for this effect
    pipeline: RenderPipeline,
    /// Bind group layout (shared with pipeline)
    bind_group_layout: BindGroupLayout,
}

/// Ping-pong texture pair for effect chain execution
struct PingPongTextures {
    texture_a: Texture,
    texture_b: Texture,
    view_a: TextureView,
    view_b: TextureView,
    /// Which texture is currently the "read" source (0 = A, 1 = B)
    read_index: usize,
}

impl PingPongTextures {
    fn new(device: &Device, width: u32, height: u32, format: TextureFormat, label: &str) -> Self {
        let create_texture = |suffix: &str| {
            device.create_texture(&TextureDescriptor {
                label: Some(&format!("{} {}", label, suffix)),
                size: Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };

        let texture_a = create_texture("A");
        let texture_b = create_texture("B");
        let view_a = texture_a.create_view(&TextureViewDescriptor::default());
        let view_b = texture_b.create_view(&TextureViewDescriptor::default());

        Self {
            texture_a,
            texture_b,
            view_a,
            view_b,
            read_index: 0,
        }
    }

    /// Get the current read texture view
    fn read_view(&self) -> &TextureView {
        if self.read_index == 0 {
            &self.view_a
        } else {
            &self.view_b
        }
    }

    /// Get the current write texture view
    fn write_view(&self) -> &TextureView {
        if self.read_index == 0 {
            &self.view_b
        } else {
            &self.view_a
        }
    }

    /// Swap read and write textures
    fn swap(&mut self) {
        self.read_index = 1 - self.read_index;
    }

    /// Free both GPU textures synchronously (see `EffectChainRunner::destroy`).
    fn destroy(&self) {
        self.texture_a.destroy();
        self.texture_b.destroy();
    }

    /// Get the current read texture (for copying)
    fn read_texture(&self) -> &Texture {
        if self.read_index == 0 {
            &self.texture_a
        } else {
            &self.texture_b
        }
    }
}

/// Effect chain runner for executing post-processing effects
pub struct EffectChainRunner {
    /// Compiled effect pipelines by name
    compiled_effects: HashMap<String, CompiledEffect>,
    /// Ping-pong textures for density effects (`DENSITY_FORMAT`)
    density_textures: Option<PingPongTextures>,
    /// Ping-pong textures for color effects (Rgba8Unorm)
    color_textures: Option<PingPongTextures>,
    /// Shared params buffer with indexed slots for all effects
    /// Each effect writes to slot_index * UNIFORM_BUFFER_OFFSET_ALIGNMENT
    /// This allows multiple effects to have different params in a single submit
    params_buffer: Buffer,
    /// Slots `params_buffer` holds: `MAX_EFFECT_SLOTS` until a frame
    /// needs more (`reserve_slots`).
    slot_capacity: usize,
    /// Current slot index for params buffer (reset each frame)
    current_slot: usize,
    /// Linear sampler for texture sampling
    sampler: Sampler,
    /// Current texture dimensions
    width: u32,
    height: u32,
    /// Jump-flood resources for the `distance_field` effect (the
    /// escape-time plan's §7.3 bridge): an rg32float coordinate
    /// ping-pong plus the seed/flood/composite pipelines. Lazily
    /// created, absent unless the effect is in the chain.
    jfa: Option<JfaResources>,
    /// Render pixels per logical viewport point (`set_pixel_scale`): the
    /// viewport's render scale. 1 for the exporters.
    pixel_scale: f32,
}

/// Multi-pass state for the `distance_field` effect. The effect
/// registers like any other color effect (config, UI, animation all
/// free) but the runner special-cases its execution: one seed pass,
/// ~log2(max dim) jump-flood passes over the coordinate ping-pong,
/// one composite pass into the chain's output.
struct JfaResources {
    coord_a: Texture,
    coord_b: Texture,
    coord_view_a: TextureView,
    coord_view_b: TextureView,
    seed_pipeline: RenderPipeline,
    flood_pipeline: RenderPipeline,
    composite_pipeline: RenderPipeline,
    bind_group_layout: BindGroupLayout,
}

impl JfaResources {
    fn destroy(&self) {
        self.coord_a.destroy();
        self.coord_b.destroy();
    }
}

impl EffectChainRunner {
    /// Create a new effect chain runner
    pub fn new(device: &Device, width: u32, height: u32) -> Self {
        // Single shared params buffer with slots for all effects
        // Each slot is aligned to UNIFORM_BUFFER_OFFSET_ALIGNMENT (256 bytes)
        // This allows multiple effects to have independent params in a single submit
        let params_buffer = Self::create_params_buffer(device, MAX_EFFECT_SLOTS);

        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Effect Sampler"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..Default::default()
        });

        Self {
            compiled_effects: HashMap::new(),
            density_textures: None,
            color_textures: None,
            params_buffer,
            slot_capacity: MAX_EFFECT_SLOTS,
            current_slot: 0,
            sampler,
            width,
            height,
            jfa: None,
            pixel_scale: 1.0,
        }
    }

    /// Reset slot counter for new frame
    /// Call this at the start of each frame before running effects
    pub fn reset_slots(&mut self) {
        self.current_slot = 0;
    }

    /// Explicitly release the ping-pong textures + params buffer. On WebGPU,
    /// dropping defers reclamation to JS GC; a throwaway per-export chain at
    /// 8000² holds hundreds of MB (full-res ping-pong pair), so destroy it
    /// alongside the export renderer so repeated WASM exports don't OOM the
    /// device. Call after the final pixel read completes.
    pub fn destroy(&self) {
        if let Some(t) = &self.density_textures { t.destroy(); }
        if let Some(t) = &self.color_textures { t.destroy(); }
        if let Some(j) = &self.jfa { j.destroy(); }
        self.params_buffer.destroy();
    }

    fn create_params_buffer(device: &Device, slots: usize) -> Buffer {
        device.create_buffer(&BufferDescriptor {
            label: Some("Effect Params Buffer (Indexed)"),
            size: slots as u64 * UNIFORM_BUFFER_OFFSET_ALIGNMENT,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    /// Make room for `needed` more slots this frame, before any of them
    /// is allocated.
    ///
    /// Grows the params buffer to the next power of two past what the
    /// frame needs. The old buffer is dropped, not destroyed: bind groups
    /// made earlier in this frame (the density chain's, say) and the
    /// writes queued for them still reference it, and wgpu keeps it alive
    /// until they are done. The new buffer's first `current_slot` slots go
    /// unused this frame; `reset_slots` starts the next one at 0, and the
    /// capacity stays, so a chain that needed it once grows it once.
    fn reserve_slots(&mut self, device: &Device, needed: usize) {
        let want = self.current_slot + needed;
        if want <= self.slot_capacity {
            return;
        }
        let capacity = want.next_power_of_two().max(MAX_EFFECT_SLOTS);
        log::debug!("Effect params buffer: {} -> {} slots", self.slot_capacity, capacity);
        self.params_buffer = Self::create_params_buffer(device, capacity);
        self.slot_capacity = capacity;
    }

    /// Allocate a slot and return its buffer offset. `reserve_slots`
    /// must have made room first.
    fn allocate_slot(&mut self) -> u64 {
        // A slot past the buffer is an offset at or beyond its end: the
        // write and the bind group there are validation errors that reject
        // the whole frame. Reserving is the fix; this only keeps a missed
        // reservation from becoming that (it reuses the last slot, so one
        // effect may read another's parameters).
        debug_assert!(self.current_slot < self.slot_capacity, "effect slot allocated without reserve_slots");
        let slot = self.current_slot.min(self.slot_capacity - 1);
        self.current_slot += 1;
        slot as u64 * UNIFORM_BUFFER_OFFSET_ALIGNMENT
    }

    /// Resize textures if dimensions changed
    /// The viewport's render scale (docs/archive/projects/quality-governor.md):
    /// render pixels per logical point, which the effects' sizes in
    /// pixels are measured in (`EffectParams::pixel_scale`).
    pub fn set_pixel_scale(&mut self, s: f32) {
        self.pixel_scale = if s.is_finite() && s > 0.0 { s } else { 1.0 };
    }

    pub fn resize(&mut self, _device: &Device, width: u32, height: u32) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            // Textures will be recreated on next use
            self.density_textures = None;
            self.color_textures = None;
            if let Some(j) = self.jfa.take() {
                j.destroy();
            }
        }
    }

    /// Ensure ping-pong textures exist for the given category
    fn ensure_textures(&mut self, device: &Device, category: EffectCategory) {
        match category {
            EffectCategory::Density => {
                if self.density_textures.is_none() {
                    self.density_textures = Some(PingPongTextures::new(
                        device,
                        self.width,
                        self.height,
                        DENSITY_FORMAT,
                        "Density Effect",
                    ));
                }
            }
            EffectCategory::Color => {
                if self.color_textures.is_none() {
                    self.color_textures = Some(PingPongTextures::new(
                        device,
                        self.width,
                        self.height,
                        TextureFormat::Rgba8Unorm,
                        "Color Effect",
                    ));
                }
            }
        }
    }

    /// Number of jump-flood passes for the current dimensions:
    /// steps n/2, n/4, ..., 1 with n the covering power of two.
    fn jfa_pass_count(width: u32, height: u32) -> u32 {
        let max_dim = width.max(height).max(2);
        32 - (max_dim - 1).leading_zeros()
    }

    /// Create the distance-field resources if absent: the coordinate
    /// ping-pong and the three pipelines, compiled from the effect's
    /// registered WGSL (so the on-disk-override arrangement applies).
    fn ensure_jfa(&mut self, device: &Device) {
        if self.jfa.is_some() {
            return;
        }
        let registry = global_effect_registry();
        let Some(info) = registry.get("distance_field") else {
            return;
        };
        let shader_source = effect_wgsl(&info.source);
        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("distance_field Shader"),
            source: ShaderSource::Wgsl(shader_source.into()),
        });
        // One layout for all three passes: image (filterable),
        // sampler, params slot, coordinate field (rg32float is
        // UNFILTERABLE-float; every read is a textureLoad).
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("distance_field Bind Group Layout"),
            entries: &[
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("distance_field Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let make_pipeline = |entry: &str, format: TextureFormat| {
            device.create_render_pipeline(&RenderPipelineDescriptor {
                label: Some(&format!("distance_field {entry} Pipeline")),
                layout: Some(&pipeline_layout),
                vertex: VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(FragmentState {
                    module: &module,
                    entry_point: Some(entry),
                    targets: &[Some(ColorTargetState {
                        format,
                        // rg32float is not blendable; no pass blends.
                        blend: None,
                        write_mask: ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: PrimitiveState::default(),
                depth_stencil: None,
                multisample: MultisampleState::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let seed_pipeline = make_pipeline("fs_seed", TextureFormat::Rg32Float);
        let flood_pipeline = make_pipeline("fs_flood", TextureFormat::Rg32Float);
        let composite_pipeline = make_pipeline("fs_composite", TextureFormat::Rgba8Unorm);
        let make_texture = |suffix: &str| {
            device.create_texture(&TextureDescriptor {
                label: Some(&format!("JFA Coord {suffix}")),
                size: Extent3d {
                    width: self.width,
                    height: self.height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format: TextureFormat::Rg32Float,
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
        };
        let coord_a = make_texture("A");
        let coord_b = make_texture("B");
        let coord_view_a = coord_a.create_view(&TextureViewDescriptor::default());
        let coord_view_b = coord_b.create_view(&TextureViewDescriptor::default());
        self.jfa = Some(JfaResources {
            coord_a,
            coord_b,
            coord_view_a,
            coord_view_b,
            seed_pipeline,
            flood_pipeline,
            composite_pipeline,
            bind_group_layout,
        });
    }

    /// Execute the distance-field effect: seed, jump-flood, composite.
    /// `slots` holds one params-buffer offset per pass (seed, floods,
    /// composite) — each pass needs its own uniform contents in the
    /// same submit.
    #[allow(clippy::too_many_arguments)]
    fn run_jfa(
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        effect: &EffectInstance,
        jfa: &JfaResources,
        input_view: &TextureView,
        output_view: &TextureView,
        params_buffer: &Buffer,
        slots: &[u64],
        sampler: &Sampler,
        width: u32,
        height: u32,
        pixel_scale: f32,
    ) {
        let n_floods = Self::jfa_pass_count(width, height) as usize;
        if slots.len() < n_floods + 2 {
            log::warn!("distance_field: not enough param slots ({} < {})", slots.len(), n_floods + 2);
            return;
        }
        let mut effect_slot = EffectParams {
            params: [[0.0; 4]; MAX_EFFECT_PARAMS / 4],
            width,
            height,
            density_mean: 0.0,
            pixel_scale,
        };
        let registry = global_effect_registry();
        if let Some(info) = registry.get("distance_field") {
            for (i, param_def) in info.parameters.iter().enumerate() {
                if i < MAX_EFFECT_PARAMS {
                    effect_slot.params[i / 4][i % 4] = effect.get_param(&param_def.name);
                }
            }
        }
        // Seed + composite share the effect's own parameters; each
        // flood pass gets its step size in params[0].
        queue.write_buffer(params_buffer, slots[0], bytemuck::bytes_of(&effect_slot));
        queue.write_buffer(
            params_buffer,
            slots[n_floods + 1],
            bytemuck::bytes_of(&effect_slot),
        );
        let mut flood = effect_slot;
        for i in 0..n_floods {
            let step = 1u32 << (n_floods - 1 - i);
            flood.params[0][0] = step as f32;
            queue.write_buffer(params_buffer, slots[1 + i], bytemuck::bytes_of(&flood));
        }

        use wgpu::BufferBinding;
        let params_size =
            std::num::NonZeroU64::new(std::mem::size_of::<EffectParams>() as u64).unwrap();
        let bind = |slot: u64, coord_view: &TextureView| {
            device.create_bind_group(&BindGroupDescriptor {
                label: Some("distance_field Bind Group"),
                layout: &jfa.bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(input_view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(sampler),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::Buffer(BufferBinding {
                            buffer: params_buffer,
                            offset: slot,
                            size: Some(params_size),
                        }),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: BindingResource::TextureView(coord_view),
                    },
                ],
            })
        };
        let run_pass = |encoder: &mut CommandEncoder,
                        pipeline: &RenderPipeline,
                        bind_group: &wgpu::BindGroup,
                        target: &TextureView,
                        label: &str| {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some(label),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: target,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, bind_group, &[]);
            pass.draw(0..3, 0..1);
        };

        // Seed into A (binding B at slot 3 — never the render target).
        let bg = bind(slots[0], &jfa.coord_view_b);
        run_pass(encoder, &jfa.seed_pipeline, &bg, &jfa.coord_view_a, "JFA Seed");
        // Flood ping-pong: read A write B, then swap.
        let mut read_a = true;
        for i in 0..n_floods {
            let (read, write) = if read_a {
                (&jfa.coord_view_a, &jfa.coord_view_b)
            } else {
                (&jfa.coord_view_b, &jfa.coord_view_a)
            };
            let bg = bind(slots[1 + i], read);
            run_pass(encoder, &jfa.flood_pipeline, &bg, write, "JFA Flood");
            read_a = !read_a;
        }
        let final_coord = if read_a { &jfa.coord_view_a } else { &jfa.coord_view_b };
        let bg = bind(slots[n_floods + 1], final_coord);
        run_pass(encoder, &jfa.composite_pipeline, &bg, output_view, "JFA Composite");
    }

    /// Compile an effect shader if not already compiled
    fn ensure_effect_compiled(&mut self, device: &Device, effect_name: &str, category: EffectCategory) {
        if self.compiled_effects.contains_key(effect_name) {
            log::debug!("Effect {} already compiled", effect_name);
            return;
        }
        log::info!("Compiling effect shader: {}", effect_name);

        let registry = global_effect_registry();
        let effect_info = match registry.get(effect_name) {
            Some(info) => info,
            None => {
                // Record it rather than only logging. A flame can name
                // an effect this build has never heard of — the same
                // situation `missing_variations_in` handles — and the
                // app fetches it on demand. Logging alone meant the
                // effect silently did nothing and the console was the
                // only place that said why.
                crate::effects::note_missing_effect(effect_name);
                log::warn!("Unknown effect: {}", effect_name);
                return;
            }
        };

        // Load shader source (embedded for WASM, filesystem for desktop)
        let shader_source = effect_wgsl(&effect_info.source);

        // Create shader module
        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(&format!("{} Shader", effect_name)),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        // Determine texture format based on category
        let texture_format = match category {
            EffectCategory::Density => DENSITY_FORMAT,
            EffectCategory::Color => TextureFormat::Rgba8Unorm,
        };
        // Every pass overwrites its target, so no blending is wanted in
        // either chain; the density format cannot blend at all (see
        // `DENSITY_FORMAT`).
        let blend = match category {
            EffectCategory::Density => None,
            EffectCategory::Color => Some(BlendState::REPLACE),
        };

        // Create bind group layout for effect
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some(&format!("{} Bind Group Layout", effect_name)),
            entries: &[
                // Binding 0: Input texture
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Binding 1: Sampler
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                // Binding 2: Effect parameters
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        // Create pipeline layout
        let pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some(&format!("{} Pipeline Layout", effect_name)),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });

        // Create render pipeline
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some(&format!("{} Pipeline", effect_name)),
            layout: Some(&pipeline_layout),
            vertex: VertexState {
                module: &shader_module,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(FragmentState {
                module: &shader_module,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: texture_format,
                    blend,
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview_mask: None,
            cache: None,
        });

        self.compiled_effects.insert(
            effect_name.to_string(),
            CompiledEffect {
                name: effect_name.to_string(),
                pipeline,
                bind_group_layout,
            },
        );

        log::info!("Compiled effect: {}", effect_name);
    }

    /// Check if any effects in the list are enabled
    pub fn has_enabled_effects(effects: &[EffectInstance]) -> bool {
        effects.iter().any(|e| e.enabled)
    }

    /// Ensure all effects in the list are compiled
    pub fn compile_effects(&mut self, device: &Device, effects: &[EffectInstance], category: EffectCategory) {
        for effect in effects.iter().filter(|e| e.enabled) {
            // distance_field has no fs_main — the runner builds its
            // multi-pass pipelines in ensure_jfa instead.
            if effect.effect_type == "distance_field" {
                continue;
            }
            self.ensure_effect_compiled(device, &effect.effect_type, category);
        }
    }

    /// Run density effects chain
    ///
    /// Takes the accumulation texture as input and returns whether effects were run.
    /// The result is in the density ping-pong textures (call get_density_output() to get the view).
    pub fn run_density_effects(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        input_view: &TextureView,
        effects: &[EffectInstance],
        density_mean: f32,
    ) -> bool {
        let enabled_effects: Vec<_> = effects.iter().filter(|e| e.enabled).collect();
        if enabled_effects.is_empty() {
            return false;
        }

        // Density effects bilinear-sample the Rgba32Float accumulation, which
        // requires FLOAT32_FILTERABLE. The app/export devices request it when
        // the adapter advertises it (see gpu/device.rs); on an adapter that
        // lacks it, the bind group would be invalid — skip rather than crash.
        if !device.features().contains(wgpu::Features::FLOAT32_FILTERABLE) {
            log::warn!("Skipping density effects: adapter lacks FLOAT32_FILTERABLE (can't filter the Rgba32Float accumulation).");
            return false;
        }

        log::debug!("Running {} density effects (chain: {}x{})", enabled_effects.len(), self.width, self.height);

        // First, ensure all effects are compiled (before taking texture borrow)
        self.compile_effects(device, effects, EffectCategory::Density);
        self.ensure_textures(device, EffectCategory::Density);

        // Pre-allocate slots for all effects (before borrowing textures)
        self.reserve_slots(device, enabled_effects.len());
        let slot_offsets: Vec<u64> = enabled_effects.iter().map(|_| self.allocate_slot()).collect();

        // Extract data needed for effect execution
        let width = self.width;
        let height = self.height;

        // Now run effects
        if let Some(textures) = self.density_textures.as_mut() {
            // Reset read index so first write goes to texture A
            textures.read_index = 1; // So write_view() returns A

            for (i, effect) in enabled_effects.iter().enumerate() {
                // First effect reads from input texture (accumulation), subsequent effects read from ping-pong
                let read_view = if i == 0 {
                    input_view
                } else {
                    textures.read_view()
                };

                Self::run_single_effect_with_input(
                    device,
                    queue,
                    encoder,
                    &effect.effect_type,
                    effect,
                    read_view,
                    textures.write_view(),
                    &self.compiled_effects,
                    &self.params_buffer,
                    slot_offsets[i],
                    &self.sampler,
                    width,
                    height,
                    density_mean,
                    self.pixel_scale,
                );

                // Swap ping-pong textures for next effect
                textures.swap();
            }
            return true;
        }
        false
    }

    /// Get the output texture view from density effects (if any were run)
    pub fn get_density_output(&self) -> Option<&TextureView> {
        self.density_textures.as_ref().map(|t| t.read_view())
    }

    /// The density chain's output texels, row-major (test readback).
    #[cfg(test)]
    fn read_density_output_blocking(&self, device: &Device, queue: &Queue) -> Vec<[f32; 4]> {
        let textures = self.density_textures.as_ref().expect("density effects ran");
        let row = self.width * 16;
        let padded = row.div_ceil(COPY_BYTES_PER_ROW_ALIGNMENT) * COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Density Output Readback"),
            size: (padded * self.height) as u64,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("Density Output Readback") });
        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo { texture: textures.read_texture(), mip_level: 0, origin: Origin3d::ZERO, aspect: TextureAspect::All },
            TexelCopyBufferInfo {
                buffer: &buffer,
                layout: TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(padded), rows_per_image: Some(self.height) },
            },
            Extent3d { width: self.width, height: self.height, depth_or_array_layers: 1 },
        );
        queue.submit(Some(encoder.finish()));
        buffer.slice(..).map_async(MapMode::Read, |_| {});
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let data = buffer.slice(..).get_mapped_range();
        let mut out = Vec::with_capacity((self.width * self.height) as usize);
        for y in 0..self.height {
            let at = (y * padded) as usize;
            for px in data[at..at + row as usize].chunks_exact(16) {
                let f = |k: usize| f32::from_le_bytes([px[k], px[k + 1], px[k + 2], px[k + 3]]);
                out.push([f(0), f(4), f(8), f(12)]);
            }
        }
        out
    }

    /// Run color effects chain
    ///
    /// Takes the tonemap output texture as input. First effect reads from input_view,
    /// subsequent effects ping-pong between internal textures. Returns true if effects were run.
    pub fn run_color_effects(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        input_view: &TextureView,
        effects: &[EffectInstance],
    ) -> bool {
        let enabled_effects: Vec<_> = effects.iter().filter(|e| e.enabled).collect();
        if enabled_effects.is_empty() {
            return false;
        }
        log::debug!("Running {} color effects (chain: {}x{})", enabled_effects.len(), self.width, self.height);

        // First, ensure all effects are compiled (before taking texture borrow)
        self.compile_effects(device, effects, EffectCategory::Color);
        self.ensure_textures(device, EffectCategory::Color);
        if enabled_effects.iter().any(|e| e.effect_type == "distance_field") {
            self.ensure_jfa(device);
        }

        // Verify all enabled effects are actually compiled (diagnostic)
        for effect in &enabled_effects {
            if effect.effect_type == "distance_field" {
                continue; // multi-pass special case, not in compiled_effects
            }
            if !self.compiled_effects.contains_key(&effect.effect_type) {
                log::warn!("Effect '{}' is enabled but NOT compiled — likely unknown to registry", effect.effect_type);
            }
        }

        // Pre-allocate slots for all effects (before borrowing textures).
        // The distance-field effect runs one pass per slot: seed +
        // jump floods + composite.
        let jfa_slots = Self::jfa_pass_count(self.width, self.height) as usize + 2;
        let slots_of = |e: &EffectInstance| if e.effect_type == "distance_field" { jfa_slots } else { 1 };
        self.reserve_slots(device, enabled_effects.iter().map(|e| slots_of(e)).sum());
        let slot_offsets: Vec<Vec<u64>> = enabled_effects
            .iter()
            .map(|e| (0..slots_of(e)).map(|_| self.allocate_slot()).collect())
            .collect();
        // The JFA resources leave `self` for the loop below (the
        // color ping-pong holds the &mut borrow).
        let jfa = self.jfa.take();

        // Extract data needed for effect execution
        let width = self.width;
        let height = self.height;

        // Now run effects
        if let Some(textures) = self.color_textures.as_mut() {
            // Reset read index so first write goes to texture A
            textures.read_index = 1; // So write_view() returns A

            for (i, effect) in enabled_effects.iter().enumerate() {
                // First effect reads from input texture, subsequent effects read from ping-pong
                let read_view = if i == 0 {
                    input_view
                } else {
                    textures.read_view()
                };

                if effect.effect_type == "distance_field" {
                    if let Some(j) = jfa.as_ref() {
                        Self::run_jfa(
                            device,
                            queue,
                            encoder,
                            effect,
                            j,
                            read_view,
                            textures.write_view(),
                            &self.params_buffer,
                            &slot_offsets[i],
                            &self.sampler,
                            width,
                            height,
                            self.pixel_scale,
                        );
                        textures.swap();
                    }
                    continue;
                }

                Self::run_single_effect_with_input(
                    device,
                    queue,
                    encoder,
                    &effect.effect_type,
                    effect,
                    read_view,
                    textures.write_view(),
                    &self.compiled_effects,
                    &self.params_buffer,
                    slot_offsets[i][0],
                    &self.sampler,
                    width,
                    height,
                    0.0, // colour effects have no density
                    self.pixel_scale,
                );

                // Swap ping-pong textures for next effect
                textures.swap();
            }
            self.jfa = jfa;
            return true;
        }
        self.jfa = jfa;
        false
    }

    /// Get the output texture view from color effects (if any were run)
    pub fn get_color_output(&self) -> Option<&TextureView> {
        self.color_textures.as_ref().map(|t| t.read_view())
    }

    /// Run a single effect with explicit input/output views
    ///
    /// `buffer_offset` is the byte offset into the shared params buffer for this effect's slot.
    /// Each effect gets a unique slot to avoid parameter overwrites in the same command buffer.
    fn run_single_effect_with_input(
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        effect_name: &str,
        effect: &EffectInstance,
        input_view: &TextureView,
        output_view: &TextureView,
        compiled_effects: &HashMap<String, CompiledEffect>,
        params_buffer: &Buffer,
        buffer_offset: u64,
        sampler: &Sampler,
        width: u32,
        height: u32,
        density_mean: f32,
        pixel_scale: f32,
    ) {
        let compiled = match compiled_effects.get(effect_name) {
            Some(c) => c,
            None => {
                log::warn!("Effect {} not compiled, skipping", effect_name);
                return;
            }
        };
        log::debug!("Executing effect: {} ({}x{}) at slot offset {}", effect_name, width, height, buffer_offset);

        // Update params buffer at the allocated slot offset
        let mut params = EffectParams {
            params: [[0.0; 4]; MAX_EFFECT_PARAMS / 4],
            width,
            height,
            density_mean,
            pixel_scale,
        };

        // Fill in effect parameters (packed into vec4s)
        let registry = global_effect_registry();
        if let Some(info) = registry.get(effect_name) {
            for (i, param_def) in info.parameters.iter().enumerate() {
                if i < MAX_EFFECT_PARAMS {
                    params.params[i / 4][i % 4] = effect.get_param(&param_def.name);
                }
            }
        }

        // Write to this effect's slot in the shared buffer
        queue.write_buffer(params_buffer, buffer_offset, bytemuck::bytes_of(&params));

        // Create bind group with buffer slice at this slot's offset
        use wgpu::BufferBinding;
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some(&format!("{} Bind Group", effect_name)),
            layout: &compiled.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(input_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(sampler),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::Buffer(BufferBinding {
                        buffer: params_buffer,
                        offset: buffer_offset,
                        size: Some(std::num::NonZeroU64::new(std::mem::size_of::<EffectParams>() as u64).unwrap()),
                    }),
                },
            ],
        });

        // Run the effect
        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some(&format!("{} Pass", effect_name)),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: output_view,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            render_pass.set_pipeline(&compiled.pipeline);
            render_pass.set_bind_group(0, &bind_group, &[]);
            render_pass.draw(0..3, 0..1); // Fullscreen triangle
        }
    }

    /// Read pixels from the color effect output texture
    /// Returns (width, height, rgba_data)
    pub async fn read_color_output_pixels(
        &self,
        device: &Device,
        queue: &Queue,
    ) -> Result<(u32, u32, Vec<u8>), String> {
        let textures = self.color_textures.as_ref()
            .ok_or_else(|| "No color effect output available".to_string())?;

        // Wait for any pending rendering to complete
        let sync_encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Effect Read Sync"),
        });
        queue.submit(std::iter::once(sync_encoder.finish()));
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });

        // Create staging buffer
        let bytes_per_pixel = 4; // RGBA8
        let unpadded_bytes_per_row = self.width * bytes_per_pixel;
        let align = COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
        let buffer_size = (padded_bytes_per_row * self.height) as u64;

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Effect Read Buffer"),
            size: buffer_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        // Copy texture to buffer
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor {
            label: Some("Effect Read Encoder"),
        });
        encoder.copy_texture_to_buffer(
            TexelCopyTextureInfo {
                texture: textures.read_texture(),
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            TexelCopyBufferInfo {
                buffer: &buffer,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded_bytes_per_row),
                    rows_per_image: Some(self.height),
                },
            },
            Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );
        queue.submit(std::iter::once(encoder.finish()));

        // Map buffer and read data
        let buffer_slice = buffer.slice(..);
        let (tx, rx) = futures::channel::oneshot::channel();
        buffer_slice.map_async(MapMode::Read, move |result| {
            let _ = tx.send(result);
        });

        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });

        rx.await
            .map_err(|_| "Failed to receive map result".to_string())?
            .map_err(|e| format!("Failed to map buffer: {:?}", e))?;

        // Extract pixel data (removing row padding)
        let data = buffer_slice.get_mapped_range();
        let mut rgba_data = Vec::with_capacity((self.width * self.height * 4) as usize);

        for row in 0..self.height {
            let row_start = (row * padded_bytes_per_row) as usize;
            let row_end = row_start + (self.width * bytes_per_pixel) as usize;
            rgba_data.extend_from_slice(&data[row_start..row_end]);
        }

        drop(data);
        buffer.unmap();
        // Explicit, because dropping frees nothing on WebGPU — this
        // read runs per export/tile when color effects are on.
        buffer.destroy();

        Ok((self.width, self.height, rgba_data))
    }

    /// Check if there's a color output texture available
    pub fn has_color_output(&self) -> bool {
        self.color_textures.is_some()
    }

    /// Create a staging buffer for reading color effect output (for WASM async export)
    /// Returns (buffer, padded_bytes_per_row)
    pub fn create_color_staging_buffer(&self, device: &Device) -> (Buffer, u32) {
        let bytes_per_pixel = 4u32; // RGBA8
        let unpadded_bytes_per_row = self.width * bytes_per_pixel;
        let align = COPY_BYTES_PER_ROW_ALIGNMENT;
        let padded_bytes_per_row = ((unpadded_bytes_per_row + align - 1) / align) * align;
        let buffer_size = (padded_bytes_per_row * self.height) as u64;

        let buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Effect Color Staging Buffer"),
            size: buffer_size,
            usage: BufferUsages::COPY_DST | BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        (buffer, padded_bytes_per_row)
    }

    /// Copy color effect output texture to a staging buffer (for WASM async export)
    pub fn copy_color_to_buffer(&self, encoder: &mut CommandEncoder, buffer: &Buffer, padded_bytes_per_row: u32) {
        if let Some(textures) = self.color_textures.as_ref() {
            encoder.copy_texture_to_buffer(
                TexelCopyTextureInfo {
                    texture: textures.read_texture(),
                    mip_level: 0,
                    origin: Origin3d::ZERO,
                    aspect: TextureAspect::All,
                },
                TexelCopyBufferInfo {
                    buffer,
                    layout: TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(padded_bytes_per_row),
                        rows_per_image: Some(self.height),
                    },
                },
                Extent3d {
                    width: self.width,
                    height: self.height,
                    depth_or_array_layers: 1,
                },
            );
        }
    }

    /// Get the dimensions of the effect chain
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
}

#[cfg(test)]
mod include_tests {
    use super::process_shader_includes;

    const MARKER: &str = "// INCLUDE_BLEND_MODES";

    /// A marker on its own line is a directive and gets spliced.
    #[test]
    fn a_standalone_marker_is_replaced() {
        let out = process_shader_includes(format!("fn a() {{}}\n{MARKER}\nfn b() {{}}"));
        assert!(out.contains("fn apply_blend"), "the library must be spliced in");
        assert!(!out.contains(MARKER), "and the directive consumed");
        assert!(out.contains("fn a()") && out.contains("fn b()"));
    }

    /// A marker QUOTED IN PROSE is not a directive.
    ///
    /// A plain `replace` fires on it, splicing two hundred lines of
    /// library into the middle of a sentence — the shader then fails to
    /// compile pointing at a line its author never wrote. No built-in
    /// quotes the marker, which is why this went unnoticed until the
    /// first plugin written to *document* it did exactly that.
    #[test]
    fn a_marker_quoted_in_a_comment_is_left_alone() {
        let src = format!("// this shader uses the `{MARKER}` splice.\nfn a() {{}}");
        let out = process_shader_includes(src.clone());
        assert_eq!(out, src, "prose must survive untouched");
        assert!(!out.contains("fn apply_blend"));
    }

    /// ...and a shader that both documents and uses the marker gets
    /// exactly one copy of the library.
    #[test]
    fn documenting_and_using_the_marker_splices_once() {
        let src = format!("// mentions `{MARKER}` in prose\n{MARKER}\nfn a() {{}}");
        let out = process_shader_includes(src);
        assert_eq!(out.matches("fn apply_blend").count(), 1);
        assert!(out.contains(&format!("`{MARKER}`")), "the prose copy stays");
    }

    /// A repeated directive includes once. Two copies of the library is
    /// a redefinition error, and a duplicated directive plainly means
    /// "include it", not "include it twice".
    #[test]
    fn a_repeated_directive_includes_once() {
        let out = process_shader_includes(format!("{MARKER}\nfn a() {{}}\n{MARKER}"));
        assert_eq!(out.matches("fn apply_blend").count(), 1);
    }

    /// No marker, no change — and no cost.
    #[test]
    fn a_shader_without_the_marker_is_untouched() {
        let src = "fn a() {}".to_string();
        assert_eq!(process_shader_includes(src.clone()), src);
    }
}

#[cfg(test)]
mod gpu_tests {
    use super::*;

    /// A device with `features` (None, and the test skips, without an
    /// adapter offering them). The density chain needs
    /// `FLOAT32_FILTERABLE`; the colour chain nothing.
    fn device_with(features: wgpu::Features) -> Option<(Device, Queue)> {
        pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions::default()).await.ok()?;
            if !adapter.features().contains(features) {
                return None;
            }
            adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("effect chain test"),
                    required_features: features,
                    required_limits: wgpu::Limits::default(),
                    memory_hints: wgpu::MemoryHints::Performance,
                    experimental_features: Default::default(),
                    trace: Default::default(),
                })
                .await
                .ok()
        })
    }

    /// A `w` x `h` texture of `format`, every texel `texel` (as bytes).
    fn filled(device: &Device, queue: &Queue, w: u32, h: u32, format: TextureFormat, texel: &[u8]) -> (Texture, TextureView) {
        let data: Vec<u8> = (0..w * h).flat_map(|_| texel.iter().copied()).collect();
        texture_of(device, queue, w, h, format, &data)
    }

    /// A `w` x `h` texture of `format` holding `data`, row-major.
    fn texture_of(device: &Device, queue: &Queue, w: u32, h: u32, format: TextureFormat, data: &[u8]) -> (Texture, TextureView) {
        let texel_bytes = data.len() as u32 / (w * h);
        let tex = device.create_texture(&TextureDescriptor {
            label: Some("effect chain test input"),
            size: Extent3d { width: w, height: h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            TexelCopyTextureInfo { texture: &tex, mip_level: 0, origin: Origin3d::ZERO, aspect: TextureAspect::All },
            data,
            TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(w * texel_bytes), rows_per_image: Some(h) },
            Extent3d { width: w, height: h, depth_or_array_layers: 1 },
        );
        let view = tex.create_view(&TextureViewDescriptor::default());
        (tex, view)
    }

    /// A raw hit count past f16's 65504 survives the density chain. The
    /// ping-pong was Rgba16Float, so sharpen at amount 0 -- which returns
    /// its input untouched -- turned a pixel of a million hits into Inf,
    /// black under the Log tone map.
    #[test]
    fn a_hit_count_past_half_float_survives_the_density_chain() {
        let Some((device, queue)) = device_with(wgpu::Features::FLOAT32_FILTERABLE) else {
            eprintln!("no adapter with FLOAT32_FILTERABLE; skipping");
            return;
        };
        const W: u32 = 64;
        const H: u32 = 4;
        let texel: Vec<u8> = [0.5f32, 0.25, 0.75, 1.0e6].iter().flat_map(|v| v.to_le_bytes()).collect();
        let (_input, view) = filled(&device, &queue, W, H, TextureFormat::Rgba32Float, &texel);
        let mut sharpen = EffectInstance::new("sharpen");
        sharpen.set_param("amount", 0.0);
        let mut chain = EffectChainRunner::new(&device, W, H);
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("density") });
        chain.reset_slots();
        let ran = chain.run_density_effects(&device, &queue, &mut encoder, &view, &[sharpen], 1.0);
        queue.submit(Some(encoder.finish()));
        let error = pollster::block_on(scope.pop());
        assert!(error.is_none(), "the density chain failed validation: {error:?}");
        assert!(ran, "the chain ran");
        let out = chain.read_density_output_blocking(&device, &queue);
        for (i, px) in out.iter().enumerate() {
            assert!(px[3].is_finite(), "texel {i}: alpha {} is not finite", px[3]);
            assert!((px[3] / 1.0e6 - 1.0).abs() < 1e-6, "texel {i}: alpha {} is not the million hits it was given", px[3]);
            assert!((px[0] - 0.5).abs() < 1e-6 && (px[2] - 0.75).abs() < 1e-6, "texel {i}: the colour changed: {px:?}");
        }
        chain.destroy();
    }

    /// density_blur's threshold and falloff are multiples of the frame's
    /// mean density, the unit Levels uses: the same pixels blur whether
    /// the render has run a million iterations or a billion. They were
    /// compared with the raw hit count, so a pixel with a single hit was
    /// never blurred, at any setting in the 0-1 range.
    ///
    /// Columns alternate red 0 and 1 at 1,000 hits each. Against a mean
    /// of 10,000 that is a tenth of the mean, below the default 0.25, and
    /// a middle texel blurs toward 0.5; against a mean of 100 it is ten
    /// times the mean, past 0.25 + 0.5, and stays as it was.
    #[test]
    fn density_blur_measures_density_in_multiples_of_the_mean() {
        let Some((device, queue)) = device_with(wgpu::Features::FLOAT32_FILTERABLE) else {
            eprintln!("no adapter with FLOAT32_FILTERABLE; skipping");
            return;
        };
        const W: u32 = 32;
        const H: u32 = 8;
        let data: Vec<u8> = (0..W * H)
            .flat_map(|i| [(i % W % 2) as f32, 0.0, 0.0, 1000.0])
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let (_input, view) = texture_of(&device, &queue, W, H, TextureFormat::Rgba32Float, &data);
        let red_at_centre = |mean: f32| {
            let mut chain = EffectChainRunner::new(&device, W, H);
            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("density") });
            chain.reset_slots();
            let ran = chain.run_density_effects(&device, &queue, &mut encoder, &view, &[EffectInstance::new("density_blur")], mean);
            queue.submit(Some(encoder.finish()));
            assert!(ran, "the chain ran");
            let out = chain.read_density_output_blocking(&device, &queue);
            chain.destroy();
            out[(H / 2 * W + W / 2) as usize][0]
        };
        let sparse = red_at_centre(10_000.0);
        assert!((sparse - 0.5).abs() < 0.2, "a tenth of the mean is sparse, but the stripes stayed sharp: red {sparse}");
        let dense = red_at_centre(100.0);
        assert!(dense == 0.0 || dense == 1.0, "ten times the mean is dense, but the stripes blurred: red {dense}");
    }

    /// Sizes in pixels are in logical points (`set_pixel_scale`, the
    /// viewport's render scale): at half scale a radius of 8 is the radius
    /// 4 at full scale, to the bit, for a density blur and a colour
    /// effect's offset alike.
    #[test]
    fn a_size_in_pixels_follows_the_pixel_scale() {
        let Some((device, queue)) = device_with(wgpu::Features::FLOAT32_FILTERABLE) else {
            eprintln!("no adapter with FLOAT32_FILTERABLE; skipping");
            return;
        };
        const W: u32 = 32;
        const H: u32 = 8;
        let data: Vec<u8> = (0..W * H)
            .flat_map(|i| [(i % W % 2) as f32, 0.0, 0.0, 1000.0])
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let (_input, view) = texture_of(&device, &queue, W, H, TextureFormat::Rgba32Float, &data);
        let blurred = |radius: f32, scale: f32| {
            let mut blur = EffectInstance::new("density_blur");
            blur.set_param("radius", radius);
            let mut chain = EffectChainRunner::new(&device, W, H);
            chain.set_pixel_scale(scale);
            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("density") });
            chain.reset_slots();
            assert!(chain.run_density_effects(&device, &queue, &mut encoder, &view, &[blur], 10_000.0));
            queue.submit(Some(encoder.finish()));
            let out = chain.read_density_output_blocking(&device, &queue);
            chain.destroy();
            out
        };
        assert_eq!(blurred(8.0, 0.5), blurred(4.0, 1.0), "density_blur's radius");
        assert_ne!(blurred(8.0, 1.0), blurred(4.0, 1.0), "the radius changes the blur at all");

        let colour: Vec<u8> = (0..W * H).flat_map(|i| [((i % W) * 8) as u8, 0, ((i / W) * 30) as u8, 255]).collect();
        let (_c, cview) = texture_of(&device, &queue, W, H, TextureFormat::Rgba8Unorm, &colour);
        let shifted = |amount: f32, scale: f32| {
            let mut ca = EffectInstance::new("chromatic_aberration");
            ca.set_param("amount", amount);
            ca.set_param("radial", 0.0);
            let mut chain = EffectChainRunner::new(&device, W, H);
            chain.set_pixel_scale(scale);
            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("colour") });
            chain.reset_slots();
            assert!(chain.run_color_effects(&device, &queue, &mut encoder, &cview, &[ca]));
            queue.submit(Some(encoder.finish()));
            let out = pollster::block_on(chain.read_color_output_pixels(&device, &queue)).expect("read").2;
            chain.destroy();
            out
        };
        assert_eq!(shifted(6.0, 0.5), shifted(3.0, 1.0), "chromatic aberration's amount");
        assert_ne!(shifted(6.0, 1.0), shifted(3.0, 1.0), "the amount moves the channels at all");
    }

    /// A frame's chains may need more parameter slots than the buffer
    /// starts with. Three distance fields at 1080p take 13 slots each (11
    /// jump floods + seed + composite), and a vignette one more: 40
    /// against 32. The 33rd allocation used to be clamped to slot 32 --
    /// an offset equal to the buffer's size -- so its write and bind group
    /// were validation errors and the whole frame was rejected.
    #[test]
    fn a_chain_needing_more_than_the_initial_slots_validates() {
        let Some((device, queue)) = device_with(wgpu::Features::empty()) else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        const W: u32 = 1920;
        const H: u32 = 1080;
        let (_input, view) = filled(&device, &queue, W, H, TextureFormat::Rgba8Unorm, &[200, 120, 40, 255]);
        let effects: Vec<EffectInstance> = ["distance_field", "distance_field", "distance_field", "vignette"]
            .into_iter()
            .map(EffectInstance::new)
            .collect();
        let mut chain = EffectChainRunner::new(&device, W, H);
        let needed = 3 * (EffectChainRunner::jfa_pass_count(W, H) as usize + 2) + 1;
        assert!(needed > MAX_EFFECT_SLOTS, "the fixture needs {needed} slots, not more than the initial {MAX_EFFECT_SLOTS}");
        // Two frames: the first grows the buffer, the second reuses it.
        for frame in 0..2 {
            let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("color") });
            chain.reset_slots();
            let ran = chain.run_color_effects(&device, &queue, &mut encoder, &view, &effects);
            queue.submit(Some(encoder.finish()));
            let error = pollster::block_on(scope.pop());
            assert!(error.is_none(), "frame {frame}: the colour chain failed validation: {error:?}");
            assert!(ran, "frame {frame}: the chain ran");
            assert!(chain.slot_capacity >= needed, "frame {frame}: {} slots for a chain of {needed}", chain.slot_capacity);
        }
        let _ = device.poll(PollType::wait_indefinitely());
        chain.destroy();
    }
}
