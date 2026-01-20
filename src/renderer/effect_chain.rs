//! Effect Chain Runner
//!
//! Executes shader effects in sequence using ping-pong textures.
//! - Density effects: Run on Rgba16Float before tonemap
//! - Color effects: Run on Rgba8Unorm after tonemap

use std::collections::HashMap;
use egui_wgpu::wgpu;

// ============================================================================
// WASM: Embed effect shaders at compile time (no filesystem access)
// Desktop: Load from filesystem at runtime (allows hot-reloading)
// ============================================================================

/// Embedded effect shaders for WASM builds
#[cfg(target_arch = "wasm32")]
mod embedded_shaders {
    // Common includes
    pub const BLEND_MODES: &str = include_str!("../../shaders/effects/common/blend_modes.wgsl");

    // Color effects
    pub const CHROMATIC_ABERRATION: &str = include_str!("../../shaders/effects/color/chromatic_aberration.wgsl");
    pub const DOMAIN_WARP: &str = include_str!("../../shaders/effects/color/domain_warp.wgsl");
    pub const FILM_GRAIN: &str = include_str!("../../shaders/effects/color/film_grain.wgsl");
    pub const HUE_CYCLE: &str = include_str!("../../shaders/effects/color/hue_cycle.wgsl");
    pub const KALEIDOSCOPE: &str = include_str!("../../shaders/effects/color/kaleidoscope.wgsl");
    pub const PLASMA: &str = include_str!("../../shaders/effects/color/plasma.wgsl");
    pub const SIMPLEX_NOISE: &str = include_str!("../../shaders/effects/color/simplex_noise.wgsl");
    pub const SOBEL_EDGES: &str = include_str!("../../shaders/effects/color/sobel_edges.wgsl");
    pub const TUNNEL: &str = include_str!("../../shaders/effects/color/tunnel.wgsl");
    pub const VIGNETTE: &str = include_str!("../../shaders/effects/color/vignette.wgsl");
    pub const WORLEY_NOISE: &str = include_str!("../../shaders/effects/color/worley_noise.wgsl");

    // Density effects
    pub const BILATERAL_BLUR: &str = include_str!("../../shaders/effects/density/bilateral_blur.wgsl");
    pub const DENSITY_BLUR: &str = include_str!("../../shaders/effects/density/density_blur.wgsl");
    pub const SHARPEN: &str = include_str!("../../shaders/effects/density/sharpen.wgsl");
}

/// Load common include file
fn load_blend_modes() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        embedded_shaders::BLEND_MODES.to_string()
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        std::fs::read_to_string("shaders/effects/common/blend_modes.wgsl")
            .unwrap_or_else(|e| {
                log::error!("Failed to load blend_modes.wgsl: {}", e);
                String::new()
            })
    }
}

/// Process shader includes by replacing markers with include content
fn process_shader_includes(source: String) -> String {
    const BLEND_MODES_MARKER: &str = "// INCLUDE_BLEND_MODES";

    if source.contains(BLEND_MODES_MARKER) {
        let blend_modes = load_blend_modes();
        source.replace(BLEND_MODES_MARKER, &blend_modes)
    } else {
        source
    }
}

/// Load effect shader source by path
/// - WASM: Returns embedded shader source
/// - Desktop: Loads from filesystem
/// - Processes include markers (e.g., // INCLUDE_BLEND_MODES)
fn load_effect_shader(shader_path: &str) -> Result<String, String> {
    let source = {
        #[cfg(target_arch = "wasm32")]
        {
            // Map shader path to embedded source
            match shader_path {
                "effects/color/chromatic_aberration.wgsl" => Ok(embedded_shaders::CHROMATIC_ABERRATION.to_string()),
                "effects/color/domain_warp.wgsl" => Ok(embedded_shaders::DOMAIN_WARP.to_string()),
                "effects/color/film_grain.wgsl" => Ok(embedded_shaders::FILM_GRAIN.to_string()),
                "effects/color/hue_cycle.wgsl" => Ok(embedded_shaders::HUE_CYCLE.to_string()),
                "effects/color/kaleidoscope.wgsl" => Ok(embedded_shaders::KALEIDOSCOPE.to_string()),
                "effects/color/plasma.wgsl" => Ok(embedded_shaders::PLASMA.to_string()),
                "effects/color/simplex_noise.wgsl" => Ok(embedded_shaders::SIMPLEX_NOISE.to_string()),
                "effects/color/sobel_edges.wgsl" => Ok(embedded_shaders::SOBEL_EDGES.to_string()),
                "effects/color/tunnel.wgsl" => Ok(embedded_shaders::TUNNEL.to_string()),
                "effects/color/vignette.wgsl" => Ok(embedded_shaders::VIGNETTE.to_string()),
                "effects/color/worley_noise.wgsl" => Ok(embedded_shaders::WORLEY_NOISE.to_string()),
                "effects/density/bilateral_blur.wgsl" => Ok(embedded_shaders::BILATERAL_BLUR.to_string()),
                "effects/density/density_blur.wgsl" => Ok(embedded_shaders::DENSITY_BLUR.to_string()),
                "effects/density/sharpen.wgsl" => Ok(embedded_shaders::SHARPEN.to_string()),
                _ => Err(format!("Unknown effect shader: {}", shader_path)),
            }
        }

        #[cfg(not(target_arch = "wasm32"))]
        {
            let full_path = format!("shaders/{}", shader_path);
            std::fs::read_to_string(&full_path)
                .map_err(|e| format!("Failed to load effect shader {}: {}", full_path, e))
        }
    };

    // Process includes
    source.map(process_shader_includes)
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

/// Maximum number of effect parameters per effect
const MAX_EFFECT_PARAMS: usize = 16;

/// Maximum number of effect slots in the shared params buffer
/// Supports up to 32 effects running in a single frame
const MAX_EFFECT_SLOTS: usize = 32;

/// Alignment for uniform buffer dynamic offsets (256 bytes is typical minimum)
const UNIFORM_BUFFER_OFFSET_ALIGNMENT: u64 = 256;

/// GPU uniform buffer for effect parameters
/// Uses [[f32; 4]; 4] layout to match WGSL array<vec4<f32>, 4> for uniform alignment
/// Total size: 80 bytes (params=64 + width=4 + height=4 + padding=8 for 16-byte alignment)
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct EffectParams {
    /// Effect parameters (up to 16 floats, packed as 4 vec4s for alignment)
    params: [[f32; 4]; 4],
    /// Texture dimensions
    width: u32,
    height: u32,
    /// Padding to align struct to 16 bytes (WGSL uniform buffer requirement)
    _padding: [f32; 2],
}

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
    /// Ping-pong textures for density effects (Rgba16Float)
    density_textures: Option<PingPongTextures>,
    /// Ping-pong textures for color effects (Rgba8Unorm)
    color_textures: Option<PingPongTextures>,
    /// Shared params buffer with indexed slots for all effects
    /// Each effect writes to slot_index * UNIFORM_BUFFER_OFFSET_ALIGNMENT
    /// This allows multiple effects to have different params in a single submit
    params_buffer: Buffer,
    /// Current slot index for params buffer (reset each frame)
    current_slot: usize,
    /// Linear sampler for texture sampling
    sampler: Sampler,
    /// Current texture dimensions
    width: u32,
    height: u32,
}

