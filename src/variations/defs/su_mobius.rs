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
//! This variation covers the BAKED generator tables only (group
//! indices 0–4: SU(2) 6-Group, SU(3), SU(4), SU(5), SO(5) reduced).
//! The live reduction-tensor groups (Reduce sliders + init pass) live
//! in [`su_custom`](super::su_custom); Bagula's ⟨2,3,12⟩ Kleinian
//! *triangle* group is a separate construction (not a special-unitary
//! reduction) and lives in [`fuchsian_triangle`](super::fuchsian_triangle).
//! All three share the SL(2,ℂ) machinery in `shaders/core/su_mobius.wgsl`.
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
    features: &[Feature::NeedsRng, Feature::WritesColor, Feature::NeedsMobiusLib],
    // Slot 0: previous generator index (avoid_reversal). Slot 1: color
    // register for the Generator color mode.
    state_count: 2,
    wgsl_state_init: None,
    parameters: &[
        param!("group", "Group", enum, 1, &["SU(2) 6-Group", "SU(3) Reduced", "SU(4) Reduced", "SU(5) Reduced", "SO(5) Reduced"], "Which baked Lie-group generator set. SU(2) 6-Group (12 generators) — set Angle 0 / Hyper Angle 45 for Bagula's hypertriquasiconformal Apollonian disk. SU(3) Reduced (16 generators) — Angle 45 / Hyper 0 (the defaults) for the three-quark-disk limit set. SU(4) Reduced (30 gens, our baked reduction). SU(5) Reduced (46 gens, our reduction) — pair with Symmetry 4-Fold. SO(5) Reduced (20 gens): the antisymmetric subset of the generalized Gell-Mann set is exactly so(5) ≅ sp(4) — the rotation-only cousin of SU(5). For LIVE reduction-tensor groups see the SU(n) Custom variation; for Bagula's ⟨2,3,12⟩ triangle group see Fuchsian Triangle."),
        param!("avoid_reversal", "Avoid Reversal", bool, true, "Skip a generator's inverse immediately after applying it (no g·g⁻¹ cancellation), so the orbit drifts through the limit set instead of dithering on a small subset. Off = all generators equally likely every call."),
        param!("conj_angle", "Angle", angle, 45.0, "Elliptic rotation θ in the conjugator qf = rotate(θ + iη). SU(3)'s triquasiconformal '45'. Sweeping it deforms the whole limit set."),
        param!("conj_hyper", "Hyper Angle", angle, 0.0, "HYPERBOLIC rotation η (imaginary angle) in qf = rotate(θ + iη) — the 'hyper' in SU(2)'s hypertriquasiconformal. 45° with Angle 0 is Bagula's SU(2) 6-group; nonzero η bends the group loxodromically, compacting lattices toward Apollonian disks."),
        param!("qc_strength", "QC Strength", float, 1.0, 0.0, 2.0, "Quasiconformal deformation δ in dk = [[1+iδ,1],[1,1−iδ]]. 1 = Bagula's groups; toward 0 the generators lose their quasiconformal stretch and the limit set collapses; > 1 exaggerates it."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Generator"], "Direct-color source (needs the transform's Direct Color > 0). Generator: each group element has its own palette position, blended through a persistent color register at Color Speed."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the Generator color (wrapped with fract)."),
        param!("color_speed", "Color Speed", float, 0.5, 0.0, 1.0, "How hard each generator pulls the color register toward its own palette position. Low = long blends of orbit history, 1 = hard per-generator assignment."),
        param!("space", "Space", enum, 0, &["Planar", "Hyperbolic H3"], "3D render mode only. Planar: the ordinary complex Möbius map in the xy plane (z passes through) — the flat limit set. Hyperbolic H3: the Poincaré extension — SL(2,C) is exactly the isometry group of hyperbolic 3-space, so each generator acts on upper-half-space (the point as a quaternion x+yi+t·j, t = height) via (Aq+B)(Cq+D)⁻¹. The limit set fills 3D: SU(3)'s three disks become three spheres, SU(2)'s Apollonian disk an Apollonian sphere-packing. The z-slice at t→0 is the 2D picture."),
        param!("symmetry", "Symmetry", enum, 0, &["None", "2-Fold Point", "2-Fold Mirror", "4-Fold"], "4-Fold applies a random one of {z, −z, conj z, −conj z} to the output each call — 2-Fold Point = {z,−z} (180° rotation), 2-Fold Mirror = {z,conj z} (reflection), 4-Fold = both (Bagula's SU(5) orbit-symmetrization for the 'symmetry enhanced' elliptical picture). Usable on any group."),
    ],
    init_param_count: 0,
    wgsl_init: None,
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

    var out = su_mobius_apply(rc.x + k, p, cj, cji);
    let symm = u32(get_param(xform_id, variation_id, 9u));
    if (symm == 1u) { if (rng_nextf(rng) < 0.5) { out = -out; } }
    else if (symm == 2u) { if (rng_nextf(rng) < 0.5) { out = vec2<f32>(out.x, -out.y); } }
    else if (symm == 3u) {
        let r = min(u32(rng_nextf(rng) * 4.0), 3u);
        if (r == 1u) { out = -out; }
        else if (r == 2u) { out = vec2<f32>(out.x, -out.y); }
        else if (r == 3u) { out = vec2<f32>(-out.x, out.y); }
    }
    return out;
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
    let space = u32(get_param(xform_id, variation_id, 8u));
    var out: vec3<f32>;
    if (space == 1u) {
        out = su_mobius_apply3(gidx, p, cj, cji);
    } else {
        out = vec3<f32>(su_mobius_apply(gidx, p.xy, cj, cji), p.z);
    }
    let symm = u32(get_param(xform_id, variation_id, 9u));
    if (symm == 1u) { if (rng_nextf(rng) < 0.5) { out = vec3<f32>(-out.x, -out.y, out.z); } }
    else if (symm == 2u) { if (rng_nextf(rng) < 0.5) { out = vec3<f32>(out.x, -out.y, out.z); } }
    else if (symm == 3u) {
        let r = min(u32(rng_nextf(rng) * 4.0), 3u);
        if (r == 1u) { out = vec3<f32>(-out.x, -out.y, out.z); }
        else if (r == 2u) { out = vec3<f32>(out.x, -out.y, out.z); }
        else if (r == 3u) { out = vec3<f32>(-out.x, out.y, out.z); }
    }
    return out;
}
"#;
