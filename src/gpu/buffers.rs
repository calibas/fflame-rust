use egui_wgpu::wgpu::*;
use egui_wgpu::wgpu::util::DeviceExt;
use crate::scene::transforms::{Transform, Flame};
use crate::scene::palette::Palette;

/// Maximum number of transforms supported (buffer is pre-allocated for this many)
pub const MAX_TRANSFORMS: usize = 32;

/// Maximum parameters per variation (expandable if needed)
pub const MAX_PARAMS_PER_VARIATION: usize = 12;

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

    // Variations (100 floats: supports all Apophysis 7X + future expansion)
    pub variations: [f32; 100],

    // Color (palette position) + color_speed + opacity + padding (forms vec4 for alignment)
    pub color: f32,
    pub color_speed: f32,
    pub opacity: f32,
    pub _padding: f32,
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
            color: xform.color,
            color_speed: xform.color_speed,
            opacity: xform.opacity,
            _padding: 0.0,
        }
    }
}

/// GPU representation of variation parameters for ONE transform
/// Total size: 100 variations × 12 params = 1200 floats = 4800 bytes per transform
#[repr(C)]
#[derive(Copy, Clone, Debug)]
pub struct GpuVariationParams {
    /// Flat array indexed by: variation_id * MAX_PARAMS_PER_VARIATION + param_slot
    /// Each variation gets MAX_PARAMS_PER_VARIATION consecutive slots
    pub params: [f32; 1200],  // 100 variations × 12 params
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
        let mut params = [0.0f32; 1200];

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

/// Calculate bits needed per transform index based on transform count
/// - 1-2 transforms: 1 bit
/// - 3-4 transforms: 2 bits
/// - 5-8 transforms: 3 bits
/// - 9-16 transforms: 4 bits
/// - 17-32 transforms: 5 bits
pub fn bits_per_transform(num_transforms: u32) -> u32 {
    if num_transforms <= 2 {
        1
    } else if num_transforms <= 4 {
        2
    } else if num_transforms <= 8 {
        3
    } else if num_transforms <= 16 {
        4
    } else {
        5  // Up to 32 transforms
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
    pub color_mode: u32, // 0 = palette, 1 = speed, 2 = path_map
    pub render_mode: u32, // 0 = 2D, 1 = 3D
    pub splat_size: f32,
    pub zoom: f32,
    pub pan_x: f32,
    pub pan_y: f32,
    pub rotation: f32, // Rotation in radians (2D rotation around Z)
    pub speed_factor: f32, // Blend factor for speed-based coloring
    pub perspective_strength: f32, // Strength for perspective projection
    pub camera_rotation_x: f32, // 3D camera pitch (rotation around X axis)
    pub camera_rotation_y: f32, // 3D camera yaw (rotation around Y axis)
    pub camera_z: f32, // 3D camera Z position (height)
    pub histogram_color_scale: f32, // Precision vs overflow (default: 10.0)
    pub has_final_transform: u32, // 0 = disabled, 1 = enabled
    pub final_transform_index: u32, // Index in transform buffer (always last slot)
    pub bits_per_transform: u32, // Bits needed per transform index (1-4 based on num_transforms)
    pub path_map_style: u32, // 0=Prefix, 1=Suffix, 2=PrefixDistinct, 3=SuffixDistinct
    pub path_capture_mode: u32, // 0=FirstHit, 1=FirstAfterBurnIn, 2=DeepestHit
    pub path_tracking_mode: u32, // 0=First (first 32 iterations), 1=Recent (rolling window of 32)
    pub num_path_filters: u32, // Number of active path filters (0 = disabled)
    pub min_suffix_filter_length: u32, // Minimum length among depth=0 filters (for optimization)
}

/// Maximum number of path filters supported
pub const MAX_PATH_FILTERS: usize = 64;

/// GPU representation of a path filter (must match WGSL PathFilter struct)
/// Used to block specific transform sequences during iteration
#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuPathFilter {
    /// Packed pattern (up to 8 iterations at 4 bits each, LSB = first)
    pub pattern: u32,
    /// Number of iterations in pattern (1-8)
    pub length: u32,
    /// 0 = suffix match (any depth), >0 = match at this exact depth
    pub depth: u32,
    /// Padding for 16-byte alignment
    pub _padding: u32,
}

impl GpuPathFilter {
    /// Create an empty (unused) filter
    pub fn empty() -> Self {
        Self {
            pattern: 0,
            length: 0,
            depth: 0,
            _padding: 0,
        }
    }

