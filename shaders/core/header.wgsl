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

    // Post-affine matrix (applied after variations)
    post_a: f32,
    post_b: f32,
    post_c: f32,
    post_d: f32,
    post_e: f32,
    post_f: f32,
    post_g: f32,
    post_enabled: f32, // 0.0 = disabled, 1.0 = enabled

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
    dof_focus_distance: f32,  // Depth of field: distance where image is sharpest
    dof_blur_strength: f32,  // Depth of field: blur amount (0.0 = disabled)
    fog_strength: f32,  // Depth fog: exponential fog density (0.0 = disabled)
    fog_start: f32,  // Depth fog: distance where fog begins
    histogram_color_scale: f32,  // Precision vs overflow (default: 10.0)
    has_final_transform: u32,  // 0 = disabled, 1 = enabled
    final_transform_index: u32,  // Index in transform buffer (after regular transforms)
    bits_per_transform: u32,  // Bits needed per transform index (1-5 based on num_transforms)
    path_map_style: u32,  // 0=Prefix, 1=Suffix, 2=Prefix (Distinct), 3=Suffix (Distinct)
    path_capture_mode: u32,  // 0=FirstHit, 1=FirstAfterBurnIn, 2=LastHit
    path_tracking_mode: u32,  // 0=First (first 32 iterations), 1=Recent (rolling window of 32 most recent)
    num_path_filters: u32,  // Number of active path filters (0 = disabled)
    min_suffix_filter_length: u32,  // Minimum length among depth=0 filters (for optimization)
    background_r: f32,  // Background color R (for depth fog)
    background_g: f32,  // Background color G (for depth fog)
    background_b: f32,  // Background color B (for depth fog)
}

// Variation parameters for one transform
// Indexed as: params[variation_id * 12 + param_slot]
struct VariationParams {
    params: array<f32, 1200>,  // 100 variations × 12 params
}

// Path storage for PathMap color mode
// Stores up to 32 iterations losslessly (4 bits per transform, up to 16 transforms)
// Also stores initial random X/Y coordinates for complete path reconstruction
struct PathEntry {
    path0: u32,  // Iterations 0-7 (4 bits each, LSB = iteration 0)
    path1: u32,  // Iterations 8-15
    path2: u32,  // Iterations 16-23
    path3: u32,  // Iterations 24-31
    iteration_count: u32,  // Actual iteration when pixel was hit (not capped at 32)
    initial_x: f32,  // Initial random X coordinate [-1, 1]
    initial_y: f32,  // Initial random Y coordinate [-1, 1]
}

// Path filter for blocking specific transform sequences
// depth=0: suffix match (block paths ending with pattern at any depth)
// depth>0: exact depth match (block paths matching pattern at specific iteration)
struct PathFilter {
    pattern: u32,  // Packed pattern (up to 8 iterations at 4 bits each, LSB = first)
    length: u32,   // Number of iterations in pattern (1-8)
    depth: u32,    // 0 = suffix match, >0 = match at this exact depth
    _padding: u32, // Padding for 16-byte alignment
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
@group(0) @binding(8) var<storage, read> path_filters: array<PathFilter>;
// Xaos (chaos) transition weights: xaos_weights[src * num_transforms + dst]
// Modifies probability of selecting dst transform when coming from src
@group(0) @binding(9) var<storage, read> xaos_weights: array<f32>;
