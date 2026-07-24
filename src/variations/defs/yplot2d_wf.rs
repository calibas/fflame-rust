//! yplot2d_wf (Andreas Maschke) — 2D curve plot ribbon, preset formulas only
//!
//! Samples a random `x ∈ [xmin, xmax]`, evaluates a curve formula
//! `y = f(x)`, samples `z ∈ [zmin, zmax]` independently (extruding the
//! curve into a ribbon), and emits `(x, y, z)` weight-outside. Direct-color
//! maps X (default) or Y position onto the palette.
//!
//! Preset-only port — same model and limitations as `yplot3d_wf`
//! (see that module's docs): the 14 stock presets from
//! `yplot2d_wf_presets.txt` are baked into a WGSL switch
//! (regenerate with `scripts/gen_plot_wf_formulas.py`); custom JWF
//! formulas render as `y = 0`; colormap/displacement image resources
//! unsupported, their tuning params declared for XML parity (JWF
//! defaults 1, 0.1, 1); `preset_id` defaults to 0 instead of JWF's
//! random pick; selecting a preset does NOT auto-apply its
//! range/param_a..f defaults (XML imports carry explicit values).
//!
//! Unlike yplot3d, several presets here DO use `param_a`/`param_b`
//! (formulas by Rick Sidwell) — the live param values feed the formula.
//!
//! Sources:
//!   - `output/variation-jwf-source/plot/YPlot2DWFFunc.java`
//!   - `output/variation-jwf-source/plot/yplot2d_wf_presets.txt`

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// 2D curve plot extruded into a Z ribbon — samples a random `x`,
/// evaluates a preset curve formula `y = f(x)`, samples `z` independently,
/// and emits `(x, y, z)`. The 14 presets are JWildfire's stock yplot2d
/// formulas (harmonics, chirps, power curves, square/triangle waves; some
/// take `param_a`/`param_b`). Custom JWF formulas are not supported.
///
/// # Authors
/// - Andreas Maschke
/// - Rick Sidwell
pub static YPLOT2D_WF: VariationDef = VariationDef {
    name: "yplot2d_wf",
    aliases: &[],
    display_name: "Y-Plot 2D WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::AlwaysZ, Feature::WritesColor],
    parameters: &[
        param!("preset_id", "Preset ID", int, 0.0, -1.0, 13.0, "Which baked formula preset to plot (0-13). JWF's default is a random preset; ours is 0 for determinism. -1 means a custom JWF formula we can't evaluate — renders as y = 0."),
        param!("xmin", "X Min", unlimited_float, -3.0, -10.0, 10.0, "Lower X bound of the sampled interval (swapped with xmax if larger)."),
        param!("xmax", "X Max", unlimited_float, 2.0, -10.0, 10.0, "Upper X bound of the sampled interval."),
        param!("ymin", "Y Min", unlimited_float, -4.0, -10.0, 10.0, "Lower Y bound — only used to normalize the Y color mode (the curve itself is unclamped)."),
        param!("ymax", "Y Max", unlimited_float, 4.0, -10.0, 10.0, "Upper Y bound for the Y color mode."),
        param!("zmin", "Z Min", unlimited_float, -2.0, -10.0, 10.0, "Lower Z bound of the ribbon extrusion (swapped with zmax if larger)."),
        param!("zmax", "Z Max", unlimited_float, 2.0, -10.0, 10.0, "Upper Z bound of the ribbon extrusion."),
        param!("direct_color", "Direct Color", bool, true, "When on, writes the color register from the sample position per Color Mode. Visible color requires the transform's Direct Color slider > 0."),
        param!("color_mode", "Color Mode", enum, 1, &["Colormap (unsupported)", "X", "Y"], "Position→palette mapping. X/Y normalize the corresponding coordinate over its range. Colormap needs an image file (unsupported) and falls back to clamping the incoming color, as JWF does with no map loaded."),
        param!("blend_colormap", "Blend Colormap", int, 1.0, 0.0, 1.0, "Unused — colormap images are unsupported. Preserved for JWF XML round-trip (JWF default 1)."),
        param!("displ_amount", "Displ Amount", unlimited_float, 0.1, -10.0, 10.0, "Unused — displacement-map images are unsupported. Preserved for JWF XML round-trip (JWF default 0.1)."),
        param!("blend_displ_map", "Blend Displ Map", int, 1.0, 0.0, 1.0, "Unused — displacement-map images are unsupported. Preserved for JWF XML round-trip (JWF default 1)."),
        param!("param_a", "Param A", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_a` — used by presets 3, 4, 6-13 (JWF applies each preset's own default when selected; set it manually here)."),
        param!("param_b", "Param B", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_b` — used by presets 6, 8, 10-13."),
        param!("param_c", "Param C", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_c` — unused by the stock presets."),
        param!("param_d", "Param D", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_d` — unused by the stock presets."),
        param!("param_e", "Param E", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_e` — unused by the stock presets."),
        param!("param_f", "Param F", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_f` — unused by the stock presets."),
    ],
    // 6 init slots (18..24): _xmin, _dx, _ymin, _dy, _zmin, _dz
    // (min/max swapped if reversed, matching JWF's init()).
    init_param_count: 6,
    wgsl_init: Some(r#"
fn init_yplot2d_wf(user: array<f32, 18>) -> array<f32, 6> {
    var out: array<f32, 6>;
    var xmin = user[1];
    var xmax = user[2];
    if (xmin > xmax) { let t = xmin; xmin = xmax; xmax = t; }
    out[0] = xmin;
    out[1] = xmax - xmin;
    var ymin = user[3];
    var ymax = user[4];
    if (ymin > ymax) { let t = ymin; ymin = ymax; ymax = t; }
    out[2] = ymin;
    out[3] = ymax - ymin;
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
fn yplot2d_wf_sqr(v: f32) -> f32 { return v * v; }

fn yplot2d_wf_formula(id: i32, x: f32, param_a: f32, param_b: f32, param_c: f32, param_d: f32, param_e: f32, param_f: f32) -> f32 {
    let pi = 3.14159265358979;
    if (id == 0) { return (sin(x)+2.0*sin(2.0*x)+1.0*sin(4.0*x)); }
    if (id == 1) { return sin(x)*cos(x); }
    if (id == 2) { return sin(2.0*x*x); }
    if (id == 3) { return sin(param_a*x)/cos(x*x); }
    if (id == 4) { return sin(x+sin(x)/param_a); }
    if (id == 5) { return log(abs(x)); }
    if (id == 6) { return abs(sin(x)*param_a) + abs(cos(x)*param_b); }
    if (id == 7) { return select(-pow(-x, param_a), pow(x, param_a), x > 0.0); }
    if (id == 8) { return param_a*x + param_b; }
    if (id == 9) { return pow(x, param_a); }
    if (id == 10) { return param_a*sin(param_b*x); }
    if (id == 11) { return param_a*pow(-1.0, floor(x*param_b)); }
    if (id == 12) { return 2.0*param_a/param_b*abs(abs(x)%param_b - param_b/2.0) - 2.0*param_a/4.0; }
    if (id == 13) { return floor(param_a*x)/param_b; }
    return 0.0;
}
// END GENERATED FORMULAS

fn variation_yplot2d_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let preset_id = i32(get_param(xform_id, variation_id, 0u));
    let direct_color = i32(get_param(xform_id, variation_id, 7u));
    let color_mode = i32(get_param(xform_id, variation_id, 8u));
    let pa = get_param(xform_id, variation_id, 12u);
    let pb = get_param(xform_id, variation_id, 13u);
    let pc = get_param(xform_id, variation_id, 14u);
    let pd = get_param(xform_id, variation_id, 15u);
    let pe = get_param(xform_id, variation_id, 16u);
    let pf = get_param(xform_id, variation_id, 17u);
    let xmin_s = get_param(xform_id, variation_id, 18u);
    let dx = get_param(xform_id, variation_id, 19u);
    let ymin_s = get_param(xform_id, variation_id, 20u);
    let dy = get_param(xform_id, variation_id, 21u);
    let zmin_s = get_param(xform_id, variation_id, 22u);
    let dz = get_param(xform_id, variation_id, 23u);

    let rand_u = rng_nextf(rng);
    let rand_v = rng_nextf(rng);
    let x = xmin_s + rand_u * dx;
    // z is sampled in 2D mode too (RNG parity with JWF) but unused.
    let z = zmin_s + rand_v * dz;
    let y = yplot2d_wf_formula(preset_id, x, pa, pb, pc, pd, pe, pf);

    if (direct_color > 0) {
        var c = *vc;
        if (color_mode == 2) {
            c = (y - ymin_s) / dy;
        } else if (color_mode != 0) {
            // CM_X (1) and any other value: JWF's default case.
            c = (x - xmin_s) / dx;
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
fn yplot2d_wf_sqr(v: f32) -> f32 { return v * v; }

fn yplot2d_wf_formula(id: i32, x: f32, param_a: f32, param_b: f32, param_c: f32, param_d: f32, param_e: f32, param_f: f32) -> f32 {
    let pi = 3.14159265358979;
    if (id == 0) { return (sin(x)+2.0*sin(2.0*x)+1.0*sin(4.0*x)); }
    if (id == 1) { return sin(x)*cos(x); }
    if (id == 2) { return sin(2.0*x*x); }
    if (id == 3) { return sin(param_a*x)/cos(x*x); }
    if (id == 4) { return sin(x+sin(x)/param_a); }
    if (id == 5) { return log(abs(x)); }
    if (id == 6) { return abs(sin(x)*param_a) + abs(cos(x)*param_b); }
    if (id == 7) { return select(-pow(-x, param_a), pow(x, param_a), x > 0.0); }
    if (id == 8) { return param_a*x + param_b; }
    if (id == 9) { return pow(x, param_a); }
    if (id == 10) { return param_a*sin(param_b*x); }
    if (id == 11) { return param_a*pow(-1.0, floor(x*param_b)); }
    if (id == 12) { return 2.0*param_a/param_b*abs(abs(x)%param_b - param_b/2.0) - 2.0*param_a/4.0; }
    if (id == 13) { return floor(param_a*x)/param_b; }
    return 0.0;
}
// END GENERATED FORMULAS

fn variation_yplot2d_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let preset_id = i32(get_param(xform_id, variation_id, 0u));
    let direct_color = i32(get_param(xform_id, variation_id, 7u));
    let color_mode = i32(get_param(xform_id, variation_id, 8u));
    let pa = get_param(xform_id, variation_id, 12u);
    let pb = get_param(xform_id, variation_id, 13u);
    let pc = get_param(xform_id, variation_id, 14u);
    let pd = get_param(xform_id, variation_id, 15u);
    let pe = get_param(xform_id, variation_id, 16u);
    let pf = get_param(xform_id, variation_id, 17u);
    let xmin_s = get_param(xform_id, variation_id, 18u);
    let dx = get_param(xform_id, variation_id, 19u);
    let ymin_s = get_param(xform_id, variation_id, 20u);
    let dy = get_param(xform_id, variation_id, 21u);
    let zmin_s = get_param(xform_id, variation_id, 22u);
    let dz = get_param(xform_id, variation_id, 23u);

    let rand_u = rng_nextf(rng);
    let rand_v = rng_nextf(rng);
    let x = xmin_s + rand_u * dx;
    let z = zmin_s + rand_v * dz;
    let y = yplot2d_wf_formula(preset_id, x, pa, pb, pc, pd, pe, pf);

    if (direct_color > 0) {
        var c = *vc;
        if (color_mode == 2) {
            c = (y - ymin_s) / dy;
        } else if (color_mode != 0) {
            // CM_X (1) and any other value: JWF's default case.
            c = (x - xmin_s) / dx;
        }
        // Mode 0 (colormap, unsupported): clamp the incoming color only —
        // exactly JWF's behavior when no colormap image is loaded.
        *vc = clamp(c, 0.0, 1.0);
    }

    return vec3<f32>(x, y, z);
}
"#,
};
