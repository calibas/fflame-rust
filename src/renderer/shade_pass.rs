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
    _pad0: u32,
    _pad1: u32,
    lights: [ShadeLight; 4],
}

pub struct ShadePass {
    pipeline: ComputePipeline,
    bind_group_layout: BindGroupLayout,
    params_buffer: Buffer,
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

        let output = Some(Self::create_output(device, width, height));

        Self {
            pipeline,
            bind_group_layout,
            params_buffer,
            output,
            width,
            height,
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
            _pad0: 0,
            _pad1: 0,
            lights,
        };
        queue.write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

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
