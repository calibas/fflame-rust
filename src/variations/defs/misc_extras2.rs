//! Miscellaneous misc — second extras file
//!
//! Six more standalone variations (skipping `cubicLattice_3D` which
//! reads both pre-affine `FTx` and post-variations `FPx` mid-iteration
//! — the same architectural blocker as `farblur` and `post_depth`).
//!
//!   - `collideoscope` (Faber)              — radial branch-and-mod
//!   - `bent2`         (Apophysis pack)      — per-quadrant scale
//!   - `mcarpet`       (FracFx)              — bubble + twist + tilt
//!   - `lineart3d`     (FractalDesire)        — 3D sign-preserving power
//!   - `oscilloscope`  (Apophysis pack)      — Y-flip inside damped cosine band
//!   - `fibonacci2`    (Larry Berlin)         — Binet-formula complex Fibonacci
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.
//!
//! All factor VVAR cleanly through the outer multiplier.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// collideoscope: radial branch-and-mod (Michael Faber)
//   r = sqrt(x²+y²) (with weight applied via outer)
//   a = atan2(y, x)
//   alt = floor(|a| · num/π)
//   Mirror-and-shift `a` based on sign of original a and parity of alt
//   out = r · (cos a, sin a)
// =============================================================================
/// Radial branch-and-mod (kaleidoscope-style) — converts the input to polar
/// `(r, a)`, splits the angle into `num` equal sectors, and mirrors/shifts
/// alternating sectors based on the parity of the sector index, with a per-
/// sector angular offset `a`. Produces a kaleidoscope-like radial tiling.
///
/// # Authors
/// - Michael Faber
pub static COLLIDEOSCOPE: VariationDef = VariationDef {
    name: "collideoscope",
    aliases: &[],
    display_name: "Collideoscope",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("a", "A", unlimited_float, 0.2, -10.0, 10.0, "Per-sector angular offset — the parity-based mirror is shifted by ±a."),
        param!("num", "N", int, 1.0, 1.0, 50.0, "Number of angular sectors. Higher = finer kaleidoscope."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_collideoscope(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a_p = get_param(xform_id, variation_id, 0u);
    let num = max(get_param(xform_id, variation_id, 1u), 1.0);
    let pi = 3.14159265358979;
    let inv_pi = 0.3183098861837907;
    let pi_kn = pi / num;
    let ka_kn = pi * a_p / num;

    var a = atan2(p.y, p.x);
    let r = sqrt(p.x * p.x + p.y * p.y);
    let alt_input = select(-a, a, a >= 0.0);
    let alt = floor(alt_input * num * inv_pi);
    let alt_i = i32(alt);
    if (a >= 0.0) {
        if (alt_i % 2 == 0) {
            let v = ka_kn + a;
            a = alt * pi_kn + (v - floor(v / pi_kn) * pi_kn);
        } else {
            let v = -ka_kn + a;
            a = alt * pi_kn + (v - floor(v / pi_kn) * pi_kn);
        }
    } else {
        if (alt_i % 2 != 0) {
            let v = -ka_kn - a;
            a = -(alt * pi_kn + (v - floor(v / pi_kn) * pi_kn));
        } else {
            let v = ka_kn - a;
            a = -(alt * pi_kn + (v - floor(v / pi_kn) * pi_kn));
        }
    }
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: r#"
fn variation_collideoscope(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a_p = get_param(xform_id, variation_id, 0u);
    let num = max(get_param(xform_id, variation_id, 1u), 1.0);
    let pi = 3.14159265358979;
    let inv_pi = 0.3183098861837907;
    let pi_kn = pi / num;
    let ka_kn = pi * a_p / num;

    var a = atan2(p.y, p.x);
    let r = sqrt(p.x * p.x + p.y * p.y);
    let alt_input = select(-a, a, a >= 0.0);
    let alt = floor(alt_input * num * inv_pi);
    let alt_i = i32(alt);
    if (a >= 0.0) {
        if (alt_i % 2 == 0) {
            let v = ka_kn + a;
            a = alt * pi_kn + (v - floor(v / pi_kn) * pi_kn);
        } else {
            let v = -ka_kn + a;
            a = alt * pi_kn + (v - floor(v / pi_kn) * pi_kn);
        }
    } else {
        if (alt_i % 2 != 0) {
            let v = -ka_kn - a;
            a = -(alt * pi_kn + (v - floor(v / pi_kn) * pi_kn));
        } else {
            let v = ka_kn - a;
            a = -(alt * pi_kn + (v - floor(v / pi_kn) * pi_kn));
        }
    }
    return vec3<f32>(r * cos(a), r * sin(a), p.z);
}
"#,
};

// =============================================================================
// bent2: per-quadrant scale (Apophysis pack)
//   if x < 0:  out_x = x · x_scale  else: out_x = x
//   if y < 0:  out_y = y · y_scale  else: out_y = y
// =============================================================================
/// Per-quadrant scale — positive coordinates pass through unchanged,
/// while negative coordinates are multiplied by per-axis scale factors.
/// Bends the quadrants asymmetrically.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static BENT2: VariationDef = VariationDef {
    name: "bent2",
    aliases: &[],
    display_name: "Bent 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("x", "X scale", unlimited_float, 1.0, -10.0, 10.0, "X-axis scale applied where x < 0. Positive x passes through unchanged."),
        param!("y", "Y scale", unlimited_float, 1.0, -10.0, 10.0, "Y-axis scale applied where y < 0. Positive y passes through unchanged."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_bent2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let xs = get_param(xform_id, variation_id, 0u);
    let ys = get_param(xform_id, variation_id, 1u);
    let nx = select(p.x, p.x * xs, p.x < 0.0);
    let ny = select(p.y, p.y * ys, p.y < 0.0);
    return vec2<f32>(nx, ny);
}
"#,
    wgsl_3d: r#"
fn variation_bent2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xs = get_param(xform_id, variation_id, 0u);
    let ys = get_param(xform_id, variation_id, 1u);
    let nx = select(p.x, p.x * xs, p.x < 0.0);
    let ny = select(p.y, p.y * ys, p.y < 0.0);
    return vec3<f32>(nx, ny, p.z);
}
"#,
};

