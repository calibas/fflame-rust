//! parplot2d_wf (Andreas Maschke) — parametric surface plot, preset
//! formulas only
//!
//! Evaluates a parametric surface `(x, y, z) = F(u, v)` over
//! `[umin, umax] × [vmin, vmax]` and emits the point weight-outside.
//! With `solid = 1` (the JWF default) the `(u, v)` parameters are drawn
//! randomly each iteration; with `solid = 0` they come from the INPUT
//! point (`u = umin + p.x·du`, `v = vmin + p.y·dv`) so the surface is
//! traversed by the chaos game itself. Direct-color maps u/v position
//! (or u·v) onto the palette.
//!
//! Preset-only port — same model and limitations as `yplot3d_wf`
//! (see that module's docs): the 47 stock presets from
//! `parplot2d_wf_presets.txt` (×3 formulas each) are baked into a WGSL
//! switch (regenerate with `scripts/gen_plot_wf_formulas.py`); custom
//! JWF formulas render as the origin; colormap/displacement image
//! resources unsupported, their tuning params declared for XML parity
//! (JWF defaults 1, 0.1, 1); `preset_id` defaults to 0 instead of JWF's
//! random pick; selecting a preset does NOT auto-apply its range or
//! `param_a..f` defaults (XML imports carry explicit values).
//!
//! Sources:
//!   - `output/variation-jwf-source/plot/ParPlot2DWFFunc.java`
//!   - `output/variation-jwf-source/plot/parplot2d_wf_presets.txt`

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Parametric surface plot — evaluates `(x, y, z) = F(u, v)` for one of 47
/// baked JWildfire presets (tori, shells, springs, Klein-style surfaces;
/// many take `param_a..f`). `solid` picks random `(u, v)` per iteration
/// (default) vs. mapping the input point into parameter space. Custom JWF
/// formulas are not supported.
///
/// # Authors
/// - Andreas Maschke
/// - Frank Baumann and other contributors (preset formulas)
pub static PARPLOT2D_WF: VariationDef = VariationDef {
    name: "parplot2d_wf",
    aliases: &[],
    display_name: "Par-Plot 2D WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::AlwaysZ, Feature::WritesColor],
    parameters: &[
        param!("preset_id", "Preset ID", int, 0.0, -1.0, 46.0, "Which baked formula preset to plot (0-46). JWF's default is a random preset; ours is 0 for determinism. -1 means a custom JWF formula we can't evaluate — renders at the origin."),
        param!("umin", "U Min", unlimited_float, 0.0, -100.0, 100.0, "Lower U bound of the parameter rectangle (swapped with umax if larger)."),
        param!("umax", "U Max", unlimited_float, 6.283185307179586, -100.0, 100.0, "Upper U bound. JWF default 2π."),
        param!("vmin", "V Min", unlimited_float, 0.0, -100.0, 100.0, "Lower V bound of the parameter rectangle."),
        param!("vmax", "V Max", unlimited_float, 6.283185307179586, -100.0, 100.0, "Upper V bound. JWF default 2π."),
        param!("direct_color", "Direct Color", bool, true, "When on, writes the color register from the (u, v) position per Color Mode. Visible color requires the transform's Direct Color slider > 0."),
        param!("color_mode", "Color Mode", enum, 3, &["Colormap (unsupported)", "U", "V", "U×V"], "Parameter→palette mapping. U/V normalize the corresponding parameter over its range; U×V multiplies the two. Colormap needs an image file (unsupported) and falls back to clamping the incoming color, as JWF does with no map loaded."),
        param!("blend_colormap", "Blend Colormap", int, 1.0, 0.0, 1.0, "Unused — colormap images are unsupported. Preserved for JWF XML round-trip (JWF default 1)."),
        param!("displ_amount", "Displ Amount", unlimited_float, 0.1, -10.0, 10.0, "Unused — displacement-map images are unsupported. Preserved for JWF XML round-trip (JWF default 0.1)."),
        param!("blend_displ_map", "Blend Displ Map", int, 1.0, 0.0, 1.0, "Unused — displacement-map images are unsupported. Preserved for JWF XML round-trip (JWF default 1)."),
        param!("solid", "Solid", bool, true, "When on (JWF default), draws random (u, v) each iteration — a solid surface. When off, maps the input point into parameter space (u from x, v from y) so the chaos game traverses the surface."),
        param!("param_a", "Param A", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_a` — used by many presets (JWF applies each preset's own default when selected; set it manually here)."),
        param!("param_b", "Param B", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_b`."),
        param!("param_c", "Param C", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_c`."),
        param!("param_d", "Param D", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_d`."),
        param!("param_e", "Param E", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_e`."),
        param!("param_f", "Param F", unlimited_float, 0.0, -10.0, 10.0, "Formula input `param_f`."),
    ],
    // 4 init slots (17..21): _umin, _du, _vmin, _dv
    // (min/max swapped if reversed, matching JWF's init()).
    init_param_count: 4,
    wgsl_init: Some(r#"
fn init_parplot2d_wf(user: array<f32, 17>) -> array<f32, 4> {
    var out: array<f32, 4>;
    var umin = user[1];
    var umax = user[2];
    if (umin > umax) { let t = umin; umin = umax; umax = t; }
    out[0] = umin;
    out[1] = umax - umin;
    var vmin = user[3];
    var vmax = user[4];
    if (vmin > vmax) { let t = vmin; vmin = vmax; vmax = t; }
    out[2] = vmin;
    out[3] = vmax - vmin;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    wgsl_2d: r#"
// BEGIN GENERATED FORMULAS (scripts/gen_plot_wf_formulas.py)
fn parplot2d_wf_sqr(v: f32) -> f32 { return v * v; }

fn parplot2d_wf_formula(id: i32, u: f32, v: f32, param_a: f32, param_b: f32, param_c: f32, param_d: f32, param_e: f32, param_f: f32) -> vec3<f32> {
    let pi = 3.14159265358979;
    if (id == 0) {
        return vec3<f32>(
            cos(u)*(4.0+cos(v)),
            sin(u)*(4.0+cos(v)),
            4.0*sin(2.0*u)+sin(v)*(1.2-sin(v)));
    }
    if (id == 1) {
        return vec3<f32>(
            cos(v)*sin(2.0*u),
            sin(v)*sin(2.0*u),
            sin(2.0*v)*parplot2d_wf_sqr((cos(u))));
    }
    if (id == 2) {
        return vec3<f32>(
            cos(u)*(exp(u/10.0)-1.0)*(cos(v)+0.8),
            sin(u)*(exp(u/10.0)-1.0)*(cos(v)+0.8),
            (exp(u/10.0)-1.0)*sin(v));
    }
    if (id == 3) {
        return vec3<f32>(
            cos(v)*(2.0+sin(u+v/3.0)),
            sin(v)*(2.0+sin(u+v/3.0)),
            cos(u+v/3.0));
    }
    if (id == 4) {
        return vec3<f32>(
            cos(u)*(2.0+cos(v)),
            sin(u)*(2.0+cos(v)),
            (u-2.0*pi)+sin(v));
    }
    if (id == 5) {
        return vec3<f32>(
            u*cos(v),
            u*sin(v),
            parplot2d_wf_sqr(cos(4.0*u))*exp(0.0-u));
    }
    if (id == 6) {
        return vec3<f32>(
            cos(u)*(2.0+parplot2d_wf_sqr(cos(u/2.0))*sin(v)),
            sin(u)*(2.0+parplot2d_wf_sqr(cos(u/2.0))*sin(v)),
            parplot2d_wf_sqr(cos(u/2.0))*cos(v));
    }
    if (id == 7) {
        return vec3<f32>(
            cos(u)*(4.0+cos(v)),
            sin(u)*(4.0+cos(v)),
            3.0*sin(u)+(sin(3.0*v)*(1.2+sin(3.0*v))));
    }
    if (id == 8) {
        return vec3<f32>(
            u*cos(v),
            v*cos(u),
            u*v*sin(u)*sin(v));
    }
    if (id == 9) {
        return vec3<f32>(
            cos(u)*sin(v*v*v/(pi*pi)),
            sin(u)*sin(v),
            cos(v));
    }
    if (id == 10) {
        return vec3<f32>(
            cos(u)*((cos(3.0*u)+2.0)*sin(v)+0.5),
            sin(u)*((cos(3.0*u)+2.0)*sin(v)+0.5),
            (cos(3.0*u)+2.0)*cos(v));
    }
    if (id == 11) {
        return vec3<f32>(
            sin(u)*sin(v)+0.05*cos(20.0*v),
            cos(u)*sin(v)+0.05*cos(20.0*u),
            cos(v));
    }
    if (id == 12) {
        return vec3<f32>(
            2.0*(1.0-exp(u/(6.0*pi)))*cos(u)*parplot2d_wf_sqr(cos(v/2.0)),
            2.0*(-1.0+exp(u/(6.0*pi)))*sin(u)*parplot2d_wf_sqr(cos(v/2.0)),
            1.0-exp(u/(3.0*pi))-sin(v)+exp(u/(6.0*pi))*sin(v));
    }
    if (id == 13) {
        return vec3<f32>(
            (6.0+2.0*cos(u*v))*cos(u),
            (6.0+2.0*cos(u*v))*sin(u),
            (2.0*u+2.0*sin(u*v)));
    }
    if (id == 14) {
        return vec3<f32>(
            (1.0+0.25*cos(75.0*u))*cos(u),
            (1.0+0.25*cos(75.0*u))*sin(u),
            u+sin(75.0*u));
    }
    if (id == 15) {
        return vec3<f32>(
            7.83*cos((v-pi)/2.0)*(cos(16.4*v)),
            7.83*cos((v-pi)/2.0)*(sin(16.4*v)),
            7.83*sin((v-pi)/2.0));
    }
    if (id == 16) {
        return vec3<f32>(
            (2.0 + sin(7.0*u + 5.0*v))*cos(u)*sin(v),
            (2.0 + sin(7.0*u + 5.0*v))*sin(u)*sin(v),
            (2.0 + sin(7.0*u + 5.0*v))*cos(v));
    }
    if (id == 17) {
        return vec3<f32>(
            sin(u)*sin(v),
            cos(v)*cos(u),
            sin(sin(u)+cos(v)));
    }
    if (id == 18) {
        return vec3<f32>(
            (2.0*v*cos(u)),
            2.0*v*(sin(u))+v*abs(cos(u)),
            cos(3.0*v)*sin(3.0*v));
    }
    if (id == 19) {
        return vec3<f32>(
            v*sin(abs(u)),
            u*sin(abs(v)),
            u+abs(sin(v*u)));
    }
    if (id == 20) {
        return vec3<f32>(
            cos(u)*(6.0-(5.0/4.0+sin(3.0-v))*sin(v-3.0-u)),
            (6.0-(5.0/4.0+sin(3.0*v))*sin(v-3.0*u))*sin(u),
            -cos(v-3.0*u)*(5.0/4.0+sin(3.0*v)));
    }
    if (id == 21) {
        return vec3<f32>(
            (4.0+(sin(4.0*(v+2.0*u))+1.25)*cos(v))*cos(u),
            (4.0+(sin(4.0*(v+2.0*u))+1.25)*cos(v))*sin(u),
            ((sin(4.0*(v+2.0*u))+1.25)*sin(v)));
    }
    if (id == 22) {
        return vec3<f32>(
            u,
            sin(v)*(u*u*u+2.0*u*u-2.0*u+2.0)/5.0,
            cos(v)*(u*u*u+2.0*u*u-2.0*u+2.0)/5.0);
    }
    if (id == 23) {
        return vec3<f32>(
            -0.8*u+(2.0*0.75*cosh(0.5*u)*sinh(0.5*u))/(0.5*((sqrt(0.75)*parplot2d_wf_sqr(cosh(0.5*u))) +parplot2d_wf_sqr(0.5*sin(sqrt(0.75)*v)))),
            (2.0*sqrt(0.75)*cosh(0.5*u)*(-(sqrt(0.75)*cos(v)*cos(sqrt(0.75)*v))-sin(v)*sin(sqrt(0.75)*v)))/(0.5*parplot2d_wf_sqr((sqrt(0.75)*cosh(0.5*u)) +parplot2d_wf_sqr(0.5*sin(sqrt(0.75)*v)))),
            (2.0*sqrt(0.75)*cosh(0.5*u)*(-(sqrt(0.75)*sin(v)*cos(sqrt(0.75)*v))+cos(v)*sin(sqrt(0.75)*v)))/(0.5*parplot2d_wf_sqr((sqrt(0.75)*cosh(0.5*u)) +parplot2d_wf_sqr(0.5*sin(sqrt(0.75)*v)))));
    }
    if (id == 24) {
        return vec3<f32>(
            cos(u+0.0)+0.06*sin(1.0*v),
            cos(15.0*u+0.0)-0.6*cos(1.0*v),
            sin(12.0*u+0.0)+0.06*sin(1.0*v));
    }
    if (id == 25) {
        return vec3<f32>(
            (cos(2.0*u))/(sqrt(2.0)+sin(2.0*v)),
            sin(2.0*u)/(sqrt(2.0)+sin(2.0*v)),
            v/(sqrt(5.0)+cos(2.0*v)));
    }
    if (id == 26) {
        return vec3<f32>(
            2.0*sin(3.0*u)/(2.0+cos(v)),
            2.0*(sin(u)+2.0*sin(2.0*u))/(2.0+cos(v+2.0*pi/3.0)),
            (cos(u)-2.0*cos(2.0*u))*(2.0+cos(v))*(2.0+cos(v+2.0*pi/3.0))/4.0);
    }
    if (id == 27) {
        return vec3<f32>(
            pow(1.2,u)*(1.0+cos(v))*cos(u),
            pow(1.2,u)*(1.0+cos(v))*sin(u),
            pow(1.2,u)*sin(v)-1.5*pow(1.2,u));
    }
    if (id == 28) {
        return vec3<f32>(
            u*cos(u)*(cos(v)+1.0),
            u*sin(u)*(cos(v)+1.0),
            u*sin(v)-((u+3.0)/8.0*pi)*u/3.0);
    }
    if (id == 29) {
        return vec3<f32>(
            cos(u)*cos(v)+3.0*cos(u)*(1.5+sin(u*5.0/3.0)/2.0),
            sin(u)*cos(v)+3.0*sin(u)*(1.5+sin(u*5.0/3.0)/2.0),
            sin(v)+2.0*cos(u*5.0/3.0));
    }
    if (id == 30) {
        return vec3<f32>(
            0.1*cos(u),
            -0.1*sin(u),
            v+0.1*sin(u));
    }
    if (id == 31) {
        return vec3<f32>(
            (u/(pi+pi))*(1.0-2.0*v*v)*cos(u),
            (u/(pi+pi))*(1.0-2.0*v*v)*sin(u),
            v);
    }
    if (id == 32) {
        return vec3<f32>(
            (3.0+2.0*cos(v))*cos(u),
            (3.0+2.0*cos(v))*sin(u),
            u+2.0*sin(v));
    }
    if (id == 33) {
        return vec3<f32>(
            u+(1.0/10.0)*sin(10.0*v),
            ((2.0*v)/3.0)*(1.2-(1.0/(1.0+u*u))),
            sin(pi*v)/(2.0*pi*v));
    }
    if (id == 34) {
        return vec3<f32>(
            (v/3.0)*cos(u-(pi+pi)/3.0),
            (v/3.0)*sin(u-(pi+pi)/3.0),
            u/10.0+(v*v)/2.0);
    }
    if (id == 35) {
        return vec3<f32>(
            u*cos(v),
            u*sin(v),
            exp(-u*u)*(sin(param_a*pi*(u))-u*cos(param_b*v)));
    }
    if (id == 36) {
        return vec3<f32>(
            u*param_a,
            v*param_b,
            u*param_c+v*param_d);
    }
    if (id == 37) {
        return vec3<f32>(
            u*param_a,
            v*param_b,
            sin(v*param_c)* param_d);
    }
    if (id == 38) {
        return vec3<f32>(
            cos(u*param_a)*sin(v*param_b),
            sin(u*param_c)*sin(v*param_d),
            sin(v*param_e));
    }
    if (id == 39) {
        return vec3<f32>(
            cos(u*param_a)*sin(v*param_b),
            sin(u*param_c)*sin(v*param_d),
            cos(v*param_e));
    }
    if (id == 40) {
        return vec3<f32>(
            u*cos(v*param_a)-u*param_b,
            v*cos(u*param_c)-v*param_d,
            u*v*sin(u*param_e)*sin(v*param_f)-u/v);
    }
    if (id == 41) {
        return vec3<f32>(
            cos(u*param_a)*sin(u*param_b)-u*param_c,
            sin(u*param_d)*cos(v*param_e)-u*param_f,
            cos(u)*sin(u));
    }
    if (id == 42) {
        return vec3<f32>(
            cos(u*param_a)*sin(u*param_b)-u*param_c,
            (v/param_d)*cos(u*param_e)-v*param_f,
            cos(u)*sin(u));
    }
    if (id == 43) {
        return vec3<f32>(
            cos(v*param_a)+sin(v*param_b)-u*param_c,
            (v/param_d)*cos(u*param_e)-v*param_f,
            cos(u)-sin(u));
    }
    if (id == 44) {
        return vec3<f32>(
            cos(u*param_a)*sin(v*param_b)-u-v,
            sin(v*param_c)*cos(v*param_d)-u*param_e,
            cos(u*param_f)*sin(u*param_f));
    }
    if (id == 45) {
        return vec3<f32>(
            cos(u)*sin(u)-v,
            (v/3.0)*cos(u)-v,
            cos(u)*sin(u));
    }
    if (id == 46) {
        return vec3<f32>(
            cos(u)*sin(v)+(u*v),
            sin(v)*cos(v)*(u+v),
            cos(u)*sin(u));
    }
    return vec3<f32>(0.0, 0.0, 0.0);
}
// END GENERATED FORMULAS

fn variation_parplot2d_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let preset_id = i32(get_param(xform_id, variation_id, 0u));
    let direct_color = i32(get_param(xform_id, variation_id, 5u));
    let color_mode = i32(get_param(xform_id, variation_id, 6u));
    let solid = i32(get_param(xform_id, variation_id, 10u));
    let pa = get_param(xform_id, variation_id, 11u);
    let pb = get_param(xform_id, variation_id, 12u);
    let pc = get_param(xform_id, variation_id, 13u);
    let pd = get_param(xform_id, variation_id, 14u);
    let pe = get_param(xform_id, variation_id, 15u);
    let pf = get_param(xform_id, variation_id, 16u);
    let umin_s = get_param(xform_id, variation_id, 17u);
    let du = get_param(xform_id, variation_id, 18u);
    let vmin_s = get_param(xform_id, variation_id, 19u);
    let dv = get_param(xform_id, variation_id, 20u);

    // solid = 0 takes (u, v) from the INPUT point; solid = 1 draws
    // randoms. JWF draws no randoms in the solid = 0 path — keep the
    // RNG call count identical.
    var rand_u: f32;
    var rand_v: f32;
    if (solid == 0) {
        rand_u = p.x;
        rand_v = p.y;
    } else {
        rand_u = rng_nextf(rng);
        rand_v = rng_nextf(rng);
    }
    let u = umin_s + rand_u * du;
    let v = vmin_s + rand_v * dv;
    let xyz = parplot2d_wf_formula(preset_id, u, v, pa, pb, pc, pd, pe, pf);

    if (direct_color > 0) {
        var c = *vc;
        if (color_mode == 1) {
            c = (u - umin_s) / du;
        } else if (color_mode == 2) {
            c = (v - vmin_s) / dv;
        } else if (color_mode != 0) {
            // CM_UV (3) and any other value: JWF's default case.
            c = (v - vmin_s) / dv * (u - umin_s) / du;
        }
        // Mode 0 (colormap, unsupported): clamp the incoming color only —
        // exactly JWF's behavior when no colormap image is loaded.
        *vc = clamp(c, 0.0, 1.0);
    }

    return vec2<f32>(xyz.x, xyz.y);
}
"#,
    wgsl_3d: r#"
// BEGIN GENERATED FORMULAS (scripts/gen_plot_wf_formulas.py)
fn parplot2d_wf_sqr(v: f32) -> f32 { return v * v; }

fn parplot2d_wf_formula(id: i32, u: f32, v: f32, param_a: f32, param_b: f32, param_c: f32, param_d: f32, param_e: f32, param_f: f32) -> vec3<f32> {
    let pi = 3.14159265358979;
    if (id == 0) {
        return vec3<f32>(
            cos(u)*(4.0+cos(v)),
            sin(u)*(4.0+cos(v)),
            4.0*sin(2.0*u)+sin(v)*(1.2-sin(v)));
    }
    if (id == 1) {
        return vec3<f32>(
            cos(v)*sin(2.0*u),
            sin(v)*sin(2.0*u),
            sin(2.0*v)*parplot2d_wf_sqr((cos(u))));
    }
    if (id == 2) {
        return vec3<f32>(
            cos(u)*(exp(u/10.0)-1.0)*(cos(v)+0.8),
            sin(u)*(exp(u/10.0)-1.0)*(cos(v)+0.8),
            (exp(u/10.0)-1.0)*sin(v));
    }
    if (id == 3) {
        return vec3<f32>(
            cos(v)*(2.0+sin(u+v/3.0)),
            sin(v)*(2.0+sin(u+v/3.0)),
            cos(u+v/3.0));
    }
    if (id == 4) {
        return vec3<f32>(
            cos(u)*(2.0+cos(v)),
            sin(u)*(2.0+cos(v)),
            (u-2.0*pi)+sin(v));
    }
    if (id == 5) {
        return vec3<f32>(
            u*cos(v),
            u*sin(v),
            parplot2d_wf_sqr(cos(4.0*u))*exp(0.0-u));
    }
    if (id == 6) {
        return vec3<f32>(
            cos(u)*(2.0+parplot2d_wf_sqr(cos(u/2.0))*sin(v)),
            sin(u)*(2.0+parplot2d_wf_sqr(cos(u/2.0))*sin(v)),
            parplot2d_wf_sqr(cos(u/2.0))*cos(v));
    }
    if (id == 7) {
        return vec3<f32>(
            cos(u)*(4.0+cos(v)),
            sin(u)*(4.0+cos(v)),
            3.0*sin(u)+(sin(3.0*v)*(1.2+sin(3.0*v))));
    }
    if (id == 8) {
        return vec3<f32>(
            u*cos(v),
            v*cos(u),
            u*v*sin(u)*sin(v));
    }
    if (id == 9) {
        return vec3<f32>(
            cos(u)*sin(v*v*v/(pi*pi)),
            sin(u)*sin(v),
            cos(v));
    }
    if (id == 10) {
        return vec3<f32>(
            cos(u)*((cos(3.0*u)+2.0)*sin(v)+0.5),
            sin(u)*((cos(3.0*u)+2.0)*sin(v)+0.5),
            (cos(3.0*u)+2.0)*cos(v));
    }
    if (id == 11) {
        return vec3<f32>(
            sin(u)*sin(v)+0.05*cos(20.0*v),
            cos(u)*sin(v)+0.05*cos(20.0*u),
            cos(v));
    }
    if (id == 12) {
        return vec3<f32>(
            2.0*(1.0-exp(u/(6.0*pi)))*cos(u)*parplot2d_wf_sqr(cos(v/2.0)),
            2.0*(-1.0+exp(u/(6.0*pi)))*sin(u)*parplot2d_wf_sqr(cos(v/2.0)),
            1.0-exp(u/(3.0*pi))-sin(v)+exp(u/(6.0*pi))*sin(v));
    }
    if (id == 13) {
        return vec3<f32>(
            (6.0+2.0*cos(u*v))*cos(u),
            (6.0+2.0*cos(u*v))*sin(u),
            (2.0*u+2.0*sin(u*v)));
    }
    if (id == 14) {
        return vec3<f32>(
            (1.0+0.25*cos(75.0*u))*cos(u),
            (1.0+0.25*cos(75.0*u))*sin(u),
            u+sin(75.0*u));
    }
    if (id == 15) {
        return vec3<f32>(
            7.83*cos((v-pi)/2.0)*(cos(16.4*v)),
            7.83*cos((v-pi)/2.0)*(sin(16.4*v)),
            7.83*sin((v-pi)/2.0));
    }
    if (id == 16) {
        return vec3<f32>(
            (2.0 + sin(7.0*u + 5.0*v))*cos(u)*sin(v),
            (2.0 + sin(7.0*u + 5.0*v))*sin(u)*sin(v),
            (2.0 + sin(7.0*u + 5.0*v))*cos(v));
    }
    if (id == 17) {
        return vec3<f32>(
            sin(u)*sin(v),
            cos(v)*cos(u),
            sin(sin(u)+cos(v)));
    }
    if (id == 18) {
        return vec3<f32>(
            (2.0*v*cos(u)),
            2.0*v*(sin(u))+v*abs(cos(u)),
            cos(3.0*v)*sin(3.0*v));
    }
    if (id == 19) {
        return vec3<f32>(
            v*sin(abs(u)),
            u*sin(abs(v)),
            u+abs(sin(v*u)));
    }
    if (id == 20) {
        return vec3<f32>(
            cos(u)*(6.0-(5.0/4.0+sin(3.0-v))*sin(v-3.0-u)),
            (6.0-(5.0/4.0+sin(3.0*v))*sin(v-3.0*u))*sin(u),
            -cos(v-3.0*u)*(5.0/4.0+sin(3.0*v)));
    }
    if (id == 21) {
        return vec3<f32>(
            (4.0+(sin(4.0*(v+2.0*u))+1.25)*cos(v))*cos(u),
            (4.0+(sin(4.0*(v+2.0*u))+1.25)*cos(v))*sin(u),
            ((sin(4.0*(v+2.0*u))+1.25)*sin(v)));
    }
    if (id == 22) {
        return vec3<f32>(
            u,
            sin(v)*(u*u*u+2.0*u*u-2.0*u+2.0)/5.0,
            cos(v)*(u*u*u+2.0*u*u-2.0*u+2.0)/5.0);
    }
    if (id == 23) {
        return vec3<f32>(
            -0.8*u+(2.0*0.75*cosh(0.5*u)*sinh(0.5*u))/(0.5*((sqrt(0.75)*parplot2d_wf_sqr(cosh(0.5*u))) +parplot2d_wf_sqr(0.5*sin(sqrt(0.75)*v)))),
            (2.0*sqrt(0.75)*cosh(0.5*u)*(-(sqrt(0.75)*cos(v)*cos(sqrt(0.75)*v))-sin(v)*sin(sqrt(0.75)*v)))/(0.5*parplot2d_wf_sqr((sqrt(0.75)*cosh(0.5*u)) +parplot2d_wf_sqr(0.5*sin(sqrt(0.75)*v)))),
            (2.0*sqrt(0.75)*cosh(0.5*u)*(-(sqrt(0.75)*sin(v)*cos(sqrt(0.75)*v))+cos(v)*sin(sqrt(0.75)*v)))/(0.5*parplot2d_wf_sqr((sqrt(0.75)*cosh(0.5*u)) +parplot2d_wf_sqr(0.5*sin(sqrt(0.75)*v)))));
    }
    if (id == 24) {
        return vec3<f32>(
            cos(u+0.0)+0.06*sin(1.0*v),
            cos(15.0*u+0.0)-0.6*cos(1.0*v),
            sin(12.0*u+0.0)+0.06*sin(1.0*v));
    }
    if (id == 25) {
        return vec3<f32>(
            (cos(2.0*u))/(sqrt(2.0)+sin(2.0*v)),
            sin(2.0*u)/(sqrt(2.0)+sin(2.0*v)),
            v/(sqrt(5.0)+cos(2.0*v)));
    }
    if (id == 26) {
        return vec3<f32>(
            2.0*sin(3.0*u)/(2.0+cos(v)),
            2.0*(sin(u)+2.0*sin(2.0*u))/(2.0+cos(v+2.0*pi/3.0)),
            (cos(u)-2.0*cos(2.0*u))*(2.0+cos(v))*(2.0+cos(v+2.0*pi/3.0))/4.0);
    }
    if (id == 27) {
        return vec3<f32>(
            pow(1.2,u)*(1.0+cos(v))*cos(u),
            pow(1.2,u)*(1.0+cos(v))*sin(u),
            pow(1.2,u)*sin(v)-1.5*pow(1.2,u));
    }
    if (id == 28) {
        return vec3<f32>(
            u*cos(u)*(cos(v)+1.0),
            u*sin(u)*(cos(v)+1.0),
            u*sin(v)-((u+3.0)/8.0*pi)*u/3.0);
    }
    if (id == 29) {
        return vec3<f32>(
            cos(u)*cos(v)+3.0*cos(u)*(1.5+sin(u*5.0/3.0)/2.0),
            sin(u)*cos(v)+3.0*sin(u)*(1.5+sin(u*5.0/3.0)/2.0),
            sin(v)+2.0*cos(u*5.0/3.0));
    }
    if (id == 30) {
        return vec3<f32>(
            0.1*cos(u),
            -0.1*sin(u),
            v+0.1*sin(u));
    }
    if (id == 31) {
        return vec3<f32>(
            (u/(pi+pi))*(1.0-2.0*v*v)*cos(u),
            (u/(pi+pi))*(1.0-2.0*v*v)*sin(u),
            v);
    }
    if (id == 32) {
        return vec3<f32>(
            (3.0+2.0*cos(v))*cos(u),
            (3.0+2.0*cos(v))*sin(u),
            u+2.0*sin(v));
    }
    if (id == 33) {
        return vec3<f32>(
            u+(1.0/10.0)*sin(10.0*v),
            ((2.0*v)/3.0)*(1.2-(1.0/(1.0+u*u))),
            sin(pi*v)/(2.0*pi*v));
    }
    if (id == 34) {
        return vec3<f32>(
            (v/3.0)*cos(u-(pi+pi)/3.0),
            (v/3.0)*sin(u-(pi+pi)/3.0),
            u/10.0+(v*v)/2.0);
    }
    if (id == 35) {
        return vec3<f32>(
            u*cos(v),
            u*sin(v),
            exp(-u*u)*(sin(param_a*pi*(u))-u*cos(param_b*v)));
    }
    if (id == 36) {
        return vec3<f32>(
            u*param_a,
            v*param_b,
            u*param_c+v*param_d);
    }
    if (id == 37) {
        return vec3<f32>(
            u*param_a,
            v*param_b,
            sin(v*param_c)* param_d);
    }
    if (id == 38) {
        return vec3<f32>(
            cos(u*param_a)*sin(v*param_b),
            sin(u*param_c)*sin(v*param_d),
            sin(v*param_e));
    }
    if (id == 39) {
        return vec3<f32>(
            cos(u*param_a)*sin(v*param_b),
            sin(u*param_c)*sin(v*param_d),
            cos(v*param_e));
    }
    if (id == 40) {
        return vec3<f32>(
            u*cos(v*param_a)-u*param_b,
            v*cos(u*param_c)-v*param_d,
            u*v*sin(u*param_e)*sin(v*param_f)-u/v);
    }
    if (id == 41) {
        return vec3<f32>(
            cos(u*param_a)*sin(u*param_b)-u*param_c,
            sin(u*param_d)*cos(v*param_e)-u*param_f,
            cos(u)*sin(u));
    }
    if (id == 42) {
        return vec3<f32>(
            cos(u*param_a)*sin(u*param_b)-u*param_c,
            (v/param_d)*cos(u*param_e)-v*param_f,
            cos(u)*sin(u));
    }
    if (id == 43) {
        return vec3<f32>(
            cos(v*param_a)+sin(v*param_b)-u*param_c,
            (v/param_d)*cos(u*param_e)-v*param_f,
            cos(u)-sin(u));
    }
    if (id == 44) {
        return vec3<f32>(
            cos(u*param_a)*sin(v*param_b)-u-v,
            sin(v*param_c)*cos(v*param_d)-u*param_e,
            cos(u*param_f)*sin(u*param_f));
    }
    if (id == 45) {
        return vec3<f32>(
            cos(u)*sin(u)-v,
            (v/3.0)*cos(u)-v,
            cos(u)*sin(u));
    }
    if (id == 46) {
        return vec3<f32>(
            cos(u)*sin(v)+(u*v),
            sin(v)*cos(v)*(u+v),
            cos(u)*sin(u));
    }
    return vec3<f32>(0.0, 0.0, 0.0);
}
// END GENERATED FORMULAS

fn variation_parplot2d_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let preset_id = i32(get_param(xform_id, variation_id, 0u));
    let direct_color = i32(get_param(xform_id, variation_id, 5u));
    let color_mode = i32(get_param(xform_id, variation_id, 6u));
    let solid = i32(get_param(xform_id, variation_id, 10u));
    let pa = get_param(xform_id, variation_id, 11u);
    let pb = get_param(xform_id, variation_id, 12u);
    let pc = get_param(xform_id, variation_id, 13u);
    let pd = get_param(xform_id, variation_id, 14u);
    let pe = get_param(xform_id, variation_id, 15u);
    let pf = get_param(xform_id, variation_id, 16u);
    let umin_s = get_param(xform_id, variation_id, 17u);
    let du = get_param(xform_id, variation_id, 18u);
    let vmin_s = get_param(xform_id, variation_id, 19u);
    let dv = get_param(xform_id, variation_id, 20u);

    // solid = 0 takes (u, v) from the INPUT point; solid = 1 draws
    // randoms. JWF draws no randoms in the solid = 0 path — keep the
    // RNG call count identical.
    var rand_u: f32;
    var rand_v: f32;
    if (solid == 0) {
        rand_u = p.x;
        rand_v = p.y;
    } else {
        rand_u = rng_nextf(rng);
        rand_v = rng_nextf(rng);
    }
    let u = umin_s + rand_u * du;
    let v = vmin_s + rand_v * dv;
    let xyz = parplot2d_wf_formula(preset_id, u, v, pa, pb, pc, pd, pe, pf);

    if (direct_color > 0) {
        var c = *vc;
        if (color_mode == 1) {
            c = (u - umin_s) / du;
        } else if (color_mode == 2) {
            c = (v - vmin_s) / dv;
        } else if (color_mode != 0) {
            // CM_UV (3) and any other value: JWF's default case.
            c = (v - vmin_s) / dv * (u - umin_s) / du;
        }
        // Mode 0 (colormap, unsupported): clamp the incoming color only —
        // exactly JWF's behavior when no colormap image is loaded.
        *vc = clamp(c, 0.0, 1.0);
    }

    return xyz;
}
"#,
};
