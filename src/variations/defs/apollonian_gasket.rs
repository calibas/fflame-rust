//! `apollonian_gasket` — the Apollonian gasket Kleinian group
//! (*Indra's Pearls* Ch. 7).
//!
//! The specific parabolic two-generator group whose limit set is the
//! exact "even" Apollonian gasket (Mumford, Series & Wright,
//! *Indra's Pearls* Ch. 7, the "glowing gasket"):
//!
//! ```text
//! a = [[1, 0], [−2i, 1]]      b = [[1−i, 1], [1, 1+i]]
//! ```
//!
//! Both parabolic (trace 2) with parabolic commutator
//! (tr aba⁻¹b⁻¹ = −2, verified numerically) — the triple-tangency
//! condition that makes every circle in the limit set kiss its
//! neighbours. Chaos game over `{a, b, a⁻¹, b⁻¹}` with
//! `avoid_reversal`, exactly the *Indra's Pearls* algorithm
//! (`klein_group`'s Grandma recipe at ta = tb = 2 reaches a Möbius
//! -equivalent group; this is the canonical gasket framing).
//!
//! `qc_deform` applies Bagula's triquasiconformal conjugation
//! `C = dk(δ)·s0·qf(θ+iη)` from the shared library to every generator —
//! the "even → uneven" gasket deformation (conjugation is a Möbius
//! change of coordinates: circles stay circles, tangencies stay
//! tangencies, but the packing warps out of its symmetric frame). Off
//! by default so the untouched even gasket renders.
//!
//! `space` = Hyperbolic H3 uses the Poincaré extension from the shared
//! library: the same matrices act on upper-half-space quaternions,
//! filling the gasket out to the Apollonian **sphere** packing.
//!
//! Uses `Feature::NeedsMobiusLib` (`shaders/core/su_mobius.wgsl`).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static APOLLONIAN_GASKET: VariationDef = VariationDef {
    name: "apollonian_gasket",
    aliases: &[],
    display_name: "Apollonian Gasket",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor, Feature::NeedsMobiusLib],
    // Slot 0: previous generator index (avoid_reversal). Slot 1: color
    // register for the Generator color mode.
    state_count: 2,
    wgsl_state_init: None,
    parameters: &[
        param!("avoid_reversal", "Avoid Reversal", bool, true, "Skip a generator's inverse immediately after applying it (no g·g⁻¹ cancellation) — the Indra's Pearls chaos-game rule. Off = all four generators equally likely every call."),
        param!("qc_deform", "QC Deform", bool, false, "Apply Bagula's triquasiconformal conjugation C = dk(δ)·s0·qf(θ+iη) to every generator — the 'even → uneven' gasket deformation. Conjugation is a Möbius change of coordinates: circles stay circles and tangencies stay tangencies, but the packing warps out of its symmetric frame. Off = the exact even gasket."),
        param!("conj_angle", "Angle", angle, 45.0, "Elliptic rotation θ in the conjugator qf = rotate(θ + iη) (QC Deform on). Sweeping it slides the gasket through uneven framings."),
        param!("conj_hyper", "Hyper Angle", angle, 0.0, "Hyperbolic rotation η (imaginary angle) in qf = rotate(θ + iη) (QC Deform on). Compresses the packing toward a limit point."),
        param!("qc_strength", "QC Strength", float, 1.0, 0.1, 2.0, "Quasiconformal δ in dk = [[1+iδ,1],[1,1−iδ]] (QC Deform on)."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Generator"], "Direct-color source (needs the transform's Direct Color > 0). Generator: each of the four group elements has its own palette position, blended through a persistent color register at Color Speed."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the Generator color (wrapped with fract)."),
        param!("color_speed", "Color Speed", float, 0.5, 0.0, 1.0, "How hard each generator pulls the color register toward its own palette position."),
        param!("space", "Space", enum, 0, &["Planar", "Hyperbolic H3"], "3D render mode only. Planar: the flat gasket in the xy plane (z passes through). Hyperbolic H3: the Poincaré extension — the same SL(2,C) matrices act on upper-half-space (the point as a quaternion x+yi+t·j), filling the gasket out to the Apollonian sphere packing. The z-slice at t→0 is the 2D picture."),
        param!("symmetry", "Symmetry", enum, 0, &["None", "2-Fold Point", "2-Fold Mirror", "4-Fold"], "Random output symmetrization each call: 2-Fold Point = {z,−z}, 2-Fold Mirror = {z,conj z}, 4-Fold = both."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Generator table shared by both bodies: a, b, a⁻¹, b⁻¹ (inverse of
// local index k is (k + 2) mod 4). det = 1 throughout, so inverses are
// the [d,−b,−c,a] shortcut.
const WGSL_2D: &str = r#"
fn apollonian_gen(k: u32) -> SuMat {
    switch k {
        case 0u: { return SuMat(vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 0.0), vec2<f32>(0.0, -2.0), vec2<f32>(1.0, 0.0)); }
        case 1u: { return SuMat(vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0)); }
        case 2u: { return SuMat(vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 0.0), vec2<f32>(0.0, 2.0), vec2<f32>(1.0, 0.0)); }
        default: { return SuMat(vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 0.0), vec2<f32>(-1.0, 0.0), vec2<f32>(1.0, -1.0)); }
    }
}

fn variation_apollonian_gasket(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let avoid = get_param(xform_id, variation_id, 0u) > 0.5;
    let deform = get_param(xform_id, variation_id, 1u) > 0.5;
    let theta = get_param(xform_id, variation_id, 2u) * 0.01745329252;
    let eta = get_param(xform_id, variation_id, 3u) * 0.01745329252;
    let delta = get_param(xform_id, variation_id, 4u);
    let dc_mode = u32(get_param(xform_id, variation_id, 5u));
    let dc_scale = get_param(xform_id, variation_id, 6u);
    let color_speed = get_param(xform_id, variation_id, 7u);

    var k = min(u32(rng_nextf(rng) * 4.0), 3u);
    if (avoid) {
        let prev = u32(get_state(xform_id, variation_id, 0u));
        if (prev < 4u && k == (prev + 2u) % 4u) {
            k = (k + 1u) % 4u;
        }
    }
    set_state(xform_id, variation_id, 0u, f32(k));

    if (dc_mode == 1u) {
        var creg = get_state(xform_id, variation_id, 1u);
        creg = mix(creg, fract((f32(k) + 0.5) / 4.0 * dc_scale), color_speed);
        set_state(xform_id, variation_id, 1u, creg);
        *vc = creg;
    }

    let g = apollonian_gen(k);
    var out: vec2<f32>;
    if (deform) {
        let cj = su_conjugator(theta, eta, delta);
        out = su_apply_m(g, p, cj, su_matinv(cj));
    } else {
        out = su_apply_plain(g, p);
    }
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
fn apollonian_gen(k: u32) -> SuMat {
    switch k {
        case 0u: { return SuMat(vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 0.0), vec2<f32>(0.0, -2.0), vec2<f32>(1.0, 0.0)); }
        case 1u: { return SuMat(vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 0.0), vec2<f32>(1.0, 1.0)); }
        case 2u: { return SuMat(vec2<f32>(1.0, 0.0), vec2<f32>(0.0, 0.0), vec2<f32>(0.0, 2.0), vec2<f32>(1.0, 0.0)); }
        default: { return SuMat(vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 0.0), vec2<f32>(-1.0, 0.0), vec2<f32>(1.0, -1.0)); }
    }
}

