//! Stub-bucket recoveries — second file
//!
//! Four more `unported_stub` recoveries (cpp `PluginVarCalc` empty;
//! formulas live in the embedded Java comment block):
//!
//!   - `disc3`        (Stefanov)             — disc with 8 tunable knobs
//!   - `projective`   (eralex61)             — linear-fractional projective transform
//!   - `tqmirror`     (Anderson / Stefanov)  — quadrant-based fold-mirror
//!   - `intersection` (Stefanov)             — random row-or-column tile blur
//!
//! Sources: each variation's `.cpp` file in
//! `output/jwildfire-vars/output/`.
//!
//! Notes on faithfulness:
//!   - `disc3` and `projective` factor VVAR cleanly through the outer
//!     multiplier.
//!   - `tqmirror` uses VVAR (`pAmount`) as a comparison threshold AND
//!     in additive offsets (`x + pAmount · f`). Body uses `needs_transform`
//!     to read the weight and divides by it on the way out.
//!   - `intersection`'s output lines lack VVAR. Same divide-out pattern.
//!
//! Skipped from the originally-considered set:
//!   - `mobius_strip` — 10 user params + multi-mode init (`width_mode`,
//!     `radial_mode`) with rotxSin/cos and rotySin/cos plus xmin/xmax/
//!     ymin/ymax. Tractable but a focused single-port batch.
//!   - `pre_recip` — uses Java's `Complex` class (Recip, Inc, Dec, Div,
//!     AsinH, AcosH, ...) with 15 user params. Translatable but the
//!     Complex math library would be substantial.
//!   - `sunvoroni` (351 lines) — Voronoi distance computation with
//!     fixed-size point arrays.
//!   - `gridout3d` — 26 user params; exceeds the 16-slot per-variation
//!     budget. Documented under [16-slot budget overflow].

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// disc3: disc with 8 tunable knobs (Stefanov)
//   rPI = π · sqrt(x²·d·e + y²·f·g)
//   sinr = sin(rPI) · a
//   cosr = cos(rPI) · b
//   r = atan2(y, x) / π · c       (VVAR factored through outer)
//   out = (sinr·h·r, cosr·h·r)
// =============================================================================
pub static DISC3: VariationDef = VariationDef {
    name: "disc3",
    display_name: "Disc 3",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("a", "A", unlimited_float, 1.0, -10.0, 10.0),
        param!("b", "B", unlimited_float, 1.0, -10.0, 10.0),
        param!("c", "C", unlimited_float, 1.0, -10.0, 10.0),
        param!("d", "D", unlimited_float, 1.0, -10.0, 10.0),
        param!("e", "E", unlimited_float, 1.0, -10.0, 10.0),
        param!("f", "F", unlimited_float, 1.0, -10.0, 10.0),
        param!("g", "G", unlimited_float, 1.0, -10.0, 10.0),
        param!("h", "H", unlimited_float, 1.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_disc3(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    let e = get_param(xform_id, variation_id, 4u);
    let f = get_param(xform_id, variation_id, 5u);
    let g = get_param(xform_id, variation_id, 6u);
    let h = get_param(xform_id, variation_id, 7u);
    let pi = 3.14159265358979;
    let inv_pi = 0.3183098861837907;

    let r_pi = pi * sqrt(max(p.x * d * p.x * e + p.y * f * p.y * g, 0.0));
    let sinr = sin(r_pi) * a;
    let cosr = cos(r_pi) * b;
    let r = atan2(p.y, p.x) * inv_pi * c;
    return vec2<f32>(sinr * h * r, cosr * h * r);
}
"#,
    wgsl_3d: Some(r#"
fn variation_disc3(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    let e = get_param(xform_id, variation_id, 4u);
    let f = get_param(xform_id, variation_id, 5u);
    let g = get_param(xform_id, variation_id, 6u);
    let h = get_param(xform_id, variation_id, 7u);
    let pi = 3.14159265358979;
    let inv_pi = 0.3183098861837907;

    let r_pi = pi * sqrt(max(p.x * d * p.x * e + p.y * f * p.y * g, 0.0));
    let sinr = sin(r_pi) * a;
    let cosr = cos(r_pi) * b;
    let r = atan2(p.y, p.x) * inv_pi * c;
    return vec3<f32>(sinr * h * r, cosr * h * r, p.z);
}
"#),
};

// =============================================================================
// projective: linear-fractional projective transform (eralex61)
//   U = A·x + B·y + C
//   out = ((A1·x + B1·y + C1) / U, (A2·x + B2·y + C2) / U)
// =============================================================================
pub static PROJECTIVE: VariationDef = VariationDef {
    name: "projective",
    display_name: "Projective",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("a", "A", unlimited_float, 0.0, -10.0, 10.0),
        param!("b", "B", unlimited_float, 0.0, -10.0, 10.0),
        param!("c", "C", unlimited_float, 1.0, -10.0, 10.0),
        param!("a1", "A1", unlimited_float, 1.0, -10.0, 10.0),
        param!("b1", "B1", unlimited_float, 0.0, -10.0, 10.0),
        param!("c1", "C1", unlimited_float, 0.0, -10.0, 10.0),
        param!("a2", "A2", unlimited_float, 0.0, -10.0, 10.0),
        param!("b2", "B2", unlimited_float, 1.0, -10.0, 10.0),
        param!("c2", "C2", unlimited_float, 0.0, -10.0, 10.0),
    ],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_projective(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let a1 = get_param(xform_id, variation_id, 3u);
    let b1 = get_param(xform_id, variation_id, 4u);
    let c1 = get_param(xform_id, variation_id, 5u);
    let a2 = get_param(xform_id, variation_id, 6u);
    let b2 = get_param(xform_id, variation_id, 7u);
    let c2 = get_param(xform_id, variation_id, 8u);

    let u = a * p.x + b * p.y + c;
    let inv_u = 1.0 / select(u, 1e-30, abs(u) < 1e-30);
    return vec2<f32>(
        (a1 * p.x + b1 * p.y + c1) * inv_u,
        (a2 * p.x + b2 * p.y + c2) * inv_u,
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_projective(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let a1 = get_param(xform_id, variation_id, 3u);
    let b1 = get_param(xform_id, variation_id, 4u);
    let c1 = get_param(xform_id, variation_id, 5u);
    let a2 = get_param(xform_id, variation_id, 6u);
    let b2 = get_param(xform_id, variation_id, 7u);
    let c2 = get_param(xform_id, variation_id, 8u);

    let u = a * p.x + b * p.y + c;
    let inv_u = 1.0 / select(u, 1e-30, abs(u) < 1e-30);
    return vec3<f32>(
        (a1 * p.x + b1 * p.y + c1) * inv_u,
        (a2 * p.x + b2 * p.y + c2) * inv_u,
        p.z,
    );
}
"#),
};

// =============================================================================
// tqmirror: quadrant-based fold-mirror (Anderson / Stefanov)
//   if d+x < 0 OR e+y < 0:
//     type=0:  out = (y, x)         (no VVAR — divide-out)
//     type=1:  out = (x, y)
//   else if x < 0 AND y < 0:
//     out = (x + w·f, y + w·g)
//   else if x < w AND y < a AND x+b > 0 AND y+c > 0:
//     out = (-y·h, -x·h)
//   else:
//     out = (x, y)                  (no VVAR — divide-out)
// =============================================================================
pub static TQMIRROR: VariationDef = VariationDef {
    name: "tqmirror",
    display_name: "TQ Mirror",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("a", "A", unlimited_float, 1.0, -10.0, 10.0),
        param!("b", "B", unlimited_float, 1.0, -10.0, 10.0),
        param!("c", "C", unlimited_float, 1.0, -10.0, 10.0),
        param!("d", "D", unlimited_float, 1.0, -10.0, 10.0),
        param!("e", "E", unlimited_float, 1.0, -10.0, 10.0),
        param!("f", "F", unlimited_float, 1.0, -10.0, 10.0),
        param!("g", "G", unlimited_float, 1.0, -10.0, 10.0),
        param!("h", "H", unlimited_float, 1.0, -10.0, 10.0),
        param!("type", "Type", int, 0.0, 0.0, 1.0),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_tqmirror(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    let e = get_param(xform_id, variation_id, 4u);
    let f = get_param(xform_id, variation_id, 5u);
    let g = get_param(xform_id, variation_id, 6u);
    let h = get_param(xform_id, variation_id, 7u);
    let type_p = get_param(xform_id, variation_id, 8u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    var fx: f32;
    var fy: f32;
    if (d + p.x < 0.0 || e + p.y < 0.0) {
        if (type_p != 0.0) {
            fx = p.x;
            fy = p.y;
        } else {
            fx = p.y;
            fy = p.x;
        }
    } else if (p.x < 0.0 && p.y < 0.0) {
        fx = p.x + w * f;
        fy = p.y + w * g;
    } else if (p.x < w && p.y < a && p.x + b > 0.0 && p.y + c > 0.0) {
        fx = -p.y * h;
        fy = -p.x * h;
    } else {
        fx = p.x;
        fy = p.y;
    }
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_tqmirror(p: vec3<f32>, xform_id: u32, variation_id: u32) -> vec3<f32> {
    let a = get_param(xform_id, variation_id, 0u);
    let b = get_param(xform_id, variation_id, 1u);
    let c = get_param(xform_id, variation_id, 2u);
    let d = get_param(xform_id, variation_id, 3u);
    let e = get_param(xform_id, variation_id, 4u);
    let f = get_param(xform_id, variation_id, 5u);
    let g = get_param(xform_id, variation_id, 6u);
    let h = get_param(xform_id, variation_id, 7u);
    let type_p = get_param(xform_id, variation_id, 8u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    var fx: f32;
    var fy: f32;
    if (d + p.x < 0.0 || e + p.y < 0.0) {
        if (type_p != 0.0) {
            fx = p.x;
            fy = p.y;
        } else {
            fx = p.y;
            fy = p.x;
        }
    } else if (p.x < 0.0 && p.y < 0.0) {
        fx = p.x + w * f;
        fy = p.y + w * g;
    } else if (p.x < w && p.y < a && p.x + b > 0.0 && p.y + c > 0.0) {
        fx = -p.y * h;
        fy = -p.x * h;
    } else {
        fx = p.x;
        fy = p.y;
    }
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#),
};

// =============================================================================
// intersection: random row-or-column tile blur (Stefanov)
//   Init: xr1 = xmod2 · xmod1;  yr1 = ymod2 · ymod1
//   if rand < 0.5:
//     row branch: x' = xtilesize · (x + round(±xwidth · log(rand)))
//     y' = piecewise fmod-fold around xmod1 with xheight · ...
//   else:
//     column branch: similar but on Y
//   (no VVAR multiplier on the output lines — divide-out)
// =============================================================================
pub static INTERSECTION: VariationDef = VariationDef {
    name: "intersection",
    display_name: "Intersection",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("xwidth", "X Width", unlimited_float, 5.0, -50.0, 50.0),
        param!("xtilesize", "X Tile", unlimited_float, 0.5, -10.0, 10.0),
        param!("xmod1", "X Mod 1", unlimited_float, 0.3, -10.0, 10.0),
        param!("xmod2", "X Mod 2", unlimited_float, 1.0, -10.0, 10.0),
        param!("xheight", "X Height", unlimited_float, 0.5, -10.0, 10.0),
        param!("yheight", "Y Height", unlimited_float, 5.0, -50.0, 50.0),
        param!("ytilesize", "Y Tile", unlimited_float, 0.5, -10.0, 10.0),
        param!("ymod1", "Y Mod 1", unlimited_float, 0.3, -10.0, 10.0),
        param!("ymod2", "Y Mod 2", unlimited_float, 1.0, -10.0, 10.0),
        param!("ywidth", "Y Width", unlimited_float, 0.5, -10.0, 10.0),
    ],
    needs_transform: true,
    writes_color: false,
    // 2 derived values at slots 10..12:
    //   10: xr1  (xmod2 · xmod1)
    //   11: yr1  (ymod2 · ymod1)
    init_param_count: 2,
    wgsl_init: Some(r#"
fn init_intersection(user: array<f32, 10>) -> array<f32, 2> {
    var out: array<f32, 2>;
    out[0] = user[3] * user[2];   // xmod2 * xmod1
    out[1] = user[8] * user[7];   // ymod2 * ymod1
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_intersection(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let xwidth = get_param(xform_id, variation_id, 0u);
    let xtilesize = get_param(xform_id, variation_id, 1u);
    let xmod1 = get_param(xform_id, variation_id, 2u);
    let xheight = get_param(xform_id, variation_id, 4u);
    let yheight = get_param(xform_id, variation_id, 5u);
    let ytilesize = get_param(xform_id, variation_id, 6u);
    let ymod1 = get_param(xform_id, variation_id, 7u);
    let ywidth = get_param(xform_id, variation_id, 9u);
    let xr1 = get_param(xform_id, variation_id, 10u);
    let yr1 = get_param(xform_id, variation_id, 11u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let safe_xr1 = select(xr1, 1e-30, abs(xr1) < 1e-30);
    let safe_yr1 = select(yr1, 1e-30, abs(yr1) < 1e-30);

    var fx: f32;
    var fy: f32;
    if (rng_nextf(rng) < 0.5) {
        let x_sign_choice = select(-xwidth, xwidth, rng_nextf(rng) < 0.5);
        let log_rand = log(max(rng_nextf(rng), 1e-30));
        fx = xtilesize * (p.x + round(x_sign_choice * log_rand));
        if (p.y > xmod1) {
            let v = p.y + xmod1;
            fy = xheight * (-xmod1 + (v - floor(v / safe_xr1) * safe_xr1));
        } else if (p.y < -xmod1) {
            let v = xmod1 - p.y;
            fy = xheight * (xmod1 - (v - floor(v / safe_xr1) * safe_xr1));
        } else {
            fy = xheight * p.y;
        }
    } else {
        let y_sign_choice = select(-yheight, yheight, rng_nextf(rng) < 0.5);
        let log_rand = log(max(rng_nextf(rng), 1e-30));
        fy = ytilesize * (p.y + round(y_sign_choice * log_rand));
        if (p.x > ymod1) {
            let v = p.x + ymod1;
            fx = ywidth * (-ymod1 + (v - floor(v / safe_yr1) * safe_yr1));
        } else if (p.x < -ymod1) {
            let v = ymod1 - p.x;
            fx = ywidth * (ymod1 - (v - floor(v / safe_yr1) * safe_yr1));
        } else {
            fx = ywidth * p.x;
        }
    }
    return vec2<f32>(fx * inv_w, fy * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_intersection(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let xwidth = get_param(xform_id, variation_id, 0u);
    let xtilesize = get_param(xform_id, variation_id, 1u);
    let xmod1 = get_param(xform_id, variation_id, 2u);
    let xheight = get_param(xform_id, variation_id, 4u);
    let yheight = get_param(xform_id, variation_id, 5u);
    let ytilesize = get_param(xform_id, variation_id, 6u);
    let ymod1 = get_param(xform_id, variation_id, 7u);
    let ywidth = get_param(xform_id, variation_id, 9u);
    let xr1 = get_param(xform_id, variation_id, 10u);
    let yr1 = get_param(xform_id, variation_id, 11u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let safe_xr1 = select(xr1, 1e-30, abs(xr1) < 1e-30);
    let safe_yr1 = select(yr1, 1e-30, abs(yr1) < 1e-30);

    var fx: f32;
    var fy: f32;
    if (rng_nextf(rng) < 0.5) {
        let x_sign_choice = select(-xwidth, xwidth, rng_nextf(rng) < 0.5);
        let log_rand = log(max(rng_nextf(rng), 1e-30));
        fx = xtilesize * (p.x + round(x_sign_choice * log_rand));
        if (p.y > xmod1) {
            let v = p.y + xmod1;
            fy = xheight * (-xmod1 + (v - floor(v / safe_xr1) * safe_xr1));
        } else if (p.y < -xmod1) {
            let v = xmod1 - p.y;
            fy = xheight * (xmod1 - (v - floor(v / safe_xr1) * safe_xr1));
        } else {
            fy = xheight * p.y;
        }
    } else {
        let y_sign_choice = select(-yheight, yheight, rng_nextf(rng) < 0.5);
        let log_rand = log(max(rng_nextf(rng), 1e-30));
        fy = ytilesize * (p.y + round(y_sign_choice * log_rand));
        if (p.x > ymod1) {
            let v = p.x + ymod1;
            fx = ywidth * (-ymod1 + (v - floor(v / safe_yr1) * safe_yr1));
        } else if (p.x < -ymod1) {
            let v = ymod1 - p.x;
            fx = ywidth * (ymod1 - (v - floor(v / safe_yr1) * safe_yr1));
        } else {
            fx = ywidth * p.x;
        }
    }
    return vec3<f32>(fx * inv_w, fy * inv_w, p.z);
}
"#),
};
