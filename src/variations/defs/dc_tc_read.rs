//! TC-read direct-color variations: dc_ztransl, pre_dcztransl,
//! colorscale_wf, post_colorscale_wf
//!
//! All four read the color register `*vc` and use the value to drive
//! spatial output (specifically Z displacement / scaling). This is the
//! "blocker #5" family — the pattern was previously believed to require
//! a write-then-read color pipeline restructuring; turned out to be a
//! direct consequence of the `vc: ptr<function, f32>` parameter already
//! being read+write. See dc_carpet3D for the canonical write-then-read
//! demonstration; these four exercise the pure read direction.
//!
//! Sources (cpp + embedded Java references):
//!   - `output/jwildfire-vars/output/dc_ztransl.cpp` (Xyrus02; cpp-only,
//!     no embedded Java)
//!   - `output/jwildfire-vars/output/pre_dcztransl.cpp` (Xyrus02; Java
//!     embedded — verified against cpp)
//!   - `output/jwildfire-vars/output/colorscale_wf.cpp` (Andreas Maschke;
//!     Java embedded — Java reads `pAffineTP.color` while cpp reads
//!     `pVarTP.color`; differs only when other DC variations precede
//!     this one in the chain. Our `*vc` matches the cpp semantics.)
//!   - `output/jwildfire-vars/output/post_colorscale_wf.cpp` (Andreas
//!     Maschke; Java + cpp agree on `pVarTP.color`)
//!
//! Known reset_z limitation (colorscale_wf, post_colorscale_wf):
//! When `reset_z > 0` the cpp/Java sets `pVarTP.z = dz` outright,
//! discarding any prior Z contribution from other variations in the
//! same transform. Our `needs_transform` outer-multiplier model can
//! only add (`result.z += w · nz`), not override. The current port
//! emits `nz = dz / w` regardless of `reset_z`, which gives the
//! correct result when the variation is the only normal-phase Z
//! contributor (the typical use). Mixed cases differ from upstream;
//! revisit with `needs_accum` if a real flame needs it.

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

// ---------------------------------------------------------------------------
// dc_ztransl (Xyrus02)
// ---------------------------------------------------------------------------

