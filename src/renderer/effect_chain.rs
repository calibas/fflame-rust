//! Effect Chain Runner
//!
//! Executes shader effects in sequence using ping-pong textures.
//! - Density effects: Run on Rgba16Float before tonemap
//! - Color effects: Run on Rgba8Unorm after tonemap

use std::collections::HashMap;
use egui_wgpu::wgpu;
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindGroupLayout, BindGroupLayoutDescriptor,
    BindGroupLayoutEntry, BindingResource, BindingType, BlendState, Buffer, BufferBindingType,
    BufferDescriptor, BufferUsages, Color, ColorTargetState, ColorWrites, CommandEncoder,
    Device, Extent3d, FilterMode, FragmentState, LoadOp, MultisampleState, Operations,
    PipelineLayoutDescriptor, PrimitiveState, Queue, RenderPassColorAttachment,
    RenderPassDescriptor, RenderPipeline, RenderPipelineDescriptor, Sampler, SamplerBindingType,
    SamplerDescriptor, ShaderModuleDescriptor, ShaderSource, ShaderStages, StoreOp, Texture,
    TextureDescriptor, TextureDimension, TextureFormat, TextureSampleType, TextureUsages,
    TextureView, TextureViewDescriptor, TextureViewDimension, VertexState,
};

use crate::effects::{global_effect_registry, EffectCategory, EffectInstance};

/// Maximum number of effect parameters per effect
const MAX_EFFECT_PARAMS: usize = 16;

/// GPU uniform buffer for effect parameters
#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct EffectParams {
    /// Effect parameters (up to 16 floats)
    params: [f32; MAX_EFFECT_PARAMS],
    /// Texture dimensions
    width: u32,
    height: u32,
    /// Time for animated effects (in seconds)
    time: f32,
    /// Padding for alignment
    _padding: f32,
}

/// A compiled effect pipeline ready for execution
struct CompiledEffect {
    /// Name of the effect
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
                usage: TextureUsages::TEXTURE_BINDING | TextureUsages::RENDER_ATTACHMENT,
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
}

/// Effect chain runner for executing post-processing effects
pub struct EffectChainRunner {
    /// Compiled effect pipelines by name
    compiled_effects: HashMap<String, CompiledEffect>,
    /// Ping-pong textures for density effects (Rgba16Float)
    density_textures: Option<PingPongTextures>,
    /// Ping-pong textures for color effects (Rgba8Unorm)
    color_textures: Option<PingPongTextures>,
    /// Uniform buffer for effect parameters
    params_buffer: Buffer,
    /// Linear sampler for texture sampling
    sampler: Sampler,
    /// Current texture dimensions
    width: u32,
    height: u32,
    /// Time accumulator for animated effects
    time: f32,
}

impl EffectChainRunner {
    /// Create a new effect chain runner
    pub fn new(device: &Device, width: u32, height: u32) -> Self {
        let params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Effect Params Buffer"),
            size: std::mem::size_of::<EffectParams>() as u64,
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
            sampler,
            width,
            height,
            time: 0.0,
        }
    }

    /// Update time for animated effects
    pub fn update_time(&mut self, delta_seconds: f32) {
        self.time += delta_seconds;
    }

    /// Resize textures if dimensions changed
    pub fn resize(&mut self, device: &Device, width: u32, height: u32) {
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
            return;
        }

        let registry = global_effect_registry();
        let effect_info = match registry.get(effect_name) {
            Some(info) => info,
            None => {
                log::warn!("Unknown effect: {}", effect_name);
                return;
            }
        };

        // Load shader source
        let shader_path = format!("shaders/{}", effect_info.shader_path);
        let shader_source = match std::fs::read_to_string(&shader_path) {
            Ok(source) => source,
            Err(e) => {
                log::error!("Failed to load effect shader {}: {}", shader_path, e);
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
        _input_view: &TextureView,
        effects: &[EffectInstance],
    ) -> bool {
        let enabled_effects: Vec<_> = effects.iter().filter(|e| e.enabled).collect();
        if enabled_effects.is_empty() {
            return false;
        }

        // First, ensure all effects are compiled (before taking texture borrow)
        self.compile_effects(device, effects, EffectCategory::Density);
        self.ensure_textures(device, EffectCategory::Density);

        // Extract data needed for effect execution
        let width = self.width;
        let height = self.height;
        let time = self.time;

        // Now run effects
        if let Some(textures) = self.density_textures.as_mut() {
            // Note: copy_texture_to_view is a placeholder - input texture handling TBD
            for effect in enabled_effects {
                Self::run_single_effect_impl(
                    device,
                    queue,
                    encoder,
                    &effect.effect_type,
                    effect,
                    textures,
                    &self.compiled_effects,
                    &self.params_buffer,
                    &self.sampler,
                    width,
                    height,
                    time,
                );
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

        // First, ensure all effects are compiled (before taking texture borrow)
        self.compile_effects(device, effects, EffectCategory::Color);
        self.ensure_textures(device, EffectCategory::Color);

        // Extract data needed for effect execution
        let width = self.width;
        let height = self.height;
        let time = self.time;

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
                    &self.sampler,
                    width,
                    height,
                    time,
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

    /// Run a single effect (static helper to avoid borrow issues)
    fn run_single_effect_impl(
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        effect_name: &str,
        effect: &EffectInstance,
        textures: &mut PingPongTextures,
        compiled_effects: &HashMap<String, CompiledEffect>,
        params_buffer: &Buffer,
        sampler: &Sampler,
        width: u32,
        height: u32,
        time: f32,
    ) {
        let compiled = match compiled_effects.get(effect_name) {
            Some(c) => c,
            None => return,
        };

        // Update params buffer
        let mut params = EffectParams {
            params: [0.0; MAX_EFFECT_PARAMS],
            width,
            height,
            time,
            _padding: 0.0,
        };

        // Fill in effect parameters
        let registry = global_effect_registry();
        if let Some(info) = registry.get(effect_name) {
            for (i, param_def) in info.parameters.iter().enumerate() {
                if i < MAX_EFFECT_PARAMS {
                    params.params[i] = effect.get_param(&param_def.name);
                }
            }
        }

        queue.write_buffer(params_buffer, 0, bytemuck::bytes_of(&params));

        // Create bind group for this pass
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some(&format!("{} Bind Group", effect_name)),
            layout: &compiled.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(textures.read_view()),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(sampler),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });

        // Run the effect
        {
            let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
                label: Some(&format!("{} Pass", effect_name)),
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: textures.write_view(),
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

        // Swap ping-pong textures
        textures.swap();
    }

    /// Run a single effect with explicit input/output views
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
        sampler: &Sampler,
        width: u32,
        height: u32,
        time: f32,
    ) {
        let compiled = match compiled_effects.get(effect_name) {
            Some(c) => c,
            None => return,
        };

        // Update params buffer
        let mut params = EffectParams {
            params: [0.0; MAX_EFFECT_PARAMS],
            width,
            height,
            time,
            _padding: 0.0,
        };

        // Fill in effect parameters
        let registry = global_effect_registry();
        if let Some(info) = registry.get(effect_name) {
            for (i, param_def) in info.parameters.iter().enumerate() {
                if i < MAX_EFFECT_PARAMS {
                    params.params[i] = effect.get_param(&param_def.name);
                }
            }
        }

        queue.write_buffer(params_buffer, 0, bytemuck::bytes_of(&params));

        // Create bind group for this pass
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
                    resource: params_buffer.as_entire_binding(),
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
}
