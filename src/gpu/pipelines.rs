use egui_wgpu::wgpu::*;
use crate::shader_cache::ShaderCache;
use crate::scene::transforms::Flame;

/// A compute bind group, and the bindings it holds: those of the shader it
/// was made for.
pub struct ComputeBindGroup {
    pub group: BindGroup,
    pub bindings: Vec<u32>,
}

pub struct FlamePipelines {
    /// The flame shaders, each with its own layout of the bindings it uses
    /// (`shader_cache.compute_layout`).
    pub shader_cache: ShaderCache,
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
        let shader_cache = ShaderCache::new(device, flame);

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
    /// (rebuilds, cache hits, total compile ms) from the flame shader
    /// cache — docs/projects/sticky-shader-compilation.md.
    pub fn shader_rebuild_stats(&self) -> (u64, u64, f64) {
        self.shader_cache.rebuild_stats()
    }

    pub fn ensure_shaders_current(&mut self, device: &Device, flame: &Flame, render_mode: crate::scene::transforms::RenderMode) -> bool {
        self.shader_cache.ensure_current(device, flame, render_mode)
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
        census: bool,
        cylinder_targeting: bool,
        cylinder_replay: bool,
        cylinder_relative: bool,
        cylinder_offsets: bool,
        leak_probe: bool,
    ) -> bool {
        // Census is renderer state, not config state — a .fflame cannot
        // ask to be instrumented. Threaded from FlameRenderer::census.
        let mut constants = crate::shader_cache::ShaderCache::constants_from_config(config);
        constants.census = census;
        // Targeting is a property of the VIEW as well as the flame —
        // the enumeration has to succeed AND be worth it — so the
        // renderer decides and threads the verdict, exactly as it
        // does for the census. `constants_from_config` cannot know:
        // it has no frame size and does not run the enumeration.
        constants.cylinder_targeting = cylinder_targeting;
        // ...and which ARM of it. Threaded for the same reason and
        // with the same consequence if it is not: the buffer would
        // hold the replay layout while the shader read the composed
        // one, which renders an empty frame rather than a wrong
        // picture -- silent unless a gate compares against a
        // reference.
        constants.cylinder_replay = cylinder_replay;
        constants.cylinder_relative = cylinder_relative;
        constants.cylinder_offsets = cylinder_offsets;
        // The leak probe rides the frame-coverage counters, and like
        // the two above it is renderer state: a `.fflame` cannot ask
        // to be measured. Without this the probe is set, the params
        // carry the region, and the shader simply has no counters to
        // add to -- which reads as "no leak" rather than as an error.
        constants.frame_coverage = constants.frame_coverage || leak_probe;
        self.shader_cache.ensure_current_full(
            device,
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
            // Escape mode has no trajectory pipeline — the fragment
            // renderer owns it, and the app branches before the chaos
            // game runs. Degrade to 2D rather than panic if mis-routed.
            crate::scene::transforms::RenderMode::Escape => self.shader_cache.pipeline_2d(),
            // Same for simulation: the grid stepper owns its pipelines
            // and the app branches before the chaos game runs.
            crate::scene::transforms::RenderMode::Simulation => self.shader_cache.pipeline_2d(),
        }
    }

