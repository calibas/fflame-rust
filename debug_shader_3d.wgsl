// Hard-coded shader constants (compiled at shader build time)
const NUM_TRANSFORMS: u32 = 1u;
const COLOR_MODE: u32 = 0u;
const HAS_FINAL_TRANSFORM: bool = false;
const FINAL_TRANSFORM_INDEX: u32 = 1u;
const HAS_POST_AFFINE: bool = false;
const USE_INLINED_WEIGHTS: bool = false;
const USE_INLINED_TRANSFORMS: bool = false;

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

    // Color (palette position) + color_speed + opacity + direct_color (vec4 for alignment)
    color: f32,
    color_speed: f32,
    opacity: f32,
    direct_color: f32,
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
// Indexed as: params[variation_id * 16 + param_slot]
struct VariationParams {
    params: array<f32, 1600>,  // 100 variations × 16 params (user + init-derived)
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

// PCG random number generator
// Provides deterministic random number generation for shader execution

// RNG state
struct RngState {
    state: u32,
}

// PCG hash function
fn pcg_hash(input: u32) -> u32 {
    var state = input * 747796405u + 2891336453u;
    var word = ((state >> ((state >> 28u) + 4u)) ^ state) * 277803737u;
    return (word >> 22u) ^ word;
}

// Initialize RNG with thread ID and global seed
fn rng_init(thread_id: u32, seed: u32) -> RngState {
    var rng: RngState;
    rng.state = pcg_hash(thread_id ^ seed);
    return rng;
}

// Generate random u32
fn rng_next(rng: ptr<function, RngState>) -> u32 {
    let old_state = (*rng).state;
    (*rng).state = old_state * 747796405u + 2891336453u;
    let xor_shifted = ((old_state >> ((old_state >> 28u) + 4u)) ^ old_state) * 277803737u;
    return (xor_shifted >> 22u) ^ xor_shifted;
}

// Generate random f32 in [0, 1)
fn rng_nextf(rng: ptr<function, RngState>) -> f32 {
    return f32(rng_next(rng)) / 4294967296.0;
}

// Affine transformations for 3D mode

// Apply pre-affine transformation (3D)
// Standard affine formula: x' = ax + by + e, y' = cx + dy + f, z' = z + g
fn apply_affine(xform: Transform, p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        xform.a * p.x + xform.b * p.y + xform.e,
        xform.c * p.x + xform.d * p.y + xform.f,
        p.z + xform.g  // Z is just offset
    );
}

// Apply post-affine transformation (3D)
// Same simultaneous affine formula, applied after variations
fn apply_post_affine(xform: Transform, p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        xform.post_a * p.x + xform.post_b * p.y + xform.post_e,
        xform.post_c * p.x + xform.post_d * p.y + xform.post_f,
        p.z + xform.post_g
    );
}

// Rotate around X axis (affects Y and Z)
fn rotate_x(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    // Apophysis: z' = c*z - s*y, y' = s*z + c*y
    return vec3<f32>(
        p.x,
        s * p.z + c * p.y,
        c * p.z - s * p.y
    );
}

// Rotate around Y axis (affects X and Z)
fn rotate_y(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    // Apophysis: x' = c*x - s*z, z' = s*x + c*z
    return vec3<f32>(
        c * p.x - s * p.z,
        p.y,
        s * p.x + c * p.z
    );
}


fn hex_seg60_3d(loc: u32) -> vec2<f32> {
    let hlift = 0.86602540378443864;
    switch (loc) {
        case 0u: { return vec2<f32>(1.0, 0.0); }
        case 1u: { return vec2<f32>(0.5, hlift); }
        case 2u: { return vec2<f32>(-0.5, hlift); }
        case 3u: { return vec2<f32>(-1.0, 0.0); }
        case 4u: { return vec2<f32>(-0.5, -hlift); }
        default: { return vec2<f32>(0.5, -hlift); }
    }
}

fn hex_seg120_3d(loc: u32) -> vec2<f32> {
    let hlift = 0.86602540378443864;
    switch (loc) {
        case 0u: { return vec2<f32>(1.0, 0.0); }
        case 1u: { return vec2<f32>(-0.5, hlift); }
        default: { return vec2<f32>(-0.5, -hlift); }
    }
}

