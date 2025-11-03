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

fn variation_blur(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis: Random circular blur
    // θ = random × 2π, r = random
    // result = (r × cos(θ), r × sin(θ))
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng);
    return vec2<f32>(r * cos(theta), r * sin(theta));
}

fn variation_gaussian_blur(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis: Gaussian distributed blur
    // θ = random × 2π
    // r = (rand₁ + rand₂ + rand₃ + rand₄ - 2) - Gaussian approximation via central limit theorem
    // result = (r × cos(θ), r × sin(θ))
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec2<f32>(r * cos(theta), r * sin(theta));
}

fn variation_zblur(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    // ZBlur only affects Z (3D mode), pass through in 2D
    return p;
}

fn variation_blur3d(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis: 3D Gaussian spherical blur
    // In 2D mode, apply XY components only
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let phi = rng_nextf(rng) * 3.14159265359;    // π
    let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec2<f32>(r * sin(phi) * cos(theta), r * sin(phi) * sin(theta));
}

fn variation_pre_blur(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis: Pre-phase Gaussian blur applied before variations
    // FTx += r * cos(θ), FTy += r * sin(θ)
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec2<f32>(p.x + r * cos(theta), p.y + r * sin(theta));
}

fn variation_pre_zscale(p: vec2<f32>, weight: f32) -> vec2<f32> {
    // Pre_ZScale only affects Z (3D mode), pass through in 2D
    return p;
}

fn variation_pre_ztranslate(p: vec2<f32>, weight: f32) -> vec2<f32> {
    // Pre_ZTranslate only affects Z (3D mode), pass through in 2D
    return p;
}

fn variation_ztranslate(p: vec2<f32>) -> vec2<f32> {
    // ZTranslate only affects Z (3D mode), pass through in 2D
    return p;
}

fn variation_waves2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Waves2: Sine wave distortion with 6 parameters
    // FPx += VVAR * (FTx + scalex * sin(FTy * freqx))
    // FPy += VVAR * (FTy + scaley * sin(FTx * freqy))
    let freqx = get_param(xform_id, variation_id, 0u);
    let scalex = get_param(xform_id, variation_id, 1u);
    let freqy = get_param(xform_id, variation_id, 2u);
    let scaley = get_param(xform_id, variation_id, 3u);

    let new_x = p.x + scalex * sin(p.y * freqx);
    let new_y = p.y + scaley * sin(p.x * freqy);

    return vec2<f32>(new_x, new_y);
}

fn variation_julia3d(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Julia3D in 2D mode (Z = 0)
    let power_f = get_param(xform_id, variation_id, 0u);
    let N = i32(power_f);

    // Handle special case: power = 0 becomes power = 1
    if (N == 0) {
        return p;
    }

    let absN = abs(N);
    let absN_f = f32(absN);

    // Special optimized cases
    if (N == 1) {
        return p;  // Linear
    } else if (N == -1) {
        let r2 = dot(p, p);
        return p / r2;  // Inversion
    } else if (N == 2) {
        let r2d = dot(p, p);
        let r = 1.0 / sqrt(sqrt(r2d));
        let angle = atan2(p.y, p.x) / 2.0 + 3.14159265359 * f32(i32(rng_nextf(rng) * 2.0));
        return vec2<f32>(r * sqrt(r2d) * cos(angle), r * sqrt(r2d) * sin(angle));
    } else if (N == -2) {
        let r2d = dot(p, p);
        let r3d = sqrt(r2d);
        let r = 1.0 / (sqrt(r3d) * r3d);
        let angle = atan2(p.y, p.x) / 2.0 + 3.14159265359 * f32(i32(rng_nextf(rng) * 2.0));
        return vec2<f32>(r * sqrt(r2d) * cos(angle), -r * sqrt(r2d) * sin(angle));
    } else {
        // General case
        let r2d = dot(p, p);
        let cN = (1.0 / power_f - 1.0) / 2.0;
        let r = pow(r2d, cN);

        let random_idx = i32(rng_nextf(rng) * absN_f);
        let angle = (atan2(p.y, p.x) + 6.28318530718 * f32(random_idx)) / power_f;
        let tmp = r * sqrt(r2d);

        if (N > 0) {
            return vec2<f32>(tmp * cos(angle), tmp * sin(angle));
        } else {
            return vec2<f32>(tmp * cos(angle), -tmp * sin(angle));
        }
    }
}

