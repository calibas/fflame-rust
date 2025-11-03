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
