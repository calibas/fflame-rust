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

fn variation_linear(p: vec3<f32>) -> vec3<f32> {
    return p;
}

fn variation_sinusoidal(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(sin(p.x), sin(p.y), p.z);
}

fn variation_spherical(p: vec3<f32>) -> vec3<f32> {
    let r2 = dot(p.xy, p.xy) + 1e-6;
    return vec3<f32>((p.xy / r2), p.z);
}

fn variation_swirl(p: vec3<f32>) -> vec3<f32> {
    let r2 = dot(p.xy, p.xy);
    let s = sin(r2);
    let c = cos(r2);
    return vec3<f32>(p.x * s - p.y * c, p.x * c + p.y * s, p.z);
}

fn variation_horseshoe(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy) + 1e-6;
    let r_inv = 1.0 / r;
    return vec3<f32>(
        (p.x - p.y) * (p.x + p.y) * r_inv,
        2.0 * p.x * p.y * r_inv,
        p.z
    );
}

fn variation_polar(p: vec3<f32>) -> vec3<f32> {
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    let r = length(p.xy);
    return vec3<f32>(theta / 3.14159265359, r - 1.0, p.z);
}

fn variation_handkerchief(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy);
    let theta = atan2(p.y, p.x);  // Standard atan2(y,x) for plugin variations
    return vec3<f32>(r * sin(theta + r), r * cos(theta - r), p.z);
}

fn variation_heart(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy);
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    let r_theta = r * theta;
    return vec3<f32>(r * sin(r_theta), -r * cos(r_theta), p.z);
}

fn variation_disc(p: vec3<f32>) -> vec3<f32> {
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    let r = length(p.xy);
    let theta_pi = theta / 3.14159265359;
    let pi_r = 3.14159265359 * r;
    return vec3<f32>(theta_pi * sin(pi_r), theta_pi * cos(pi_r), p.z);
}

fn variation_spiral(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy) + 1e-6;
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)
    let r_inv = 1.0 / r;
    return vec3<f32>(
        r_inv * (cos(theta) + sin(r)),
        r_inv * (sin(theta) - cos(r)),
        p.z
    );
}

fn variation_hyperbolic(p: vec3<f32>) -> vec3<f32> {
    let r2 = dot(p.xy, p.xy) + 1e-6;  // x² + y² + epsilon
    return vec3<f32>(p.x / r2, p.y, p.z);
}

fn variation_diamond(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy);
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y) not atan2(y,x)
    return vec3<f32>(sin(theta) * cos(r), cos(theta) * sin(r), p.z);
}

fn variation_ex(p: vec3<f32>) -> vec3<f32> {
    let r = length(p.xy);
    let theta = atan2(p.y, p.x);  // Standard atan2(y,x) for plugin variations
    let n0 = sin(theta + r);
    let n1 = cos(theta - r);
    let m0 = n0 * n0 * n0;  // n0³
    let m1 = n1 * n1 * n1;  // n1³
    return vec3<f32>(r * (m0 + m1), r * (m0 - m1), p.z);
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

fn variation_julian(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Get parameters
    let power = get_param(xform_id, variation_id, 0u);
    let dist = get_param(xform_id, variation_id, 1u);

    let abs_power = abs(power);
    let cpower = dist / abs_power / 2.0;

    // Apophysis: r := Math.Power(sqr(FTx) + sqr(FTy), cN) = pow(x² + y², cN)
    let r2 = dot(p.xy, p.xy);  // x² + y²
    let r = pow(r2, cpower);
    let theta = atan2(p.y, p.x);  // JuliaN uses standard atan2(y,x) convention

    // Random selection of symmetry
    let trunc_val = floor(abs_power * rng_nextf(rng));
    let t = (theta + 6.28318530718 * trunc_val) / power;

    return vec3<f32>(r * cos(t), r * sin(t), p.z);
}

fn variation_blob(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Get parameters: p1 = high, p2 = low, p3 = waves
    let p1 = get_param(xform_id, variation_id, 0u);  // high
    let p2 = get_param(xform_id, variation_id, 1u);  // low
    let p3 = get_param(xform_id, variation_id, 2u);  // waves

    let r = length(p.xy);
    let theta = atan2(p.x, p.y);  // Apophysis uses atan2(x,y)

    // r · (p2 + ((p1 − p2)/2)(sin(p3θ) + 1))
    let scale = r * (p2 + ((p1 - p2) / 2.0) * (sin(p3 * theta) + 1.0));

    return vec3<f32>(scale * cos(theta), scale * sin(theta), p.z);
}

// === 3D-Specific Utilities ===

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

// Hemisphere - project onto hemisphere
fn variation_hemisphere(p: vec3<f32>) -> vec3<f32> {
    // Apophysis: t = 1 / sqrt(x² + y² + 1)
    // result = (x*t, y*t, t)
    let r2_xy = dot(p.xy, p.xy);  // x² + y²
    let t = 1.0 / sqrt(r2_xy + 1.0);
    return vec3<f32>(p.x * t, p.y * t, t);
}

fn variation_eyefish(p: vec3<f32>) -> vec3<f32> {
    // Apophysis: r = 2 / (sqrt(x² + y²) + 1)
    // result = (r×x, r×y, z)
    let r_xy = length(p.xy) + 1.0;
    let scale = 2.0 / r_xy;
    return vec3<f32>(scale * p.x, scale * p.y, p.z);
}

fn variation_bubble(p: vec3<f32>) -> vec3<f32> {
    // Apophysis: r = (x² + y²)/4 + 1
    // z' = 2/r - 1, scale = 1/r
    let r2_xy = dot(p.xy, p.xy);
    let r = r2_xy / 4.0 + 1.0;
    let scale = 1.0 / r;
    let new_z = 2.0 / r - 1.0;
    return vec3<f32>(scale * p.x, scale * p.y, new_z);
}

fn variation_cylinder(p: vec3<f32>) -> vec3<f32> {
    // Apophysis: result = (sin(x), y, cos(x))
    return vec3<f32>(sin(p.x), p.y, cos(p.x));
}

fn variation_noise(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis: Random polar displacement
    // θ = random × 2π, r = random
    // result = (x × r × cos(θ), y × r × sin(θ), z)
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng);
    return vec3<f32>(p.x * r * cos(theta), p.y * r * sin(theta), p.z);
}

