use wgpu::*;
use wgpu::util::DeviceExt;
use crate::scene::transforms::{Transform, Flame};
use crate::scene::palette::Palette;

/// Maximum number of transforms supported (buffer is pre-allocated for this many)
pub const MAX_TRANSFORMS: usize = 32;

/// Maximum parameters per variation (expandable if needed)
pub const MAX_PARAMS_PER_VARIATION: usize = 8;

/// GPU representation of Transform (must match WGSL struct layout)
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct GpuTransform {
    // Affine matrix
    pub a: f32,
    pub b: f32,
    pub c: f32,
    pub d: f32,
    pub e: f32,
    pub f: f32,
    pub g: f32, // Z offset for 3D mode
    pub weight: f32,

    // Variations (50 floats: future-proof for plugins)
    pub variations: [f32; 50],

    // Padding to align color to 16-byte boundary (WGSL std430 vec3 alignment)
    _pad1: f32,
    _pad2: f32,

    // Color (vec3<f32> in WGSL requires 16-byte alignment)
    pub color: [f32; 3],
    pub color_speed: f32,
}

// Manual implementation for bytemuck (arrays of size 50 not auto-derived)
unsafe impl bytemuck::Pod for GpuTransform {}
unsafe impl bytemuck::Zeroable for GpuTransform {}

impl GpuTransform {
    /// Create from Transform using a VariationRegistry
    pub fn from_transform(xform: &Transform, registry: &crate::variations::VariationRegistry) -> Self {
        Self {
            a: xform.a,
            b: xform.b,
            c: xform.c,
            d: xform.d,
            e: xform.e,
            f: xform.f,
            g: xform.g,
            weight: xform.weight,
            variations: xform.to_fixed_array(registry),
            _pad1: 0.0,
            _pad2: 0.0,
            color: xform.color,
            color_speed: xform.color_speed,
        }
    }
}

/// GPU representation of variation parameters for ONE transform
/// Total size: 50 variations × 8 params = 400 floats = 1600 bytes per transform
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct GpuVariationParams {
    /// Flat array indexed by: variation_id * MAX_PARAMS_PER_VARIATION + param_slot
    /// Each variation gets MAX_PARAMS_PER_VARIATION consecutive slots
    pub params: [f32; 400],  // 50 variations × 8 params
}

// Manual implementation for bytemuck (arrays > 128 not auto-derived)
unsafe impl bytemuck::Pod for GpuVariationParams {}
unsafe impl bytemuck::Zeroable for GpuVariationParams {}

