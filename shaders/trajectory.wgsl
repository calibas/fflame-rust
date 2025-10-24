// Trajectory compute shader for fractal flame rendering
// Generates points via iterated function system (IFS)

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

    // Variation weights (50 slots: 26 current + 24 reserved for plugins)
    variations: array<f32, 50>,

    // Color (RGB)
    color: vec3<f32>,

    // Color speed
    color_speed: f32,
}

// Dispatch parameters
struct Params {
    num_transforms: u32,
    iterations_per_thread: u32,
    burn_in: u32,
    width: u32,
    height: u32,
    seed: u32,
    color_mode: u32,  // 0 = transform colors, 1 = palette, 2 = speed
    render_mode: u32,  // 0 = 2D, 1 = 3D
    projection_type: u32,  // 0 = orthographic, 1 = perspective
    splat_size: f32,
    zoom: f32,
    pan_x: f32,
    pan_y: f32,
    rotation: f32,  // Rotation in radians (2D, around Z)
    speed_factor: f32,  // Blend factor for speed-based coloring
    perspective_strength: f32,  // Strength for perspective projection
    camera_rotation_x: f32,  // 3D camera pitch (rotation around X)
    camera_rotation_y: f32,  // 3D camera yaw (rotation around Y)
    _pad3: f32,
    _pad4: f32,
}

// Bindings
@group(0) @binding(0) var<storage, read> transforms: array<Transform>;
@group(0) @binding(1) var<uniform> params: Params;
@group(0) @binding(2) var output_texture: texture_storage_2d<rgba16float, write>;
@group(0) @binding(3) var palette_texture: texture_1d<f32>;
@group(0) @binding(4) var palette_sampler: sampler;

// PCG random number generator state
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

// Apply affine transformation
fn apply_affine(xform: Transform, p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(
        xform.a * p.x + xform.b * p.y + xform.e,
        xform.c * p.x + xform.d * p.y + xform.f
    );
}

// Variation functions
fn variation_linear(p: vec2<f32>) -> vec2<f32> {
    return p;
}

fn variation_sinusoidal(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(sin(p.x), sin(p.y));
}

fn variation_spherical(p: vec2<f32>) -> vec2<f32> {
    let r2 = dot(p, p) + 1e-6;
    return p / r2;
}

fn variation_swirl(p: vec2<f32>) -> vec2<f32> {
    let r2 = dot(p, p);
    let s = sin(r2);
    let c = cos(r2);
    return vec2<f32>(p.x * s - p.y * c, p.x * c + p.y * s);
}

fn variation_horseshoe(p: vec2<f32>) -> vec2<f32> {
    let r = length(p) + 1e-6;
    let r_inv = 1.0 / r;
    return vec2<f32>(
        (p.x - p.y) * (p.x + p.y) * r_inv,
        2.0 * p.x * p.y * r_inv
    );
}

fn variation_polar(p: vec2<f32>) -> vec2<f32> {
    let theta = atan2(p.y, p.x);
    let r = length(p);
    return vec2<f32>(theta / 3.14159265359, r - 1.0);
}

fn variation_handkerchief(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);
    let theta_r = theta + r;
    return vec2<f32>(r * sin(theta_r), r * cos(theta_r));
}

fn variation_heart(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);
    let r_theta = r * theta;
    return vec2<f32>(r * sin(r_theta), -r * cos(r_theta));
}

fn variation_disc(p: vec2<f32>) -> vec2<f32> {
    let theta = atan2(p.y, p.x);
    let r = length(p);
    let theta_pi = theta / 3.14159265359;
    let pi_r = 3.14159265359 * r;
    return vec2<f32>(theta_pi * sin(pi_r), theta_pi * cos(pi_r));
}

fn variation_spiral(p: vec2<f32>) -> vec2<f32> {
    let r = length(p) + 1e-6;
    let theta = atan2(p.y, p.x);
    let r_inv = 1.0 / r;
    return vec2<f32>(
        r_inv * (cos(theta) + sin(theta)),
        r_inv * (cos(theta) - sin(theta))
    );
}

fn variation_hyperbolic(p: vec2<f32>) -> vec2<f32> {
    let r = length(p) + 1e-6;
    let theta = atan2(p.y, p.x);
    return vec2<f32>(sin(theta) / r, r * cos(theta));
}

fn variation_diamond(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);
    return vec2<f32>(sin(theta) * cos(r), cos(theta) * sin(r));
}

fn variation_ex(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);
    let p0 = theta + r;
    let p1 = theta - r;
    let p0_sin = sin(p0);
    let p1_sin = sin(p1);
    let p0_cubed = p0_sin * p0_sin * p0_sin;
    let p1_cubed = p1_sin * p1_sin * p1_sin;
    return vec2<f32>(r * (p0_cubed + p1_cubed), r * (p0_cubed - p1_cubed));
}

fn variation_julia(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);
    let sqrt_r = sqrt(r);
    let omega = select(0.0, 3.14159265359, rng_nextf(rng) < 0.5);
    let half_theta = theta / 2.0 + omega;
    return vec2<f32>(sqrt_r * cos(half_theta), sqrt_r * sin(half_theta));
}

fn variation_bent(p: vec2<f32>) -> vec2<f32> {
    let nx = select(2.0 * p.x, p.x, p.x >= 0.0);
    let ny = select(p.y / 2.0, p.y, p.y >= 0.0);
    return vec2<f32>(nx, ny);
}

fn variation_waves(p: vec2<f32>) -> vec2<f32> {
    let b = 0.5;
    let c = 0.5;
    let e = 0.5;
    let f = 0.5;
    return vec2<f32>(
        p.x + b * sin(p.y / (c * c + 1e-6)),
        p.y + e * sin(p.x / (f * f + 1e-6))
    );
}