fn variation_blur(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis: Random circular blur
    // θ = random × 2π, r = random
    // result = (r × cos(θ), r × sin(θ), z)
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng);
    return vec3<f32>(r * cos(theta), r * sin(theta), p.z);
}

fn variation_gaussian_blur(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis: Gaussian distributed blur
    // θ = random × 2π
    // r = (rand₁ + rand₂ + rand₃ + rand₄ - 2) - Gaussian approximation via central limit theorem
    // result = (r × cos(θ), r × sin(θ), z)
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec3<f32>(r * cos(theta), r * sin(theta), p.z);
}

fn variation_zblur(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis: Gaussian blur on Z axis only
    // result = (x, y, (rand₁ + rand₂ + rand₃ + rand₄ - 2))
    let z_offset = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec3<f32>(p.x, p.y, z_offset);
}

fn variation_blur3d(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis: 3D Gaussian spherical blur
    // θ = random × 2π (azimuth)
    // φ = random × π (polar angle)
    // r = Gaussian (sum of 4 randoms - 2)
    // result = (r × sin(φ) × cos(θ), r × sin(φ) × sin(θ), r × cos(φ))
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let phi = rng_nextf(rng) * 3.14159265359;    // π
    let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec3<f32>(r * sin(phi) * cos(theta), r * sin(phi) * sin(theta), r * cos(phi));
}

fn variation_pre_blur(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis: Pre-phase Gaussian blur applied before variations
    // FTx += r * cos(θ), FTy += r * sin(θ), FTz unchanged
    let theta = rng_nextf(rng) * 6.28318530718;  // 2π
    let r = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;
    return vec3<f32>(p.x + r * cos(theta), p.y + r * sin(theta), p.z);
}

fn variation_pre_zscale(p: vec3<f32>, weight: f32) -> vec3<f32> {
    // Apophysis: Pre-phase Z scaling
    // FTz *= vars[variation_id] (weight is the scale factor)
    // In Pre-phase, we directly modify the point, so scale Z by weight
    return vec3<f32>(p.x, p.y, p.z * weight);
}

fn variation_pre_ztranslate(p: vec3<f32>, weight: f32) -> vec3<f32> {
    // Apophysis: Pre-phase Z translation
    // FTz += vars[variation_id] (weight is the translation amount)
    // In Pre-phase, we directly modify the point, so add weight to Z
    return vec3<f32>(p.x, p.y, p.z + weight);
}

fn variation_ztranslate(p: vec3<f32>) -> vec3<f32> {
    // Apophysis: Normal-phase Z translation
    // FPz += vars[variation_id] (added during weighted sum)
    // Return (0, 0, 1) so weighted sum adds weight to Z: result.z += weight * 1
    return vec3<f32>(0.0, 0.0, 1.0);
}

fn variation_waves2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Waves2: Sine wave distortion with 6 parameters (3D version)
    // FPx += VVAR * (FTx + scalex * sin(FTy * freqx))
    // FPy += VVAR * (FTy + scaley * sin(FTx * freqy))
    // FPz += VVAR * (FTz + scalez * sin(sqrt(FTx² + FTy²) * freqz))
    let freqx = get_param(xform_id, variation_id, 0u);
    let scalex = get_param(xform_id, variation_id, 1u);
    let freqy = get_param(xform_id, variation_id, 2u);
    let scaley = get_param(xform_id, variation_id, 3u);
    let freqz = get_param(xform_id, variation_id, 4u);
    let scalez = get_param(xform_id, variation_id, 5u);

    let r_xy = length(p.xy);

    let new_x = p.x + scalex * sin(p.y * freqx);
    let new_y = p.y + scaley * sin(p.x * freqy);
    let new_z = p.z + scalez * sin(r_xy * freqz);

    return vec3<f32>(new_x, new_y, new_z);
}

fn variation_julia3d(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Julia3D: Full 3D Julia set implementation by Joel Faber
    let power_f = get_param(xform_id, variation_id, 0u);
    let N = i32(power_f);

    // Handle special case: power = 0 becomes power = 1
    if (N == 0) {
        return p;  // Linear fallback
    }

    let absN = abs(N);
    let absN_f = f32(absN);

    // Special optimized cases
    if (N == 1) {
        // Power 1: Linear (identity)
        return p;
    } else if (N == -1) {
        // Power -1: Inversion
        let r2 = dot(p, p);
        return p / r2;
    } else if (N == 2) {
        // Power 2: Optimized sqrt version
        let z = p.z / 2.0;
        let r2d = dot(p.xy, p.xy);
        let r3d = sqrt(r2d + z * z);
        let r = 1.0 / sqrt(sqrt(r3d));  // vvar / sqrt(sqrt(r3d)), weight applied after

        let angle = atan2(p.y, p.x) / 2.0 + 3.14159265359 * f32(i32(rng_nextf(rng) * 2.0));
        let tmp = r * sqrt(r2d);

        return vec3<f32>(tmp * cos(angle), tmp * sin(angle), r * z);
    } else if (N == -2) {
        // Power -2: Optimized inverse sqrt version
        let z = p.z / 2.0;
        let r2d = dot(p.xy, p.xy);
        let r3d = sqrt(r2d + z * z);
        let r = 1.0 / (sqrt(r3d) * r3d);

        let angle = atan2(p.y, p.x) / 2.0 + 3.14159265359 * f32(i32(rng_nextf(rng) * 2.0));
        let tmp = r * sqrt(r2d);

        return vec3<f32>(tmp * cos(angle), -tmp * sin(angle), r * z);  // Note: negative Y for negative power
    } else {
        // General case: arbitrary power
        let z = p.z / absN_f;
        let r2d = dot(p.xy, p.xy);
        let cN = (1.0 / power_f - 1.0) / 2.0;
        let r = pow(r2d + z * z, cN);  // r^(n-0.5) / sqrt(r), weight applied after

        let random_idx = i32(rng_nextf(rng) * absN_f);
        let angle = (atan2(p.y, p.x) + 6.28318530718 * f32(random_idx)) / power_f;
        let tmp = r * sqrt(r2d);

        if (N > 0) {
            return vec3<f32>(tmp * cos(angle), tmp * sin(angle), r * z);
        } else {
            return vec3<f32>(tmp * cos(angle), -tmp * sin(angle), r * z);  // Negative Y for negative power
        }
    }
}

