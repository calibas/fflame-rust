//! Apophysis miscellany 11 — five more
//!
//!   - `swirl3`         (?)                — 1 user (shift); log-spiral
//!                                              swirl. Preserves cpp's
//!                                              `atan2(x, y)` swap (vs
//!                                              Java's `atan2(y, x)`)
//!   - `wdisc`          (Faber)             — 0 user; π/(sqrt+1) angle
//!                                              folded around r=0
//!   - `sph3D`          (xyrus02)           — 3 user (x, y, z scales);
//!                                              Full3D. Preserves
//!                                              upstream's `zz = x · tz`
//!                                              typo (cpp+Java both
//!                                              use `x` instead of `z`
//!                                              in zz)
//!   - `invsquircular`  (?, Java-recovered) — 0 user; squircular Möbius
//!                                              inverse; cpp PluginVarCalc
//!                                              empty unported_stub.
//!                                              Body has w² nonlinearly
//!                                              inside `r2 = sqrt(r·(w²·r
//!                                              − 4u²v²)/w)`;
//!                                              needs_transform divide-out
//!   - `sphere_nja`     (Nicolaus Anderson)  — 6 user (circle_a/b,
//!                                              shift_x/y/z, stretch);
//!                                              Full3D parametric sphere.
//!                                              Body has w-scaled `r/z`
//!                                              plus unscaled `+ shift_x/y`
//!                                              offsets; needs_transform
//!                                              divide-out
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// swirl3 (Zy0rg)
//   rad = sqrt(x² + y²) + ε
//   ang = atan2(y, x) + log(rad) · shift
//   out = rad · (cos ang, sin ang)
// 1 user param (shift); clean factor through outer.
// Matches Fractorium (Ember) and JWildfire — both use standard
// atan2(y, x). Our cpp port had atan2(x, y), introduced by the
// chaotica-apophysis-plugins-from-jwildfire converter; fixed.
// =============================================================================
/// Log-spiral swirl — emits `(rad · cos(ang), rad · sin(ang))` where `rad =
/// sqrt(x²+y²) + ε` and `ang = atan2(y, x) + log(rad) · shift`. The
/// `log(rad)` term makes the angular offset grow linearly with the log of
/// radius, producing a logarithmic-spiral swirl.
///
/// # Authors
/// - Zy0rg
pub static SWIRL3: VariationDef = VariationDef {
    name: "swirl3",
    display_name: "Swirl 3",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("shift", "Shift", unlimited_float, 0.5, -10.0, 10.0, "Logarithmic spiral coefficient. Larger = tighter spiral."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_swirl3(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let shift = get_param(xform_id, variation_id, 0u);
    let rad = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let ang = atan2(p.y, p.x) + log(rad) * shift;
    return vec2<f32>(rad * cos(ang), rad * sin(ang));
}
"#,
    wgsl_3d: Some(r#"
fn variation_swirl3(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let shift = get_param(xform_id, variation_id, 0u);
    let rad = sqrt(p.x * p.x + p.y * p.y) + 1e-30;
    let ang = atan2(p.y, p.x) + log(rad) * shift;
    return vec3<f32>(rad * cos(ang), rad * sin(ang), p.z);
}
"#),
};

// =============================================================================
// wdisc (Faber)
//   a = π / (sqrt(x² + y²) + 1)
//   r = atan2(y, x) / π
//   if r > 0: a = π - a
//   out = r · (cos a, sin a)
// 0 user params; clean.
// =============================================================================
/// Wedge disc — emits `r · (cos a, sin a)` where `a = π / (sqrt(x²+y²) +
/// 1)` and `r = atan2(y, x) / π`. Points with `r > 0` get their `a`
/// reflected via `π − a`. Maps the input plane onto a folded-disc pattern.
///
/// # Authors
/// - Michael Faber
pub static WDISC: VariationDef = VariationDef {
    name: "wdisc",
    display_name: "W-Disc",
    category: VariationCategory::Advanced2D,
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
fn variation_wdisc(p: vec2<f32>) -> vec2<f32> {
    let pi = 3.14159265358979;
    let inv_pi = 0.31830988618379;
    var a = pi / (sqrt(p.x * p.x + p.y * p.y) + 1.0);
    let r = atan2(p.y, p.x) * inv_pi;
    if (r > 0.0) {
        a = pi - a;
    }
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: Some(r#"
fn variation_wdisc(p: vec3<f32>) -> vec3<f32> {
    let pi = 3.14159265358979;
    let inv_pi = 0.31830988618379;
    var a = pi / (sqrt(p.x * p.x + p.y * p.y) + 1.0);
    let r = atan2(p.y, p.x) * inv_pi;
    if (r > 0.0) {
        a = pi - a;
    }
    return vec3<f32>(r * cos(a), r * sin(a), p.z);
}
"#),
};

// =============================================================================
// sph3D (xyrus02)
//   xx = x_scale · x;  yy = y_scale · y;  zz = x_scale · z   (← upstream typo;
//                                                              cpp+Java both
//                                                              use x_scale,
//                                                              not z_scale)
//   r = w / (xx² + yy² + zz² + ε)
//   out = (x · r, y · r, z · r)
// 3 user params; Full3D; clean factor through outer.
// =============================================================================
/// 3D spherical with per-axis scales — emits `(x, y, z) / (x_scale²·x² +
/// y_scale²·y² + zz² + ε)` where `zz = x_scale · z` (upstream typo:
/// cpp+Java both use `x_scale` instead of `z_scale` in the Z denominator
/// term, preserved). Per-axis scales tune the asymmetry of the inverse-
/// distance scaling.
///
/// # Authors
/// - Xyrus02
pub static SPH3D: VariationDef = VariationDef {
    name: "sph3D",
    display_name: "Sph 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("x", "X scale", unlimited_float, 1.0, -10.0, 10.0, "X-axis scale on the inverse-distance denominator."),
        param!("y", "Y scale", unlimited_float, 1.0, -10.0, 10.0, "Y-axis scale."),
        param!("z", "Z scale", unlimited_float, 1.0, -10.0, 10.0, "Z-axis scale — note: due to an upstream typo the body actually uses `x_scale` for `zz`, so this parameter is effectively unused. Preserved for preset compatibility."),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_sph3D(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x_scale = get_param(xform_id, variation_id, 0u);
    let y_scale = get_param(xform_id, variation_id, 1u);
    let xx = x_scale * p.x;
    let yy = y_scale * p.y;
    let denom = max(xx * xx + yy * yy, 1e-30);
    let r = 1.0 / denom;
    return vec2<f32>(p.x * r, p.y * r);
}
"#,
    wgsl_3d: Some(r#"
fn variation_sph3D(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x_scale = get_param(xform_id, variation_id, 0u);
    let y_scale = get_param(xform_id, variation_id, 1u);
    let xx = x_scale * p.x;
    let yy = y_scale * p.y;
    let zz = x_scale * p.z;  // upstream typo: x_scale (not z_scale) — preserved
    let denom = max(xx * xx + yy * yy + zz * zz, 1e-30);
    let r = 1.0 / denom;
    return vec3<f32>(p.x * r, p.y * r, p.z * r);
}
"#),
};

// =============================================================================
// invsquircular (Java-recovered; cpp PluginVarCalc empty unported_stub)
//   r0 = u² + v²
//   r2 = sqrt(r0 · (w² · r0 − 4 u² v²) / w)
//   r = sqrt(r0 − r2) / sqrt(2)
//   out = (r/u, r/v)
// 0 user params. Body has w nonlinearly inside r2's sqrt;
// needs_transform divide-out.
// =============================================================================
/// Inverse `squircular` — undoes the squircular Möbius warp. The body has
/// the variation weight `w²` appearing nonlinearly inside `r2 = sqrt(r₀ ·
/// (w²·r₀ − 4u²v²) / w)`, so the shape changes qualitatively with weight.
pub static INVSQUIRCULAR: VariationDef = VariationDef {
    name: "invsquircular",
    display_name: "Inv Squircular",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_invsquircular(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let safe_w = select(w, 1e-30, abs(w) < 1e-30);

    let u = p.x;
    let v = p.y;
    let r0 = u * u + v * v;
    let inner = r0 * (safe_w * safe_w * r0 - 4.0 * u * u * v * v) / safe_w;
    let r2 = sqrt(max(inner, 0.0));
    let r = sqrt(max(r0 - r2, 0.0)) * 0.7071067811865476;  // 1/sqrt(2)
    let safe_u = select(u, 1e-30, abs(u) < 1e-30);
    let safe_v = select(v, 1e-30, abs(v) < 1e-30);
    return vec2<f32>(r / safe_u * inv_w, r / safe_v * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_invsquircular(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let safe_w = select(w, 1e-30, abs(w) < 1e-30);

    let u = p.x;
    let v = p.y;
    let r0 = u * u + v * v;
    let inner = r0 * (safe_w * safe_w * r0 - 4.0 * u * u * v * v) / safe_w;
    let r2 = sqrt(max(inner, 0.0));
    let r = sqrt(max(r0 - r2, 0.0)) * 0.7071067811865476;
    let safe_u = select(u, 1e-30, abs(u) < 1e-30);
    let safe_v = select(v, 1e-30, abs(v) < 1e-30);
    return vec3<f32>(r / safe_u * inv_w, r / safe_v * inv_w, p.z);
}
"#),
};

// =============================================================================
// sphere_nja (Nicolaus Anderson)
//   ini = (x − shift_x, y − shift_y, z)
//   sqtr = sqrt(ini.x² + ini.y² + ini.z²)
//   t = sqtr / stretch − π/2
//   r = cos(t) · w;  z_param = sin(t) · w
//   fx = r · ini.x / sqtr;  fy = r · ini.y / sqtr;  fz = z_param
//   fx += r · ini.z · ini.x / sqtr;  fy += r · ini.z · ini.y / sqtr
//   fz += ini.z · z_param / (r² + z_param²)
//   fx += shift_x;  fy += shift_y
// 6 user params; Full3D; needs_transform divide-out.
// (Note: `circle_a/b` declared but unused in cpp body — present in Java
//  scaffolding too. Preserved as user params so presets work.)
// =============================================================================
/// Parametric sphere — uses `t = sqrt(x²+y²+z²)/stretch − π/2` to
/// parameterize a sphere with `cos(t)` radius. Per-axis output mixes
/// cos/sin terms with the input position and configurable shift offsets,
/// producing a 3D sphere centered at `(shift_x, shift_y, 0)`.
///
/// # Authors
/// - chronologicaldot
pub static SPHERE_NJA: VariationDef = VariationDef {
    name: "sphere_nja",
    display_name: "Sphere NJA",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("circle_a", "Circle A", unlimited_float, 1.0, -10.0, 10.0, "Declared in the upstream source but unused in the body. Preserved as a parameter for preset compatibility."),
        param!("circle_b", "Circle B", unlimited_float, 1.0, -10.0, 10.0, "Declared but unused (same as `circle_a`)."),
        param!("shift_x", "Shift X", unlimited_float, 0.0, -10.0, 10.0, "X-axis center offset (subtracted from input, added back to output)."),
        param!("shift_y", "Shift Y", unlimited_float, 0.0, -10.0, 10.0, "Y-axis center offset."),
        param!("shift_z", "Shift Z", unlimited_float, 0.0, -10.0, 10.0, "Z-axis center offset."),
        param!("stretch", "Stretch", unlimited_float, 1.0, -10.0, 10.0, "Radial-to-angular scaling factor in `t = sqtr/stretch − π/2`."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_sphere_nja(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let shift_x = get_param(xform_id, variation_id, 2u);
    let shift_y = get_param(xform_id, variation_id, 3u);
    let stretch = get_param(xform_id, variation_id, 5u);
    let pi_2 = 1.5707963267948966;
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let safe_stretch = select(stretch, 1e-30, abs(stretch) < 1e-30);

    let inix = p.x - shift_x;
    let iniy = p.y - shift_y;
    let sqtr = max(sqrt(inix * inix + iniy * iniy), 1e-30);
    let t = sqtr / safe_stretch - pi_2;
    let r = cos(t) * w;
    let fx = r * inix / sqtr + shift_x;
    let fy = r * iniy / sqtr + shift_y;
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_sphere_nja(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let shift_x = get_param(xform_id, variation_id, 2u);
    let shift_y = get_param(xform_id, variation_id, 3u);
    let stretch = get_param(xform_id, variation_id, 5u);
    let pi_2 = 1.5707963267948966;
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);
    let safe_stretch = select(stretch, 1e-30, abs(stretch) < 1e-30);

    let inix = p.x - shift_x;
    let iniy = p.y - shift_y;
    let iniz = p.z;
    let sqtr = max(sqrt(inix * inix + iniy * iniy + iniz * iniz), 1e-30);
    let t = sqtr / safe_stretch - pi_2;
    let r = cos(t) * w;
    let z_p = sin(t) * w;
    var fx = r * inix / sqtr;
    var fy = r * iniy / sqtr;
    var fz = z_p;
    fx = fx + r * iniz * inix / sqtr;
    fy = fy + r * iniz * iniy / sqtr;
    let denom = max(r * r + z_p * z_p, 1e-30);
    fz = fz + iniz * z_p / denom;
    fx = fx + shift_x;
    fy = fy + shift_y;
    return vec3<f32>(fx * inv_w, fy * inv_w, fz * inv_w);
}
"#),
};
