use egui_wgpu::wgpu::*;
use crate::shader_cache::ShaderCache;
use crate::scene::transforms::Flame;

pub struct FlamePipelines {
    pub shader_cache: ShaderCache,
    pub compute_bind_group_layout: BindGroupLayout,
    pub accumulate_pipeline: ComputePipeline,
    pub accumulate_bind_group_layout: BindGroupLayout,
    pub tonemap_pipeline: RenderPipeline,
    pub tonemap_bind_group_layout: BindGroupLayout,
}

impl FlamePipelines {
    pub fn new(device: &Device, _surface_format: TextureFormat, flame: &Flame) -> Self {
        // Load non-trajectory shaders (these don't need dynamic compilation)
        let accumulate_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Accumulate Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/accumulate.wgsl").into()),
        });

        let tonemap_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Tonemap Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/tonemap.wgsl").into()),
        });

        // Create bind group layouts
        let compute_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Compute Bind Group Layout"),
            entries: &[
                // Transform buffer (storage)
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Params buffer (uniform)
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Histogram buffer (storage, read-write for atomics)
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Palette texture (2D with height=1)
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Palette sampler
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                // Variation parameters buffer (storage)
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Iteration count buffer (storage, read-write for atomics)
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
                // Path buffer (storage, read-write for PathMap color mode)
                BindGroupLayoutEntry {
                    binding: 7,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Path filter buffer (storage, read-only for blocking transform sequences)
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
            ],
        });

        let tonemap_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Tonemap Bind Group Layout"),
            entries: &[
                // Accumulation texture (sampled)
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
                // Sampler
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                // Tonemap params (uniform)
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
                // Curve LUT texture (2D with height=1, sampled)
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Curve LUT sampler
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
                // Path buffer (storage, read-only for PathMap color mode visualization)
                BindGroupLayoutEntry {
                    binding: 5,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Palette texture (for gradient-based PathMap styles)
                BindGroupLayoutEntry {
                    binding: 6,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Palette sampler
                BindGroupLayoutEntry {
                    binding: 7,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // Create shader cache with dynamic trajectory pipelines
        let shader_cache = ShaderCache::new(device, flame, &compute_bind_group_layout);

        // Create accumulation bind group layout
        let accumulate_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Accumulate Bind Group Layout"),
            entries: &[
                // Previous accumulation (sampled texture)
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Histogram buffer (storage, read-only)
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Output texture (storage, write)
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba16Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
                // Params uniform
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Iteration count buffer (storage, read-only)
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
            ],
        });

        let accumulate_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Accumulate Pipeline Layout"),
            bind_group_layouts: &[&accumulate_bind_group_layout],
            push_constant_ranges: &[],
        });

        let accumulate_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Accumulate Compute Pipeline"),
            layout: Some(&accumulate_pipeline_layout),
            module: &accumulate_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Create tonemap render pipeline
        let tonemap_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Tonemap Pipeline Layout"),
            bind_group_layouts: &[&tonemap_bind_group_layout],
            push_constant_ranges: &[],
        });

        let tonemap_pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("Tonemap Pipeline"),
            layout: Some(&tonemap_pipeline_layout),
            vertex: VertexState {
                module: &tonemap_shader,
                entry_point: Some("vs_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            primitive: PrimitiveState {
                topology: PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            fragment: Some(FragmentState {
                module: &tonemap_shader,
                entry_point: Some("fs_main"),
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Rgba8Unorm, // Use Rgba8Unorm for egui compatibility
                    blend: None, // No blending - shader does color mixing internally
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        });

        Self {
            shader_cache,
            compute_bind_group_layout,
            accumulate_pipeline,
            accumulate_bind_group_layout,
            tonemap_pipeline,
            tonemap_bind_group_layout,
        }
    }

    /// Ensure shaders are up-to-date with current flame configuration
    /// Returns true if shaders were recompiled
    pub fn ensure_shaders_current(&mut self, device: &Device, flame: &Flame) -> bool {
        self.shader_cache.ensure_current(device, &self.compute_bind_group_layout, flame)
    }

    /// Ensure shaders are up-to-date with current flame configuration and path features state
    /// Returns true if shaders were recompiled
    pub fn ensure_shaders_current_with_path_features(
        &mut self,
        device: &Device,
        flame: &Flame,
        path_features_enabled: bool,
    ) -> bool {
        self.shader_cache.ensure_current_with_path_features(
            device,
            &self.compute_bind_group_layout,
            flame,
            path_features_enabled,
        )
    }

    /// Get current path_features_enabled state from shader cache
    pub fn path_features_enabled(&self) -> bool {
        self.shader_cache.path_features_enabled()
    }

    /// Get the appropriate compute pipeline for the current render mode
    pub fn get_trajectory_pipeline(&self, render_mode: crate::scene::transforms::RenderMode) -> &ComputePipeline {
        match render_mode {
            crate::scene::transforms::RenderMode::TwoD => self.shader_cache.pipeline_2d(),
            crate::scene::transforms::RenderMode::ThreeD => self.shader_cache.pipeline_3d(),
        }
    }

    /// Create bind group for compute pass
    pub fn create_compute_bind_group(
        &self,
        device: &Device,
        buffers: &super::buffers::FlameBuffers,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: &self.compute_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: buffers.transform_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: buffers.params_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: buffers.histogram_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(&buffers.palette_view),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::Sampler(&buffers.sampler),
                },
                BindGroupEntry {
                    binding: 5,
                    resource: buffers.variation_params_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 6,
                    resource: buffers.iteration_count_buffer.as_entire_binding(),
                },
                // Use helper methods that return real or dummy buffers
                BindGroupEntry {
                    binding: 7,
                    resource: buffers.get_path_buffer_for_binding().as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 8,
                    resource: buffers.get_filter_buffer_for_binding().as_entire_binding(),
                },
            ],
        })
    }

    /// Create bind group for accumulation pass
    pub fn create_accumulate_bind_group(
        &self,
        device: &Device,
        buffers: &super::buffers::FlameBuffers,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("Accumulate Bind Group"),
            layout: &self.accumulate_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(buffers.previous_accumulation_view()),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: buffers.histogram_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(buffers.output_accumulation_view()),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: buffers.accumulate_params_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: buffers.iteration_count_buffer.as_entire_binding(),
                },
            ],
        })
    }

    // Note: create_adjust_scale_bind_group() removed - adjust_scale pipeline unused

    /// Create bind group for tonemap pass
    pub fn create_tonemap_bind_group(
        &self,
        device: &Device,
        buffers: &super::buffers::FlameBuffers,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("Tonemap Bind Group"),
            layout: &self.tonemap_bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(buffers.current_accumulation_view()),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: BindingResource::Sampler(&buffers.sampler),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: buffers.tonemap_params_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(&buffers.curve_lut_view),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::Sampler(&buffers.curve_lut_sampler),
                },
                // Use helper method that returns real or dummy buffer
                BindGroupEntry {
                    binding: 5,
                    resource: buffers.get_path_buffer_for_binding().as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 6,
                    resource: BindingResource::TextureView(&buffers.palette_view),
                },
                BindGroupEntry {
                    binding: 7,
                    resource: BindingResource::Sampler(&buffers.sampler),
                },
            ],
        })
    }
}