fn variation_log(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Log: Logarithmic transformation (3D)
    // FPx += vvar * ln(x² + y²) * (0.5 / ln(base))
    // FPy += vvar * atan2(y, x)
    // FPz += vvar * z
    let base = get_param(xform_id, variation_id, 0u);
    let denom = 0.5 / log(base);

    let r2 = dot(p.xy, p.xy);
    let new_x = log(r2) * denom;
    let new_y = atan2(p.y, p.x);

    return vec3<f32>(new_x, new_y, p.z);
}

fn variation_polar2(p: vec3<f32>) -> vec3<f32> {
    // Apophysis Polar2: Improved polar coordinates (3D)
    // FPx += vvar * atan2(x, y) / PI
    // FPy += vvar * 0.5 * ln(x² + y²) / PI
    // FPz += vvar * z
    const PI: f32 = 3.14159265359;
    let r2 = dot(p.xy, p.xy);
    let new_x = atan2(p.x, p.y) / PI;
    let new_y = 0.5 * log(r2) / PI;

    return vec3<f32>(new_x, new_y, p.z);
}

fn variation_cross(p: vec3<f32>) -> vec3<f32> {
    // Apophysis Cross: Cross/plus shape (3D)
    // r = abs((x - y) * (x + y) + 1e-6)
    // if r < 0: r *= -1
    // r = vvar / r
    // FPx += x * r
    // FPy += y * r
    // FPz += vvar * z
    var r = abs((p.x - p.y) * (p.x + p.y) + 1e-6);
    if (r < 0.0) {
        r = r * -1.0;
    }
    r = 1.0 / r;

    return vec3<f32>(p.x * r, p.y * r, p.z);
}

fn variation_loonie(p: vec3<f32>) -> vec3<f32> {
    // Apophysis Loonie: Lune/crescent shape (3D)
    // r2 = x² + y²
    // if r2 < vvar² and r2 != 0:
    //   r = vvar * sqrt(vvar² / r2 - 1)
    //   FPx += r * x
    //   FPy += r * y
    // else:
    //   FPx += vvar * x
    //   FPy += vvar * y
    // FPz += vvar * z
    let r2 = dot(p.xy, p.xy);

    // Since we normalize by weight, sqrvar becomes 1.0
    if (r2 < 1.0 && r2 != 0.0) {
        let r = sqrt(1.0 / r2 - 1.0);
        return vec3<f32>(p.x * r, p.y * r, p.z);
    } else {
        return p;
    }
}

fn variation_escher(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Escher: Escher-style tessellation (3D)
    // beta = parameter
    // c = 0.5 * (1.0 + cos(beta))
    // d = 0.5 * sin(beta)
    // a = atan2(y, x)
    // lnr = 0.5 * ln(x² + y²)
    // m = vvar * exp(c * lnr - d * a)
    // x' = m * cos(c * a + d * lnr)
    // y' = m * sin(c * a + d * lnr)
    // z' = vvar * z
    const PI: f32 = 3.14159265359;
    let beta = get_param(xform_id, variation_id, 0u) * PI / 180.0; // Convert degrees to radians
    let c = 0.5 * (1.0 + cos(beta));
    let d = 0.5 * sin(beta);

    let a = atan2(p.y, p.x);
    let lnr = 0.5 * log(dot(p.xy, p.xy));
    let m = exp(c * lnr - d * a);

    let angle = c * a + d * lnr;
    return vec3<f32>(m * cos(angle), m * sin(angle), p.z);
}

fn variation_scry(p: vec3<f32>) -> vec3<f32> {
    // Apophysis Scry: Crystal ball effect (3D)
    // t = x² + y²
    // r = 1 / (sqrt(t) * (t + 1/vvar))
    // Since we normalize by vvar: r = 1 / (sqrt(t) * (t + 1))
    // x' = x * r
    // y' = y * r
    // z' = vvar * z
    let t = dot(p.xy, p.xy);
    var r = 1.0 / (sqrt(t) * (t + 1.0));

    return vec3<f32>(p.x * r, p.y * r, p.z);
}

fn variation_foci(p: vec3<f32>) -> vec3<f32> {
    // Apophysis Foci: Focal point distortion (3D)
    // expx = exp(x) * 0.5
    // expnx = 0.25 / expx = 0.5 * exp(-x)
    // tmp = vvar / (expx + expnx - cos(y))
    // Since we normalize: tmp = 1 / (expx + expnx - cos(y))
    // x' = (expx - expnx) * tmp
    // y' = sin(y) * tmp
    // z' = vvar * z
    let expx = exp(p.x) * 0.5;
    let expnx = 0.5 * exp(-p.x);
    var tmp = expx + expnx - cos(p.y);

    if (tmp == 0.0) {
        tmp = 1e-6;
    }
    tmp = 1.0 / tmp;

    let new_x = (expx - expnx) * tmp;
    let new_y = sin(p.y) * tmp;

    return vec3<f32>(new_x, new_y, p.z);
}

