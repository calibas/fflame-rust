//! Apophysis miscellany batch 20: cannabiscurve_wf, spherical3D_wf, swirl3D_wf
//!
//!   - cannabiscurve_wf: cannabis-curve polar plot (mathworld.wolfram.com
//!     /CannabisCurve.html). 1 user param `filled` (int). RNG when
//!     `filled == 1` (multiplies r by random01). Body factors cleanly
//!     through outer multiplier; Z passes through.
//!
//!   - spherical3D_wf: 3D spherical inversion with adjustable exponent.
//!     2 user params (invert int, exponent) + 1 init slot
//!     (_regularForm = |exponent - 2| < ε, stored as float). Body
//!     factors cleanly. Full3D.
//!
//!   - swirl3D_wf: 3D swirl with z-modulation. 1 user param
//!     `n`. No init. Body factors cleanly; cpp also writes color (TC),
//!     skipped here per `writes_color`-model conflict (compromise
//!     established in batch 60 for spirograph3D).
//!
//! Sources:
//!   - `output/jwildfire-vars/output/cannabiscurve_wf.cpp`
//!   - `output/jwildfire-vars/output/spherical3d_wf.cpp`
//!   - `output/jwildfire-vars/output/swirl3d_wf.cpp`

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// cannabiscurve_wf
// ---------------------------------------------------------------------------

/// Cannabis-curve polar plot — emits a point on the cannabis curve `r = (1
/// + 0.9·cos 8a) · (1 + 0.1·cos 24a) · (0.9 + 0.1·cos 200a) · (1 + sin a)`.
/// The curve is documented at
/// [MathWorld](https://mathworld.wolfram.com/CannabisCurve.html) by Eric W.
/// Weisstein. When `filled = 1`, randomizes the radius to fill the interior
/// of the curve.
pub static CANNABISCURVE_WF: VariationDef = VariationDef {
    name: "cannabiscurve_wf",
    aliases: &[],
    display_name: "Cannabis Curve WF",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("filled", "Filled", bool, true, "When on, fill the curve interior by randomizing the radius per iteration. When off, trace only the outline."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_cannabiscurve_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let filled = i32(get_param(xform_id, variation_id, 0u));
    var a = atan2(p.x, p.y);
    var r = (1.0 + 0.9 * cos(8.0 * a))
          * (1.0 + 0.1 * cos(24.0 * a))
          * (0.9 + 0.1 * cos(200.0 * a))
          * (1.0 + sin(a));
    a = a + 1.5707963267948966;
    if (filled == 1) {
        r = r * rng_nextf(rng);
    }
    return vec2<f32>(sin(a) * r, cos(a) * r);
}
"#,
    wgsl_3d: r#"
fn variation_cannabiscurve_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let filled = i32(get_param(xform_id, variation_id, 0u));
    var a = atan2(p.x, p.y);
    var r = (1.0 + 0.9 * cos(8.0 * a))
          * (1.0 + 0.1 * cos(24.0 * a))
          * (0.9 + 0.1 * cos(200.0 * a))
          * (1.0 + sin(a));
    a = a + 1.5707963267948966;
    if (filled == 1) {
        r = r * rng_nextf(rng);
    }
    return vec3<f32>(sin(a) * r, cos(a) * r, p.z);
}
"#,
};

// ---------------------------------------------------------------------------
// spherical3D_wf
// ---------------------------------------------------------------------------