fn variation_hexaplay3D(p: vec3<f32>, accum: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let majp = get_param(xform_id, variation_id, 0u);
    let scale_param = get_param(xform_id, variation_id, 1u);
    let zlift = get_param(xform_id, variation_id, 2u);

    var rswtch = get_state(xform_id, variation_id, 0u);
    var fcycle = get_state(xform_id, variation_id, 1u);
    var bcycle = get_state(xform_id, variation_id, 2u);

    if (fcycle > 5.0) {
        fcycle = 0.0;
        rswtch = floor(rng_nextf(rng) * 3.0);
    }
    if (bcycle > 2.0) {
        bcycle = 0.0;
        rswtch = floor(rng_nextf(rng) * 3.0);
    }

    let weight = transforms[xform_id].variations[variation_id];
    let safe_w = select(weight, 1e-30, abs(weight) < 1e-30);
    let scale = scale_param * 0.5;

    var pos_neg = 1.0;
    if (rng_nextf(rng) < 0.5) { pos_neg = -1.0; }

    let abmajp = abs(majp);
    var oz_extra = 0.0;
    if (abmajp > 1.0) {
        let boost = (abmajp - 1.0) * 0.5;
        oz_extra = pos_neg * boost;
    }

    var ox = 0.0;
    var oy = 0.0;
    if (rswtch <= 1.0) {
        let loc = u32(clamp(fcycle, 0.0, 5.0));
        let v = hex_seg60_3d(loc);
        ox = (accum.x * (scale - 1.0) + p.x * scale) / safe_w + v.x;
        oy = (accum.y * (scale - 1.0) + p.y * scale) / safe_w + v.y;
        fcycle = fcycle + 1.0;
    } else {
        let loc = u32(clamp(bcycle, 0.0, 2.0));
        let v = hex_seg120_3d(loc);
        ox = (accum.x * (scale - 1.0) + p.x * scale) / safe_w + v.x;
        oy = (accum.y * (scale - 1.0) + p.y * scale) / safe_w + v.y;
        bcycle = bcycle + 1.0;
    }

    let oz = (p.z * 0.5 * zlift + oz_extra) / safe_w;

    set_state(xform_id, variation_id, 0u, rswtch);
    set_state(xform_id, variation_id, 1u, fcycle);
    set_state(xform_id, variation_id, 2u, bcycle);
    return vec3<f32>(ox, oy, oz);
}


// Apply all variations with Apophysis 4-phase execution model (XForm.pas:343-383)
// See 2D variant for the meaning of the `vc` pointer.
fn apply_variations(xform: Transform, xform_id: u32, p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    var temp = p;

    // Phase 2: Precalculation handled per-variation

    // Phase 3: Normal variations (weighted sum from modified input)
    var result = vec3<f32>(0.0, 0.0, 0.0);

    // 0: Hexaplay 3D (NORMAL)
    if (xform.variations[0] != 0.0) {
        result += xform.variations[0] * variation_hexaplay3D(temp, result, xform_id, 0u, rng);
    }

    return result;
}

// Per-flame packed get_param: each active variation has its own
// contiguous slot range in variation_params, with offsets baked
// at flame compile time. See build_packed_get_param in
// shader_builder_v2.rs.
fn get_param(xform_id: u32, variation_id: u32, param_slot: u32) -> f32 {
    var offset: u32 = 0u;
    switch (variation_id) {
        case 0u: { offset = 0u; }  // hexaplay3D: 3 slots
        default: { offset = 0u; }
    }
    return variation_params[xform_id].params[offset + param_slot];
}

// Per-thread variation state. var<private> is per-invocation and
// zero-initialized by WGSL spec. Persists across the inner iteration
// loop within a single main() call. See
// docs/projects/intra-iteration-state-and-accum.md.
var<private> thread_state: array<f32, 3u>;

fn get_state(xform_id: u32, variation_id: u32, slot: u32) -> f32 {
    var offset: u32 = 0u;
    let key = xform_id * 100u + variation_id;
    switch (key) {
        case 0u: { offset = 0u; }  // xform 0, hexaplay3D: 3 slots
        default: { offset = 0u; }
    }
    return thread_state[offset + slot];
}

