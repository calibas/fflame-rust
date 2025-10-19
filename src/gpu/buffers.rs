use wgpu::*;
use wgpu::util::DeviceExt;
use crate::scene::transforms::{Transform, Flame};
use crate::scene::palette::Palette;

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
    pub color_mode: u32, // 0 = transform colors, 1 = palette
    pub splat_size: f32,
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub _pad0: f32,
}

/// Tonemap parameters
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TonemapParams {
    pub exposure: f32,
    pub gamma: f32,
    pub density_scale: f32, // Controls how density maps to alpha
    pub _pad0: f32,
}

impl Default for TonemapParams {
    fn default() -> Self {
        Self {
            exposure: 1.0,
            gamma: 2.2,
            density_scale: 1.0,
            _pad0: 0.0,
        }
    }
}

/// Accumulation parameters
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct AccumulateParams {
    pub width: u32,
    pub height: u32,
    pub blend_factor: f32,
    pub _pad0: f32,
}

/// Manages GPU buffers and textures for fractal flame rendering
pub struct FlameBuffers {
    pub transform_buffer: Buffer,
    pub params_buffer: Buffer,
    pub tonemap_params_buffer: Buffer,
    pub accumulate_params_buffer: Buffer,

    // Dual textures for ping-pong accumulation
    pub accumulation_texture_a: Texture,
    pub accumulation_texture_b: Texture,
    pub accumulation_view_a: TextureView,
    pub accumulation_view_b: TextureView,

    // Temp texture for new samples (written by trajectory shader)
    pub temp_samples_texture: Texture,
    pub temp_samples_view: TextureView,

    // Palette texture (1D)
    pub palette_texture: Texture,
    pub palette_view: TextureView,

    pub sampler: Sampler,

    // Track which texture is current for display
    pub current_is_a: bool,
}

