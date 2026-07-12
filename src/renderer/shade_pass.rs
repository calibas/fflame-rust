//! Solid-rendering deferred shade pass (Phase 1).
//!
//! Host side of `shaders/shade.wgsl`: reads the current accumulator + the
//! depth region inside the histogram buffer, writes a shaded Rgba32Float
//! texture that feeds `tonemap_pass_with_input` (density effects, when
//! enabled, read the shade output instead of the accumulator).
//!
//! Zero cost when off: the pass is simply not dispatched (and this
//! module's output texture is the only standing allocation, created
//! lazily on first use). See docs/projects/solid-rendering.md.

use egui_wgpu::wgpu::*;

use crate::config::SolidShadingSettings;

/// One light, GPU layout. Directions are precomputed host-side in
/// camera space; `enabled` is baked into intensity (0 = off).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadeLight {
    dir_intensity: [f32; 4],
    color: [f32; 4],
}

/// Mirrors WGSL `ShadeParams` (shade.wgsl) — 16 scalars + 4 lights × 32 B
/// = 192 bytes.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct ShadeParams {
    width: u32,
    height: u32,
    zoom: f32,
    rotation: f32,
    pan_x: f32,
    pan_y: f32,
    perspective_strength: f32,
    shading_strength: f32,
    ambient: f32,
    diffuse: f32,
    specular: f32,
    shininess: f32,
    ssao_strength: f32,
    ssao_radius: f32,
    surface_thickness: f32,
    depth_word_offset: u32,
    tex_y0: u32,
    tex_height: u32,
    use_normal_tex: u32,
    _pad1: u32,
    lights: [ShadeLight; 4],
}

/// Mirrors WGSL `AtrousParams` (atrous.wgsl).
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct AtrousParams {
    width: u32,
    height: u32,
    stride: u32,
    _pad0: u32,
    sigma_z: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
}

pub struct ShadePass {
    pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
    params_buffer: Buffer,
    // Normal-estimation + à-trous smoothing pipelines (full-image paths).
    normals_pipeline: ComputePipeline,
    normals_bgl: BindGroupLayout,
    atrous_pipeline: ComputePipeline,
    atrous_bgl: BindGroupLayout,
    /// One uniform buffer per à-trous iteration (strides 1, 2, 4) —
    /// distinct buffers because queue writes flatten before dispatches.
    atrous_params: [Buffer; 3],
    /// Full-image (normal.xyz, depth) ping-pong; sized (w, h) when present.
    normal_texs: Option<(u32, u32, [(Texture, TextureView); 2])>,
    /// 1×1 stand-in bound when the inline-normal path is used.
    dummy_normal: (Texture, TextureView),
    /// Full-image output (interactive path). Absent for pipeline-only
    /// users (exporters bring their own region-sized outputs).
    output: Option<(Texture, TextureView)>,
    width: u32,
    height: u32,
}

impl ShadePass {
    /// Pipeline-only construction (exporters): no standing output
    /// allocation; callers provide region outputs to `run_region`.
    pub fn new_pipeline_only(device: &Device) -> Self {
        let mut this = Self::new(device, 1, 1);
        this.output = None;
        this.normal_texs = None;
        this.width = 0;
        this.height = 0;
        this
    }