// =============================================================================
// mcarpet: bubble + twist + tilt (FracFx)
//   T = (x²+y²)/4 + 1
//   r = 1/T          (VVAR factored out)
//   FPx += x·r·x_scale + (1 − twist·x² + y)
//   FPy += y·r·y_scale + tilt·x
// =============================================================================
/// Bubble warp with twist and tilt — applies a bubble-style radial scaling
/// `r = 1 / (r²/4 + 1)` to each axis with independent scale factors, then
/// adds a quadratic twist term to X and a linear tilt term to Y.
///
/// # Authors
/// - FracFx
pub static MCARPET: VariationDef = VariationDef {
    name: "mcarpet",
    aliases: &[],
    display_name: "MCarpet",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("x", "X scale", unlimited_float, 1.0, -10.0, 10.0, "X-axis bubble scale."),
        param!("y", "Y scale", unlimited_float, 1.0, -10.0, 10.0, "Y-axis bubble scale."),
        param!("twist", "Twist", unlimited_float, 0.0, -10.0, 10.0, "Quadratic twist amount on X output (subtracts `twist · x²`)."),
        param!("tilt", "Tilt", unlimited_float, 0.0, -10.0, 10.0, "Linear tilt amount on Y output (adds `tilt · x`)."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_mcarpet(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let xs = get_param(xform_id, variation_id, 0u);
    let ys = get_param(xform_id, variation_id, 1u);
    let twist = get_param(xform_id, variation_id, 2u);
    let tilt = get_param(xform_id, variation_id, 3u);
    let T = 0.25 * (p.x * p.x + p.y * p.y) + 1.0;
    let r = 1.0 / max(T, 1e-30);
    return vec2<f32>(
        p.x * r * xs + (1.0 - twist * p.x * p.x + p.y),
        p.y * r * ys + tilt * p.x,
    );
}
"#,
    wgsl_3d: r#"
fn variation_mcarpet(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let xs = get_param(xform_id, variation_id, 0u);
    let ys = get_param(xform_id, variation_id, 1u);
    let twist = get_param(xform_id, variation_id, 2u);
    let tilt = get_param(xform_id, variation_id, 3u);
    let T = 0.25 * (p.x * p.x + p.y * p.y) + 1.0;
    let r = 1.0 / max(T, 1e-30);
    return vec3<f32>(
        p.x * r * xs + (1.0 - twist * p.x * p.x + p.y),
        p.y * r * ys + tilt * p.x,
        p.z,
    );
}
"#,
};

// =============================================================================
// lineart3d: per-axis sign-preserving power (FractalDesire)
//   out = (sign(x)·|x|^powX, sign(y)·|y|^powY, sign(z)·|z|^powZ)
// (Note: cpp/Java filename is `lineart3D` but the APO_PLUGIN string is
// `linearT3D` — porter naming inconsistency. We use lowercase `lineart3d`
// for our registry; it's the same variation.)
// =============================================================================
/// Per-axis sign-preserving power — applies `sign(coord) · |coord|^pow` to
/// each axis independently, with separate exponents per axis. Preserves the
/// sign while compressing or expanding the magnitude.
///
/// # Authors
/// - FractalDesire
pub static LINEART3D: VariationDef = VariationDef {
    name: "linearT3D",
    aliases: &[],
    // "Linear T 3D" — 3D power-curve linear variant (JWF `LinearT3DFunc`).
    // Previously mislabeled "Line Art 3D" from misreading "linearT".
    display_name: "Linear T 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    parameters: &[
        param!("powX", "Power X", unlimited_float, 1.2, -10.0, 10.0, "X-axis power exponent."),
        param!("powY", "Power Y", unlimited_float, 1.2, -10.0, 10.0, "Y-axis power exponent."),
        param!("powZ", "Power Z", unlimited_float, 1.2, -10.0, 10.0, "Z-axis power exponent (3D only)."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_linearT3D(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let powx = get_param(xform_id, variation_id, 0u);
    let powy = get_param(xform_id, variation_id, 1u);
    let sx = select(sign(p.x), 1.0, p.x == 0.0);
    let sy = select(sign(p.y), 1.0, p.y == 0.0);
    return vec2<f32>(
        sx * pow(max(abs(p.x), 1e-30), powx),
        sy * pow(max(abs(p.y), 1e-30), powy),
    );
}
"#,
    wgsl_3d: r#"
fn variation_linearT3D(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let powx = get_param(xform_id, variation_id, 0u);
    let powy = get_param(xform_id, variation_id, 1u);
    let powz = get_param(xform_id, variation_id, 2u);
    let sx = select(sign(p.x), 1.0, p.x == 0.0);
    let sy = select(sign(p.y), 1.0, p.y == 0.0);
    let sz = select(sign(p.z), 1.0, p.z == 0.0);
    return vec3<f32>(
        sx * pow(max(abs(p.x), 1e-30), powx),
        sy * pow(max(abs(p.y), 1e-30), powy),
        sz * pow(max(abs(p.z), 1e-30), powz),
    );
}
"#,
};

// =============================================================================
// oscilloscope: Y-flip inside damped cosine band (Apophysis pack)
//   tpf = 2π · frequency
//   noDamping = |damping| ≤ ε
//   t = noDamping ? amplitude · cos(tpf · x) + separation
//                 : amplitude · exp(−|x|·damping) · cos(tpf · x) + separation
//   if |y| ≤ t: out = (x, -y)
//   else:        out = (x, y)
// =============================================================================
/// Y-flip inside a damped cosine band — computes a (possibly damped)
/// cosine envelope `t(x) = amp · cos(2π·freq·x) · exp(−|x|·damping) +
/// separation`. Points whose `|y| ≤ t(x)` get their Y coordinate
/// flipped; others pass through. Produces an oscilloscope-trace-like
/// masking pattern.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static OSCILLOSCOPE: VariationDef = VariationDef {
    name: "oscilloscope",
    aliases: &[],
    display_name: "Oscilloscope",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("separation", "Separation", unlimited_float, 1.0, -10.0, 10.0, "Vertical offset of the band threshold, added to the cosine envelope."),
        param!("frequency", "Frequency", unlimited_float, 3.14159265, -100.0, 100.0, "Cosine frequency (multiplied by 2π internally)."),
        param!("amplitude", "Amplitude", unlimited_float, 1.0, -10.0, 10.0, "Cosine amplitude."),
        param!("damping", "Damping", unlimited_float, 0.0, -10.0, 10.0, "Exponential damping rate. Values near zero disable the damping term."),
    ],
    // 1 derived value at slot 4:
    //   4: tpf  (2π · frequency)
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_oscilloscope(user: array<f32, 4>) -> array<f32, 1> {
    var out: array<f32, 1>;
    out[0] = 6.28318530717959 * user[1];
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_oscilloscope(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let separation = get_param(xform_id, variation_id, 0u);
    let amplitude = get_param(xform_id, variation_id, 2u);
    let damping = get_param(xform_id, variation_id, 3u);
    let tpf = get_param(xform_id, variation_id, 4u);
    let no_damping = abs(damping) <= 1e-6;
    var t: f32;
    if (no_damping) {
        t = amplitude * cos(tpf * p.x) + separation;
    } else {
        t = amplitude * exp(-abs(p.x) * damping) * cos(tpf * p.x) + separation;
    }
    if (abs(p.y) <= t) {
        return vec2<f32>(p.x, -p.y);
    }
    return p;
}
"#,
    wgsl_3d: r#"
fn variation_oscilloscope(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let separation = get_param(xform_id, variation_id, 0u);
    let amplitude = get_param(xform_id, variation_id, 2u);
    let damping = get_param(xform_id, variation_id, 3u);
    let tpf = get_param(xform_id, variation_id, 4u);
    let no_damping = abs(damping) <= 1e-6;
    var t: f32;
    if (no_damping) {
        t = amplitude * cos(tpf * p.x) + separation;
    } else {
        t = amplitude * exp(-abs(p.x) * damping) * cos(tpf * p.x) + separation;
    }
    if (abs(p.y) <= t) {
        return vec3<f32>(p.x, -p.y, p.z);
    }
    return p;
}
"#,
};

// =============================================================================
// fibonacci2: Binet-formula complex Fibonacci (Larry Berlin)
//   Generates the Fibonacci sequence on integer x via the closed-form
//     F(z) = (φ^z − (−φ)^(−z)) / sqrt(5)
//   extended to complex z; full body uses two polar-radius/angle pairs.
//   Constants:
//     φ           = (1 + sqrt(5))/2 ≈ 1.6180339887
//     1/sqrt(5)   ≈ 0.4472135955
//     log(φ)      ≈ 0.4812118250596034
// =============================================================================
/// Binet-formula complex Fibonacci — extends the Fibonacci sequence to
/// complex inputs via the closed form `F(z) = (φ^z − (−φ)^(−z)) / √5`, with
/// `sc` scaling the magnitude and `sc2` scaling the polar-radius
/// exponential growth rate. Produces spiral patterns based on the golden
/// ratio.
///
/// # Authors
/// - Larry Berlin
pub static FIBONACCI2: VariationDef = VariationDef {
    name: "fibonacci2",
    aliases: &[],
    display_name: "Fibonacci 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[],
    parameters: &[
        param!("sc", "Scale", unlimited_float, 1.0, -10.0, 10.0, "Output magnitude scale."),
        param!("sc2", "Scale 2", unlimited_float, 1.0, -10.0, 10.0, "Polar-radius exponential scale — controls how quickly the magnitude grows along X."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_fibonacci2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let sc = get_param(xform_id, variation_id, 0u);
    let sc2 = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    let ffive = 0.4472135954999580;       // 1 / sqrt(5)
    let fnatlog = 0.4812118250596034;     // log(φ)

    let a = p.y * fnatlog;
    let snum1 = sin(a);
    let cnum1 = cos(a);
    let b = -(p.x * pi + p.y * fnatlog);
    let snum2 = sin(b);
    let cnum2 = cos(b);

    let eradius1 = sc * exp(sc2 * (p.x * fnatlog));
    let eradius2 = sc * exp(sc2 * (-(p.x * fnatlog - p.y * pi)));
    return vec2<f32>(
        (eradius1 * cnum1 - eradius2 * cnum2) * ffive,
        (eradius1 * snum1 - eradius2 * snum2) * ffive,
    );
}
"#,
    wgsl_3d: r#"
fn variation_fibonacci2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let sc = get_param(xform_id, variation_id, 0u);
    let sc2 = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    let ffive = 0.4472135954999580;
    let fnatlog = 0.4812118250596034;

    let a = p.y * fnatlog;
    let snum1 = sin(a);
    let cnum1 = cos(a);
    let b = -(p.x * pi + p.y * fnatlog);
    let snum2 = sin(b);
    let cnum2 = cos(b);

    let eradius1 = sc * exp(sc2 * (p.x * fnatlog));
    let eradius2 = sc * exp(sc2 * (-(p.x * fnatlog - p.y * pi)));
    return vec3<f32>(
        (eradius1 * cnum1 - eradius2 * cnum2) * ffive,
        (eradius1 * snum1 - eradius2 * snum2) * ffive,
        p.z,
    );
}
"#,
};