fn variation_bipolar(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Bipolar: Bipolar coordinates (3D)
    // shift parameter controls vertical offset
    const PI: f32 = 3.14159265359;
    const HALF_PI: f32 = 1.57079632679489661923;
    let shift = get_param(xform_id, variation_id, 0u);

    let x2y2 = dot(p.xy, p.xy);
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
        return vec3<f32>(0.0, 0.0, p.z);
    }

    // v_4 = vvar * 1/(2π), v = vvar * 2/π
    // Since we normalize by vvar: v_4 = 1/(2π), v = 2/π
    let new_x = (1.0 / (2.0 * PI)) * log(f / g);
    let new_y = (2.0 / PI) * y;

    return vec3<f32>(new_x, new_y, p.z);
}

fn variation_elliptic(p: vec3<f32>) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    let v = 2.0 / PI;
    let tmp = dot(p.xy, p.xy) + 1.0;
    let x2 = 2.0 * p.x;
    let xmax = 0.5 * (sqrt(tmp + x2) + sqrt(tmp - x2));
    let a = p.x / xmax;
    let b = sqrt(max(0.0, 1.0 - a * a));
    let new_x = v * atan2(a, b);
    var new_y = v * log(xmax + sqrt(max(0.0, xmax - 1.0)));
    if (p.y < 0.0) { new_y = -new_y; }
    return vec3<f32>(new_x, new_y, p.z);
}

fn variation_lazysusan(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    const PI: f32 = 3.14159265359;
    let spin = get_param(xform_id, variation_id, 0u) * PI / 180.0;
    let space = get_param(xform_id, variation_id, 1u);
    let twist = get_param(xform_id, variation_id, 2u);
    let x_offset = get_param(xform_id, variation_id, 3u);
    let y_offset = get_param(xform_id, variation_id, 4u);
    let x = p.x - x_offset;
    let y = p.y + y_offset;
    let r = sqrt(x * x + y * y);
    if (r < 1.0) {
        let a = atan2(y, x) + spin + twist * (1.0 - r);
        return vec3<f32>(r * cos(a) + x_offset, r * sin(a) - y_offset, p.z);
    } else {
        let r_scale = 1.0 + space / (r + 1e-6);
        return vec3<f32>(r_scale * x + x_offset, r_scale * y - y_offset, p.z);
    }
}

fn variation_falloff2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Falloff2: Distance-based blur effect with 3 modes (3D)
    const PI: f32 = 3.14159265359;
    let scatter = get_param(xform_id, variation_id, 0u);
    let mindist = get_param(xform_id, variation_id, 1u);
    let mul_x = get_param(xform_id, variation_id, 2u);
    let mul_y = get_param(xform_id, variation_id, 3u);
    let mul_z = get_param(xform_id, variation_id, 4u);
    let x0 = get_param(xform_id, variation_id, 6u);
    let y0 = get_param(xform_id, variation_id, 7u);
    let z0 = get_param(xform_id, variation_id, 8u);
    let invert = get_param(xform_id, variation_id, 9u);
    let blur_type = get_param(xform_id, variation_id, 10u);

    let rmax = 0.04 * scatter;
    var d = sqrt((p.x - x0) * (p.x - x0) + (p.y - y0) * (p.y - y0) + (p.z - z0) * (p.z - z0));

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
        let rand_z = rng_nextf(rng);
        return vec3<f32>(
            p.x + mul_x * rand_x * d,
            p.y + mul_y * rand_y * d,
            p.z + mul_z * rand_z * d
        );
    }
    // Mode 1: Radial (spherical coordinate space)
    else if (blur_type < 1.5) {
        let r_in = sqrt(p.x * p.x + p.y * p.y + p.z * p.z) + 1e-6;
        let sigma = asin(p.z / r_in) + mul_z * rng_nextf(rng) * d;
        let phi = atan2(p.y, p.x) + mul_y * rng_nextf(rng) * d;
        let r = r_in + mul_x * rng_nextf(rng) * d;
        return vec3<f32>(
            r * cos(sigma) * cos(phi),
            r * cos(sigma) * sin(phi),
            r * sin(sigma)
        );
    }
    // Mode 2: Gaussian (spherical distribution)
    else {
        let sigma = d * rng_nextf(rng) * 2.0 * PI;
        let phi = d * rng_nextf(rng) * PI;
        let r = d * rng_nextf(rng);
        return vec3<f32>(
            p.x + mul_x * r * cos(sigma) * cos(phi),
            p.y + mul_y * r * cos(sigma) * sin(phi),
            p.z + mul_z * r * sin(sigma)
        );
    }
}

fn variation_pre_spherical(p: vec3<f32>) -> vec3<f32> {
    // Apophysis Pre-Spherical: Pre-phase spherical distortion (3D)
    let r = 1.0 / (dot(p.xy, p.xy) + 1e-5);
    return vec3<f32>(p.x * r, p.y * r, p.z);
}

fn variation_pre_sinusoidal(p: vec3<f32>, weight: f32) -> vec3<f32> {
    // Apophysis Pre-Sinusoidal: Pre-phase sinusoidal wave (3D)
    // FTx := vvar * sin(FTx); FTy := vvar * sin(FTy); FTz := vvar * FTz;
    return vec3<f32>(weight * sin(p.x), weight * sin(p.y), weight * p.z);
}

fn variation_pre_disc(p: vec3<f32>, weight: f32) -> vec3<f32> {
    // Apophysis Pre-Disc: Pre-phase disc transformation (3D)
    // r := vvar/π * atan2(x, y)
    // sincos(π * sqrt(x²+y²), sinr, cosr)
    // FTx := sinr * r; FTy := cosr * r; FTz := vvar * FTz;
    const PI: f32 = 3.14159265359;
    let rad = sqrt(dot(p.xy, p.xy));
    let r = (weight / PI) * atan2(p.x, p.y);
    return vec3<f32>(sin(PI * rad) * r, cos(PI * rad) * r, weight * p.z);
}

