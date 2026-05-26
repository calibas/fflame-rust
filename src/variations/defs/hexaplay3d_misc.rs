//! hexaplay3D (Larry Berlin, September 2009)
//!
//! Builds an hexagonal structure for snowflake designs. Sequentially
//! places points at the 6 vertices of a hexagon (rswtch ≤ 1) or the 3
//! vertices of an inscribed triangle (rswtch > 1), cycling through them
//! with `fcycle`/`bcycle` counters. `majp` controls whether all points
//! sit on one Z plane (≤ 1) or split into two planes (> 1) separated by
//! `±boost` along Z.
//!
//! cpp does X/Y as **replacement** assignments rather than +=:
//!   FPx = (FPx + FTx) · scale + weight · seg60x[loc]
//!
//! In our additive model (result += weight · body), this requires the
//! `body = (desired_value − accum) / weight` workaround, which uses
//! `needs_transform` to read the weight and `needs_accum` to read the
//! current accumulator. Z stays additive (cpp uses `FPz +=`).
//!
//! 3 user params (majp, scale, zlift) + 3 state slots (rswtch, fcycle,
//! bcycle). Custom thread-init seeds `rswtch ∈ {0, 1, 2}` from rng;
//! cycle counters start at 0.
//!
//! Source: `output/jwildfire-vars/output/hexaplay3D.cpp`.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Hexagonal vertex sequencer for snowflake designs. Places successive
/// iterations at the 6 vertices of a unit hexagon (when internal state
/// `rswtch ≤ 1`) or the 3 vertices of an inscribed triangle (when `rswtch >
/// 1`), cycling through them with internal `fcycle`/`bcycle` counters and
/// re-randomizing `rswtch ∈ {0, 1, 2}` each time a cycle completes. `majp`
/// controls Z-plane behavior: `|majp| ≤ 1` puts all points on a single Z
/// plane; `|majp| > 1` splits into two planes separated by `±(|majp| - 1) ·
/// 0.5` along Z, sign picked randomly per iteration. Uses `needs_accum +
/// needs_transform` to implement cpp's replacement-style FPx/FPy via the
/// `(desired - accum) / weight` workaround.
///
/// # Authors
/// - Larry Berlin
pub static HEXAPLAY_3D: VariationDef = VariationDef {
    name: "hexaplay3D",
    display_name: "Hexaplay 3D",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("majp", "Major Plane", unlimited_float, 1.0, -10.0, 10.0, "Major-plane threshold for Z behavior. `|majp| ≤ 1` = all points on a single Z plane (no Z split). `|majp| > 1` = points split into two planes separated by `±(|majp| - 1) · 0.5` along Z; sign picked randomly per iteration. Unused in 2D mode (Z param)."),
        param!("scale", "Scale", unlimited_float, 0.25, -10.0, 10.0, "Input-blend scale. Internally pre-multiplied by 0.5; the X/Y output is `(accum · (scale - 1) + p · scale) / weight + vertex_offset`."),
        param!("zlift", "Z Lift", unlimited_float, 0.25, -10.0, 10.0, "Z input scale: `oz = p.z · 0.5 · zlift / weight`. Unused in 2D mode."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 3,
    wgsl_state_init: Some(
        "        let r = rng_nextf(&rng);\n\
         \x20       set_state(xform_id, variation_id, 0u, floor(r * 3.0));\n\
         \x20       set_state(xform_id, variation_id, 1u, 0.0);\n\
         \x20       set_state(xform_id, variation_id, 2u, 0.0);"
    ),
    needs_accum: true,
    wgsl_2d: r#"
fn hex_seg60_2d(loc: u32) -> vec2<f32> {
    let hlift = 0.86602540378443864;
    switch (loc) {
        case 0u: { return vec2<f32>(1.0, 0.0); }
        case 1u: { return vec2<f32>(0.5, hlift); }
        case 2u: { return vec2<f32>(-0.5, hlift); }
        case 3u: { return vec2<f32>(-1.0, 0.0); }
        case 4u: { return vec2<f32>(-0.5, -hlift); }
        default: { return vec2<f32>(0.5, -hlift); }
    }
}

fn hex_seg120_2d(loc: u32) -> vec2<f32> {
    let hlift = 0.86602540378443864;
    switch (loc) {
        case 0u: { return vec2<f32>(1.0, 0.0); }
        case 1u: { return vec2<f32>(-0.5, hlift); }
        default: { return vec2<f32>(-0.5, -hlift); }
    }
}

fn variation_hexaplay3D(p: vec2<f32>, accum: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let majp = get_param(xform_id, variation_id, 0u);
    let scale_param = get_param(xform_id, variation_id, 1u);

    var rswtch = get_state(xform_id, variation_id, 0u);
    var fcycle = get_state(xform_id, variation_id, 1u);
    var bcycle = get_state(xform_id, variation_id, 2u);

    if (fcycle > 5.0) {
        fcycle = 0.0;
        rswtch = floor(rng_nextf(rng) * 3.0);
    }
    if (bcycle > 2.0) {
        bcycle = 0.0;
        rswtch = floor(rng_nextf(rng) * 3.0);
    }

    let weight = transforms[xform_id].variations[variation_id];
    let safe_w = select(weight, 1e-30, abs(weight) < 1e-30);
    let scale = scale_param * 0.5;

    var ox = 0.0;
    var oy = 0.0;
    if (rswtch <= 1.0) {
        let loc = u32(clamp(fcycle, 0.0, 5.0));
        let v = hex_seg60_2d(loc);
        ox = (accum.x * (scale - 1.0) + p.x * scale) / safe_w + v.x;
        oy = (accum.y * (scale - 1.0) + p.y * scale) / safe_w + v.y;
        fcycle = fcycle + 1.0;
    } else {
        let loc = u32(clamp(bcycle, 0.0, 2.0));
        let v = hex_seg120_2d(loc);
        ox = (accum.x * (scale - 1.0) + p.x * scale) / safe_w + v.x;
        oy = (accum.y * (scale - 1.0) + p.y * scale) / safe_w + v.y;
        bcycle = bcycle + 1.0;
    }

    set_state(xform_id, variation_id, 0u, rswtch);
    set_state(xform_id, variation_id, 1u, fcycle);
    set_state(xform_id, variation_id, 2u, bcycle);
    return vec2<f32>(ox, oy);
}
"#,
    wgsl_3d: Some(r#"
fn hex_seg60_3d(loc: u32) -> vec2<f32> {
    let hlift = 0.86602540378443864;
    switch (loc) {
        case 0u: { return vec2<f32>(1.0, 0.0); }
        case 1u: { return vec2<f32>(0.5, hlift); }
        case 2u: { return vec2<f32>(-0.5, hlift); }
        case 3u: { return vec2<f32>(-1.0, 0.0); }
        case 4u: { return vec2<f32>(-0.5, -hlift); }
        default: { return vec2<f32>(0.5, -hlift); }
    }
}

fn hex_seg120_3d(loc: u32) -> vec2<f32> {
    let hlift = 0.86602540378443864;
    switch (loc) {
        case 0u: { return vec2<f32>(1.0, 0.0); }
        case 1u: { return vec2<f32>(-0.5, hlift); }
        default: { return vec2<f32>(-0.5, -hlift); }
    }
}

fn variation_hexaplay3D(p: vec3<f32>, accum: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let majp = get_param(xform_id, variation_id, 0u);
    let scale_param = get_param(xform_id, variation_id, 1u);
    let zlift = get_param(xform_id, variation_id, 2u);

    var rswtch = get_state(xform_id, variation_id, 0u);
    var fcycle = get_state(xform_id, variation_id, 1u);
    var bcycle = get_state(xform_id, variation_id, 2u);

    if (fcycle > 5.0) {
        fcycle = 0.0;
        rswtch = floor(rng_nextf(rng) * 3.0);
    }
    if (bcycle > 2.0) {
        bcycle = 0.0;
        rswtch = floor(rng_nextf(rng) * 3.0);
    }

    let weight = transforms[xform_id].variations[variation_id];
    let safe_w = select(weight, 1e-30, abs(weight) < 1e-30);
    let scale = scale_param * 0.5;

    var pos_neg = 1.0;
    if (rng_nextf(rng) < 0.5) { pos_neg = -1.0; }

    let abmajp = abs(majp);
    var oz_extra = 0.0;
    if (abmajp > 1.0) {
        let boost = (abmajp - 1.0) * 0.5;
        oz_extra = pos_neg * boost;
    }

    var ox = 0.0;
    var oy = 0.0;
    if (rswtch <= 1.0) {
        let loc = u32(clamp(fcycle, 0.0, 5.0));
        let v = hex_seg60_3d(loc);
        ox = (accum.x * (scale - 1.0) + p.x * scale) / safe_w + v.x;
        oy = (accum.y * (scale - 1.0) + p.y * scale) / safe_w + v.y;
        fcycle = fcycle + 1.0;
    } else {
        let loc = u32(clamp(bcycle, 0.0, 2.0));
        let v = hex_seg120_3d(loc);
        ox = (accum.x * (scale - 1.0) + p.x * scale) / safe_w + v.x;
        oy = (accum.y * (scale - 1.0) + p.y * scale) / safe_w + v.y;
        bcycle = bcycle + 1.0;
    }

    let oz = (p.z * 0.5 * zlift + oz_extra) / safe_w;

    set_state(xform_id, variation_id, 0u, rswtch);
    set_state(xform_id, variation_id, 1u, fcycle);
    set_state(xform_id, variation_id, 2u, bcycle);
    return vec3<f32>(ox, oy, oz);
}
"#),
};
