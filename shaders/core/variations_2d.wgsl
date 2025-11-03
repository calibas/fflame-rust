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
        let rand_x = rng_nextf(rng);
        let rand_y = rng_nextf(rng);
        return vec2<f32>(
            p.x + mul_x * rand_x * d,
            p.y + mul_y * rand_y * d
        );
    }
    // Mode 1: Radial (polar coordinate space)
    else if (blur_type < 1.5) {
        let r_in = sqrt(p.x * p.x + p.y * p.y) + 1e-6;
        let phi = atan2(p.y, p.x) + mul_y * rng_nextf(rng) * d;
        let r = r_in + mul_x * rng_nextf(rng) * d;
        return vec2<f32>(r * cos(phi), r * sin(phi));
    }
    // Mode 2: Gaussian (spherical distribution)
    else {
        let phi = d * rng_nextf(rng) * PI;
        let r = d * rng_nextf(rng);
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

fn variation_pre_sinusoidal(p: vec2<f32>, weight: f32) -> vec2<f32> {
    // Apophysis Pre-Sinusoidal: Pre-phase sinusoidal wave
    // FTx := vvar * sin(FTx); FTy := vvar * sin(FTy);
    return vec2<f32>(weight * sin(p.x), weight * sin(p.y));
}

fn variation_pre_disc(p: vec2<f32>, weight: f32) -> vec2<f32> {
    // Apophysis Pre-Disc: Pre-phase disc transformation
    // r := vvar/π * atan2(x, y)
    // sincos(π * sqrt(x²+y²), sinr, cosr)
    // FTx := sinr * r; FTy := cosr * r
    const PI: f32 = 3.14159265359;
    let rad = sqrt(dot(p, p));
    let r = (weight / PI) * atan2(p.x, p.y);
    return vec2<f32>(sin(PI * rad) * r, cos(PI * rad) * r);
}

fn variation_rings2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Rings2: Ring pattern with adjustable spacing
    // dx = val² + EPS
    // r = vvar * (2 - dx * (floor((length/dx + 1)/2) * 2 / length + 1))
    // Since we normalize: r = 2 - dx * (floor((length/dx + 1)/2) * 2 / length + 1)
    let val = get_param(xform_id, variation_id, 0u);
    let dx = val * val + 1e-10;
    let length = sqrt(dot(p, p));
    let r = 2.0 - dx * (floor((length / dx + 1.0) / 2.0) * 2.0 / length + 1.0);
    return p * r;
}

fn variation_fan2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Fan2: Fan effect with offset
    const PI: f32 = 3.14159265359;
    let x_param = get_param(xform_id, variation_id, 0u);
    let y_param = get_param(xform_id, variation_id, 1u);
    
    let dx = PI * (x_param * x_param + 1e-10);
    let dx2 = dx / 2.0;
    let angle = atan2(p.x, p.y);
    
    var a: f32;
    if (fract((angle + y_param) / dx) > 0.5) {
        a = angle - dx2;
    } else {
        a = angle + dx2;
    }
    
    let r = sqrt(dot(p, p));
    return vec2<f32>(r * cos(a), r * sin(a));
}

fn variation_wedge(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Wedge: Wedge shape with controllable angle and swirl
    const PI: f32 = 3.14159265359;
    const C1_2PI: f32 = 0.15915494309189533576888376337251; // 1/(2π)
    
    let angle_deg = get_param(xform_id, variation_id, 0u);
    let hole = get_param(xform_id, variation_id, 1u);
    let count = get_param(xform_id, variation_id, 2u);
    let swirl = get_param(xform_id, variation_id, 3u);
    
    let angle_rad = angle_deg * PI / 180.0;
    let comp_fac = 1.0 - angle_rad * count * C1_2PI;
    
    let r = sqrt(dot(p, p));
    var a = atan2(p.y, p.x) + swirl * r;
    let c = floor((count * a + PI) * C1_2PI);
    a = a * comp_fac + c * angle_rad;
    
    let r_out = r + hole;
    return vec2<f32>(r_out * cos(a), r_out * sin(a));
}