fn variation_apollonian_gasket(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let avoid = get_param(xform_id, variation_id, 0u) > 0.5;
    let deform = get_param(xform_id, variation_id, 1u) > 0.5;
    let theta = get_param(xform_id, variation_id, 2u) * 0.01745329252;
    let eta = get_param(xform_id, variation_id, 3u) * 0.01745329252;
    let delta = get_param(xform_id, variation_id, 4u);
    let dc_mode = u32(get_param(xform_id, variation_id, 5u));
    let dc_scale = get_param(xform_id, variation_id, 6u);
    let color_speed = get_param(xform_id, variation_id, 7u);

    var k = min(u32(rng_nextf(rng) * 4.0), 3u);
    if (avoid) {
        let prev = u32(get_state(xform_id, variation_id, 0u));
        if (prev < 4u && k == (prev + 2u) % 4u) {
            k = (k + 1u) % 4u;
        }
    }
    set_state(xform_id, variation_id, 0u, f32(k));

    if (dc_mode == 1u) {
        var creg = get_state(xform_id, variation_id, 1u);
        creg = mix(creg, fract((f32(k) + 0.5) / 4.0 * dc_scale), color_speed);
        set_state(xform_id, variation_id, 1u, creg);
        *vc = creg;
    }

    let g = apollonian_gen(k);
    let space = u32(get_param(xform_id, variation_id, 8u));
    var out: vec3<f32>;
    if (deform) {
        let cj = su_conjugator(theta, eta, delta);
        let cji = su_matinv(cj);
        if (space == 1u) { out = su_apply_m3(g, p, cj, cji); }
        else { out = vec3<f32>(su_apply_m(g, p.xy, cj, cji), p.z); }
    } else if (space == 1u) {
        out = su_apply_plain3(g, p);
    } else {
        out = vec3<f32>(su_apply_plain(g, p.xy), p.z);
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