impl GpuVariationParams {
    /// Create from Transform using VariationRegistry
    pub fn from_transform(
        xform: &Transform,
        registry: &crate::variations::VariationRegistry,
    ) -> Self {
        let mut params = [0.0f32; 400];

        // For each active variation, copy its parameters
        for (var_name, _weight) in &xform.variations {
            if let Some(info) = registry.get(var_name) {
                // Get variation ID from registry
                let var_id = registry.names()
                    .iter()
                    .position(|n| n == var_name)
                    .unwrap_or(0);

                // Copy each parameter for this variation
                for (param_idx, param_def) in info.parameters.iter().enumerate() {
                    if param_idx >= MAX_PARAMS_PER_VARIATION {
                        break;  // Safety check
                    }

                    // Get parameter value (or default)
                    let value = xform.get_variation_param_or_default(
                        var_name,
                        &param_def.name,
                        registry,
                    );

                    // Write to buffer at correct index
                    let buffer_idx = var_id * MAX_PARAMS_PER_VARIATION + param_idx;
                    params[buffer_idx] = value;
                }
            }
        }

        Self { params }
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
    pub color_mode: u32, // 0 = transform colors, 1 = palette, 2 = speed
    pub render_mode: u32, // 0 = 2D, 1 = 3D
    pub projection_type: u32, // 0 = orthographic, 1 = perspective
    pub splat_size: f32,
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub rotation: f32, // Rotation in radians (2D rotation around Z)
    pub speed_factor: f32, // Blend factor for speed-based coloring
    pub perspective_strength: f32, // Strength for perspective projection
    pub camera_rotation_x: f32, // 3D camera pitch (rotation around X axis)
    pub camera_rotation_y: f32, // 3D camera yaw (rotation around Y axis)
    pub _pad3: f32,
    pub _pad4: f32,
}

/// Tonemap parameters
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TonemapParams {
    pub exposure: f32,
    pub gamma: f32,
    pub density_scale: f32, // Controls how density maps to alpha
    pub tonemap_mode: u32,  // 0 = Linear, 1 = Logarithmic
    pub background_color: [f32; 3],
    pub use_curve: u32,  // 0 = disabled, 1 = enabled
}

impl Default for TonemapParams {
    fn default() -> Self {
        Self {
            exposure: 1.0,
            gamma: 2.2,
            density_scale: 1.0,
            tonemap_mode: 1,  // Default to Logarithmic
            background_color: [0.0, 0.0, 0.0],
            use_curve: 0,  // Curves disabled by default
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
    pub variation_params_buffer: Buffer,  // NEW: Parameter buffer for variations
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

    // Tone curve LUT texture (1D, 256 samples)
    pub curve_lut_texture: Texture,
    pub curve_lut_view: TextureView,

    pub sampler: Sampler,
    pub curve_lut_sampler: Sampler,  // Separate sampler for curve LUT (nearest neighbor)

    // Track which texture is current for display
    pub current_is_a: bool,
}

impl FlameBuffers {
    pub fn new(device: &Device, queue: &Queue, width: u32, height: u32, flame: &Flame) -> Self {
        // Create transform storage buffer sized for MAX_TRANSFORMS
        // This allows loading presets with different numbers of transforms without recreating the buffer
        let buffer_size = (MAX_TRANSFORMS * std::mem::size_of::<GpuTransform>()) as u64;
        let transform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Transform Buffer"),
            size: buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Upload initial transforms
        let registry = crate::variations::global_registry();
        let gpu_transforms: Vec<GpuTransform> = flame
            .transforms
            .iter()
            .map(|xform| GpuTransform::from_transform(xform, registry))
            .collect();
        queue.write_buffer(&transform_buffer, 0, bytemuck::cast_slice(&gpu_transforms));

        // Create variation parameters storage buffer sized for MAX_TRANSFORMS
        let params_buffer_size = (MAX_TRANSFORMS * std::mem::size_of::<GpuVariationParams>()) as u64;
        let variation_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Variation Params Buffer"),
            size: params_buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Upload initial variation parameters
        let gpu_params: Vec<GpuVariationParams> = flame
            .transforms
            .iter()
            .map(|xform| GpuVariationParams::from_transform(xform, registry))
            .collect();
        queue.write_buffer(&variation_params_buffer, 0, bytemuck::cast_slice(&gpu_params));

        // Create params uniform buffer
        let params = GpuParams {
            num_transforms: flame.transforms.len() as u32,
            iterations_per_thread: 2048, // More iterations for better coverage per frame
            burn_in: 20,
            width,
            height,
            seed: 12345,
            color_mode: 0, // Default to transform colors
            render_mode: 0, // Default to 2D
            projection_type: 0, // Default to orthographic
            splat_size: 1.0,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            rotation: 0.0,
            speed_factor: 0.5,
            perspective_strength: 2.0,
            camera_rotation_x: 0.0,
            camera_rotation_y: 0.0,
            _pad3: 0.0,
            _pad4: 0.0,
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
            // WASM needs RENDER_ATTACHMENT for clear_texture_wasm() to work
            // Desktop uses CLEAR_TEXTURE feature instead
            #[cfg(target_arch = "wasm32")]
            let usage = TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC | TextureUsages::RENDER_ATTACHMENT;

            #[cfg(not(target_arch = "wasm32"))]
            let usage = TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::COPY_SRC;

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
                usage,
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
        // Use Rgba8Unorm for efficient, standard color storage
        let default_palette = Palette::fire(); // Default palette
        let palette_data = default_palette.generate_texture_data(256);

        // Convert f32 [0.0, 1.0] to u8 [0, 255] for Rgba8Unorm
        let palette_data_u8: Vec<u8> = palette_data
            .iter()
            .map(|&v| (v.clamp(0.0, 1.0) * 255.0) as u8)
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
            format: TextureFormat::Rgba8Unorm,
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
            &palette_data_u8,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(256 * 4), // 256 pixels * 4 components * 1 byte
                rows_per_image: None,
            },
            Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let palette_view = palette_texture.create_view(&TextureViewDescriptor::default());

        // Create curve LUT texture (1D, 256 samples) - start with linear curve
        let default_curve = crate::scene::tonemap::ToneCurve::linear();
        let curve_lut_data = default_curve.generate_lut();

        let curve_lut_texture = device.create_texture(&TextureDescriptor {
            label: Some("Curve LUT Texture"),
            size: Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D1,
            format: TextureFormat::Rgba16Float,  // Use 16-bit float for precision (filterable)
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload curve LUT data
        queue.write_texture(
            ImageCopyTexture {
                texture: &curve_lut_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &curve_lut_data,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: None,  // 1D textures don't have rows
                rows_per_image: None,
            },
            Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
        );

        let curve_lut_view = curve_lut_texture.create_view(&TextureViewDescriptor::default());

        // Create sampler for tonemap shader (accumulation texture - needs linear filtering)
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

        // Create separate sampler for curve LUT (linear filtering for smooth interpolation)
        let curve_lut_sampler = device.create_sampler(&SamplerDescriptor {
            label: Some("Curve LUT Sampler"),
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
            variation_params_buffer,
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
            curve_lut_texture,
            curve_lut_view,
            sampler,
            curve_lut_sampler,
            current_is_a: true,
        }
    }

    /// Clear all accumulation buffers
    pub fn clear_all(&self, encoder: &mut CommandEncoder) {
        // Use CLEAR_TEXTURE feature if available (desktop usually has it)
        #[cfg(not(target_arch = "wasm32"))]
        {
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

        // WASM: Clear textures by rendering black to them
        // This is more compatible than CLEAR_TEXTURE which may not be supported
        #[cfg(target_arch = "wasm32")]
        {
            self.clear_texture_wasm(encoder, &self.accumulation_view_a);
            self.clear_texture_wasm(encoder, &self.accumulation_view_b);
            self.clear_texture_wasm(encoder, &self.temp_samples_view);
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn clear_texture_wasm(&self, encoder: &mut CommandEncoder, texture_view: &TextureView) {
        // Use a render pass to clear the texture
        // This is more compatible than clear_texture() which may not work in WebGPU
        // Clear to all zeros: (0,0,0,0) - zero RGB color and zero alpha (density)
        let mut render_pass = encoder.begin_render_pass(&RenderPassDescriptor {
            label: Some("Clear Texture Pass"),
            color_attachments: &[Some(RenderPassColorAttachment {
                view: texture_view,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.0,
                        a: 0.0,
                    }),
                    store: StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        drop(render_pass); // End the render pass immediately
    }

    /// Clear temp samples texture only
    pub fn clear_temp(&self, encoder: &mut CommandEncoder) {
        #[cfg(not(target_arch = "wasm32"))]
        {
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

        // WASM: Don't clear temp texture - it's a storage texture that can't be cleared with render pass
        // The trajectory compute shader writes to all pixels anyway, so clearing isn't necessary
        #[cfg(target_arch = "wasm32")]
        {
            // No-op for WASM
        }
    }

    /// Get the current accumulation texture view (for display)
    pub fn current_accumulation_view(&self) -> &TextureView {
        if self.current_is_a {
            &self.accumulation_view_a
        } else {
            &self.accumulation_view_b
        }
    }

    /// Get the current accumulation texture (for copy operations)
    pub fn current_accumulation_texture(&self) -> &Texture {
        if self.current_is_a {
            &self.accumulation_texture_a
        } else {
            &self.accumulation_texture_b
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
        if flame.transforms.len() > MAX_TRANSFORMS {
            panic!("Flame has {} transforms but MAX_TRANSFORMS is {}", flame.transforms.len(), MAX_TRANSFORMS);
        }

        // Create a fixed-size array with all transforms, padding with zeroes
        let registry = crate::variations::global_registry();
        let mut gpu_transforms: Vec<GpuTransform> = flame
            .transforms
            .iter()
            .map(|xform| GpuTransform::from_transform(xform, registry))
            .collect();

        // Pad with zeroed transforms to fill the buffer
        // This ensures old transforms don't remain in GPU memory when switching to fewer transforms
        while gpu_transforms.len() < MAX_TRANSFORMS {
            gpu_transforms.push(bytemuck::Zeroable::zeroed());
        }

        queue.write_buffer(&self.transform_buffer, 0, bytemuck::cast_slice(&gpu_transforms));
    }

    /// Update variation parameters
    pub fn update_variation_params(&self, queue: &Queue, flame: &Flame) {
        if flame.transforms.len() > MAX_TRANSFORMS {
            panic!("Flame has {} transforms but MAX_TRANSFORMS is {}", flame.transforms.len(), MAX_TRANSFORMS);
        }

        // Create a fixed-size array with all variation parameters, padding with zeroes
        let registry = crate::variations::global_registry();
        let mut gpu_params: Vec<GpuVariationParams> = flame
            .transforms
            .iter()
            .map(|xform| GpuVariationParams::from_transform(xform, registry))
            .collect();

        // Pad with zeroed params to fill the buffer
        while gpu_params.len() < MAX_TRANSFORMS {
            gpu_params.push(bytemuck::Zeroable::zeroed());
        }

        queue.write_buffer(&self.variation_params_buffer, 0, bytemuck::cast_slice(&gpu_params));
    }

    /// Update palette texture
    pub fn update_palette(&self, queue: &Queue, palette: &Palette) {
        let palette_data = palette.generate_texture_data(256);

        // Convert f32 [0.0, 1.0] to u8 [0, 255] for Rgba8Unorm
        let palette_data_u8: Vec<u8> = palette_data
            .iter()
            .map(|&v| (v.clamp(0.0, 1.0) * 255.0) as u8)
            .collect();

        queue.write_texture(
            ImageCopyTexture {
                texture: &self.palette_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &palette_data_u8,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: Some(256 * 4), // 256 pixels * 4 components * 1 byte
                rows_per_image: None,
            },
            Extent3d {
                width: 256,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Update tone curve LUT texture
    pub fn update_curve_lut(&self, queue: &Queue, curve: &crate::scene::tonemap::ToneCurve) {
        let curve_lut_data = curve.generate_lut();

        queue.write_texture(
            ImageCopyTexture {
                texture: &self.curve_lut_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &curve_lut_data,
            ImageDataLayout {
                offset: 0,
                bytes_per_row: None,  // 1D textures don't have rows
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
