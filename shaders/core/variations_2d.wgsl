// Core 2D variations (indices 0-15)
// Optimized to use precalculated values (r, r2, theta, sin_theta, cos_theta)

fn variation_linear(p: vec2<f32>) -> vec2<f32> {
    return p;
}

fn variation_sinusoidal(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(sin(p.x), sin(p.y));
}

fn variation_spherical(p: vec2<f32>, r2: f32) -> vec2<f32> {
    let r2_safe = r2 + 1e-6;
    return p / r2_safe;
}

fn variation_swirl(p: vec2<f32>, r2: f32) -> vec2<f32> {
    let s = sin(r2);
    let c = cos(r2);
    return vec2<f32>(p.x * s - p.y * c, p.x * c + p.y * s);
}

fn variation_horseshoe(p: vec2<f32>, r: f32) -> vec2<f32> {
    let r_safe = r + 1e-6;
    let r_inv = 1.0 / r_safe;
    return vec2<f32>(
        (p.x - p.y) * (p.x + p.y) * r_inv,
        2.0 * p.x * p.y * r_inv
    );
}

fn variation_polar(theta: f32, r: f32) -> vec2<f32> {
    return vec2<f32>(theta / 3.14159265359, r - 1.0);
}

fn variation_handkerchief(r: f32, theta: f32) -> vec2<f32> {
    let theta_r = theta + r;
    return vec2<f32>(r * sin(theta_r), r * cos(theta_r));
}

fn variation_heart(r: f32, theta: f32) -> vec2<f32> {
    let r_theta = r * theta;
    return vec2<f32>(r * sin(r_theta), -r * cos(r_theta));
}

fn variation_disc(theta: f32, r: f32) -> vec2<f32> {
    let theta_pi = theta / 3.14159265359;
    let pi_r = 3.14159265359 * r;
    return vec2<f32>(theta_pi * sin(pi_r), theta_pi * cos(pi_r));
}

fn variation_spiral(r: f32, sin_theta: f32, cos_theta: f32) -> vec2<f32> {
    let r_safe = r + 1e-6;
    let r_inv = 1.0 / r_safe;
    return vec2<f32>(
        r_inv * (cos_theta + sin_theta),
        r_inv * (cos_theta - sin_theta)
    );
}

fn variation_hyperbolic(r: f32, sin_theta: f32, cos_theta: f32) -> vec2<f32> {
    let r_safe = r + 1e-6;
    return vec2<f32>(sin_theta / r_safe, r_safe * cos_theta);
}

fn variation_diamond(r: f32, sin_theta: f32, cos_theta: f32) -> vec2<f32> {
    return vec2<f32>(sin_theta * cos(r), cos_theta * sin(r));
}

fn variation_ex(r: f32, theta: f32) -> vec2<f32> {
    let p0 = theta + r;
    let p1 = theta - r;
    let p0_sin = sin(p0);
    let p1_sin = sin(p1);
    let p0_cubed = p0_sin * p0_sin * p0_sin;
    let p1_cubed = p1_sin * p1_sin * p1_sin;
    return vec2<f32>(r * (p0_cubed + p1_cubed), r * (p0_cubed - p1_cubed));
}

fn julia(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);  // Julia uses standard atan2(y,x) convention
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

fn variation_julian(p: vec2<f32>, xform_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Get parameters
    let power = get_param(xform_id, 14u, 0u);  // julian is variation index 14 (after julia at 13)
    let dist = get_param(xform_id, 14u, 1u);

    let abs_power = abs(power);
    let cpower = dist / abs_power / 2.0;

    let r = pow(length(p), cpower);
    let theta = atan2(p.y, p.x);  // JuliaN uses standard atan2(y,x) convention

    // Random selection of symmetry
    let trunc_val = floor(abs_power * rng_nextf(rng));
    let t = (theta + 6.28318530718 * trunc_val) / power;

    return vec2<f32>(r * cos(t), r * sin(t));
}

fn variation_blob(r: f32, theta: f32, sin_theta: f32, cos_theta: f32, xform_id: u32) -> vec2<f32> {
    // Get parameters: p1 = high, p2 = low, p3 = waves
    let p1 = get_param(xform_id, 17u, 0u);  // high
    let p2 = get_param(xform_id, 17u, 1u);  // low
    let p3 = get_param(xform_id, 17u, 2u);  // waves

    // r · (p2 + ((p1 − p2)/2)(sin(p3θ) + 1))
    let scale = r * (p2 + ((p1 - p2) / 2.0) * (sin(p3 * theta) + 1.0));

    return vec2<f32>(scale * cos_theta, scale * sin_theta);
}