fn set_state(xform_id: u32, variation_id: u32, slot: u32, value: f32) {
    var offset: u32 = 0u;
    let key = xform_id * 100u + variation_id;
    switch (key) {
        case 0u: { offset = 0u; }  // xform 0, hexaplay3D: 3 slots
        default: { offset = 0u; }
    }
    thread_state[offset + slot] = value;
}

// Utility functions shared by 2D and 3D shaders
//
// NOTE: `get_param` is generated per-flame by the shader builder
// (see `build_packed_get_param` in src/shader_builder_v2.rs) and
// injected before this file is included. The injected version uses
// per-variation packed offsets baked from the active variation set,
// so each variation occupies exactly `parameters.len() +
// init_param_count` slots in the variation_params buffer instead of
// a fixed 16.

// Select transform based on cumulative weights (uses params.num_transforms)
fn select_transform(rand_val: f32) -> u32 {
    var cumulative = 0.0;
    var total_weight = 0.0;

    // Calculate total weight
    for (var i = 0u; i < params.num_transforms; i++) {
        total_weight += transforms[i].weight;
    }

    let ttarget = rand_val * total_weight;

    for (var i = 0u; i < params.num_transforms; i++) {
        cumulative += transforms[i].weight;
        if (ttarget <= cumulative) {
            return i;
        }
    }

    return params.num_transforms - 1u;
}

// Select transform using hard-coded NUM_TRANSFORMS constant
// Enables loop unrolling optimization by compiler
fn select_transform_const(rand_val: f32) -> u32 {
    var cumulative = 0.0;
    var total_weight = 0.0;

    // Calculate total weight (loop can be unrolled with known NUM_TRANSFORMS)
    for (var i = 0u; i < NUM_TRANSFORMS; i++) {
        total_weight += transforms[i].weight;
    }

    let ttarget = rand_val * total_weight;

    for (var i = 0u; i < NUM_TRANSFORMS; i++) {
        cumulative += transforms[i].weight;
        if (ttarget <= cumulative) {
            return i;
        }
    }

    return NUM_TRANSFORMS - 1u;
}

// Select transform with xaos (chaos) weighting
// Uses hard-coded NUM_TRANSFORMS for loop unrolling
// prev_xform: Index of the transform that was just applied
// Xaos modifies probability: P(src→dst) = weight[dst] × xaos[src][dst]
fn select_transform_xaos(rand_val: f32, prev_xform: u32) -> u32 {
    var cumulative = 0.0;
    var total_weight = 0.0;

    // Base index into xaos_weights array for this source transform
    let xaos_base = prev_xform * NUM_TRANSFORMS;

    // Calculate total modified weight
    for (var i = 0u; i < NUM_TRANSFORMS; i++) {
        let base_weight = transforms[i].weight;
        let xaos_modifier = xaos_weights[xaos_base + i];
        total_weight += base_weight * xaos_modifier;
    }

    let threshold = rand_val * total_weight;

    // Select based on modified weights
    for (var i = 0u; i < NUM_TRANSFORMS; i++) {
        let base_weight = transforms[i].weight;
        let xaos_modifier = xaos_weights[xaos_base + i];
        cumulative += base_weight * xaos_modifier;
        if (threshold <= cumulative) {
            return i;
        }
    }

    return NUM_TRANSFORMS - 1u;
}

// Convert speed to color using palette lookup
fn speed_to_color(speed: f32) -> vec3<f32> {
    // Normalize speed to [0, 1] range
    // Use logarithmic scale for better visualization
    let normalized_speed = clamp(log(speed * 10.0 + 1.0) / 3.0, 0.0, 1.0);

    // Sample from palette texture (2D texture with height=1, so y=0.5)
    return textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(normalized_speed, 0.5), 0.0).rgb;
}

// Build Apophysis camera matrix (ZXY Euler rotation: yaw around Z, then pitch around X)
// Source: Apophysis ControlPoint.pas:467-475
fn build_camera_matrix(pitch: f32, yaw: f32) -> mat3x3<f32> {
    let cy = cos(-yaw);  // Note: -yaw inversion as in Apophysis
    let sy = sin(-yaw);
    let cp = cos(pitch);
    let sp = sin(pitch);

    // Camera matrix from Apophysis formula
    // Returns column-major matrix for WGSL
    return mat3x3<f32>(
        vec3<f32>(cy,           cp * sy,      sp * sy),      // Column 0
        vec3<f32>(-sy,          cp * cy,      sp * cy),      // Column 1
        vec3<f32>(0.0,          -sp,          cp)            // Column 2
    );
}

