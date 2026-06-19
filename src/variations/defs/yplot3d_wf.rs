//! yplot3d_wf (Andreas Maschke) — 3D surface plot, preset formulas only
//!
//! Samples a random `(x, z)` in `[xmin, xmax] × [zmin, zmax]`, evaluates a
//! height formula `y = f(x, z)`, and emits `(x, y, z)` (weight-outside,
//! `pVarTP += pAmount·(x, y, z)`). Direct-color modes map x/y/z position to
//! the palette.
//!
//! **Preset-only port.** JWF compiles the formula *string* to Java bytecode
//! at runtime (`YPlot3DFormulaEvaluator`); supporting custom formulas here
//! would need a formula→WGSL transpiler. Instead the 7 stock presets from
//! `yplot3d_wf_presets.txt` are baked into a WGSL switch on `preset_id`
//! (same pattern as iconattractor_js's 17 presets). A flame whose formula
//! was hand-edited in JWF imports with `preset_id = -1` and renders as
//! `y = 0` (matching JWF's `createDefaultPreset` formula `"0.0"`), which
//! will NOT match JWF — documented limitation.
//!
//! Also unsupported (image-file resources, no texture plumbing in
//! variations): `colormap_filename` (color_mode 0 falls back to a plain
//! clamp of the incoming color, exactly what JWF does when no map is
//! loaded) and `displ_map_filename` (surface-normal displacement; inactive
//! in JWF too unless a map is loaded). Their tuning params
//! (blend_colormap, displ_amount, blend_displ_map) are declared with JWF's
//! defaults (1, 0.1, 1) for XML round-trip parity but never read.
//!
//! Divergences from JWF (documented, deliberate):
//!   - JWF's default `preset_id` is RANDOM (picked in the constructor);
//!     ours defaults to 0 for determinism. XML import always carries an
//!     explicit value, so this only affects newly added variations.
//!   - Selecting a preset in the JWF UI also overwrites xmin/xmax and
//!     (via a JWF parser quirk: the preset file's `zmin/zmax` values are
//!     stored in the preset's ymin/ymax fields) ymin/ymax. Our params have
//!     no side effects — ranges always come from the params themselves,
//!     which is what the XML carries, so imports match.
//!
//! `param_a..param_f` are formula inputs; none of the 7 stock presets
//! reference them, so they're declared (XML parity, custom-formula flames
//! carry values) but unused.
//!
//! Sources:
//!   - `output/variation-jwf-source/plot/YPlot3DWFFunc.java`
//!   - `output/variation-jwf-source/plot/YPlot3DWFFuncPresets.java`
//!   - `output/variation-jwf-source/plot/yplot3d_wf_presets.txt`

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// 3D surface plot — samples a random `(x, z)` in the configured rectangle,
/// evaluates a preset height formula `y = f(x, z)`, and emits `(x, y, z)`.
/// The 7 presets are JWildfire's stock yplot3d formulas (ripples, waves,
/// radial decay patterns). Direct-color maps the x/y/z position (or x·z)
/// onto the palette. Custom JWF formulas are not supported — only presets.
///
/// # Authors
/// - Andreas Maschke
pub static YPLOT3D_WF: VariationDef = VariationDef {
    name: "yplot3d_wf",
    aliases: &[],
    display_name: "Y-Plot 3D WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Any,
    features: &[Feature::NeedsRng, Feature::AlwaysZ, Feature::WritesColor],
    parameters: &[
        param!("preset_id", "Preset ID", int, 0.0, -1.0, 6.0, "Which baked formula preset to plot (0-6). JWF's default is a random preset; ours is 0 for determinism. -1 means a custom JWF formula we can't evaluate — renders as y = 0."),
        param!("xmin", "X Min", unlimited_float, -3.0, -10.0, 10.0, "Lower X bound of the sampled rectangle (swapped with xmax if larger)."),
        param!("xmax", "X Max", unlimited_float, 2.0, -10.0, 10.0, "Upper X bound of the sampled rectangle."),
        param!("ymin", "Y Min", unlimited_float, -4.0, -10.0, 10.0, "Lower Y bound — only used to normalize the Y color mode (the surface itself is unclamped)."),
        param!("ymax", "Y Max", unlimited_float, 4.0, -10.0, 10.0, "Upper Y bound for the Y color mode."),
        param!("zmin", "Z Min", unlimited_float, -2.0, -10.0, 10.0, "Lower Z bound of the sampled rectangle (swapped with zmax if larger)."),
        param!("zmax", "Z Max", unlimited_float, 2.0, -10.0, 10.0, "Upper Z bound of the sampled rectangle."),
        param!("direct_color", "Direct Color", bool, true, "When on, writes the color register from the sample position per Color Mode. Visible color requires the transform's Direct Color slider > 0."),
        param!("color_mode", "Color Mode", enum, 3, &["Colormap (unsupported)", "X", "Y", "Z", "X×Z"], "Position→palette mapping. X/Y/Z normalize the corresponding coordinate over its range; X×Z multiplies the two. Colormap needs an image file (unsupported) and falls back to clamping the incoming color, as JWF does with no map loaded."),
        param!("blend_colormap", "Blend Colormap", int, 1.0, 0.0, 1.0, "Unused — colormap images are unsupported. Preserved for JWF XML round-trip (JWF default 1)."),
        param!("displ_amount", "Displ Amount", unlimited_float, 0.1, -10.0, 10.0, "Unused — displacement-map images are unsupported. Preserved for JWF XML round-trip (JWF default 0.1)."),
        param!("blend_displ_map", "Blend Displ Map", int, 1.0, 0.0, 1.0, "Unused — displacement-map images are unsupported. Preserved for JWF XML round-trip (JWF default 1)."),
        param!("param_a", "Param A", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_a` — none of the 7 stock presets use it. Preserved for JWF XML round-trip."),
        param!("param_b", "Param B", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_b` — unused by the stock presets."),
        param!("param_c", "Param C", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_c` — unused by the stock presets."),
        param!("param_d", "Param D", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_d` — unused by the stock presets."),
        param!("param_e", "Param E", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_e` — unused by the stock presets."),
        param!("param_f", "Param F", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_f` — unused by the stock presets."),
    ],
    // 6 init slots (18..24): _xmin, _dx, _ymin, _dy, _zmin, _dz
    // (min/max swapped if reversed, matching JWF's init()).
    init_param_count: 6,
    wgsl_init: Some(r#"
fn init_yplot3d_wf(user: array<f32, 18>) -> array<f32, 6> {
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
fn yplot3d_wf_formula(id: i32, x: f32, z: f32) -> f32 {
    // The 7 stock presets from yplot3d_wf_presets.txt. Out-of-range ids
    // (including -1 = custom JWF formula) fall to 0.0, matching JWF's
    // createDefaultPreset formula "0.0".
    let pi = 3.14159265358979;
    let r2 = x * x + z * z;
    if (id == 0) { return sin(2.0 * exp(-4.0 * r2)); }
    if (id == 1) { return cos(x * z * 12.0) / 6.0; }
    if (id == 2) { return cos(sqrt(r2) * 14.0) * exp(-2.0 * r2); }
    if (id == 3) {
        let safe_z = select(z, 1e-30, abs(z) < 1e-30);
        return cos(atan(x / safe_z) * 8.0) / 4.0 * sin(sqrt(r2) * 3.0);
    }
    if (id == 4) { return (sin(x) * sin(x) + cos(z) * cos(z)) / (5.0 + r2); }
    if (id == 5) { return cos(2.0 * pi * (x + z)) * (1.0 - sqrt(r2)); }
    if (id == 6) { return sin(r2); }
    return 0.0;
}

fn variation_yplot3d_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let preset_id = i32(get_param(xform_id, variation_id, 0u));
    let direct_color = i32(get_param(xform_id, variation_id, 7u));
    let color_mode = i32(get_param(xform_id, variation_id, 8u));
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
    let y = yplot3d_wf_formula(preset_id, x, z);

    if (direct_color > 0) {
        var c = *vc;
        if (color_mode == 1) {
            c = (x - xmin_s) / dx;
        } else if (color_mode == 2) {
            c = (y - ymin_s) / dy;
        } else if (color_mode == 4) {
            c = (x - xmin_s) / dx * (z - zmin_s) / dz;
        } else if (color_mode != 0) {
            // CM_Z (3) and any other value: JWF's default case.
            c = (z - zmin_s) / dz;
        }
        // Mode 0 (colormap, unsupported): clamp the incoming color only —
        // exactly JWF's behavior when no colormap image is loaded.
        *vc = clamp(c, 0.0, 1.0);
    }

    // 2D mode: z is still sampled (RNG parity + it drives the default
    // color mode) but only (x, y) is emitted.
    return vec2<f32>(x, y);
}
"#,
    wgsl_3d: r#"
fn yplot3d_wf_formula(id: i32, x: f32, z: f32) -> f32 {
    // The 7 stock presets from yplot3d_wf_presets.txt. Out-of-range ids
    // (including -1 = custom JWF formula) fall to 0.0, matching JWF's
    // createDefaultPreset formula "0.0".
    let pi = 3.14159265358979;
    let r2 = x * x + z * z;
    if (id == 0) { return sin(2.0 * exp(-4.0 * r2)); }
    if (id == 1) { return cos(x * z * 12.0) / 6.0; }
    if (id == 2) { return cos(sqrt(r2) * 14.0) * exp(-2.0 * r2); }
    if (id == 3) {
        let safe_z = select(z, 1e-30, abs(z) < 1e-30);
        return cos(atan(x / safe_z) * 8.0) / 4.0 * sin(sqrt(r2) * 3.0);
    }
    if (id == 4) { return (sin(x) * sin(x) + cos(z) * cos(z)) / (5.0 + r2); }
    if (id == 5) { return cos(2.0 * pi * (x + z)) * (1.0 - sqrt(r2)); }
    if (id == 6) { return sin(r2); }
    return 0.0;
}

fn variation_yplot3d_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let preset_id = i32(get_param(xform_id, variation_id, 0u));
    let direct_color = i32(get_param(xform_id, variation_id, 7u));
    let color_mode = i32(get_param(xform_id, variation_id, 8u));
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
    let y = yplot3d_wf_formula(preset_id, x, z);

    if (direct_color > 0) {
        var c = *vc;
        if (color_mode == 1) {
            c = (x - xmin_s) / dx;
        } else if (color_mode == 2) {
            c = (y - ymin_s) / dy;
        } else if (color_mode == 4) {
            c = (x - xmin_s) / dx * (z - zmin_s) / dz;
        } else if (color_mode != 0) {
            // CM_Z (3) and any other value: JWF's default case.
            c = (z - zmin_s) / dz;
        }
        // Mode 0 (colormap, unsupported): clamp the incoming color only —
        // exactly JWF's behavior when no colormap image is loaded.
        *vc = clamp(c, 0.0, 1.0);
    }

    return vec3<f32>(x, y, z);
}
"#,
};
