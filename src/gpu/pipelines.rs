use wgpu::*;

pub struct FlamePipelines {
    pub compute_pipeline: ComputePipeline,
    pub compute_bind_group_layout: BindGroupLayout,
    pub accumulate_pipeline: ComputePipeline,
    pub accumulate_bind_group_layout: BindGroupLayout,
    pub tonemap_pipeline: RenderPipeline,
    pub tonemap_bind_group_layout: BindGroupLayout,
}

impl FlamePipelines {
    pub fn new(device: &Device, surface_format: TextureFormat) -> Self {
        // Load shaders
        let trajectory_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Trajectory Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/trajectory.wgsl").into()),
        });

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
                // Accumulation texture (storage, write-only)
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
                // Palette texture (1D)
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D1,
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
            ],
        });

        // Create compute pipeline
        let compute_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Compute Pipeline Layout"),
            bind_group_layouts: &[&compute_bind_group_layout],
            push_constant_ranges: &[],
        });

        let compute_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Trajectory Compute Pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &trajectory_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Create accumulation bind group layout (temporary minimal version)
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
                // New samples (sampled texture)
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: true },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
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
                    format: surface_format,
                    blend: Some(BlendState::ALPHA_BLENDING),
                    write_mask: ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            multiview: None,
            cache: None,
        });

        Self {
            compute_pipeline,
            compute_bind_group_layout,
            accumulate_pipeline,
            accumulate_bind_group_layout,
            tonemap_pipeline,
            tonemap_bind_group_layout,
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
                    resource: BindingResource::TextureView(&buffers.temp_samples_view),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(&buffers.palette_view),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::Sampler(&buffers.sampler),
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
                    resource: BindingResource::TextureView(&buffers.temp_samples_view),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: BindingResource::TextureView(buffers.output_accumulation_view()),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: buffers.accumulate_params_buffer.as_entire_binding(),
                },
            ],
        })
    }

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
            ],
        })
    }
}