fn variation_rings2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Rings2: Ring pattern with adjustable spacing (3D)
    let val = get_param(xform_id, variation_id, 0u);
    let dx = val * val + 1e-10;
    let length = sqrt(dot(p.xy, p.xy));
    let r = 2.0 - dx * (floor((length / dx + 1.0) / 2.0) * 2.0 / length + 1.0);
    return vec3<f32>(p.x * r, p.y * r, p.z);
}

fn variation_fan2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Fan2: Fan effect with offset (3D)
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
    
    let r = sqrt(dot(p.xy, p.xy));
    return vec3<f32>(r * cos(a), r * sin(a), p.z);
}

fn variation_wedge(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Wedge: Wedge shape with controllable angle and swirl (3D)
    const PI: f32 = 3.14159265359;
    const C1_2PI: f32 = 0.15915494309189533576888376337251; // 1/(2π)
    
    let angle_deg = get_param(xform_id, variation_id, 0u);
    let hole = get_param(xform_id, variation_id, 1u);
    let count = get_param(xform_id, variation_id, 2u);
    let swirl = get_param(xform_id, variation_id, 3u);
    
    let angle_rad = angle_deg * PI / 180.0;
    let comp_fac = 1.0 - angle_rad * count * C1_2PI;
    
    let r = sqrt(dot(p.xy, p.xy));
    var a = atan2(p.y, p.x) + swirl * r;
    let c = floor((count * a + PI) * C1_2PI);
    a = a * comp_fac + c * angle_rad;
    
    let r_out = r + hole;
    return vec3<f32>(r_out * cos(a), r_out * sin(a), p.z);
}

fn variation_epispiral(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Epispiral: Epicycloid spiral pattern (3D)
    let n = get_param(xform_id, variation_id, 0u);
    let thickness = get_param(xform_id, variation_id, 1u);
    let holes = get_param(xform_id, variation_id, 2u);
    
    let theta = atan2(p.y, p.x);
    let t = rng_nextf(rng) * thickness / cos(n * theta) - holes;
    
    if (abs(t) < 1e-6) {
        return vec3<f32>(0.0, 0.0, p.z);
    }
    
    return vec3<f32>(t * cos(theta), t * sin(theta), p.z);
}

fn variation_bwraps(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis BWraps: Bubble wraps (3D)
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

    return vec3<f32>(vx, vy, p.z);
}

fn variation_pdj(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis PDJ (Peter de Jong attractor) - 3D
    // FPx := sin(a*y) - cos(b*x)
    // FPy := sin(c*x) - cos(d*y)
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);

    return vec3<f32>(
        sin(a * p.y) - cos(b * p.x),
        sin(c * p.x) - cos(d * p.y),
        p.z
    );
}

fn variation_juliascope(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis JuliaScope variation - 3D
    // Has special cases for power = ±1, ±2 and general case
    const PI: f32 = 3.14159265359;

    let power = i32(get_param(xform_id, variation_id, 0u));
    let dist = get_param(xform_id, variation_id, 1u);

    let r2 = p.x * p.x + p.y * p.y;

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
        return vec3<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign, p.z);
    } else if (power == -1) {
        let r_out = pow(r2, dist * 0.5);
        return vec3<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign, p.z);
    } else if (power == 2) {
        let r_out = pow(r2, dist * 0.25);
        return vec3<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign, p.z);
    } else if (power == -2) {
        let r_out = pow(r2, dist * 0.25);
        return vec3<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign, p.z);
    } else {
        // General case: r = vvar * (r²)^(dist/(2*power))
        let cn = dist / f32(power) * 0.5;
        let r_out = pow(r2, cn);
        return vec3<f32>(r_out * cos(t) * sign, r_out * sin(t) * sign, p.z);
    }
}

fn variation_julia3dz(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Julia3Dz - Full 3D Julia set with Z modification
    // Special cases for power = 1, -1, 2, -2, general case otherwise
    const PI: f32 = 3.14159265359;

    let power = i32(get_param(xform_id, variation_id, 0u));
    let abs_power = abs(power);
    let power_f = f32(power);

    let r2d = p.x * p.x + p.y * p.y;

    // Special case: power = 1 (linear passthrough)
    if (power == 1) {
        return p;  // FPx = FTx, FPy = FTy, FPz = FTz
    }

    // Special case: power = -1 (inverse)
    if (power == -1) {
        let r = 1.0 / r2d;
        return vec3<f32>(r * p.x, -r * p.y, r * p.z);
    }

    // Random angle selection: (atan2(y,x) + 2π*random(absN)) / N
    let rnd_int = i32(rng_nextf(rng) * f32(abs_power));
    let angle = (atan2(p.y, p.x) + 2.0 * PI * f32(rnd_int)) / power_f;

    // Special case: power = 2
    if (power == 2) {
        let r2d_sqrt = sqrt(r2d);
        let r = sqrt(r2d_sqrt);
        let z_out = r * p.z / r2d_sqrt / 2.0;
        return vec3<f32>(r * cos(angle), r * sin(angle), z_out);
    }

    // Special case: power = -2
    if (power == -2) {
        let r2d_sqrt = sqrt(r2d);
        let r = 1.0 / sqrt(r2d_sqrt);
        let z_out = r * p.z / r2d_sqrt / 2.0;
        return vec3<f32>(r * cos(angle), -r * sin(angle), z_out);
    }

    // General case: r = vvar * (r²)^(cN) where cN = 1/(2*N)
    let cN = 1.0 / power_f / 2.0;
    let r = pow(r2d, cN);
    let r2d_sqrt = sqrt(r2d);
    let z_out = r * p.z / (r2d_sqrt * f32(abs_power));

    return vec3<f32>(r * cos(angle), r * sin(angle), z_out);
}

