//! `su3_mobius` — SU(3)-reduced SL(2,ℂ) Möbius group limit set
//! (Roger Bagula).
//!
//! A faithful port of Roger Bagula's *Three Programs: SU3 reduction,
//! McMullen Möbius, Nylander SU3-reduced SL2C triquasiconformal*
//! (Programing4, July 2026). The eight Gell-Mann matrices of SU(3) —
//! the algebra of the strong force, whose three SU(2) subgroups are
//! the u/d/s quark isospins — are reduced from 3×3 to 2×2 by the
//! tensor sandwich `u0[i] = tt·λ[i]·t` (`tt = [[1,0,1],[a,b,c]]`),
//! then each is conjugated by a fixed **triquasiconformal**
//! deformation `C = dk·s0·qf` (`qf = rotate(π/4)` — the "45";
//! `dk = [[1+i,1],[1,1-i]]`, `s0 = [[1,-i],[-i,1]]/√2`), which turns
//! the reduced generators loxodromic. The group is
//! `{u_i} ∪ {u_i⁻¹} = 16` SL(2,ℂ) elements (the "16-limit"), each read
//! as a Möbius map `z ↦ (Az+B)/(Cz+D)` of the Riemann sphere.
//!
//! Each call applies one random generator (Bagula's McMullen-style
//! random-IFS orbit `NestList[maps[[RandomInteger]]]`) — so iterating
//! this variation in the chaos game plots the group's limit set: three
//! disks in a row down the real axis (one per SU(2) subgroup), the
//! central isospin disk the most active, exactly as in the notebook's
//! renders. `avoid_reversal` skips a generator's inverse right after
//! it (no `g·g⁻¹` backtracking), drifting through the limit set
//! instead of dithering — as in `klein_group`.
//!
//! The 16 generators are precomputed exactly and baked into
//! `shaders/core/su3_mobius.wgsl` (the conjugation and SL(2,ℂ)
//! normalization are constant, so there is nothing to evaluate per
//! call beyond the Möbius map itself).
//!
//! Fundamentally a map of the complex plane; the 3D body applies it in
//! xy and passes z through. Original construction by Roger Bagula;
//! ported here.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static SU3_MOBIUS: VariationDef = VariationDef {
    name: "su3_mobius",
    aliases: &[],
    display_name: "SU(3) Möbius",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    // Slot 0: previous generator index (avoid_reversal). Slot 1: color
    // register for the Generator color mode.
    state_count: 2,
    wgsl_state_init: None,
    parameters: &[
        param!("avoid_reversal", "Avoid Reversal", bool, true, "Skip a generator's inverse immediately after applying it (no g·g⁻¹ cancellation), so the orbit drifts through the limit set instead of dithering on a small subset. Off = all 16 generators equally likely every call."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Generator"], "Direct-color source (needs the transform's Direct Color > 0). Generator: each of the 16 group elements has its own palette position, blended through a persistent color register at Color Speed — colors the three quark disks by which subgroup's generators reach them."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the Generator color (wrapped with fract)."),
        param!("color_speed", "Color Speed", float, 0.5, 0.0, 1.0, "How hard each generator pulls the color register toward its own palette position. Low = long blends of orbit history, 1 = hard per-generator assignment."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_su3_mobius(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let avoid = get_param(xform_id, variation_id, 0u) > 0.5;
    let dc_mode = u32(get_param(xform_id, variation_id, 1u));
    let dc_scale = get_param(xform_id, variation_id, 2u);
    let color_speed = get_param(xform_id, variation_id, 3u);

    var k = min(u32(rng_nextf(rng) * 16.0), 15u);
    if (avoid) {
        // Inverse of generator j is (j+8) mod 16 (g = u ++ inv(u)).
        let prev = u32(get_state(xform_id, variation_id, 0u));
        if (prev < 16u && k == (prev + 8u) % 16u) {
            k = (k + 1u) % 16u;
        }
    }
    set_state(xform_id, variation_id, 0u, f32(k));

    if (dc_mode == 1u) {
        var creg = get_state(xform_id, variation_id, 1u);
        creg = mix(creg, fract((f32(k) + 0.5) / 16.0 * dc_scale), color_speed);
        set_state(xform_id, variation_id, 1u, creg);
        *vc = creg;
    }
    return su3_mobius_apply(k, p);
}
"#;

const WGSL_3D: &str = r#"
fn variation_su3_mobius(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let avoid = get_param(xform_id, variation_id, 0u) > 0.5;
    let dc_mode = u32(get_param(xform_id, variation_id, 1u));
    let dc_scale = get_param(xform_id, variation_id, 2u);
    let color_speed = get_param(xform_id, variation_id, 3u);

    var k = min(u32(rng_nextf(rng) * 16.0), 15u);
    if (avoid) {
        let prev = u32(get_state(xform_id, variation_id, 0u));
        if (prev < 16u && k == (prev + 8u) % 16u) {
            k = (k + 1u) % 16u;
        }
    }
    set_state(xform_id, variation_id, 0u, f32(k));

    if (dc_mode == 1u) {
        var creg = get_state(xform_id, variation_id, 1u);
        creg = mix(creg, fract((f32(k) + 0.5) / 16.0 * dc_scale), color_speed);
        set_state(xform_id, variation_id, 1u, creg);
        *vc = creg;
    }
    return vec3<f32>(su3_mobius_apply(k, p.xy), p.z);
}
"#;
