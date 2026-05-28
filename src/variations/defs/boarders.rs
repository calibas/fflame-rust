//! Boarders / border-tile family
//!
//! Cell-grid border-warp variations: each input point is decomposed
//! into a rounded "cell index" and an "offset within the cell", with
//! various rules for re-mapping the offset:
//!   - `boarders`       (Apophysis pack)             — coin-flip + diag
//!   - `boarders2`      (Xyrus02)                    — 3-knob version
//!   - `pre_boarders2`  (Xyrus02)                    — pre-phase variant
//!   - `splitbrdr`      (FracFx)                     — bubble + border combo
//!
//! Sources:
//!   - output/jwildfire-vars/output/boarders.cpp
//!   - output/jwildfire-vars/output/boarders2.cpp
//!   - output/jwildfire-vars/output/pre_boarders2.cpp
//!   - output/jwildfire-vars/output/splitbrdr.cpp
//!
//! Notes on faithfulness:
//!   - `boarders` and `boarders2` have VVAR strictly as outer
//!     multiplier — clean factor through outer-multiplier convention.
//!   - `pre_boarders2` runs in pre-phase (no outer multiplier), so
//!     the body reads the per-variation weight via `needs_transform`
//!     and applies VVAR directly. cpp uses `FTx = VVAR · stuff`
//!     (assignment, replacing `temp`), which is exactly how our
//!     pre-phase calling convention works.
//!   - `splitbrdr` mostly factors but ends with two extra lines
//!     `FPx += FTx · px;  FPy += FTy · py` that lack the VVAR
//!     multiplier. Body reads w via `needs_transform` and emits
//!     `cpp_output / w` so the outer × w restores the cpp result
//!     (same divide-out pattern as `onion` / `target_sp` /
//!     `truchet_fill`).

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// =============================================================================
// boarders: cell-grid coin-flip border (Apophysis pack)
// =============================================================================
/// Cell-grid border warp — each input point is decomposed into a cell
/// index and an offset within the cell. A coin flip per iteration either
/// shrinks the offset toward the cell center or pushes it toward the
/// nearest cell border.
///
/// # Authors
/// - Apophysis Plugin Pack
pub static BOARDERS: VariationDef = VariationDef {
    name: "boarders",
    aliases: &[],
    display_name: "Boarders",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[],
    needs_transform: false,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_boarders(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32> {
    let round_x = round(p.x);
    let round_y = round(p.y);
    let off_x = p.x - round_x;
    let off_y = p.y - round_y;

    if (rng_nextf(rng) >= 0.75) {
        return vec2<f32>(off_x * 0.5 + round_x, off_y * 0.5 + round_y);
    }
    if (abs(off_x) >= abs(off_y)) {
        if (off_x >= 0.0) {
            return vec2<f32>(
                off_x * 0.5 + round_x + 0.25,
                off_y * 0.5 + round_y + 0.25 * off_y / off_x,
            );
        }
        return vec2<f32>(
            off_x * 0.5 + round_x - 0.25,
            off_y * 0.5 + round_y - 0.25 * off_y / off_x,
        );
    }
    if (off_y >= 0.0) {
        return vec2<f32>(
            off_x * 0.5 + round_x + off_x / off_y * 0.25,
            off_y * 0.5 + round_y + 0.25,
        );
    }
    return vec2<f32>(
        off_x * 0.5 + round_x - off_x / off_y * 0.25,
        off_y * 0.5 + round_y - 0.25,
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_boarders(p: vec3<f32>, rng: ptr<function, RngState>) -> vec3<f32> {
    let round_x = round(p.x);
    let round_y = round(p.y);
    let off_x = p.x - round_x;
    let off_y = p.y - round_y;

    if (rng_nextf(rng) >= 0.75) {
        return vec3<f32>(off_x * 0.5 + round_x, off_y * 0.5 + round_y, p.z);
    }
    if (abs(off_x) >= abs(off_y)) {
        if (off_x >= 0.0) {
            return vec3<f32>(
                off_x * 0.5 + round_x + 0.25,
                off_y * 0.5 + round_y + 0.25 * off_y / off_x,
                p.z,
            );
        }
        return vec3<f32>(
            off_x * 0.5 + round_x - 0.25,
            off_y * 0.5 + round_y - 0.25 * off_y / off_x,
            p.z,
        );
    }
    if (off_y >= 0.0) {
        return vec3<f32>(
            off_x * 0.5 + round_x + off_x / off_y * 0.25,
            off_y * 0.5 + round_y + 0.25,
            p.z,
        );
    }
    return vec3<f32>(
        off_x * 0.5 + round_x - off_x / off_y * 0.25,
        off_y * 0.5 + round_y - 0.25,
        p.z,
    );
}
"#),
};

// =============================================================================
// boarders2: 3-knob cell-grid border (Xyrus02)
//   Init computes:
//     _c  = max(|c|, ε)
//     _cl = max(|left|, ε)  →  _c · _cl
//     _cr = max(|right|, ε) →  _c + (_c · _cr)
// =============================================================================
/// Variant of Boarders with 3 tunable knobs — scale factor `c` and per-
/// direction `left`/`right` offsets controlling the border-push behavior.
///
/// # Authors
/// - Xyrus02
pub static BOARDERS2: VariationDef = VariationDef {
    name: "boarders2",
    aliases: &[],
    display_name: "Boarders 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("c", "C", unlimited_float, 0.4, -5.0, 5.0, "Cell scale factor — how much the in-cell offset shrinks toward the cell center."),
        param!("left", "Left", unlimited_float, 0.65, -5.0, 5.0, "Border push-out distance — how far points get pushed toward the nearest cell border."),
        param!("right", "Right", unlimited_float, 0.35, -5.0, 5.0, "Threshold for border behavior vs pass-through. Higher = more points get pushed to borders."),
    ],
    needs_transform: false,
    writes_color: false,
    // 3 derived values at slots 3..6:
    //   3: _c   (max(|c|, ε))
    //   4: _cl  (_c · max(|left|, ε))
    //   5: _cr  (_c + _c · max(|right|, ε))
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_boarders2(user: array<f32, 3>) -> array<f32, 3> {
    let c0 = max(abs(user[0]), 1e-6);
    let cl0 = max(abs(user[1]), 1e-6);
    let cr0 = max(abs(user[2]), 1e-6);
    var out: array<f32, 3>;
    out[0] = c0;
    out[1] = c0 * cl0;
    out[2] = c0 + c0 * cr0;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_boarders2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let c = get_param(xform_id, variation_id, 3u);
    let cl = get_param(xform_id, variation_id, 4u);
    let cr = get_param(xform_id, variation_id, 5u);

    let round_x = round(p.x);
    let round_y = round(p.y);
    let off_x = p.x - round_x;
    let off_y = p.y - round_y;

    if (rng_nextf(rng) >= cr) {
        return vec2<f32>(off_x * c + round_x, off_y * c + round_y);
    }
    if (abs(off_x) >= abs(off_y)) {
        if (off_x >= 0.0) {
            return vec2<f32>(
                off_x * c + round_x + cl,
                off_y * c + round_y + cl * off_y / off_x,
            );
        }
        return vec2<f32>(
            off_x * c + round_x - cl,
            off_y * c + round_y - cl * off_y / off_x,
        );
    }
    if (off_y >= 0.0) {
        return vec2<f32>(
            off_x * c + round_x + off_x / off_y * cl,
            off_y * c + round_y + cl,
        );
    }
    return vec2<f32>(
        off_x * c + round_x - off_x / off_y * cl,
        off_y * c + round_y - cl,
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_boarders2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let c = get_param(xform_id, variation_id, 3u);
    let cl = get_param(xform_id, variation_id, 4u);
    let cr = get_param(xform_id, variation_id, 5u);

    let round_x = round(p.x);
    let round_y = round(p.y);
    let off_x = p.x - round_x;
    let off_y = p.y - round_y;

    if (rng_nextf(rng) >= cr) {
        return vec3<f32>(off_x * c + round_x, off_y * c + round_y, p.z);
    }
    if (abs(off_x) >= abs(off_y)) {
        if (off_x >= 0.0) {
            return vec3<f32>(
                off_x * c + round_x + cl,
                off_y * c + round_y + cl * off_y / off_x,
                p.z,
            );
        }
        return vec3<f32>(
            off_x * c + round_x - cl,
            off_y * c + round_y - cl * off_y / off_x,
            p.z,
        );
    }
    if (off_y >= 0.0) {
        return vec3<f32>(
            off_x * c + round_x + off_x / off_y * cl,
            off_y * c + round_y + cl,
            p.z,
        );
    }
    return vec3<f32>(
        off_x * c + round_x - off_x / off_y * cl,
        off_y * c + round_y - cl,
        p.z,
    );
}
"#),
};

// =============================================================================
// pre_boarders2: pre-phase form of boarders2 (Xyrus02)
//   Same init, same body — but pre-phase replaces `temp` with `VVAR · stuff`.
//   Body reads w via needs_transform and applies VVAR directly.
// =============================================================================
/// Pre-phase version of Boarders 2 — same border-warp math but applied
/// before the rest of the variations run.
///
/// # Authors
/// - Xyrus02
pub static PRE_BOARDERS2: VariationDef = VariationDef {
    name: "pre_boarders2",
    aliases: &[],
    display_name: "Pre Boarders 2",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Pre,
    needs_rng: true,
    parameters: &[
        param!("c", "C", unlimited_float, 0.4, -5.0, 5.0, "Cell scale factor — how much the in-cell offset shrinks toward the cell center."),
        param!("left", "Left", unlimited_float, 0.65, -5.0, 5.0, "Border push-out distance — how far points get pushed toward the nearest cell border."),
        param!("right", "Right", unlimited_float, 0.35, -5.0, 5.0, "Threshold for border behavior vs pass-through. Higher = more points get pushed to borders."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 3,
    wgsl_init: Some(r#"
fn init_pre_boarders2(user: array<f32, 3>) -> array<f32, 3> {
    let c0 = max(abs(user[0]), 1e-6);
    let cl0 = max(abs(user[1]), 1e-6);
    let cr0 = max(abs(user[2]), 1e-6);
    var out: array<f32, 3>;
    out[0] = c0;
    out[1] = c0 * cl0;
    out[2] = c0 + c0 * cr0;
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pre_boarders2(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let c = get_param(xform_id, variation_id, 3u);
    let cl = get_param(xform_id, variation_id, 4u);
    let cr = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];

    let round_x = round(p.x);
    let round_y = round(p.y);
    let off_x = p.x - round_x;
    let off_y = p.y - round_y;

    var ox: f32;
    var oy: f32;
    if (rng_nextf(rng) >= cr) {
        ox = off_x * c + round_x;
        oy = off_y * c + round_y;
    } else if (abs(off_x) >= abs(off_y)) {
        if (off_x >= 0.0) {
            ox = off_x * c + round_x + cl;
            oy = off_y * c + round_y + cl * off_y / off_x;
        } else {
            ox = off_x * c + round_x - cl;
            oy = off_y * c + round_y - cl * off_y / off_x;
        }
    } else {
        if (off_y >= 0.0) {
            ox = off_x * c + round_x + off_x / off_y * cl;
            oy = off_y * c + round_y + cl;
        } else {
            ox = off_x * c + round_x - off_x / off_y * cl;
            oy = off_y * c + round_y - cl;
        }
    }
    return vec2<f32>(w * ox, w * oy);
}
"#,
    wgsl_3d: Some(r#"
fn variation_pre_boarders2(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let c = get_param(xform_id, variation_id, 3u);
    let cl = get_param(xform_id, variation_id, 4u);
    let cr = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];

    let round_x = round(p.x);
    let round_y = round(p.y);
    let off_x = p.x - round_x;
    let off_y = p.y - round_y;

    var ox: f32;
    var oy: f32;
    if (rng_nextf(rng) >= cr) {
        ox = off_x * c + round_x;
        oy = off_y * c + round_y;
    } else if (abs(off_x) >= abs(off_y)) {
        if (off_x >= 0.0) {
            ox = off_x * c + round_x + cl;
            oy = off_y * c + round_y + cl * off_y / off_x;
        } else {
            ox = off_x * c + round_x - cl;
            oy = off_y * c + round_y - cl * off_y / off_x;
        }
    } else {
        if (off_y >= 0.0) {
            ox = off_x * c + round_x + off_x / off_y * cl;
            oy = off_y * c + round_y + cl;
        } else {
            ox = off_x * c + round_x - off_x / off_y * cl;
            oy = off_y * c + round_y - cl;
        }
    }
    return vec3<f32>(w * ox, w * oy, p.z);
}
"#),
};

// =============================================================================
// splitbrdr: bubble warp + border (FracFx)
//   Body is bubble + border combined:
//     B = (x²+y²)/4 + 1
//     b = w / B
//     out = (FTx · b, FTy · b)            (bubble; weight factors out)
//     + boarders-style branch (cell-grid border, weight factors out)
//     + (FTx · px, FTy · py)              (NO VVAR multiplier — needs
//                                          divide-out via needs_transform)
// =============================================================================
/// Combines a Bubble warp (radial sphere projection) with a Boarders-style
/// cell-grid border. The `x`/`y` parameters control the border behavior;
/// `px`/`py` add an extra linear pass-through component.
///
/// # Authors
/// - FracFx
pub static SPLITBRDR: VariationDef = VariationDef {
    name: "splitbrdr",
    aliases: &[],
    display_name: "Split Brdr",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        param!("x", "X", unlimited_float, 0.25, -5.0, 5.0, "Border push offset in one direction."),
        param!("y", "Y", unlimited_float, 0.25, -5.0, 5.0, "Border push offset in the other direction."),
        param!("px", "PX", unlimited_float, 0.0, -5.0, 5.0, "Linear pass-through scaling for X — adds a fraction of the input X to the output."),
        param!("py", "PY", unlimited_float, 0.0, -5.0, 5.0, "Linear pass-through scaling for Y."),
    ],
    needs_transform: true,
    writes_color: false,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_splitbrdr(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32> {
    let xp = get_param(xform_id, variation_id, 0u);
    let yp = get_param(xform_id, variation_id, 1u);
    let px = get_param(xform_id, variation_id, 2u);
    let py = get_param(xform_id, variation_id, 3u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    // Bubble warp: weight factors out cleanly, no needs_transform required.
    let b = w / max(0.25 * (p.x * p.x + p.y * p.y) + 1.0, 1e-30);
    var fpx = p.x * b;
    var fpy = p.y * b;

    // Boarders-style cell-grid contribution (multiplied by w internally).
    let round_x = round(p.x);
    let round_y = round(p.y);
    let off_x = p.x - round_x;
    let off_y = p.y - round_y;

    var bx: f32;
    var by: f32;
    if (rng_nextf(rng) >= 0.75) {
        bx = off_x * 0.5 + round_x;
        by = off_y * 0.5 + round_y;
    } else if (abs(off_x) >= abs(off_y)) {
        if (off_x >= 0.0) {
            bx = off_x * 0.5 + round_x + xp;
            by = off_y * 0.5 + round_y + yp * off_y / off_x;
        } else {
            bx = off_x * 0.5 + round_x - yp;
            by = off_y * 0.5 + round_y - yp * off_y / off_x;
        }
    } else {
        if (off_y >= 0.0) {
            bx = off_x * 0.5 + round_x + off_x / off_y * yp;
            by = off_y * 0.5 + round_y + yp;
        } else {
            bx = off_x * 0.5 + round_x - off_x / off_y * xp;
            by = off_y * 0.5 + round_y - yp;
        }
    }
    fpx = fpx + w * bx;
    fpy = fpy + w * by;

    // Two extra lines without VVAR multiplier — needs divide-out so
    // outer × w gives back the unscaled cpp output.
    fpx = fpx + p.x * px;
    fpy = fpy + p.y * py;

    return vec2<f32>(fpx * inv_w, fpy * inv_w);
}
"#,
    wgsl_3d: Some(r#"
fn variation_splitbrdr(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec3<f32> {
    let xp = get_param(xform_id, variation_id, 0u);
    let yp = get_param(xform_id, variation_id, 1u);
    let px = get_param(xform_id, variation_id, 2u);
    let py = get_param(xform_id, variation_id, 3u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let b = w / max(0.25 * (p.x * p.x + p.y * p.y) + 1.0, 1e-30);
    var fpx = p.x * b;
    var fpy = p.y * b;

    let round_x = round(p.x);
    let round_y = round(p.y);
    let off_x = p.x - round_x;
    let off_y = p.y - round_y;

    var bx: f32;
    var by: f32;
    if (rng_nextf(rng) >= 0.75) {
        bx = off_x * 0.5 + round_x;
        by = off_y * 0.5 + round_y;
    } else if (abs(off_x) >= abs(off_y)) {
        if (off_x >= 0.0) {
            bx = off_x * 0.5 + round_x + xp;
            by = off_y * 0.5 + round_y + yp * off_y / off_x;
        } else {
            bx = off_x * 0.5 + round_x - yp;
            by = off_y * 0.5 + round_y - yp * off_y / off_x;
        }
    } else {
        if (off_y >= 0.0) {
            bx = off_x * 0.5 + round_x + off_x / off_y * yp;
            by = off_y * 0.5 + round_y + yp;
        } else {
            bx = off_x * 0.5 + round_x - off_x / off_y * xp;
            by = off_y * 0.5 + round_y - yp;
        }
    }
    fpx = fpx + w * bx;
    fpy = fpy + w * by;
    fpx = fpx + p.x * px;
    fpy = fpy + p.y * py;

    // Z preserve scales with weight (FPz += VVAR · FTz upstream); leave
    // p.z so the outer multiplier produces VVAR · p.z.
    return vec3<f32>(fpx * inv_w, fpy * inv_w, p.z);
}
"#),
};