fn variation_epispiral(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Epispiral: Epicycloid spiral pattern with random thickness
    let n = get_param(xform_id, variation_id, 0u);
    let thickness = get_param(xform_id, variation_id, 1u);
    let holes = get_param(xform_id, variation_id, 2u);
    
    let theta = atan2(p.y, p.x);
    let t = rng_nextf(rng) * thickness / cos(n * theta) - holes;
    
    if (abs(t) < 1e-6) {
        return vec2<f32>(0.0, 0.0);
    }
    
    return vec2<f32>(t * cos(theta), t * sin(theta));
}

fn variation_bwraps(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis BWraps: Bubble wraps with cell-based transformation
    let cellsize = get_param(xform_id, variation_id, 0u);
    let space = get_param(xform_id, variation_id, 1u);
    let gain = get_param(xform_id, variation_id, 2u);
    let inner_twist = get_param(xform_id, variation_id, 3u);
    let outer_twist = get_param(xform_id, variation_id, 4u);
    
    if (cellsize == 0.0) {
        return p;
    }
    
    let radius = 0.5 * (cellsize / (1.0 + space * space));
    let g2 = (gain * gain) / (radius + 1e-6) + 1e-6;
    var max_bubble = g2 * radius;
    
    if (max_bubble > 2.0) {
        max_bubble = 1.0;
    } else {
        max_bubble = max_bubble * (1.0 / ((max_bubble * max_bubble) / 4.0 + 1.0));
    }
    
    let r2 = radius * radius;
    let rfactor = radius / max_bubble;
    
    let cx = (floor(p.x / cellsize) + 0.5) * cellsize;
    let cy = (floor(p.y / cellsize) + 0.5) * cellsize;
    
    var lx = p.x - cx;
    var ly = p.y - cy;
    
    if ((lx * lx + ly * ly) > r2) {
        return p;
    }
    
    lx = lx * g2;
    ly = ly * g2;
    
    let r_dist = rfactor / ((lx * lx + ly * ly) / 4.0 + 1.0);
    lx = lx * r_dist;
    ly = ly * r_dist;
    
    let r_ratio = (lx * lx + ly * ly) / r2;
    let theta = inner_twist * (1.0 - r_ratio) + outer_twist * r_ratio;
    
    let vx = cx + cos(theta) * lx + sin(theta) * ly;
    let vy = cy - sin(theta) * lx + cos(theta) * ly;

    return vec2<f32>(vx, vy);
}

fn variation_pdj(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis PDJ (Peter de Jong attractor)
    // FPx := sin(a*y) - cos(b*x)
    // FPy := sin(c*x) - cos(d*y)
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);

    return vec2<f32>(
        sin(a * p.y) - cos(b * p.x),
        sin(c * p.x) - cos(d * p.y)
    );
}

fn variation_juliascope(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis JuliaScope variation
    // Has special cases for power = ±1, ±2 and general case
    const PI: f32 = 3.14159265359;

    let power = i32(get_param(xform_id, variation_id, 0u));
    let dist = get_param(xform_id, variation_id, 1u);

    let r2 = dot(p, p);

    // Random angle selection with alternating sign
    // In Apophysis: trunc(Abs(N)*random) * (π/N) * sign
    let rnd = rng_nextf(rng);
    let t = (atan2(p.y, p.x) + 2.0 * PI * f32(i32(abs(f32(power)) * rnd))) / f32(power);

    // Sign alternation: random even/odd determines sign
    let sign = select(-1.0, 1.0, (i32(abs(f32(power)) * rng_nextf(rng)) & 1) == 0);

    // Optimized special cases for common power values
    if (power == 1) {
        // r = vvar * r^dist
        let r_out = pow(r2, dist * 0.5);
        return vec2<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign);
    } else if (power == -1) {
        let r_out = pow(r2, dist * 0.5);
        return vec2<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign);
    } else if (power == 2) {
        let r_out = pow(r2, dist * 0.25);
        return vec2<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign);
    } else if (power == -2) {
        let r_out = pow(r2, dist * 0.25);
        return vec2<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign);
    } else {
        // General case: r = vvar * (r²)^(dist/(2*power))
        let cn = dist / f32(power) * 0.5;
        let r_out = pow(r2, cn);
        return vec2<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign);
    }
}

