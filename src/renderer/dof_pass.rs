//! Post-process depth-of-field pass for solid rendering.
//!
//! Host side of `shaders/dof.wgsl`: a dense 2-D disk gather over the
//! HDR pre-tonemap image (shade output when lighting is on, else the
//! accumulator) using the solid pipeline's per-pixel nearest-depth
//! region. Four dispatches: CoC prepass (bilaterally smoothed depth →
//! CoC field), separable max-dilate (H + V → per-pixel gather bound),
//! then the disk gather. Runs between shade and tonemap. Zero cost
//! when off: nothing is dispatched; textures are allocated lazily.

use egui_wgpu::wgpu::*;

/// Mirrors WGSL `DofParams` (dof.wgsl) — 32 bytes.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct DofParams {
    width: u32,
    height: u32,
    depth_word_offset: u32,
    radius: u32,
    focus: f32,
    coc_scale: f32,
    surface_thickness: f32,
    _pad0: f32,
    dir_x: i32,
    dir_y: i32,
    _pad1: i32,
    _pad2: i32,
}

/// CoC clamp in pixels — bounds both the per-pass kernel width and how
/// far a firefly can smear.
const MAX_RADIUS: u32 = 32;

pub struct DofPass {
    pipeline: ComputePipeline,
    coc_pipeline: ComputePipeline,
    max_pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
    /// One uniform buffer per dispatch — queue writes flatten before
    /// dispatches, so the passes need distinct buffers.
    params_h: Buffer,
    params_v: Buffer,
    params_coc: Buffer,
    /// Lazily (re)created full-image textures [CoC field, max scratch,
    /// max-reach field, output]; sized (w, h) when present.
    texs: Option<(u32, u32, [(Texture, TextureView); 4])>,
}

impl DofPass {
    pub fn new(device: &Device) -> Self {
        let shader = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("Solid DoF Shader"),
            source: ShaderSource::Wgsl(include_str!("../../shaders/dof.wgsl").into()),
        });
        let bind_group_layout = device.create_bind_group_layout(&BindGroupLayoutDescriptor {
            label: Some("DoF Bind Group Layout"),
            entries: &[
                // 0: HDR input (Rgba32Float, textureLoad — non-filterable)
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
                // 1: depth buffer (read-only view of the histogram)
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
                // 2: DofParams uniform
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
                // 3: pass output (storage write)
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
                // 4: smoothed CoC field (stand-in for passes that
                // don't read it)
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
                // 5: dilated max-reach field (gather pass only)
                BindGroupLayoutEntry {
                    binding: 5,
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
        let layout = device.create_pipeline_layout(&PipelineLayoutDescriptor {
            label: Some("DoF Pipeline Layout"),
            bind_group_layouts: &[Some(&bind_group_layout)],
            immediate_size: 0,
        });
        let pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("DoF Pipeline"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("dof_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let coc_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("DoF CoC Pipeline"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("coc_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let max_pipeline = device.create_compute_pipeline(&ComputePipelineDescriptor {
            label: Some("DoF Max Pipeline"),
            layout: Some(&layout),
            module: &shader,
            entry_point: Some("max_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let mk_params = |label: &str| {
            device.create_buffer(&BufferDescriptor {
                label: Some(label),
                size: std::mem::size_of::<DofParams>() as u64,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            })
        };
        Self {
            pipeline,
            coc_pipeline,
            max_pipeline,
            bind_group_layout,
            params_h: mk_params("DoF Params H"),
            params_v: mk_params("DoF Params V"),
            params_coc: mk_params("DoF Params CoC"),
            texs: None,
        }
    }

    fn ensure_texs(&mut self, device: &Device, width: u32, height: u32) {
        if matches!(&self.texs, Some((w, h, _)) if *w == width && *h == height) {
            return;
        }
        let mk = |label: &str| {
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
        };
        self.texs = Some((width, height, [
            mk("DoF CoC Field"),
            mk("DoF Max Scratch"),
            mk("DoF Max Reach"),
            mk("DoF Output"),
        ]));
    }

    pub fn output_view(&self) -> &TextureView {
        &self.texs.as_ref().expect("DofPass ran before textures were created").2[3].1
    }

    /// Encode the four passes: CoC prepass, max-dilate H + V, then the
    /// dense 2-D disk gather into the internal output. CoC matches the
    /// at-splat formula (see dof.wgsl).
    #[allow(clippy::too_many_arguments)]
    pub fn run(
        &mut self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        input_view: &TextureView,
        depth_buffer: &Buffer,
        depth_word_offset: u32,
        width: u32,
        height: u32,
        zoom: f32,
        focus: f32,
        strength: f32,
        surface_thickness: f32,
    ) {
        self.ensure_texs(device, width, height);
        let coc_scale = strength * 0.1 * (width.min(height) as f32) * 0.25 * zoom;
        let params = |dir_x: i32, dir_y: i32| DofParams {
            width,
            height,
            depth_word_offset,
            radius: MAX_RADIUS,
            focus,
            coc_scale,
            surface_thickness,
            _pad0: 0.0,
            dir_x,
            dir_y,
            _pad1: 0,
            _pad2: 0,
        };
        queue.write_buffer(&self.params_coc, 0, bytemuck::bytes_of(&params(0, 0)));
        queue.write_buffer(&self.params_h, 0, bytemuck::bytes_of(&params(1, 0)));
        queue.write_buffer(&self.params_v, 0, bytemuck::bytes_of(&params(0, 1)));
        let (_, _, texs) = self.texs.as_ref().unwrap();
        let bg = |input: &TextureView, params_buf: &Buffer, output: &TextureView, coc: &TextureView, max: &TextureView, label: &str| {
            device.create_bind_group(&BindGroupDescriptor {
                label: Some(label),
                layout: &self.bind_group_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(input),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: depth_buffer.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: params_buf.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 3,
                        resource: BindingResource::TextureView(output),
                    },
                    BindGroupEntry {
                        binding: 4,
                        resource: BindingResource::TextureView(coc),
                    },
                    BindGroupEntry {
                        binding: 5,
                        resource: BindingResource::TextureView(max),
                    },
                ],
            })
        };
        // texs: [0]=CoC field, [1]=max scratch, [2]=max reach, [3]=out.
        // Each pass's storage output must differ from every texture it
        // samples (aliasing rules); unused sampled slots get stand-ins.
        let bg_coc = bg(input_view, &self.params_coc, &texs[0].1, &texs[1].1, &texs[2].1, "DoF CoC Bind Group");
        let bg_max_h = bg(input_view, &self.params_h, &texs[1].1, &texs[0].1, &texs[2].1, "DoF Max H Bind Group");
        let bg_max_v = bg(input_view, &self.params_v, &texs[2].1, &texs[1].1, &texs[0].1, "DoF Max V Bind Group");
        let bg_blur = bg(input_view, &self.params_coc, &texs[3].1, &texs[0].1, &texs[2].1, "DoF Gather Bind Group");
        for (pipeline, bg, label) in [
            (&self.coc_pipeline, &bg_coc, "Solid DoF CoC Pass"),
            (&self.max_pipeline, &bg_max_h, "Solid DoF Max H Pass"),
            (&self.max_pipeline, &bg_max_v, "Solid DoF Max V Pass"),
            (&self.pipeline, &bg_blur, "Solid DoF Gather Pass"),
        ] {
            let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
                label: Some(label),
                timestamp_writes: None,
            });
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, bg, &[]);
            pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
        }
    }
}
