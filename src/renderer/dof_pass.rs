//! Post-process depth-of-field pass for solid rendering.
//!
//! Host side of `shaders/dof.wgsl`: gathers a depth-weighted blur of
//! the HDR pre-tonemap image (shade output when lighting is on, else
//! the accumulator) using the solid pipeline's per-pixel nearest-depth
//! region. Runs between shade and tonemap. Zero cost when off: the
//! pass is simply not dispatched, and the output texture is allocated
//! lazily on first use.

use egui_wgpu::wgpu::*;

/// Mirrors WGSL `DofParams` (dof.wgsl) — 32 bytes.
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
struct DofParams {
    width: u32,
    height: u32,
    depth_word_offset: u32,
    taps: u32,
    focus: f32,
    coc_scale: f32,
    max_radius: f32,
    _pad0: f32,
}

/// CoC clamp in pixels — bounds both the gather cost and how far a
/// firefly can smear.
const MAX_RADIUS: f32 = 24.0;

pub struct DofPass {
    pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
    params_buffer: Buffer,
    /// Lazily (re)created full-image output; sized (w, h) when present.
    output: Option<(u32, u32, Texture, TextureView)>,
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
                // 3: blurred output (storage write)
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
        let params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("DoF Params"),
            size: std::mem::size_of::<DofParams>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            bind_group_layout,
            params_buffer,
            output: None,
        }
    }

    fn ensure_output(&mut self, device: &Device, width: u32, height: u32) {
        if matches!(&self.output, Some((w, h, _, _)) if *w == width && *h == height) {
            return;
        }
        let tex = device.create_texture(&TextureDescriptor {
            label: Some("DoF Output"),
            size: Extent3d { width, height, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba32Float,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let view = tex.create_view(&TextureViewDescriptor::default());
        self.output = Some((width, height, tex, view));
    }

    pub fn output_view(&self) -> &TextureView {
        &self.output.as_ref().expect("DofPass ran before output was created").3
    }

    /// Encode the blur: `input_view` (full-image HDR) + the depth region
    /// at `depth_word_offset` inside `depth_buffer` → internal output.
    /// CoC matches the at-splat formula (see dof.wgsl).
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
    ) {
        self.ensure_output(device, width, height);
        let coc_scale = strength * 0.1 * (width.min(height) as f32) * 0.25 * zoom;
        let params = DofParams {
            width,
            height,
            depth_word_offset,
            // More taps at wide radii; a light kernel when nearly sharp.
            taps: 32,
            focus,
            coc_scale,
            max_radius: MAX_RADIUS,
            _pad0: 0.0,
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));
        let bind_group = device.create_bind_group(&BindGroupDescriptor {
            label: Some("DoF Bind Group"),
            layout: &self.bind_group_layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: BindingResource::TextureView(input_view),
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
                    resource: BindingResource::TextureView(self.output_view()),
                },
            ],
        });
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("Solid DoF Pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
    }
}