fn variation_curl(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Curl variation
    // Formula: f(z) = z / (c2*z² + c1*z + 1), where z = x + iy (complex)
    let c1 = get_param(xform_id, variation_id, 0u);
    let c2 = get_param(xform_id, variation_id, 1u);

    // Complex arithmetic: z² = (x² - y²) + 2xyi
    let re = 1.0 + c1 * p.x + c2 * (p.x * p.x - p.y * p.y);
    let im = c1 * p.y + 2.0 * c2 * p.x * p.y;

    // r = vvar / |denominator|² = 1 / (re² + im²)
    let r = 1.0 / (re * re + im * im);

    // Complex division: (x + iy) / (re + i*im) = ((x*re + y*im) + i(y*re - x*im)) / (re² + im²)
    return vec2<f32>(
        (p.x * re + p.y * im) * r,
        (p.y * re - p.x * im) * r
    );
}

fn variation_radial_blur(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Radial Blur - combines radial spin and zoom blur
    const PI: f32 = 3.14159265359;

    let angle_deg = get_param(xform_id, variation_id, 0u);
    let angle_rad = angle_deg * PI / 180.0;

    // Precomputed: spin_var = vvar * sin(angle * π/2), zoom_var = vvar * cos(angle * π/2)
    let spin_var = sin(angle_rad * 0.5);
    let zoom_var = cos(angle_rad * 0.5);

    // Gaussian blur approximation: sum of 4 random values - 2
    let rnd_g = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;

    let ra = sqrt(p.x * p.x + p.y * p.y);
    let angle_out = atan2(p.y, p.x) + spin_var * rnd_g;
    let rz = zoom_var * rnd_g - 1.0;

    return vec2<f32>(
        ra * cos(angle_out) + rz * p.x,
        ra * sin(angle_out) + rz * p.y
    );
}

fn variation_blur_circle(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Blur Circle - uniform blur in a circle using square-to-circle mapping
    const PI: f32 = 3.14159265359;
    const PI_4: f32 = 0.78539816339;  // π/4

    // Random point in square [-1, 1]
    let x = 2.0 * rng_nextf(rng) - 1.0;
    let y = 2.0 * rng_nextf(rng) - 1.0;

    let absx = abs(x);
    let absy = abs(y);

    var perimeter: f32;
    var side: f32;

    // Map square to circle using perimeter-based mapping
    if (absx >= absy) {
        if (x >= absy) {
            perimeter = absx + y;
        } else {
            perimeter = 5.0 * absx - y;
        }
        side = absx;
    } else {
        if (y >= absx) {
            perimeter = 3.0 * absy - x;
        } else {
            perimeter = 7.0 * absy + x;
        }
        side = absy;
    }

    let r = side;
    let angle = PI_4 * perimeter / side - PI_4;

    return vec2<f32>(r * cos(angle), r * sin(angle));
}

fn variation_blur_zoom(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Blur Zoom - zoom blur from a center point
    let length = get_param(xform_id, variation_id, 0u);
    let zoom_x = get_param(xform_id, variation_id, 1u);
    let zoom_y = get_param(xform_id, variation_id, 2u);

    let z = 1.0 + length * rng_nextf(rng);

    return vec2<f32>(
        (p.x - zoom_x) * z + zoom_x,
        (p.y - zoom_y) * z + zoom_y
    );
}

fn variation_blur_pixelize(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Blur Pixelize - pixelated/mosaic blur effect
    let size = get_param(xform_id, variation_id, 0u);
    let scale = get_param(xform_id, variation_id, 1u);

    let inv_size = 1.0 / size;

    // Quantize to pixel grid
    let x = floor(p.x * inv_size);
    let y = floor(p.y * inv_size);

    // Add random offset within pixel
    return vec2<f32>(
        size * (x + scale * (rng_nextf(rng) - 0.5) + 0.5),
        size * (y + scale * (rng_nextf(rng) - 0.5) + 0.5)
    );
}

fn variation_rectangles(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Rectangles - creates rectangular tiling pattern
    let rect_x = get_param(xform_id, variation_id, 0u);
    let rect_y = get_param(xform_id, variation_id, 1u);

    // Formula: (2*floor(p/rect) + 1)*rect - p
    // This creates a mirrored tiling effect
    return vec2<f32>(
        (2.0 * floor(p.x / rect_x) + 1.0) * rect_x - p.x,
        (2.0 * floor(p.y / rect_y) + 1.0) * rect_y - p.y
    );
}

fn variation_splits(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Splits - splits space at origin with offset
    let splits_x = get_param(xform_id, variation_id, 0u);
    let splits_y = get_param(xform_id, variation_id, 1u);

    // If x >= 0: add offset, else subtract offset
    // Same for y independently
    return vec2<f32>(
        select(p.x - splits_x, p.x + splits_x, p.x >= 0.0),
        select(p.y - splits_y, p.y + splits_y, p.y >= 0.0)
    );
}

