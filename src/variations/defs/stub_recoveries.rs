//! Stub-bucket recoveries (cpp PluginVarCalc empty; Java has the formula)
//!
//! Six small variations from the upstream `unported_stub` /
//! porter-omitted bucket. Each cpp port has an empty
//! `PluginVarCalc` body or a hard-coded zero-param `APO_VARIABLES`
//! list — the formulas live in the embedded Java comment block at
//! the bottom of each file. We translate directly from the Java.
//!
//!   - `bsplit`     (Raykoid666)   — recovered 2 user params (x, y) from Java
//!   - `cylinder2`  (Maschke)      — `x / sqrt(x²+1)` lengthwise warp
//!   - `eclipse`    (Faber)        — eclipse-shaped X clamp; 1 user param
//!   - `lozi`       (TyrantWave)   — Lozi strange attractor; 3 user params
//!   - `pulse`      (Maschke)      — sine-wave additive distortion; 4 params
//!   - `hypershift` (Zy0rg/Stefanov) — Möbius-style shift+stretch; 2 params
//!
//! Sources: each variation's `.cpp` file under `output/jwildfire-vars/output/`.
//!
//! Notes on faithfulness:
//!   - `bsplit`: cpp `APO_VARIABLES()` is empty (porter-omitted); Java
//!     has `x` and `y` as user params. We recover them. cpp's "doHide"
//!     branch on degenerate input (`FTx + x == 0` or `== π`) returns
//!     0 contribution.
//!   - `eclipse`: VVAR appears as a comparison threshold (`|y| <=
//!     pAmount`, `|x| >= sqrt(pAmount² − y²)`) and as the X "shift"
//!     component (`shift · pAmount`). Body uses `needs_transform` to
//!     read w; output factors cleanly through the outer multiplier in
//!     the simple branches.
//!   - `hypershift`: the `+ shift` add on the X output line lacks VVAR
//!     — needs `needs_transform` + divide-out (same pattern as `onion`,
//!     `target_sp`, etc.).
//!   - `cylinder2`, `lozi`, `pulse`: VVAR strictly as outer multiplier;
//!     factor cleanly with no `needs_transform` required.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// bsplit: tan/sin split (Raykoid666 / Anderson, recovered from Java)
//   if FTx + x == 0 or FTx + x == π:  contribute 0 (cpp's "doHide")
//   else:
//     out_x = cos(FTy + y) / tan(FTx + x)
//     out_y = (-FTy + y) / sin(FTx + x)
// =============================================================================
pub static BSPLIT: VariationDef = VariationDef {
    name: "bsplit",
    display_name: "BSplit",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("x", "X shift", unlimited_float, 0.0, -10.0, 10.0),
        param!("y", "Y shift", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_bsplit(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let x_shift = get_param(xform_id, variation_id, 0u);
    let y_shift = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    let arg = p.x + x_shift;
    let s = sin(arg);
    if (s == 0.0 || arg == pi) {
        return vec2<f32>(0.0, 0.0);
    }
    let c = cos(arg);
    return vec2<f32>(
        cos(p.y + y_shift) * c / s,
        (-p.y + y_shift) / s,
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_bsplit(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let x_shift = get_param(xform_id, variation_id, 0u);
    let y_shift = get_param(xform_id, variation_id, 1u);
    let pi = 3.14159265358979;
    let arg = p.x + x_shift;
    let s = sin(arg);
    if (s == 0.0 || arg == pi) {
        return vec3<f32>(0.0, 0.0, p.z);
    }
    let c = cos(arg);
    return vec3<f32>(
        cos(p.y + y_shift) * c / s,
        (-p.y + y_shift) / s,
        p.z,
    );
}
"#),
};

// =============================================================================
// cylinder2: lengthwise unit-cylinder warp
//   out = (x / sqrt(x² + 1), y)
// =============================================================================
pub static CYLINDER2: VariationDef = VariationDef {
    name: "cylinder2",
    display_name: "Cylinder 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_cylinder2(p: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(p.x / sqrt(p.x * p.x + 1.0), p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_cylinder2(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(p.x / sqrt(p.x * p.x + 1.0), p.y, p.z);
}
"#),
};

// =============================================================================
// eclipse: eclipse-shaped X-axis clamp (Faber)
//   if |y| <= w:
//     c2 = sqrt(w² − y²)
//     if |x| <= c2:
//       x_shifted = x + shift · w
//       if |x_shifted| >= c2:
//         out_x = -x       (flip back)
//       else:
//         out_x = x_shifted
//     else:
//       out_x = x
//     out_y = y
//   else:
//     out_x = x;  out_y = y      (pass-through outside the eclipse)
// =============================================================================
pub static ECLIPSE: VariationDef = VariationDef {
    name: "eclipse",
    display_name: "Eclipse",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("shift", "Shift", float, 0.0, -2.0, 2.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_eclipse(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let shift = get_param(xform_id, variation_id, 0u);
    let w = transforms[xform_id].variations[variation_id];

    var ox = p.x;
    if (abs(p.y) <= w) {
        let c2 = sqrt(max(w * w - p.y * p.y, 0.0));
        if (abs(p.x) <= c2) {
            let x_shifted = p.x + shift * w;
            if (abs(x_shifted) >= c2) {
                ox = -p.x;
            } else {
                ox = x_shifted;
            }
        }
    }
    return vec2<f32>(ox, p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_eclipse(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let shift = get_param(xform_id, variation_id, 0u);
    let w = transforms[xform_id].variations[variation_id];

    var ox = p.x;
    if (abs(p.y) <= w) {
        let c2 = sqrt(max(w * w - p.y * p.y, 0.0));
        if (abs(p.x) <= c2) {
            let x_shifted = p.x + shift * w;
            if (abs(x_shifted) >= c2) {
                ox = -p.x;
            } else {
                ox = x_shifted;
            }
        }
    }
    return vec3<f32>(ox, p.y, p.z);
}
"#),
};

// =============================================================================
// lozi: Lozi strange attractor (TyrantWave)
//   out = (c − a·|x| + y, b · x)
// =============================================================================
pub static LOZI: VariationDef = VariationDef {
    name: "lozi",
    display_name: "Lozi",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("a", "A", unlimited_float, 0.5, -10.0, 10.0),
        param!("b", "B", unlimited_float, 1.0, -10.0, 10.0),
        param!("c", "C", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_lozi(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    return vec2<f32>(c - a * abs(p.x) + p.y, b * p.x);
}
"#,
    wgsl_3d: Some(r#"
fn variation_lozi(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    return vec3<f32>(c - a * abs(p.x) + p.y, b * p.x, p.z);
}
"#),
};

// =============================================================================
// pulse: sine-wave additive distortion (Maschke)
//   out_x = x + scalex · sin(x · freqx)
//   out_y = y + scaley · sin(y · freqy)
// =============================================================================
pub static PULSE: VariationDef = VariationDef {
    name: "pulse",
    display_name: "Pulse",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("freqx", "Freq X", unlimited_float, 2.0, -50.0, 50.0),
        param!("freqy", "Freq Y", unlimited_float, 2.0, -50.0, 50.0),
        param!("scalex", "Scale X", unlimited_float, 1.0, -10.0, 10.0),
        param!("scaley", "Scale Y", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_pulse(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let freqx = get_param(xform_id, variation_id, 0u);
    let freqy = get_param(xform_id, variation_id, 1u);
    let scalex = get_param(xform_id, variation_id, 2u);
    let scaley = get_param(xform_id, variation_id, 3u);
    return vec2<f32>(
        p.x + scalex * sin(p.x * freqx),
        p.y + scaley * sin(p.y * freqy),
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_pulse(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let freqx = get_param(xform_id, variation_id, 0u);
    let freqy = get_param(xform_id, variation_id, 1u);
    let scalex = get_param(xform_id, variation_id, 2u);
    let scaley = get_param(xform_id, variation_id, 3u);
    return vec3<f32>(
        p.x + scalex * sin(p.x * freqx),
        p.y + scaley * sin(p.y * freqy),
        p.z,
    );
}
"#),
};

// =============================================================================
// hypershift: Möbius-style shift + stretch (Zy0rg / Stefanov)
//   scale = 1 − shift²
//   rad   = 1 / (x² + y²)
//   x' = rad·x + shift
//   y' = rad·y
//   rad   = w · scale / (x'² + y'²)
//   FPx += rad · x' + shift          (the trailing `+ shift` lacks VVAR
//                                     — divide-out via needs_transform)
//   FPy += rad · y' · stretch
// =============================================================================
pub static HYPERSHIFT: VariationDef = VariationDef {
    name: "hypershift",
    display_name: "Hypershift",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("shift", "Shift", unlimited_float, 2.0, -10.0, 10.0),
        param!("stretch", "Stretch", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: r#"
fn variation_hypershift(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let shift = get_param(xform_id, variation_id, 0u);
    let stretch = get_param(xform_id, variation_id, 1u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let scale = 1.0 - shift * shift;
    let rad1 = 1.0 / max(p.x * p.x + p.y * p.y, 1e-30);
    let x1 = rad1 * p.x + shift;
    let y1 = rad1 * p.y;
    let rad2 = w * scale / max(x1 * x1 + y1 * y1, 1e-30);
    let fx = rad2 * x1 + shift;
    let fy = rad2 * y1 * stretch;
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_hypershift(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let shift = get_param(xform_id, variation_id, 0u);
    let stretch = get_param(xform_id, variation_id, 1u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let scale = 1.0 - shift * shift;
    let rad1 = 1.0 / max(p.x * p.x + p.y * p.y, 1e-30);
    let x1 = rad1 * p.x + shift;
    let y1 = rad1 * p.y;
    let rad2 = w * scale / max(x1 * x1 + y1 * y1, 1e-30);
    let fx = rad2 * x1 + shift;
    let fy = rad2 * y1 * stretch;
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#),
};