fn variation_log(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Log: Logarithmic transformation
    // FPx += vvar * ln(x² + y²) * (0.5 / ln(base))
    // FPy += vvar * atan2(y, x)
    let base = get_param(xform_id, variation_id, 0u);
    let denom = 0.5 / log(base);

    let r2 = dot(p, p);
    let new_x = log(r2) * denom;
    let new_y = atan2(p.y, p.x);

    return vec2<f32>(new_x, new_y);
}

fn variation_polar2(p: vec2<f32>) -> vec2<f32> {
    // Apophysis Polar2: Improved polar coordinates
    // FPx += vvar * atan2(x, y) / PI
    // FPy += vvar * 0.5 * ln(x² + y²) / PI
    const PI: f32 = 3.14159265359;
    let r2 = dot(p, p);
    let new_x = atan2(p.x, p.y) / PI;
    let new_y = 0.5 * log(r2) / PI;

    return vec2<f32>(new_x, new_y);
}

fn variation_cross(p: vec2<f32>) -> vec2<f32> {
    // Apophysis Cross: Cross/plus shape
    // r = abs((x - y) * (x + y) + 1e-6)
    // if r < 0: r *= -1
    // r = vvar / r
    // FPx += x * r
    // FPy += y * r
    var r = abs((p.x - p.y) * (p.x + p.y) + 1e-6);
    if (r < 0.0) {
        r = r * -1.0;
    }
    r = 1.0 / r;

    return p * r;
}

fn variation_loonie(p: vec2<f32>) -> vec2<f32> {
    // Apophysis Loonie: Lune/crescent shape
    // r2 = x² + y²
    // if r2 < vvar² and r2 != 0:
    //   r = vvar * sqrt(vvar² / r2 - 1)
    //   FPx += r * x
    //   FPy += r * y
    // else:
    //   FPx += vvar * x
    //   FPy += vvar * y
    let r2 = dot(p, p);

    // Since we normalize by weight, sqrvar becomes 1.0
    if (r2 < 1.0 && r2 != 0.0) {
        let r = sqrt(1.0 / r2 - 1.0);
        return p * r;
    } else {
        return p;
    }
}

fn variation_escher(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Escher: Escher-style tessellation
    // beta = parameter
    // c = 0.5 * (1.0 + cos(beta))
    // d = 0.5 * sin(beta)
    // a = atan2(y, x)
    // lnr = 0.5 * ln(x² + y²)
    // m = vvar * exp(c * lnr - d * a)
    // x' = m * cos(c * a + d * lnr)
    // y' = m * sin(c * a + d * lnr)
    const PI: f32 = 3.14159265359;
    let beta = get_param(xform_id, variation_id, 0u) * PI / 180.0; // Convert degrees to radians
    let c = 0.5 * (1.0 + cos(beta));
    let d = 0.5 * sin(beta);

    let a = atan2(p.y, p.x);
    let lnr = 0.5 * log(dot(p, p));
    let m = exp(c * lnr - d * a);

    let angle = c * a + d * lnr;
    return vec2<f32>(m * cos(angle), m * sin(angle));
}

fn variation_scry(p: vec2<f32>) -> vec2<f32> {
    // Apophysis Scry: Crystal ball effect
    // t = x² + y²
    // r = 1 / (sqrt(t) * (t + 1/vvar))
    // Since we normalize by vvar: r = 1 / (sqrt(t) * (t + 1))
    // x' = x * r
    // y' = y * r
    let t = dot(p, p);
    var r = 1.0 / (sqrt(t) * (t + 1.0));

    return p * r;
}

