// Shared header for all trajectory shaders
// Contains struct definitions and bind group declarations

// Transform structure (matching CPU-side Transform)
struct Transform {
    // Affine matrix coefficients
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,

    // Z offset for 3D mode
    g: f32,

    // Probability weight
    weight: f32,

    // Variation weights (100 slots: supports all Apophysis 7X + future expansion)
    variations: array<f32, 100>,

    // Color (palette position) + color_speed + opacity + padding (vec4 for alignment)
    color: f32,
    color_speed: f32,
    opacity: f32,
    _padding: f32,
}

// Dispatch parameters
struct Params {
    num_transforms: u32,
    iterations_per_thread: u32,
    burn_in: u32,
    width: u32,
    height: u32,
    seed: u32,
    color_mode: u32,  // 0 = palette, 1 = speed, 2 = path_map
    render_mode: u32,  // 0 = 2D, 1 = 3D
    splat_size: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    rotation: f32,  // Rotation in radians (2D, around Z)
    speed_factor: f32,  // Blend factor for speed-based coloring
    perspective_strength: f32,  // Strength for perspective projection
    camera_rotation_x: f32,  // 3D camera pitch (rotation around X)
    camera_rotation_y: f32,  // 3D camera yaw (rotation around Y)
    camera_z: f32,  // 3D camera Z position (height)
    histogram_color_scale: f32,  // Precision vs overflow (default: 10.0)
    has_final_transform: u32,  // 0 = disabled, 1 = enabled
    final_transform_index: u32,  // Index in transform buffer (after regular transforms)
    bits_per_transform: u32,  // Bits needed per transform index (1-5 based on num_transforms)
    path_map_style: u32,  // 0=Similar, 1=Distinct
}

// Variation parameters for one transform
// Indexed as: params[variation_id * 12 + param_slot]
struct VariationParams {
    params: array<f32, 1200>,  // 100 variations × 12 params
}

// Path storage for PathMap color mode
// Stores u64 path as vec2<u32> (hi, lo) per pixel
// Path is packed MSB-first: transform indices stored from high bits down
struct PathEntry {
    hi: u32,  // High 32 bits of path
    lo: u32,  // Low 32 bits of path
}

// Bindings
@group(0) @binding(0) var<storage, read> transforms: array<Transform>;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var<storage, read_write> histogram: array<atomic<u32>>;
@group(0) @binding(3) var palette_texture: texture_2d<f32>;
@group(0) @binding(4) var palette_sampler: sampler;
@group(0) @binding(5) var<storage, read> variation_params: array<VariationParams>;
@group(0) @binding(6) var<storage, read_write> iteration_counts: array<atomic<u32>>;
@group(0) @binding(7) var<storage, read_write> path_buffer: array<PathEntry>;
