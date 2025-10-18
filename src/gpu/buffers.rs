use wgpu::*;
use wgpu::util::DeviceExt;
use crate::scene::transforms::{Transform, Flame};

/// GPU representation of Transform (must match WGSL struct layout)
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuTransform {
    // Affine matrix
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
    pub weight: f32,
    pub _pad0: f32,

    // Variations (16 floats)
    pub variations: [f32; 16],

    // Color
    pub color: [f32; 3],
    pub color_speed: f32,
}

impl From<&Transform> for GpuTransform {
    fn from(xform: &Transform) -> Self {
        Self {
            a: xform.a,
            b: xform.b,
            c: xform.c,
            d: xform.d,
            e: xform.e,
            f: xform.f,
            weight: xform.weight,
            _pad0: 0.0,
            variations: xform.variations,
            color: xform.color,
            color_speed: xform.color_speed,
        }
    }
}

/// Dispatch parameters for compute shader
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuParams {
    pub num_transforms: u32,
    pub iterations_per_thread: u32,
    pub burn_in: u32,
    pub width: u32,
    pub height: u32,
    pub seed: u32,
    pub splat_size: f32,
    pub _pad0: f32,
}

/// Tonemap parameters
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TonemapParams {
    pub exposure: f32,
    pub gamma: f32,
    pub _pad0: f32,
    pub _pad1: f32,
}

impl Default for TonemapParams {
    fn default() -> Self {
        Self {
            exposure: 1.0,
            gamma: 2.2,
            _pad0: 0.0,
            _pad1: 0.0,
        }
    }
}

/// Manages GPU buffers and textures for fractal flame rendering
pub struct FlameBuffers {
    pub transform_buffer: Buffer,
    pub params_buffer: Buffer,
    pub tonemap_params_buffer: Buffer,
    pub accumulation_texture: Texture,
    pub accumulation_view: TextureView,
    pub sampler: Sampler,
}

impl FlameBuffers {
    pub fn new(device: &Device, width: u32, height: u32, flame: &Flame) -> Self {
        // Convert transforms to GPU format
        let gpu_transforms: Vec<GpuTransform> = flame
            .transforms
            .iter()
            .map(|xform| xform.into())
            .collect();

        // Create transform storage buffer
        let transform_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Transform Buffer"),
            contents: bytemuck::cast_slice(&gpu_transforms),
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
        });

        // Create params uniform buffer
        let params = GpuParams {
            num_transforms: flame.transforms.len() as u32,
            iterations_per_thread: 2048, // More iterations for better coverage per frame
            burn_in: 20,
            width,
            height,
            seed: 12345,
            splat_size: 1.0,
            _pad0: 0.0,
        };

        let params_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Params Buffer"),
            contents: bytemuck::cast_slice(&[params]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        // Create tonemap params buffer
        let tonemap_params = TonemapParams::default();
        let tonemap_params_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Tonemap Params Buffer"),
            contents: bytemuck::cast_slice(&[tonemap_params]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        // Create accumulation texture (RGBA16Float for good precision and filterability)
        let accumulation_texture = device.create_texture(&TextureDescriptor {
            label: Some("Accumulation Texture"),
            size: Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba16Float,
            usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let accumulation_view = accumulation_texture.create_view(&TextureViewDescriptor::default());

        // Create sampler for tonemap shader
        let sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Accumulation Sampler"),
            address_mode_u: AddressMode::ClampToEdge,
            address_mode_v: AddressMode::ClampToEdge,
            address_mode_w: AddressMode::ClampToEdge,
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            mipmap_filter: FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            transform_buffer,
            params_buffer,
            tonemap_params_buffer,
            accumulation_texture,
            accumulation_view,
            sampler,
        }
    }

    /// Clear accumulation buffer
    pub fn clear(&self, encoder: &mut CommandEncoder) {
        encoder.clear_texture(
            &self.accumulation_texture,
            &ImageSubresourceRange {
                aspect: TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            },
        );
    }

    /// Update parameters (e.g., when changing resolution or settings)
    pub fn update_params(&self, queue: &Queue, params: &GpuParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[*params]));
    }

    /// Update tonemap parameters
    pub fn update_tonemap_params(&self, queue: &Queue, params: &TonemapParams) {
        queue.write_buffer(&self.tonemap_params_buffer, 0, bytemuck::cast_slice(&[*params]));
    }

    /// Update transforms
    pub fn update_transforms(&self, queue: &Queue, flame: &Flame) {
        let gpu_transforms: Vec<GpuTransform> = flame
            .transforms
            .iter()
            .map(|xform| xform.into())
            .collect();
        queue.write_buffer(&self.transform_buffer, 0, bytemuck::cast_slice(&gpu_transforms));
    }
}