// Apply all variations with weights
fn apply_variations(xform: Transform, p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    var result = vec2<f32>(0.0, 0.0);

    // Variation 0: Linear
    if (xform.variations[0] != 0.0) {
        result += xform.variations[0] * variation_linear(p);
    }
    // Variation 1: Sinusoidal
    if (xform.variations[1] != 0.0) {
        result += xform.variations[1] * variation_sinusoidal(p);
    }
    // Variation 2: Spherical
    if (xform.variations[2] != 0.0) {
        result += xform.variations[2] * variation_spherical(p);
    }
    // Variation 3: Swirl
    if (xform.variations[3] != 0.0) {
        result += xform.variations[3] * variation_swirl(p);
    }
    // Variation 4: Horseshoe
    if (xform.variations[4] != 0.0) {
        result += xform.variations[4] * variation_horseshoe(p);
    }
    // Variation 5: Polar
    if (xform.variations[5] != 0.0) {
        result += xform.variations[5] * variation_polar(p);
    }
    // Variation 6: Handkerchief
    if (xform.variations[6] != 0.0) {
        result += xform.variations[6] * variation_handkerchief(p);
    }
    // Variation 7: Heart
    if (xform.variations[7] != 0.0) {
        result += xform.variations[7] * variation_heart(p);
    }
    // Variation 8: Disc
    if (xform.variations[8] != 0.0) {
        result += xform.variations[8] * variation_disc(p);
    }
    // Variation 9: Spiral
    if (xform.variations[9] != 0.0) {
        result += xform.variations[9] * variation_spiral(p);
    }
    // Variation 10: Hyperbolic
    if (xform.variations[10] != 0.0) {
        result += xform.variations[10] * variation_hyperbolic(p);
    }
    // Variation 11: Diamond
    if (xform.variations[11] != 0.0) {
        result += xform.variations[11] * variation_diamond(p);
    }
    // Variation 12: Ex
    if (xform.variations[12] != 0.0) {
        result += xform.variations[12] * variation_ex(p);
    }
    // Variation 13: Julia
    if (xform.variations[13] != 0.0) {
        result += xform.variations[13] * variation_julia(p, rng);
    }
    // Variation 14: Bent
    if (xform.variations[14] != 0.0) {
        result += xform.variations[14] * variation_bent(p);
    }
    // Variation 15: Waves
    if (xform.variations[15] != 0.0) {
        result += xform.variations[15] * variation_waves(p);
    }

    return result;
}

// Select transform based on cumulative weights
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

// Convert fractal space coords to pixel coords
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

// Convert speed to color using palette lookup
fn speed_to_color(speed: f32) -> vec3<f32> {
    // Normalize speed to [0, 1] range
    // Use logarithmic scale for better visualization
    let normalized_speed = clamp(log(speed * 10.0 + 1.0) / 3.0, 0.0, 1.0);

    // Sample from palette texture
    return textureSampleLevel(palette_texture, palette_sampler, normalized_speed, 0.0).rgb;
}

// Main compute shader
@compute @workgroup_size(64, 1, 1)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    let thread_id = global_id.x;

    // Initialize RNG
    var rng = rng_init(thread_id, params.seed);

    // Starting point (random in [-1, 1])
    var current = vec2<f32>(
        rng_nextf(&rng) * 2.0 - 1.0,
        rng_nextf(&rng) * 2.0 - 1.0
    );

    var color = vec3<f32>(1.0, 1.0, 1.0);
    var color_index = 0.0;  // For palette mode

    // Iterate
    for (var i = 0u; i < params.iterations_per_thread; i++) {
        // Save old position for speed calculation
        let old_pos = current;

        // Select random transform
        let rand_val = rng_nextf(&rng);
        let xform_idx = select_transform(rand_val);
        let xform = transforms[xform_idx];

        // Apply affine + variations
        let affine_p = apply_affine(xform, current);
        current = apply_variations(xform, affine_p, &rng);

        // Calculate speed (distance traveled)
        let speed = length(current - old_pos);

        // Update color based on color mode
        if (params.color_mode == 0u) {
            // Transform color mode: blend with transform color
            color = mix(color, xform.color, xform.color_speed);
        } else if (params.color_mode == 1u) {
            // Palette mode: blend color index
            let xform_color_value = (xform.color.r + xform.color.g + xform.color.b) / 3.0;
            color_index = mix(color_index, xform_color_value, xform.color_speed);
        } else {
            // Speed mode: blend with speed-based color
            let speed_color = speed_to_color(speed);
            color = mix(color, speed_color, params.speed_factor);
        }

        // Skip burn-in iterations
        if (i >= params.burn_in) {
            // Convert to pixel coordinates
            let pixel = world_to_pixel(current);

            // Check bounds
            if (pixel.x >= 0 && pixel.x < i32(params.width) &&
                pixel.y >= 0 && pixel.y < i32(params.height)) {

                // Determine final color based on mode
                var final_color: vec3<f32>;
                if (params.color_mode == 0u) {
                    final_color = color;
                } else if (params.color_mode == 1u) {
                    // Sample from palette texture
                    final_color = textureSampleLevel(palette_texture, palette_sampler, color_index, 0.0).rgb;
                } else {
                    // Speed mode uses accumulated color
                    final_color = color;
                }

                // Write to texture with small alpha for density accumulation
                // Each hit contributes a small amount to density
                // The accumulation pass will sum these up
                textureStore(output_texture, pixel, vec4<f32>(final_color, 0.01));
            }
        }
    }
}