fn variation_separation(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Separation - separates positive/negative quadrants with distance calculation
    let sep_x = get_param(xform_id, variation_id, 0u);
    let sep_y = get_param(xform_id, variation_id, 1u);
    let xinside = get_param(xform_id, variation_id, 2u);
    let yinside = get_param(xform_id, variation_id, 3u);

    // For positive x: sqrt(x² + sep_x²) - x*xinside
    // For negative x: -(sqrt(x² + sep_x²) + x*xinside)
    let x_out = select(
        -(sqrt(p.x * p.x + sep_x * sep_x) + p.x * xinside),
        sqrt(p.x * p.x + sep_x * sep_x) - p.x * xinside,
        p.x > 0.0
    );

    let y_out = select(
        -(sqrt(p.y * p.y + sep_y * sep_y) + p.y * yinside),
        sqrt(p.y * p.y + sep_y * sep_y) - p.y * yinside,
        p.y > 0.0
    );

    return vec2<f32>(x_out, y_out);
}

fn variation_ngon(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Ngon - creates N-sided polygonal shapes
    const PI: f32 = 3.14159265359;
    
    let sides = get_param(xform_id, variation_id, 0u);
    let power = get_param(xform_id, variation_id, 1u);
    let circle = get_param(xform_id, variation_id, 2u);
    let corners = get_param(xform_id, variation_id, 3u);

    let theta = atan2(p.y, p.x);
    let phi = theta - (PI * 2.0 / sides) * floor(sides * theta / (PI * 2.0));

    // Conditional: if phi > PI/sides then phi = phi - 2*PI/sides
    let phi_adj = select(phi, phi - 2.0 * PI / sides, phi > PI / sides);

    let amp = cos(phi_adj) * pow(1.0 / (cos(phi_adj * sides / 2.0) + 1e-10), circle);
    let r = pow(p.x * p.x + p.y * p.y, power * 0.5);

    return vec2<f32>(
        amp * r * cos(theta) + corners,
        amp * r * sin(theta)
    );
}

fn variation_mobius(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Mobius - Möbius transformation f(z) = (Az + B)/(Cz + D)
    let re_a = get_param(xform_id, variation_id, 0u);
    let im_a = get_param(xform_id, variation_id, 1u);
    let re_b = get_param(xform_id, variation_id, 2u);
    let im_b = get_param(xform_id, variation_id, 3u);
    let re_c = get_param(xform_id, variation_id, 4u);
    let im_c = get_param(xform_id, variation_id, 5u);
    let re_d = get_param(xform_id, variation_id, 6u);
    let im_d = get_param(xform_id, variation_id, 7u);

    // Numerator: Az + B
    // z = p.x + i*p.y, A = re_a + i*im_a
    // Az = (re_a*p.x - im_a*p.y) + i*(im_a*p.x + re_a*p.y)
    let re_u = re_a * p.x - im_a * p.y + re_b;
    let im_u = re_a * p.y + im_a * p.x + im_b;

    // Denominator: Cz + D
    let re_v = re_c * p.x - im_c * p.y + re_d;
    let im_v = re_c * p.y + im_c * p.x + im_d;

    // Division: (re_u + i*im_u) / (re_v + i*im_v)
    // = ((re_u*re_v + im_u*im_v) + i*(im_u*re_v - re_u*im_v)) / (re_v² + im_v²)
    let v_denom = re_v * re_v + im_v * im_v + 1e-10;

    return vec2<f32>(
        (re_u * re_v + im_u * im_v) / v_denom,
        (im_u * re_v - re_u * im_v) / v_denom
    );
}

