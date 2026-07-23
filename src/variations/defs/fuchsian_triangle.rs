//! `fuchsian_triangle` — Bagula's ⟨2,3,12⟩ Kleinian triangle-group
//! limit set (McMullen / Nylander).
//!
//! A faithful port of Roger Bagula's "3group triangle ⟨2,3,12⟩"
//! (`two_Programs_McMullen_Nylander_3group_triangle2312_*.nb`, Programing4
//! July 2026). Three exact 2×2 SL(2,ℂ) generators plus their inverses,
//! chaos-gamed as Möbius maps `z ↦ (Az+B)/(Cz+D)` (the notebook's
//! `RandomChoice[generators]` orbit of z₀ = 0):
//!
//! - `s1 = [[3,-1],[1,0]]` — trace 3, **hyperbolic** (the fixed points
//!   are the two roots of z² − 3z + 1; it scatters the elliptic circles
//!   fractally).
//! - `s2 = [[e^{2πi/3}, 3],[0, e^{-2πi/3}]]` — trace −1, **elliptic**
//!   order 3, with a shear that pushes its 0/∞ fixed points apart.
//! - `s3 = [[cos(2π/12), i·sin(2π/12)],[i·sin(2π/12), cos(2π/12)]]` —
//!   **elliptic**, fixing ±1; the 2π/12 = 30° rotation Bagula names the
//!   group after.
//!
//! The elliptic orbits paint circles; the hyperbolic element arranges
//! them into the swiss-cheese circle packing. This is NOT a von Dyck
//! (p,q,r) rotation presentation — there are no product relations, and a
//! cocompact triangle *rotation* group's limit set would be a round
//! circle. For the reflection *tessellation* of an (p,q,r) triangle see
//! [`honeycomb`](super::honeycomb); this is the Möbius **limit set**.
//!
//! The four sliders expose the notebook's own constants (Tri Trace 3,
//! Order Q 3, Order R 12, Shear 3); every generator is det = 1
//! identically, so the init pass stores them raw (no plug/normalize).
//! The notebook applies no conjugation — to deform the packing
//! (quasiconformal "even → uneven") chain a post or final Möbius /
//! quasiconformal transform.
//!
//! Shares the SL(2,ℂ) / quaternion machinery in
//! `shaders/core/su_mobius.wgsl` with [`su_mobius`](super::su_mobius)
//! (the SU(n) family) — the lib is pulled in for either variation. The
//! `space` = Hyperbolic H3 mode uses the Poincaré extension (a quaternion
//! `x + yi + t·j`, t = height) to fill the packing out to a 3D
//! sphere-packing. Original construction by Roger Bagula; ported here.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

