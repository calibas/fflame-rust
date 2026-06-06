//! Numbered variants of existing variations (continuation of `numbered.rs`)
//!
//! Three more "*2" / "*3d" extensions of variations we already support:
//!   - `bipolar2` (Apophysis pack, vars by Brad Stefanov) — bipolar with
//!     9 user-supplied scaling/offset knobs
//!   - `blob3d` — 3D extension of blob, with the angular wave modulating
//!     z as well as the in-plane radius
//!   - `circular2` — circular blur with user-supplied hash multipliers
//!     (the `circular` we just shipped hard-codes the magic numbers)
//!
//! Sources:
//!   - output/jwildfire-vars/output/bipolar2.cpp
//!   - output/jwildfire-vars/output/blob3D.cpp
//!   - output/jwildfire-vars/output/circular2.cpp
//!
//! All factor VVAR through the outer-multiplier convention.
//!
//! Skipped from this batch (deferred):
//!   - `loonie2` / `popcorn2_3d` / `cubic3d` / `glynnia3` — internal-weight
//!     in their math (need needs_transform AND a careful look at what
//!     "weight" means inside the formula). Watchlist material.
//!   - `bubblet3d` (532 lines, 6 params) — large; better as its own batch
//!     once we want a 3D-bubble follow-up.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// bipolar2: bipolar with 9 user-tunable knobs
//   x2y2 = (x² + y²) · g1
//   t = x2y2 + a
//   x2 = b · x
//   ps = -π/2 · shift
//   y' = c · atan2(e·y, x2y2 - d) + ps   (wrapped to [-π/2, π/2])
//   if g==0 or f/g <= 0: contribute 0
//   out_x = (2/π) · f1 · log((t + x2) / (t - x2))
//   out_y = (2/π) · y' · h
// =============================================================================
/// Bipolar coordinates with 9 user-tunable scaling/offset knobs — much more
/// configurable than the basic Bipolar.
///
/// # Authors
/// - Apophysis Plugin Pack
/// - Brad Stefanov
pub static BIPOLAR2: VariationDef = VariationDef {
    name: "bipolar2",
    aliases: &[],
    display_name: "Bipolar 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        param!("shift", "Shift", unlimited_float, 0.0, -2.0, 2.0, "Vertical offset added to the bipolar angle output."),
        param!("a", "A", unlimited_float, 1.0, -10.0, 10.0, "Inner offset for the radius² term."),
        param!("b", "B", unlimited_float, 2.0, -10.0, 10.0, "X scaling for the log term's numerator/denominator."),
        param!("c", "C", unlimited_float, 0.5, -10.0, 10.0, "Scaling on the angle output."),
        param!("d", "D", unlimited_float, 1.0, -10.0, 10.0, "Offset inside the atan2 denominator."),
        param!("e", "E", unlimited_float, 2.0, -10.0, 10.0, "Y scaling inside the atan2 numerator."),
        param!("f1", "F", unlimited_float, 0.25, -10.0, 10.0, "Output X scaling factor."),
        param!("g1", "G", unlimited_float, 1.0, -10.0, 10.0, "Outer scaling on the squared radius."),
        param!("h", "H", unlimited_float, 1.0, -10.0, 10.0, "Output Y scaling factor."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_bipolar2(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let shift = get_param(xform_id, variation_id, 0u);
    let a = get_param(xform_id, variation_id, 1u);
    let b = get_param(xform_id, variation_id, 2u);
    let c = get_param(xform_id, variation_id, 3u);
    let d = get_param(xform_id, variation_id, 4u);
    let e = get_param(xform_id, variation_id, 5u);
    let f1 = get_param(xform_id, variation_id, 6u);
    let g1 = get_param(xform_id, variation_id, 7u);
    let h = get_param(xform_id, variation_id, 8u);

    let pi = 3.14159265358979;
    let halfpi = 1.5707963267948966;
    let two_over_pi = 0.6366197723675813;

    let x2y2 = (p.x * p.x + p.y * p.y) * g1;
    let t = x2y2 + a;
    let x2 = b * p.x;
    let ps = -halfpi * shift;
    var y = c * atan2(e * p.y, x2y2 - d) + ps;

    if (y > halfpi) {
        y = -halfpi + (y + halfpi - floor((y + halfpi) / pi) * pi);
    } else if (y < -halfpi) {
        y = halfpi - (halfpi - y - floor((halfpi - y) / pi) * pi);
    }

    let f = t + x2;
    let g = t - x2;
    if (g == 0.0 || f / g <= 0.0) {
        return vec2<f32>(0.0, 0.0);
    }
    return vec2<f32>(
        f1 * two_over_pi * log(f / g),
        two_over_pi * y * h,
    );
}
"#,
    wgsl_3d: r#"
fn variation_bipolar2(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let shift = get_param(xform_id, variation_id, 0u);
    let a = get_param(xform_id, variation_id, 1u);
    let b = get_param(xform_id, variation_id, 2u);
    let c = get_param(xform_id, variation_id, 3u);
    let d = get_param(xform_id, variation_id, 4u);
    let e = get_param(xform_id, variation_id, 5u);
    let f1 = get_param(xform_id, variation_id, 6u);
    let g1 = get_param(xform_id, variation_id, 7u);
    let h = get_param(xform_id, variation_id, 8u);

    let pi = 3.14159265358979;
    let halfpi = 1.5707963267948966;
    let two_over_pi = 0.6366197723675813;

    let x2y2 = (p.x * p.x + p.y * p.y) * g1;
    let t = x2y2 + a;
    let x2 = b * p.x;
    let ps = -halfpi * shift;
    var y = c * atan2(e * p.y, x2y2 - d) + ps;

    if (y > halfpi) {
        y = -halfpi + (y + halfpi - floor((y + halfpi) / pi) * pi);
    } else if (y < -halfpi) {
        y = halfpi - (halfpi - y - floor((halfpi - y) / pi) * pi);
    }

    let f = t + x2;
    let g = t - x2;
    if (g == 0.0 || f / g <= 0.0) {
        return vec3<f32>(0.0, 0.0, p.z);
    }
    return vec3<f32>(
        f1 * two_over_pi * log(f / g),
        two_over_pi * y * h,
        p.z,
    );
}
"#,
};

// =============================================================================
// blob3d: 3D blob (radius modulated by sin(waves·θ), z modulated separately)
//   a = atan2(x, y)              (upstream cpp swap; matches Java getPrecalcAtan)
//   r = sqrt(x² + y²) · (low + (high − low) · (0.5 + 0.5·sin(waves·a)))
//   out = (sin(a)·r, cos(a)·r, sin(waves·a)·r)
//
// In 2D mode, drop the z-component (the in-plane geometry is the same as
// the original 2D blob with sin/cos swapped on the angle, faithful to
// upstream).
// =============================================================================
/// 3D version of Blob — same wavy boundary as Blob, plus a Z component that
/// modulates with the same waves pattern.
pub static BLOB3D: VariationDef = VariationDef {
    name: "blob3D",
    aliases: &[],
    display_name: "Blob 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[],
    parameters: &[
        param!("low", "Low", unlimited_float, 0.3, -5.0, 5.0, "Inner radius — how close the bumps recede in the troughs."),
        param!("high", "High", unlimited_float, 1.2, -5.0, 5.0, "Outer radius — how far the bumps reach at their peaks."),
        param!("waves", "Waves", unlimited_float, 6.0, 0.0, 30.0, "Number of bumps around the perimeter."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_blob3D(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let low = get_param(xform_id, variation_id, 0u);
    let high = get_param(xform_id, variation_id, 1u);
    let waves = get_param(xform_id, variation_id, 2u);

    let a = atan2(p.x, p.y);
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let r = r0 * (low + (high - low) * (0.5 + 0.5 * sin(waves * a)));
    return vec2<f32>(sin(a) * r, cos(a) * r);
}
"#,
    wgsl_3d: r#"
fn variation_blob3D(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let low = get_param(xform_id, variation_id, 0u);
    let high = get_param(xform_id, variation_id, 1u);
    let waves = get_param(xform_id, variation_id, 2u);

    let a = atan2(p.x, p.y);
    let r0 = sqrt(p.x * p.x + p.y * p.y);
    let r = r0 * (low + (high - low) * (0.5 + 0.5 * sin(waves * a)));
    return vec3<f32>(sin(a) * r, cos(a) * r, sin(waves * a) * r);
}
"#,
};

// =============================================================================
// circular2: circular with user-tunable hash multipliers
//   Like `circular` but exposes the magic numbers (12.9898, 78.233) as
//   `xx` and `yy` parameters for users who want to vary the hash pattern.
// =============================================================================
/// Circular with user-tunable hash multipliers — same shape as Circular but
/// exposes the `(12.9898, 78.233)` magic numbers as `xx` / `yy` parameters.
///
/// # Authors
/// - Tatyana Zabanova
/// - Brad Stefanov
pub static CIRCULAR2: VariationDef = VariationDef {
    name: "circular2",
    aliases: &[],
    display_name: "Circular 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng],
    parameters: &[
        param!("angle", "Angle", angle, 90.0, "Maximum rotation per iteration (degrees)."),
        param!("seed", "Seed", unlimited_float, 0.0, -100.0, 100.0, "Random seed for the hash term — change to vary the pattern."),
        param!("xx", "X mul", unlimited_float, 12.9898, -100.0, 100.0, "X-axis multiplier for the hash. Default 12.9898 matches the standard Circular."),
        param!("yy", "Y mul", unlimited_float, 78.233, -100.0, 100.0, "Y-axis multiplier for the hash. Default 78.233 matches the standard Circular."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_circular2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let angle_deg = get_param(xform_id, variation_id, 0u);
    let seed = get_param(xform_id, variation_id, 1u);
    let xx = get_param(xform_id, variation_id, 2u);
    let yy = get_param(xform_id, variation_id, 3u);
    let c_a = angle_deg * 3.14159265358979 / 180.0;
    let h = sin(p.x * xx + p.y * yy + seed) * 43758.5453;
    let aux = h - trunc(h);
    let rnd = (2.0 * (rng_nextf(rng) + aux) - 2.0) * c_a;
    let rad = sqrt(p.x * p.x + p.y * p.y);
    let ang = atan2(p.y, p.x);
    return vec2<f32>(cos(ang + rnd) * rad, sin(ang + rnd) * rad);
}
"#,
    wgsl_3d: r#"
fn variation_circular2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let angle_deg = get_param(xform_id, variation_id, 0u);
    let seed = get_param(xform_id, variation_id, 1u);
    let xx = get_param(xform_id, variation_id, 2u);
    let yy = get_param(xform_id, variation_id, 3u);
    let c_a = angle_deg * 3.14159265358979 / 180.0;
    let h = sin(p.x * xx + p.y * yy + seed) * 43758.5453;
    let aux = h - trunc(h);
    let rnd = (2.0 * (rng_nextf(rng) + aux) - 2.0) * c_a;
    let rad = sqrt(p.x * p.x + p.y * p.y);
    let ang = atan2(p.y, p.x);
    return vec3<f32>(cos(ang + rnd) * rad, sin(ang + rnd) * rad, p.z);
}
"#,
};