fn variation_foci(p: vec2<f32>) -> vec2<f32> {
    // Apophysis Foci: Focal point distortion
    // expx = exp(x) * 0.5
    // expnx = 0.25 / expx = 0.5 * exp(-x)
    // tmp = vvar / (expx + expnx - cos(y))
    // Since we normalize: tmp = 1 / (expx + expnx - cos(y))
    // x' = (expx - expnx) * tmp
    // y' = sin(y) * tmp
    let expx = exp(p.x) * 0.5;
    let expnx = 0.5 * exp(-p.x);
    var tmp = expx + expnx - cos(p.y);

    if (tmp == 0.0) {
        tmp = 1e-6;
    }
    tmp = 1.0 / tmp;

    let new_x = (expx - expnx) * tmp;
    let new_y = sin(p.y) * tmp;

    return vec2<f32>(new_x, new_y);
}

fn variation_bipolar(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Bipolar: Bipolar coordinates
    // shift parameter controls vertical offset
    const PI: f32 = 3.14159265359;
    const HALF_PI: f32 = 1.57079632679489661923;
    let shift = get_param(xform_id, variation_id, 0u);

    let x2y2 = dot(p, p);
    var y = 0.5 * atan2(2.0 * p.y, x2y2 - 1.0) + (-HALF_PI * shift);

    // Wrap y to [-π/2, π/2]
    if (y > HALF_PI) {
        y = -HALF_PI + (y + HALF_PI) % PI;
    } else if (y < -HALF_PI) {
        y = HALF_PI - (HALF_PI - y) % PI;
    }

    let t = x2y2 + 1.0;
    let x2 = 2.0 * p.x;
    let f = t + x2;
    let g = t - x2;

    // Check for division by zero or log of negative
    if (g == 0.0 || f / g <= 0.0) {
        return vec2<f32>(0.0, 0.0);
    }

    // v_4 = vvar * 1/(2π), v = vvar * 2/π
    // Since we normalize by vvar: v_4 = 1/(2π), v = 2/π
    let new_x = (1.0 / (2.0 * PI)) * log(f / g);
    let new_y = (2.0 / PI) * y;

    return vec2<f32>(new_x, new_y);
}

fn variation_elliptic(p: vec2<f32>) -> vec2<f32> {
    // Apophysis Elliptic: Elliptic coordinates
    // v = vvar / (PI/2) = vvar * 2/PI
    // Since we normalize: v = 2/PI
    // tmp = y² + x² + 1
    // x2 = 2*x
    // xmax = 0.5 * (sqrt(tmp+x2) + sqrt(tmp-x2))
    // a = x/xmax
    // b = sqrt(max(0, 1 - a²))
    // x' = v * atan2(a, b)
    // y' = v * ln(xmax + sqrt(max(0, xmax-1))) if y > 0, else -v * ln(...)
    const PI: f32 = 3.14159265359;
    let v = 2.0 / PI;

    let tmp = dot(p, p) + 1.0;
    let x2 = 2.0 * p.x;
    let xmax = 0.5 * (sqrt(tmp + x2) + sqrt(tmp - x2));

    let a = p.x / xmax;
    let b = sqrt(max(0.0, 1.0 - a * a));

    let new_x = v * atan2(a, b);
    var new_y = v * log(xmax + sqrt(max(0.0, xmax - 1.0)));
    if (p.y < 0.0) {
        new_y = -new_y;
    }

    return vec2<f32>(new_x, new_y);
}

