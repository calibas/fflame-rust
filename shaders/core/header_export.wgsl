// Shared header for export shaders (CPU histogram accumulation)
// Outputs samples to buffer instead of GPU histogram

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

    // Color (palette position) + color_speed + opacity + direct_color (vec4 for alignment)
    color: f32,
    color_speed: f32,
    opacity: f32,
    direct_color: f32,
}

// Dispatch parameters (must match GpuParams in buffers.rs exactly)
struct Params {
    num_transforms: u32,
    iterations_per_thread: u32,
    burn_in: u32,
    width: u32,       // Full output width
    height: u32,      // Full output height
    seed: u32,
    color_mode: u32,  // 0 = palette, 1 = speed, 2 = path_map
    render_mode: u32, // 0 = 2D, 1 = 3D
    splat_size: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    rotation: f32,
    speed_factor: f32,
    perspective_strength: f32,
    camera_rotation_x: f32,
    camera_rotation_y: f32,
    camera_z: f32,
    dof_focus_distance: f32,  // Depth of field: distance where image is sharpest
    dof_blur_strength: f32,  // Depth of field: blur amount (0.0 = disabled)
    fog_strength: f32,  // Depth fog: exponential fog density (0.0 = disabled)
    fog_start: f32,  // Depth fog: distance where fog begins
    histogram_color_scale: f32,  // Not used for export, but kept for struct compatibility
    has_final_transform: u32,
    final_transform_index: u32,
    bits_per_transform: u32,  // Bits needed per transform index (1-5 based on num_transforms)
    path_map_style: u32,  // 0=Prefix, 1=Suffix, 2=ScrambledPrefix, 3=ScrambledSuffix
    path_capture_mode: u32,  // 0=FirstHit, 1=FirstAfterBurnIn, 2=DeepestHit
    path_tracking_mode: u32,  // 0=First (first 32 iterations), 1=Recent (rolling window of 32)
    num_path_filters: u32,  // Number of active path filters (0 = disabled)
    min_suffix_filter_length: u32,  // Minimum length among depth=0 filters (for optimization)
    background_r: f32,  // Background color R (for depth fog)
    background_g: f32,  // Background color G (for depth fog)
    background_b: f32,  // Background color B (for depth fog)
}

// Variation parameters for one transform
struct VariationParams {
    params: array<f32, 1600>,  // 100 variations × 16 params (user + init-derived)
}

// Output sample structure (x, y, r, g, b with padding for alignment)
// WGSL arrays require element strides to be multiples of 16 bytes
// 5 floats = 20 bytes, rounded up to 32 bytes (8 floats) for proper alignment
struct Sample {
    x: f32,
    y: f32,
    r: f32,
    g: f32,
    b: f32,
    _pad1: f32,
    _pad2: f32,
    _pad3: f32,
}

// Atomic counter for sample count
struct SampleCounter {
    count: atomic<u32>,
}

// Per-normal-transform attachment list — see header.wgsl for full doc.
struct AttachmentList {
    linked: array<u32, 32>,
    linked_count: u32,
    final_: array<u32, 32>,
    final_count: u32,
}

// Bindings
@group(0) @binding(0) var<storage, read> transforms: array<Transform>;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var<storage, read_write> samples: array<Sample>;
@group(0) @binding(3) var palette_texture: texture_2d<f32>;
@group(0) @binding(4) var palette_sampler: sampler;
@group(0) @binding(5) var<storage, read> variation_params: array<VariationParams>;
@group(0) @binding(6) var<storage, read_write> sample_counter: SampleCounter;
// Xaos (chaos) transition weights: xaos_weights[src * num_transforms + dst]
// Modifies probability of selecting dst transform when coming from src
@group(0) @binding(7) var<storage, read> xaos_weights: array<f32>;
// Per-normal-transform attachment lists (Linked + Final chains).
@group(0) @binding(8) var<storage, read> attachments: array<AttachmentList>;
