//! `inflateZ` family + foci_3D + sintrange
//!
//! Eight ports — six members of Larry Berlin's `inflateZ_*` series
//! plus two unrelated misc.
//!
//!   - `inflateZ_1`  (Larry Berlin) — `sin(atan2 y,x) - 2y` into Z
//!   - `inflateZ_2`  (Larry Berlin) — `0.25 - 0.667(x+y)` into Z
//!   - `inflateZ_3`  (Larry Berlin) — `0.2(π - atan2)·cos(3·atan2 +
//!                                       (y-x))` into Z
//!   - `inflateZ_4`  (Larry Berlin) — `±(π/2 - atan2)·0.25` (RNG sign)
//!                                       into Z
//!   - `inflateZ_5`  (Larry Berlin) — `cos(π/2 - atan2)/2` into Z
//!   - `inflateZ_6`  (Larry Berlin) — `1.5 - acos(sin·atan·sin·0.5)`
//!                                       into Z
//!   - `foci_3D`     (Larry Berlin) — 3D foci with `expx ± expnx` and
//!                                       `cosy·cosz` denominator;
//!                                       Full3D. (In 2D mode `boot`
//!                                       falls back to `atan2(y, x)`
//!                                       since z is unavailable.)
//!   - `sintrange`   (Ffey)         — `sin(x)·(x²+w-(x²+y²)·w)`
//!                                       per axis. cpp uses `FPx = …`
//!                                       (assignment, not `+=`); same
//!                                       cpp-quirk handling as
//!                                       `anamorphcyl`/`ennepers`/
//!                                       `crop3d` — works as long as
//!                                       sintrange is the only normal
//!                                       variation in its transform.
//!                                       1 user param `w` (default 1.0)
//!                                       distinct from VVAR.
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.
//!
//! All eight factor cleanly through outer.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// inflateZ_1: sin(ang) - 2y into Z
// =============================================================================
/// Z-axis inflation #1 — adds `sin(atan2(y,x)) − 2y` to the Z output. XY
/// pass through unchanged. The angular sine plus linear-Y term produces a
/// saddle-shaped Z surface.
///
/// # Authors
/// - Larry Berlin
pub static INFLATEZ_1: VariationDef = VariationDef {
    name: "inflateZ_1",
    aliases: &[],
    display_name: "Inflate Z 1",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_inflateZ_1(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: r#"
fn variation_inflateZ_1(p: vec3<f32>) -> vec3<f32> {
    let ang = atan2(p.y, p.x);
    let val1 = p.y * 2.0;
    return vec3<f32>(0.0, 0.0, sin(ang) - val1);
}
"#,
};

// =============================================================================
// inflateZ_2: 0.25 - 0.333·(2x + 2y) into Z
// =============================================================================
/// Z-axis inflation #2 — adds `0.25 − (2x + 2y)/3` to the Z output. XY pass
/// through unchanged. Z is a tilted plane depending on `x + y`.
///
/// # Authors
/// - Larry Berlin
pub static INFLATEZ_2: VariationDef = VariationDef {
    name: "inflateZ_2",
    aliases: &[],
    display_name: "Inflate Z 2",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_inflateZ_2(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: r#"
fn variation_inflateZ_2(p: vec3<f32>) -> vec3<f32> {
    let val1 = p.y * 2.0;
    let val2 = p.x * 2.0;
    let aval = (val1 + val2) * 0.3333333333333333;
    return vec3<f32>(0.0, 0.0, 0.25 - aval);
}
"#,
};

// =============================================================================
// inflateZ_3: 0.2·(π - ang)·cos(3·ang + (y - x)) into Z
// =============================================================================
/// Z-axis inflation #3 — adds `0.2 · (π − atan2(y,x)) · cos(3·atan2(y,x) +
/// (y − x))` to the Z output. An angular-cosine modulated radial term
/// producing a wavy 3D surface.
///
/// # Authors
/// - Larry Berlin
pub static INFLATEZ_3: VariationDef = VariationDef {
    name: "inflateZ_3",
    aliases: &[],
    display_name: "Inflate Z 3",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_inflateZ_3(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: r#"
fn variation_inflateZ_3(p: vec3<f32>) -> vec3<f32> {
    let pi = 3.14159265358979;
    let ang = atan2(p.y, p.x);
    let val1 = 0.2 * (pi - ang) * cos(3.0 * ang + (p.y - p.x));
    return vec3<f32>(0.0, 0.0, val1);
}
"#,
};

// =============================================================================
// inflateZ_4: ±(π/2 - ang) · 0.25 into Z (RNG sign)
// =============================================================================
/// Z-axis inflation #4 — adds `±(π/2 − atan2(y,x)) · 0.25` to the Z output,
/// with the sign chosen randomly each iteration. Produces a Z surface
/// that's a stochastic mirror of the input angle.
///
/// # Authors
/// - Larry Berlin
pub static INFLATEZ_4: VariationDef = VariationDef {
    name: "inflateZ_4",
    aliases: &[],
    display_name: "Inflate Z 4",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_inflateZ_4(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    // Phony assignment (WGSL rejects `let _`): still consumes the
    // RNG draw its 3D sibling would, keeping the stream aligned.
    _ = rng_nextf(rng);
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: r#"
fn variation_inflateZ_4(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let pi_2 = 1.5707963267948966;
    let ang1 = atan2(p.y, p.x);
    var val1 = pi_2 - ang1;
    if (rng_nextf(rng) < 0.5) {
        val1 = -val1;
    }
    return vec3<f32>(0.0, 0.0, val1 * 0.25);
}
"#,
};

// =============================================================================
// inflateZ_5: cos(π/2 - ang) / 2 into Z
// =============================================================================
/// Z-axis inflation #5 — adds `cos(π/2 − atan2(y,x)) / 2 = sin(atan2(y,x))
/// / 2` to the Z output. The simplest of the family — a sinusoidal Z
/// surface.
///
/// # Authors
/// - Larry Berlin
pub static INFLATEZ_5: VariationDef = VariationDef {
    name: "inflateZ_5",
    aliases: &[],
    display_name: "Inflate Z 5",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_inflateZ_5(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: r#"
fn variation_inflateZ_5(p: vec3<f32>) -> vec3<f32> {
    let pi_2 = 1.5707963267948966;
    let ang1 = atan2(p.y, p.x);
    let val1 = cos(pi_2 - ang1) * 0.5;
    return vec3<f32>(0.0, 0.0, val1);
}
"#,
};

// =============================================================================
// inflateZ_6: 1.5 - acos(sin(ang) · ang · sin(y - x) · 0.5) into Z
// =============================================================================
/// Z-axis inflation #6 — adds `1.5 − acos(sin(atan2(y,x)) · atan2(y,x) ·
/// sin(y − x) · 0.5)` to the Z output. The most complex of the family — an
/// arc-cosine of a triple-product term.
///
/// # Authors
/// - Larry Berlin
pub static INFLATEZ_6: VariationDef = VariationDef {
    name: "inflateZ_6",
    aliases: &[],
    display_name: "Inflate Z 6",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_inflateZ_6(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: r#"
fn variation_inflateZ_6(p: vec3<f32>) -> vec3<f32> {
    let ang = atan2(p.y, p.x);
    let adf = p.y - p.x;
    let kik = ang * sin(adf);
    let arg = clamp(sin(ang) * kik * 0.5, -1.0, 1.0);
    return vec3<f32>(0.0, 0.0, 1.5 - acos(arg));
}
"#,
};

// =============================================================================
// sintrange (Ffey)
//   v = (x² + y²) · w
//   out_x = sin(x) · (x² + w - v)
//   out_y = sin(y) · (y² + w - v)
// (cpp uses `FPx = …` rather than `+=`; works when sintrange is the
//  only normal variation in its transform. Same handling as
//  anamorphcyl/ennepers/crop3d.)
// =============================================================================
/// Sin × (squared − weighted-radius) — per axis emits `sin(coord) · (coord²
/// + w − (x² + y²)·w)`. The trailing weighted-radius subtraction modulates
/// the local sin profile based on distance from the origin.
///
/// # Authors
/// - Ffey
pub static SINTRANGE: VariationDef = VariationDef {
    name: "sintrange",
    aliases: &[],
    display_name: "Sin T Range",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Any,
    features: &[Feature::Replace],
    parameters: &[
        param!("w", "W", unlimited_float, 1.0, -10.0, 10.0, "Weight on the radius term `(x²+y²)·w` and the constant offset `+w`. Distinct from the variation's outer weight (VVAR)."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_sintrange(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w = get_param(xform_id, variation_id, 0u);
    let v = (p.x * p.x + p.y * p.y) * w;
    let fx = sin(p.x) * (p.x * p.x + w - v);
    let fy = sin(p.y) * (p.y * p.y + w - v);
    return vec2<f32>(fx, fy);
}
"#,
    wgsl_3d: r#"
fn variation_sintrange(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = get_param(xform_id, variation_id, 0u);
    let v = (p.x * p.x + p.y * p.y) * w;
    let fx = sin(p.x) * (p.x * p.x + w - v);
    let fy = sin(p.y) * (p.y * p.y + w - v);
    return vec3<f32>(fx, fy, p.z);
}
"#,
};

// =============================================================================
// foci_3D (Larry Berlin)
//   expx = exp(x) / 2; expnx = 0.25 / expx     (= exp(-x) / 2)
//   boot = z; if boot == 0: boot = atan2(y, x)
//   tmp = w / (expx + expnx - cos(y) · cos(boot))
//   out = ((expx - expnx) · tmp, sin(y) · tmp, sin(boot) · tmp)
// Clean factor through outer; Full3D.
// In 2D mode (`p: vec2<f32>`), z is unavailable so use the kikr
// fallback unconditionally.
// =============================================================================
/// 3D extension of the `foci` variation — emits `(expx − expnx, sin(y),
/// sin(boot)) / (expx + expnx − cos(y)·cos(boot))` where `expx = e^x / 2,
/// expnx = e^(−x) / 2`, and `boot = z` (or `atan2(y, x)` when `z = 0`, the
/// 2D fallback). Adds a depth dimension to the classic foci warp.
///
/// # Authors
/// - Larry Berlin
pub static FOCI_3D: VariationDef = VariationDef {
    name: "foci_3D",
    aliases: &[],
    display_name: "Foci 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::AlwaysZ],
    parameters: &[],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_foci_3D(p: vec2<f32>) -> vec2<f32> {
    let expx = exp(p.x) * 0.5;
    let expnx = 0.25 / max(expx, 1e-30);
    let boot = atan2(p.y, p.x);
    let cosy = cos(p.y);
    let cosz = cos(boot);
    let denom = expx + expnx - cosy * cosz;
    let safe_denom = select(denom, 1e-30, abs(denom) < 1e-30);
    let tmp = 1.0 / safe_denom;
    let fx = (expx - expnx) * tmp;
    let fy = sin(p.y) * tmp;
    return vec2<f32>(fx, fy);
}
"#,
    wgsl_3d: r#"
fn variation_foci_3D(p: vec3<f32>) -> vec3<f32> {
    let expx = exp(p.x) * 0.5;
    let expnx = 0.25 / max(expx, 1e-30);
    var boot = p.z;
    if (boot == 0.0) {
        boot = atan2(p.y, p.x);
    }
    let cosy = cos(p.y);
    let cosz = cos(boot);
    let denom = expx + expnx - cosy * cosz;
    let safe_denom = select(denom, 1e-30, abs(denom) < 1e-30);
    let tmp = 1.0 / safe_denom;
    let fx = (expx - expnx) * tmp;
    let fy = sin(p.y) * tmp;
    let fz = sin(boot) * tmp;
    return vec3<f32>(fx, fy, fz);
}
"#,
};
