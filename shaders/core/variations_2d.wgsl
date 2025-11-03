// Core 2D variations (indices 0-15)
// These are always compiled and available

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
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    let r = length(p);
    return vec2<f32>(theta / 3.14159265359, r - 1.0);
}

fn variation_handkerchief(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);  // Standard atan2(y,x) for plugin variations
    return vec2<f32>(r * sin(theta + r), r * cos(theta - r));
}

fn variation_heart(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    let r_theta = r * theta;
    return vec2<f32>(r * sin(r_theta), -r * cos(r_theta));
}

fn variation_disc(p: vec2<f32>) -> vec2<f32> {
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    let r = length(p);
    let theta_pi = theta / 3.14159265359;
    let pi_r = 3.14159265359 * r;
    return vec2<f32>(theta_pi * sin(pi_r), theta_pi * cos(pi_r));
}

fn variation_spiral(p: vec2<f32>) -> vec2<f32> {
    let r = length(p) + 1e-6;
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    let r_inv = 1.0 / r;
    return vec2<f32>(
        r_inv * (cos(theta) + sin(r)),
        r_inv * (sin(theta) - cos(r))
    );
}

fn variation_hyperbolic(p: vec2<f32>) -> vec2<f32> {
    let r2 = dot(p, p) + 1e-6;  // x² + y² + epsilon
    return vec2<f32>(p.x / r2, p.y);
}

fn variation_diamond(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y) not atan2(y,x)
    return vec2<f32>(sin(theta) * cos(r), cos(theta) * sin(r));
}

fn variation_ex(p: vec2<f32>) -> vec2<f32> {
    let r = length(p);
    let theta = atan2(p.y, p.x);  // Standard atan2(y,x) for plugin variations
    let n0 = sin(theta + r);
    let n1 = cos(theta - r);
    let m0 = n0 * n0 * n0;  // n0³
    let m1 = n1 * n1 * n1;  // n1³
    return vec2<f32>(r * (m0 + m1), r * (m0 - m1));
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

fn variation_julian(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Get parameters
    let power = get_param(xform_id, variation_id, 0u);
    let dist = get_param(xform_id, variation_id, 1u);

    let abs_power = abs(power);
    let cpower = dist / abs_power / 2.0;

    // Apophysis: r := Math.Power(sqr(FTx) + sqr(FTy), cN) = pow(x² + y², cN)
    let r2 = dot(p, p);  // x² + y²
    let r = pow(r2, cpower);
    let theta = atan2(p.y, p.x);  // JuliaN uses standard atan2(y,x) convention

    // Random selection of symmetry
    let trunc_val = floor(abs_power * rng_nextf(rng));
    let t = (theta + 6.28318530718 * trunc_val) / power;

    return vec2<f32>(r * cos(t), r * sin(t));
}

fn variation_blob(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Get parameters: p1 = high, p2 = low, p3 = waves
    let p1 = get_param(xform_id, variation_id, 0u);  // high
    let p2 = get_param(xform_id, variation_id, 1u);  // low
    let p3 = get_param(xform_id, variation_id, 2u);  // waves

    let r = length(p);
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)

    // r · (p2 + ((p1 − p2)/2)(sin(p3θ) + 1))
    let scale = r * (p2 + ((p1 - p2) / 2.0) * (sin(p3 * theta) + 1.0));

    return vec2<f32>(scale * cos(theta), scale * sin(theta));
}

fn variation_eyefish(p: vec2<f32>) -> vec2<f32> {
    // Apophysis: r = 2 / (sqrt(x² + y²) + 1)
    let r_xy = length(p) + 1.0;
    let scale = 2.0 / r_xy;
    return vec2<f32>(scale * p.x, scale * p.y);
}

fn variation_bubble(p: vec2<f32>) -> vec2<f32> {
    // Apophysis: r = (x² + y²)/4 + 1
    // scale = 1 / r
    let r2 = dot(p, p);
    let r = r2 / 4.0 + 1.0;
    let scale = 1.0 / r;
    return vec2<f32>(scale * p.x, scale * p.y);
}

fn variation_cylinder(p: vec2<f32>) -> vec2<f32> {
    // Apophysis: result = (sin(x), y)
    return vec2<f32>(sin(p.x), p.y);
}

fn variation_noise(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis: Random polar displacement
    // θ = random × 2π, r = random
    // result = (x × r × cos(θ), y × r × sin(θ))
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng);
    return vec2<f32>(p.x * r * cos(theta), p.y * r * sin(theta));
}