fn variation_lazysusan(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis LazySusan: Rotating lazy susan effect
    const PI: f32 = 3.14159265359;
    let spin = get_param(xform_id, variation_id, 0u) * PI / 180.0;
    let space = get_param(xform_id, variation_id, 1u);
    let twist = get_param(xform_id, variation_id, 2u);
    let x_offset = get_param(xform_id, variation_id, 3u);
    let y_offset = get_param(xform_id, variation_id, 4u);

    let x = p.x - x_offset;
    let y = p.y + y_offset;
    let r = sqrt(x * x + y * y);

    // Since we normalize by vvar, comparison becomes r < 1.0
    if (r < 1.0) {
        let a = atan2(y, x) + spin + twist * (1.0 - r);
        return vec2<f32>(r * cos(a) + x_offset, r * sin(a) - y_offset);
    } else {
        let r_scale = 1.0 + space / (r + 1e-6);
        return vec2<f32>(r_scale * x + x_offset, r_scale * y - y_offset);
    }
}

fn variation_falloff2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Falloff2: Distance-based blur effect with 3 modes
    const PI: f32 = 3.14159265359;
    let scatter = get_param(xform_id, variation_id, 0u);
    let mindist = get_param(xform_id, variation_id, 1u);
    let mul_x = get_param(xform_id, variation_id, 2u);
    let mul_y = get_param(xform_id, variation_id, 3u);
    let x0 = get_param(xform_id, variation_id, 6u);
    let y0 = get_param(xform_id, variation_id, 7u);
    let invert = get_param(xform_id, variation_id, 9u);
    let blur_type = get_param(xform_id, variation_id, 10u);

    let rmax = 0.04 * scatter;
    var d = sqrt((p.x - x0) * (p.x - x0) + (p.y - y0) * (p.y - y0));

    if (invert > 0.5) {
        d = 1.0 - d;
    }
    if (d < 0.0) {
        d = 0.0;
    }

    d = (d - mindist) * rmax;
    if (d < 0.0) {
        d = 0.0;
    }

    // Mode 0: Cartesian (original input space)
    if (blur_type < 0.5) {
        let rand_x = rng_next_f32(rng);
        let rand_y = rng_next_f32(rng);
        return vec2<f32>(
            p.x + mul_x * rand_x * d,
            p.y + mul_y * rand_y * d
        );
    }
    // Mode 1: Radial (polar coordinate space)
    else if (blur_type < 1.5) {
        let r_in = sqrt(p.x * p.x + p.y * p.y) + 1e-6;
        let phi = atan2(p.y, p.x) + mul_y * rng_next_f32(rng) * d;
        let r = r_in + mul_x * rng_next_f32(rng) * d;
        return vec2<f32>(r * cos(phi), r * sin(phi));
    }
    // Mode 2: Gaussian (spherical distribution)
    else {
        let phi = d * rng_next_f32(rng) * PI;
        let r = d * rng_next_f32(rng);
        return vec2<f32>(
            p.x + mul_x * r * cos(phi),
            p.y + mul_y * r * sin(phi)
        );
    }
}

fn variation_pre_spherical(p: vec2<f32>) -> vec2<f32> {
    // Apophysis Pre-Spherical: Pre-phase spherical distortion
    // Same as spherical but modifies input point before other variations
    let r = 1.0 / (dot(p, p) + 1e-5);
    return p * r;
}

fn variation_pre_sinusoidal(p: vec2<f32>) -> vec2<f32> {
    // Apophysis Pre-Sinusoidal: Pre-phase sinusoidal wave
    // Same as sinusoidal but modifies input point before other variations
    return vec2<f32>(sin(p.x), sin(p.y));
}

fn variation_pre_disc(p: vec2<f32>) -> vec2<f32> {
    // Apophysis Pre-Disc: Pre-phase disc transformation
    // Same as disc but modifies input point before other variations
    const PI: f32 = 3.14159265359;
    let r = sqrt(dot(p, p));
    let theta = atan2(p.y, p.x) / PI;
    let factor = theta * sin(PI * r);
    return vec2<f32>(factor * sin(PI * r), factor * cos(PI * r));
}
