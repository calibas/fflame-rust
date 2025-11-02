// Core variations for 3D mode
// Includes 2D variations (0-15) adapted for vec3 and 3D-specific variations (16-23)

// Affine transformation for 3D (XY transformed, Z offset)
// Standard affine formula: x' = ax + by + e, y' = cx + dy + f
fn apply_affine(xform: Transform, p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(
        xform.a * p.x + xform.b * p.y + xform.e,
        xform.c * p.x + xform.d * p.y + xform.f,
        p.z + xform.g  // Z is just offset
    );
}

// === 2D Variations (0-15) - Adapted for vec3 (Z pass-through) ===
// Optimized to use precalculated values (r, r2, theta, sin_theta, cos_theta)

fn variation_linear(p: vec3<f32>) -> vec3<f32> {
    return p;
}

fn variation_sinusoidal(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(sin(p.x), sin(p.y), p.z);
}

fn variation_spherical(p: vec3<f32>, r2: f32) -> vec3<f32> {
    let r2_safe = r2 + 1e-6;
    return vec3<f32>((p.xy / r2_safe), p.z);
}

fn variation_swirl(p: vec3<f32>, r2: f32) -> vec3<f32> {
    let s = sin(r2);
    let c = cos(r2);
    return vec3<f32>(p.x * s - p.y * c, p.x * c + p.y * s, p.z);
}

fn variation_horseshoe(p: vec3<f32>, r: f32) -> vec3<f32> {
    let r_safe = r + 1e-6;
    let r_inv = 1.0 / r_safe;
    return vec3<f32>(
        (p.x - p.y) * (p.x + p.y) * r_inv,
        2.0 * p.x * p.y * r_inv,
        p.z
    );
}

fn variation_polar(theta: f32, r: f32, z: f32) -> vec3<f32> {
    return vec3<f32>(theta / 3.14159265359, r - 1.0, z);
}

fn variation_handkerchief(r: f32, theta: f32, z: f32) -> vec3<f32> {
    let theta_r = theta + r;
    return vec3<f32>(r * sin(theta_r), r * cos(theta_r), z);
}

fn variation_heart(r: f32, theta: f32, z: f32) -> vec3<f32> {
    let r_theta = r * theta;
    return vec3<f32>(r * sin(r_theta), -r * cos(r_theta), z);
}

fn variation_disc(theta: f32, r: f32, z: f32) -> vec3<f32> {
    let theta_pi = theta / 3.14159265359;
    let pi_r = 3.14159265359 * r;
    return vec3<f32>(theta_pi * sin(pi_r), theta_pi * cos(pi_r), z);
}

fn variation_spiral(r: f32, sin_theta: f32, cos_theta: f32, z: f32) -> vec3<f32> {
    let r_safe = r + 1e-6;
    let r_inv = 1.0 / r_safe;
    return vec3<f32>(
        r_inv * (cos_theta + sin_theta),
        r_inv * (cos_theta - sin_theta),
        z
    );
}

fn variation_hyperbolic(r: f32, sin_theta: f32, cos_theta: f32, z: f32) -> vec3<f32> {
    let r_safe = r + 1e-6;
    return vec3<f32>(sin_theta / r_safe, r_safe * cos_theta, z);
}

fn variation_diamond(r: f32, sin_theta: f32, cos_theta: f32, z: f32) -> vec3<f32> {
    return vec3<f32>(sin_theta * cos(r), cos_theta * sin(r), z);
}

fn variation_ex(r: f32, theta: f32, z: f32) -> vec3<f32> {
    let p0 = theta + r;
    let p1 = theta - r;
    let p0_sin = sin(p0);
    let p1_sin = sin(p1);
    let p0_cubed = p0_sin * p0_sin * p0_sin;
    let p1_cubed = p1_sin * p1_sin * p1_sin;
    return vec3<f32>(r * (p0_cubed + p1_cubed), r * (p0_cubed - p1_cubed), z);
}

fn julia(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let r = length(p.xy);
    let theta = atan2(p.y, p.x);  // Julia uses standard atan2(y,x) convention
    let sqrt_r = sqrt(r);
    let omega = select(0.0, 3.14159265359, rng_nextf(rng) < 0.5);
    let half_theta = theta / 2.0 + omega;
    return vec3<f32>(sqrt_r * cos(half_theta), sqrt_r * sin(half_theta), p.z);
}

fn variation_bent(p: vec3<f32>) -> vec3<f32> {
    let nx = select(2.0 * p.x, p.x, p.x >= 0.0);
    let ny = select(p.y / 2.0, p.y, p.y >= 0.0);
    return vec3<f32>(nx, ny, p.z);
}

fn variation_waves(p: vec3<f32>) -> vec3<f32> {
    let b = 0.5;
    let c = 0.5;
    let e = 0.5;
    let f = 0.5;
    return vec3<f32>(
        p.x + b * sin(p.y / (c * c + 1e-6)),
        p.y + e * sin(p.x / (f * f + 1e-6)),
        p.z
    );
}

fn variation_julian(p: vec3<f32>, xform_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Get parameters
    let power = get_param(xform_id, 14u, 0u);  // julian is variation index 14 (after julia at 13)
    let dist = get_param(xform_id, 14u, 1u);

    let abs_power = abs(power);
    let cpower = dist / abs_power / 2.0;

    let r = pow(length(p.xy), cpower);
    let theta = atan2(p.y, p.x);  // JuliaN uses standard atan2(y,x) convention

    // Random selection of symmetry
    let trunc_val = floor(abs_power * rng_nextf(rng));
    let t = (theta + 6.28318530718 * trunc_val) / power;

    return vec3<f32>(r * cos(t), r * sin(t), p.z);
}

fn variation_blob(r: f32, theta: f32, sin_theta: f32, cos_theta: f32, z: f32, xform_id: u32) -> vec3<f32> {
    // Get parameters: p1 = high, p2 = low, p3 = waves
    let p1 = get_param(xform_id, 17u, 0u);  // high
    let p2 = get_param(xform_id, 17u, 1u);  // low
    let p3 = get_param(xform_id, 17u, 2u);  // waves

    // r · (p2 + ((p1 − p2)/2)(sin(p3θ) + 1))
    let scale = r * (p2 + ((p1 - p2) / 2.0) * (sin(p3 * theta) + 1.0));

    return vec3<f32>(scale * cos_theta, scale * sin_theta, z);
}

// === 3D-Specific Utilities ===

// Rotate around X axis (affects Y and Z)
fn rotate_x(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(
        p.x,
        p.y * c - p.z * s,
        p.y * s + p.z * c
    );
}

// Rotate around Y axis (affects X and Z)
fn rotate_y(p: vec3<f32>, angle: f32) -> vec3<f32> {
    let c = cos(angle);
    let s = sin(angle);
    return vec3<f32>(
        p.x * c + p.z * s,
        p.y,
        -p.x * s + p.z * c
    );
}

// Hemisphere - project onto hemisphere
fn variation_hemisphere(p: vec3<f32>) -> vec3<f32> {
    let r2 = dot(p, p);
    let r = sqrt(r2);
    if (r < 1e-6) {
        return vec3<f32>(0.0, 0.0, 1.0);
    }
    let scale = 1.0 / r;
    let z = max(0.0, sqrt(1.0 - min(1.0, r2)));
    return vec3<f32>(p.x * scale, p.y * scale, z);
}
