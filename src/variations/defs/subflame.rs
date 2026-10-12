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
//!   - The variation `amount` is ignored too: JWF adds `q` to the
//!     running result unscaled, so the body divides out the weight the
//!     dispatcher multiplies it by.
//!   - Users scale/rotate the subflame via the *parent xform's
//!     post-affine*, which is applied to FP after `subflame_wf`
//!     contributes its `q` to the running result.
//!
//! State (9 slots, persistent per (thread, xform, variation_id)):
//!   0..2: subflame's current chaos-game point (x, y, z)
//!   3:    subflame's current xform index (encoded as f32 ↔ u32)
//!   4:    subflame's current color scalar (0..1, for `color_mode`)
//!   5..8: the last output q (x, y, z, colour), returned again by a call
//!         that finds no visible transform
//!
//! The nested step is `subflame_iterate` in `shaders/core/subflame.wgsl`,
//! a port of JWF's `SubFlameWFFunc.subflameIter`: it also applies
//! `scale`, `angle`, the offsets and `colorscale_z`, for both variations.
//!
//! Refs:
//!   - `output/variation-jwf-source/SubFlameWFFunc.java`,
//!     `PreSubFlameWFFunc.java`
//!   - `output/jwildfire-vars/output/subflame_wf.cpp`
//!   - https://www.jwfsanctuary.club/variation-information/subflame/
//!   - `docs/archive/projects/subflames.md`

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
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
/// Apophysis flame files. Per-thread state (9 slots) carries the subflame's
/// chaos-game point, current xform index, color scalar and last output
/// across iterations. Inside the subflame, transform opacity works as in
/// JWildfire: a step on a hidden transform, or one whose opacity roll
/// fails, moves the walk but is not output; the walk steps on until a
/// visible one. The nested chaos-game step is implemented in
/// [`shaders/core/subflame.wgsl`](../../shaders/core/subflame.wgsl),
/// injected by the shader builder when `subflame_wf` is active.
///
/// # Authors
/// - Andreas Maschke
pub static SUBFLAME_WF: VariationDef = VariationDef {
    name: "subflame_wf",
    aliases: &[],
    display_name: "Subflame",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor, Feature::AlwaysZ],
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
        param!("color_mode", "Color Mode", int, -1.0, -1.0, 4.0, "How the subflame's color scalar interacts with the parent's color register. -1 = Off (leave parent's color alone). 0 = Direct (overwrite with subflame's color). 1-4 are JWildfire's CM_RED/GREEN/BLUE/BRIGHTNESS, which in JWF pick a specific channel of the subflame's RGB output. Our flat single-channel color model collapses those four to the same behavior as Direct — they still trigger the override, just with the subflame's `vc` instead of a per-channel value. Use Off to keep parent color; any other value to pull from subflame."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    // 9 slots per instance — see module doc. Zero-init is the "fresh"
    // sentinel: subflame_iterate's first call on a thread starts from a
    // random point and walks JWildfire's 42-step prefuse, as
    // SubFlameWFFunc.prefuseIter does at setup time (and again after a
    // walk goes non-finite). It is also JWF's initial q, (0, 0, 0).
    state_count: 9,
    wgsl_state_init: None,
    // Calls subflame_iterate() to advance the nested chaos game, which
    // returns q already scaled, rotated and offset, then applies the
    // color_mode rule. Forward-references subflame_iterate (defined in
    // `shaders/core/subflame.wgsl`, injected after this variation by the
    // shader builder when subflame_wf is active).
    //
    // "Blur" semantics: input `p` is ignored. The variation amount is
    // also ignored: JWF's transform does `pVarTP += q`, but the
    // apply_variations dispatcher multiplies every normal variation's
    // output by its weight, so the body divides the weight out (the
    // idisc pattern). The dispatcher gates on a non-zero weight, so the
    // division is safe. Users scale via the parent xform's post-affine.
    wgsl_2d: r#"
fn variation_subflame_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let subflame_id = u32(get_param(xform_id, variation_id, 0u));
    let color_mode = i32(get_param(xform_id, variation_id, 7u));

    let q = subflame_iterate(subflame_id, xform_id, variation_id, rng);

    // JWildfire's setColor rule: any color_mode != CM_OFF (-1)
    // overrides the parent's color register with the subflame's
    // color. Modes 0..4 in JWF pick which channel of the subflame's
    // RGB output to read (Direct / Red / Green / Blue / Brightness),
    // but our flat color model carries a single palette-index `vc`,
    // so the meaningful action collapses to "use subflame color" for
    // all five non-Off values.
    if (color_mode != -1) {
        *vc = q.w;
    }

    // Unscaled by the weight -- see above.
    let w = transforms[xform_id].variations[variation_id];
    return vec2<f32>(q.x, q.y) / w;
}
"#,
    wgsl_3d: r#"
fn variation_subflame_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let subflame_id = u32(get_param(xform_id, variation_id, 0u));
    let color_mode = i32(get_param(xform_id, variation_id, 7u));

    let q = subflame_iterate(subflame_id, xform_id, variation_id, rng);

    // See 2D body for the color_mode != -1 rationale.
    if (color_mode != -1) {
        *vc = q.w;
    }

    // Unscaled by the weight -- see the 2D body.
    let w = transforms[xform_id].variations[variation_id];
    return q.xyz / w;
}
"#,
};

/// Pre-phase variant of subflame_wf. Steps the nested chaos game (same
/// `subflame_iterate` helper) and writes its `q` to the affine point —
/// overriding the chaos-game state going into the rest of the
/// pre/normal/post variation chain. JWildfire's
/// `PreSubFlameWFFunc.transform` extends `SubFlameWFFunc` and does
/// `pAffineTP.{x,y,z} = q.{x,y,z}`; `q` comes from the shared
/// `subflameIter`, which has already applied scale, angle, the offsets
/// and colorscale_z, so those parameters work here too. A pre variation's
/// weight is never applied, as JWF ignores it. `color_mode` still applies
/// — non-Off pulls the subflame's color scalar into the parent's color
/// register.
///
/// Source: `output/variation-jwf-source/PreSubFlameWFFunc.java`.
///
/// # Authors
/// - Andreas Maschke
pub static PRE_SUBFLAME_WF: VariationDef = VariationDef {
    name: "pre_subflame_wf",
    aliases: &[],
    display_name: "Pre Subflame",
    category: VariationCategory::Plugin,
    phase: VariationPhase::Pre,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    // Same params as SUBFLAME_WF — JWildfire's PreSubFlameWFFunc extends
    // SubFlameWFFunc, so file XML carries identical names, and the
    // shared subflame_iterate reads the same slots.
    parameters: SUBFLAME_WF.parameters,
    init_param_count: 0,
    wgsl_init: None,
    state_count: 9,
    wgsl_state_init: None,
    wgsl_2d: r#"
fn variation_pre_subflame_wf(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let subflame_id = u32(get_param(xform_id, variation_id, 0u));
    let color_mode = i32(get_param(xform_id, variation_id, 7u));
    let q = subflame_iterate(subflame_id, xform_id, variation_id, rng);
    if (color_mode != -1) {
        *vc = q.w;
    }
    return vec2<f32>(q.x, q.y);
}
"#,
    wgsl_3d: r#"
fn variation_pre_subflame_wf(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let subflame_id = u32(get_param(xform_id, variation_id, 0u));
    let color_mode = i32(get_param(xform_id, variation_id, 7u));
    let q = subflame_iterate(subflame_id, xform_id, variation_id, rng);
    if (color_mode != -1) {
        *vc = q.w;
    }
    return vec3<f32>(q.x, q.y, q.z);
}
"#,
};
