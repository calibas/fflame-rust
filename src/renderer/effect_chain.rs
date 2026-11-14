//! Effect Chain Runner
//!
//! Executes shader effects in sequence using ping-pong textures.
//! - Density effects: Run on Rgba32Float before tonemap (`DENSITY_FORMAT`)
//! - Color effects: Run on Rgba8Unorm after tonemap

use std::collections::HashMap;

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

/// Where a picture is in an animation, for the effects that read it
/// (`ctx.frame`, `ctx.time`, `ctx.dt`) and for a multi-pass effect's
/// persistent buffers (docs/archive/projects/multipass-effects.md, T1-T4;
/// `renderer::effect_passes`).
///
/// A still -- the default -- has no history: frame 0, a persistent
/// buffer zero. A frame of a sequence is `epoch` and `frame`: a run at the
/// frame after the last carries the history on, one at the same frame
/// re-reads it, any other starts a new history. A new `epoch` is a reset.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EffectClock {
    pub still: bool,
    pub epoch: u64,
    pub frame: u64,
    /// The animation's time, seconds.
    pub time: f32,
    /// Seconds since the frame before: `1 / fps` in a video, the real
    /// interval in the viewport's playback, 0 for a still.
    pub dt: f32,
}

impl EffectClock {
    pub const STILL: Self = Self { still: true, epoch: 0, frame: 0, time: 0.0, dt: 0.0 };

    /// Frame `frame` of the sequence `epoch`, at `time`, `dt` after the
    /// one before.
    pub fn frame(epoch: u64, frame: u64, time: f32, dt: f32) -> Self {
        Self { still: false, epoch, frame, time, dt }
    }

    /// Frame `frame` of a video at `fps`: one sequence, frames in order.
    pub fn video(frame: u64, fps: u32) -> Self {
        let fps = fps.max(1) as f32;
        Self::frame(0, frame, frame as f32 / fps, 1.0 / fps)
    }
}

impl Default for EffectClock {
    fn default() -> Self {
        Self::STILL
    }
}

/// GPU uniform buffer for effect parameters
///
/// `[[f32; 4]; 12]` to match WGSL `array<vec4<f32>, 12>` — vec4 groups
/// because a uniform array's stride must be 16-byte aligned. The 12
/// must stay in step with `MAX_EFFECT_PARAMS` (48 = 12 × 4) and with
/// the struct declared in every `shaders/effects/**/*.wgsl`; a mismatch
/// is a binding-size validation error at pipeline creation.
///
/// Total size: 256 bytes, the slot stride exactly: params=192, then
/// width, height, density_mean, pixel_scale (16), then a multi-pass
/// effect's pass fields and the frame's time fields
/// (docs/archive/projects/multipass-effects.md; 48 with padding). A shader that
/// declares the first 208 bytes alone -- every WGSL effect written before
/// them, `density_mean` where an 8-byte padding was -- binds the same
/// layout and ignores the rest.
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(super) struct EffectParams {
    /// Effect parameters (`MAX_EFFECT_PARAMS` floats, packed as vec4s
    /// for alignment)
    params: [[f32; 4]; MAX_EFFECT_PARAMS / 4],
    /// The size of the grid the pass writes (the image's, for one pass)
    pub(super) width: u32,
    pub(super) height: u32,
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
    /// A multi-pass effect's pass run (`renderer::effect_passes`): the
    /// repeat's index and count, the jump flood's step `2^(passes - 1 -
    /// pass_index)`, the pyramid level a pass over levels writes and the
    /// level count of its target, and each buffer's level to read, 4 bits
    /// a buffer. 0 (one run) for a one-pass effect.
    pub(super) pass_index: u32,
    pub(super) passes: u32,
    pub(super) step: u32,
    pub(super) level: u32,
    pub(super) levels: u32,
    pub(super) buffer_levels: u32,
    /// The frame's place in an animation (`ctx.frame`, `ctx.time`,
    /// `ctx.dt`): 0 for a still.
    pub(super) frame: u32,
    pub(super) time: f32,
    pub(super) dt: f32,
    _pad: [f32; 3],
}

impl EffectParams {
    /// The fields every effect reads, the rest 0 until a pass run or a
    /// frame sets them.
    fn new(width: u32, height: u32, density_mean: f32, pixel_scale: f32) -> Self {
        Self { width, height, density_mean, pixel_scale, passes: 1, levels: 1, ..bytemuck::Zeroable::zeroed() }
    }

    /// The frame's fields from `clock`.
    fn with_clock(mut self, clock: &EffectClock) -> Self {
        self.frame = if clock.still { 0 } else { clock.frame.min(u32::MAX as u64) as u32 };
        self.time = clock.time;
        self.dt = clock.dt;
        self
    }

    /// `effect`'s parameters by `name`'s registered definitions, packed
    /// into the vec4 groups.
    fn with_parameters(mut self, name: &str, effect: &EffectInstance) -> Self {
        if let Some(info) = global_effect_registry().get(name) {
            for (i, param_def) in info.parameters.iter().enumerate().take(MAX_EFFECT_PARAMS) {
                self.params[i / 4][i % 4] = effect.get_param(&param_def.name);
            }
        }
        self
    }
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
    /// The registry's revision of the effect it was built from
    /// (`EffectRegistry::revision`): a newer one is compiled again.
    revision: u64,
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
    /// Multi-pass effects compiled, by name (`renderer::effect_passes`;
    /// docs/archive/projects/multipass-effects.md), and the buffers of each one in
    /// a chain, by its chain and its place among the enabled effects.
    multipass: HashMap<String, super::effect_passes::Compiled>,
    multipass_states: HashMap<(EffectCategory, usize), super::effect_passes::State>,
    /// Render pixels per logical viewport point (`set_pixel_scale`): the
    /// viewport's render scale. 1 for the exporters.
    pixel_scale: f32,
    /// The frame the chain runs at (`set_clock`): a still until told.
    clock: EffectClock,
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
            multipass: HashMap::new(),
            multipass_states: HashMap::new(),
            pixel_scale: 1.0,
            clock: EffectClock::STILL,
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
        self.multipass_states.values().for_each(|s| s.destroy());
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

    /// The frame the next runs are at ([`EffectClock`]).
    pub fn set_clock(&mut self, clock: EffectClock) {
        self.clock = clock;
    }

