//! Subflame variation (Andreas Maschke)
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

/// Subflame — nested chaos-game variation. Owns a complete inner flame
/// definition (referenced by `subflame_id` into `FractalConfig.subflames`);
/// during each step of the parent flame's chaos game it advances a *nested*
/// chaos game by one iteration on the subflame's IFS and uses the resulting
/// point as the variation's output. Classified as a **blur** variation in
/// JWildfire: it ignores both the input `p` (the parent chaos-game state)
/// and the variation `amount` — users scale/rotate the subflame via the
/// parent xform's post-affine instead, though `scale`/`angle`/`offset_*`
/// are also kept here for round-trip fidelity with existing JWildfire /
/// Apophysis flame files. Per-thread state (5 slots) carries the subflame's
/// chaos-game point, current xform index, and color scalar across
/// iterations. The nested chaos-game step is implemented in
/// [`shaders/core/subflame.wgsl`](../../shaders/core/subflame.wgsl),
/// injected by the shader builder when `subflame_wf` is active.
///
/// # Authors
/// - Andreas Maschke
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
        param!("subflame_id", "Subflame", int, 0.0, 0.0, 7.0, "Index into `FractalConfig.subflames` (0..MAX_SUBFLAMES-1). Selects which inner flame definition this variation iterates. Not really an enum — `MAX_SUBFLAMES` is a config-time constant in `gpu/buffers.rs` and the param range tracks it."),
        // scale & angle: applied to the subflame's q INSIDE the
        // variation, before adding to FP. The sanctuary spec
        // recommends users go through the parent xform's post-affine
        // instead, but we keep these for round-trip fidelity with
        // existing JWildfire / Apophysis flame files that set them.
        param!("scale", "Scale", unlimited_float, 1.0, -10.0, 10.0, "Scale factor applied to the subflame's per-step XY output (and Z output in 3D mode) before adding to the parent's accumulator. JWildfire's spec recommends using the parent xform's post-affine for this instead; the param is preserved for round-trip fidelity with files that set it."),
        param!("angle", "Angle", angle, 0.0, "Rotation angle (degrees) applied to the subflame's XY output before adding to the parent's accumulator. Same round-trip-fidelity note as `scale`."),
        param!("offset_x", "Offset X", unlimited_float, 0.0, -10.0, 10.0, "X translation added to the subflame's output after scale + rotation."),
        param!("offset_y", "Offset Y", unlimited_float, 0.0, -10.0, 10.0, "Y translation."),
        param!("offset_z", "Offset Z", unlimited_float, 0.0, -10.0, 10.0, "Z translation (3D mode only — added to the subflame's Z after `scale · q.z` and `colorscale_z · q.w`)."),
        // colorscale_z: multiplies the subflame's color scalar and
        // adds the product to the variation's z output. JWildfire's
        // `colorscale_wf`-style mechanism.
        param!("colorscale_z", "Color Scale Z", unlimited_float, 0.0, -10.0, 10.0, "Multiplier applied to the subflame's color scalar (0..1) and added to the Z output. JWildfire's `colorscale_wf`-style depth-from-color mechanism — non-zero values let the subflame's color drive a Z offset for pseudo-3D effects in 2D-classified subflames. 3D mode only."),
        // color_mode: -1 (Off, default) leaves the parent's color
        // alone. 0 (Direct) overrides with the subflame's color
        // scalar. 1..4 are JWildfire's CM_RED / GREEN / BLUE /
        // BRIGHTNESS modes (rarely used in practice, but supported
        // for round-trip fidelity).
        param!("color_mode", "Color Mode", int, -1.0, -1.0, 4.0, "How the subflame's color scalar interacts with the parent's color register. -1 = Off (default; leave parent's color alone), 0 = Direct (overwrite parent's vc with subflame's color). Modes 1-4 are JWildfire's CM_RED/GREEN/BLUE/BRIGHTNESS — declared in the param range but currently silently no-op'd (treated as Off); v1 only implements Off and Direct."),
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