    pub fn new(device: &Device, width: u32, height: u32) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Solid Shade Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/shade.wgsl").into()),
        });

        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Shade Bind Group Layout"),
            entries: &[
                // 0: accumulator (Rgba32Float, textureLoad — non-filterable)
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
                // 1: histogram buffer (read-only; depth region at W*H*4 words)
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
                // 2: ShadeParams uniform
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
                // 3: shaded output (storage write)
                BindGroupLayoutEntry {
                    binding: 3,
                    visibility: ShaderStages::COMPUTE,
                    ty: BindingType::StorageTexture {
                        access: StorageTextureAccess::WriteOnly,
                        format: TextureFormat::Rgba32Float,
                        view_dimension: TextureViewDimension::D2,
                    },
                    count: None,
                },
                // 4: pre-smoothed normals (1×1 dummy when inline path used)
                BindGroupLayoutEntry {
                    binding: 4,
                    visibility: ShaderStages::COMPUTE,
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
            label: Some("Shade Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Shade Pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("shade_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        let params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Shade Params"),
            size: std::mem::size_of::<ShadeParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ── Normal estimation pipeline ──
        let normals_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Solid Normals Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/normals.wgsl").into()),
        });
        let normals_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Normals Bind Group Layout"),
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
                        ty: BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
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
            ],
        });
        let normals_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Normals Pipeline Layout"),
            bind_group_layouts: &[Some(&normals_bgl)],
            immediate_size: 0,
        });
        let normals_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Normals Pipeline"),
            layout: Some(&normals_layout),
            module: &normals_shader,
            entry_point: Some("normals_main"),
            compilation_options: Default::default(),
            cache: None,
        });

        // ── À-trous smoothing pipeline ──
        let atrous_shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Solid Atrous Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/atrous.wgsl").into()),
        });
        let atrous_bgl = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("Atrous Bind Group Layout"),
            entries: &[
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
            ],
        });
        let atrous_layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("Atrous Pipeline Layout"),
            bind_group_layouts: &[Some(&atrous_bgl)],
            immediate_size: 0,
        });
        let atrous_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("Atrous Pipeline"),
            layout: Some(&atrous_layout),
            module: &atrous_shader,
            entry_point: Some("atrous_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let atrous_params = [
            Self::create_atrous_params(device, 0),
            Self::create_atrous_params(device, 1),
            Self::create_atrous_params(device, 2),
        ];
        let dummy_normal = Self::create_normal_tex(device, 1, 1, "Shade Dummy Normal");

        let output = Some(Self::create_output(device, width, height));
        let normal_texs = Some((
            width,
            height,
            [
                Self::create_normal_tex(device, width, height, "Shade Normals A"),
                Self::create_normal_tex(device, width, height, "Shade Normals B"),
            ],
        ));

        Self {
            pipeline,
            bind_group_layout,
            params_buffer,
            normals_pipeline,
            normals_bgl,
            atrous_pipeline,
            atrous_bgl,
            atrous_params,
            normal_texs,
            dummy_normal,
            output,
            width,
            height,
        }
    }

    fn create_atrous_params(device: &Device, idx: u32) -> Buffer {
        device.create_buffer(&BufferDescriptor {
            label: Some(&format!("Atrous Params {}", idx)),
            size: std::mem::size_of::<AtrousParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        })
    }

    fn create_normal_tex(device: &Device, width: u32, height: u32, label: &str) -> (Texture, TextureView) {
        let tex = device.create_texture(&TextureDescriptor {
            label: Some(label),
            size: Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&TextureViewDescriptor::default());
        (tex, view)
    }

    /// Allocate (or resize) the full-image normal ping-pong — exporters
    /// call this before `run_region` to enable à-trous on one-shot shades.
    pub fn ensure_normal_textures(&mut self, device: &Device, width: u32, height: u32) {
        let needs = match &self.normal_texs {
            Some((w, h, _)) => *w != width || *h != height,
            None => true,
        };
        if needs {
            self.normal_texs = Some((
                width,
                height,
                [
                    Self::create_normal_tex(device, width, height, "Shade Normals A"),
                    Self::create_normal_tex(device, width, height, "Shade Normals B"),
                ],
            ));
        }
    }

    fn create_output(device: &Device, width: u32, height: u32) -> (Texture, TextureView) {
        let tex = device.create_texture(&TextureDescriptor {
            label: Some("Shade Output"),
            size: Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&TextureViewDescriptor::default());
        (tex, view)
    }

    pub fn resize(&mut self, device: &Device, width: u32, height: u32) {
        if width == self.width && height == self.height {
            return;
        }
        self.output = Some(Self::create_output(device, width, height));
        self.ensure_normal_textures(device, width, height);
        self.width = width;
        self.height = height;
    }

    pub fn output_view(&self) -> &TextureView {
        &self.output.as_ref().expect("ShadePass built pipeline-only has no output").1
    }

    /// Interactive-path dispatch: full image, depth region inside the
    /// histogram binding, internal output texture. The caller guarantees
    /// the depth region exists and this runs after the frame's
    /// accumulate pass.
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        accumulation_view: &TextureView,
        histogram_buffer: &Buffer,
        shading: &SolidShadingSettings,
        zoom: f32,
        rotation: f32,
        pan_x: f32,
        pan_y: f32,
        perspective_strength: f32,
        surface_thickness: f32,
    ) {
        let output_view = &self.output.as_ref().expect("interactive ShadePass has an output").1;
        self.run_region(
            device,
            queue,
            encoder,
            accumulation_view,
            histogram_buffer,
            output_view,
            shading,
            zoom,
            rotation,
            pan_x,
            pan_y,
            perspective_strength,
            surface_thickness,
            self.width,
            self.height,
            self.width * self.height * 4, // depth region inside the histogram
            0,
            self.height,
        );
    }

    /// Region dispatch (exporters): shade full-width rows
    /// [tex_y0, tex_y0 + tex_height) of a full_width×full_height image.
    /// `depth_buffer` holds encoded depths at `depth_word_offset` (0 for
    /// a dedicated depth buffer); the input/output textures are
    /// region-sized.
    #[allow(clippy::too_many_arguments)]
    pub fn run_region(
        &self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        accumulation_view: &TextureView,
        depth_buffer: &Buffer,
        output_view: &TextureView,
        shading: &SolidShadingSettings,
        zoom: f32,
        rotation: f32,
        pan_x: f32,
        pan_y: f32,
        perspective_strength: f32,
        surface_thickness: f32,
        full_width: u32,
        full_height: u32,
        depth_word_offset: u32,
        tex_y0: u32,
        tex_height: u32,
    ) {
        let mut lights = [ShadeLight {
            dir_intensity: [0.0; 4],
            color: [0.0; 4],
        }; 4];
        for (i, l) in shading.lights.iter().enumerate().take(4) {
            let az = l.azimuth.to_radians();
            let el = l.elevation.to_radians();
            // Camera-space direction TO the light: az = 0, el = 0 is a
            // headlight (from the viewer, +z); az swings around the
            // vertical axis, el tilts up.
            let dir = [
                el.cos() * az.sin(),
                el.sin(),
                el.cos() * az.cos(),
            ];
            let intensity = if l.enabled { l.intensity.max(0.0) } else { 0.0 };
            lights[i] = ShadeLight {
                dir_intensity: [dir[0], dir[1], dir[2], intensity],
                color: [l.color[0], l.color[1], l.color[2], 0.0],
            };
        }

        // Full-image normal path (normals pass + à-trous) is available when
        // the dispatch covers the whole image and the ping-pong textures
        // match it; the strip-tiled export path falls back to the inline
        // estimator in shade.wgsl.
        let smoothing = shading.normal_smoothing.min(3) as usize;
        let full_cover = tex_y0 == 0 && tex_height == full_height;
        let normal_view: Option<&TextureView> = match &self.normal_texs {
            Some((w, h, texs)) if full_cover && *w == full_width && *h == full_height => {
                Some(&texs[smoothing % 2].1) // final resting texture after ping-pong
            }
            _ => None,
        };

        let params = ShadeParams {
            width: full_width,
            height: full_height,
            zoom,
            rotation,
            pan_x,
            pan_y,
            perspective_strength,
            shading_strength: shading.shading_strength,
            ambient: shading.ambient,
            diffuse: shading.diffuse,
            specular: shading.specular,
            shininess: shading.shininess,
            ssao_strength: shading.ssao_strength,
            ssao_radius: shading.ssao_radius,
            surface_thickness,
            depth_word_offset,
            tex_y0,
            tex_height,
            use_normal_tex: u32::from(normal_view.is_some()),
            _pad1: 0,
            lights,
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        // Encode the normal-estimation + smoothing chain ahead of the shade
        // dispatch (same encoder, ordered).
        if normal_view.is_some() {
            let (_, _, texs) = self.normal_texs.as_ref().unwrap();
            let normals_bg = device.create_bind_group(&BindGroupDescriptor {
                label: Some("Normals Bind Group"),
                layout: &self.normals_bgl,
                entries: &[
                    BindGroupEntry { binding: 0, resource: depth_buffer.as_entire_binding() },
                    BindGroupEntry { binding: 1, resource: self.params_buffer.as_entire_binding() },
                    BindGroupEntry { binding: 2, resource: BindingResource::TextureView(&texs[0].1) },
                ],
            });
            {
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Solid Normals Pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.normals_pipeline);
                pass.set_bind_group(0, &normals_bg, &[]);
                pass.dispatch_workgroups(full_width.div_ceil(8), full_height.div_ceil(8), 1);
            }
            // σ_z tracks the depth-noise scale (the surface shell).
            let sigma_z = surface_thickness.max(0.005) * 2.0;
            for i in 0..smoothing {
                let ap = AtrousParams {
                    width: full_width,
                    height: full_height,
                    stride: 1u32 << i,
                    _pad0: 0,
                    sigma_z,
                    _pad1: 0.0,
                    _pad2: 0.0,
                    _pad3: 0.0,
                };
                queue.write_buffer(&self.atrous_params[i], 0, bytemuck::bytes_of(&ap));
                let src = i % 2;
                let dst = 1 - src;
                let bg = device.create_bind_group(&BindGroupDescriptor {
                    label: Some("Atrous Bind Group"),
                    layout: &self.atrous_bgl,
                    entries: &[
                        BindGroupEntry { binding: 0, resource: BindingResource::TextureView(&texs[src].1) },
                        BindGroupEntry { binding: 1, resource: self.atrous_params[i].as_entire_binding() },
                        BindGroupEntry { binding: 2, resource: BindingResource::TextureView(&texs[dst].1) },
                    ],
                });
                let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                    label: Some("Solid Atrous Pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.atrous_pipeline);
                pass.set_bind_group(0, &bg, &[]);
                pass.dispatch_workgroups(full_width.div_ceil(8), full_height.div_ceil(8), 1);
            }
        }

        // The accumulator ping-pongs every frame, so the bind group is
        // per-dispatch (same pattern as tonemap_pass_with_input).
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("Shade Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(accumulation_view),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: depth_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 2,
                    resource: self.params_buffer.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 3,
                    resource: BindingResource::TextureView(output_view),
                },
                BindGroupEntry {
                    binding: 4,
                    resource: BindingResource::TextureView(
                        normal_view.unwrap_or(&self.dummy_normal.1),
                    ),
                },
            ],
        });

        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Solid Shade Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(full_width.div_ceil(8), tex_height.div_ceil(8), 1);
    }
}