fn variation_curl(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Curl variation - 3D (Z passes through scaled)
    // Formula: f(z) = z / (c2*z² + c1*z + 1), where z = x + iy (complex)
    let c1 = get_param(xform_id, variation_id, 0u);
    let c2 = get_param(xform_id, variation_id, 1u);

    // Complex arithmetic: z² = (x² - y²) + 2xyi
    let re = 1.0 + c1 * p.x + c2 * (p.x * p.x - p.y * p.y);
    let im = c1 * p.y + 2.0 * c2 * p.x * p.y;

    // r = vvar / |denominator|² = 1 / (re² + im²)
    let r = 1.0 / (re * re + im * im);

    // Complex division: (x + iy) / (re + i*im) = ((x*re + y*im) + i(y*re - x*im)) / (re² + im²)
    // In Apophysis: FPz = FPz + vvar * FTz (Z passes through with weight)
    return vec3<f32>(
        (p.x * re + p.y * im) * r,
        (p.y * re - p.x * im) * r,
        p.z  // Z passes through
    );
}

fn variation_curl3d(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Curl3D - Full 3D curl transformation
    // Formula: r = vvar / (r²*c² + 2cx*x - 2cy*y + 2cz*z + 1)
    // Result: ((x + cx*r²), (y - cy*r²), (z + cz*r²)) * r
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);
    let cz = get_param(xform_id, variation_id, 2u);

    let r2 = p.x * p.x + p.y * p.y + p.z * p.z;

    // c² = cx² + cy² + cz²
    let c2 = cx * cx + cy * cy + cz * cz;

    // Denominator: r²*c² + 2cx*x - 2cy*y + 2cz*z + 1
    let denom = r2 * c2 + 2.0 * cx * p.x - 2.0 * cy * p.y + 2.0 * cz * p.z + 1.0;
    let r = 1.0 / denom;

    return vec3<f32>(
        r * (p.x + cx * r2),
        r * (p.y - cy * r2),
        r * (p.z + cz * r2)
    );
}

fn variation_radial_blur(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Radial Blur - 3D (Z passes through)
    const PI: f32 = 3.14159265359;

    let angle_deg = get_param(xform_id, variation_id, 0u);
    let angle_rad = angle_deg * PI / 180.0;

    let spin_var = sin(angle_rad * 0.5);
    let zoom_var = cos(angle_rad * 0.5);

    // Gaussian blur approximation: sum of 4 random values - 2
    let rnd_g = rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0;

    let ra = sqrt(p.x * p.x + p.y * p.y);
    let angle_out = atan2(p.y, p.x) + spin_var * rnd_g;
    let rz = zoom_var * rnd_g - 1.0;

    return vec3<f32>(
        ra * cos(angle_out) + rz * p.x,
        ra * sin(angle_out) + rz * p.y,
        p.z  // Z passes through
    );
}

fn variation_blur_circle(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Blur Circle - 3D (Z passes through)
    const PI: f32 = 3.14159265359;
    const PI_4: f32 = 0.78539816339;

    let x = 2.0 * rng_nextf(rng) - 1.0;
    let y = 2.0 * rng_nextf(rng) - 1.0;

    let absx = abs(x);
    let absy = abs(y);

    var perimeter: f32;
    var side: f32;

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

    return vec3<f32>(r * cos(angle), r * sin(angle), p.z);
}

fn variation_blur_zoom(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Blur Zoom - 3D (Z passes through)
    let length = get_param(xform_id, variation_id, 0u);
    let zoom_x = get_param(xform_id, variation_id, 1u);
    let zoom_y = get_param(xform_id, variation_id, 2u);

    let z = 1.0 + length * rng_nextf(rng);

    return vec3<f32>(
        (p.x - zoom_x) * z + zoom_x,
        (p.y - zoom_y) * z + zoom_y,
        p.z  // Z passes through
    );
}

fn variation_blur_pixelize(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Blur Pixelize - 3D (Z passes through)
    let size = get_param(xform_id, variation_id, 0u);
    let scale = get_param(xform_id, variation_id, 1u);

    let inv_size = 1.0 / size;

    let x = floor(p.x * inv_size);
    let y = floor(p.y * inv_size);

    return vec3<f32>(
        size * (x + scale * (rng_nextf(rng) - 0.5) + 0.5),
        size * (y + scale * (rng_nextf(rng) - 0.5) + 0.5),
        p.z
    );
}

fn variation_rectangles(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Rectangles - 3D (Z passes through)
    let rect_x = get_param(xform_id, variation_id, 0u);
    let rect_y = get_param(xform_id, variation_id, 1u);

    return vec3<f32>(
        (2.0 * floor(p.x / rect_x) + 1.0) * rect_x - p.x,
        (2.0 * floor(p.y / rect_y) + 1.0) * rect_y - p.y,
        p.z
    );
}

fn variation_splits(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Splits - 3D (Z passes through)
    let splits_x = get_param(xform_id, variation_id, 0u);
    let splits_y = get_param(xform_id, variation_id, 1u);

    return vec3<f32>(
        select(p.x - splits_x, p.x + splits_x, p.x >= 0.0),
        select(p.y - splits_y, p.y + splits_y, p.y >= 0.0),
        p.z
    );
}

fn variation_separation(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Separation - 3D (Z passes through)
    let sep_x = get_param(xform_id, variation_id, 0u);
    let sep_y = get_param(xform_id, variation_id, 1u);
    let xinside = get_param(xform_id, variation_id, 2u);
    let yinside = get_param(xform_id, variation_id, 3u);

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

    return vec3<f32>(x_out, y_out, p.z);
}

fn variation_ngon(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Ngon - 3D (Z passes through)
    const PI: f32 = 3.14159265359;
    
    let sides = get_param(xform_id, variation_id, 0u);
    let power = get_param(xform_id, variation_id, 1u);
    let circle = get_param(xform_id, variation_id, 2u);
    let corners = get_param(xform_id, variation_id, 3u);

    let theta = atan2(p.y, p.x);
    let phi = theta - (PI * 2.0 / sides) * floor(sides * theta / (PI * 2.0));

    let phi_adj = select(phi, phi - 2.0 * PI / sides, phi > PI / sides);

    let amp = cos(phi_adj) * pow(1.0 / (cos(phi_adj * sides / 2.0) + 1e-10), circle);
    let r = pow(p.x * p.x + p.y * p.y, power * 0.5);

    return vec3<f32>(
        amp * r * cos(theta) + corners,
        amp * r * sin(theta),
        p.z
    );
}