pub static FUCHSIAN_TRIANGLE: VariationDef = VariationDef {
    name: "fuchsian_triangle",
    aliases: &[],
    display_name: "Fuchsian Triangle",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor, Feature::NeedsMobiusLib],
    // Slot 0: previous generator index (avoid_reversal). Slot 1: color
    // register for the Generator color mode.
    state_count: 2,
    wgsl_state_init: None,
    parameters: &[
        param!("avoid_reversal", "Avoid Reversal", bool, true, "Skip a generator's inverse immediately after applying it (no g·g⁻¹ cancellation), so the orbit drifts through the limit set instead of dithering on a small subset (Nylander's backtrack-avoid). Off = all six generators equally likely every call."),
        param!("tri_trace", "Tri Trace", unlimited_float, 3.0, -4.0, 4.0, "Trace of the first generator s1 = [[t,-1],[1,0]]. |t| > 2 is hyperbolic (Bagula's 3 — spreads the elliptic circles fractally), t = 2 parabolic, t = 2·cos(π/n) elliptic of order n."),
        param!("tri_q", "Tri Order Q", int, 3.0, 1.0, 24.0, "Order of the second generator s2 = [[e^{2πi/q}, shear],[0, e^{-2πi/q}]] — an elliptic rotation of order q (Bagula's 3). Its orbits paint the mid-scale circles."),
        param!("tri_r", "Tri Order R", int, 12.0, 1.0, 24.0, "Order of the third generator s3 — the elliptic rotation by 2π/r fixing ±1 (Bagula's 12, the ⟨2,3,12⟩ 30° angle). Its orbits paint the fine circle rings."),
        param!("tri_shear", "Tri Shear", unlimited_float, 3.0, -4.0, 4.0, "The off-diagonal shear in s2 (Bagula's 3). Pushes s2's two fixed points apart — 0 makes s2 a pure diagonal rotation about 0/∞."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Generator"], "Direct-color source (needs the transform's Direct Color > 0). Generator: each of the six group elements has its own palette position, blended through a persistent color register at Color Speed."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the Generator color (wrapped with fract)."),
        param!("color_speed", "Color Speed", float, 0.5, 0.0, 1.0, "How hard each generator pulls the color register toward its own palette position. Low = long blends of orbit history, 1 = hard per-generator assignment."),
        param!("space", "Space", enum, 0, &["Planar", "Hyperbolic H3"], "3D render mode only. Planar: the ordinary complex Möbius map in the xy plane (z passes through) — the flat swiss-cheese packing. Hyperbolic H3: the Poincaré extension — SL(2,C) is exactly the isometry group of hyperbolic 3-space, so each generator acts on upper-half-space (the point as a quaternion x+yi+t·j, t = height) via (Aq+B)(Cq+D)⁻¹, filling the packing out to a 3D sphere-packing. The z-slice at t→0 is the 2D picture."),
        param!("symmetry", "Symmetry", enum, 0, &["None", "2-Fold Point", "2-Fold Mirror", "4-Fold"], "4-Fold applies a random one of {z, −z, conj z, −conj z} to the output each call — 2-Fold Point = {z,−z} (180° rotation), 2-Fold Mirror = {z,conj z} (reflection), 4-Fold = both. Symmetrizes the orbit into an evenly 4-fold packing."),
    ],
    // Six generators × 8 floats (2×2 complex): base at slot k·8 for
    // k = 0..2, inverses at 24 + k·8. Read at bo = n_user + k·8.
    init_param_count: 48,
    wgsl_init: Some(WGSL_INIT),
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_INIT: &str = r#"
fn init_fuchsian_triangle(user: array<f32, 10>) -> array<f32, 48> {
    var out: array<f32, 48>;
    // Bagula's exact ⟨2,3,12⟩ generators, generalized along the
    // notebook's own constants (trace / q / r / shear = 3 / 3 / 12 / 3):
    //   s1 = [[t1, -1], [1, 0]]                          (hyperbolic)
    //   s2 = [[e^{i aq}, sh], [0, e^{-i aq}]]            (elliptic order q)
    //   s3 = [[cos(ar), i sin(ar)], [i sin(ar), cos(ar)]] (elliptic, fixes ±1)
    // All three have det = 1 identically, so we store them raw + inverse.
    let pi = 3.14159265359;
    let t1 = user[1];
    let aq = 2.0 * pi / max(user[2], 1.0);
    let ar = 2.0 * pi / max(user[3], 1.0);
    let sh = user[4];
    var ga: array<vec2<f32>, 3>;
    var gb: array<vec2<f32>, 3>;
    var gc: array<vec2<f32>, 3>;
    var gd: array<vec2<f32>, 3>;
    ga[0] = vec2<f32>(t1, 0.0); gb[0] = vec2<f32>(-1.0, 0.0);
    gc[0] = vec2<f32>(1.0, 0.0); gd[0] = vec2<f32>(0.0, 0.0);
    ga[1] = vec2<f32>(cos(aq), sin(aq)); gb[1] = vec2<f32>(sh, 0.0);
    gc[1] = vec2<f32>(0.0, 0.0); gd[1] = vec2<f32>(cos(aq), -sin(aq));
    ga[2] = vec2<f32>(cos(ar), 0.0); gb[2] = vec2<f32>(0.0, sin(ar));
    gc[2] = vec2<f32>(0.0, sin(ar)); gd[2] = vec2<f32>(cos(ar), 0.0);
    for (var i = 0u; i < 3u; i = i + 1u) {
        let na = ga[i]; let nb = gb[i]; let nc = gc[i]; let nd = gd[i];
        let o = i * 8u; let oi = 24u + i * 8u;
        out[o] = na.x; out[o + 1u] = na.y; out[o + 2u] = nb.x; out[o + 3u] = nb.y;
        out[o + 4u] = nc.x; out[o + 5u] = nc.y; out[o + 6u] = nd.x; out[o + 7u] = nd.y;
        // inverse of a det-1 matrix: [[d,-b],[-c,a]]
        out[oi] = nd.x; out[oi + 1u] = nd.y; out[oi + 2u] = -nb.x; out[oi + 3u] = -nb.y;
        out[oi + 4u] = -nc.x; out[oi + 5u] = -nc.y; out[oi + 6u] = na.x; out[oi + 7u] = na.y;
    }
    return out;
}
"#;

// Local generator count: 3 base + 3 inverse. Derived matrices start at
// slot 10 (the user-param count), matrix k at 10 + k·8.
const WGSL_2D: &str = r#"
fn fuchsian_triangle_read(xform_id: u32, variation_id: u32, k: u32) -> SuMat {
    let bo = 10u + k * 8u;
    return SuMat(
        vec2<f32>(get_param(xform_id, variation_id, bo), get_param(xform_id, variation_id, bo + 1u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 2u), get_param(xform_id, variation_id, bo + 3u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 4u), get_param(xform_id, variation_id, bo + 5u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 6u), get_param(xform_id, variation_id, bo + 7u)));
}

fn variation_fuchsian_triangle(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let avoid = get_param(xform_id, variation_id, 0u) > 0.5;
    let dc_mode = u32(get_param(xform_id, variation_id, 5u));
    let dc_scale = get_param(xform_id, variation_id, 6u);
    let color_speed = get_param(xform_id, variation_id, 7u);

    let cnt = 6u;
    let half = 3u;
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

    var out = su_apply_plain(fuchsian_triangle_read(xform_id, variation_id, k), p);
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
fn fuchsian_triangle_read(xform_id: u32, variation_id: u32, k: u32) -> SuMat {
    let bo = 10u + k * 8u;
    return SuMat(
        vec2<f32>(get_param(xform_id, variation_id, bo), get_param(xform_id, variation_id, bo + 1u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 2u), get_param(xform_id, variation_id, bo + 3u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 4u), get_param(xform_id, variation_id, bo + 5u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 6u), get_param(xform_id, variation_id, bo + 7u)));
}

fn variation_fuchsian_triangle(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let avoid = get_param(xform_id, variation_id, 0u) > 0.5;
    let dc_mode = u32(get_param(xform_id, variation_id, 5u));
    let dc_scale = get_param(xform_id, variation_id, 6u);
    let color_speed = get_param(xform_id, variation_id, 7u);

    let cnt = 6u;
    let half = 3u;
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

    let space = u32(get_param(xform_id, variation_id, 8u));
    let ba = fuchsian_triangle_read(xform_id, variation_id, k);
    var out: vec3<f32>;
    if (space == 1u) { out = su_apply_plain3(ba, p); }
    else { out = vec3<f32>(su_apply_plain(ba, p.xy), p.z); }
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
