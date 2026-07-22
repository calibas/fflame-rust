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
    // Slot 0: previous generator index (avoid_reversal). Slot 1: color
    // register for the Generator color mode.
    state_count: 2,
    wgsl_state_init: None,
    parameters: &[
        param!("group", "Group", enum, 1, &["SU(2) 6-Group", "SU(3) Reduced", "SU(5) Reduced", "SU(3) Custom"], "Which SU(n) generator set. SU(2) 6-Group (12 generators) — set Angle 0 / Hyper Angle 45 for Bagula's hypertriquasiconformal Apollonian disk. SU(3) Reduced (16 generators) — Angle 45 / Hyper 0 (the defaults) for the three-quark-disk limit set. SU(5) Reduced (48 gens, our reduction) — pair with Symmetry 4-Fold. SU(3) Custom: computes the Gell-Mann reduction LIVE from the Reduce A/B/C + Plug sliders (init pass) — dial the reduction tensor and the pole-plug to explore the infinite SU(3) family and the circle packing."),
        param!("avoid_reversal", "Avoid Reversal", bool, true, "Skip a generator's inverse immediately after applying it (no g·g⁻¹ cancellation), so the orbit drifts through the limit set instead of dithering on a small subset. Off = all generators equally likely every call."),
        param!("conj_angle", "Angle", angle, 45.0, "Elliptic rotation θ in the conjugator qf = rotate(θ + iη). SU(3)'s triquasiconformal '45'. Sweeping it deforms the whole limit set."),
        param!("conj_hyper", "Hyper Angle", angle, 0.0, "HYPERBOLIC rotation η (imaginary angle) in qf = rotate(θ + iη) — the 'hyper' in SU(2)'s hypertriquasiconformal. 45° with Angle 0 is Bagula's SU(2) 6-group; nonzero η bends the group loxodromically, compacting lattices toward Apollonian disks."),
        param!("qc_strength", "QC Strength", float, 1.0, 0.0, 2.0, "Quasiconformal deformation δ in dk = [[1+iδ,1],[1,1−iδ]]. 1 = Bagula's groups; toward 0 the generators lose their quasiconformal stretch and the limit set collapses; > 1 exaggerates it."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Generator"], "Direct-color source (needs the transform's Direct Color > 0). Generator: each group element has its own palette position, blended through a persistent color register at Color Speed."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the Generator color (wrapped with fract)."),
        param!("color_speed", "Color Speed", float, 0.5, 0.0, 1.0, "How hard each generator pulls the color register toward its own palette position. Low = long blends of orbit history, 1 = hard per-generator assignment."),
        param!("space", "Space", enum, 0, &["Planar", "Hyperbolic H3"], "3D render mode only. Planar: the ordinary complex Möbius map in the xy plane (z passes through) — the flat limit set. Hyperbolic H3: the Poincaré extension — SL(2,C) is exactly the isometry group of hyperbolic 3-space, so each generator acts on upper-half-space (the point as a quaternion x+yi+t·j, t = height) via (Aq+B)(Cq+D)⁻¹. The limit set fills 3D: SU(3)'s three disks become three spheres, SU(2)'s Apollonian disk an Apollonian sphere-packing. The z-slice at t→0 is the 2D picture."),
        param!("symmetry", "Symmetry", enum, 0, &["None", "4-Fold"], "4-Fold applies a random one of {z, −z, conj z, −conj z} to the output each call — Bagula's orbit-symmetrization for the SU(5) 'symmetry enhanced' elliptical picture (a 2×2 mirror). Also usable to symmetrize the other groups."),
        param!("red_a_re", "Reduce A re", unlimited_float, 1.0, -4.0, 4.0, "SU(3) Custom mode: real part of the reduction-tensor entry a (tt = [[1,0,1],[a,b,c]]). The reduction is a free design choice — each (a,b,c) gives a different fractal in the SU(3) family. Computed live via the init pass."),
        param!("red_a_im", "Reduce A im", unlimited_float, 0.0, -4.0, 4.0, "Imag part of reduction entry a."),
        param!("red_b_re", "Reduce B re", unlimited_float, 0.0, -4.0, 4.0, "Real part of reduction entry b."),
        param!("red_b_im", "Reduce B im", unlimited_float, 1.0, -4.0, 4.0, "Imag part of reduction entry b."),
        param!("red_c_re", "Reduce C re", unlimited_float, 1.0, -4.0, 4.0, "Real part of reduction entry c."),
        param!("red_c_im", "Reduce C im", unlimited_float, 0.0, -4.0, 4.0, "Imag part of reduction entry c."),
        param!("red_plug", "Plug", float, 2.0, 0.0, 4.0, "SU(3) Custom mode: trace added to the traceless reduced matrices to plug their Möbius poles. 2 makes them parabolic → Apollonian circle packing; 0 leaves them as pole-y involutions; the circles live near 2."),
    ],
    init_param_count: 128,
    wgsl_init: Some(WGSL_INIT),
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_INIT: &str = r#"
fn init_su_mobius(user: array<f32, 17>) -> array<f32, 128> {
    let a = vec2<f32>(user[10], user[11]);
    let b = vec2<f32>(user[12], user[13]);
    let c = vec2<f32>(user[14], user[15]);
    let plug = user[16];
    var out: array<f32, 128>;
    {
        var wa = vec2<f32>(0.0,0.0);
        let wb = cmul(vec2<f32>(1.000000, 0.000000), b);
        let wc = cmul(vec2<f32>(1.000000, 0.000000), b);
        let wd = cmul(vec2<f32>(2.000000, 0.000000), cmul(a,b));
        // trace-plug: give traceless reductions a trace (parabolic poles -> circles)
        let tr = wa + wd;
        if (tr.x*tr.x + tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug, 0.0); }
        // normalize to SL(2,C): divide by sqrt(det)
        let det = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(det);
        let na = cdiv(wa, sd); let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd); let nd = cdiv(wd, sd);
        out[0u]=na.x; out[1u]=na.y; out[2u]=nb.x; out[3u]=nb.y;
        out[4u]=nc.x; out[5u]=nc.y; out[6u]=nd.x; out[7u]=nd.y;
        // inverse (det=1): [[d,-b],[-c,a]]
        out[64u]=nd.x; out[65u]=nd.y; out[66u]=-nb.x; out[67u]=-nb.y;
        out[68u]=-nc.x; out[69u]=-nc.y; out[70u]=na.x; out[71u]=na.y;
    }
    {
        var wa = vec2<f32>(0.0,0.0);
        let wb = cmul(vec2<f32>(0.000000, -1.000000), b);
        let wc = cmul(vec2<f32>(0.000000, 1.000000), b);
        let wd = vec2<f32>(0.0,0.0);
        // trace-plug: give traceless reductions a trace (parabolic poles -> circles)
        let tr = wa + wd;
        if (tr.x*tr.x + tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug, 0.0); }
        // normalize to SL(2,C): divide by sqrt(det)
        let det = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(det);
        let na = cdiv(wa, sd); let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd); let nd = cdiv(wd, sd);
        out[8u]=na.x; out[9u]=na.y; out[10u]=nb.x; out[11u]=nb.y;
        out[12u]=nc.x; out[13u]=nc.y; out[14u]=nd.x; out[15u]=nd.y;
        // inverse (det=1): [[d,-b],[-c,a]]
        out[72u]=nd.x; out[73u]=nd.y; out[74u]=-nb.x; out[75u]=-nb.y;
        out[76u]=-nc.x; out[77u]=-nc.y; out[78u]=na.x; out[79u]=na.y;
    }
    {
        var wa = vec2<f32>(1.000000, 0.000000);
        let wb = cmul(vec2<f32>(1.000000, 0.000000), a);
        let wc = cmul(vec2<f32>(1.000000, 0.000000), a);
        let wd = cmul(vec2<f32>(-1.000000, 0.000000), cmul(b,b)) + cmul(vec2<f32>(1.000000, 0.000000), cmul(a,a));
        // trace-plug: give traceless reductions a trace (parabolic poles -> circles)
        let tr = wa + wd;
        if (tr.x*tr.x + tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug, 0.0); }
        // normalize to SL(2,C): divide by sqrt(det)
        let det = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(det);
        let na = cdiv(wa, sd); let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd); let nd = cdiv(wd, sd);
        out[16u]=na.x; out[17u]=na.y; out[18u]=nb.x; out[19u]=nb.y;
        out[20u]=nc.x; out[21u]=nc.y; out[22u]=nd.x; out[23u]=nd.y;
        // inverse (det=1): [[d,-b],[-c,a]]
        out[80u]=nd.x; out[81u]=nd.y; out[82u]=-nb.x; out[83u]=-nb.y;
        out[84u]=-nc.x; out[85u]=-nc.y; out[86u]=na.x; out[87u]=na.y;
    }
    {
        var wa = vec2<f32>(2.000000, 0.000000);
        let wb = cmul(vec2<f32>(1.000000, 0.000000), c) + cmul(vec2<f32>(1.000000, 0.000000), a);
        let wc = cmul(vec2<f32>(1.000000, 0.000000), c) + cmul(vec2<f32>(1.000000, 0.000000), a);
        let wd = cmul(vec2<f32>(2.000000, 0.000000), cmul(a,c));
        // trace-plug: give traceless reductions a trace (parabolic poles -> circles)
        let tr = wa + wd;
        if (tr.x*tr.x + tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug, 0.0); }
        // normalize to SL(2,C): divide by sqrt(det)
        let det = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(det);
        let na = cdiv(wa, sd); let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd); let nd = cdiv(wd, sd);
        out[24u]=na.x; out[25u]=na.y; out[26u]=nb.x; out[27u]=nb.y;
        out[28u]=nc.x; out[29u]=nc.y; out[30u]=nd.x; out[31u]=nd.y;
        // inverse (det=1): [[d,-b],[-c,a]]
        out[88u]=nd.x; out[89u]=nd.y; out[90u]=-nb.x; out[91u]=-nb.y;
        out[92u]=-nc.x; out[93u]=-nc.y; out[94u]=na.x; out[95u]=na.y;
    }
    {
        var wa = vec2<f32>(0.0,0.0);
        let wb = cmul(vec2<f32>(0.000000, -1.000000), c) + cmul(vec2<f32>(0.000000, 1.000000), a);
        let wc = cmul(vec2<f32>(0.000000, 1.000000), c) + cmul(vec2<f32>(0.000000, -1.000000), a);
        let wd = vec2<f32>(0.0,0.0);
        // trace-plug: give traceless reductions a trace (parabolic poles -> circles)
        let tr = wa + wd;
        if (tr.x*tr.x + tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug, 0.0); }
        // normalize to SL(2,C): divide by sqrt(det)
        let det = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(det);
        let na = cdiv(wa, sd); let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd); let nd = cdiv(wd, sd);
        out[32u]=na.x; out[33u]=na.y; out[34u]=nb.x; out[35u]=nb.y;
        out[36u]=nc.x; out[37u]=nc.y; out[38u]=nd.x; out[39u]=nd.y;
        // inverse (det=1): [[d,-b],[-c,a]]
        out[96u]=nd.x; out[97u]=nd.y; out[98u]=-nb.x; out[99u]=-nb.y;
        out[100u]=-nc.x; out[101u]=-nc.y; out[102u]=na.x; out[103u]=na.y;
    }
    {
        var wa = vec2<f32>(0.0,0.0);
        let wb = cmul(vec2<f32>(1.000000, 0.000000), b);
        let wc = cmul(vec2<f32>(1.000000, 0.000000), b);
        let wd = cmul(vec2<f32>(2.000000, 0.000000), cmul(b,c));
        // trace-plug: give traceless reductions a trace (parabolic poles -> circles)
        let tr = wa + wd;
        if (tr.x*tr.x + tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug, 0.0); }
        // normalize to SL(2,C): divide by sqrt(det)
        let det = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(det);
        let na = cdiv(wa, sd); let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd); let nd = cdiv(wd, sd);
        out[40u]=na.x; out[41u]=na.y; out[42u]=nb.x; out[43u]=nb.y;
        out[44u]=nc.x; out[45u]=nc.y; out[46u]=nd.x; out[47u]=nd.y;
        // inverse (det=1): [[d,-b],[-c,a]]
        out[104u]=nd.x; out[105u]=nd.y; out[106u]=-nb.x; out[107u]=-nb.y;
        out[108u]=-nc.x; out[109u]=-nc.y; out[110u]=na.x; out[111u]=na.y;
    }
    {
        var wa = vec2<f32>(0.0,0.0);
        let wb = cmul(vec2<f32>(0.000000, 1.000000), b);
        let wc = cmul(vec2<f32>(0.000000, -1.000000), b);
        let wd = vec2<f32>(0.0,0.0);
        // trace-plug: give traceless reductions a trace (parabolic poles -> circles)
        let tr = wa + wd;
        if (tr.x*tr.x + tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug, 0.0); }
        // normalize to SL(2,C): divide by sqrt(det)
        let det = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(det);
        let na = cdiv(wa, sd); let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd); let nd = cdiv(wd, sd);
        out[48u]=na.x; out[49u]=na.y; out[50u]=nb.x; out[51u]=nb.y;
        out[52u]=nc.x; out[53u]=nc.y; out[54u]=nd.x; out[55u]=nd.y;
        // inverse (det=1): [[d,-b],[-c,a]]
        out[112u]=nd.x; out[113u]=nd.y; out[114u]=-nb.x; out[115u]=-nb.y;
        out[116u]=-nc.x; out[117u]=-nc.y; out[118u]=na.x; out[119u]=na.y;
    }
    {
        var wa = vec2<f32>(-0.577350, 0.000000);
        let wb = cmul(vec2<f32>(-1.154701, 0.000000), c) + cmul(vec2<f32>(0.577350, 0.000000), a);
        let wc = cmul(vec2<f32>(-1.154701, 0.000000), c) + cmul(vec2<f32>(0.577350, 0.000000), a);
        let wd = cmul(vec2<f32>(-1.154701, 0.000000), cmul(c,c)) + cmul(vec2<f32>(0.577350, 0.000000), cmul(b,b)) + cmul(vec2<f32>(0.577350, 0.000000), cmul(a,a));
        // trace-plug: give traceless reductions a trace (parabolic poles -> circles)
        let tr = wa + wd;
        if (tr.x*tr.x + tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug, 0.0); }
        // normalize to SL(2,C): divide by sqrt(det)
        let det = csub(cmul(wa, wd), cmul(wb, wc));
        let sd = csqrt(det);
        let na = cdiv(wa, sd); let nb = cdiv(wb, sd);
        let nc = cdiv(wc, sd); let nd = cdiv(wd, sd);
        out[56u]=na.x; out[57u]=na.y; out[58u]=nb.x; out[59u]=nb.y;
        out[60u]=nc.x; out[61u]=nc.y; out[62u]=nd.x; out[63u]=nd.y;
        // inverse (det=1): [[d,-b],[-c,a]]
        out[120u]=nd.x; out[121u]=nd.y; out[122u]=-nb.x; out[123u]=-nb.y;
        out[124u]=-nc.x; out[125u]=-nc.y; out[126u]=na.x; out[127u]=na.y;
    }
    return out;
}
"#;

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
    var out: vec2<f32>;
    if (group == 3u) {
        let bo = 17u + k * 8u;   // init-derived base matrix, 17 = user param count
        let ba = SuMat(
            vec2<f32>(get_param(xform_id, variation_id, bo), get_param(xform_id, variation_id, bo + 1u)),
            vec2<f32>(get_param(xform_id, variation_id, bo + 2u), get_param(xform_id, variation_id, bo + 3u)),
            vec2<f32>(get_param(xform_id, variation_id, bo + 4u), get_param(xform_id, variation_id, bo + 5u)),
            vec2<f32>(get_param(xform_id, variation_id, bo + 6u), get_param(xform_id, variation_id, bo + 7u)));
        out = su_apply_m(ba, p, cj, cji);
    } else {
        out = su_mobius_apply(gidx, p, cj, cji);
    }
    let symm = u32(get_param(xform_id, variation_id, 9u));
    if (symm == 1u) {
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
    if (group == 3u) {
        let bo = 17u + k * 8u;
        let ba = SuMat(
            vec2<f32>(get_param(xform_id, variation_id, bo), get_param(xform_id, variation_id, bo + 1u)),
            vec2<f32>(get_param(xform_id, variation_id, bo + 2u), get_param(xform_id, variation_id, bo + 3u)),
            vec2<f32>(get_param(xform_id, variation_id, bo + 4u), get_param(xform_id, variation_id, bo + 5u)),
            vec2<f32>(get_param(xform_id, variation_id, bo + 6u), get_param(xform_id, variation_id, bo + 7u)));
        if (space == 1u) { out = su_apply_m3(ba, p, cj, cji); }
        else { out = vec3<f32>(su_apply_m(ba, p.xy, cj, cji), p.z); }
    } else if (space == 1u) {
        out = su_mobius_apply3(gidx, p, cj, cji);
    } else {
        out = vec3<f32>(su_mobius_apply(gidx, p.xy, cj, cji), p.z);
    }
    let symm = u32(get_param(xform_id, variation_id, 9u));
    if (symm == 1u) {
        let r = min(u32(rng_nextf(rng) * 4.0), 3u);
        if (r == 1u) { out = vec3<f32>(-out.x, -out.y, out.z); }
        else if (r == 2u) { out = vec3<f32>(out.x, -out.y, out.z); }
        else if (r == 3u) { out = vec3<f32>(-out.x, out.y, out.z); }
    }
    return out;
}
"#;