fn variation_mobius(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Mobius - 3D (Z passes through)
    let re_a = get_param(xform_id, variation_id, 0u);
    let im_a = get_param(xform_id, variation_id, 1u);
    let re_b = get_param(xform_id, variation_id, 2u);
    let im_b = get_param(xform_id, variation_id, 3u);
    let re_c = get_param(xform_id, variation_id, 4u);
    let im_c = get_param(xform_id, variation_id, 5u);
    let re_d = get_param(xform_id, variation_id, 6u);
    let im_d = get_param(xform_id, variation_id, 7u);

    // Numerator: Az + B
    let re_u = re_a * p.x - im_a * p.y + re_b;
    let im_u = re_a * p.y + im_a * p.x + im_b;

    // Denominator: Cz + D
    let re_v = re_c * p.x - im_c * p.y + re_d;
    let im_v = re_c * p.y + im_c * p.x + im_d;

    let v_denom = re_v * re_v + im_v * im_v + 1e-10;

    return vec3<f32>(
        (re_u * re_v + im_u * im_v) / v_denom,
        (im_u * re_v - re_u * im_v) / v_denom,
        p.z
    );
}

fn variation_crop(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Crop - 3D (Z passes through)
    let x0 = get_param(xform_id, variation_id, 0u);  // left
    let y0 = get_param(xform_id, variation_id, 1u);  // top
    let x1 = get_param(xform_id, variation_id, 2u);  // right
    let y1 = get_param(xform_id, variation_id, 3u);  // bottom
    let scatter = get_param(xform_id, variation_id, 4u);  // scatter_area
    let zero = get_param(xform_id, variation_id, 5u);  // zero flag

    // Sort bounds
    let _x0 = select(x1, x0, x0 < x1);
    let _x1 = select(x0, x1, x0 < x1);
    let _y0 = select(y1, y0, y0 < y1);
    let _y1 = select(y0, y1, y0 < y1);

    let w = (_x1 - _x0) * 0.5 * scatter;
    let h = (_y1 - _y0) * 0.5 * scatter;

    var x = p.x;
    var y = p.y;

    if ((x < _x0) || (x > _x1) || (y < _y0) || (y > _y1)) && (zero > 0.5) {
        return vec3<f32>(0.0, 0.0, p.z);
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

    return vec3<f32>(x, y, p.z);
}

fn variation_auger(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Auger - 3D (Z passes through)
    let freq = get_param(xform_id, variation_id, 0u);
    let weight = get_param(xform_id, variation_id, 1u);
    let scale = get_param(xform_id, variation_id, 2u);
    let sym = get_param(xform_id, variation_id, 3u);

    let s = sin(freq * p.x);
    let t = sin(freq * p.y);

    let dx = p.x + weight * (0.5 * scale * t + abs(p.x) * t);
    let dy = p.y + weight * (0.5 * scale * s + abs(p.y) * s);

    return vec3<f32>(
        p.x + sym * (dx - p.x),
        dy,
        p.z
    );
}

fn variation_pre_bwraps(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Pre_Bwraps - 3D (Z passes through)
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

        return vec3<f32>(
            cx + c * lx + s * ly,
            cy - s * lx + c * ly,
            p.z
        );
    }

    return p;
}

fn variation_post_bwraps(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Post_Bwraps - 3D (Z passes through)
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

        return vec3<f32>(
            cx + c * lx + s * ly,
            cy - s * lx + c * ly,
            p.z
        );
    }

    return p;
}

fn variation_pre_crop(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Pre_Crop - 3D (Z passes through)
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
        return vec3<f32>(0.0, 0.0, p.z);
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

    return vec3<f32>(x, y, p.z);
}

fn variation_post_crop(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Post_Crop - 3D (Z passes through)
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
        return vec3<f32>(0.0, 0.0, p.z);
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

    return vec3<f32>(x, y, p.z);
}

fn variation_pre_falloff2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Pre_Falloff2 - Distance-based scatter with multiple blur modes (3D)
    const PI: f32 = 3.14159265359;

    let scatter = get_param(xform_id, variation_id, 0u);
    let mindist = get_param(xform_id, variation_id, 1u);
    let mul_x = get_param(xform_id, variation_id, 2u);
    let mul_y = get_param(xform_id, variation_id, 3u);
    let mul_z = get_param(xform_id, variation_id, 4u);
    // mul_c (param 5) affects color channel - not used in coordinate calculation
    let x0 = get_param(xform_id, variation_id, 6u);
    let y0 = get_param(xform_id, variation_id, 7u);
    let z0 = get_param(xform_id, variation_id, 8u);
    let invert = get_param(xform_id, variation_id, 9u);
    let blurtype = get_param(xform_id, variation_id, 10u);

    // Calculate 3D distance from center
    let dx = p.x - x0;
    let dy = p.y - y0;
    let dz = p.z - z0;
    let dist = sqrt(dx * dx + dy * dy + dz * dz);

    // Calculate falloff based on distance
    var factor: f32;
    if invert > 0.5 {
        factor = select(1.0, dist / mindist, dist < mindist);
    } else {
        factor = select(1.0, mindist / dist, dist > mindist);
    }

    // Apply scatter based on blur type
    var sx: f32;
    var sy: f32;
    var sz: f32;

    let blurtype_int = i32(blurtype + 0.5);

    if blurtype_int == 0 {
        // Linear blur (3D spherical)
        let r = scatter * factor;
        let theta = rng_nextf(rng) * 2.0 * PI;
        let phi = acos(2.0 * rng_nextf(rng) - 1.0);
        let d = rng_nextf(rng) * r;
        sx = d * sin(phi) * cos(theta);
        sy = d * sin(phi) * sin(theta);
        sz = d * cos(phi);
    } else if blurtype_int == 1 {
        // Radial blur (gaussian-like, 3D)
        let r = scatter * factor;
        let theta = rng_nextf(rng) * 2.0 * PI;
        let phi = acos(2.0 * rng_nextf(rng) - 1.0);
        let d = (rng_nextf(rng) + rng_nextf(rng)) * 0.5 * r;
        sx = d * sin(phi) * cos(theta);
        sy = d * sin(phi) * sin(theta);
        sz = d * cos(phi);
    } else {
        // Gaussian blur (blurtype == 2, 3D)
        let r = scatter * factor;
        let theta = rng_nextf(rng) * 2.0 * PI;
        let phi = acos(2.0 * rng_nextf(rng) - 1.0);
        let d = sqrt(-log(rng_nextf(rng) + 1e-10)) * r;
        sx = d * sin(phi) * cos(theta);
        sy = d * sin(phi) * sin(theta);
        sz = d * cos(phi);
    }

    return vec3<f32>(p.x + sx * mul_x, p.y + sy * mul_y, p.z + sz * mul_z);
}

