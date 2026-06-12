//! Classic blades + simple Apophysis classics
//!
//! Nine more variations:
//!
//!   - `arch`        (Scott Draves)       — RNG-driven angle bend
//!   - `bi_linear`                        — clean (y, x) swap
//!   - `blade`       (Z+ Jan 2007)        — RNG-driven blade fan
//!   - `blade3D`     (Z+ Jan 2007)        — blade with z output
//!                                           (Full3D)
//!   - `squarize`    (Faber)              — angle pack: square map
//!   - `squish`      (Faber)              — square map + cell mod
//!                                           power
//!   - `twoface`                          — half-spherical/half-pass
//!   - `twintrian`   (Z+ Jan 2007)        — RNG twin trignal
//!   - `unpolar`                          — exp/sin polar inversion
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.
//!
//! Z+ family (`blade`, `blade3D`, `twintrian`) has VVAR
//! inside an angle (non-linear), so each uses `needs_transform`
//! to divide-out the cpp's outer `VVAR ·` factor.
//!
//! `twoface` and `unpolar` likewise use internal-VVAR factors
//! (twoface: `r := VVAR; if x>0 r/=…`; unpolar: precompute
//! `vvar_2 = w/(2π)`). Both divide-out via `needs_transform`.
//!
//! `bi_linear` and `squarize` factor cleanly through outer.
//!
//! `squish` is clean (factor through outer) with one user param
//! (`power` int, default 2) and one init slot (`inv_power = 1/power`).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// arch: RNG-driven angle bend
//   ang = rand · w · π
//   out = (w · sin ang, w · sin² ang / cos ang)
// VVAR inside `ang` is non-linear → needs_transform divide-out.
// =============================================================================
/// RNG-driven angle bend — picks a random angle `ang = rand · w · π`, then
/// emits `(w · sin(ang), w · sin²(ang) / cos(ang))`. The `sin²/cos = sin ·
/// tan` term creates a smooth arch shape with a vertical asymptote.
///
/// # Authors
/// - Scott Draves
pub static ARCH: VariationDef = VariationDef {
    name: "arch",
    aliases: &[],
    display_name: "Arch",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsTransform],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_arch(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let pi = 3.14159265358979;
    let ang = rng_nextf(rng) * w * pi;
    let sinr = sin(ang);
    let cosr = cos(ang);
    let safe_cosr = select(cosr, 1e-30, abs(cosr) < 1e-30);
    let fx = w * sinr;
    let fy = w * sinr * sinr / safe_cosr;
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_arch(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let pi = 3.14159265358979;
    let ang = rng_nextf(rng) * w * pi;
    let sinr = sin(ang);
    let cosr = cos(ang);
    let safe_cosr = select(cosr, 1e-30, abs(cosr) < 1e-30);
    let fx = w * sinr;
    let fy = w * sinr * sinr / safe_cosr;
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#,
};

// =============================================================================
// bi_linear: clean (y, x) swap
//   out = (w·y, w·x) — clean factor through outer.
// =============================================================================
/// Coordinate swap — outputs `(y, x)`. The simplest possible 2D coordinate
/// operation; useful as a building block in combination with other
/// variations.
pub static BI_LINEAR: VariationDef = VariationDef {
    name: "bi_linear",
    aliases: &[],
    display_name: "Bi-Linear",
    category: VariationCategory::Basic2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_bi_linear(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(p.y, p.x);
}
"#,
    wgsl_3d: r#"
fn variation_bi_linear(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(p.y, p.x, p.z);
}
"#,
};

// =============================================================================
// blade: RNG-driven blade fan (Z+ Jan 2007)
//   r = rand · w · sqrt(x² + y²)
//   out = (w · x · (cos r + sin r), w · x · (cos r - sin r))
// VVAR inside r is non-linear → divide-out.
// =============================================================================
/// RNG-driven blade fan — picks a random radius `r = rand · w · sqrt(x² +
/// y²)`, then emits `(w · x · (cos r + sin r), w · x · (cos r − sin r))`.
/// Both output axes are driven by the input X, so the result spreads along
/// the X axis like blades of a fan.
///
/// # Authors
/// - Z+
pub static BLADE: VariationDef = VariationDef {
    name: "blade",
    aliases: &[],
    display_name: "Blade",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsTransform],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_blade(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let r = rng_nextf(rng) * w * sqrt(p.x * p.x + p.y * p.y);
    let sinr = sin(r);
    let cosr = cos(r);
    let fx = w * p.x * (cosr + sinr);
    let fy = w * p.x * (cosr - sinr);
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_blade(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let r = rng_nextf(rng) * w * sqrt(p.x * p.x + p.y * p.y);
    let sinr = sin(r);
    let cosr = cos(r);
    let fx = w * p.x * (cosr + sinr);
    let fy = w * p.x * (cosr - sinr);
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#,
};

// =============================================================================
// blade3D: blade with z output (Z+ Jan 2007)
//   Same x, y as blade; additionally:
//     z_out = w · y · (sin r - cos r)
// Full3D variation (writes z explicitly, not preserve-z pass-through).
// =============================================================================
/// 3D extension of `blade` — same X/Y outputs as `blade`, plus a Z output
/// `w · y · (sin r − cos r)` driven by input Y. Completes the 3D blade
/// structure.
///
/// # Authors
/// - Z+
pub static BLADE3D: VariationDef = VariationDef {
    name: "blade3D",
    aliases: &[],
    display_name: "Blade 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsTransform, Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_blade3D(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let r = rng_nextf(rng) * w * sqrt(p.x * p.x + p.y * p.y);
    let sinr = sin(r);
    let cosr = cos(r);
    let fx = w * p.x * (cosr + sinr);
    let fy = w * p.x * (cosr - sinr);
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_blade3D(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let r = rng_nextf(rng) * w * sqrt(p.x * p.x + p.y * p.y);
    let sinr = sin(r);
    let cosr = cos(r);
    let fx = w * p.x * (cosr + sinr);
    let fy = w * p.x * (cosr - sinr);
    let fz = w * p.y * (sinr - cosr);
    return vec3<f32>(fx * inv_w, fy * inv_w, fz * inv_w);
}
"#,
};

// =============================================================================
// squarize: angle-pack square map (Faber)
//   s = sqrt(x² + y²)
//   a = atan2(y, x); if a < 0: a += 2π
//   p = 4·s·a/π
//   Five branches based on p vs k·s for k ∈ {1, 3, 5, 7}.
// Clean factor through outer.
// =============================================================================
/// Angle-pack square map — converts polar coordinates `(s, a)` to a
/// position on a square of side `s` by treating `q = 4·s·a/π` as a
/// perimeter parameter and dispatching to one of 5 edge-segment branches.
/// Effectively wraps the unit circle around a unit square.
///
/// # Authors
/// - Michael Faber
pub static SQUARIZE: VariationDef = VariationDef {
    name: "squarize",
    aliases: &[],
    display_name: "Squarize",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_squarize(p: vec2<f32>) -> vec2<f32> {
    let two_pi = 6.28318530717959;
    let inv_pi = 0.31830988618379;
    let s = sqrt(p.x * p.x + p.y * p.y);
    var a = atan2(p.y, p.x);
    if (a < 0.0) {
        a = a + two_pi;
    }
    let q = 4.0 * s * a * inv_pi;
    if (q <= s) {
        return vec2<f32>(s, q);
    } else if (q <= 3.0 * s) {
        return vec2<f32>(2.0 * s - q, s);
    } else if (q <= 5.0 * s) {
        return vec2<f32>(-s, 4.0 * s - q);
    } else if (q <= 7.0 * s) {
        return vec2<f32>(-(6.0 * s - q), -s);
    } else {
        return vec2<f32>(s, -(8.0 * s - q));
    }
}
"#,
    wgsl_3d: r#"
fn variation_squarize(p: vec3<f32>) -> vec3<f32> {
    let two_pi = 6.28318530717959;
    let inv_pi = 0.31830988618379;
    let s = sqrt(p.x * p.x + p.y * p.y);
    var a = atan2(p.y, p.x);
    if (a < 0.0) {
        a = a + two_pi;
    }
    let q = 4.0 * s * a * inv_pi;
    if (q <= s) {
        return vec3<f32>(s, q, p.z);
    } else if (q <= 3.0 * s) {
        return vec3<f32>(2.0 * s - q, s, p.z);
    } else if (q <= 5.0 * s) {
        return vec3<f32>(-s, 4.0 * s - q, p.z);
    } else if (q <= 7.0 * s) {
        return vec3<f32>(-(6.0 * s - q), -s, p.z);
    } else {
        return vec3<f32>(s, -(8.0 * s - q), p.z);
    }
}
"#,
};

// =============================================================================
// squish: square map + cell mod power (Faber)
//   1 user param (power int, default 2)
//   1 init slot (inv_power = 1/power)
//   Quadrant routing on |x| vs |y| → p value, then add
//   8·s·floor(power · rand) cell offset, then route through
//   squarize-style branches.
// Clean factor through outer; needs RNG for cell selection.
// =============================================================================
/// Square map with cell mod power — extends `squarize` with a random cell
/// selection: adds an `8·s·floor(power · rand)` offset to the perimeter
/// parameter before dispatch, then divides by `power`. Produces `power`
/// discrete tiles of the squarize pattern.
///
/// # Authors
/// - Michael Faber
pub static SQUISH: VariationDef = VariationDef {
    name: "squish",
    aliases: &[],
    display_name: "Squish",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("power", "Power", int, 2.0, 2.0, 100.0, "Number of discrete tiles the squarize pattern is divided into (≥ 2)."),
    ],
    // 1 derived value at slot 1: inv_power = 1/power
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_squish(user: array<f32, 1>) -> array<f32, 1> {
    let power = max(user[0], 2.0);
    var out: array<f32, 1>;
    out[0] = 1.0 / power;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_squish(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let power = max(get_param(xform_id, variation_id, 0u), 2.0);
    let inv_power = get_param(xform_id, variation_id, 1u);

    let ax = abs(p.x);
    let ay = abs(p.y);
    var s: f32;
    var q: f32;
    if (ax > ay) {
        s = ax;
        if (p.x > 0.0) {
            q = p.y;
        } else {
            q = 4.0 * s - p.y;
        }
    } else {
        s = ay;
        if (p.y > 0.0) {
            q = 2.0 * s - p.x;
        } else {
            q = 6.0 * s + p.x;
        }
    }
    q = inv_power * (q + 8.0 * s * floor(power * rng_nextf(rng)));

    if (q <= s) {
        return vec2<f32>(s, q);
    } else if (q <= 3.0 * s) {
        return vec2<f32>(2.0 * s - q, s);
    } else if (q <= 5.0 * s) {
        return vec2<f32>(-s, 4.0 * s - q);
    } else if (q <= 7.0 * s) {
        return vec2<f32>(-(6.0 * s - q), -s);
    } else {
        return vec2<f32>(s, -(8.0 * s - q));
    }
}
"#,
    wgsl_3d: r#"
fn variation_squish(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let power = max(get_param(xform_id, variation_id, 0u), 2.0);
    let inv_power = get_param(xform_id, variation_id, 1u);

    let ax = abs(p.x);
    let ay = abs(p.y);
    var s: f32;
    var q: f32;
    if (ax > ay) {
        s = ax;
        if (p.x > 0.0) {
            q = p.y;
        } else {
            q = 4.0 * s - p.y;
        }
    } else {
        s = ay;
        if (p.y > 0.0) {
            q = 2.0 * s - p.x;
        } else {
            q = 6.0 * s + p.x;
        }
    }
    q = inv_power * (q + 8.0 * s * floor(power * rng_nextf(rng)));

    if (q <= s) {
        return vec3<f32>(s, q, p.z);
    } else if (q <= 3.0 * s) {
        return vec3<f32>(2.0 * s - q, s, p.z);
    } else if (q <= 5.0 * s) {
        return vec3<f32>(-s, 4.0 * s - q, p.z);
    } else if (q <= 7.0 * s) {
        return vec3<f32>(-(6.0 * s - q), -s, p.z);
    } else {
        return vec3<f32>(s, -(8.0 * s - q), p.z);
    }
}
"#,
};

// =============================================================================
// twoface: half-spherical / half-pass 
//   r = w; if x > 0: r /= x² + y²
//   out = r · (x, y)
// VVAR inside `r` is multiplied with input — needs_transform divide-out.
// =============================================================================
/// Half-spherical / half-pass — points with `x ≤ 0` get scaled output `w ·
/// (x, y)`; points with `x > 0` get spherical-inverted (`w/(x²+y²) · (x,
/// y)`). Combines a linear left side with a spherical right side — hence
/// the name.
pub static TWOFACE: VariationDef = VariationDef {
    name: "twoface",
    aliases: &[],
    display_name: "Two Face",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsTransform],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_twoface(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    var r = w;
    if (p.x > 0.0) {
        let denom = p.x * p.x + p.y * p.y;
        r = r / max(denom, 1e-30);
    }
    return vec2<f32>(r * p.x * inv_w, r * p.y * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_twoface(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    var r = w;
    if (p.x > 0.0) {
        let denom = p.x * p.x + p.y * p.y;
        r = r / max(denom, 1e-30);
    }
    return vec3<f32>(r * p.x * inv_w, r * p.y * inv_w, p.z);
}
"#,
};

// =============================================================================
// twintrian: RNG twin trignal (Z+ Jan 2007)
//   r = rand · w · (sqrt(x²+y²) + ε)
//   diff = log10(sin² r) + cos r;  if |diff| < ε: diff = -30
//   out = (w · x · diff, w · x · (diff - sin r · π))
// VVAR inside `r` non-linear → divide-out.
// =============================================================================
/// RNG twin trigonal — picks a random `r = rand · w · sqrt(x² + y²)`,
/// computes `diff = log₁₀(sin² r) + cos r` (forced to −30 when degenerate),
/// then emits `(w · x · diff, w · x · (diff − sin r · π))`. The log term
/// creates twin trigonometric interference patterns.
///
/// # Authors
/// - Z+
pub static TWINTRIAN: VariationDef = VariationDef {
    name: "twintrian",
    aliases: &[],
    display_name: "Twin Trian",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::NeedsTransform],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_twintrian(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let pi = 3.14159265358979;
    let inv_log10 = 0.43429448190325176;  // 1/ln(10)

    let r = rng_nextf(rng) * w * (sqrt(p.x * p.x + p.y * p.y) + 1e-30);
    let sinr = sin(r);
    let cosr = cos(r);
    var diff = log(max(sinr * sinr, 1e-30)) * inv_log10 + cosr;
    if (abs(diff) < 1e-30) {
        diff = -30.0;
    }
    let fx = w * p.x * diff;
    let fy = w * p.x * (diff - sinr * pi);
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_twintrian(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let pi = 3.14159265358979;
    let inv_log10 = 0.43429448190325176;

    let r = rng_nextf(rng) * w * (sqrt(p.x * p.x + p.y * p.y) + 1e-30);
    let sinr = sin(r);
    let cosr = cos(r);
    var diff = log(max(sinr * sinr, 1e-30)) * inv_log10 + cosr;
    if (abs(diff) < 1e-30) {
        diff = -30.0;
    }
    let fx = w * p.x * diff;
    let fy = w * p.x * (diff - sinr * pi);
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#,
};

// =============================================================================
// unpolar: exp/sin polar inversion 
//   precompute vvar_2 = w / (2π)
//   r = exp(y), s = sin(x), c = cos(x)
//   out = (vvar_2 · r · s, vvar_2 · r · c)
// VVAR inside vvar_2 → needs_transform divide-out (factor strips
// one w; outer × w restores).
// =============================================================================
/// Exp/sin polar inversion — outputs `(w/(2π) · exp(y) · sin x, w/(2π) ·
/// exp(y) · cos x)`. The inverse of `polar` — converts log-polar
/// coordinates back to cartesian.
pub static UNPOLAR: VariationDef = VariationDef {
    name: "unpolar",
    aliases: &[],
    display_name: "Unpolar",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsTransform],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_unpolar(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let inv_two_pi = 0.15915494309189535;  // 1/(2π)

    let r = exp(p.y);
    let s = sin(p.x);
    let c = cos(p.x);
    let vvar_2 = w * inv_two_pi;
    let fx = vvar_2 * r * s;
    let fy = vvar_2 * r * c;
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: r#"
fn variation_unpolar(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let inv_two_pi = 0.15915494309189535;

    let r = exp(p.y);
    let s = sin(p.x);
    let c = cos(p.x);
    let vvar_2 = w * inv_two_pi;
    let fx = vvar_2 * r * s;
    let fy = vvar_2 * r * c;
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#,
};