    /// Create a suffix filter (matches at any depth)
    /// pattern: array of transform indices (0-15), up to 8 elements
    pub fn suffix(pattern: &[u32]) -> Self {
        assert!(pattern.len() <= 8, "Pattern can have at most 8 elements");
        let mut packed = 0u32;
        for (i, &idx) in pattern.iter().enumerate() {
            packed |= (idx & 0xF) << (i * 4);
        }
        Self {
            pattern: packed,
            length: pattern.len() as u32,
            depth: 0, // 0 = suffix match
            _padding: 0,
        }
    }

    /// Create an exact depth filter (only matches at specific iteration depth)
    /// pattern: array of transform indices (0-15), up to 8 elements
    /// depth: the iteration count at which this pattern should match
    pub fn at_depth(pattern: &[u32], depth: u32) -> Self {
        assert!(pattern.len() <= 8, "Pattern can have at most 8 elements");
        assert!(depth >= pattern.len() as u32, "Depth must be >= pattern length");
        let mut packed = 0u32;
        for (i, &idx) in pattern.iter().enumerate() {
            packed |= (idx & 0xF) << (i * 4);
        }
        Self {
            pattern: packed,
            length: pattern.len() as u32,
            depth, // >0 = exact depth match
            _padding: 0,
        }
    }
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
    pub _pad_bg: f32,  // Padding to align vec3 to 16 bytes (std140 rule)
    pub use_curve: u32,  // 0 = disabled, 1 = enabled
    pub vibrancy: f32,  // Blend between old and new color algorithms (0.0-30.0)
    pub brightness: f32,  // Logarithmic brightness scaling (0.0-5.0, default 1.0)
    pub white_level: f32,  // Apophysis white_level constant (default 200.0)
    pub prefilter_white: f32,  // Apophysis PREFILTER_WHITE constant (67108864.0)
    pub bright_adjust: f32,  // Apophysis BRIGHT_ADJUST constant (2.3)
    pub area: f32,  // Render area (width * height)
    pub sample_density: f32,  // Iterations per pixel
    pub saturation: f32,  // Color saturation boost (1.0 = no change, >1.0 = more saturated)
    pub hue_shift: f32,  // Hue rotation in degrees (-180.0 to 180.0)
    pub gamma_threshold: f32,  // Smooths gamma curve at low densities (default 0.0025)
    pub alpha_blend_low: f32,  // Start blending toward linear alpha at this gamma-corrected value
    pub alpha_blend_high: f32,  // Full linear alpha above this value
    pub transparent_mode: u32,  // 0 = normal (blend with background), 1 = transparent export
    pub color_mode: u32,  // 0 = palette, 1 = speed, 2 = path_map
    pub width: u32,  // Texture width for path buffer indexing
    pub height: u32,  // Texture height for path buffer indexing
    pub path_map_style: u32,  // 0=Prefix, 1=Suffix, 2=PrefixDistinct, 3=SuffixDistinct, 4=Depth, 5=OriginRadial, 6=OriginHorizontal, 7=OriginVertical
    pub burn_in: u32,  // Burn-in iterations (for Depth gradient: start depth)
    pub num_transforms: u32,  // Number of transforms (for path coloring entropy)
    pub palette_size: u32,  // Palette texture size (256-4096), for shader index calculations
    pub _pad_end: [u32; 3],  // Padding to align struct to 16-byte boundary (128 bytes total)
}

impl Default for TonemapParams {
    fn default() -> Self {
        use crate::config::defaults::*;
        Self {
            exposure: DEFAULT_EXPOSURE,
            gamma: DEFAULT_GAMMA,
            density_scale: DEFAULT_DENSITY_SCALE,
            tonemap_mode: 1,  // Default to Logarithmic
            background_color: [0.0, 0.0, 0.0],
            _pad_bg: 0.0,
            use_curve: 0,  // Curves disabled by default
            vibrancy: 1.0,  // Modern vibrant colors by default
            brightness: DEFAULT_BRIGHTNESS,
            white_level: DEFAULT_WHITE_LEVEL,
            prefilter_white: PREFILTER_WHITE,
            bright_adjust: BRIGHT_ADJUST,
            area: 800.0 * 600.0,  // Default resolution
            sample_density: 1.0,  // Will be updated per frame
            saturation: DEFAULT_SATURATION,
            hue_shift: DEFAULT_HUE_SHIFT,
            gamma_threshold: DEFAULT_GAMMA_THRESHOLD,
            alpha_blend_low: DEFAULT_ALPHA_BLEND_LOW,
            alpha_blend_high: DEFAULT_ALPHA_BLEND_HIGH,
            transparent_mode: 0,  // Normal display mode
            color_mode: 0,  // Palette mode by default
            width: 800,  // Default width (will be updated per frame)
            height: 600,  // Default height (will be updated per frame)
            path_map_style: 0,  // Prefix mode by default
            burn_in: 20,  // Default burn-in
            num_transforms: 3,  // Default 3 transforms
            palette_size: 256,  // Default palette size
            _pad_end: [0, 0, 0],
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
    pub histogram_color_scale: f32, // Must match compute shader value
    pub target_iterations_per_pixel: u32, // Per-pixel convergence threshold (0 = disabled)
    pub _pad0: f32,  // Padding for alignment
    pub background_r: f32,  // Background color RGB (for blending when no samples)
    pub background_g: f32,
    pub background_b: f32,
    pub _pad1: f32,  // Total 10 fields = 40 bytes (rounds to 48 with padding)
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

    // Histogram storage buffer for atomic color accumulation (within-frame)
    // Layout: [r, g, b, density] × (width × height) as u32 array
    pub histogram_buffer: Buffer,

    // Per-pixel iteration count buffer for convergence tracking
    // Layout: 1× u32 per pixel (total iteration hits)
    // Used to stop accumulating pixels after target iteration count
    pub iteration_count_buffer: Buffer,

    // Per-pixel path buffer for PathMap color mode (OPTIONAL)
    // Layout: 7× u32 per pixel (PathEntry struct)
    // Path is packed MSB-first: transform indices stored from high bits down
    // Last-write-wins semantics (not accumulated)
    // None when path features are disabled to save ~58MB at 1920×1080
    pub path_buffer: Option<Buffer>,

    // Path filter buffer for blocking specific transform sequences (OPTIONAL)
    // Layout: MAX_PATH_FILTERS × GpuPathFilter (16 bytes each)
    // None when path features are disabled
    pub path_filter_buffer: Option<Buffer>,

    // Dummy buffers for binding when path features are disabled
    // WebGPU requires all bindings to be present, so we bind minimal buffers when disabled
    pub dummy_path_buffer: Buffer,
    pub dummy_filter_buffer: Buffer,

    // Per-pixel scale buffer for adaptive histogram scaling
    // Note: scale_buffer removed - now using params.histogram_color_scale (global uniform)

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

    // Track current dimensions for path buffer recreation
    width: u32,
    height: u32,

    // Track current palette size for dynamic palette texture
    palette_size: u32,
}

/// Default palette size (256 colors for backward compatibility)
pub const DEFAULT_PALETTE_SIZE: u32 = 256;

/// Maximum supported palette size
pub const MAX_PALETTE_SIZE: u32 = 4096;

impl FlameBuffers {
    /// Create new FlameBuffers with default palette size (256)
    pub fn new(device: &Device, queue: &Queue, width: u32, height: u32, flame: &Flame) -> Self {
        Self::with_palette_size(device, queue, width, height, flame, DEFAULT_PALETTE_SIZE)
    }

    /// Create new FlameBuffers with specified palette size
    pub fn with_palette_size(device: &Device, queue: &Queue, width: u32, height: u32, flame: &Flame, palette_size: u32) -> Self {
        // Clamp palette size to valid range
        let palette_size = palette_size.clamp(DEFAULT_PALETTE_SIZE, MAX_PALETTE_SIZE);
        // Create transform storage buffer sized for MAX_TRANSFORMS
        // This allows loading presets with different numbers of transforms without recreating the buffer
        let buffer_size = (MAX_TRANSFORMS * std::mem::size_of::<GpuTransform>()) as u64;
        let transform_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Transform Buffer"),
            size: buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Upload initial transforms (include final transform if present)
        let registry = crate::variations::global_registry();
        let mut gpu_transforms: Vec<GpuTransform> = flame
            .transforms
            .iter()
            .map(|xform| GpuTransform::from_transform(xform, registry))
            .collect();
        // Append final transform if present (same as update_transforms)
        if let Some(ref final_xform) = flame.final_transform {
            gpu_transforms.push(GpuTransform::from_transform(final_xform, registry));
        }
        queue.write_buffer(&transform_buffer, 0, bytemuck::cast_slice(&gpu_transforms));

        // Create variation parameters storage buffer sized for MAX_TRANSFORMS
        let params_buffer_size = (MAX_TRANSFORMS * std::mem::size_of::<GpuVariationParams>()) as u64;
        let variation_params_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Variation Params Buffer"),
            size: params_buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Upload initial variation parameters (include final transform if present)
        let mut gpu_params: Vec<GpuVariationParams> = flame
            .transforms
            .iter()
            .map(|xform| GpuVariationParams::from_transform(xform, registry))
            .collect();
        // Append final transform params if present (same as update_variation_params)
        if let Some(ref final_xform) = flame.final_transform {
            gpu_params.push(GpuVariationParams::from_transform(final_xform, registry));
        }
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
            splat_size: 1.0,
            zoom: 1.0,
            pan_x: 0.0,
            pan_y: 0.0,
            rotation: 0.0,
            speed_factor: 0.5,
            perspective_strength: 2.0,
            camera_rotation_x: 0.0,
            camera_rotation_y: 0.0,
            camera_z: 0.0,
            histogram_color_scale: crate::config::DEFAULT_HISTOGRAM_COLOR_SCALE,
            has_final_transform: if flame.final_transform.is_some() { 1 } else { 0 },
            final_transform_index: flame.transforms.len() as u32,
            bits_per_transform: bits_per_transform(flame.transforms.len() as u32),
            path_map_style: 0,
            path_capture_mode: 0, // FirstHit by default
            path_tracking_mode: 0, // First (first 32 iterations) by default
            num_path_filters: 0, // No filters by default
            min_suffix_filter_length: 0, // No filters by default
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
            histogram_color_scale: crate::config::DEFAULT_HISTOGRAM_COLOR_SCALE,
            target_iterations_per_pixel: 0, // Default: disabled (no per-pixel convergence)
            _pad0: 0.0,
            background_r: 0.0,  // Default black background
            background_g: 0.0,
            background_b: 0.0,
            _pad1: 0.0,
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

        // Create histogram storage buffer for atomic color accumulation
        // Buffer layout: 4× u32 per pixel (unpacked, no bit manipulation needed)
        //   u32[0]: R (full u32)
        //   u32[1]: G (full u32)
        //   u32[2]: B (full u32)
        //   u32[3]: Density (full u32)
        // Size: width × height × 4 × sizeof(u32)
        // Memory: ~7.7MB @ 800×600 (was ~5.8MB with packed format)
        let histogram_buffer_size = (width * height * 4 * std::mem::size_of::<u32>() as u32) as u64;
        let histogram_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Histogram Buffer"),
            size: histogram_buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Create iteration count buffer (1× u32 per pixel)
        // Tracks how many times each pixel has been hit
        // Used for per-pixel convergence control
        // Size: width × height × sizeof(u32)
        // Memory: ~1.9MB @ 800×600, ~8.3MB @ 1920×1080
        let iteration_count_buffer_size = (width * height * std::mem::size_of::<u32>() as u32) as u64;
        let iteration_count_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Iteration Count Buffer"),
            size: iteration_count_buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Path buffers are optional - only created when path features are enabled
        // See create_path_buffers() and drop_path_buffers() methods
        // Initially None to save memory (~58MB at 1920×1080)
        let path_buffer: Option<Buffer> = None;
        let path_filter_buffer: Option<Buffer> = None;

        // Create minimal dummy buffers for binding when path features are disabled
        // WebGPU requires all declared bindings to be bound, even if unused
        // Path buffer: 28 bytes minimum (PathEntry = 7 × u32)
        // Filter buffer: 16 bytes minimum (GpuPathFilter = 4 × u32)
        let dummy_path_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Dummy Path Buffer"),
            size: 28,  // PathEntry size: 7 × sizeof(u32) = 28 bytes
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        let dummy_filter_buffer = device.create_buffer(&BufferDescriptor {
            label: Some("Dummy Filter Buffer"),
            size: 16,  // GpuPathFilter size: 4 × sizeof(u32) = 16 bytes
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // Note: scale_buffer removed - now using params.histogram_color_scale (global uniform)

        // Create palette texture (1D, dynamic size: 256-4096 samples)
        // Use Rgba8Unorm for efficient, standard color storage
        let default_palette = Palette::fire(); // Default palette
        let palette_data = default_palette.generate_texture_data(palette_size as usize);

        // Convert f32 [0.0, 1.0] to u8 [0, 255] for Rgba8Unorm
        let palette_data_u8: Vec<u8> = palette_data
            .iter()
            .map(|&v| (v.clamp(0.0, 1.0) * 255.0) as u8)
            .collect();

        let palette_texture = device.create_texture(&TextureDescriptor {
            label: Some("Palette Texture"),
            size: Extent3d {
                width: palette_size,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba8Unorm,
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload palette data
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &palette_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &palette_data_u8,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(palette_size * 4), // N pixels * 4 components * 1 byte
                rows_per_image: None,
            },
            Extent3d {
                width: palette_size,
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
            dimension: TextureDimension::D2,
            format: TextureFormat::Rgba16Float,  // Use 16-bit float for precision (filterable)
            usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
            view_formats: &[],
        });

        // Upload curve LUT data
        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &curve_lut_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &curve_lut_data,
            TexelCopyBufferLayout {
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
            histogram_buffer,
            iteration_count_buffer,
            path_buffer,
            path_filter_buffer,
            dummy_path_buffer,
            dummy_filter_buffer,
            // scale_buffer removed - using params.histogram_color_scale instead
            palette_texture,
            palette_view,
            curve_lut_texture,
            curve_lut_view,
            sampler,
            curve_lut_sampler,
            current_is_a: true,
            width,
            height,
            palette_size,
        }
    }

    /// Get the current palette size
    pub fn palette_size(&self) -> u32 {
        self.palette_size
    }

    /// Clear all accumulation buffers
    pub fn clear_all(&self, encoder: &mut CommandEncoder, _queue: &Queue) {
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

        // Clear histogram buffer (zero out all pixels)
        // Note: This is done via queue.write_buffer to avoid encoder ordering issues
        encoder.clear_buffer(&self.histogram_buffer, 0, None);

        // Clear iteration count buffer (zero out all iteration counts)
        encoder.clear_buffer(&self.iteration_count_buffer, 0, None);

        // Clear path buffer (zero out all paths) - only if enabled
        if let Some(ref path_buffer) = self.path_buffer {
            encoder.clear_buffer(path_buffer, 0, None);
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
                depth_slice: None,
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        drop(render_pass); // End the render pass immediately
    }

    /// Clear histogram buffer only (before each batch for proper accumulation math)
    pub fn clear_histogram(&self, encoder: &mut CommandEncoder) {
        encoder.clear_buffer(&self.histogram_buffer, 0, None);
    }

    /// Clear path buffer only (on full reset: view change, flame change, etc.)
    pub fn clear_paths(&self, encoder: &mut CommandEncoder) {
        if let Some(ref path_buffer) = self.path_buffer {
            encoder.clear_buffer(path_buffer, 0, None);
        }
    }

    /// Clear histogram and path buffers (convenience method for full reset)
    pub fn clear_histogram_and_paths(&self, encoder: &mut CommandEncoder) {
        encoder.clear_buffer(&self.histogram_buffer, 0, None);
        if let Some(ref path_buffer) = self.path_buffer {
            encoder.clear_buffer(path_buffer, 0, None);
        }
    }

    // Note: reset_scale_buffer() removed - scale is now a uniform constant

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
        // Check space for regular transforms + optional final transform
        let total_transforms = flame.transforms.len() + if flame.final_transform.is_some() { 1 } else { 0 };
        if total_transforms > MAX_TRANSFORMS {
            panic!("Flame has {} transforms (+ final) but MAX_TRANSFORMS is {}", flame.transforms.len(), MAX_TRANSFORMS);
        }

        // Create a fixed-size array with all transforms, padding with zeroes
        let registry = crate::variations::global_registry();
        let mut gpu_transforms: Vec<GpuTransform> = flame
            .transforms
            .iter()
            .map(|xform| GpuTransform::from_transform(xform, registry))
            .collect();

        // Append final transform if present (always at end of regular transforms)
        if let Some(final_xform) = &flame.final_transform {
            gpu_transforms.push(GpuTransform::from_transform(final_xform, registry));
        }

        // Pad with zeroed transforms to fill the buffer
        // This ensures old transforms don't remain in GPU memory when switching to fewer transforms
        while gpu_transforms.len() < MAX_TRANSFORMS {
            gpu_transforms.push(bytemuck::Zeroable::zeroed());
        }

        queue.write_buffer(&self.transform_buffer, 0, bytemuck::cast_slice(&gpu_transforms));
    }

    /// Update variation parameters
    pub fn update_variation_params(&self, queue: &Queue, flame: &Flame) {
        // Check space for regular transforms + optional final transform
        let total_transforms = flame.transforms.len() + if flame.final_transform.is_some() { 1 } else { 0 };
        if total_transforms > MAX_TRANSFORMS {
            panic!("Flame has {} transforms (+ final) but MAX_TRANSFORMS is {}", flame.transforms.len(), MAX_TRANSFORMS);
        }

        // Create a fixed-size array with all variation parameters, padding with zeroes
        let registry = crate::variations::global_registry();
        let mut gpu_params: Vec<GpuVariationParams> = flame
            .transforms
            .iter()
            .map(|xform| GpuVariationParams::from_transform(xform, registry))
            .collect();

        // Append final transform parameters if present
        if let Some(final_xform) = &flame.final_transform {
            gpu_params.push(GpuVariationParams::from_transform(final_xform, registry));
        }

        // Pad with zeroed params to fill the buffer
        while gpu_params.len() < MAX_TRANSFORMS {
            gpu_params.push(bytemuck::Zeroable::zeroed());
        }

        queue.write_buffer(&self.variation_params_buffer, 0, bytemuck::cast_slice(&gpu_params));
    }

    /// Update palette texture
    /// Uses the palette_size set during FlameBuffers creation
    ///
    /// # Arguments
    /// * `palette_rotation` - Rotation amount (-1.0 to 1.0), shifts palette indices
    /// * `palette_squeeze` - Squeeze factor: 1.0 = normal, >1 = repeat palette N times, <1 = show only N% of palette
    pub fn update_palette(&self, queue: &Queue, palette: &Palette, palette_rotation: f32, palette_squeeze: f32) {
        let size = self.palette_size as usize;
        let palette_data = palette.generate_texture_data(size);

        // Apply squeeze transformation first
        // squeeze > 1: palette repeats N times (e.g., 16x means palette repeats 16 times)
        // squeeze < 1: only shows portion of palette (e.g., 0.1 shows 10% stretched to fill)
        // Formula: src_t = (dst_t * squeeze) % 1.0
        let squeezed_data = if palette_squeeze != 1.0 {
            let mut squeezed = vec![0.0f32; size * 4];

            for i in 0..size {
                let t = i as f32 / size as f32;
                let src_t = (t * palette_squeeze).fract(); // fract() handles modulo for floats
                let src_idx = ((src_t * size as f32) as usize).min(size - 1);

                let dst_base = i * 4;
                let src_base = src_idx * 4;

                squeezed[dst_base] = palette_data[src_base];
                squeezed[dst_base + 1] = palette_data[src_base + 1];
                squeezed[dst_base + 2] = palette_data[src_base + 2];
                squeezed[dst_base + 3] = palette_data[src_base + 3];
            }
            squeezed
        } else {
            palette_data
        };

        // Apply palette rotation by shifting indices
        // Rotation range: -1.0 to 1.0 (Apophysis uses -128 to 128, we normalize)
        // Negative rotation: colors shift left (color at 0 comes from 1, at 1 from 2, ..., at N-1 from 0)
        // Positive rotation: colors shift right (color at 0 comes from N-1, at 1 from 0, ..., at N-1 from N-2)
        let rotated_data = if palette_rotation != 0.0 {
            let rotation_amount = (palette_rotation * size as f32).round() as i32;
            let mut rotated = vec![0.0f32; size * 4];

            for i in 0..size {
                // Calculate source index with wrapping
                let src_idx = ((i as i32 + rotation_amount).rem_euclid(size as i32)) as usize;
                let dst_idx = i * 4;
                let src_base = src_idx * 4;

                rotated[dst_idx] = squeezed_data[src_base];
                rotated[dst_idx + 1] = squeezed_data[src_base + 1];
                rotated[dst_idx + 2] = squeezed_data[src_base + 2];
                rotated[dst_idx + 3] = squeezed_data[src_base + 3];
            }
            rotated
        } else {
            squeezed_data
        };

        // Convert f32 [0.0, 1.0] to u8 [0, 255] for Rgba8Unorm
        let palette_data_u8: Vec<u8> = rotated_data
            .iter()
            .map(|&v| (v.clamp(0.0, 1.0) * 255.0) as u8)
            .collect();

        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.palette_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &palette_data_u8,
            TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.palette_size * 4), // N pixels * 4 components * 1 byte
                rows_per_image: None,
            },
            Extent3d {
                width: self.palette_size,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
    }

    /// Update tone curve LUT texture
    pub fn update_curve_lut(&self, queue: &Queue, curve: &crate::scene::tonemap::ToneCurve) {
        let curve_lut_data = curve.generate_lut();

        queue.write_texture(
            TexelCopyTextureInfo {
                texture: &self.curve_lut_texture,
                mip_level: 0,
                origin: Origin3d::ZERO,
                aspect: TextureAspect::All,
            },
            &curve_lut_data,
            TexelCopyBufferLayout {
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

    // ============================================================
    // Path buffer management (optional buffers for memory savings)
    // ============================================================

    /// Check if path features are currently enabled (buffers allocated)
    pub fn path_features_enabled(&self) -> bool {
        self.path_buffer.is_some()
    }

    /// Create path buffers if not already created
    /// Call when PathMap color mode is enabled or path filters are added
    /// Returns true if buffers were created (bind groups need rebuilding)
    pub fn create_path_buffers(&mut self, device: &Device, queue: &Queue) -> bool {
        if self.path_buffer.is_some() {
            return false;  // Already created
        }

        log::info!(
            "Creating path buffers: {}×{} ({:.1}MB)",
            self.width,
            self.height,
            (self.width as f64 * self.height as f64 * 7.0 * 4.0) / (1024.0 * 1024.0)
        );

        // Create path buffer (7 × u32 per pixel for PathEntry struct)
        let path_buffer_size = (self.width * self.height * 7 * std::mem::size_of::<u32>() as u32) as u64;
        self.path_buffer = Some(device.create_buffer(&BufferDescriptor {
            label: Some("Path Buffer"),
            size: path_buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        }));

        // Create path filter buffer (MAX_PATH_FILTERS × 16 bytes each)
        let path_filter_buffer_size = (MAX_PATH_FILTERS * std::mem::size_of::<GpuPathFilter>()) as u64;
        self.path_filter_buffer = Some(device.create_buffer(&BufferDescriptor {
            label: Some("Path Filter Buffer"),
            size: path_filter_buffer_size,
            usage: BufferUsages::STORAGE | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        }));

        // Initialize filter buffer with empty filters
        let empty_filters = vec![GpuPathFilter::empty(); MAX_PATH_FILTERS];
        queue.write_buffer(
            self.path_filter_buffer.as_ref().unwrap(),
            0,
            bytemuck::cast_slice(&empty_filters),
        );

        true  // Bind groups need rebuilding
    }

    /// Drop path buffers to free memory
    /// Call when PathMap color mode is disabled AND no path filters are active
    /// Returns true if buffers were dropped (bind groups need rebuilding)
    pub fn drop_path_buffers(&mut self) -> bool {
        if self.path_buffer.is_none() {
            return false;  // Already dropped
        }

        log::info!(
            "Dropping path buffers: {:.1}MB freed",
            (self.width as f64 * self.height as f64 * 7.0 * 4.0) / (1024.0 * 1024.0)
        );

        self.path_buffer = None;
        self.path_filter_buffer = None;

        true  // Bind groups need rebuilding
    }

    /// Get the path buffer for binding (real or dummy)
    /// Use this when creating bind groups
    pub fn get_path_buffer_for_binding(&self) -> &Buffer {
        self.path_buffer.as_ref().unwrap_or(&self.dummy_path_buffer)
    }

    /// Get the path filter buffer for binding (real or dummy)
    /// Use this when creating bind groups
    pub fn get_filter_buffer_for_binding(&self) -> &Buffer {
        self.path_filter_buffer.as_ref().unwrap_or(&self.dummy_filter_buffer)
    }

    /// Write path filters to the GPU buffer
    /// Only writes if path buffers are enabled
    pub fn write_path_filters(&self, queue: &Queue, filters: &[GpuPathFilter]) {
        if let Some(ref filter_buffer) = self.path_filter_buffer {
            // Pad with empty filters if needed
            let mut padded_filters = filters.to_vec();
            padded_filters.resize(MAX_PATH_FILTERS, GpuPathFilter::empty());
            queue.write_buffer(filter_buffer, 0, bytemuck::cast_slice(&padded_filters));
        }
    }
}