/// Z displacement driven by the color register — `zf = factor · (*vc − x0)
/// / (x1 − x0)` (with `x0`/`x1` order-corrected and a zero-divisor guard
/// in init), optionally clamped to `[0, 1]`. XY pass through; Z is either
/// replaced by `w · zf` (`overwrite = 1`, default) or multiplied by `zf`
/// (`overwrite = 0`).
///
/// The "ztransl" name reflects the typical use: place a DC variation
/// upstream that paints a color gradient into `*vc`, then use this
/// variation to translate Z based on the gradient — points in different
/// color ranges land at different Z layers.
///
/// # Authors
/// - Xyrus02
pub static DC_ZTRANSL: VariationDef = VariationDef {
    name: "dc_ztransl",
    display_name: "DC Z Translation",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("x0", "X0", unlimited_float, 0.0, 0.0, 1.0, "Lower edge of the color-register input range. (Swapped with `x1` in init if `x0 > x1`.)"),
        param!("x1", "X1", unlimited_float, 1.0, 0.0, 1.0, "Upper edge of the color-register input range. Determines the normalization denominator `x1 − x0`."),
        param!("factor", "Factor", unlimited_float, 1.0, -10.0, 10.0, "Multiplier on the normalized `zf` value before it drives Z."),
        param!("overwrite", "Overwrite", bool, true, "When on, Z = `w · zf` (the input Z is discarded). When off, Z = `w · input_z · zf` (multiplicative)."),
        param!("clamp", "Clamp", bool, false, "When on, `zf` is clamped to `[0, 1]` before use."),
    ],
    needs_transform: true,
    writes_color: true, // read-only on *vc, but the flag is what gets the parameter passed
    // 1 init slot at 5: x1_m_x0 = max(x0, x1) - min(x0, x1)   (zero-guarded)
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_dc_ztransl(user: array<f32, 5>) -> array<f32, 1> {
    var out: array<f32, 1>;
    let lo = min(user[0], user[1]);
    let hi = max(user[0], user[1]);
    let span = hi - lo;
    out[0] = select(span, 1e-30, span == 0.0);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_dc_ztransl(p: vec2<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec2<f32> {
    // 2D mode: no Z output, but vc-driven computation has no other XY effect
    // (XY just passes through), so this is effectively a no-op spatially in 2D.
    return p;
}
"#,
    wgsl_3d: Some(r#"
fn variation_dc_ztransl(p: vec3<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec3<f32> {
    let x0_u = get_param(xform_id, variation_id, 0u);
    let x1_u = get_param(xform_id, variation_id, 1u);
    let factor = get_param(xform_id, variation_id, 2u);
    let overwrite = i32(get_param(xform_id, variation_id, 3u));
    let clamp_on = i32(get_param(xform_id, variation_id, 4u));
    let x1_m_x0 = get_param(xform_id, variation_id, 5u);
    let lo = min(x0_u, x1_u);

    let tc = *vc;
    var zf = factor * (tc - lo) / x1_m_x0;
    if (clamp_on != 0) {
        zf = clamp(zf, 0.0, 1.0);
    }

    // X and Y pass through (cpp: FPx += w·FTx, FPy += w·FTy → return p.x, p.y).
    // Z (overwrite=1, default): w·nz = w·zf → nz = zf
    // Z (overwrite=0):           w·nz = w·p.z·zf → nz = p.z·zf
    let nz = select(zf, p.z * zf, overwrite == 0);
    return vec3<f32>(p.x, p.y, nz);
}
"#),
};

// ---------------------------------------------------------------------------
// pre_dcztransl (Xyrus02)
// ---------------------------------------------------------------------------

/// Pre-phase Z displacement driven by `*vc` — same formula as
/// `dc_ztransl` but runs in the PRE phase (modifies the input to the
/// normal-phase variation chain rather than contributing to the
/// accumulator). cpp/Java replace `pAffineTP.{x,y,z}` outright with
/// weighted values; in our pre-phase model the body returns the new
/// input directly.
///
/// # Authors
/// - Xyrus02
pub static PRE_DCZTRANSL: VariationDef = VariationDef {
    name: "pre_dcztransl",
    display_name: "Pre DC Z Translation",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Pre,
    needs_rng: false,
    parameters: &[
        param!("x0", "X0", unlimited_float, 0.0, -10.0, 10.0, "Lower edge of the color-register input range."),
        param!("x1", "X1", unlimited_float, 1.0, -10.0, 10.0, "Upper edge of the color-register input range."),
        param!("factor", "Factor", unlimited_float, 1.0, -10.0, 10.0, "Multiplier on the normalized `zf` value."),
        param!("overwrite", "Overwrite", bool, true, "When on, Z = `w · zf`. When off, Z = `w · input_z · zf`."),
        param!("clamp", "Clamp", bool, false, "When on, `zf` is clamped to `[0, 1]`."),
    ],
    needs_transform: true,
    writes_color: true,
    init_param_count: 1,
    wgsl_init: Some(r#"
fn init_pre_dcztransl(user: array<f32, 5>) -> array<f32, 1> {
    var out: array<f32, 1>;
    let lo = min(user[0], user[1]);
    let hi = max(user[0], user[1]);
    let span = hi - lo;
    out[0] = select(span, 1e-30, span == 0.0);
    return out;
}
"#),
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_pre_dcztransl(p: vec2<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec2<f32> {
    let w = transforms[xform_id].variations[variation_id];
    // 2D pre-phase: just scale XY by weight (the Z-driven part is dropped in 2D).
    return vec2<f32>(w * p.x, w * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_pre_dcztransl(p: vec3<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec3<f32> {
    let x0_u = get_param(xform_id, variation_id, 0u);
    let x1_u = get_param(xform_id, variation_id, 1u);
    let factor = get_param(xform_id, variation_id, 2u);
    let overwrite = i32(get_param(xform_id, variation_id, 3u));
    let clamp_on = i32(get_param(xform_id, variation_id, 4u));
    let x1_m_x0 = get_param(xform_id, variation_id, 5u);
    let w = transforms[xform_id].variations[variation_id];
    let lo = min(x0_u, x1_u);

    let tc = *vc;
    var zf = factor * (tc - lo) / x1_m_x0;
    if (clamp_on != 0) {
        zf = clamp(zf, 0.0, 1.0);
    }

    // Pre phase replaces input directly (no outer multiplier).
    //   overwrite=1: nz = w · zf
    //   overwrite=0: nz = w · p.z · zf
    let nz = select(w * zf, w * p.z * zf, overwrite == 0);
    return vec3<f32>(w * p.x, w * p.y, nz);
}
"#),
};

// ---------------------------------------------------------------------------
// colorscale_wf (Andreas Maschke)
// ---------------------------------------------------------------------------

/// Color-scaled XYZ output — `(x, y) = (scale_x · p.x, scale_y · p.y)`
/// and `z = *vc · scale_z + offset_z` (with `reset_z > 0` controlling
/// override vs accumulate in cpp; our model approximates by always
/// accumulating, see file header note). XY use the pre-affine input
/// (the variation's `p`).
///
/// # Authors
/// - Andreas Maschke
pub static COLORSCALE_WF: VariationDef = VariationDef {
    name: "colorscale_wf",
    display_name: "Color Scale WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Normal,
    needs_rng: false,
    parameters: &[
        param!("scale_x", "Scale X", unlimited_float, 0.0, -10.0, 10.0, "X output scale: contribution to result.x is `w · scale_x · p.x`."),
        param!("scale_y", "Scale Y", unlimited_float, 0.0, -10.0, 10.0, "Y output scale."),
        param!("scale_z", "Scale Z", unlimited_float, 0.5, -10.0, 10.0, "Color-register → Z coupling multiplier in `dz = *vc · scale_z · w + offset_z`."),
        param!("offset_z", "Offset Z", unlimited_float, 0.0, -10.0, 10.0, "Constant Z bump added after the color-derived term."),
        param!("reset_z", "Reset Z", unlimited_float, 0.0, 0.0, 1.0, "Upstream uses this to override Z when > 0 instead of accumulating; this port always accumulates (see file header)."),
    ],
    needs_transform: true,
    writes_color: true,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_colorscale_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec2<f32> {
    let scale_x = get_param(xform_id, variation_id, 0u);
    let scale_y = get_param(xform_id, variation_id, 1u);
    return vec2<f32>(scale_x * p.x, scale_y * p.y);
}
"#,
    wgsl_3d: Some(r#"
fn variation_colorscale_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec3<f32> {
    let scale_x = get_param(xform_id, variation_id, 0u);
    let scale_y = get_param(xform_id, variation_id, 1u);
    let scale_z = get_param(xform_id, variation_id, 2u);
    let offset_z = get_param(xform_id, variation_id, 3u);
    let w = transforms[xform_id].variations[variation_id];
    let inv_w = 1.0 / select(w, 1e-30, abs(w) < 1e-30);

    let tc = *vc;
    // cpp: dz = TC · scale_z · w + offset_z
    // outer contribution: w · nz = dz, so nz = dz · inv_w
    let dz = tc * scale_z * w + offset_z;
    let nz = dz * inv_w;
    return vec3<f32>(scale_x * p.x, scale_y * p.y, nz);
}
"#),
};

// ---------------------------------------------------------------------------
// post_colorscale_wf (Andreas Maschke)
// ---------------------------------------------------------------------------

/// Post-phase `colorscale_wf` — same formula but operates on the
/// accumulator value (`pVarTP.{x,y}`) after normal-phase variations
/// have run. Post phase doesn't apply the outer multiplier; the body
/// directly produces the modified accumulator.
///
/// XY: `nx = p.x · (1 + w · scale_x)`, `ny = p.y · (1 + w · scale_y)` —
/// cpp's `pVarTP.x += w · scale_x · pVarTP.x` desugared.
/// Z: `reset_z > 0` truly overrides Z here (post phase has direct
/// access to `p.z`, unlike the normal-phase variant).
///
/// # Authors
/// - Andreas Maschke
pub static POST_COLORSCALE_WF: VariationDef = VariationDef {
    name: "post_colorscale_wf",
    display_name: "Post Color Scale WF",
    category: VariationCategory::Full3D,
    phase: VariationPhase::Post,
    needs_rng: false,
    parameters: &[
        param!("scale_x", "Scale X", unlimited_float, 0.0, -10.0, 10.0, "X scale: `p.x · (1 + w · scale_x)`."),
        param!("scale_y", "Scale Y", unlimited_float, 0.0, -10.0, 10.0, "Y scale."),
        param!("scale_z", "Scale Z", unlimited_float, 0.5, -10.0, 10.0, "Color-register → Z coupling: `dz = *vc · scale_z · w + offset_z`."),
        param!("offset_z", "Offset Z", unlimited_float, 0.0, -10.0, 10.0, "Constant Z bump."),
        param!("reset_z", "Reset Z", unlimited_float, 0.0, 0.0, 1.0, "When > 0, output Z = `dz`; otherwise output Z = `p.z + dz`."),
    ],
    needs_transform: true,
    writes_color: true,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    needs_accum: false,
    wgsl_2d: r#"
fn variation_post_colorscale_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec2<f32> {
    let scale_x = get_param(xform_id, variation_id, 0u);
    let scale_y = get_param(xform_id, variation_id, 1u);
    let w = transforms[xform_id].variations[variation_id];
    return vec2<f32>(p.x * (1.0 + w * scale_x), p.y * (1.0 + w * scale_y));
}
"#,
    wgsl_3d: Some(r#"
fn variation_post_colorscale_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, vc: ptr<function, f32>) -> vec3<f32> {
    let scale_x = get_param(xform_id, variation_id, 0u);
    let scale_y = get_param(xform_id, variation_id, 1u);
    let scale_z = get_param(xform_id, variation_id, 2u);
    let offset_z = get_param(xform_id, variation_id, 3u);
    let reset_z = get_param(xform_id, variation_id, 4u);
    let w = transforms[xform_id].variations[variation_id];

    let tc = *vc;
    let dz = tc * scale_z * w + offset_z;
    let nz = select(p.z + dz, dz, reset_z > 0.0);
    return vec3<f32>(p.x * (1.0 + w * scale_x), p.y * (1.0 + w * scale_y), nz);
}
"#),
};
