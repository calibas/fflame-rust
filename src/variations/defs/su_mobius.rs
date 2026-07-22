//! `su_mobius` — SU(n)-reduced SL(2,ℂ) Möbius group limit sets
//! (Roger Bagula).
//!
//! Faithful ports of Roger Bagula's SU(n) triquasiconformal Möbius
//! group fractals (Programing4, July 2026). Each is a chaos game over
//! a baked set of 2×2 base matrices plus their inverses, every element
//! conjugated by the tunable triquasiconformal deformation
//! `C = dk(δ)·s0·qf(θ + iη)` (`dk = [[1+iδ,1],[1,1−iδ]]`,
//! `s0 = [[1,-i],[-i,1]]/√2`, `qf = rotate(θ + iη)`), read as a Möbius
//! map `z ↦ (Az+B)/(Cz+D)` of the Riemann sphere. Iterating the
//! variation plots the group's limit set (Bagula's McMullen-style
//! random-IFS orbit `NestList[maps[[RandomInteger]]]`).
//!
//! `group` selects the SU(n) family:
//! - **SU(2) 6-Group** — the "plugged two ways" 6 matrices + inverses
//!   (12 generators). Bagula's *hyper*triquasiconformal setting is the
//!   HYPERBOLIC rotation `qf = rotate(iπ/4)`: set Angle = 0, Hyper
//!   Angle = 45°. This compacts the lattice-6 group into an Apollonian
//!   disk.
//! - **SU(3) Reduced** — the eight Gell-Mann matrices reduced 3×3→2×2
//!   (`u0[i] = tt·λ[i]·t`) + inverses (16 generators). Bagula's
//!   triquasiconformal setting is the ELLIPTIC `qf = rotate(π/4)`:
//!   Angle = 45°, Hyper Angle = 0 (the defaults) — three disks down
//!   the real axis, central isospin disk most active.
//!
//! The distinction between the two families' conjugators is exactly
//! the elliptic-vs-hyperbolic rotation angle, exposed here as two
//! sliders — so the "45" and the "hyper" are both live, and the base
//! tables (in `shaders/core/su_mobius.wgsl`) are the only thing a new
//! SU(n) member would add.
//!
//! `avoid_reversal` skips a generator's inverse right after it (no
//! `g·g⁻¹` backtracking; the inverse of local index j is
//! `(j + count/2) mod count`), drifting through the limit set instead
//! of dithering — as in `klein_group`.
//!
//! Fundamentally a map of the complex plane; the 3D body applies it in
//! xy and passes z through. Original constructions by Roger Bagula;
//! ported here.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static SU_MOBIUS: VariationDef = VariationDef {
    name: "su_mobius",
    // Kept so flames/presets referring to the pre-generalization name
    // still resolve to this variation.
    aliases: &["su3_mobius"],
    display_name: "SU(n) Möbius",
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
        param!("group", "Group", enum, 1, &["SU(2) 6-Group", "SU(3) Reduced"], "Which SU(n) generator set. SU(2) 6-Group (12 generators) — set Angle 0 / Hyper Angle 45 for Bagula's hypertriquasiconformal Apollonian disk. SU(3) Reduced (16 generators) — Angle 45 / Hyper 0 (the defaults) for the three-quark-disk limit set."),
        param!("avoid_reversal", "Avoid Reversal", bool, true, "Skip a generator's inverse immediately after applying it (no g·g⁻¹ cancellation), so the orbit drifts through the limit set instead of dithering on a small subset. Off = all generators equally likely every call."),
        param!("conj_angle", "Angle", angle, 45.0, "Elliptic rotation θ in the conjugator qf = rotate(θ + iη). SU(3)'s triquasiconformal '45'. Sweeping it deforms the whole limit set."),
        param!("conj_hyper", "Hyper Angle", angle, 0.0, "HYPERBOLIC rotation η (imaginary angle) in qf = rotate(θ + iη) — the 'hyper' in SU(2)'s hypertriquasiconformal. 45° with Angle 0 is Bagula's SU(2) 6-group; nonzero η bends the group loxodromically, compacting lattices toward Apollonian disks."),
        param!("qc_strength", "QC Strength", float, 1.0, 0.0, 2.0, "Quasiconformal deformation δ in dk = [[1+iδ,1],[1,1−iδ]]. 1 = Bagula's groups; toward 0 the generators lose their quasiconformal stretch and the limit set collapses; > 1 exaggerates it."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Generator"], "Direct-color source (needs the transform's Direct Color > 0). Generator: each group element has its own palette position, blended through a persistent color register at Color Speed."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the Generator color (wrapped with fract)."),
        param!("color_speed", "Color Speed", float, 0.5, 0.0, 1.0, "How hard each generator pulls the color register toward its own palette position. Low = long blends of orbit history, 1 = hard per-generator assignment."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_su_mobius(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {

    let group = u32(get_param(xform_id, variation_id, 0u));
    let avoid = get_param(xform_id, variation_id, 1u) > 0.5;
    let theta = get_param(xform_id, variation_id, 2u) * 0.01745329252;
    let eta = get_param(xform_id, variation_id, 3u) * 0.01745329252;
    let delta = get_param(xform_id, variation_id, 4u);
    let dc_mode = u32(get_param(xform_id, variation_id, 5u));
    let dc_scale = get_param(xform_id, variation_id, 6u);
    let color_speed = get_param(xform_id, variation_id, 7u);

    let rc = su_group_range(group);
    let cnt = rc.y;
    let half = cnt / 2u;
    let cj = su_conjugator(theta, eta, delta);
    let cji = su_matinv(cj);

    var k = min(u32(rng_nextf(rng) * f32(cnt)), cnt - 1u);
    if (avoid) {
        let prev = u32(get_state(xform_id, variation_id, 0u));
        if (prev < cnt && k == (prev + half) % cnt) {
            k = (k + 1u) % cnt;
        }
    }
    set_state(xform_id, variation_id, 0u, f32(k));

    if (dc_mode == 1u) {
        var creg = get_state(xform_id, variation_id, 1u);
        creg = mix(creg, fract((f32(k) + 0.5) / f32(cnt) * dc_scale), color_speed);
        set_state(xform_id, variation_id, 1u, creg);
        *vc = creg;
    }
    let gidx = rc.x + k;
    return su_mobius_apply(gidx, p, cj, cji);
}
"#;

const WGSL_3D: &str = r#"
fn variation_su_mobius(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {

    let group = u32(get_param(xform_id, variation_id, 0u));
    let avoid = get_param(xform_id, variation_id, 1u) > 0.5;
    let theta = get_param(xform_id, variation_id, 2u) * 0.01745329252;
    let eta = get_param(xform_id, variation_id, 3u) * 0.01745329252;
    let delta = get_param(xform_id, variation_id, 4u);
    let dc_mode = u32(get_param(xform_id, variation_id, 5u));
    let dc_scale = get_param(xform_id, variation_id, 6u);
    let color_speed = get_param(xform_id, variation_id, 7u);

    let rc = su_group_range(group);
    let cnt = rc.y;
    let half = cnt / 2u;
    let cj = su_conjugator(theta, eta, delta);
    let cji = su_matinv(cj);

    var k = min(u32(rng_nextf(rng) * f32(cnt)), cnt - 1u);
    if (avoid) {
        let prev = u32(get_state(xform_id, variation_id, 0u));
        if (prev < cnt && k == (prev + half) % cnt) {
            k = (k + 1u) % cnt;
        }
    }
    set_state(xform_id, variation_id, 0u, f32(k));

    if (dc_mode == 1u) {
        var creg = get_state(xform_id, variation_id, 1u);
        creg = mix(creg, fract((f32(k) + 0.5) / f32(cnt) * dc_scale), color_speed);
        set_state(xform_id, variation_id, 1u, creg);
        *vc = creg;
    }
    let gidx = rc.x + k;
    return vec3<f32>(su_mobius_apply(gidx, p.xy, cj, cji), p.z);
}
"#;
