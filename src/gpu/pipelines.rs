use egui_wgpu::wgpu::*;
use crate::shader_cache::ShaderCache;
use crate::scene::transforms::Flame;

pub struct FlamePipelines {
    pub shader_cache: ShaderCache,
    pub compute_bind_group_layout: BindGroupLayout,
    pub accumulate_pipeline: ComputePipeline,
    pub accumulate_bind_group_layout: BindGroupLayout,
    pub histogram_blur_pipeline: ComputePipeline,
    pub histogram_blur_bind_group_layout: BindGroupLayout,
    pub blur_convolve_pipeline: ComputePipeline,
    pub blur_convolve_bind_group_layout: BindGroupLayout,
    pub blur_upscale_pipeline: ComputePipeline,
    pub blur_stage_bind_group_layout: BindGroupLayout,
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

        let histogram_blur_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Histogram Blur Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/histogram_blur.wgsl").into()),
        });

        let blur_convolve_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Blur Convolve Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/blur_convolve.wgsl").into()),
        });

        let blur_upscale_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Blur Upscale Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/blur_upscale.wgsl").into()),
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
                // (binding 6 is a historical gap — the old
                // iteration_counts slot.)
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
                // Xaos weights buffer (storage, read-only for chaos-weighted transform selection)
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
                // Per-normal-transform attachment list buffer.
                // Each entry holds up to `flame.attachment_cap()` linked +
                // cap final GLOBAL xform_ids (plus counts); the main loop
                // walks them after the chaos game picks a normal transform.
                BindGroupLayoutEntry {
                    binding: 10,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // (binding 11 was the legacy subflame_transforms buffer
                // — removed in v2 of the subflame variation work.
                // Subflame xforms now share the parent's `transforms`
                // buffer at @binding(0). Slot 11 is intentionally left
                // unbound to preserve binding numbering with existing
                // shaders.)
                // Subflame metadata: array<SubflameMeta> with per-subflame
                // (normals_offset/count, finals_offset/count, render_mode).
                // Indexed by `subflame_id` (variation param). Storage rather
                // than uniform so the WGSL array is runtime-sized — same
                // access pattern as the other bindings in this layout.
                BindGroupLayoutEntry {
                    binding: 12,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Analytic-blur mean-splat histograms (read_write). Always in
                // the layout; a 1-element dummy is bound when the feature is
                // inactive. See docs/projects/analytic-blur-buffer.md.
                BindGroupLayoutEntry {
                    binding: 13,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                // Analytic-blur convolve params (uniform) — the routing reads
                // D / lowres dims / count to splat into the low-res buffer.
                BindGroupLayoutEntry {
                    binding: 14,
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

        let tonemap_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Tonemap Bind Group Layout"),
            entries: &[
                // Accumulation texture (point-fetched via textureLoad
                // — Rgba32Float is non-filterable without the
                // FLOAT32_FILTERABLE feature, and a 1:1 fullscreen
                // pass doesn't benefit from filtering anyway).
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
                        view_dimension: TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                // Sampler — kept for binding-layout compatibility with
                // the shader's `accumulation_sampler` declaration but
                // unused (textureLoad takes no sampler).
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::FRAGMENT,
                    ty: BindingType::Sampler(SamplerBindingType::NonFiltering),
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
                // Previous accumulation. Read via `textureLoad` in
                // accumulate.wgsl, so it just needs to be a typed
                // texture binding — non-filterable, since the
                // accumulation texture is Rgba32Float (Phase 8c) and
                // the FLOAT32_FILTERABLE feature isn't requested.
                BindGroupLayoutEntry {
                    binding: 0,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Texture {
                        sample_type: TextureSampleType::Float { filterable: false },
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
                // Output texture (storage, write) — Rgba32Float to
                // match the accumulation texture format change in
                // gpu/buffers.rs (Phase 8c precision fix).
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba32Float,
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
                // 4: accumulator depth-ownership tracker (solid
                // rendering depth-tightening reset; dummy when off)
                BindGroupLayoutEntry {
                    binding: 4,
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

        let accumulate_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Accumulate Pipeline Layout"),
            bind_group_layouts: &[Some(&accumulate_bind_group_layout)],
            immediate_size: 0,
        });

        let accumulate_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Accumulate Compute Pipeline"),
            layout: Some(&accumulate_pipeline_layout),
            module: &accumulate_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Histogram-blur bind group layout (matches histogram_blur.wgsl):
        //   0 — histogram_in   (storage, read-only)
        //   1 — histogram_out  (storage, read+write)
        //   2 — BlurParams     (uniform)
        let histogram_blur_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Histogram Blur Bind Group Layout"),
            entries: &[
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
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
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

        let histogram_blur_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Histogram Blur Pipeline Layout"),
            bind_group_layouts: &[Some(&histogram_blur_bind_group_layout)],
            immediate_size: 0,
        });

        let histogram_blur_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Histogram Blur Compute Pipeline"),
            layout: Some(&histogram_blur_pipeline_layout),
            module: &histogram_blur_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Blur-convolve bind group layout (matches blur_convolve.wgsl):
        //   0 — blur_in        (storage, read-only)
        //   1 — histogram_out  (storage, read+write)
        //   2 — ConvolveParams (uniform)
        //   3 — kernel_weights (storage, read-only)
        let blur_convolve_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Blur Convolve Bind Group Layout"),
            entries: &[
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
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 3,
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

        let blur_convolve_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Blur Convolve Pipeline Layout"),
            bind_group_layouts: &[Some(&blur_convolve_bind_group_layout)],
            immediate_size: 0,
        });

        let blur_convolve_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Blur Convolve Compute Pipeline"),
            layout: Some(&blur_convolve_pipeline_layout),
            module: &blur_convolve_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Shared 3-binding layout for the downsample and upscale stages:
        //   0 — input  (storage, read-only)
        //   1 — output (storage, read+write)
        //   2 — ConvolveParams (uniform)
        let blur_stage_bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Blur Stage Bind Group Layout"),
            entries: &[
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
                BindGroupLayoutEntry {
                    binding: 1,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::Buffer {
                        ty: BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                BindGroupLayoutEntry {
                    binding: 2,
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

        let blur_stage_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Blur Stage Pipeline Layout"),
            bind_group_layouts: &[Some(&blur_stage_bind_group_layout)],
            immediate_size: 0,
        });

        let blur_upscale_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Blur Upscale Compute Pipeline"),
            layout: Some(&blur_stage_pipeline_layout),
            module: &blur_upscale_shader,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // Create tonemap render pipeline
        let tonemap_pipeline_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Tonemap Pipeline Layout"),
            bind_group_layouts: &[Some(&tonemap_bind_group_layout)],
            immediate_size: 0,
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
            multiview_mask: None,
            cache: None,
        });

        Self {
            shader_cache,
            compute_bind_group_layout,
            accumulate_pipeline,
            accumulate_bind_group_layout,
            histogram_blur_pipeline,
            histogram_blur_bind_group_layout,
            blur_convolve_pipeline,
            blur_convolve_bind_group_layout,
            blur_upscale_pipeline,
            blur_stage_bind_group_layout,
            tonemap_pipeline,
            tonemap_bind_group_layout,
        }
    }

    /// Ensure shaders are up-to-date with current flame configuration
    /// Returns true if shaders were recompiled
    pub fn ensure_shaders_current(&mut self, device: &Device, flame: &Flame, render_mode: crate::scene::transforms::RenderMode) -> bool {
        self.shader_cache.ensure_current(device, &self.compute_bind_group_layout, flame, render_mode)
    }

    /// Ensure shaders are up-to-date with current flame configuration and path features state
    /// Returns true if shaders were recompiled
    pub fn ensure_shaders_current_with_path_features(
        &mut self,
        device: &Device,
        flame: &Flame,
        path_features_enabled: bool,
        render_mode: crate::scene::transforms::RenderMode,
    ) -> bool {
        self.shader_cache.ensure_current_with_path_features(
            device,
            &self.compute_bind_group_layout,
            flame,
            path_features_enabled,
            render_mode,
        )
    }

    /// Ensure shaders are up-to-date with full FractalConfig (variations, path features, and constants)
    /// This is the preferred method for loading configs as it properly updates all shader constants.
    /// Returns true if shaders were recompiled
    pub fn ensure_shaders_current_with_config(
        &mut self,
        device: &Device,
        config: &crate::config::FractalConfig,
        path_features_enabled: bool,
    ) -> bool {
        let constants = crate::shader_cache::ShaderCache::constants_from_config(config);
        self.shader_cache.ensure_current_full(
            device,
            &self.compute_bind_group_layout,
            &config.flame,
            path_features_enabled,
            constants,
            config.render_mode,
        )
    }

    /// Ensure shaders are up-to-date with explicit constants
    /// Used for incremental updates where full FractalConfig isn't available
    /// Returns true if shaders were recompiled
    pub fn ensure_shaders_current_with_constants(
        &mut self,
        device: &Device,
        flame: &Flame,
        path_features_enabled: bool,
        constants: crate::shader_builder_v2::ShaderConstants,
        render_mode: crate::scene::transforms::RenderMode,
    ) -> bool {
        self.shader_cache.ensure_current_full(
            device,
            &self.compute_bind_group_layout,
            flame,
            path_features_enabled,
            constants,
            render_mode,
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
                // Use helper methods that return real or dummy buffers
                BindGroupEntry {
                    binding: 7,
                    resource: buffers.get_path_buffer_for_binding().as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 8,
                    resource: buffers.get_filter_buffer_for_binding().as_entire_binding(),
                },
                // Xaos weights for chaos-weighted transform selection
                BindGroupEntry {
                    binding: 9,
                    resource: buffers.get_xaos_buffer_for_binding().as_entire_binding(),
                },
                // Per-normal-transform attachment lists (Linked + Final chains)
                BindGroupEntry {
                    binding: 10,
                    resource: buffers.attachments_buffer.as_entire_binding(),
                },
                // (binding 11 dropped — see layout comment.)
                // Subflame metadata uniform.
                BindGroupEntry {
                    binding: 12,
                    resource: buffers.subflame_metadata_buffer.as_entire_binding(),
                },
                // Analytic-blur low-res splat buffer (real or dummy).
                BindGroupEntry {
                    binding: 13,
                    resource: buffers.get_blur_splat_for_binding().as_entire_binding(),
                },
                // Analytic-blur convolve params (D / lowres dims / count).
                BindGroupEntry {
                    binding: 14,
                    resource: buffers.blur_convolve_params_buffer.as_entire_binding(),
                },
            ],
        })
    }

    /// Create the init compute pass bind group.
    /// One binding: variation_params buffer with read_write access. Layout is
    /// owned by the ShaderCache (`init_bind_group_layout`).
    pub fn create_init_bind_group(
        &self,
        device: &Device,
        buffers: &super::buffers::FlameBuffers,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("Variation Init Bind Group"),
            layout: &self.shader_cache.init_bind_group_layout,
            entries: &[BindGroupEntry {
                binding: 0,
                resource: buffers.variation_params_buffer.as_entire_binding(),
            }],
        })
    }

    /// Bind group for histogram-blur horizontal pass: in = primary, out = scratch.
    pub fn create_histogram_blur_h_bind_group(
        &self,
        device: &Device,
        buffers: &super::buffers::FlameBuffers,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("Histogram Blur Bind Group (H)"),
            layout: &self.histogram_blur_bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: buffers.histogram_buffer.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: buffers.histogram_buffer_scratch.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: buffers.histogram_blur_params_buffer_h.as_entire_binding() },
            ],
        })
    }

    /// Bind group for histogram-blur vertical pass: in = scratch, out = primary.
    pub fn create_histogram_blur_v_bind_group(
        &self,
        device: &Device,
        buffers: &super::buffers::FlameBuffers,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("Histogram Blur Bind Group (V)"),
            layout: &self.histogram_blur_bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: buffers.histogram_buffer_scratch.as_entire_binding() },
                BindGroupEntry { binding: 1, resource: buffers.histogram_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: buffers.histogram_blur_params_buffer_v.as_entire_binding() },
            ],
        })
    }

    /// Bind group for the analytic-blur convolution pass (at low res): in =
    /// the low-res splat buffer (the chaos game splatted directly to low res),
    /// out = low-res convolved scratch, + convolve params + kernel weights.
    pub fn create_blur_convolve_bind_group(
        &self,
        device: &Device,
        buffers: &super::buffers::FlameBuffers,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("Blur Convolve Bind Group"),
            layout: &self.blur_convolve_bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: buffers.get_blur_splat_for_binding().as_entire_binding() },
                BindGroupEntry { binding: 1, resource: buffers.get_blur_convolved_for_binding().as_entire_binding() },
                BindGroupEntry { binding: 2, resource: buffers.blur_convolve_params_buffer.as_entire_binding() },
                BindGroupEntry { binding: 3, resource: buffers.blur_kernel_weights_buffer.as_entire_binding() },
            ],
        })
    }

    /// Bind group for the analytic-blur upscale + add stage: in = low-res
    /// convolved scratch, out = the main histogram, + convolve params.
    pub fn create_blur_upscale_bind_group(
        &self,
        device: &Device,
        buffers: &super::buffers::FlameBuffers,
    ) -> BindGroup {
        device.create_bind_group(&BindGroupDescriptor {
            label: Some("Blur Upscale Bind Group"),
            layout: &self.blur_stage_bind_group_layout,
            entries: &[
                BindGroupEntry { binding: 0, resource: buffers.get_blur_convolved_for_binding().as_entire_binding() },
                BindGroupEntry { binding: 1, resource: buffers.histogram_buffer.as_entire_binding() },
                BindGroupEntry { binding: 2, resource: buffers.blur_convolve_params_buffer.as_entire_binding() },
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
                    resource: buffers
                        .accum_depth_buffer
                        .as_ref()
                        .unwrap_or(&buffers.dummy_xaos_buffer)
                        .as_entire_binding(),
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
