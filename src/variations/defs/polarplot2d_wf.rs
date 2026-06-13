//! polarplot2d_wf (Andreas Maschke) — polar curve plot ribbon, preset
//! formulas only
//!
//! Samples a random `t ∈ [tmin, tmax]`, evaluates a polar radius formula
//! `r = f(t)`, emits `(r·cos t, r·sin t, z)` with `z` sampled
//! independently from `[zmin, zmax]` (extruding the curve into a ribbon).
//! Direct-color maps T (default) or R onto the palette; `rmin`/`rmax`
//! only normalize the R color mode.
//!
//! Preset-only port — same model and limitations as `yplot3d_wf`
//! (see that module's docs): the 16 stock presets from
//! `polarplot2d_wf_presets.txt` (spirals, roses, conchoids, lemniscates;
//! most take `param_a..c`) are baked into a WGSL switch (regenerate with
//! `scripts/gen_plot_wf_formulas.py`); custom JWF formulas render as
//! `r = 0`; colormap/displacement image resources unsupported, their
//! tuning params declared for XML parity (JWF defaults 1, 0.1, 1);
//! `preset_id` defaults to 0 instead of JWF's random pick; selecting a
//! preset does NOT auto-apply its range or `param_a..f` defaults (XML
//! imports carry explicit values).
//!
//! Sources:
//!   - `output/variation-jwf-source/plot/PolarPlot2DWFFunc.java`
//!   - `output/variation-jwf-source/plot/polarplot2d_wf_presets.txt`

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Polar curve plot extruded into a Z ribbon — samples a random angle `t`,
/// evaluates a preset radius formula `r = f(t)`, and emits
/// `(r·cos t, r·sin t, z)`. The 16 presets are JWildfire's stock polar
/// curves (Archimedes/hyperbolic/logarithmic spirals, rose curves,
/// cardioids, lemniscates; most take `param_a..c`). Custom JWF formulas
/// are not supported.
///
/// # Authors
/// - Andreas Maschke
pub static POLARPLOT2D_WF: VariationDef = VariationDef {
    name: "polarplot2d_wf",
    aliases: &[],
    display_name: "Polar-Plot 2D WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::AlwaysZ, Feature::WritesColor],
    parameters: &[
        param!("preset_id", "Preset ID", int, 0.0, -1.0, 15.0, "Which baked formula preset to plot (0-15). JWF's default is a random preset; ours is 0 for determinism. -1 means a custom JWF formula we can't evaluate — renders as r = 0."),
        param!("tmin", "T Min", unlimited_float, -3.0, -100.0, 100.0, "Lower bound of the sampled angle parameter (swapped with tmax if larger)."),
        param!("tmax", "T Max", unlimited_float, 2.0, -100.0, 100.0, "Upper bound of the sampled angle parameter."),
        param!("rmin", "R Min", unlimited_float, 0.0, -10.0, 10.0, "Lower R bound — only used to normalize the R color mode (the radius itself is unclamped)."),
        param!("rmax", "R Max", unlimited_float, 2.0, -10.0, 10.0, "Upper R bound for the R color mode."),
        param!("zmin", "Z Min", unlimited_float, -2.0, -10.0, 10.0, "Lower Z bound of the ribbon extrusion (swapped with zmax if larger)."),
        param!("zmax", "Z Max", unlimited_float, 2.0, -10.0, 10.0, "Upper Z bound of the ribbon extrusion."),
        param!("direct_color", "Direct Color", bool, true, "When on, writes the color register from the sample per Color Mode. Visible color requires the transform's Direct Color slider > 0."),
        param!("color_mode", "Color Mode", enum, 1, &["Colormap (unsupported)", "T", "R"], "Sample→palette mapping. T/R normalize the angle/radius over its range. Colormap needs an image file (unsupported) and falls back to clamping the incoming color, as JWF does with no map loaded."),
        param!("blend_colormap", "Blend Colormap", int, 1.0, 0.0, 1.0, "Unused — colormap images are unsupported. Preserved for JWF XML round-trip (JWF default 1)."),
        param!("displ_amount", "Displ Amount", unlimited_float, 0.1, -10.0, 10.0, "Unused — displacement-map images are unsupported. Preserved for JWF XML round-trip (JWF default 0.1)."),
        param!("blend_displ_map", "Blend Displ Map", int, 1.0, 0.0, 1.0, "Unused — displacement-map images are unsupported. Preserved for JWF XML round-trip (JWF default 1)."),
        param!("param_a", "Param A", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_a` — used by most presets (JWF applies each preset's own default when selected; set it manually here)."),
        param!("param_b", "Param B", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_b`."),
        param!("param_c", "Param C", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_c`."),
        param!("param_d", "Param D", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_d` — unused by the stock presets."),
        param!("param_e", "Param E", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_e` — unused by the stock presets."),
        param!("param_f", "Param F", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_f` — unused by the stock presets."),
    ],
    // 6 init slots (18..24): _tmin, _dt, _rmin, _dr, _zmin, _dz
    // (min/max swapped if reversed, matching JWF's init()).
    init_param_count: 6,
    wgsl_init: Some(r#"
fn init_polarplot2d_wf(user: array<f32, 18>) -> array<f32, 6> {
    var out: array<f32, 6>;
    var tmin = user[1];
    var tmax = user[2];
    if (tmin > tmax) { let t = tmin; tmin = tmax; tmax = t; }
    out[0] = tmin;
    out[1] = tmax - tmin;
    var rmin = user[3];
    var rmax = user[4];
    if (rmin > rmax) { let t = rmin; rmin = rmax; rmax = t; }
    out[2] = rmin;
    out[3] = rmax - rmin;
    var zmin = user[5];
    var zmax = user[6];
    if (zmin > zmax) { let t = zmin; zmin = zmax; zmax = t; }
    out[4] = zmin;
    out[5] = zmax - zmin;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
// BEGIN GENERATED FORMULAS (scripts/gen_plot_wf_formulas.py)
fn polarplot2d_wf_sqr(v: f32) -> f32 { return v * v; }

fn polarplot2d_wf_formula(id: i32, t: f32, param_a: f32, param_b: f32, param_c: f32, param_d: f32, param_e: f32, param_f: f32) -> f32 {
    let pi = 3.14159265358979;
    if (id == 0) { return t*param_a; }
    if (id == 1) { return param_a / t; }
    if (id == 2) { return cos(t*param_a/param_b) + param_c; }
    if (id == 3) { return param_b + param_a*cos(t); }
    if (id == 4) { return sqrt(polarplot2d_wf_sqr(param_a) * sin(2.0*t)); }
    if (id == 5) { return sqrt((polarplot2d_wf_sqr(param_a)*polarplot2d_wf_sqr(sin(t)) - polarplot2d_wf_sqr(param_b)*polarplot2d_wf_sqr(cos(t))) / (polarplot2d_wf_sqr(sin(t)) - polarplot2d_wf_sqr(cos(t)))); }
    if (id == 6) { return t * cos(param_a*t); }
    if (id == 7) { return sqrt(polarplot2d_wf_sqr(param_a)/t); }
    if (id == 8) { return sqrt(4.0*param_b*(param_a - param_b*polarplot2d_wf_sqr(sin(t)))); }
    if (id == 9) { return param_a * t + param_b; }
    if (id == 10) { return cos(t) * (4.0*param_a*polarplot2d_wf_sqr(sin(t)) - param_b); }
    if (id == 11) { return param_a * exp(param_b * t); }
    if (id == 12) { return sqrt(polarplot2d_wf_sqr(param_a)*t); }
    if (id == 13) { return pow(sin(t),param_a) + pow(cos(t),param_b); }
    if (id == 14) { return param_a * sin(t) / t; }
    if (id == 15) { return param_a + tanh(param_b * sin(param_c*t))/param_b; }
    return 0.0;
}
// END GENERATED FORMULAS

fn variation_polarplot2d_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let preset_id = i32(get_param(xform_id, variation_id, 0u));
    let direct_color = i32(get_param(xform_id, variation_id, 7u));
    let color_mode = i32(get_param(xform_id, variation_id, 8u));
    let pa = get_param(xform_id, variation_id, 12u);
    let pb = get_param(xform_id, variation_id, 13u);
    let pc = get_param(xform_id, variation_id, 14u);
    let pd = get_param(xform_id, variation_id, 15u);
    let pe = get_param(xform_id, variation_id, 16u);
    let pf = get_param(xform_id, variation_id, 17u);
    let tmin_s = get_param(xform_id, variation_id, 18u);
    let dt = get_param(xform_id, variation_id, 19u);
    let rmin_s = get_param(xform_id, variation_id, 20u);
    let dr = get_param(xform_id, variation_id, 21u);
    let zmin_s = get_param(xform_id, variation_id, 22u);
    let dz = get_param(xform_id, variation_id, 23u);

    let rand_u = rng_nextf(rng);
    let rand_v = rng_nextf(rng);
    let t = tmin_s + rand_u * dt;
    // z is sampled in 2D mode too (RNG parity with JWF) but unused.
    let z = zmin_s + rand_v * dz;
    let r = polarplot2d_wf_formula(preset_id, t, pa, pb, pc, pd, pe, pf);
    let x = r * cos(t);
    let y = r * sin(t);

    if (direct_color > 0) {
        var c = *vc;
        if (color_mode == 2) {
            c = (r - rmin_s) / dr;
        } else if (color_mode != 0) {
            // CM_T (1) and any other value: JWF's default case.
            c = (t - tmin_s) / dt;
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
fn polarplot2d_wf_sqr(v: f32) -> f32 { return v * v; }

fn polarplot2d_wf_formula(id: i32, t: f32, param_a: f32, param_b: f32, param_c: f32, param_d: f32, param_e: f32, param_f: f32) -> f32 {
    let pi = 3.14159265358979;
    if (id == 0) { return t*param_a; }
    if (id == 1) { return param_a / t; }
    if (id == 2) { return cos(t*param_a/param_b) + param_c; }
    if (id == 3) { return param_b + param_a*cos(t); }
    if (id == 4) { return sqrt(polarplot2d_wf_sqr(param_a) * sin(2.0*t)); }
    if (id == 5) { return sqrt((polarplot2d_wf_sqr(param_a)*polarplot2d_wf_sqr(sin(t)) - polarplot2d_wf_sqr(param_b)*polarplot2d_wf_sqr(cos(t))) / (polarplot2d_wf_sqr(sin(t)) - polarplot2d_wf_sqr(cos(t)))); }
    if (id == 6) { return t * cos(param_a*t); }
    if (id == 7) { return sqrt(polarplot2d_wf_sqr(param_a)/t); }
    if (id == 8) { return sqrt(4.0*param_b*(param_a - param_b*polarplot2d_wf_sqr(sin(t)))); }
    if (id == 9) { return param_a * t + param_b; }
    if (id == 10) { return cos(t) * (4.0*param_a*polarplot2d_wf_sqr(sin(t)) - param_b); }
    if (id == 11) { return param_a * exp(param_b * t); }
    if (id == 12) { return sqrt(polarplot2d_wf_sqr(param_a)*t); }
    if (id == 13) { return pow(sin(t),param_a) + pow(cos(t),param_b); }
    if (id == 14) { return param_a * sin(t) / t; }
    if (id == 15) { return param_a + tanh(param_b * sin(param_c*t))/param_b; }
    return 0.0;
}
// END GENERATED FORMULAS

fn variation_polarplot2d_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let preset_id = i32(get_param(xform_id, variation_id, 0u));
    let direct_color = i32(get_param(xform_id, variation_id, 7u));
    let color_mode = i32(get_param(xform_id, variation_id, 8u));
    let pa = get_param(xform_id, variation_id, 12u);
    let pb = get_param(xform_id, variation_id, 13u);
    let pc = get_param(xform_id, variation_id, 14u);
    let pd = get_param(xform_id, variation_id, 15u);
    let pe = get_param(xform_id, variation_id, 16u);
    let pf = get_param(xform_id, variation_id, 17u);
    let tmin_s = get_param(xform_id, variation_id, 18u);
    let dt = get_param(xform_id, variation_id, 19u);
    let rmin_s = get_param(xform_id, variation_id, 20u);
    let dr = get_param(xform_id, variation_id, 21u);
    let zmin_s = get_param(xform_id, variation_id, 22u);
    let dz = get_param(xform_id, variation_id, 23u);

    let rand_u = rng_nextf(rng);
    let rand_v = rng_nextf(rng);
    let t = tmin_s + rand_u * dt;
    let z = zmin_s + rand_v * dz;
    let r = polarplot2d_wf_formula(preset_id, t, pa, pb, pc, pd, pe, pf);
    let x = r * cos(t);
    let y = r * sin(t);

    if (direct_color > 0) {
        var c = *vc;
        if (color_mode == 2) {
            c = (r - rmin_s) / dr;
        } else if (color_mode != 0) {
            // CM_T (1) and any other value: JWF's default case.
            c = (t - tmin_s) / dt;
        }
        // Mode 0 (colormap, unsupported): clamp the incoming color only —
        // exactly JWF's behavior when no colormap image is loaded.
        *vc = clamp(c, 0.0, 1.0);
    }

    return vec3<f32>(x, y, z);
}
"#,
};