impl FlameBuffers {
    pub fn new(device: &Device, queue: &Queue, width: u32, height: u32, flame: &Flame) -> Self {
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
            color_mode: 0, // Default to transform colors
            splat_size: 1.0,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
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

        // Create accumulate params buffer
        let accumulate_params = AccumulateParams {
            width,
            height,
            blend_factor: 1.0,
            _pad0: 0.0,
        };
        let accumulate_params_buffer = device.create_buffer_init(&util::BufferInitDescriptor {
            label: Some("Accumulate Params Buffer"),
            contents: bytemuck::cast_slice(&[accumulate_params]),
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
        });

        // Helper function to create accumulation texture
        let create_accum_texture = |label: &str| {
            let texture = device.create_texture(&TextureDescriptor {
                label: Some(label),
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
            let view = texture.create_view(&TextureViewDescriptor::default());
            (texture, view)
        };

        // Create dual accumulation textures for ping-pong
        let (accumulation_texture_a, accumulation_view_a) = create_accum_texture("Accumulation Texture A");
        let (accumulation_texture_b, accumulation_view_b) = create_accum_texture("Accumulation Texture B");

        // Create temp samples texture (written by trajectory shader)
        let (temp_samples_texture, temp_samples_view) = create_accum_texture("Temp Samples Texture");

        // Create palette texture (1D, 256 samples)
        // Use Rgba16Float instead of Rgba32Float for guaranteed filterability
        let default_palette = Palette::fire(); // Default palette
        let palette_data = default_palette.generate_texture_data(256);

        // Convert f32 to f16 for Rgba16Float
        let palette_data_f16: Vec<u8> = palette_data.chunks(4)
            .flat_map(|chunk| {
                let r = half::f16::from_f32(chunk[0]);
                let g = half::f16::from_f32(chunk[1]);
                let b = half::f16::from_f32(chunk[2]);
                let a = half::f16::from_f32(chunk[3]);
                [
                    r.to_bits().to_le_bytes(),
                    g.to_bits().to_le_bytes(),
                    b.to_bits().to_le_bytes(),
                    a.to_bits().to_le_bytes(),
                ].concat()
            })
            .collect();

        let palette_texture = device.create_texture(&TextureDescriptor {
            label: Some("Palette Texture"),
            size: Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D1,
            format: TextureFormat::Rgba16Float,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload palette data
        queue.write_texture(
            ImageCopyTexture {
                texture: &palette_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &palette_data_f16,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(256 * 4 * 2), // 256 pixels * 4 components * 2 bytes (f16)
                rows_per_image: None,
            },
            Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let palette_view = palette_texture.create_view(&TextureViewDescriptor::default());

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
            accumulate_params_buffer,
            accumulation_texture_a,
            accumulation_texture_b,
            accumulation_view_a,
            accumulation_view_b,
            temp_samples_texture,
            temp_samples_view,
            palette_texture,
            palette_view,
            sampler,
            current_is_a: true,
        }
    }

    /// Clear all accumulation buffers
    pub fn clear_all(&self, encoder: &mut CommandEncoder) {
        let range = ImageSubresourceRange {
            aspect: TextureAspect::All,
            base_mip_level: 0,
            mip_level_count: None,
            base_array_layer: 0,
            array_layer_count: None,
        };
        encoder.clear_texture(&self.accumulation_texture_a, &range);
        encoder.clear_texture(&self.accumulation_texture_b, &range);
        encoder.clear_texture(&self.temp_samples_texture, &range);
    }

    /// Clear temp samples texture only
    pub fn clear_temp(&self, encoder: &mut CommandEncoder) {
        encoder.clear_texture(
            &self.temp_samples_texture,
            &ImageSubresourceRange {
                aspect: TextureAspect::All,
                base_mip_level: 0,
                mip_level_count: None,
                base_array_layer: 0,
                array_layer_count: None,
            },
        );
    }

    /// Get the current accumulation texture view (for display)
    pub fn current_accumulation_view(&self) -> &TextureView {
        if self.current_is_a {
            &self.accumulation_view_a
        } else {
            &self.accumulation_view_b
        }
    }

    /// Get the previous accumulation texture view (for reading in accumulation shader)
    pub fn previous_accumulation_view(&self) -> &TextureView {
        if self.current_is_a {
            &self.accumulation_view_b
        } else {
            &self.accumulation_view_a
        }
    }

    /// Get the output accumulation texture view (for writing in accumulation shader)
    pub fn output_accumulation_view(&self) -> &TextureView {
        if self.current_is_a {
            &self.accumulation_view_a
        } else {
            &self.accumulation_view_b
        }
    }

    /// Swap which texture is current
    pub fn swap_textures(&mut self) {
        self.current_is_a = !self.current_is_a;
    }

    /// Update parameters (e.g., when changing resolution or settings)
    pub fn update_params(&self, queue: &Queue, params: &GpuParams) {
        queue.write_buffer(&self.params_buffer, 0, bytemuck::cast_slice(&[*params]));
    }

    /// Update tonemap parameters
    pub fn update_tonemap_params(&self, queue: &Queue, params: &TonemapParams) {
        queue.write_buffer(&self.tonemap_params_buffer, 0, bytemuck::cast_slice(&[*params]));
    }

    /// Update accumulate parameters
    pub fn update_accumulate_params(&self, queue: &Queue, params: &AccumulateParams) {
        queue.write_buffer(&self.accumulate_params_buffer, 0, bytemuck::cast_slice(&[*params]));
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

    /// Update palette texture
    pub fn update_palette(&self, queue: &Queue, palette: &Palette) {
        let palette_data = palette.generate_texture_data(256);

        // Convert f32 to f16 for Rgba16Float
        let palette_data_f16: Vec<u8> = palette_data.chunks(4)
            .flat_map(|chunk| {
                let r = half::f16::from_f32(chunk[0]);
                let g = half::f16::from_f32(chunk[1]);
                let b = half::f16::from_f32(chunk[2]);
                let a = half::f16::from_f32(chunk[3]);
                [
                    r.to_bits().to_le_bytes(),
                    g.to_bits().to_le_bytes(),
                    b.to_bits().to_le_bytes(),
                    a.to_bits().to_le_bytes(),
                ].concat()
            })
            .collect();

        queue.write_texture(
            ImageCopyTexture {
                texture: &self.palette_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &palette_data_f16,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(256 * 4 * 2), // 256 pixels * 4 components * 2 bytes (f16)
                rows_per_image: None,
            },
            Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }
}