// Apply Apophysis camera transformation: translate + rotate
fn camera_transform(p: vec3<f32>, camera_matrix: mat3x3<f32>, camera_z: f32) -> vec3<f32> {
    // Step 1: Translate Z coordinate (move origin to camera height)
    let z_translated = p.z - camera_z;

    // Step 2: Rotate using camera matrix (Apophysis formula)
    // Matrix multiplication: camera_matrix * vec3(p.x, p.y, z_translated)
    let x = camera_matrix[0][0] * p.x + camera_matrix[1][0] * p.y;
    let y = camera_matrix[0][1] * p.x + camera_matrix[1][1] * p.y + camera_matrix[2][1] * z_translated;
    let z = camera_matrix[0][2] * p.x + camera_matrix[1][2] * p.y + camera_matrix[2][2] * z_translated;

    return vec3<f32>(x, y, z);
}

// Apply Apophysis perspective projection
// Source: Apophysis ControlPoint.pas:606
fn apply_perspective(p: vec3<f32>, persp_strength: f32) -> vec2<f32> {
    if (abs(persp_strength) < 1e-6) {
        // Orthographic: no perspective
        return p.xy;
    }

    // Apophysis formula: zr = 1 - cameraPersp * z
    let zr = 1.0 - persp_strength * p.z;

    // Avoid division by zero
    if (abs(zr) < 1e-6) {
        return p.xy;
    }

    // Perspective divide: x' = x / zr, y' = y / zr
    return p.xy / zr;
}

// Complete Apophysis 3D → 2D projection pipeline
fn project_3d_to_2d_apophysis(
    p: vec3<f32>,
    pitch: f32,
    yaw: f32,
    camera_z: f32,
    persp_strength: f32
) -> vec2<f32> {
    // Build camera matrix
    let camera_matrix = build_camera_matrix(pitch, yaw);

    // Transform to camera space (translate + rotate)
    let camera_space = camera_transform(p, camera_matrix, camera_z);

    // Apply perspective projection
    return apply_perspective(camera_space, persp_strength);
}

// Convert 3D fractal coords to pixel coords (with Apophysis camera system)
fn world_to_pixel_3d(p: vec3<f32>) -> vec2<i32> {
    // Apply Apophysis 3D → 2D projection (camera transform + perspective)
    let p2d = project_3d_to_2d_apophysis(
        p,
        params.camera_rotation_x,  // pitch
        params.camera_rotation_y,  // yaw
        params.camera_z,
        params.perspective_strength
    );

    // Apply view transform: pan, rotation, and zoom
    var transformed = p2d - vec2<f32>(params.pan_x, params.pan_y);

    // Apply rotation
    let cos_r = cos(params.rotation);
    let sin_r = sin(params.rotation);
    transformed = vec2<f32>(
        transformed.x * cos_r - transformed.y * sin_r,
        transformed.x * sin_r + transformed.y * cos_r
    );

    // Apply zoom
    transformed = transformed * params.zoom;

    // Map from fractal space (typically -2 to 2) to pixel space
    let scale = f32(min(params.width, params.height)) * 0.25;
    let center = vec2<f32>(f32(params.width), f32(params.height)) * 0.5;
    let pixel = center + transformed * scale;
    return vec2<i32>(i32(pixel.x), i32(pixel.y));
}

// Convert 2D fractal coords to pixel coords
fn world_to_pixel(p: vec2<f32>) -> vec2<i32> {
    // Apply view transform: pan, rotation, and zoom
    var transformed = p - vec2<f32>(params.pan_x, params.pan_y);

    // Apply rotation
    let cos_r = cos(params.rotation);
    let sin_r = sin(params.rotation);
    transformed = vec2<f32>(
        transformed.x * cos_r - transformed.y * sin_r,
        transformed.x * sin_r + transformed.y * cos_r
    );

    // Apply zoom
    transformed = transformed * params.zoom;

    // Map from fractal space (typically -2 to 2) to pixel space
    let scale = f32(min(params.width, params.height)) * 0.25;
    let center = vec2<f32>(f32(params.width), f32(params.height)) * 0.5;
    let pixel = center + transformed * scale;
    return vec2<i32>(i32(pixel.x), i32(pixel.y));
}