fn variation_crop(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Crop - crops to a rectangular region with optional scatter
    let x0 = get_param(xform_id, variation_id, 0u);  // left
    let y0 = get_param(xform_id, variation_id, 1u);  // top
    let x1 = get_param(xform_id, variation_id, 2u);  // right
    let y1 = get_param(xform_id, variation_id, 3u);  // bottom
    let scatter = get_param(xform_id, variation_id, 4u);  // scatter_area
    let zero = get_param(xform_id, variation_id, 5u);  // zero flag

    // Sort bounds (ensure _x0 < _x1, _y0 < _y1)
    let _x0 = select(x1, x0, x0 < x1);
    let _x1 = select(x0, x1, x0 < x1);
    let _y0 = select(y1, y0, y0 < y1);
    let _y1 = select(y0, y1, y0 < y1);

    // Calculate scatter dimensions
    let w = (_x1 - _x0) * 0.5 * scatter;
    let h = (_y1 - _y0) * 0.5 * scatter;

    var x = p.x;
    var y = p.y;

    // If outside bounds and zero flag is set, return zero
    if ((x < _x0) || (x > _x1) || (y < _y0) || (y > _y1)) && (zero > 0.5) {
        return vec2<f32>(0.0, 0.0);
    }

    // Apply scatter if point is outside bounds
    if x < _x0 {
        x = _x0 + rng_nextf(rng) * w;
    } else if x > _x1 {
        x = _x1 - rng_nextf(rng) * w;
    }

    if y < _y0 {
        y = _y0 + rng_nextf(rng) * h;
    } else if y > _y1 {
        y = _y1 - rng_nextf(rng) * h;
    }

    return vec2<f32>(x, y);
}

fn variation_auger(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Auger - wave distortion with symmetry control
    let freq = get_param(xform_id, variation_id, 0u);
    let weight = get_param(xform_id, variation_id, 1u);
    let scale = get_param(xform_id, variation_id, 2u);
    let sym = get_param(xform_id, variation_id, 3u);

    let s = sin(freq * p.x);
    let t = sin(freq * p.y);

    let dx = p.x + weight * (0.5 * scale * t + abs(p.x) * t);
    let dy = p.y + weight * (0.5 * scale * s + abs(p.y) * s);

    return vec2<f32>(
        p.x + sym * (dx - p.x),
        dy
    );
}

fn variation_pre_bwraps(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Pre_Bwraps - bubble wrap effect applied before variations
    const PI: f32 = 3.14159265359;

    let cellsize = get_param(xform_id, variation_id, 0u);
    let space = get_param(xform_id, variation_id, 1u);
    let gain = get_param(xform_id, variation_id, 2u);
    let inner_twist = get_param(xform_id, variation_id, 3u) * PI / 180.0;  // Convert to radians
    let outer_twist = get_param(xform_id, variation_id, 4u) * PI / 180.0;

    if cellsize == 0.0 {
        return p;
    }

    // Calculate cell center
    let cx = (floor(p.x / cellsize) + 0.5) * cellsize;
    let cy = (floor(p.y / cellsize) + 0.5) * cellsize;

    // Local coordinates within cell
    var lx = p.x - cx;
    var ly = p.y - cy;

    // Prepare constants (same as Prepare() in Pascal)
    let radius = 0.5 * (cellsize / (1.0 + space * space));
    let g2 = gain * gain / (radius + 1e-6) + 1e-6;
    var max_bubble = g2 * radius;

    if max_bubble > 2.0 {
        max_bubble = 1.0;
    } else {
        max_bubble = max_bubble * (1.0 / (max_bubble * max_bubble / 4.0 + 1.0));
    }

    let r2 = radius * radius;
    let rfactor = radius / max_bubble;

    // Only process if inside bubble radius
    if (lx * lx + ly * ly) <= r2 {
        // Scale by gain
        lx = lx * g2;
        ly = ly * g2;

        // Calculate radial factor
        var r = rfactor / ((lx * lx + ly * ly) / 4.0 + 1.0);

        lx = lx * r;
        ly = ly * r;

        // Calculate twist angle based on radius
        r = (lx * lx + ly * ly) / r2;
        let theta = inner_twist * (1.0 - r) + outer_twist * r;

        // Rotate
        let s = sin(theta);
        let c = cos(theta);

        return vec2<f32>(
            cx + c * lx + s * ly,
            cy - s * lx + c * ly
        );
    }

    return p;
}