    /// Create bind group for compute pass
    /// The compute bind group for the current flame shader: the bindings
    /// it uses, in its layout (`compute_layout`). Recorded with them, so a
    /// shader whose bindings changed is seen to need a new one
    /// (`FlameRenderer::compute_pass`).
    pub fn create_compute_bind_group(
        &self,
        device: &Device,
        buffers: &super::buffers::FlameBuffers,
    ) -> ComputeBindGroup {
        let bindings = self.shader_cache.compute_bindings.clone();
        let entries: Vec<BindGroupEntry> = compute_entries(buffers).into_iter().filter(|e| bindings.contains(&e.binding)).collect();
        let group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Compute Bind Group"),
            layout: &self.shader_cache.compute_layout,
            entries: &entries,
        });
        ComputeBindGroup { group, bindings }
    }

    /// Every binding, and a layout of them all: for a shader that is not
    /// the flame's own (the numerical probe). Made when asked for, never at
    /// startup -- all eleven storage buffers at once are more than a
    /// browser may allow.
    pub fn create_full_compute_bind_group(&self, device: &Device, buffers: &super::buffers::FlameBuffers) -> (BindGroupLayout, BindGroup) {
        let layout = compute_layout(device, None);
        let group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Compute Bind Group (all bindings)"),
            layout: &layout,
            entries: &compute_entries(buffers),
        });
        (layout, group)
    }

    /// The bindings the current flame shader uses.
    pub fn compute_bindings(&self) -> &[u32] {
        &self.shader_cache.compute_bindings
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

/// **Every binding the flame compute shader can use.** A shader's layout
/// is these filtered to the ones its WGSL uses (`used_bindings`): a
/// browser counts the storage buffers in the LAYOUT against its
/// per-stage limit, whether the shader reads them or not, and a layout
/// of all eleven exceeded the ten a laptop's Chrome allows, so no flame
/// rendered there at all.
fn compute_layout_entries() -> Vec<BindGroupLayoutEntry> {
    vec![
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
            // Binding 8 is a historical gap: the path filters, which
            // word editing replaced (docs/projects/word-editing.md).
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
            // Frame-coverage counters (auto exposure). Declared by
            // the shader only under FRAME_COVERAGE; the layout
            // always carries it.
            BindGroupLayoutEntry {
                binding: 16,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: false },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // Enumerated cylinders (docs/projects/flame-deep-zoom.md
            // stage 2). Declared by the shader only under
            // CYLINDER_TARGETING; the layout always carries it.
            BindGroupLayoutEntry {
                binding: 15,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            // Biased selection weights + likelihood ratios
            // (docs/projects/flame-deep-zoom.md stage 1). Slot 11
            // was the legacy subflame_transforms buffer, removed in
            // v2 of the subflame work and empty since; this takes
            // it rather than extending the layout.
            //
            // The LAYOUT always carries this entry, but the SHADER
            // declares the binding only under IMPORTANCE_SAMPLING
            // — that is what keeps the WGSL byte-identical when the
            // feature is off, and a layout entry a shader does not
            // reference is allowed.
            BindGroupLayoutEntry {
                binding: 11,
                visibility: ShaderStages::COMPUTE,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
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
    ]
}

/// The resources for `compute_layout_entries`, from the renderer's buffers.
fn compute_entries(buffers: &super::buffers::FlameBuffers) -> Vec<BindGroupEntry<'_>> {
    vec![
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
            // Frame-coverage counters.
            BindGroupEntry {
                binding: 16,
                resource: buffers.coverage_buffer.as_entire_binding(),
            },
            // Cylinder table (real or dummy).
            BindGroupEntry {
                binding: 15,
                resource: buffers.cylinder_binding().as_entire_binding(),
            },
            // Biased selection table (real or dummy).
            BindGroupEntry {
                binding: 11,
                resource: buffers.bias_binding().as_entire_binding(),
            },
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
    ]
}

/// The flame compute layout holding `bindings`, or every binding for `None`.
pub fn compute_layout(device: &Device, bindings: Option<&[u32]>) -> BindGroupLayout {
    let entries: Vec<BindGroupLayoutEntry> =
        compute_layout_entries().into_iter().filter(|e| bindings.is_none_or(|b| b.contains(&e.binding))).collect();
    device.create_bind_group_layout(&BindGroupLayoutDescriptor { label: Some("Compute Bind Group Layout"), entries: &entries })
}

/// Which of `bindings` are storage buffers: what a device's
/// `max_storage_buffers_per_shader_stage` counts.
pub fn storage_bindings(bindings: &[u32]) -> Vec<u32> {
    compute_layout_entries()
        .into_iter()
        .filter(|e| bindings.contains(&e.binding) && matches!(e.ty, BindingType::Buffer { ty: BufferBindingType::Storage { .. }, .. }))
        .map(|e| e.binding)
        .collect()
}

/// **The group-0 bindings a WGSL module uses**: each declared
/// `@binding(N) var ... name`, if `name` appears again outside comments.
///
/// Read from the source because the web build has no WGSL parser of its
/// own (wgpu's naga is configured out there; the browser parses). A name
/// that appears in a function the entry point never calls still counts,
/// so this can include a binding the shader does not use -- one layout
/// entry too many -- but never leave out one it does: a use is the name.
/// `a_layout_holds_every_binding_its_shader_uses` holds it to naga's
/// answer on the desktop.
pub fn used_bindings(source: &str) -> Vec<u32> {
    let code: String = source
        .lines()
        .map(|l| match l.find("//") {
            Some(at) => &l[..at],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n");
    let words = |name: &str| -> usize {
        code.match_indices(name)
            .filter(|(at, _)| {
                let before = code[..*at].chars().next_back();
                let after = code[at + name.len()..].chars().next();
                let ident = |c: Option<char>| c.is_some_and(|c| c.is_alphanumeric() || c == '_');
                !ident(before) && !ident(after)
            })
            .count()
    };
    let mut used = Vec::new();
    for line in code.lines() {
        let Some(at) = line.find("@binding(") else { continue };
        let rest = &line[at + "@binding(".len()..];
        let Some(n) = rest.split(')').next().and_then(|n| n.trim().parse::<u32>().ok()) else { continue };
        let Some(var) = rest.find("var") else { continue };
        let mut decl = rest[var + 3..].trim_start();
        if decl.starts_with('<') {
            decl = decl.split_once('>').map_or("", |(_, r)| r).trim_start();
        }
        let name: String = decl.chars().take_while(|c| c.is_alphanumeric() || *c == '_').collect();
        if !name.is_empty() && words(&name) > 1 && !used.contains(&n) {
            used.push(n);
        }
    }
    used.sort_unstable();
    used
}

#[cfg(test)]
mod layout_tests {
    use super::*;
    use crate::config::FractalConfig;

    /// The group-0 bindings naga says the entry point `main` uses.
    fn naga_used(source: &str) -> Vec<u32> {
        use wgpu::naga;
        let module = naga::front::wgsl::parse_str(source).expect("the WGSL parses");
        let info = naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&module)
            .expect("the WGSL validates");
        let ep = module.entry_points.iter().position(|e| e.name == "main").expect("an entry point `main`");
        let uses = info.get_entry_point(ep);
        let mut used: Vec<u32> = module
            .global_variables
            .iter()
            .filter(|(h, _)| !uses[*h].is_empty())
            .filter_map(|(_, v)| v.binding.as_ref().map(|b| b.binding))
            .collect();
        used.sort_unstable();
        used
    }

    /// The flame shaders to hold the scan to: the committed dumps, and the
    /// in-app 2D shader built with each binding-bearing feature on.
    fn shaders() -> Vec<(String, String)> {
        let mut out = Vec::new();
        for entry in std::fs::read_dir("tests/shader_dumps").expect("tests/shader_dumps") {
            let path = entry.expect("an entry").path();
            if path.extension().is_some_and(|e| e == "wgsl") {
                out.push((path.file_name().unwrap().to_string_lossy().into_owned(), std::fs::read_to_string(&path).expect("a dump")));
            }
        }
        let cfg = FractalConfig::default();
        let builder = crate::shader_builder_v2::ShaderBuilder::new(crate::variations::global_registry().clone());
        let active = cfg.flame.extract_active_variations();
        type Tweak = fn(&mut crate::shader_builder_v2::ShaderConstants);
        let tweaks: [(&str, Tweak, bool); 7] = [
            ("plain", |_| {}, false),
            ("auto exposure", |c| c.frame_coverage = true, false),
            ("importance", |c| c.importance_sampling = true, false),
            ("targeting, composed", |c| c.cylinder_targeting = true, false),
            ("targeting, replayed in offsets", |c| {
                c.cylinder_targeting = true;
                c.cylinder_replay = true;
                c.cylinder_relative = true;
                c.cylinder_offsets = true;
            }, false),
            ("PathMap", |c| {
                c.cylinder_targeting = true;
                c.color_mode = crate::prelude::ColorMode::PathMap as u32;
            }, true),
            ("all of them", |c| {
                c.frame_coverage = true;
                c.importance_sampling = true;
                c.cylinder_targeting = true;
                c.cylinder_replay = true;
                c.cylinder_relative = true;
                c.cylinder_offsets = true;
            }, true),
        ];
        for (name, tweak, path) in tweaks {
            let mut constants = crate::shader_cache::ShaderCache::constants_from_config(&cfg);
            tweak(&mut constants);
            let src = builder.build_from_template(&cfg.flame, &active, false, path, false, true, &constants);
            out.push((format!("built: {name}"), src));
        }
        out
    }

    /// **A layout holds every binding its shader uses**: the text scan
    /// (`used_bindings`) against naga's own account of the entry point, on
    /// every shader above. The scan may hold more -- a name in a function
    /// never called -- but never less, or the pipeline would not build.
    #[test]
    fn a_layout_holds_every_binding_its_shader_uses() {
        for (name, src) in shaders() {
            let scanned = used_bindings(&src);
            let used = naga_used(&src);
            let missing: Vec<u32> = used.iter().copied().filter(|b| !scanned.contains(b)).collect();
            let extra: Vec<u32> = scanned.iter().copied().filter(|b| !used.contains(b)).collect();
            println!(
                "  {name:<34} storage {:>2} {:?}{}",
                storage_bindings(&scanned).len(),
                storage_bindings(&scanned),
                if extra.is_empty() { String::new() } else { format!("  (scanned, not used: {extra:?})") }
            );
            assert!(missing.is_empty(), "{name}: the shader uses {missing:?}, which the scan left out");
        }
    }

    /// A device limited as a browser limits one: `storage` storage buffers
    /// a shader stage.
    fn limited_device(storage: u32) -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .expect("adapter");
        let mut limits = wgpu::Limits::default();
        limits.max_storage_buffers_per_shader_stage = storage;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("limited device"),
            required_features: wgpu::Features::CLEAR_TEXTURE,
            required_limits: limits,
            ..Default::default()
        }))
        .expect("device")
    }

    /// **A flame renders within a browser's storage-buffer limit.** A
    /// layout of every binding held eleven storage buffers, and a laptop's
    /// Chrome allows ten: no flame rendered there. Now a plain flame needs
    /// no more than WebGPU's minimum of eight, and one with Focused
    /// Rendering, auto exposure and importance sampling all on fits in ten.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_flame_renders_within_a_browsers_storage_limit() {
        let lit = |rgba: &[u8]| rgba.chunks(4).filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24).count();
        let render = |device: &wgpu::Device, queue: &wgpu::Queue, cfg: &FractalConfig| {
            let job = crate::renderer::RenderJob::new(cfg, 96, 96).with_iterations(4_000_000);
            pollster::block_on(crate::renderer::render(device, queue, job, &mut crate::renderer::NoProgress)).expect("render").rgba_data
        };
        // The Heighway dragon: two affine maps.
        let dragon = || {
            let mut cfg = FractalConfig::default();
            cfg.flame.transforms.clear();
            for (m, colour) in [([0.5f32, -0.5, 0.5, 0.5, 0.0, 0.0], 0.2f32), ([-0.5, -0.5, 0.5, -0.5, 1.0, 0.0], 0.8)] {
                let mut t = crate::scene::transforms::Transform::default();
                (t.a, t.b, t.c, t.d, t.e, t.f) = (m[0], m[1], m[2], m[3], m[4], m[5]);
                t.weight = 1.0;
                t.color = colour;
                t.variations.clear();
                t.variation_order.clear();
                t.set_variation("linear", 1.0);
                cfg.flame.transforms.push(t);
            }
            cfg.levels_enabled = false;
            (cfg.pan_x, cfg.pan_y) = (0.5, 0.25);
            cfg
        };
        // A plain flame, at WebGPU's minimum.
        let (device, queue) = limited_device(8);
        let n = lit(&render(&device, &queue, &dragon()));
        println!("  plain flame, 8 storage buffers: {n} pixels lit");
        assert!(n > 100, "the plain flame drew {n} pixels");

        // Everything that adds a binding, at the laptop's ten.
        let (device, queue) = limited_device(10);
        let mut cfg = dragon();
        cfg.zoom = 1e4;
        cfg.cylinder_targeting = true;
        cfg.cylinder_always = true;
        cfg.auto_exposure = true;
        cfg.importance.enabled = true;
        let n = lit(&render(&device, &queue, &cfg));
        println!("  targeted, auto-exposed, importance-sampled, 10 storage buffers: {n} pixels lit");
        assert!(n > 100, "the targeted flame drew {n} pixels");
    }
}