    pub fn resize(&mut self, _device: &Device, width: u32, height: u32) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            // Textures will be recreated on next use
            self.density_textures = None;
            self.color_textures = None;
            for (_, state) in self.multipass_states.drain() {
                state.destroy();
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

    /// Compile an effect shader if not already compiled
    fn ensure_effect_compiled(&mut self, device: &Device, effect_name: &str, category: EffectCategory) {
        let registry = global_effect_registry();
        // Built from the current registration: an effect registered again
        // (a Rhai effect edited, a download updated) is built again.
        let revision = registry.revision(effect_name);
        if self.compiled_effects.get(effect_name).is_some_and(|c| c.revision == revision)
            || self.multipass.get(effect_name).is_some_and(|c| c.revision == revision)
        {
            log::debug!("Effect {} already compiled", effect_name);
            return;
        }
        log::info!("Compiling effect shader: {}", effect_name);

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

        // A multi-pass effect: a pipeline a pass (`effect_passes`).
        if let Some(plan) = effect_info.passes.clone() {
            let (format, blend) = match category {
                EffectCategory::Density => (DENSITY_FORMAT, None),
                EffectCategory::Color => (TextureFormat::Rgba8Unorm, Some(BlendState::REPLACE)),
            };
            let compiled = super::effect_passes::Compiled::new(device, effect_name, &shader_source, plan, revision, format, blend);
            self.multipass.insert(effect_name.to_string(), compiled);
            log::info!("Compiled multi-pass effect: {}", effect_name);
            return;
        }

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
                revision,
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

        // Pre-allocate slots for all effects (before borrowing textures):
        // a multi-pass effect takes one a pass run.
        let slot_offsets = self.allocate_effect_slots(device, &enabled_effects);
        self.ensure_multipass_states(device, &enabled_effects, EffectCategory::Density);

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

                if let (Some(compiled), Some(state)) = (self.multipass.get(&effect.effect_type), self.multipass_states.get_mut(&(EffectCategory::Density, i))) {
                    let inputs = super::effect_passes::Inputs {
                        input: read_view,
                        output: textures.write_view(),
                        sampler: &self.sampler,
                        params_buffer: &self.params_buffer,
                        slots: &slot_offsets[i],
                        base: EffectParams::new(width, height, density_mean, self.pixel_scale).with_clock(&self.clock).with_parameters(&effect.effect_type, effect),
                    };
                    super::effect_passes::run(device, queue, encoder, compiled, state, &inputs, &self.clock);
                    textures.swap();
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
                    density_mean,
                    self.pixel_scale,
                    &self.clock,
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

        // Verify all enabled effects are actually compiled (diagnostic)
        for effect in &enabled_effects {
            if !self.compiled_effects.contains_key(&effect.effect_type) && !self.multipass.contains_key(&effect.effect_type) {
                log::warn!("Effect '{}' is enabled but NOT compiled — likely unknown to registry", effect.effect_type);
            }
        }

        // Pre-allocate slots for all effects (before borrowing textures):
        // a multi-pass effect takes one a pass run.
        let slot_offsets = self.allocate_effect_slots(device, &enabled_effects);
        self.ensure_multipass_states(device, &enabled_effects, EffectCategory::Color);

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

                if let (Some(compiled), Some(state)) = (self.multipass.get(&effect.effect_type), self.multipass_states.get_mut(&(EffectCategory::Color, i))) {
                    let inputs = super::effect_passes::Inputs {
                        input: read_view,
                        output: textures.write_view(),
                        sampler: &self.sampler,
                        params_buffer: &self.params_buffer,
                        slots: &slot_offsets[i],
                        base: EffectParams::new(width, height, 0.0, self.pixel_scale).with_clock(&self.clock).with_parameters(&effect.effect_type, effect),
                    };
                    super::effect_passes::run(device, queue, encoder, compiled, state, &inputs, &self.clock);
                    textures.swap();
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
                    &self.clock,
                );

                // Swap ping-pong textures for next effect
                textures.swap();
            }
            return true;
        }
        false
    }

    /// Parameter slots for this frame's `effects`, before any texture is
    /// borrowed: one each, a multi-pass effect one a pass run.
    fn allocate_effect_slots(&mut self, device: &Device, effects: &[&EffectInstance]) -> Vec<Vec<u64>> {
        let (w, h) = (self.width, self.height);
        let counts: Vec<usize> = effects.iter().map(|e| self.multipass.get(&e.effect_type).map_or(1, |c| c.plan.runs(w, h))).collect();
        self.reserve_slots(device, counts.iter().sum());
        counts.iter().map(|&n| (0..n).map(|_| self.allocate_slot()).collect()).collect()
    }

    /// Buffers for each multi-pass effect in a chain, by its place: kept
    /// while the same effect, revision and size sit there, else made
    /// again; those of places with no multi-pass effect now freed.
    fn ensure_multipass_states(&mut self, device: &Device, effects: &[&EffectInstance], category: EffectCategory) {
        let size = (self.width, self.height);
        let stale: Vec<(EffectCategory, usize)> = self
            .multipass_states
            .iter()
            .filter(|(&(c, i), state)| {
                c == category
                    && !effects.get(i).and_then(|e| self.multipass.get(&e.effect_type).map(|comp| state.fits(&e.effect_type, comp, size))).unwrap_or(false)
            })
            .map(|(k, _)| *k)
            .collect();
        for k in stale {
            if let Some(state) = self.multipass_states.remove(&k) {
                state.destroy();
            }
        }
        for (i, e) in effects.iter().enumerate() {
            if let Some(compiled) = self.multipass.get(&e.effect_type) {
                self.multipass_states.entry((category, i)).or_insert_with(|| super::effect_passes::State::new(device, &e.effect_type, compiled, size));
            }
        }
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
        clock: &EffectClock,
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
        let params = EffectParams::new(width, height, density_mean, pixel_scale).with_clock(clock).with_parameters(effect_name, effect);

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

    /// Every built-in effect compiles as the chain builds it, includes
    /// spliced. Without a GPU nothing else compiles them, and a WGSL slip
    /// is a pipeline panic the first time someone adds that effect.
    #[test]
    fn every_built_in_effect_validates() {
        use wgpu::naga;
        let registry = crate::effects::global_effect_registry();
        let mut checked = 0;
        for info in registry.all().filter(|i| i.provenance == crate::provenance::Provenance::Builtin) {
            let src = super::effect_wgsl(&info.source);
            let module = naga::front::wgsl::parse_str(&src)
                .unwrap_or_else(|e| panic!("{}: {}", info.name, e.emit_to_string(&src)));
            naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
                .validate(&module)
                .unwrap_or_else(|e| panic!("{}: {e:?}", info.name));
            checked += 1;
        }
        assert!(checked >= 15, "{checked}");
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

    /// A colour effect through the chain: `fn effect` per pixel, written
    /// back over the input.
    fn run_colour(device: &Device, queue: &Queue, view: &TextureView, w: u32, h: u32, effects: &[EffectInstance]) -> Vec<u8> {
        let mut chain = EffectChainRunner::new(device, w, h);
        let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
        let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("colour") });
        chain.reset_slots();
        assert!(chain.run_color_effects(device, queue, &mut encoder, view, effects), "the chain ran");
        queue.submit(Some(encoder.finish()));
        let error = pollster::block_on(scope.pop());
        assert!(error.is_none(), "the colour chain failed validation: {error:?}");
        let out = pollster::block_on(chain.read_color_output_pixels(device, queue)).expect("read").2;
        chain.destroy();
        out
    }

