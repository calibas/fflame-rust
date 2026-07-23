//! `lorentz_mobius` — five SL(2,ℂ) Lorentz generators as a Möbius chaos
//! game (Roger Bagula).
//!
//! SL(2,ℂ) is the double cover of the Lorentz group: acting on the
//! Riemann sphere (the celestial sphere), rotations and boosts are
//! Möbius maps — a boost is literally relativistic aberration of the
//! star field. Bagula's McMullen-notebook program (the
//! `two_Programs_McMullen_Nylander_*` series opens with "Dirac five
//! SL(2,C)" and builds these in its first cells) takes five one-
//! parameter subgroup elements via `sl2c[θ,η] = MatrixExp[Σθᵢ·σᵢ/2 +
//! Ση ᵢ·i σᵢ/2]`:
//!
//! ```text
//! g1 = sl2c[{0,0,0},{0,0, 1}]   ("boost +z")
//! g2 = sl2c[{0,0,0},{0,0,−1}]   ("boost −z")
//! g3 = sl2c[{0,0,1},{0,0,0}]    ("rotation z")
//! g4 = sl2c[{0,1,0},{0,0,0}]    ("rotation y")
//! g5 = sl2c[{1,0,0},{0,0,0}]    ("rotation x")
//! ```
//!
//! Since σₙ² = I every element has the closed form
//! `exp(m·σₙ/2) = cosh(m/2)·I + sinh(m/2)·σₙ` (verified against
//! MatrixExp) — real m in the θ-slot, imaginary in the η-slot. Note
//! the notebook's labels are the reverse of the physics convention:
//! its θ-slot "rotations" exponentiate σ/2 (hyperbolic ⇒ boost-like
//! Möbius maps) and its η-slot "boosts" exponentiate iσ/2 (elliptic ⇒
//! rotation-like). We port the construction as written and expose the
//! two slot magnitudes as sliders (both 1 in the notebook).
//!
//! Chaos game over the five generators + inverses (10 elements,
//! inverse of k is k+5). The elliptic pair orbits circles about the
//! ±z celestial poles; the three hyperbolic elements squeeze the
//! sphere toward their fixed points — their interplay draws flow-line
//! fans between the poles of the light cone.
//!
//! `space` = Hyperbolic H3: the Poincaré extension — SL(2,ℂ) is also
//! Isom(H³), so the same matrices act on upper-half-space quaternions.
//!
//! Uses `Feature::NeedsMobiusLib` (`shaders/core/su_mobius.wgsl`).

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static LORENTZ_MOBIUS: VariationDef = VariationDef {
    name: "lorentz_mobius",
    aliases: &[],
    display_name: "Lorentz Möbius",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor, Feature::NeedsMobiusLib],
    // Slot 0: previous generator index (avoid_reversal). Slot 1: color
    // register for the Generator color mode.
    state_count: 2,
    wgsl_state_init: None,
    parameters: &[
        param!("theta", "Theta", unlimited_float, 1.0, -4.0, 4.0, "Magnitude of the θ-slot generators (the notebook's 'rotation z/y/x' trio g3–g5). These exponentiate σ/2, so as Möbius maps they are HYPERBOLIC — boost-like squeezes toward antipodal fixed points (the notebook's labels are the reverse of the physics convention). Notebook value: 1."),
        param!("eta", "Eta", unlimited_float, 1.0, -4.0, 4.0, "Magnitude of the η-slot generators (the notebook's 'boost ±z' pair g1–g2). These exponentiate iσ/2, so as Möbius maps they are ELLIPTIC — rotations about the ±z celestial poles by η radians. Notebook value: 1."),
        param!("avoid_reversal", "Avoid Reversal", bool, true, "Skip a generator's inverse immediately after applying it (no g·g⁻¹ cancellation; the inverse of generator k is k+5). Off = all ten elements equally likely every call."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Generator"], "Direct-color source (needs the transform's Direct Color > 0). Generator: each of the ten group elements has its own palette position, blended through a persistent color register at Color Speed."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the Generator color (wrapped with fract)."),
        param!("color_speed", "Color Speed", float, 0.5, 0.0, 1.0, "How hard each generator pulls the color register toward its own palette position."),
        param!("space", "Space", enum, 0, &["Planar", "Hyperbolic H3"], "3D render mode only. Planar: the celestial-sphere Möbius action in the xy plane (z passes through). Hyperbolic H3: the Poincaré extension — the same matrices act on upper-half-space (SL(2,C) = Isom(H³)). The t→0 slice is the 2D picture."),
        param!("symmetry", "Symmetry", enum, 0, &["None", "2-Fold Point", "2-Fold Mirror", "4-Fold"], "Random output symmetrization each call: 2-Fold Point = {z,−z}, 2-Fold Mirror = {z,conj z}, 4-Fold = both."),
    ],
    init_param_count: 0,
    wgsl_init: None,
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

// Closed forms (σₙ² = I ⇒ exp(m·σₙ/2) = cosh(m/2)I + sinh(m/2)σₙ):
//   k 0,1: exp(±i·η·σ3/2) = diag(e^{±iη/2}, e^{∓iη/2})     (elliptic)
//   k 2:   exp(θ·σ3/2)    = diag(e^{θ/2}, e^{−θ/2})         (hyperbolic)
//   k 3:   exp(θ·σ2/2)    = [[ch, −i·sh], [i·sh, ch]]
//   k 4:   exp(θ·σ1/2)    = [[ch, sh], [sh, ch]]
//   k 5–9: inverses (negate the argument).
const WGSL_2D: &str = r#"
fn lorentz_gen(k: u32, theta: f32, eta: f32) -> SuMat {
    let base = k % 5u;
    let sgn = select(1.0, -1.0, k >= 5u);
    if (base == 0u || base == 1u) {
        var h = eta * 0.5 * sgn;
        if (base == 1u) { h = -h; }
        let c = cos(h); let s = sin(h);
        return SuMat(vec2<f32>(c, s), vec2<f32>(0.0, 0.0), vec2<f32>(0.0, 0.0), vec2<f32>(c, -s));
    }
    let h = theta * 0.5 * sgn;
    let ch = cosh(h); let sh = sinh(h);
    if (base == 2u) {
        return SuMat(vec2<f32>(ch + sh, 0.0), vec2<f32>(0.0, 0.0), vec2<f32>(0.0, 0.0), vec2<f32>(ch - sh, 0.0));
    }
    if (base == 3u) {
        return SuMat(vec2<f32>(ch, 0.0), vec2<f32>(0.0, -sh), vec2<f32>(0.0, sh), vec2<f32>(ch, 0.0));
    }
    return SuMat(vec2<f32>(ch, 0.0), vec2<f32>(sh, 0.0), vec2<f32>(sh, 0.0), vec2<f32>(ch, 0.0));
}

fn variation_lorentz_mobius(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let theta = get_param(xform_id, variation_id, 0u);
    let eta = get_param(xform_id, variation_id, 1u);
    let avoid = get_param(xform_id, variation_id, 2u) > 0.5;
    let dc_mode = u32(get_param(xform_id, variation_id, 3u));
    let dc_scale = get_param(xform_id, variation_id, 4u);
    let color_speed = get_param(xform_id, variation_id, 5u);

    var k = min(u32(rng_nextf(rng) * 10.0), 9u);
    if (avoid) {
        let prev = u32(get_state(xform_id, variation_id, 0u));
        if (prev < 10u && k == (prev + 5u) % 10u) {
            k = (k + 1u) % 10u;
        }
    }
    set_state(xform_id, variation_id, 0u, f32(k));

    if (dc_mode == 1u) {
        var creg = get_state(xform_id, variation_id, 1u);
        creg = mix(creg, fract((f32(k) + 0.5) / 10.0 * dc_scale), color_speed);
        set_state(xform_id, variation_id, 1u, creg);
        *vc = creg;
    }

    var out = su_apply_plain(lorentz_gen(k, theta, eta), p);
    let symm = u32(get_param(xform_id, variation_id, 7u));
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
fn lorentz_gen(k: u32, theta: f32, eta: f32) -> SuMat {
    let base = k % 5u;
    let sgn = select(1.0, -1.0, k >= 5u);
    if (base == 0u || base == 1u) {
        var h = eta * 0.5 * sgn;
        if (base == 1u) { h = -h; }
        let c = cos(h); let s = sin(h);
        return SuMat(vec2<f32>(c, s), vec2<f32>(0.0, 0.0), vec2<f32>(0.0, 0.0), vec2<f32>(c, -s));
    }
    let h = theta * 0.5 * sgn;
    let ch = cosh(h); let sh = sinh(h);
    if (base == 2u) {
        return SuMat(vec2<f32>(ch + sh, 0.0), vec2<f32>(0.0, 0.0), vec2<f32>(0.0, 0.0), vec2<f32>(ch - sh, 0.0));
    }
    if (base == 3u) {
        return SuMat(vec2<f32>(ch, 0.0), vec2<f32>(0.0, -sh), vec2<f32>(0.0, sh), vec2<f32>(ch, 0.0));
    }
    return SuMat(vec2<f32>(ch, 0.0), vec2<f32>(sh, 0.0), vec2<f32>(sh, 0.0), vec2<f32>(ch, 0.0));
}

fn variation_lorentz_mobius(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let theta = get_param(xform_id, variation_id, 0u);
    let eta = get_param(xform_id, variation_id, 1u);
    let avoid = get_param(xform_id, variation_id, 2u) > 0.5;
    let dc_mode = u32(get_param(xform_id, variation_id, 3u));
    let dc_scale = get_param(xform_id, variation_id, 4u);
    let color_speed = get_param(xform_id, variation_id, 5u);

    var k = min(u32(rng_nextf(rng) * 10.0), 9u);
    if (avoid) {
        let prev = u32(get_state(xform_id, variation_id, 0u));
        if (prev < 10u && k == (prev + 5u) % 10u) {
            k = (k + 1u) % 10u;
        }
    }
    set_state(xform_id, variation_id, 0u, f32(k));

    if (dc_mode == 1u) {
        var creg = get_state(xform_id, variation_id, 1u);
        creg = mix(creg, fract((f32(k) + 0.5) / 10.0 * dc_scale), color_speed);
        set_state(xform_id, variation_id, 1u, creg);
        *vc = creg;
    }

    let g = lorentz_gen(k, theta, eta);
    let space = u32(get_param(xform_id, variation_id, 6u));
    var out: vec3<f32>;
    if (space == 1u) { out = su_apply_plain3(g, p); }
    else { out = vec3<f32>(su_apply_plain(g, p.xy), p.z); }
    let symm = u32(get_param(xform_id, variation_id, 7u));
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
