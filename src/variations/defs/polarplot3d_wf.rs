//! polarplot3d_wf (Andreas Maschke) — 3D polar surface plot, preset
//! formulas only
//!
//! Samples random `t ∈ [tmin, tmax]` and `u ∈ [umin, umax]`, evaluates a
//! radius formula `r = f(t, u)`, and emits either spherical coordinates
//! (`cylindrical = 0`, the default: `x = r·sin u·cos t, y = r·sin u·sin t,
//! z = r·cos u`) or cylindrical (`x = r·cos t, y = r·sin t, z = u`).
//! Direct-color maps t/u/r (or t·u) onto the palette; `rmin`/`rmax` only
//! normalize the R color mode.
//!
//! Preset-only port — same model and limitations as `yplot3d_wf`
//! (see that module's docs): the 10 stock presets from
//! `polarplot3d_wf_presets.txt` are baked into a WGSL switch (regenerate
//! with `scripts/gen_plot_wf_formulas.py`); custom JWF formulas render
//! as `r = 0`; colormap/displacement image resources unsupported, their
//! tuning params declared for XML parity (JWF defaults 1, 0.1, 1);
//! `preset_id` defaults to 0 instead of JWF's random pick; selecting a
//! preset does NOT auto-apply its range or `param_a..f` defaults (XML
//! imports carry explicit values).
//!
//! Sources:
//!   - `output/variation-jwf-source/plot/PolarPlot3DWFFunc.java`
//!   - `output/variation-jwf-source/plot/polarplot3d_wf_presets.txt`

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// 3D polar surface plot — samples random angles `(t, u)`, evaluates a
/// preset radius formula `r = f(t, u)`, and emits the point in spherical
/// (default) or cylindrical coordinates. The 10 presets are JWildfire's
/// stock polar surfaces (all take `param_a..c`). Custom JWF formulas are
/// not supported.
///
/// # Authors
/// - Andreas Maschke
pub static POLARPLOT3D_WF: VariationDef = VariationDef {
    name: "polarplot3d_wf",
    aliases: &[],
    display_name: "Polar-Plot 3D WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::AlwaysZ, Feature::WritesColor],
    parameters: &[
        param!("preset_id", "Preset ID", int, 0.0, -1.0, 9.0, "Which baked formula preset to plot (0-9). JWF's default is a random preset; ours is 0 for determinism. -1 means a custom JWF formula we can't evaluate — renders as r = 0."),
        param!("tmin", "T Min", unlimited_float, -3.141592653589793, -100.0, 100.0, "Lower bound of the azimuthal parameter t (swapped with tmax if larger). JWF default -π."),
        param!("tmax", "T Max", unlimited_float, 3.141592653589793, -100.0, 100.0, "Upper bound of the azimuthal parameter t. JWF default π."),
        param!("umin", "U Min", unlimited_float, 0.0, -100.0, 100.0, "Lower bound of the polar parameter u."),
        param!("umax", "U Max", unlimited_float, 3.141592653589793, -100.0, 100.0, "Upper bound of the polar parameter u. JWF default π."),
        param!("rmin", "R Min", unlimited_float, -2.0, -10.0, 10.0, "Lower R bound — only used to normalize the R color mode (the radius itself is unclamped)."),
        param!("rmax", "R Max", unlimited_float, 2.0, -10.0, 10.0, "Upper R bound for the R color mode."),
        param!("cylindrical", "Cylindrical", bool, false, "When off (JWF default), spherical mapping: `(r·sin u·cos t, r·sin u·sin t, r·cos u)`. When on, cylindrical: `(r·cos t, r·sin t, u)`."),
        param!("direct_color", "Direct Color", bool, true, "When on, writes the color register from the sample per Color Mode. Visible color requires the transform's Direct Color slider > 0."),
        param!("color_mode", "Color Mode", enum, 2, &["Colormap (unsupported)", "T", "U", "R", "T×U"], "Sample→palette mapping. T/U/R normalize the corresponding value over its range; T×U multiplies the two angles. Colormap needs an image file (unsupported) and falls back to clamping the incoming color, as JWF does with no map loaded."),
        param!("blend_colormap", "Blend Colormap", int, 1.0, 0.0, 1.0, "Unused — colormap images are unsupported. Preserved for JWF XML round-trip (JWF default 1)."),
        param!("displ_amount", "Displ Amount", unlimited_float, 0.1, -10.0, 10.0, "Unused — displacement-map images are unsupported. Preserved for JWF XML round-trip (JWF default 0.1)."),
        param!("blend_displ_map", "Blend Displ Map", int, 1.0, 0.0, 1.0, "Unused — displacement-map images are unsupported. Preserved for JWF XML round-trip (JWF default 1)."),
        param!("param_a", "Param A", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_a` — used by all presets (JWF applies each preset's own default when selected; set it manually here)."),
        param!("param_b", "Param B", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_b`."),
        param!("param_c", "Param C", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_c`."),
        param!("param_d", "Param D", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_d` — unused by the stock presets."),
        param!("param_e", "Param E", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_e` — unused by the stock presets."),
        param!("param_f", "Param F", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_f` — unused by the stock presets."),
    ],
    // 6 init slots (19..25): _tmin, _dt, _umin, _du, _rmin, _dr
    // (min/max swapped if reversed, matching JWF's init()).
    init_param_count: 6,
    wgsl_init: Some(r#"
fn init_polarplot3d_wf(user: array<f32, 19>) -> array<f32, 6> {
    var out: array<f32, 6>;
    var tmin = user[1];
    var tmax = user[2];
    if (tmin > tmax) { let t = tmin; tmin = tmax; tmax = t; }
    out[0] = tmin;
    out[1] = tmax - tmin;
    var umin = user[3];
    var umax = user[4];
    if (umin > umax) { let t = umin; umin = umax; umax = t; }
    out[2] = umin;
    out[3] = umax - umin;
    var rmin = user[5];
    var rmax = user[6];
    if (rmin > rmax) { let t = rmin; rmin = rmax; rmax = t; }
    out[4] = rmin;
    out[5] = rmax - rmin;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
// BEGIN GENERATED FORMULAS (scripts/gen_plot_wf_formulas.py)
fn polarplot3d_wf_sqr(v: f32) -> f32 { return v * v; }

fn polarplot3d_wf_formula(id: i32, t: f32, u: f32, param_a: f32, param_b: f32, param_c: f32, param_d: f32, param_e: f32, param_f: f32) -> f32 {
    let pi = 3.14159265358979;
    if (id == 0) { return sin(param_a*u) + param_b; }
    if (id == 1) { return sin(param_a*u) + param_b; }
    if (id == 2) { return u; }
    if (id == 3) { return (sin(param_a*t) + cos(param_b*u)) + param_c; }
    if (id == 4) { return (sin(param_a*t + param_b*u)) + param_c; }
    if (id == 5) { return (sin(param_a*t + param_b*u)) + param_c; }
    if (id == 6) { return sin(param_a*t)+param_b*u; }
    if (id == 7) { return t / (param_a + u) + param_b; }
    if (id == 8) { return polarplot3d_wf_sqr(u) + param_a; }
    if (id == 9) { return cos(param_a*t + sin(param_b*u)) + param_c; }
    return 0.0;
}
// END GENERATED FORMULAS

fn variation_polarplot3d_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let preset_id = i32(get_param(xform_id, variation_id, 0u));
    let cylindrical = i32(get_param(xform_id, variation_id, 7u));
    let direct_color = i32(get_param(xform_id, variation_id, 8u));
    let color_mode = i32(get_param(xform_id, variation_id, 9u));
    let pa = get_param(xform_id, variation_id, 13u);
    let pb = get_param(xform_id, variation_id, 14u);
    let pc = get_param(xform_id, variation_id, 15u);
    let pd = get_param(xform_id, variation_id, 16u);
    let pe = get_param(xform_id, variation_id, 17u);
    let pf = get_param(xform_id, variation_id, 18u);
    let tmin_s = get_param(xform_id, variation_id, 19u);
    let dt = get_param(xform_id, variation_id, 20u);
    let umin_s = get_param(xform_id, variation_id, 21u);
    let du = get_param(xform_id, variation_id, 22u);
    let rmin_s = get_param(xform_id, variation_id, 23u);
    let dr = get_param(xform_id, variation_id, 24u);

    let rand_t = rng_nextf(rng);
    let rand_u = rng_nextf(rng);
    let t = tmin_s + rand_t * dt;
    let u = umin_s + rand_u * du;
    let r = polarplot3d_wf_formula(preset_id, t, u, pa, pb, pc, pd, pe, pf);
    var x: f32;
    var y: f32;
    if (cylindrical == 0) {
        x = r * sin(u) * cos(t);
        y = r * sin(u) * sin(t);
    } else {
        x = r * cos(t);
        y = r * sin(t);
    }

    if (direct_color > 0) {
        var c = *vc;
        if (color_mode == 1) {
            c = (t - tmin_s) / dt;
        } else if (color_mode == 3) {
            c = (r - rmin_s) / dr;
        } else if (color_mode == 4) {
            c = (t - tmin_s) / dt * (u - umin_s) / du;
        } else if (color_mode != 0) {
            // CM_U (2) and any other value: JWF's default case.
            c = (u - umin_s) / du;
        }
        // Mode 0 (colormap, unsupported): clamp the incoming color only —
        // exactly JWF's behavior when no colormap image is loaded.
        *vc = clamp(c, 0.0, 1.0);
    }

    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
// BEGIN GENERATED FORMULAS (scripts/gen_plot_wf_formulas.py)
fn polarplot3d_wf_sqr(v: f32) -> f32 { return v * v; }

fn polarplot3d_wf_formula(id: i32, t: f32, u: f32, param_a: f32, param_b: f32, param_c: f32, param_d: f32, param_e: f32, param_f: f32) -> f32 {
    let pi = 3.14159265358979;
    if (id == 0) { return sin(param_a*u) + param_b; }
    if (id == 1) { return sin(param_a*u) + param_b; }
    if (id == 2) { return u; }
    if (id == 3) { return (sin(param_a*t) + cos(param_b*u)) + param_c; }
    if (id == 4) { return (sin(param_a*t + param_b*u)) + param_c; }
    if (id == 5) { return (sin(param_a*t + param_b*u)) + param_c; }
    if (id == 6) { return sin(param_a*t)+param_b*u; }
    if (id == 7) { return t / (param_a + u) + param_b; }
    if (id == 8) { return polarplot3d_wf_sqr(u) + param_a; }
    if (id == 9) { return cos(param_a*t + sin(param_b*u)) + param_c; }
    return 0.0;
}
// END GENERATED FORMULAS

fn variation_polarplot3d_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let preset_id = i32(get_param(xform_id, variation_id, 0u));
    let cylindrical = i32(get_param(xform_id, variation_id, 7u));
    let direct_color = i32(get_param(xform_id, variation_id, 8u));
    let color_mode = i32(get_param(xform_id, variation_id, 9u));
    let pa = get_param(xform_id, variation_id, 13u);
    let pb = get_param(xform_id, variation_id, 14u);
    let pc = get_param(xform_id, variation_id, 15u);
    let pd = get_param(xform_id, variation_id, 16u);
    let pe = get_param(xform_id, variation_id, 17u);
    let pf = get_param(xform_id, variation_id, 18u);
    let tmin_s = get_param(xform_id, variation_id, 19u);
    let dt = get_param(xform_id, variation_id, 20u);
    let umin_s = get_param(xform_id, variation_id, 21u);
    let du = get_param(xform_id, variation_id, 22u);
    let rmin_s = get_param(xform_id, variation_id, 23u);
    let dr = get_param(xform_id, variation_id, 24u);

    let rand_t = rng_nextf(rng);
    let rand_u = rng_nextf(rng);
    let t = tmin_s + rand_t * dt;
    let u = umin_s + rand_u * du;
    let r = polarplot3d_wf_formula(preset_id, t, u, pa, pb, pc, pd, pe, pf);
    var x: f32;
    var y: f32;
    var z: f32;
    if (cylindrical == 0) {
        x = r * sin(u) * cos(t);
        y = r * sin(u) * sin(t);
        z = r * cos(u);
    } else {
        x = r * cos(t);
        y = r * sin(t);
        z = u;
    }

    if (direct_color > 0) {
        var c = *vc;
        if (color_mode == 1) {
            c = (t - tmin_s) / dt;
        } else if (color_mode == 3) {
            c = (r - rmin_s) / dr;
        } else if (color_mode == 4) {
            c = (t - tmin_s) / dt * (u - umin_s) / du;
        } else if (color_mode != 0) {
            // CM_U (2) and any other value: JWF's default case.
            c = (u - umin_s) / du;
        }
        // Mode 0 (colormap, unsupported): clamp the incoming color only —
        // exactly JWF's behavior when no colormap image is loaded.
        *vc = clamp(c, 0.0, 1.0);
    }

    return vec3<f32>(x, y, z);
}
"#,
};