    /// A Rhai effect (P2) renders through the colour chain as its source
    /// says: an inversion, and a blend over the input by the library.
    #[test]
    fn a_rhai_effect_renders_through_the_colour_chain() {
        let Some((device, queue)) = device_with(wgpu::Features::empty()) else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        const W: u32 = 16;
        const H: u32 = 4;
        let (_input, view) = filled(&device, &queue, W, H, TextureFormat::Rgba8Unorm, &[200, 120, 40, 255]);
        let register = |id: &str, body: &str| {
            let src = format!("const INFO = #{{ id: \"{id}\" }};\n{body}");
            crate::effects::custom::register(&src, crate::provenance::Provenance::Local).unwrap_or_else(|e| panic!("{e}"))
        };
        let invert = register("3c1b5f0e-2a4d-4e6f-8a9b-0c1d2e3f4a5b", "fn effect(c, p) { rgba(vec3(1.0) - c.rgb, c.a) }");
        let out = run_colour(&device, &queue, &view, W, H, &[EffectInstance::new(&invert)]);
        for px in out.chunks(4) {
            let near = |a: u8, b: u8| (a as i32 - b as i32).abs() <= 1;
            assert!(near(px[0], 55) && near(px[1], 135) && near(px[2], 215) && px[3] == 255, "inverted: {px:?}");
        }
        // Multiply (2) by white at full amount leaves the input.
        let multiply = register(
            "4d2c6a1f-3b5e-4f70-9bac-1d2e3f4a5b6c",
            "fn effect(c, p) { #{ color: vec3(1.0, 1.0, 1.0), blend: 2, amount: 1.0 } }",
        );
        let out = run_colour(&device, &queue, &view, W, H, &[EffectInstance::new(&multiply)]);
        assert!(out.chunks(4).all(|px| px == [200, 120, 40, 255]), "multiplied by white: {:?}", &out[..4]);
        let mut registry = crate::effects::global_effect_registry_mut();
        registry.remove_by_name(&invert);
        registry.remove_by_name(&multiply);
    }

