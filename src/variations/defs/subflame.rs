//! Subflame variations (JWildfire subflame_wf family)
//!
//! A subflame is **not** a layered render. The `subflame_wf` variation
//! owns a complete inner flame definition (referenced by index into
//! `FractalConfig.subflames`), and during each step of the parent
//! flame's chaos game it advances a *nested* chaos game by one
//! iteration on the subflame's IFS, then uses the resulting point as
//! the variation's output.
//!
//! Crucially `subflame_wf` is classified as a **blur** variation in
//! JWildfire (same family as `circleblur`, `starblur`):
//!   - It ignores its input `p` (the chaos-game state of the parent).
//!   - The variation `amount` is ignored too.
//!   - Users scale/rotate the subflame via the *parent xform's
//!     post-affine*, which is applied to FP after `subflame_wf`
//!     contributes its `q` to the running result.
//!
//! State (5 slots, persistent per (thread, xform, variation_id)):
//!   0..2: subflame's current chaos-game point (x, y, z)
//!   3:    subflame's current xform index (encoded as f32 ↔ u32)
//!   4:    subflame's current color scalar (0..1, for `color_mode`)
//!
//! P3 — variation REGISTRATION only. Body is a stub returning
//! vec3(0). P4 implements the nested chaos-game iteration in
//! `subflame_iterate(subflame_id, state_offset)` and wires it into
//! the variation body.
//!
//! Refs:
//!   - `output/jwildfire-vars/output/subflame_wf.cpp`
//!   - https://www.jwfsanctuary.club/variation-information/subflame/
//!   - `docs/projects/subflames.md`

use crate::variations::{
    definition::{VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static SUBFLAME_WF: VariationDef = VariationDef {
    name: "subflame_wf",
    display_name: "Subflame",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Normal,
    needs_rng: true,
    parameters: &[
        // The subflame index — selects which entry of
        // `FractalConfig.subflames` this variation reads its IFS from.
        // Stored as f32 like every other parameter; the shader casts
        // to u32 before indexing the metadata array. Range is the
        // current MAX_SUBFLAMES (8) — bumping the cap requires only
        // changing the constant in gpu/buffers.rs.
        param!("subflame_id", "Subflame", int, 0.0, 0.0, 7.0),
        // scale & angle: applied to the subflame's q INSIDE the
        // variation, before adding to FP. The sanctuary spec
        // recommends users go through the parent xform's post-affine
        // instead, but we keep these for round-trip fidelity with
        // existing JWildfire / Apophysis flame files that set them.
        param!("scale", "Scale", unlimited_float, 1.0, -10.0, 10.0),
        param!("angle", "Angle", angle, 0.0),
        param!("offset_x", "Offset X", unlimited_float, 0.0, -10.0, 10.0),
        param!("offset_y", "Offset Y", unlimited_float, 0.0, -10.0, 10.0),
        param!("offset_z", "Offset Z", unlimited_float, 0.0, -10.0, 10.0),
        // colorscale_z: multiplies the subflame's color scalar and
        // adds the product to the variation's z output. JWildfire's
        // `colorscale_wf`-style mechanism.
        param!("colorscale_z", "Color Scale Z", unlimited_float, 0.0, -10.0, 10.0),
        // color_mode: -1 (Off, default) leaves the parent's color
        // alone. 0 (Direct) overrides with the subflame's color
        // scalar. 1..4 are JWildfire's CM_RED / GREEN / BLUE /
        // BRIGHTNESS modes (rarely used in practice, but supported
        // for round-trip fidelity).
        param!("color_mode", "Color Mode", int, -1.0, -1.0, 4.0),
    ],
    needs_transform: false,
    writes_color: true,
    init_param_count: 0,
    wgsl_init: None,
    // 5 slots per instance — see module doc. Zero-init is OK; the
    // chaos game self-corrects within ~50 iterations. (JWildfire's
    // reference does an explicit 42-iteration prefuse at variation
    // setup time; we get the same effect from cumulative-mean
    // dilution at the histogram level.)
    state_count: 5,
    wgsl_state_init: None,
    needs_accum: false,
    // P4b: real subflame_wf body. Calls subflame_iterate() to advance
    // the nested chaos game one step, then applies scale/rotate/offset
    // and the color_mode rule. Forward-references subflame_iterate
    // (defined in `shaders/core/subflame.wgsl`, injected after this
    // variation by the shader builder when subflame_wf is active).
    //
    // "Blur" semantics: input `p` is ignored. The variation amount is
    // also ignored (the apply_variations dispatcher multiplies our
    // output by amount, but JWildfire's spec is explicit that amount
    // shouldn't affect output — users scale via the parent xform's
    // post-affine instead). For perfect parity we'd return early at
    // amount=0; since the dispatcher gates on amount != 0 before
    // calling us, that's effectively the case.
    wgsl_2d: r#"
fn variation_subflame_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let subflame_id = u32(get_param(xform_id, variation_id, 0u));
    let scale = get_param(xform_id, variation_id, 1u);
    let angle_deg = get_param(xform_id, variation_id, 2u);
    let offset_x = get_param(xform_id, variation_id, 3u);
    let offset_y = get_param(xform_id, variation_id, 4u);
    let color_mode = i32(get_param(xform_id, variation_id, 7u));

    let q = subflame_iterate(subflame_id, xform_id, variation_id, rng);

    let angle_rad = angle_deg * 0.017453292519943295;
    let cos_a = cos(angle_rad);
    let sin_a = sin(angle_rad);
    let sx = scale * q.x;
    let sy = scale * q.y;

    // color_mode == 0 (Direct) overrides parent's vc with subflame color.
    // Other modes are listed in the spec but rarely used; v1 treats
    // them as Off (no-op). Spec ref: jwfsanctuary.club/variation-information/subflame.
    if (color_mode == 0) {
        *vc = q.w;
    }

    return vec2<f32>(
        sx * cos_a - sy * sin_a + offset_x,
        sx * sin_a + sy * cos_a + offset_y,
    );
}
"#,
    wgsl_3d: Some(r#"
fn variation_subflame_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let subflame_id = u32(get_param(xform_id, variation_id, 0u));
    let scale = get_param(xform_id, variation_id, 1u);
    let angle_deg = get_param(xform_id, variation_id, 2u);
    let offset_x = get_param(xform_id, variation_id, 3u);
    let offset_y = get_param(xform_id, variation_id, 4u);
    let offset_z = get_param(xform_id, variation_id, 5u);
    let colorscale_z = get_param(xform_id, variation_id, 6u);
    let color_mode = i32(get_param(xform_id, variation_id, 7u));

    let q = subflame_iterate(subflame_id, xform_id, variation_id, rng);

    let angle_rad = angle_deg * 0.017453292519943295;
    let cos_a = cos(angle_rad);
    let sin_a = sin(angle_rad);
    let sx = scale * q.x;
    let sy = scale * q.y;

    if (color_mode == 0) {
        *vc = q.w;
    }

    return vec3<f32>(
        sx * cos_a - sy * sin_a + offset_x,
        sx * sin_a + sy * cos_a + offset_y,
        scale * q.z + offset_z + colorscale_z * q.w,
    );
}
"#),
};