fn variation_post_falloff2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    // Apophysis Post_Falloff2 - Same as pre_falloff2 but applied after variations (3D)
    const PI: f32 = 3.14159265359;

    let scatter = get_param(xform_id, variation_id, 0u);
    let mindist = get_param(xform_id, variation_id, 1u);
    let mul_x = get_param(xform_id, variation_id, 2u);
    let mul_y = get_param(xform_id, variation_id, 3u);
    let mul_z = get_param(xform_id, variation_id, 4u);
    // mul_c (param 5) affects color channel - not used in coordinate calculation
    let x0 = get_param(xform_id, variation_id, 6u);
    let y0 = get_param(xform_id, variation_id, 7u);
    let z0 = get_param(xform_id, variation_id, 8u);
    let invert = get_param(xform_id, variation_id, 9u);
    let blurtype = get_param(xform_id, variation_id, 10u);

    // Calculate 3D distance from center
    let dx = p.x - x0;
    let dy = p.y - y0;
    let dz = p.z - z0;
    let dist = sqrt(dx * dx + dy * dy + dz * dz);

    // Calculate falloff based on distance
    var factor: f32;
    if invert > 0.5 {
        factor = select(1.0, dist / mindist, dist < mindist);
    } else {
        factor = select(1.0, mindist / dist, dist > mindist);
    }

    // Apply scatter based on blur type
    var sx: f32;
    var sy: f32;
    var sz: f32;

    let blurtype_int = i32(blurtype + 0.5);

    if blurtype_int == 0 {
        // Linear blur (3D spherical)
        let r = scatter * factor;
        let theta = rng_nextf(rng) * 2.0 * PI;
        let phi = acos(2.0 * rng_nextf(rng) - 1.0);
        let d = rng_nextf(rng) * r;
        sx = d * sin(phi) * cos(theta);
        sy = d * sin(phi) * sin(theta);
        sz = d * cos(phi);
    } else if blurtype_int == 1 {
        // Radial blur (gaussian-like, 3D)
        let r = scatter * factor;
        let theta = rng_nextf(rng) * 2.0 * PI;
        let phi = acos(2.0 * rng_nextf(rng) - 1.0);
        let d = (rng_nextf(rng) + rng_nextf(rng)) * 0.5 * r;
        sx = d * sin(phi) * cos(theta);
        sy = d * sin(phi) * sin(theta);
        sz = d * cos(phi);
    } else {
        // Gaussian blur (blurtype == 2, 3D)
        let r = scatter * factor;
        let theta = rng_nextf(rng) * 2.0 * PI;
        let phi = acos(2.0 * rng_nextf(rng) - 1.0);
        let d = sqrt(-log(rng_nextf(rng) + 1e-10)) * r;
        sx = d * sin(phi) * cos(theta);
        sy = d * sin(phi) * sin(theta);
        sz = d * cos(phi);
    }

    return vec3<f32>(p.x + sx * mul_x, p.y + sy * mul_y, p.z + sz * mul_z);
}

fn variation_post_curl(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Post_Curl - Same as curl but applied after variations (3D version - Z passes through)
    let c1 = get_param(xform_id, variation_id, 0u);
    let c2 = get_param(xform_id, variation_id, 1u);

    // Complex arithmetic on XY plane: denominator = 1 + c1*z + c2*z²
    let re = 1.0 + c1 * p.x + c2 * (p.x * p.x - p.y * p.y);
    let im = c1 * p.y + 2.0 * c2 * p.x * p.y;

    // r = 1 / |denominator|²
    let r = 1.0 / (re * re + im * im);

    // Complex division: z / denominator (Z passes through)
    return vec3<f32>(
        (p.x * re + p.y * im) * r,
        (p.y * re - p.x * im) * r,
        p.z
    );
}

fn variation_post_curl3d(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    // Apophysis Post_Curl3D - Full 3D curl transformation applied after variations
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);
    let cz = get_param(xform_id, variation_id, 2u);

    // Clamp input to prevent FP overflow (as in Apophysis)
    // Using 1e30 instead of 1e100 to stay within f32 range
    let x = clamp(p.x, -1e30, 1e30);
    let y = clamp(p.y, -1e30, 1e30);
    let z = clamp(p.z, -1e30, 1e30);

    let r2 = x * x + y * y + z * z;

    // c² = cx² + cy² + cz²
    let c2 = cx * cx + cy * cy + cz * cz;

    // Denominator: r²*c² + 2cx*x - 2cy*y + 2cz*z + 1
    let denom = r2 * c2 + 2.0 * cx * x - 2.0 * cy * y + 2.0 * cz * z + 1.0;
    let r = 1.0 / denom;

    return vec3<f32>(
        r * (x + cx * r2),
        r * (y - cy * r2),
        r * (z + cz * r2)
    );
}