    /// A multi-pass effect (docs/archive/projects/multipass-effects.md) runs as its
    /// plan says: a pass writes a buffer, a repeated pass reads the
    /// version before (the ping-pong), `ctx.pass` counts, `ctx.step`
    /// halves, a half-size buffer reads back filtered, and the last pass
    /// reads it all into the output.
    #[test]
    fn a_multi_pass_effect_runs_its_passes() {
        let Some((device, queue)) = device_with(wgpu::Features::empty()) else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        const W: u32 = 16;
        const H: u32 = 8;
        let (_input, view) = filled(&device, &queue, W, H, TextureFormat::Rgba8Unorm, &[200, 100, 40, 255]);
        let src = r#"
const INFO = #{
    id: "5e2d8a1b-4c3f-4a6e-9b7d-2c1e0f3a4b5d",
    buffers: #{ acc: #{}, small: #{ scale: 0.5 } },
    passes: [
        #{ run: "start", to: "acc" },
        #{ run: "add", to: "acc", repeat: 3 },
        #{ run: "shrink", to: "small" },
        #{ run: "effect" },
    ],
};
// The input inverted, and 0 in green.
fn start(c, p, ctx) { rgba(1.0 - c.r, 0.0, 0.0, 1.0) }
// Each run adds 0.1 to green, and records its step in blue: 4, 2, 1.
fn add(c, p, ctx) {
    let before = buffer_at("acc", to_int(ctx.xy.re), to_int(ctx.xy.im));
    rgba(before.r, before.g + 0.1, to_float(ctx.step) * 0.1, to_float(ctx.passes) * 0.1)
}
// Half the size: its grid is 8 x 4.
fn shrink(c, p, ctx) { rgba(to_float(ctx.width) / 16.0, to_float(ctx.height) / 16.0, 0.0, 1.0) }
fn effect(c, p, ctx) {
    let a = buffer("acc", ctx.uv);
    let s = buffer("small", ctx.uv);
    rgba(a.r, a.g + s.r, a.b + s.g, a.a)
}
"#;
        let name = crate::effects::custom::register(src, crate::provenance::Provenance::Local).unwrap_or_else(|e| panic!("{e}"));
        let out = run_colour(&device, &queue, &view, W, H, &[EffectInstance::new(&name)]);
        crate::effects::global_effect_registry_mut().remove_by_name(&name);
        let near = |a: u8, b: f32| (a as f32 - b * 255.0).abs() <= 1.0;
        for px in out.chunks(4) {
            // r: 1 - 200/255; g: three runs of 0.1, then the small grid's
            // width 8/16; b: the last step, 1, by 0.1, then 4/16; a: 3 runs.
            let want = [1.0 - 200.0 / 255.0, 0.3 + 0.5, 0.1 + 0.25, 0.3];
            assert!(px.iter().zip(want).all(|(&a, b)| near(a, b)), "{px:?}, want {want:?} (x 255)");
        }
    }

    /// Pyramids (docs/archive/projects/multipass-effects.md): a pass over levels
    /// writes each one, `ctx.level` and `ctx.levels` saying which, the
    /// grid halving; its target reads the level it came from, zero at the
    /// first; another pyramid reads at the level being written.
    #[test]
    fn a_pyramid_pass_writes_and_reads_its_levels() {
        let Some((device, queue)) = device_with(wgpu::Features::empty()) else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        const W: u32 = 16;
        const H: u32 = 8;
        let (_input, view) = filled(&device, &queue, W, H, TextureFormat::Rgba8Unorm, &[0, 0, 0, 255]);
        let src = r#"
const INFO = #{
    id: "6a3f9b2c-5d4e-4b7f-8c9a-3d2e1f0a5b6c",
    buffers: #{ p: #{ levels: 3 }, q: #{ levels: 3 } },
    passes: [
        #{ run: "down", to: "p", repeat: "levels" },
        #{ run: "up", to: "q", repeat: "levels", order: "up" },
        #{ run: "effect" },
    ],
};
// Each level its own value, the level before it read (zero at level 0,
// so the values do not stack): r 0.1 a level, b the grid's width / 64.
fn down(c, p, ctx) {
    let before = buffer("p", ctx.uv);
    rgba(0.1 * to_float(ctx.level + 1) + before.r * 0.0, before.r, to_float(ctx.width) / 64.0, 1.0)
}
// Coarsest first: this level's `p`, and `q` one coarser (zero at the top).
fn up(c, p, ctx) {
    let here = buffer("p", ctx.uv);
    let coarser = buffer("q", ctx.uv);
    rgba(here.r + coarser.r, 0.1 * to_float(ctx.levels), here.b, 1.0)
}
fn effect(c, p, ctx) { buffer("q", ctx.uv) }
"#;
        let name = crate::effects::custom::register(src, crate::provenance::Provenance::Local).unwrap_or_else(|e| panic!("{e}"));
        let out = run_colour(&device, &queue, &view, W, H, &[EffectInstance::new(&name)]);
        crate::effects::global_effect_registry_mut().remove_by_name(&name);
        let near = |a: u8, b: f32| (a as f32 - b * 255.0).abs() <= 1.0;
        // q0 = p0 + p1 + p2 = 0.1 + 0.2 + 0.3; 3 levels; level 0's width
        // 16 / 64.
        let want = [0.6, 0.3, 0.25, 1.0];
        for px in out.chunks(4) {
            assert!(px.iter().zip(want).all(|(&a, b)| near(a, b)), "{px:?}, want {want:?} (x 255)");
        }
    }

    /// A persistent buffer (docs/archive/projects/multipass-effects.md, T1-T3) on
    /// one chain run at a sequence of clocks: the next frame carries the
    /// history on, the same frame again re-reads it, a jump or a new epoch
    /// starts over, a still has none; `ctx.frame`, `ctx.time`, `ctx.dt`
    /// are the clock's.
    #[test]
    fn a_persistent_buffer_follows_the_clock() {
        let Some((device, queue)) = device_with(wgpu::Features::empty()) else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        const W: u32 = 8;
        const H: u32 = 4;
        let (_input, view) = filled(&device, &queue, W, H, TextureFormat::Rgba8Unorm, &[0, 0, 0, 255]);
        let src = r#"
const INFO = #{
    id: "7b4e0c3d-6e5f-4c8a-9d0b-4e3f2a1b6c7d",
    buffers: #{ acc: #{ persist: true } },
    passes: [#{ run: "keep", to: "acc" }, #{ run: "effect" }],
};
// Each frame adds 0.1 to what the last left.
fn keep(c, p, ctx) { buffer("acc", ctx.uv) + rgba(0.1, 0.0, 0.0, 0.0) }
fn effect(c, p, ctx) {
    let a = buffer("acc", ctx.uv);
    rgba(a.r, to_float(ctx.frame) * 0.1, ctx.time, ctx.dt)
}
"#;
        let name = crate::effects::custom::register(src, crate::provenance::Provenance::Local).unwrap_or_else(|e| panic!("{e}"));
        let effects = [EffectInstance::new(&name)];
        let mut chain = EffectChainRunner::new(&device, W, H);
        let mut run = |clock: EffectClock| -> [u8; 4] {
            chain.set_clock(clock);
            let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("temporal") });
            chain.reset_slots();
            assert!(chain.run_color_effects(&device, &queue, &mut encoder, &view, &effects));
            queue.submit(Some(encoder.finish()));
            assert!(pollster::block_on(scope.pop()).is_none(), "validation");
            let px = pollster::block_on(chain.read_color_output_pixels(&device, &queue)).expect("read").2;
            [px[0], px[1], px[2], px[3]]
        };
        let r = |v: f32| (v * 255.0).round() as u8;
        let near = |a: u8, b: u8| (a as i32 - b as i32).abs() <= 1;
        let mut check = |clock: EffectClock, acc: f32, what: &str| {
            let px = run(clock);
            let want = [r(acc), r(if clock.still { 0.0 } else { clock.frame as f32 * 0.1 }), r(clock.time), r(clock.dt)];
            assert!(px.iter().zip(want).all(|(a, b)| near(*a, b)), "{what}: {px:?}, want {want:?}");
        };
        check(EffectClock::frame(0, 0, 0.0, 0.0), 0.1, "the first frame");
        check(EffectClock::frame(0, 0, 0.0, 0.0), 0.1, "the first frame again");
        check(EffectClock::frame(0, 1, 0.25, 0.25), 0.2, "the next");
        check(EffectClock::frame(0, 2, 0.5, 0.25), 0.3, "the next");
        check(EffectClock::frame(0, 2, 0.5, 0.25), 0.3, "the same again");
        check(EffectClock::frame(0, 3, 0.75, 0.25), 0.4, "on from the same");
        check(EffectClock::frame(0, 6, 0.5, 0.25), 0.1, "a jump starts over");
        check(EffectClock::frame(0, 7, 0.75, 0.25), 0.2, "and carries on");
        check(EffectClock::frame(1, 8, 0.75, 0.25), 0.1, "a new epoch starts over");
        check(EffectClock::STILL, 0.1, "a still has no history");
        check(EffectClock::STILL, 0.1, "nor the next still");
        chain.destroy();
        crate::effects::global_effect_registry_mut().remove_by_name(&name);
    }

    /// The built-in Trails (a temporal effect), frame by frame in a video:
    /// where the picture was bright and is now dark, the trail fades by
    /// the persistence each thirtieth of a second; a still has none.
    #[test]
    fn trails_fade_frame_by_frame() {
        let Some((device, queue)) = device_with(wgpu::Features::empty()) else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        const W: u32 = 8;
        const H: u32 = 4;
        let (_white, white) = filled(&device, &queue, W, H, TextureFormat::Rgba8Unorm, &[255, 255, 255, 255]);
        let (_black, black) = filled(&device, &queue, W, H, TextureFormat::Rgba8Unorm, &[0, 0, 0, 255]);
        let effects = [EffectInstance::new("trails")];
        let mut chain = EffectChainRunner::new(&device, W, H);
        let mut run = |clock: EffectClock, view: &TextureView| -> u8 {
            chain.set_clock(clock);
            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("trails") });
            chain.reset_slots();
            assert!(chain.run_color_effects(&device, &queue, &mut encoder, view, &effects));
            queue.submit(Some(encoder.finish()));
            pollster::block_on(chain.read_color_output_pixels(&device, &queue)).expect("read").2[0]
        };
        // At 30 frames a second, each frame fades the trail by 0.85.
        assert_eq!(run(EffectClock::video(0, 30), &white), 255, "a bright frame");
        let one = run(EffectClock::video(1, 30), &black);
        let two = run(EffectClock::video(2, 30), &black);
        assert!((one as f32 - 255.0 * 0.85).abs() <= 2.0, "one frame on: {one}");
        assert!((two as f32 - 255.0 * 0.85 * 0.85).abs() <= 2.0, "two: {two}");
        // At 60, half a thirtieth: the square root of the fade a frame.
        run(EffectClock::frame(5, 0, 0.0, 0.0), &white);
        let half = run(EffectClock::frame(5, 1, 1.0 / 60.0, 1.0 / 60.0), &black);
        assert!((half as f32 - 255.0 * 0.85f32.sqrt()).abs() <= 2.0, "at 60 a second: {half}");
        assert_eq!(run(EffectClock::STILL, &black), 0, "a still has no trail");
        chain.destroy();
    }

    /// The built-in bloom (a pyramid effect): a white square on black
    /// glows into the black around it, the glow falling off with distance
    /// and reaching well past where a few taps could; a picture darker
    /// than the threshold is left as it was.
    #[test]
    fn bloom_glows_past_the_bright_parts_alone() {
        let Some((device, queue)) = device_with(wgpu::Features::empty()) else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        const W: u32 = 128;
        const H: u32 = 96;
        let data: Vec<u8> = (0..W * H)
            .flat_map(|i| {
                let (x, y) = (i % W, i / W);
                let on = (56..72).contains(&x) && (40..56).contains(&y);
                if on { [255, 255, 255, 255] } else { [0, 0, 0, 255] }
            })
            .collect();
        let (_input, view) = texture_of(&device, &queue, W, H, TextureFormat::Rgba8Unorm, &data);
        let out = run_colour(&device, &queue, &view, W, H, &[EffectInstance::new("bloom")]);
        let at = |x: u32, y: u32| out[((y * W + x) * 4) as usize];
        assert_eq!(at(64, 48), 255, "the square stays white");
        // Along the row through the square's middle, rightwards from its edge.
        let row: Vec<u8> = [73, 76, 80, 88, 100].iter().map(|&x| at(x, 48)).collect();
        assert!(row[0] > 40, "beside the square it glows: {row:?}");
        assert!(row.windows(2).all(|w| w[0] >= w[1]), "falling off with distance: {row:?}");
        assert!(row[3] > 0, "24 pixels away, still lit: {row:?}");
        assert!(at(2, 2) < row[4], "the far corner darker still");
        // Below the threshold and its soft knee (0.45 less 30%): nothing glows.
        let (_dim, dim_view) = filled(&device, &queue, W, H, TextureFormat::Rgba8Unorm, &[30, 50, 70, 255]);
        let out = run_colour(&device, &queue, &dim_view, W, H, &[EffectInstance::new("bloom")]);
        assert!(out.chunks(4).all(|px| px == [30, 50, 70, 255]), "a dim picture unchanged: {:?}", &out[..4]);
    }

    /// What a config reads of an effect: its parameters' definitions --
    /// name, kind, default, range, label and hint -- in order.
    fn values(i: &crate::effects::EffectInfo) -> &[crate::params::ParamDef] {
        &i.parameters
    }

    /// The D1 checks that need no pixels, for a built-in's twin: the
    /// registered built-in is the twin `source` compiles to, under the
    /// built-in's name, when it replaced the WGSL (`BUILTIN_TWINS`), and
    /// the WGSL when it waits (`PENDING_TWINS`); a config reads the same
    /// values of the twin as of the WGSL, in the same place in the
    /// pipeline. Registers the other of the two beside the built-in and
    /// gives the names, `(twin, WGSL)`, the one registered here second
    /// for a replaced one and first for a pending one -- [`TestPair::extra`]
    /// is which to remove; what differs goes in `failures`.
    fn reference_beside_twin(builtin: &str, source: &str, failures: &mut Vec<String>) -> TestPair {
        let mut twin = crate::effects::custom::compile(source, crate::provenance::Provenance::Builtin).unwrap_or_else(|e| panic!("{builtin}: {e}"));
        assert_eq!(twin.name, builtin, "a twin's id is its built-in's name");
        let mut wgsl = crate::effects::wgsl_builtins().into_iter().find(|i| i.name == builtin).expect("the built-in's WGSL");
        let replaced = crate::effects::custom::BUILTIN_TWINS.iter().any(|(n, _)| *n == builtin);
        {
            let registry = crate::effects::global_effect_registry();
            let registered = registry.get(builtin).expect("the built-in");
            let expected = if replaced { &twin } else { &wgsl };
            assert_eq!(registered.source.wgsl(), expected.source.wgsl(), "{builtin}: the built-in is {}", if replaced { "its twin" } else { "its WGSL" });
            assert!(registered.provenance.is_builtin(), "{builtin}: still a built-in");
        }
        let (a, b) = (values(&twin), values(&wgsl));
        if a.len() != b.len() {
            failures.push(format!("{builtin}: {} parameters, its WGSL {}", a.len(), b.len()));
        }
        for (t, w) in a.iter().zip(b) {
            if t != w {
                failures.push(format!("{builtin}: the twin's {t:?}
    the WGSL's  {w:?}"));
            }
        }
        if twin.category != wgsl.category {
            failures.push(format!("{builtin}: in the other pipeline"));
        }
        if replaced {
            let name = format!("{builtin}_wgsl_reference");
            wgsl.name = name.clone();
            crate::effects::global_effect_registry_mut().register(wgsl);
            TestPair { twin: builtin.to_string(), wgsl: name.clone(), extra: name }
        } else {
            let name = format!("{builtin}_rhai_twin");
            twin.name = name.clone();
            crate::effects::global_effect_registry_mut().register(twin);
            TestPair { twin: name.clone(), wgsl: builtin.to_string(), extra: name }
        }
    }

    /// A twin and its WGSL by the names they run under in a test, and the
    /// one the test registered.
    struct TestPair {
        twin: String,
        wgsl: String,
        extra: String,
    }

    /// The Rhai twins of the built-ins (assets/effects/*.rhai) and how
    /// each is driven: settings away from the defaults, blends among them.
    const TWINS: &[(&str, &str, &[&[(&str, f32)]])] = &[
        (
            "vignette",
            include_str!("../../assets/effects/vignette.rhai"),
            &[&[], &[("intensity", 1.0), ("radius", 0.2), ("softness", 0.9)], &[("intensity", 0.7), ("radius", 0.4), ("softness", 0.3), ("blend_mode", 2.0)], &[("intensity", 0.8), ("radius", 0.6), ("softness", 0.2), ("blend_mode", 9.0)]],
        ),
        (
            "chromatic_aberration",
            include_str!("../../assets/effects/chromatic_aberration.rhai"),
            &[&[], &[("amount", 9.0)], &[("radial", 0.0), ("amount", 6.0)], &[("amount", 14.0), ("intensity", 0.6), ("blend_mode", 3.0)]],
        ),
        (
            "sobel_edges",
            include_str!("../../assets/effects/sobel_edges.rhai"),
            &[&[], &[("threshold", 0.3), ("glow", 1.0), ("preserve_color", 0.0)], &[("threshold", 0.05), ("blend_mode", 0.0), ("intensity", 0.5)], &[("preserve_color", 1.0), ("blend_mode", 4.0)]],
        ),
        (
            "film_grain",
            include_str!("../../assets/effects/film_grain.rhai"),
            &[&[], &[("seed", 37.0)], &[("intensity", 0.6), ("blend_mode", 0.0)], &[("seed", 200.0), ("blend_mode", 3.0)]],
        ),
        (
            "hue_shift",
            include_str!("../../assets/effects/hue_shift.rhai"),
            &[&[], &[("offset", 90.0)], &[("offset", 200.0), ("intensity", 0.5)], &[("offset", 300.0), ("blend_mode", 9.0)]],
        ),
        (
            "plasma",
            include_str!("../../assets/effects/plasma.rhai"),
            &[&[], &[("time", 12.5)], &[("scale", 20.0), ("direction", 45.0), ("time", 3.0)], &[("speed", 3.0), ("time", 7.0), ("blend_mode", 4.0)]],
        ),
        (
            "tunnel",
            include_str!("../../assets/effects/tunnel.rhai"),
            &[&[], &[("time", 4.2), ("distortion", 0.5)], &[("speed", 2.0), ("rotation_speed", 1.5), ("time", 9.0)], &[("blend_mode", 3.0), ("intensity", 0.6), ("time", 1.0)]],
        ),
        (
            "kaleidoscope",
            include_str!("../../assets/effects/kaleidoscope.rhai"),
            &[&[], &[("segments", 9.5), ("rotation", 40.0)], &[("zoom", 2.2), ("square_mode", 0.0)], &[("edge_offset", 12.0), ("rotation", 200.0), ("blend_mode", 2.0)]],
        ),
        (
            "julia",
            include_str!("../../assets/effects/julia.rhai"),
            &[&[], &[("mode", 0.0), ("color_mode", 0.0)], &[("color_mode", 2.0), ("max_iter", 40.0), ("zoom", 2.5), ("center_x", -0.5)], &[("julia_cx", 0.3), ("julia_cy", -0.5), ("blend_mode", 3.0)]],
        ),
        (
            "worley_noise",
            include_str!("../../assets/effects/worley_noise.rhai"),
            &[&[], &[("mode", 1.0), ("color_mode", 0.0), ("time", 5.0)], &[("mode", 2.0), ("color_mode", 2.0), ("scale", 3.0)], &[("mode", 3.0), ("direction", 90.0), ("time", 20.0), ("blend_mode", 0.0)]],
        ),
        (
            "simplex_noise",
            include_str!("../../assets/effects/simplex_noise.rhai"),
            &[&[], &[("mode", 1.0), ("intensity", 0.8), ("time", 6.0)], &[("mode", 2.0), ("octaves", 2.0)], &[("octaves", 6.0), ("scale", 12.0), ("blend_mode", 0.0), ("direction", 10.0), ("time", 3.0)]],
        ),
        (
            "domain_warp",
            include_str!("../../assets/effects/domain_warp.rhai"),
            &[&[], &[("time", 5.0), ("octaves", 2.0)], &[("intensity", 0.4), ("scale", 6.0)], &[("octaves", 6.0), ("direction", 90.0), ("time", 2.0), ("blend_mode", 4.0)]],
        ),
        (
            "distance_field",
            include_str!("../../assets/effects/distance_field.rhai"),
            &[
                &[],
                &[("threshold", 0.6), ("mode", 1.0), ("band_count", 5.0)],
                &[("threshold", 0.5), ("mode", 2.0), ("intensity", 0.7), ("spread", 0.1)],
                &[("threshold", 0.7), ("spread", 0.6), ("intensity", 2.0)],
                // Nothing seeds: the image left alone.
                &[("threshold", 1.0)],
            ],
        ),
    ];

    /// Each built-in that is its Rhai twin renders the pixels of the WGSL it
    /// replaced, within a step of 8-bit rounding, at its defaults and the
    /// settings above, under the built-in's name with the same values
    /// ([`reference_beside_twin`]) -- the D1 checks a twin passes to
    /// replace its built-in, held now against a compiler change.
    #[test]
    fn the_rhai_twins_match_their_wgsl() {
        let Some((device, queue)) = device_with(wgpu::Features::empty()) else {
            eprintln!("no GPU adapter; skipping");
            return;
        };
        const W: u32 = 96;
        const H: u32 = 64;
        // A gradient, and squares of 8 pixels for edges to find.
        let data: Vec<u8> = (0..W * H)
            .flat_map(|i| {
                let (x, y) = (i % W, i / W);
                let square = if (x / 8 + y / 8) % 2 == 0 { 60 } else { 0 };
                [(x * 255 / W) as u8, (y * 255 / H) as u8, 100 + square, 255]
            })
            .collect();
        let (_input, view) = texture_of(&device, &queue, W, H, TextureFormat::Rgba8Unorm, &data);
        let mut failures = Vec::new();
        for (builtin, source, settings) in TWINS {
            let pair = reference_beside_twin(builtin, source, &mut failures);
            for set in settings.iter() {
                let instance = |name: &str| {
                    let mut e = EffectInstance::new(name);
                    for (k, v) in set.iter() {
                        e.set_param(k, *v);
                    }
                    e
                };
                let wgsl = run_colour(&device, &queue, &view, W, H, &[instance(&pair.wgsl)]);
                let rhai = run_colour(&device, &queue, &view, W, H, &[instance(&pair.twin)]);
                let diffs: Vec<i32> = wgsl.iter().zip(&rhai).map(|(a, b)| (*a as i32 - *b as i32).abs()).collect();
                let worst = diffs.iter().copied().max().unwrap_or(0);
                let off = diffs.iter().filter(|d| **d > 1).count();
                if worst > 1 {
                    failures.push(format!("{builtin} at {set:?}: off by {worst} ({off} of {} channels past 1)", diffs.len()));
                }
            }
            crate::effects::global_effect_registry_mut().remove_by_name(&pair.extra);
        }
        assert!(failures.is_empty(), "twins that differ from their WGSL:\n{}", failures.join("\n"));
    }

    /// The density effects' twins, as [`TWINS`] the colour ones'.
    const DENSITY_TWINS: &[(&str, &str, &[&[(&str, f32)]])] = &[
        (
            "density_blur",
            include_str!("../../assets/effects/density_blur.rhai"),
            &[&[], &[("radius", 8.0), ("threshold", 0.5)], &[("radius", 1.0)], &[("falloff", 0.1), ("threshold", 0.05), ("radius", 6.0)]],
        ),
        (
            "sharpen",
            include_str!("../../assets/effects/sharpen.rhai"),
            &[&[], &[("amount", 2.0), ("radius", 3.5)], &[("amount", 0.005)], &[("radius", 0.5), ("amount", 0.7)]],
        ),
        (
            "bilateral_blur",
            include_str!("../../assets/effects/bilateral_blur.rhai"),
            &[&[], &[("radius", 7.0), ("sigma_spatial", 6.0)], &[("sigma_range", 0.5), ("radius", 2.0)], &[("radius", 10.0), ("sigma_range", 0.05)]],
        ),
    ];

    /// Each density twin gives its WGSL's floats, to a part in a thousand
    /// of the larger: the accumulation's colours and hit counts, through
    /// sums of Gaussian taps the two sum in their own order.
    #[test]
    fn the_density_twins_match_their_wgsl() {
        let Some((device, queue)) = device_with(wgpu::Features::FLOAT32_FILTERABLE) else {
            eprintln!("no adapter with FLOAT32_FILTERABLE; skipping");
            return;
        };
        const W: u32 = 48;
        const H: u32 = 32;
        const MEAN: f32 = 400.0;
        // Colours in stripes and squares; hit counts from a tenth of the
        // mean to twenty times it, so the density blur's threshold divides
        // the image.
        let data: Vec<u8> = (0..W * H)
            .flat_map(|i| {
                let (x, y) = (i % W, i / W);
                let square = if (x / 6 + y / 6) % 2 == 0 { 0.4 } else { 0.0 };
                let hits = MEAN * (0.1 + 19.9 * ((x * 7 + y * 3) % 23) as f32 / 22.0);
                [x as f32 / W as f32, y as f32 / H as f32, 0.3 + square, hits]
            })
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let (_input, view) = texture_of(&device, &queue, W, H, TextureFormat::Rgba32Float, &data);
        let run = |effect: EffectInstance| {
            let mut chain = EffectChainRunner::new(&device, W, H);
            let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("density") });
            chain.reset_slots();
            assert!(chain.run_density_effects(&device, &queue, &mut encoder, &view, &[effect], MEAN), "the chain ran");
            queue.submit(Some(encoder.finish()));
            let out = chain.read_density_output_blocking(&device, &queue);
            chain.destroy();
            out
        };
        let mut failures = Vec::new();
        for (builtin, source, settings) in DENSITY_TWINS {
            let pair = reference_beside_twin(builtin, source, &mut failures);
            for set in settings.iter() {
                let instance = |name: &str| {
                    let mut e = EffectInstance::new(name);
                    for (k, v) in set.iter() {
                        e.set_param(k, *v);
                    }
                    e
                };
                let wgsl = run(instance(&pair.wgsl));
                let rhai = run(instance(&pair.twin));
                let mut worst = 0.0f32;
                for (a, b) in wgsl.iter().flatten().zip(rhai.iter().flatten()) {
                    worst = worst.max((a - b).abs() / a.abs().max(b.abs()).max(1.0));
                }
                if worst > 1e-3 {
                    failures.push(format!("{builtin} at {set:?}: off by {worst:e} of the larger"));
                }
            }
            crate::effects::global_effect_registry_mut().remove_by_name(&pair.extra);
        }
        assert!(failures.is_empty(), "density twins that differ from their WGSL:\n{}", failures.join("\n"));
    }

    /// D1's speed check: each built-in that is its twin against the WGSL
    /// it replaced, at its defaults, at 1080p and 4K, on one chain: the
    /// first run (the pipeline built), then nine rounds of a batch of
    /// about 20 ms each (so a small effect is timed, not its submission),
    /// the twin straight after the WGSL, and the median of the rounds'
    /// ratios (best-of-N moved with the GPU's clocks). A twin more than
    /// 5% slower fails; a pending one (`PENDING_TWINS`) is timed and
    /// reported, ready to replace its WGSL once it passes. A measurement
    /// on the GPU: run it alone, with the app closed --
    /// `cargo test --release --lib -- --ignored --nocapture the_twins_are_no_slower`.
    #[test]
    #[ignore = "a measurement on the GPU: run alone, with the app closed"]
    fn the_twins_are_no_slower() {
        let Some((device, queue)) = device_with(wgpu::Features::FLOAT32_FILTERABLE) else {
            eprintln!("no adapter with FLOAT32_FILTERABLE; skipping");
            return;
        };
        let wait = || {
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        };
        let mut failures = Vec::new();
        let mut still_pending = Vec::new();
        let mut ignored = Vec::new();
        println!("{:<22} {:>9} {:>10} {:>10} {:>7} {:>11} {:>11}", "effect", "size", "WGSL ms", "twin ms", "ratio", "WGSL 1st", "twin 1st");
        for (w, h) in [(1920u32, 1080u32), (3840, 2160)] {
            let colour: Vec<u8> = (0..w * h).flat_map(|i| [((i % w) * 255 / w) as u8, ((i / w) * 255 / h) as u8, 120, 255]).collect();
            let (_c, cview) = texture_of(&device, &queue, w, h, TextureFormat::Rgba8Unorm, &colour);
            drop(colour);
            let density: Vec<u8> = (0..w * h)
                .flat_map(|i| [(i % w) as f32 / w as f32, (i / w) as f32 / h as f32, 0.4, 50.0 + (i % 997) as f32])
                .flat_map(|v| v.to_le_bytes())
                .collect();
            let (_d, dview) = texture_of(&device, &queue, w, h, TextureFormat::Rgba32Float, &density);
            drop(density);
            let mut chain = EffectChainRunner::new(&device, w, h);
            for (name, source) in crate::effects::custom::all_twins() {
                let pending = crate::effects::custom::PENDING_TWINS.iter().any(|(n, _)| n == name);
                let pair = reference_beside_twin(name, source, &mut ignored);
                let is_density = crate::effects::global_effect_registry().get(name).is_some_and(|i| i.category == crate::effects::EffectCategory::Density);
                let mut batch = |effect: &str, n: u32| {
                    let instance = [EffectInstance::new(effect)];
                    let start = web_time::Instant::now();
                    let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("timing") });
                    for _ in 0..n {
                        chain.reset_slots();
                        let ran = if is_density {
                            chain.run_density_effects(&device, &queue, &mut encoder, &dview, &instance, 400.0)
                        } else {
                            chain.run_color_effects(&device, &queue, &mut encoder, &cview, &instance)
                        };
                        assert!(ran, "{effect} ran");
                    }
                    queue.submit(Some(encoder.finish()));
                    wait();
                    start.elapsed().as_secs_f64() * 1000.0 / f64::from(n)
                };
                let (first_ref, first_twin) = (batch(&pair.wgsl, 1), batch(&pair.twin, 1));
                let estimate = batch(&pair.wgsl, 5).min(batch(&pair.twin, 5));
                let runs = ((20.0 / estimate) as u32).clamp(5, 400);
                // Each round the twin straight after the WGSL: the median of
                // the rounds' ratios, which a clock change between rounds
                // does not move.
                let (mut best_ref, mut best_twin) = (f64::MAX, f64::MAX);
                let mut ratios = Vec::new();
                for _ in 0..9 {
                    let r = batch(&pair.wgsl, runs);
                    let t = batch(&pair.twin, runs);
                    best_ref = best_ref.min(r);
                    best_twin = best_twin.min(t);
                    ratios.push(t / r);
                }
                ratios.sort_by(|a, b| a.total_cmp(b));
                let ratio = ratios[ratios.len() / 2];
                let mark = if pending { "  (pending)" } else { "" };
                println!("{name:<22} {:>9} {best_ref:>10.3} {best_twin:>10.3} {ratio:>7.3} {first_ref:>11.1} {first_twin:>11.1}{mark}", format!("{w}x{h}"));
                if ratio > 1.05 && !pending {
                    failures.push(format!("{name} at {w}x{h}: {best_twin:.3} ms against {best_ref:.3} ({ratio:.2}x)"));
                }
                if ratio > 1.05 && pending {
                    still_pending.push(format!("{name} at {w}x{h}: {ratio:.2}x"));
                }
                crate::effects::global_effect_registry_mut().remove_by_name(&pair.extra);
            }
            chain.destroy();
        }
        assert!(ignored.is_empty(), "{ignored:?}");
        println!("pending twins still slower: {still_pending:?} (none: ready to replace their WGSL)");
        assert!(failures.is_empty(), "twins slower than their WGSL:\n{}", failures.join("\n"));
    }

    /// A tool for a twin's speed (D1): times the WGSL files in
    /// `$EFFECT_VARIANTS` (default `output/effect-variants`), one folder
    /// per effect named as the built-in, each `*.wgsl` a module the chain
    /// runs with the built-in's parameters, against the built-in's WGSL
    /// and its twin, interleaved, the best of `$EFFECT_VARIANTS_ROUNDS`
    /// (default 7) ~20 ms batches at `$EFFECT_VARIANTS_SIZE` (default
    /// `3840x2160`). Writes `wgsl.wgsl`
    /// and `twin.wgsl` beside the variants first, to start from. Files,
    /// not code, so an experiment needs no rebuild.
    #[test]
    #[ignore = "a tool: a measurement on the GPU, run alone with the app closed"]
    fn time_effect_variants() {
        let Some((device, queue)) = device_with(wgpu::Features::FLOAT32_FILTERABLE) else {
            eprintln!("no adapter with FLOAT32_FILTERABLE; skipping");
            return;
        };
        let root = std::path::PathBuf::from(std::env::var("EFFECT_VARIANTS").unwrap_or_else(|_| "output/effect-variants".into()));
        let size = std::env::var("EFFECT_VARIANTS_SIZE").unwrap_or_else(|_| "3840x2160".into());
        let (w, h) = size.split_once('x').and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?))).expect("WxH");
        let rounds: usize = std::env::var("EFFECT_VARIANTS_ROUNDS").ok().and_then(|r| r.parse().ok()).unwrap_or(7);
        let wait = || {
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
        };
        let colour: Vec<u8> = (0..w * h).flat_map(|i| [((i % w) * 255 / w) as u8, ((i / w) * 255 / h) as u8, 120, 255]).collect();
        let (_c, cview) = texture_of(&device, &queue, w, h, TextureFormat::Rgba8Unorm, &colour);
        drop(colour);
        let Ok(dirs) = std::fs::read_dir(&root) else {
            eprintln!("no {}: make a folder per effect there", root.display());
            return;
        };
        let dirs: Vec<_> = dirs.flatten().filter(|d| d.path().is_dir()).collect();
        // The density input only when a density effect is timed: at large
        // sizes it is the biggest texture here.
        let any_density = dirs.iter().any(|d| {
            crate::effects::global_effect_registry().get(&d.file_name().to_string_lossy()).is_some_and(|i| i.category == crate::effects::EffectCategory::Density)
        });
        let (dw, dh) = if any_density { (w, h) } else { (1, 1) };
        let density: Vec<u8> = (0..dw * dh)
            .flat_map(|i| [(i % dw) as f32 / dw as f32, (i / dw) as f32 / dh as f32, 0.4, 50.0 + (i % 997) as f32])
            .flat_map(|v| v.to_le_bytes())
            .collect();
        let (_d, dview) = texture_of(&device, &queue, dw, dh, TextureFormat::Rgba32Float, &density);
        drop(density);
        let mut chain = EffectChainRunner::new(&device, w, h);
        let mut registered = Vec::new();
        for dir in dirs {
            let effect = dir.file_name().to_string_lossy().to_string();
            let Some(builtin) = crate::effects::global_effect_registry().get(&effect).cloned() else {
                eprintln!("{effect}: not a built-in effect");
                continue;
            };
            let wgsl = crate::effects::wgsl_builtins().into_iter().find(|i| i.name == effect).expect("its WGSL");
            let twin = crate::effects::custom::all_twins()
                .find(|(n, _)| *n == effect)
                .map(|(_, s)| crate::effects::custom::compile(s, crate::provenance::Provenance::Builtin).expect("compiles"));
            let _ = std::fs::write(dir.path().join("wgsl.wgsl"), wgsl.source.wgsl());
            if let Some(t) = &twin {
                let _ = std::fs::write(dir.path().join("twin.wgsl"), t.source.wgsl());
            }
            let mut files: Vec<_> = std::fs::read_dir(dir.path()).into_iter().flatten().flatten().map(|f| f.path()).filter(|p| p.extension().is_some_and(|e| e == "wgsl")).collect();
            files.sort();
            let mut names = Vec::new();
            for file in &files {
                let stem = file.file_stem().unwrap().to_string_lossy().to_string();
                let mut info = builtin.clone();
                info.name = format!("{effect}__{stem}");
                info.source = crate::effects::EffectSource::Owned(std::fs::read_to_string(file).expect("reads"));
                crate::effects::global_effect_registry_mut().register(info.clone());
                registered.push(info.name.clone());
                names.push((stem, info.name));
            }
            // `wgsl` first: the ratios are to it.
            names.sort_by_key(|(s, _)| (s != "wgsl", s != "twin", s.clone()));
            let density = builtin.category == crate::effects::EffectCategory::Density;
            let mut batch = |effect: &str, n: u32| {
                let instance = [EffectInstance::new(effect)];
                let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);
                let start = web_time::Instant::now();
                let mut encoder = device.create_command_encoder(&CommandEncoderDescriptor { label: Some("variants") });
                for _ in 0..n {
                    chain.reset_slots();
                    let ran = if density {
                        chain.run_density_effects(&device, &queue, &mut encoder, &dview, &instance, 400.0)
                    } else {
                        chain.run_color_effects(&device, &queue, &mut encoder, &cview, &instance)
                    };
                    assert!(ran, "{effect} ran");
                }
                queue.submit(Some(encoder.finish()));
                wait();
                let t = start.elapsed().as_secs_f64() * 1000.0 / f64::from(n);
                pollster::block_on(scope.pop()).map_or(t, |e| {
                    eprintln!("{effect}: {e}");
                    f64::NAN
                })
            };
            let runs: Vec<u32> = names.iter().map(|(_, n)| {
                batch(n, 1);
                ((20.0 / batch(n, 5)) as u32).clamp(5, 400)
            }).collect();
            // Each variant timed straight after the WGSL, round by round: the
            // median of those ratios, which a clock change between rounds
            // does not move, beside the best time.
            let mut best = vec![f64::MAX; names.len()];
            let mut ratios = vec![Vec::new(); names.len()];
            for _ in 0..rounds {
                for k in 1..names.len() {
                    let base = batch(&names[0].1, runs[0]);
                    let t = batch(&names[k].1, runs[k]);
                    best[0] = best[0].min(base);
                    best[k] = best[k].min(t);
                    ratios[k].push(t / base);
                }
            }
            println!("{effect} at {w}x{h}: {:<28} {:>8} {:>8} {:>16}", "", "best ms", "median", "quartiles");
            for (k, (stem, _)) in names.iter().enumerate() {
                let r = &mut ratios[k];
                r.sort_by(|a, b| a.total_cmp(b));
                let at = |q: f64| r.get(((r.len() as f64 - 1.0) * q).round() as usize).copied().unwrap_or(1.0);
                println!("  {stem:<28} {:>8.3} {:>8.3}   {:.3}-{:.3}", best[k], at(0.5), at(0.25), at(0.75));
            }
        }
        let mut registry = crate::effects::global_effect_registry_mut();
        for n in registered {
            registry.remove_by_name(&n);
        }
        drop(registry);
        chain.destroy();
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
        let needed = 3 * (crate::effects::passes::log2_passes(W, H) as usize + 2) + 1;
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
