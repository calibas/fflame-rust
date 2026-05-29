//! Radial-style variations: onion + target_sp
//!
//! Both upstream variations write `FPx += stuff` *without* multiplying
//! `stuff` by VVAR, so the X/Y output is weight-independent in the cpp
//! semantics (only Z preserve, where present, scales with weight). Our
//! pipeline always multiplies the variation's return by the per-variation
//! weight in the outer dispatcher, so we read the weight via
//! `needs_transform: true` and divide it out — outer multiplier then
//! restores the unscaled cpp output.
//!
//! Sources:
//!   - output/jwildfire-vars/output/onion.cpp
//!   - output/jwildfire-vars/output/target_sp.cpp

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// onion: chronologicaldot's onion warp (3D, weight-independent X/Y)
//   r0 = VVAR (treats the variation weight as the onion radius)
//   d0 = (x − cx)² + (y − cy)²,  dr = sqrt(d0)
//   if d0 ≤ r0²:                 stay flat, push z down to bottom of sphere
//   else if dr − r0 small:        curl onto top of sphere (smooth slope=1 join)
//   else:                         exponential tail past the join point
//
//   FPx += x1 + cx;  FPy += y1 + cy;  FPz += z1 + FTz
//   (note: NO VVAR multiplier on FPx/FPy/FPz lines in upstream — see module
//   header comment for the divide-out pattern.)
// =============================================================================
/// Onion-warp 3D variation — points inside a sphere (sized by the
/// variation's weight) stay flat with Z pushed to the bottom; outside, they
/// curl onto the sphere's top with an exponential tail. Forms onion-layer
/// structures.
///
/// # Authors
/// - chronologicaldot
pub static ONION: VariationDef = VariationDef {
    name: "onion",
    aliases: &[],
    display_name: "Onion",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("centre_x", "Centre X", unlimited_float, 0.0, -5.0, 5.0, "X coordinate of the sphere center."),
        param!("centre_y", "Centre Y", unlimited_float, 0.0, -5.0, 5.0, "Y coordinate of the sphere center."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    // 2D form: drop the z curl, keep only the X/Y mapping (which is the
    // identity inside the sphere and a radial squeeze outside).
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_onion(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);
    let w_raw = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w_raw, 1e-30, abs(w_raw) < 1e-30);
    var r0 = w_raw;
    if (r0 == 0.0) { r0 = 1.0; }

    let x0 = p.x - cx;
    let y0 = p.y - cy;
    let d0 = x0 * x0 + y0 * y0;
    let dr = sqrt(d0);
    let inv_sqrt2 = 0.7071067811865476;

    var x1: f32 = 0.0;
    var y1: f32 = 0.0;
    if (d0 <= r0 * r0) {
        x1 = x0;
        y1 = y0;
    } else {
        let radial = (2.0 * r0 - dr) / max(dr, 1e-30);
        x1 = radial * x0;
        y1 = radial * y0;
    }

    return vec2<f32>((x1 + cx) * inv_w, (y1 + cy) * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_onion(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let cx = get_param(xform_id, variation_id, 0u);
    let cy = get_param(xform_id, variation_id, 1u);
    let w_raw = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w_raw, 1e-30, abs(w_raw) < 1e-30);
    var r0 = w_raw;
    if (r0 == 0.0) { r0 = 1.0; }

    let x0 = p.x - cx;
    let y0 = p.y - cy;
    let d0 = x0 * x0 + y0 * y0;
    let dr = sqrt(d0);
    let inv_sqrt2 = 0.7071067811865476;
    let join = r0 * inv_sqrt2;

    var x1: f32 = 0.0;
    var y1: f32 = 0.0;
    var z1: f32 = 0.0;
    if (d0 <= r0 * r0) {
        z1 = -sqrt(max(r0 * r0 - d0, 0.0));
        x1 = x0;
        y1 = y0;
    } else if (2.0 * r0 - dr > join) {
        let radial = (2.0 * r0 - dr) / max(dr, 1e-30);
        x1 = radial * x0;
        y1 = radial * y0;
        z1 = sqrt(max(r0 * r0 - (x1 * x1 + y1 * y1), 0.0));
    } else {
        z1 = exp(dr - r0 - (r0 - join)) - 1.0 + join;
        let radial = (2.0 * r0 - dr) / max(dr, 1e-30);
        x1 = radial * x0;
        y1 = radial * y0;
    }

    return vec3<f32>((x1 + cx) * inv_w, (y1 + cy) * inv_w, (z1 + p.z) * inv_w);
}
"#),
};