impl EffectChainRunner {
    /// Create a new effect chain runner
    pub fn new(device: &Device, width: u32, height: u32) -> Self {
        // Single shared params buffer with slots for all effects
        // Each slot is aligned to UNIFORM_BUFFER_OFFSET_ALIGNMENT (256 bytes)
        // This allows multiple effects to have independent params in a single submit
        let buffer_size = MAX_EFFECT_SLOTS as u64 * UNIFORM_BUFFER_OFFSET_ALIGNMENT;
        let params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Effect Params Buffer (Indexed)"),
            size: buffer_size,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

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
            current_slot: 0,
            sampler,
            width,
            height,
        }
    }

    /// Reset slot counter for new frame
    /// Call this at the start of each frame before running effects
    pub fn reset_slots(&mut self) {
        self.current_slot = 0;
    }

    /// Allocate a slot and return its buffer offset
    fn allocate_slot(&mut self) -> u64 {
        let slot = self.current_slot;
        self.current_slot += 1;
        if self.current_slot > MAX_EFFECT_SLOTS {
            log::warn!("Effect slot overflow! Max {} effects per frame", MAX_EFFECT_SLOTS);
            self.current_slot = MAX_EFFECT_SLOTS; // Clamp to avoid panic
        }
        slot as u64 * UNIFORM_BUFFER_OFFSET_ALIGNMENT
    }

    /// Resize textures if dimensions changed
    pub fn resize(&mut self, _device: &Device, width: u32, height: u32) {
        if self.width != width || self.height != height {
            self.width = width;
            self.height = height;
            // Textures will be recreated on next use
            self.density_textures = None;
            self.color_textures = None;
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
                        TextureFormat::Rgba16Float,
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
        if self.compiled_effects.contains_key(effect_name) {
            log::debug!("Effect {} already compiled", effect_name);
            return;
        }
        log::info!("Compiling effect shader: {}", effect_name);

        let registry = global_effect_registry();
        let effect_info = match registry.get(effect_name) {
            Some(info) => info,
            None => {
                log::warn!("Unknown effect: {}", effect_name);
                return;
            }
        };

        // Load shader source (embedded for WASM, filesystem for desktop)
        let shader_source = match load_effect_shader(&effect_info.shader_path) {
            Ok(source) => source,
            Err(e) => {
                log::error!("{}", e);
                return;
            }
        };

        // Create shader module
        let shader_module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some(&format!("{} Shader", effect_name)),
            source: ShaderSource::Wgsl(shader_source.into()),
        });

        // Determine texture format based on category
        let texture_format = match category {
            EffectCategory::Density => TextureFormat::Rgba16Float,
            EffectCategory::Color => TextureFormat::Rgba8Unorm,
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
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
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
                    blend: Some(BlendState::REPLACE),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: PrimitiveState::default(),
            depth_stencil: None,
            multisample: MultisampleState::default(),
            multiview: None,
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
    ) -> bool {
        let enabled_effects: Vec<_> = effects.iter().filter(|e| e.enabled).collect();
        if enabled_effects.is_empty() {
            return false;
        }
        log::info!("Running {} density effects", enabled_effects.len());

        // First, ensure all effects are compiled (before taking texture borrow)
        self.compile_effects(device, effects, EffectCategory::Density);
        self.ensure_textures(device, EffectCategory::Density);

        // Pre-allocate slots for all effects (before borrowing textures)
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
        log::info!("Running {} color effects", enabled_effects.len());

        // First, ensure all effects are compiled (before taking texture borrow)
        self.compile_effects(device, effects, EffectCategory::Color);
        self.ensure_textures(device, EffectCategory::Color);

        // Pre-allocate slots for all effects (before borrowing textures)
        let slot_offsets: Vec<u64> = enabled_effects.iter().map(|_| self.allocate_slot()).collect();

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
                );

                // Swap ping-pong textures for next effect
                textures.swap();
            }
            return true;
        }
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
            params: [[0.0; 4]; 4],
            width,
            height,
            _padding: [0.0; 2],
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