/// 3D spherical inversion with adjustable exponent — emits `(x, y, z) /
/// r^exponent` where `r² = x² + y² + z²`. With `exponent = 2` (the default)
/// this reduces to a standard 3D spherical inversion; other values produce
/// stronger or weaker radial scaling. `invert` flips the sign of the
/// output.
pub static SPHERICAL3D_WF: VariationDef = VariationDef {
    name: "spherical3D_wf",
    aliases: &[],
    display_name: "Spherical 3D WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::AlwaysZ],
    parameters: &[
        param!("invert", "Invert", int, 0.0, 0.0, 1.0, "1 = flip the sign of the output (inverts through the origin); 0 = standard direction."),
        param!("exponent", "Exponent", unlimited_float, 2.0, -10.0, 10.0, "Radial-inversion exponent. 2 = standard spherical (`r⁻²`); higher = stronger inverse; lower = weaker."),
    ],
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_spherical3D_wf(user: array<f32, 2>) -> array<f32, 1> {
    var out: array<f32, 1>;
    if (abs(user[1] - 2.0) < 1e-6) {
        out[0] = 1.0;
    } else {
        out[0] = 0.0;
    }
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_spherical3D_wf(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let invert = i32(get_param(xform_id, variation_id, 0u));
    let exponent = get_param(xform_id, variation_id, 1u);
    let regular = get_param(xform_id, variation_id, 2u) > 0.5;
    let small = 1e-30;
    let denom = p.x * p.x + p.y * p.y + small;
    var r: f32;
    if (regular) {
        r = 1.0 / denom;
    } else {
        r = 1.0 / pow(max(denom, small), exponent * 0.5);
    }
    if (invert != 0) {
        r = -r;
    }
    return vec2<f32>(p.x * r, p.y * r);
}
"#,
    wgsl_3d: r#"
fn variation_spherical3D_wf(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let invert = i32(get_param(xform_id, variation_id, 0u));
    let exponent = get_param(xform_id, variation_id, 1u);
    let regular = get_param(xform_id, variation_id, 2u) > 0.5;
    let small = 1e-30;
    let denom = p.x * p.x + p.y * p.y + p.z * p.z + small;
    var r: f32;
    if (regular) {
        r = 1.0 / denom;
    } else {
        r = 1.0 / pow(max(denom, small), exponent * 0.5);
    }
    if (invert != 0) {
        r = -r;
    }
    return vec3<f32>(p.x * r, p.y * r, p.z * r);
}
"#,
};

// ---------------------------------------------------------------------------
// swirl3D_wf
// ---------------------------------------------------------------------------

/// 3D swirl with Z modulation — re-emits the input radius and angle in
/// cartesian form (XY output passes the input through unchanged), plus a Z
/// output `sin(6·cos(rad) − n·ang)` that introduces a sinusoidal Z
/// modulation parameterized by `n`. Matches JWildfire's `Swirl3DWFFunc`
/// (uses `getPrecalcAtanYX()` = `atan2(y, x)`).
///
/// Also a direct-color variation: JWF writes `pVarTP.color =
/// |sin(6·cos(rad) − n·ang)|` UNCONDITIONALLY — the same value as the
/// Z output, which is why its palette stripes track Z position. The
/// write was originally skipped citing the batch-60 spirograph3D
/// compromise, but that compromise concerned CONDITIONAL color
/// writes; an unconditional `*vc` write is exactly what
/// `Feature::WritesColor` models, and `direct_color` defaults to 1.0
/// so JWF's replace-the-color semantics hold for imported flames
/// (JWF .flame files carry no pluginColor attribute).
pub static SWIRL3D_WF: VariationDef = VariationDef {
    name: "swirl3D_wf",
    aliases: &[],
    display_name: "Swirl 3D WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::AlwaysZ, Feature::WritesColor],
    parameters: &[
        param!("n", "N", unlimited_float, 0.0, -10.0, 10.0, "Angular multiplier on the Z-output sine: `sin(6·cos(rad) − n·ang)`. Also drives the direct-color stripes (color = |z output|)."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_swirl3D_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec2<f32> {
    let n = get_param(xform_id, variation_id, 0u);
    let small = 1e-30;
    let rad = sqrt(p.x * p.x + p.y * p.y) + small;
    let ang = atan2(p.y, p.x);
    // JWF writes color unconditionally — |z output| (the z itself is
    // dropped in 2D mode, but the color stripe survives).
    *vc = abs(sin(6.0 * cos(rad) - n * ang));
    return vec2<f32>(rad * cos(ang), rad * sin(ang));
}
"#,
    wgsl_3d: r#"
fn variation_swirl3D_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec3<f32> {
    let n = get_param(xform_id, variation_id, 0u);
    let small = 1e-30;
    let rad = sqrt(p.x * p.x + p.y * p.y) + small;
    let ang = atan2(p.y, p.x);
    let z_out = sin(6.0 * cos(rad) - n * ang);
    // JWF writes color unconditionally — |z output|, which is why
    // the palette stripes correspond to Z position.
    *vc = abs(z_out);
    return vec3<f32>(
        rad * cos(ang),
        rad * sin(ang),
        z_out,
    );
}
"#,
};