// =============================================================================
// target_sp: log-spiral target (Michael Faber + Dark-Beam tweak)
//   a = atan2(y, x);  r = sqrt(x² + y²)
//   t = tightness · log(r) + n_of_sp · (a + π) / π
//   if t < 0:    t -= t_size_2
//   t = |t| mod size
//   if t < t_size_2:  a += _rota   (rota = π · twist)
//   else:              a += _rotb  (rotb = -π + rota)
//   FPx += r · cos(a);  FPy += r · sin(a)   (no VVAR multiplier — same
//                                            divide-out pattern as `onion`)
// =============================================================================
/// Log-spiral target — splits the plane into log-spaced spiral arms and
/// rotates each arm by `twist`. `n_of_sp` controls how many spirals
/// interleave; `tightness` controls how rapidly they wind.
///
/// # Authors
/// - Michael Faber
/// - DarkBeam
pub static TARGET_SP: VariationDef = VariationDef {
    name: "target_sp",
    aliases: &[],
    display_name: "Target Sp",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("twist", "Twist", unlimited_float, 0.0, -5.0, 5.0, "Rotation applied to alternating spiral arms, in half-turns."),
        param!("n_of_sp", "N of Spirals", int, 1.0, -20.0, 20.0, "Number of interleaved spiral arms."),
        param!("size", "Size", unlimited_float, 1.0, 0.01, 10.0, "Width of each spiral band in log-radius space."),
        param!("tightness", "Tightness", unlimited_float, 0.5, -5.0, 5.0, "Logarithmic winding rate — higher = arms wind tighter."),
    ],
    needs_transform: true,
    writes_color: false,
    // 3 derived values at slots 4..7:
    //   4: t_size_2  (0.5 · size)
    //   5: rota      (π · twist)
    //   6: rotb      (-π + π · twist)
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_target_sp(user: array<f32, 4>) -> array<f32, 3> {
    let twist = user[0];
    let size = user[2];
    let pi = 3.14159265358979;
    let rota = pi * twist;
    var out: array<f32, 3>;
    out[0] = 0.5 * size;
    out[1] = rota;
    out[2] = -pi + rota;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_target_sp(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let n_of_sp = get_param(xform_id, variation_id, 1u);
    let size = max(get_param(xform_id, variation_id, 2u), 1e-6);
    let tightness = get_param(xform_id, variation_id, 3u);
    let t_size_2 = get_param(xform_id, variation_id, 4u);
    let rota = get_param(xform_id, variation_id, 5u);
    let rotb = get_param(xform_id, variation_id, 6u);
    let inv_pi = 0.3183098861837907;
    let pi = 3.14159265358979;

    let w_raw = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w_raw, 1e-30, abs(w_raw) < 1e-30);

    var a = atan2(p.y, p.x);
    let r = sqrt(p.x * p.x + p.y * p.y);
    var t = tightness * log(max(r, 1e-30)) + n_of_sp * inv_pi * (a + pi);
    if (t < 0.0) {
        t = t - t_size_2;
    }
    t = abs(t) - floor(abs(t) / size) * size;
    if (t < t_size_2) {
        a = a + rota;
    } else {
        a = a + rotb;
    }
    return vec2<f32>(r * cos(a) * inv_w, r * sin(a) * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_target_sp(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let n_of_sp = get_param(xform_id, variation_id, 1u);
    let size = max(get_param(xform_id, variation_id, 2u), 1e-6);
    let tightness = get_param(xform_id, variation_id, 3u);
    let t_size_2 = get_param(xform_id, variation_id, 4u);
    let rota = get_param(xform_id, variation_id, 5u);
    let rotb = get_param(xform_id, variation_id, 6u);
    let inv_pi = 0.3183098861837907;
    let pi = 3.14159265358979;

    let w_raw = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w_raw, 1e-30, abs(w_raw) < 1e-30);

    var a = atan2(p.y, p.x);
    let r = sqrt(p.x * p.x + p.y * p.y);
    var t = tightness * log(max(r, 1e-30)) + n_of_sp * inv_pi * (a + pi);
    if (t < 0.0) {
        t = t - t_size_2;
    }
    t = abs(t) - floor(abs(t) / size) * size;
    if (t < t_size_2) {
        a = a + rota;
    } else {
        a = a + rotb;
    }
    // Z preserve scales with weight (FPz += VVAR · FTz upstream); leave
    // p.z untouched here so the outer multiplier produces VVAR · p.z.
    return vec3<f32>(r * cos(a) * inv_w, r * sin(a) * inv_w, p.z);
}
"#),
};