// Main compute shader template
// This template generates variants via conditional compilation:
//   - 2D mode (vec2 points) vs 3D mode (vec3 points)
//   - Simple (no path tracking) vs Full (with path tracking)
//   - Standard vs Xaos (chaos-weighted transform selection)
//
// Conditional markers:
//    ... 
//   
//    ... 
//
// Uses hard-coded constants (compiled at shader build time):
//   NUM_TRANSFORMS, COLOR_MODE, HAS_FINAL_TRANSFORM, FINAL_TRANSFORM_INDEX
// These enable dead code elimination and loop unrolling optimizations.

@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let thread_id = global_id.x;

    // Initialize RNG
    var rng = rng_init(thread_id, params.seed);

    // Starting point (random in [-1, 1])


    var current = vec3<f32>(
        rng_nextf(&rng) * 2.0 - 1.0,
        rng_nextf(&rng) * 2.0 - 1.0,
        rng_nextf(&rng) * 2.0 - 1.0
    );



    var color = vec3<f32>(1.0, 1.0, 1.0);
    var color_index = 0.0;  // For palette mode





    // Per-thread state initialization for stateful variations that need
    // values beyond zero-fill (var<private> thread_state is already zeroed
    // by WGSL spec; this block runs the wgsl_state_init fragments declared
    // by active variations). Emitted by shader builder; empty for flames
    // with no custom-init variations.
    {
        let xform_id: u32 = 0u;
        let variation_id: u32 = 0u;
        let r = rng_nextf(rng);
        set_state(xform_id, variation_id, 0u, floor(r * 3.0));
        set_state(xform_id, variation_id, 1u, 0.0);
        set_state(xform_id, variation_id, 2u, 0.0);
    }


    // Iterate
    for (var i = 0u; i < params.iterations_per_thread; i++) {
        // Save old position for speed calculation
        let old_pos = current;

        // Select random transform
        let rand_val = rng_nextf(&rng);

        // Standard: uses hard-coded NUM_TRANSFORMS for loop unrolling
        let xform_idx = select_transform_const(rand_val);

        let xform = transforms[xform_idx];

        // Opacity check (stochastic transparency)
        // Note: We still apply the transform even when opacity=0 - opacity affects
        // visibility only, not IFS dynamics. Transform must update position for correct chaos game.
        let should_plot = rng_nextf(&rng) < xform.opacity;


        // No DC variations active: original 2-step flow — variations first,
        // then color_speed blend. Bit-identical to pre-direct-color codebase.
        let affine_p = apply_affine(xform, current);
        current = apply_variations(xform, xform_idx, affine_p, &rng);


        // Apply post-affine (compile-time gated for zero cost when unused)
        if (HAS_POST_AFFINE) {
            if (xform.post_enabled > 0.5) {
                current = apply_post_affine(xform, current);
            }
        }

        // Calculate speed (distance traveled)
        let speed = length(current - old_pos);


        // Original Step 1 (palette mode), or speed-based color (speed mode).
        if (COLOR_MODE == 0u) {
            let symmetry = xform.color_speed;
            let colorC1 = (1.0 + symmetry) / 2.0;
            let colorC2 = xform.color * (1.0 - symmetry) / 2.0;
            color_index = color_index * colorC1 + colorC2;
        } else if (COLOR_MODE == 1u) {
            let speed_color = speed_to_color(speed);
            color = mix(color, speed_color, params.speed_factor);
        }


        // Note: COLOR_MODE == 2 (PathMap) uses the full shader with path tracking




        // Skip burn-in iterations
        if (i >= params.burn_in) {
            // Apply final transform if present (hard-coded HAS_FINAL_TRANSFORM)
            var final_pos = current;
            if (HAS_FINAL_TRANSFORM) {
                let final_xform = transforms[FINAL_TRANSFORM_INDEX];


                let affine_p = apply_affine(final_xform, final_pos);
                final_pos = apply_variations(final_xform, FINAL_TRANSFORM_INDEX, affine_p, &rng);


                // Post-affine on final transform
                if (HAS_POST_AFFINE) {
                    if (final_xform.post_enabled > 0.5) {
                        final_pos = apply_post_affine(final_xform, final_pos);
                    }
                }
            }

            // Convert to pixel coordinates

            var pixel = world_to_pixel_3d(final_pos);

            // Apply depth of field blur (3D mode only)
            if (params.dof_blur_strength > 0.0) {
                // Transform to camera space to get depth along view direction
                let camera_matrix = build_camera_matrix(params.camera_rotation_x, params.camera_rotation_y);
                let camera_space = camera_transform(final_pos, camera_matrix, params.camera_z);
                let depth = camera_space.z;  // Z in camera space = depth from camera

                // Calculate blur amount based on distance from focus plane (in world units)
                let blur_world = (depth - params.dof_focus_distance) * params.dof_blur_strength;

                // Convert world-space blur to pixel-space blur
                // Same scale factor as world_to_pixel_3d: min(width, height) * 0.25 * zoom
                let pixel_scale = f32(min(params.width, params.height)) * 0.25 * params.zoom;
                let blur_pixels = abs(blur_world) * pixel_scale;

                // Generate random offset in disk shape (uniform disk distribution)
                let angle = rng_nextf(&rng) * 6.28318530718;  // 2*PI
                let radius = sqrt(rng_nextf(&rng)) * blur_pixels;
                pixel = pixel + vec2<i32>(i32(cos(angle) * radius), i32(sin(angle) * radius));
            }


            // Check bounds and opacity (only plot if both pass)
            if (pixel.x >= 0 && pixel.x < i32(params.width) &&
                pixel.y >= 0 && pixel.y < i32(params.height) && should_plot) {

                let pixel_idx = u32(pixel.y) * params.width + u32(pixel.x);

                // Determine final color based on mode (hard-coded COLOR_MODE)
                var final_color: vec3<f32>;
                if (COLOR_MODE == 0u) {
                    // Palette mode: sample from palette texture using color_index
                    final_color = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(color_index, 0.5), 0.0).rgb;
                } else if (COLOR_MODE == 1u) {
                    // Speed mode: uses accumulated RGB color
                    final_color = color;

                }


                // Apply depth fog (3D mode only, blend toward background color)
                if (params.fog_strength > 0.0) {
                    // Get camera-space depth
                    // In camera space, objects in front have negative Z (looking down -Z axis)
                    // Negate to get positive depth where larger = further from camera
                    let camera_matrix = build_camera_matrix(params.camera_rotation_x, params.camera_rotation_y);
                    let camera_space = camera_transform(final_pos, camera_matrix, params.camera_z);
                    let fog_depth = -camera_space.z;  // Negate: distant objects have larger depth

                    // Exponential fog: fog_factor increases with distance beyond fog_start
                    let fog_distance = max(fog_depth - params.fog_start, 0.0);
                    let fog_factor = 1.0 - exp(-params.fog_strength * fog_distance);

                    // Blend toward background color
                    let background = vec3<f32>(params.background_r, params.background_g, params.background_b);
                    final_color = mix(final_color, background, fog_factor);
                }


                // Atomic accumulation to histogram buffer
                // Write RGB as 4× u32 (unpacked, full 32-bit precision)
                let base_idx = pixel_idx * 4u;  // 4 words per pixel (R, G, B, density)

                // Use global color scale from params (uniform constant, fast access)
                let color_scale = params.histogram_color_scale;

                // Convert colors to u32 using global scale
                // No packing needed - each channel gets its own u32 word
                let r_u32 = u32(clamp(final_color.r, 0.0, 1.0) * color_scale);
                let g_u32 = u32(clamp(final_color.g, 0.0, 1.0) * color_scale);
                let b_u32 = u32(clamp(final_color.b, 0.0, 1.0) * color_scale);
                let density_u32 = u32(color_scale);  // Density includes scale (u32 prevents overflow)

                // Atomic add to histogram (4 separate u32 words)
                atomicAdd(&histogram[base_idx + 0u], r_u32);
                atomicAdd(&histogram[base_idx + 1u], g_u32);
                atomicAdd(&histogram[base_idx + 2u], b_u32);
                atomicAdd(&histogram[base_idx + 3u], density_u32);

                // Increment iteration count for this pixel (for per-pixel convergence tracking)
                atomicAdd(&iteration_counts[pixel_idx], 1u);
            }
        }
    }
}