fn variation_post_bwraps(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    // Apophysis Post_Bwraps - bubble wrap effect applied after variations
    const PI: f32 = 3.14159265359;

    let cellsize = get_param(xform_id, variation_id, 0u);
    let space = get_param(xform_id, variation_id, 1u);
    let gain = get_param(xform_id, variation_id, 2u);
    let inner_twist = get_param(xform_id, variation_id, 3u) * PI / 180.0;
    let outer_twist = get_param(xform_id, variation_id, 4u) * PI / 180.0;

    if cellsize == 0.0 {
        return p;
    }

    let cx = (floor(p.x / cellsize) + 0.5) * cellsize;
    let cy = (floor(p.y / cellsize) + 0.5) * cellsize;

    var lx = p.x - cx;
    var ly = p.y - cy;

    let radius = 0.5 * (cellsize / (1.0 + space * space));
    let g2 = gain * gain / (radius + 1e-6) + 1e-6;
    var max_bubble = g2 * radius;

    if max_bubble > 2.0 {
        max_bubble = 1.0;
    } else {
        max_bubble = max_bubble * (1.0 / (max_bubble * max_bubble / 4.0 + 1.0));
    }

    let r2 = radius * radius;
    let rfactor = radius / max_bubble;

    if (lx * lx + ly * ly) <= r2 {
        lx = lx * g2;
        ly = ly * g2;

        var r = rfactor / ((lx * lx + ly * ly) / 4.0 + 1.0);

        lx = lx * r;
        ly = ly * r;

        r = (lx * lx + ly * ly) / r2;
        let theta = inner_twist * (1.0 - r) + outer_twist * r;

        let s = sin(theta);
        let c = cos(theta);

        return vec2<f32>(
            cx + c * lx + s * ly,
            cy - s * lx + c * ly
        );
    }

    return p;
}

fn variation_pre_crop(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Pre_Crop - same as crop but applied before variations
    let x0 = get_param(xform_id, variation_id, 0u);  // left
    let y0 = get_param(xform_id, variation_id, 1u);  // top
    let x1 = get_param(xform_id, variation_id, 2u);  // right
    let y1 = get_param(xform_id, variation_id, 3u);  // bottom
    let scatter = get_param(xform_id, variation_id, 4u);  // scatter_area
    let zero = get_param(xform_id, variation_id, 5u);  // zero flag

    let _x0 = select(x1, x0, x0 < x1);
    let _x1 = select(x0, x1, x0 < x1);
    let _y0 = select(y1, y0, y0 < y1);
    let _y1 = select(y0, y1, y0 < y1);

    let w = (_x1 - _x0) * 0.5 * scatter;
    let h = (_y1 - _y0) * 0.5 * scatter;

    var x = p.x;
    var y = p.y;

    if ((x < _x0) || (x > _x1) || (y < _y0) || (y > _y1)) && (zero > 0.5) {
        return vec2<f32>(0.0, 0.0);
    }

    if x < _x0 {
        x = _x0 + rng_nextf(rng) * w;
    } else if x > _x1 {
        x = _x1 - rng_nextf(rng) * w;
    }

    if y < _y0 {
        y = _y0 + rng_nextf(rng) * h;
    } else if y > _y1 {
        y = _y1 - rng_nextf(rng) * h;
    }

    return vec2<f32>(x, y);
}

fn variation_post_crop(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    // Apophysis Post_Crop - same as crop but applied after variations
    let x0 = get_param(xform_id, variation_id, 0u);  // left
    let y0 = get_param(xform_id, variation_id, 1u);  // top
    let x1 = get_param(xform_id, variation_id, 2u);  // right
    let y1 = get_param(xform_id, variation_id, 3u);  // bottom
    let scatter = get_param(xform_id, variation_id, 4u);  // scatter_area
    let zero = get_param(xform_id, variation_id, 5u);  // zero flag

    let _x0 = select(x1, x0, x0 < x1);
    let _x1 = select(x0, x1, x0 < x1);
    let _y0 = select(y1, y0, y0 < y1);
    let _y1 = select(y0, y1, y0 < y1);

    let w = (_x1 - _x0) * 0.5 * scatter;
    let h = (_y1 - _y0) * 0.5 * scatter;

    var x = p.x;
    var y = p.y;

    if ((x < _x0) || (x > _x1) || (y < _y0) || (y > _y1)) && (zero > 0.5) {
        return vec2<f32>(0.0, 0.0);
    }

    if x < _x0 {
        x = _x0 + rng_nextf(rng) * w;
    } else if x > _x1 {
        x = _x1 - rng_nextf(rng) * w;
    }

    if y < _y0 {
        y = _y0 + rng_nextf(rng) * h;
    } else if y > _y1 {
        y = _y1 - rng_nextf(rng) * h;
    }

    return vec2<f32>(x, y);
}
