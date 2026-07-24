//! `schottky_group` — classical Schottky circle-pairing chaos game
//! (*Indra's Pearls* Ch. 4).
//!
//! The oldest Kleinian construction: four circles C₁…C₄ in the plane,
//! generator `a` pairing C₁→C₂ and `b` pairing C₃→C₄. Each pairing is
//! the classical (Fuchsian-style) map
//!
//! ```text
//! a(z) = c₂ + r₁·r₂ / (z − c₁)
//!      = [[c₂, r₁r₂ − c₁c₂], [1, −c₁]] / √(−r₁r₂)
//! ```
//!
//! which sends ∂C₁ → ∂C₂ and the *exterior* of C₁ into the *interior*
//! of C₂ (verified numerically; det = 1 after normalization). While the
//! four circles are mutually disjoint the group is free and the limit
//! set is fractal dust scattered on the circles' orbit; as circles
//! approach tangency the dust condenses toward connected gasket-like
//! packings — the most slider-explorable Kleinian family there is. The
//! default kissing-adjacent square arrangement (centers ±1.2 on each
//! axis, radius 0.8) starts just short of tangency.
//!
//! Chaos game over `{a, b, a⁻¹, b⁻¹}` with the *Indra's Pearls*
//! backtrack-avoid; the circle + twist sliders feed an init pass that
//! bakes the four normalized matrices into derived param slots. Each
//! pairing carries a free rotation (the Schottky *marking*, the Twist
//! sliders): at the default square arrangement pushed to kissing
//! (r ≈ 0.849) the default twists give tr[a,b] = −2 exactly — the
//! parabolic gluing that condenses the dust into the connected
//! θ-Schottky quasi-circle (found by numeric scan).
//!
//! `space` = Hyperbolic H3: the Poincaré extension — the circles become
//! hemispheres and the dust a 3D sphere-orbit cloud.
//!
//! Uses `Feature::NeedsMobiusLib` (`shaders/core/su_mobius.wgsl`).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Classical Schottky circle-pairing chaos game (Indra's Pearls Ch. 4).
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static SCHOTTKY_GROUP: VariationDef = VariationDef {
    name: "schottky_group",
    aliases: &[],
    display_name: "Schottky Group",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor, Feature::NeedsMobiusLib, Feature::AlwaysZ],
    // Slot 0: previous generator index (avoid_reversal). Slot 1: color
    // register for the Generator color mode.
    state_count: 2,
    wgsl_state_init: None,
    parameters: &[
        param!("c1_x", "C1 x", unlimited_float, -1.2, -4.0, 4.0, "Center x of circle C1 — generator a maps the exterior of C1 into the interior of C2. Keep the four circles disjoint for a free (dust) group; push them toward tangency for connected gasket-like limit sets."),
        param!("c1_y", "C1 y", unlimited_float, 0.0, -4.0, 4.0, "Center y of circle C1."),
        param!("r1", "R1", float, 0.8, 0.05, 3.0, "Radius of circle C1."),
        param!("c2_x", "C2 x", unlimited_float, 1.2, -4.0, 4.0, "Center x of circle C2 (the image circle of generator a)."),
        param!("c2_y", "C2 y", unlimited_float, 0.0, -4.0, 4.0, "Center y of circle C2."),
        param!("r2", "R2", float, 0.8, 0.05, 3.0, "Radius of circle C2."),
        param!("c3_x", "C3 x", unlimited_float, 0.0, -4.0, 4.0, "Center x of circle C3 — generator b maps the exterior of C3 into the interior of C4."),
        param!("c3_y", "C3 y", unlimited_float, -1.2, -4.0, 4.0, "Center y of circle C3."),
        param!("r3", "R3", float, 0.8, 0.05, 3.0, "Radius of circle C3."),
        param!("c4_x", "C4 x", unlimited_float, 0.0, -4.0, 4.0, "Center x of circle C4 (the image circle of generator b)."),
        param!("c4_y", "C4 y", unlimited_float, 1.2, -4.0, 4.0, "Center y of circle C4."),
        param!("r4", "R4", float, 0.8, 0.05, 3.0, "Radius of circle C4."),
        param!("avoid_reversal", "Avoid Reversal", bool, true, "Skip a generator's inverse immediately after applying it (no g·g⁻¹ cancellation) — the Indra's Pearls chaos-game rule."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Generator"], "Direct-color source (needs the transform's Direct Color > 0). Generator: each of the four group elements has its own palette position, blended through a persistent color register at Color Speed."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the Generator color (wrapped with fract)."),
        param!("color_speed", "Color Speed", float, 0.5, 0.0, 1.0, "How hard each generator pulls the color register toward its own palette position."),
        param!("space", "Space", enum, 0, &["Euclidean", "Hyperbolic H3"], "3D render mode only. Euclidean: the Möbius orbit in the xy plane (z passes through). Hyperbolic H3: the Poincaré extension — the pairing circles become hemispheres acting on upper half-space. The t→0 slice is the 2D picture."),
        param!("symmetry", "Symmetry", enum, 0, &["None", "2-Fold Point", "2-Fold Mirror", "4-Fold"], "Random output symmetrization each call: 2-Fold Point = {z,−z}, 2-Fold Mirror = {z,conj z}, 4-Fold = both."),
        param!("twist_a", "Twist A", angle, 180.0, "Rotation of generator a about C1 before pairing — the free 'marking' of a Schottky pairing (any rotation composed with a circle pairing still pairs the circles). It controls how the group glues at tangency: at the default square arrangement pushed to kissing (r ≈ 0.849), Twist A = 180 / Twist B = 0 gives tr[a,b] = −2 exactly — the parabolic gluing that condenses the dust into the connected θ-Schottky quasi-circle."),
        param!("twist_b", "Twist B", angle, 0.0, "Rotation of generator b about C3 before pairing (see Twist A)."),
    ],
    // Four matrices × 8 floats: a, b at slots 0/8, inverses at 16/24.
    // Read at bo = n_user (20) + k·8.
    init_param_count: 32,
    wgsl_init: Some(WGSL_INIT),
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_INIT: &str = r#"
fn init_schottky_group(user: array<f32, 20>) -> array<f32, 32> {
    var out: array<f32, 32>;
    // Pairing matrix C_src -> C_dst: [[c2, r1r2 - c1c2], [1, -c1]],
    // det = -r1*r2, normalized by csqrt to det 1. Inverse of a det-1
    // matrix is [d, -b, -c, a].
    for (var pair = 0u; pair < 2u; pair = pair + 1u) {
        let o = pair * 6u;
        let c1 = vec2<f32>(user[o], user[o + 1u]);
        let r1 = max(user[o + 2u], 0.05);
        let c2 = vec2<f32>(user[o + 3u], user[o + 4u]);
        let r2 = max(user[o + 5u], 0.05);
        let ma = c2;
        let mb = vec2<f32>(r1 * r2, 0.0) - cmul(c1, c2);
        let mc = vec2<f32>(1.0, 0.0);
        let md = -c1;
        // Compose with the marking rotation about c1: R = [[e^{i phi},
        // c1(1 - e^{i phi})], [0, 1]] (Twist A/B; user slots 18/19).
        let phi = user[18u + pair] * 0.01745329252;
        let e = vec2<f32>(cos(phi), sin(phi));
        let rb = cmul(c1, vec2<f32>(1.0, 0.0) - e);
        let ta = cmul(ma, e);
        let tb = cmul(ma, rb) + mb;
        let tc = cmul(mc, e);
        let td = cmul(mc, rb) + md;
        let sd = csqrt(csub(cmul(ta, td), cmul(tb, tc)));
        let na = cdiv(ta, sd); let nb = cdiv(tb, sd);
        let nc = cdiv(tc, sd); let nd = cdiv(td, sd);
        let ob = pair * 8u;       // a at 0, b at 8
        let oi = 16u + pair * 8u; // inverses at 16, 24
        out[ob] = na.x; out[ob + 1u] = na.y; out[ob + 2u] = nb.x; out[ob + 3u] = nb.y;
        out[ob + 4u] = nc.x; out[ob + 5u] = nc.y; out[ob + 6u] = nd.x; out[ob + 7u] = nd.y;
        out[oi] = nd.x; out[oi + 1u] = nd.y; out[oi + 2u] = -nb.x; out[oi + 3u] = -nb.y;
        out[oi + 4u] = -nc.x; out[oi + 5u] = -nc.y; out[oi + 6u] = na.x; out[oi + 7u] = na.y;
    }
    return out;
}
"#;

// Generator order: a, b, a⁻¹, b⁻¹ — the inverse of local index k is
// (k + 2) mod 4. Derived matrices start at slot 18 (user param count).
const WGSL_2D: &str = r#"
fn schottky_read(xform_id: u32, variation_id: u32, k: u32) -> SuMat {
    let bo = 20u + k * 8u;
    return SuMat(
        vec2<f32>(get_param(xform_id, variation_id, bo), get_param(xform_id, variation_id, bo + 1u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 2u), get_param(xform_id, variation_id, bo + 3u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 4u), get_param(xform_id, variation_id, bo + 5u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 6u), get_param(xform_id, variation_id, bo + 7u)));
}

fn variation_schottky_group(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let avoid = get_param(xform_id, variation_id, 12u) > 0.5;
    let dc_mode = u32(get_param(xform_id, variation_id, 13u));
    let dc_scale = get_param(xform_id, variation_id, 14u);
    let color_speed = get_param(xform_id, variation_id, 15u);

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

    var out = su_apply_plain(schottky_read(xform_id, variation_id, k), p);
    let symm = u32(get_param(xform_id, variation_id, 17u));
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
fn schottky_read(xform_id: u32, variation_id: u32, k: u32) -> SuMat {
    let bo = 20u + k * 8u;
    return SuMat(
        vec2<f32>(get_param(xform_id, variation_id, bo), get_param(xform_id, variation_id, bo + 1u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 2u), get_param(xform_id, variation_id, bo + 3u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 4u), get_param(xform_id, variation_id, bo + 5u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 6u), get_param(xform_id, variation_id, bo + 7u)));
}

fn variation_schottky_group(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let avoid = get_param(xform_id, variation_id, 12u) > 0.5;
    let dc_mode = u32(get_param(xform_id, variation_id, 13u));
    let dc_scale = get_param(xform_id, variation_id, 14u);
    let color_speed = get_param(xform_id, variation_id, 15u);

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

    let g = schottky_read(xform_id, variation_id, k);
    let space = u32(get_param(xform_id, variation_id, 16u));
    var out: vec3<f32>;
    if (space == 1u) { out = su_apply_plain3(g, p); }
    else { out = vec3<f32>(su_apply_plain(g, p.xy), p.z); }
    let symm = u32(get_param(xform_id, variation_id, 17u));
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
