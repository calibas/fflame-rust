//! Variations that depended on init-step support to port at all
//!
//! Both were on the porter-omitted-init watchlist before init-dispatch
//! infrastructure landed:
//!   - `target` (Michael Faber) — uses `_t_size_2 = size / 2`
//!   - `yin_yang` (dark-beam) — uses `sin/cos(π · ang1)` and `sin/cos(π · ang2)`
//!
//! Both ports are faithful to upstream — no internal-weight tricks, no
//! preserved bugs.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};

// =============================================================================
// target: Michael Faber's "Target" variation
//   Init: t_size_2 = size / 2
//   Body:
//     a = atan2(y, x);  r = sqrt(x² + y²);  t = log(r)
//     if t < 0: t -= t_size_2
//     t = |t| mod size
//     a += t < t_size_2 ? even : odd
//     output (r · cos(a), r · sin(a))
//
//   Upstream defaults `size = 0`, but that yields `t mod 0` = NaN and
//   breaks the variation. We default to 1.0 so the variation is usable
//   out-of-the-box; users can override.
// =============================================================================
pub static TARGET: VariationDef = VariationDef {
    name: "target",
    display_name: "Target",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        VariationParamDef { name: "even", display_name: "Even", param_type: ParamType::Angle,
                            default_value: 0.0, min_value: Some(-360.0), max_value: Some(360.0) },
        VariationParamDef { name: "odd", display_name: "Odd", param_type: ParamType::Angle,
                            default_value: 0.0, min_value: Some(-360.0), max_value: Some(360.0) },
        VariationParamDef { name: "size", display_name: "Size", param_type: ParamType::UnlimitedFloat,
                            default_value: 1.0, min_value: Some(0.01), max_value: Some(10.0) },
    ],
    // 1 derived value at slot 3:
    //   3: t_size_2  (size / 2)
    needs_affine: false,
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_target(user: array<f32, 3>) -> array<f32, 1> {
    var out: array<f32, 1>;
    out[0] = 0.5 * user[2];
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_target(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let even_p = get_param(xform_id, variation_id, 0u);
    let odd_p = get_param(xform_id, variation_id, 1u);
    let size = max(get_param(xform_id, variation_id, 2u), 1e-6);
    let t_size_2 = get_param(xform_id, variation_id, 3u);

    var a = atan2(p.y, p.x);
    let r = sqrt(p.x * p.x + p.y * p.y);
    var t = log(max(r, 1e-30));
    if (t < 0.0) {
        t = t - t_size_2;
    }
    t = abs(t) - floor(abs(t) / size) * size;  // |t| mod size
    if (t < t_size_2) {
        a = a + even_p;
    } else {
        a = a + odd_p;
    }
    return vec2<f32>(r * cos(a), r * sin(a));
}
"#,
    wgsl_3d: Some(r#"
fn variation_target(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let even_p = get_param(xform_id, variation_id, 0u);
    let odd_p = get_param(xform_id, variation_id, 1u);
    let size = max(get_param(xform_id, variation_id, 2u), 1e-6);
    let t_size_2 = get_param(xform_id, variation_id, 3u);

    var a = atan2(p.y, p.x);
    let r = sqrt(p.x * p.x + p.y * p.y);
    var t = log(max(r, 1e-30));
    if (t < 0.0) {
        t = t - t_size_2;
    }
    t = abs(t) - floor(abs(t) / size) * size;
    if (t < t_size_2) {
        a = a + even_p;
    } else {
        a = a + odd_p;
    }
    return vec3<f32>(r * cos(a), r * sin(a), p.z);
}
"#),
};

// =============================================================================
// yin_yang: dark-beam's yin-yang variation
//   Init: sina = sin(π·ang1), cosa = cos(π·ang1)
//         sinb = sin(π·ang2), cosb = cos(π·ang2)
//   Body: rotation by (cosa, sina) inside the unit disc, with optional
//   second rotation by (cosb, sinb) chosen randomly when dual_t is set.
//   Then a fancy reflection-onto-the-yin-yang shape using the radius
//   parameter. Outside the unit disc: pass-through if `outside=1`,
//   else discard the point.
// =============================================================================
pub static YIN_YANG: VariationDef = VariationDef {
    name: "yin_yang",
    display_name: "Yin Yang",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        VariationParamDef { name: "radius", display_name: "Radius", param_type: ParamType::Float,
                            default_value: 0.5, min_value: Some(0.0), max_value: Some(1.0) },
        VariationParamDef { name: "ang1", display_name: "Ang1", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.0, min_value: Some(-2.0), max_value: Some(2.0) },
        VariationParamDef { name: "ang2", display_name: "Ang2", param_type: ParamType::UnlimitedFloat,
                            default_value: 0.0, min_value: Some(-2.0), max_value: Some(2.0) },
        VariationParamDef { name: "dual_t", display_name: "Dual T", param_type: ParamType::Boolean,
                            default_value: 1.0, min_value: None, max_value: None },
        VariationParamDef { name: "outside", display_name: "Outside", param_type: ParamType::Boolean,
                            default_value: 0.0, min_value: None, max_value: None },
    ],
    // 4 derived values at slots 5..9:
    //   5: sina   sin(π · ang1)
    //   6: cosa   cos(π · ang1)
    //   7: sinb   sin(π · ang2)
    //   8: cosb   cos(π · ang2)
    needs_affine: false,
    init_param_count: 4,
    wgsl_init: Some(r#"
fn init_yin_yang(user: array<f32, 5>) -> array<f32, 4> {
    let pi = 3.14159265358979;
    let ang1 = user[1];
    let ang2 = user[2];
    var out: array<f32, 4>;
    out[0] = sin(pi * ang1);
    out[1] = cos(pi * ang1);
    out[2] = sin(pi * ang2);
    out[3] = cos(pi * ang2);
    return out;
}
"#),
    wgsl_2d: r#"
fn variation_yin_yang(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let dual_t = get_param(xform_id, variation_id, 3u);
    let outside = get_param(xform_id, variation_id, 4u);
    let sina = get_param(xform_id, variation_id, 5u);
    let cosa = get_param(xform_id, variation_id, 6u);
    let sinb = get_param(xform_id, variation_id, 7u);
    let cosb = get_param(xform_id, variation_id, 8u);

    let r2 = p.x * p.x + p.y * p.y;
    if (r2 < 1.0) {
        var inv = 1.0;
        var rr = radius;
        var nx: f32; var ny: f32;
        if (dual_t > 0.5 && rng_nextf(rng) > 0.5) {
            inv = -1.0;
            rr = 1.0 - radius;
            nx = p.x * cosb - p.y * sinb;
            ny = p.x * sinb + p.y * cosb;
        } else {
            nx = p.x * cosa - p.y * sina;
            ny = p.x * sina + p.y * cosa;
        }
        if (ny > 0.0) {
            let t = sqrt(max(1.0 - ny * ny, 0.0));
            let k = nx / max(t, 1e-30);
            let t1 = (t - 0.5) * 2.0;
            let alfa = (1.0 - k) * 0.5;
            let beta = 1.0 - alfa;
            let dx = alfa * (rr - 1.0);
            let k1 = alfa * rr + beta;
            return vec2<f32>(
                (t1 * k1 + dx) * inv,
                sqrt(max(1.0 - t1 * t1, 0.0)) * k1 * inv,
            );
        } else {
            return vec2<f32>(
                (nx * (1.0 - rr) + rr) * inv,
                (ny * (1.0 - rr)) * inv,
            );
        }
    } else if (outside > 0.5) {
        return p;
    }
    return vec2<f32>(0.0, 0.0);
}
"#,
    wgsl_3d: Some(r#"
fn variation_yin_yang(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let radius = get_param(xform_id, variation_id, 0u);
    let dual_t = get_param(xform_id, variation_id, 3u);
    let outside = get_param(xform_id, variation_id, 4u);
    let sina = get_param(xform_id, variation_id, 5u);
    let cosa = get_param(xform_id, variation_id, 6u);
    let sinb = get_param(xform_id, variation_id, 7u);
    let cosb = get_param(xform_id, variation_id, 8u);

    let r2 = p.x * p.x + p.y * p.y;
    if (r2 < 1.0) {
        var inv = 1.0;
        var rr = radius;
        var nx: f32; var ny: f32;
        if (dual_t > 0.5 && rng_nextf(rng) > 0.5) {
            inv = -1.0;
            rr = 1.0 - radius;
            nx = p.x * cosb - p.y * sinb;
            ny = p.x * sinb + p.y * cosb;
        } else {
            nx = p.x * cosa - p.y * sina;
            ny = p.x * sina + p.y * cosa;
        }
        if (ny > 0.0) {
            let t = sqrt(max(1.0 - ny * ny, 0.0));
            let k = nx / max(t, 1e-30);
            let t1 = (t - 0.5) * 2.0;
            let alfa = (1.0 - k) * 0.5;
            let beta = 1.0 - alfa;
            let dx = alfa * (rr - 1.0);
            let k1 = alfa * rr + beta;
            return vec3<f32>(
                (t1 * k1 + dx) * inv,
                sqrt(max(1.0 - t1 * t1, 0.0)) * k1 * inv,
                p.z,
            );
        } else {
            return vec3<f32>(
                (nx * (1.0 - rr) + rr) * inv,
                (ny * (1.0 - rr)) * inv,
                p.z,
            );
        }
    } else if (outside > 0.5) {
        return p;
    }
    return vec3<f32>(0.0, 0.0, p.z);
}
"#),
};
