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
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// inflateZ_1: sin(ang) - 2y into Z
// =============================================================================
pub static INFLATEZ_1: VariationDef = VariationDef {
    name: "inflateZ_1",
    display_name: "Inflate Z 1",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_inflateZ_1(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: Some(r#"
fn variation_inflateZ_1(p: vec3<f32>) -> vec3<f32> {
    let ang = atan2(p.y, p.x);
    let val1 = p.y * 2.0;
    return vec3<f32>(0.0, 0.0, sin(ang) - val1);
}
"#),
};

// =============================================================================
// inflateZ_2: 0.25 - 0.333·(2x + 2y) into Z
// =============================================================================
pub static INFLATEZ_2: VariationDef = VariationDef {
    name: "inflateZ_2",
    display_name: "Inflate Z 2",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_inflateZ_2(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: Some(r#"
fn variation_inflateZ_2(p: vec3<f32>) -> vec3<f32> {
    let val1 = p.y * 2.0;
    let val2 = p.x * 2.0;
    let aval = (val1 + val2) * 0.3333333333333333;
    return vec3<f32>(0.0, 0.0, 0.25 - aval);
}
"#),
};

// =============================================================================
// inflateZ_3: 0.2·(π - ang)·cos(3·ang + (y - x)) into Z
// =============================================================================
pub static INFLATEZ_3: VariationDef = VariationDef {
    name: "inflateZ_3",
    display_name: "Inflate Z 3",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_inflateZ_3(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: Some(r#"
fn variation_inflateZ_3(p: vec3<f32>) -> vec3<f32> {
    let pi = 3.14159265358979;
    let ang = atan2(p.y, p.x);
    let val1 = 0.2 * (pi - ang) * cos(3.0 * ang + (p.y - p.x));
    return vec3<f32>(0.0, 0.0, val1);
}
"#),
};

// =============================================================================
// inflateZ_4: ±(π/2 - ang) · 0.25 into Z (RNG sign)
// =============================================================================
pub static INFLATEZ_4: VariationDef = VariationDef {
    name: "inflateZ_4",
    display_name: "Inflate Z 4",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_inflateZ_4(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let _ = rng_nextf(rng);
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: Some(r#"
fn variation_inflateZ_4(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let pi_2 = 1.5707963267948966;
    let ang1 = atan2(p.y, p.x);
    var val1 = pi_2 - ang1;
    if (rng_nextf(rng) < 0.5) {
        val1 = -val1;
    }
    return vec3<f32>(0.0, 0.0, val1 * 0.25);
}
"#),
};

// =============================================================================
// inflateZ_5: cos(π/2 - ang) / 2 into Z
// =============================================================================
pub static INFLATEZ_5: VariationDef = VariationDef {
    name: "inflateZ_5",
    display_name: "Inflate Z 5",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_inflateZ_5(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: Some(r#"
fn variation_inflateZ_5(p: vec3<f32>) -> vec3<f32> {
    let pi_2 = 1.5707963267948966;
    let ang1 = atan2(p.y, p.x);
    let val1 = cos(pi_2 - ang1) * 0.5;
    return vec3<f32>(0.0, 0.0, val1);
}
"#),
};

// =============================================================================
// inflateZ_6: 1.5 - acos(sin(ang) · ang · sin(y - x) · 0.5) into Z
// =============================================================================
pub static INFLATEZ_6: VariationDef = VariationDef {
    name: "inflateZ_6",
    display_name: "Inflate Z 6",
    category: VariationCategory::Depth3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_inflateZ_6(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: Some(r#"
fn variation_inflateZ_6(p: vec3<f32>) -> vec3<f32> {
    let ang = atan2(p.y, p.x);
    let adf = p.y - p.x;
    let kik = ang * sin(adf);
    let arg = clamp(sin(ang) * kik * 0.5, -1.0, 1.0);
    return vec3<f32>(0.0, 0.0, 1.5 - acos(arg));
}
"#),
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
pub static SINTRANGE: VariationDef = VariationDef {
    name: "sintrange",
    display_name: "Sin T Range",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("w", "W", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_sintrange(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w = get_param(xform_id, variation_id, 0u);
    let v = (p.x * p.x + p.y * p.y) * w;
    let fx = sin(p.x) * (p.x * p.x + w - v);
    let fy = sin(p.y) * (p.y * p.y + w - v);
    return vec2<f32>(fx, fy);
}
"#,
    wgsl_3d: Some(r#"
fn variation_sintrange(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = get_param(xform_id, variation_id, 0u);
    let v = (p.x * p.x + p.y * p.y) * w;
    let fx = sin(p.x) * (p.x * p.x + w - v);
    let fy = sin(p.y) * (p.y * p.y + w - v);
    return vec3<f32>(fx, fy, p.z);
}
"#),
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
pub static FOCI_3D: VariationDef = VariationDef {
    name: "foci_3D",
    display_name: "Foci 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
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
    wgsl_3d: Some(r#"
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
"#),
};
