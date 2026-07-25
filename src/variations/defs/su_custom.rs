//! `su_custom` — SU(n) Möbius groups with LIVE reduction tensors
//! (Roger Bagula).
//!
//! The runtime-reduction half of the former unified `su_mobius`: where
//! [`su_mobius`](super::su_mobius) chaos-games over BAKED generator
//! tables, this computes its 2×2 SL(2,ℂ) generators in the init pass
//! from the Reduce A/B/C/D + Plug sliders — the reduction tensor
//! `w[i] = t·s[i]·tᵀ` applied to the Lie-algebra generator set
//! (Gell-Mann and friends), with traceless results trace-plugged into
//! parabolics (the circle-packing knob). Each tensor is a free design
//! choice: the sliders walk an infinite family of limit sets per SU(n).
//!
//! `group` (FIXED indices — deliberately NOT the old unified su_mobius
//! numbering):
//! - **0 SU(2) Custom** — 6 generators (3 base + inverses)
//! - **1 SU(3) Custom** — 16 generators (8 Gell-Mann + inverses)
//! - **2 SU(4) Custom** — 30 generators (15 + inverses)
//!
//! Every element is conjugated by the triquasiconformal deformation
//! `C = dk(δ)·s0·qf(θ + iη)` (Angle / Hyper Angle / QC Strength), and
//! the chaos game, avoid-reversal, generator coloring, symmetry, and
//! Poincaré H3 space toggle all match `su_mobius`.
//!
//! Uses `Feature::NeedsMobiusLib` (`shaders/core/su_mobius.wgsl`) for
//! the SuMat machinery; it never reads the baked tables.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// SU(n) Möbius groups with live reduction tensors dialed from the
/// Reduce sliders.
///
/// # Authors
/// - Roger Bagula
/// - Fractals for All
/// - Claude Fable 5
pub static SU_CUSTOM: VariationDef = VariationDef {
    name: "su_custom",
    aliases: &[],
    display_name: "SU(n) Custom",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor, Feature::NeedsMobiusLib, Feature::AlwaysZ],
    // Slot 0: previous generator index (avoid_reversal). Slot 1: color
    // register for the Generator color mode.
    state_count: 2,
    wgsl_state_init: None,
    parameters: &[
        param!("group", "Group", enum, 0, &["SU(2) Custom", "SU(3) Custom", "SU(4) Custom"], "Which SU(n) algebra to reduce LIVE from the Reduce A/B/C/D + Plug sliders (init pass). SU(2) Custom: 6 generators. SU(3) Custom: the 8 Gell-Mann matrices reduced 3×3→2×2 (16 generators) — Bagula's family. SU(4) Custom: 30 generators (the D slider becomes live). Dial the reduction tensor and the pole-plug to explore the infinite family and the circle packing."),
        param!("avoid_reversal", "Avoid Reversal", bool, true, "Skip a generator's inverse immediately after applying it (no g·g⁻¹ cancellation), so the orbit drifts through the limit set instead of dithering on a small subset. Off = all generators equally likely every call."),
        param!("conj_angle", "Angle", angle, 45.0, "Elliptic rotation θ in the conjugator qf = rotate(θ + iη). SU(3)'s triquasiconformal '45'. Sweeping it deforms the whole limit set."),
        param!("conj_hyper", "Hyper Angle", angle, 0.0, "HYPERBOLIC rotation η (imaginary angle) in qf = rotate(θ + iη) — the 'hyper' in SU(2)'s hypertriquasiconformal. 45° with Angle 0 is Bagula's SU(2) 6-group; nonzero η bends the group loxodromically, compacting lattices toward Apollonian disks."),
        param!("qc_strength", "QC Strength", float, 1.0, 0.0, 2.0, "Quasiconformal deformation δ in dk = [[1+iδ,1],[1,1−iδ]]. 1 = Bagula's groups; toward 0 the generators lose their quasiconformal stretch and the limit set collapses; > 1 exaggerates it."),
        param!("dc_mode", "Color Mode", enum, 0, &["Off", "Generator"], "Direct-color source (needs the transform's Direct Color > 0). Generator: each group element has its own palette position, blended through a persistent color register at Color Speed."),
        param!("dc_scale", "Color Scale", float, 1.0, 0.1, 8.0, "Palette-index multiplier for the Generator color (wrapped with fract)."),
        param!("color_speed", "Color Speed", float, 0.5, 0.0, 1.0, "How hard each generator pulls the color register toward its own palette position. Low = long blends of orbit history, 1 = hard per-generator assignment."),
        param!("space", "Space", enum, 0, &["Euclidean", "Hyperbolic H3"], "3D render mode only. Euclidean: the ordinary complex Möbius map in the xy plane (z passes through) — the flat limit set. Hyperbolic H3: the Poincaré extension — SL(2,C) is exactly the isometry group of hyperbolic 3-space, so each generator acts on upper-half-space (the point as a quaternion x+yi+t·j, t = height) via (Aq+B)(Cq+D)⁻¹. The limit set fills 3D: SU(3)'s three disks become three spheres, SU(2)'s Apollonian disk an Apollonian sphere-packing. The z-slice at t→0 is the 2D picture."),
        param!("symmetry", "Symmetry", enum, 0, &["None", "2-Fold Point", "2-Fold Mirror", "4-Fold"], "4-Fold applies a random one of {z, −z, conj z, −conj z} to the output each call — 2-Fold Point = {z,−z} (180° rotation), 2-Fold Mirror = {z,conj z} (reflection), 4-Fold = both (Bagula's SU(5) orbit-symmetrization for the 'symmetry enhanced' elliptical picture). Usable on any group."),
        param!("red_a_re", "Reduce A re", unlimited_float, 2.0, -4.0, 4.0, "Real part of the reduction-tensor entry a (SU(3): tt = [[1,0,1],[a,b,c]]). The reduction is a free design choice — each (a,b,c) gives a different fractal in the SU(3) family. Computed live via the init pass."),
        param!("red_a_im", "Reduce A im", unlimited_float, 0.0, -4.0, 4.0, "Imag part of reduction entry a."),
        param!("red_b_re", "Reduce B re", unlimited_float, 0.0, -4.0, 4.0, "Real part of reduction entry b."),
        param!("red_b_im", "Reduce B im", unlimited_float, 0.0, -4.0, 4.0, "Imag part of reduction entry b."),
        param!("red_c_re", "Reduce C re", unlimited_float, 1.0, -4.0, 4.0, "Real part of reduction entry c."),
        param!("red_c_im", "Reduce C im", unlimited_float, 0.0, -4.0, 4.0, "Imag part of reduction entry c."),
        param!("red_d_re", "Reduce D re", unlimited_float, -1.0, -4.0, 4.0, "SU(4) Custom: real part of reduction entry d (the 4th tensor column). Unused by SU(2)/SU(3) Custom."),
        param!("red_d_im", "Reduce D im", unlimited_float, 0.0, -4.0, 4.0, "Imag part of reduction entry d."),
        param!("red_plug", "Plug", float, 2.0, 0.0, 4.0, "Trace added to the traceless reduced matrices to plug their Möbius poles. 2 makes them parabolic → Apollonian circle packing; 0 leaves them as pole-y involutions; the circles live near 2."),
    ],
    init_param_count: 240,
    wgsl_init: Some(WGSL_INIT),
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_INIT: &str = r#"
fn init_su_custom(user: array<f32, 19>) -> array<f32, 240> {
    let group = u32(user[0]);
    let a = vec2<f32>(user[10], user[11]); let b = vec2<f32>(user[12], user[13]);
    let c = vec2<f32>(user[14], user[15]); let d = vec2<f32>(user[16], user[17]);
    let plug = user[18];
    var out: array<f32, 240>;
    if (group == 1u) {
        {
            var wa = vec2<f32>(0.0,0.0); let wb = cmul(vec2<f32>(1.000000, 0.000000), b);
            let wc = cmul(vec2<f32>(1.000000, 0.000000), b); let wd = cmul(vec2<f32>(2.000000, 0.000000), cmul(a,b));
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[0u]=na.x; out[1u]=na.y; out[2u]=nb.x; out[3u]=nb.y; out[4u]=nc.x; out[5u]=nc.y; out[6u]=nd.x; out[7u]=nd.y;
            out[64u]=nd.x; out[65u]=nd.y; out[66u]=-nb.x; out[67u]=-nb.y; out[68u]=-nc.x; out[69u]=-nc.y; out[70u]=na.x; out[71u]=na.y;
        }
        {
            var wa = vec2<f32>(2.000000, 0.000000); let wb = cmul(vec2<f32>(1.000000, 0.000000), c) + cmul(vec2<f32>(1.000000, 0.000000), a);
            let wc = cmul(vec2<f32>(1.000000, 0.000000), c) + cmul(vec2<f32>(1.000000, 0.000000), a); let wd = cmul(vec2<f32>(2.000000, 0.000000), cmul(a,c));
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[8u]=na.x; out[9u]=na.y; out[10u]=nb.x; out[11u]=nb.y; out[12u]=nc.x; out[13u]=nc.y; out[14u]=nd.x; out[15u]=nd.y;
            out[72u]=nd.x; out[73u]=nd.y; out[74u]=-nb.x; out[75u]=-nb.y; out[76u]=-nc.x; out[77u]=-nc.y; out[78u]=na.x; out[79u]=na.y;
        }
        {
            var wa = vec2<f32>(0.0,0.0); let wb = cmul(vec2<f32>(1.000000, 0.000000), b);
            let wc = cmul(vec2<f32>(1.000000, 0.000000), b); let wd = cmul(vec2<f32>(2.000000, 0.000000), cmul(b,c));
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[16u]=na.x; out[17u]=na.y; out[18u]=nb.x; out[19u]=nb.y; out[20u]=nc.x; out[21u]=nc.y; out[22u]=nd.x; out[23u]=nd.y;
            out[80u]=nd.x; out[81u]=nd.y; out[82u]=-nb.x; out[83u]=-nb.y; out[84u]=-nc.x; out[85u]=-nc.y; out[86u]=na.x; out[87u]=na.y;
        }
        {
            var wa = vec2<f32>(0.0,0.0); let wb = cmul(vec2<f32>(0.000000, -1.000000), b);
            let wc = cmul(vec2<f32>(0.000000, 1.000000), b); let wd = vec2<f32>(0.0,0.0);
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[24u]=na.x; out[25u]=na.y; out[26u]=nb.x; out[27u]=nb.y; out[28u]=nc.x; out[29u]=nc.y; out[30u]=nd.x; out[31u]=nd.y;
            out[88u]=nd.x; out[89u]=nd.y; out[90u]=-nb.x; out[91u]=-nb.y; out[92u]=-nc.x; out[93u]=-nc.y; out[94u]=na.x; out[95u]=na.y;
        }
        {
            var wa = vec2<f32>(0.0,0.0); let wb = cmul(vec2<f32>(0.000000, -1.000000), c) + cmul(vec2<f32>(0.000000, 1.000000), a);
            let wc = cmul(vec2<f32>(0.000000, 1.000000), c) + cmul(vec2<f32>(0.000000, -1.000000), a); let wd = vec2<f32>(0.0,0.0);
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[32u]=na.x; out[33u]=na.y; out[34u]=nb.x; out[35u]=nb.y; out[36u]=nc.x; out[37u]=nc.y; out[38u]=nd.x; out[39u]=nd.y;
            out[96u]=nd.x; out[97u]=nd.y; out[98u]=-nb.x; out[99u]=-nb.y; out[100u]=-nc.x; out[101u]=-nc.y; out[102u]=na.x; out[103u]=na.y;
        }
        {
            var wa = vec2<f32>(0.0,0.0); let wb = cmul(vec2<f32>(0.000000, 1.000000), b);
            let wc = cmul(vec2<f32>(0.000000, -1.000000), b); let wd = vec2<f32>(0.0,0.0);
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[40u]=na.x; out[41u]=na.y; out[42u]=nb.x; out[43u]=nb.y; out[44u]=nc.x; out[45u]=nc.y; out[46u]=nd.x; out[47u]=nd.y;
            out[104u]=nd.x; out[105u]=nd.y; out[106u]=-nb.x; out[107u]=-nb.y; out[108u]=-nc.x; out[109u]=-nc.y; out[110u]=na.x; out[111u]=na.y;
        }
        {
            var wa = vec2<f32>(1.000000, 0.000000); let wb = cmul(vec2<f32>(1.000000, 0.000000), a);
            let wc = cmul(vec2<f32>(1.000000, 0.000000), a); let wd = cmul(vec2<f32>(-1.000000, 0.000000), cmul(b,b)) + cmul(vec2<f32>(1.000000, 0.000000), cmul(a,a));
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[48u]=na.x; out[49u]=na.y; out[50u]=nb.x; out[51u]=nb.y; out[52u]=nc.x; out[53u]=nc.y; out[54u]=nd.x; out[55u]=nd.y;
            out[112u]=nd.x; out[113u]=nd.y; out[114u]=-nb.x; out[115u]=-nb.y; out[116u]=-nc.x; out[117u]=-nc.y; out[118u]=na.x; out[119u]=na.y;
        }
        {
            var wa = vec2<f32>(-0.577350, 0.000000); let wb = cmul(vec2<f32>(-1.154701, 0.000000), c) + cmul(vec2<f32>(0.577350, 0.000000), a);
            let wc = cmul(vec2<f32>(-1.154701, 0.000000), c) + cmul(vec2<f32>(0.577350, 0.000000), a); let wd = cmul(vec2<f32>(-1.154701, 0.000000), cmul(c,c)) + cmul(vec2<f32>(0.577350, 0.000000), cmul(b,b)) + cmul(vec2<f32>(0.577350, 0.000000), cmul(a,a));
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[56u]=na.x; out[57u]=na.y; out[58u]=nb.x; out[59u]=nb.y; out[60u]=nc.x; out[61u]=nc.y; out[62u]=nd.x; out[63u]=nd.y;
            out[120u]=nd.x; out[121u]=nd.y; out[122u]=-nb.x; out[123u]=-nb.y; out[124u]=-nc.x; out[125u]=-nc.y; out[126u]=na.x; out[127u]=na.y;
        }
    }
    if (group == 0u) {
        {
            var wa = vec2<f32>(-2.000000, 0.000000); let wb = cmul(vec2<f32>(1.000000, 0.000000), b) + cmul(vec2<f32>(-1.000000, 0.000000), a);
            let wc = cmul(vec2<f32>(1.000000, 0.000000), b) + cmul(vec2<f32>(-1.000000, 0.000000), a); let wd = cmul(vec2<f32>(2.000000, 0.000000), cmul(a,b));
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[0u]=na.x; out[1u]=na.y; out[2u]=nb.x; out[3u]=nb.y; out[4u]=nc.x; out[5u]=nc.y; out[6u]=nd.x; out[7u]=nd.y;
            out[24u]=nd.x; out[25u]=nd.y; out[26u]=-nb.x; out[27u]=-nb.y; out[28u]=-nc.x; out[29u]=-nc.y; out[30u]=na.x; out[31u]=na.y;
        }
        {
            var wa = vec2<f32>(0.0,0.0); let wb = cmul(vec2<f32>(0.000000, -1.000000), b) + cmul(vec2<f32>(0.000000, -1.000000), a);
            let wc = cmul(vec2<f32>(0.000000, 1.000000), b) + cmul(vec2<f32>(0.000000, 1.000000), a); let wd = vec2<f32>(0.0,0.0);
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[8u]=na.x; out[9u]=na.y; out[10u]=nb.x; out[11u]=nb.y; out[12u]=nc.x; out[13u]=nc.y; out[14u]=nd.x; out[15u]=nd.y;
            out[32u]=nd.x; out[33u]=nd.y; out[34u]=-nb.x; out[35u]=-nb.y; out[36u]=-nc.x; out[37u]=-nc.y; out[38u]=na.x; out[39u]=na.y;
        }
        {
            var wa = vec2<f32>(0.0,0.0); let wb = cmul(vec2<f32>(1.000000, 0.000000), b) + cmul(vec2<f32>(1.000000, 0.000000), a);
            let wc = cmul(vec2<f32>(1.000000, 0.000000), b) + cmul(vec2<f32>(1.000000, 0.000000), a); let wd = cmul(vec2<f32>(-1.000000, 0.000000), cmul(b,b)) + cmul(vec2<f32>(1.000000, 0.000000), cmul(a,a));
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[16u]=na.x; out[17u]=na.y; out[18u]=nb.x; out[19u]=nb.y; out[20u]=nc.x; out[21u]=nc.y; out[22u]=nd.x; out[23u]=nd.y;
            out[40u]=nd.x; out[41u]=nd.y; out[42u]=-nb.x; out[43u]=-nb.y; out[44u]=-nc.x; out[45u]=-nc.y; out[46u]=na.x; out[47u]=na.y;
        }
    }
    if (group == 2u) {
        {
            var wa = vec2<f32>(2.000000, 0.000000); let wb = cmul(vec2<f32>(1.000000, 0.000000), b) + cmul(vec2<f32>(1.000000, 0.000000), a);
            let wc = cmul(vec2<f32>(1.000000, 0.000000), b) + cmul(vec2<f32>(1.000000, 0.000000), a); let wd = cmul(vec2<f32>(2.000000, 0.000000), cmul(a,b));
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[0u]=na.x; out[1u]=na.y; out[2u]=nb.x; out[3u]=nb.y; out[4u]=nc.x; out[5u]=nc.y; out[6u]=nd.x; out[7u]=nd.y;
            out[120u]=nd.x; out[121u]=nd.y; out[122u]=-nb.x; out[123u]=-nb.y; out[124u]=-nc.x; out[125u]=-nc.y; out[126u]=na.x; out[127u]=na.y;
        }
        {
            var wa = vec2<f32>(-2.000000, 0.000000); let wb = cmul(vec2<f32>(1.000000, 0.000000), c) + cmul(vec2<f32>(-1.000000, 0.000000), a);
            let wc = cmul(vec2<f32>(1.000000, 0.000000), c) + cmul(vec2<f32>(-1.000000, 0.000000), a); let wd = cmul(vec2<f32>(2.000000, 0.000000), cmul(a,c));
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[8u]=na.x; out[9u]=na.y; out[10u]=nb.x; out[11u]=nb.y; out[12u]=nc.x; out[13u]=nc.y; out[14u]=nd.x; out[15u]=nd.y;
            out[128u]=nd.x; out[129u]=nd.y; out[130u]=-nb.x; out[131u]=-nb.y; out[132u]=-nc.x; out[133u]=-nc.y; out[134u]=na.x; out[135u]=na.y;
        }
        {
            var wa = vec2<f32>(-2.000000, 0.000000); let wb = cmul(vec2<f32>(1.000000, 0.000000), d) + cmul(vec2<f32>(-1.000000, 0.000000), a);
            let wc = cmul(vec2<f32>(1.000000, 0.000000), d) + cmul(vec2<f32>(-1.000000, 0.000000), a); let wd = cmul(vec2<f32>(2.000000, 0.000000), cmul(a,d));
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[16u]=na.x; out[17u]=na.y; out[18u]=nb.x; out[19u]=nb.y; out[20u]=nc.x; out[21u]=nc.y; out[22u]=nd.x; out[23u]=nd.y;
            out[136u]=nd.x; out[137u]=nd.y; out[138u]=-nb.x; out[139u]=-nb.y; out[140u]=-nc.x; out[141u]=-nc.y; out[142u]=na.x; out[143u]=na.y;
        }
        {
            var wa = vec2<f32>(-2.000000, 0.000000); let wb = cmul(vec2<f32>(1.000000, 0.000000), c) + cmul(vec2<f32>(-1.000000, 0.000000), b);
            let wc = cmul(vec2<f32>(1.000000, 0.000000), c) + cmul(vec2<f32>(-1.000000, 0.000000), b); let wd = cmul(vec2<f32>(2.000000, 0.000000), cmul(b,c));
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[24u]=na.x; out[25u]=na.y; out[26u]=nb.x; out[27u]=nb.y; out[28u]=nc.x; out[29u]=nc.y; out[30u]=nd.x; out[31u]=nd.y;
            out[144u]=nd.x; out[145u]=nd.y; out[146u]=-nb.x; out[147u]=-nb.y; out[148u]=-nc.x; out[149u]=-nc.y; out[150u]=na.x; out[151u]=na.y;
        }
        {
            var wa = vec2<f32>(-2.000000, 0.000000); let wb = cmul(vec2<f32>(1.000000, 0.000000), d) + cmul(vec2<f32>(-1.000000, 0.000000), b);
            let wc = cmul(vec2<f32>(1.000000, 0.000000), d) + cmul(vec2<f32>(-1.000000, 0.000000), b); let wd = cmul(vec2<f32>(2.000000, 0.000000), cmul(b,d));
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[32u]=na.x; out[33u]=na.y; out[34u]=nb.x; out[35u]=nb.y; out[36u]=nc.x; out[37u]=nc.y; out[38u]=nd.x; out[39u]=nd.y;
            out[152u]=nd.x; out[153u]=nd.y; out[154u]=-nb.x; out[155u]=-nb.y; out[156u]=-nc.x; out[157u]=-nc.y; out[158u]=na.x; out[159u]=na.y;
        }
        {
            var wa = vec2<f32>(2.000000, 0.000000); let wb = cmul(vec2<f32>(-1.000000, 0.000000), d) + cmul(vec2<f32>(-1.000000, 0.000000), c);
            let wc = cmul(vec2<f32>(-1.000000, 0.000000), d) + cmul(vec2<f32>(-1.000000, 0.000000), c); let wd = cmul(vec2<f32>(2.000000, 0.000000), cmul(c,d));
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[40u]=na.x; out[41u]=na.y; out[42u]=nb.x; out[43u]=nb.y; out[44u]=nc.x; out[45u]=nc.y; out[46u]=nd.x; out[47u]=nd.y;
            out[160u]=nd.x; out[161u]=nd.y; out[162u]=-nb.x; out[163u]=-nb.y; out[164u]=-nc.x; out[165u]=-nc.y; out[166u]=na.x; out[167u]=na.y;
        }
        {
            var wa = vec2<f32>(0.0,0.0); let wb = cmul(vec2<f32>(0.000000, -1.000000), b) + cmul(vec2<f32>(0.000000, 1.000000), a);
            let wc = cmul(vec2<f32>(0.000000, 1.000000), b) + cmul(vec2<f32>(0.000000, -1.000000), a); let wd = vec2<f32>(0.0,0.0);
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[48u]=na.x; out[49u]=na.y; out[50u]=nb.x; out[51u]=nb.y; out[52u]=nc.x; out[53u]=nc.y; out[54u]=nd.x; out[55u]=nd.y;
            out[168u]=nd.x; out[169u]=nd.y; out[170u]=-nb.x; out[171u]=-nb.y; out[172u]=-nc.x; out[173u]=-nc.y; out[174u]=na.x; out[175u]=na.y;
        }
        {
            var wa = vec2<f32>(0.0,0.0); let wb = cmul(vec2<f32>(0.000000, -1.000000), c) + cmul(vec2<f32>(0.000000, -1.000000), a);
            let wc = cmul(vec2<f32>(0.000000, 1.000000), c) + cmul(vec2<f32>(0.000000, 1.000000), a); let wd = vec2<f32>(0.0,0.0);
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[56u]=na.x; out[57u]=na.y; out[58u]=nb.x; out[59u]=nb.y; out[60u]=nc.x; out[61u]=nc.y; out[62u]=nd.x; out[63u]=nd.y;
            out[176u]=nd.x; out[177u]=nd.y; out[178u]=-nb.x; out[179u]=-nb.y; out[180u]=-nc.x; out[181u]=-nc.y; out[182u]=na.x; out[183u]=na.y;
        }
        {
            var wa = vec2<f32>(0.0,0.0); let wb = cmul(vec2<f32>(0.000000, -1.000000), d) + cmul(vec2<f32>(0.000000, -1.000000), a);
            let wc = cmul(vec2<f32>(0.000000, 1.000000), d) + cmul(vec2<f32>(0.000000, 1.000000), a); let wd = vec2<f32>(0.0,0.0);
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[64u]=na.x; out[65u]=na.y; out[66u]=nb.x; out[67u]=nb.y; out[68u]=nc.x; out[69u]=nc.y; out[70u]=nd.x; out[71u]=nd.y;
            out[184u]=nd.x; out[185u]=nd.y; out[186u]=-nb.x; out[187u]=-nb.y; out[188u]=-nc.x; out[189u]=-nc.y; out[190u]=na.x; out[191u]=na.y;
        }
        {
            var wa = vec2<f32>(0.0,0.0); let wb = cmul(vec2<f32>(0.000000, -1.000000), c) + cmul(vec2<f32>(0.000000, -1.000000), b);
            let wc = cmul(vec2<f32>(0.000000, 1.000000), c) + cmul(vec2<f32>(0.000000, 1.000000), b); let wd = vec2<f32>(0.0,0.0);
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[72u]=na.x; out[73u]=na.y; out[74u]=nb.x; out[75u]=nb.y; out[76u]=nc.x; out[77u]=nc.y; out[78u]=nd.x; out[79u]=nd.y;
            out[192u]=nd.x; out[193u]=nd.y; out[194u]=-nb.x; out[195u]=-nb.y; out[196u]=-nc.x; out[197u]=-nc.y; out[198u]=na.x; out[199u]=na.y;
        }
        {
            var wa = vec2<f32>(0.0,0.0); let wb = cmul(vec2<f32>(0.000000, -1.000000), d) + cmul(vec2<f32>(0.000000, -1.000000), b);
            let wc = cmul(vec2<f32>(0.000000, 1.000000), d) + cmul(vec2<f32>(0.000000, 1.000000), b); let wd = vec2<f32>(0.0,0.0);
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[80u]=na.x; out[81u]=na.y; out[82u]=nb.x; out[83u]=nb.y; out[84u]=nc.x; out[85u]=nc.y; out[86u]=nd.x; out[87u]=nd.y;
            out[200u]=nd.x; out[201u]=nd.y; out[202u]=-nb.x; out[203u]=-nb.y; out[204u]=-nc.x; out[205u]=-nc.y; out[206u]=na.x; out[207u]=na.y;
        }
        {
            var wa = vec2<f32>(0.0,0.0); let wb = cmul(vec2<f32>(0.000000, 1.000000), d) + cmul(vec2<f32>(0.000000, -1.000000), c);
            let wc = cmul(vec2<f32>(0.000000, -1.000000), d) + cmul(vec2<f32>(0.000000, 1.000000), c); let wd = vec2<f32>(0.0,0.0);
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[88u]=na.x; out[89u]=na.y; out[90u]=nb.x; out[91u]=nb.y; out[92u]=nc.x; out[93u]=nc.y; out[94u]=nd.x; out[95u]=nd.y;
            out[208u]=nd.x; out[209u]=nd.y; out[210u]=-nb.x; out[211u]=-nb.y; out[212u]=-nc.x; out[213u]=-nc.y; out[214u]=na.x; out[215u]=na.y;
        }
        {
            var wa = vec2<f32>(0.0,0.0); let wb = cmul(vec2<f32>(-1.000000, 0.000000), b) + cmul(vec2<f32>(1.000000, 0.000000), a);
            let wc = cmul(vec2<f32>(-1.000000, 0.000000), b) + cmul(vec2<f32>(1.000000, 0.000000), a); let wd = cmul(vec2<f32>(-1.000000, 0.000000), cmul(b,b)) + cmul(vec2<f32>(1.000000, 0.000000), cmul(a,a));
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[96u]=na.x; out[97u]=na.y; out[98u]=nb.x; out[99u]=nb.y; out[100u]=nc.x; out[101u]=nc.y; out[102u]=nd.x; out[103u]=nd.y;
            out[216u]=nd.x; out[217u]=nd.y; out[218u]=-nb.x; out[219u]=-nb.y; out[220u]=-nc.x; out[221u]=-nc.y; out[222u]=na.x; out[223u]=na.y;
        }
        {
            var wa = vec2<f32>(0.0,0.0); let wb = cmul(vec2<f32>(1.154701, 0.000000), c) + cmul(vec2<f32>(0.577350, 0.000000), b) + cmul(vec2<f32>(0.577350, 0.000000), a);
            let wc = cmul(vec2<f32>(1.154701, 0.000000), c) + cmul(vec2<f32>(0.577350, 0.000000), b) + cmul(vec2<f32>(0.577350, 0.000000), a); let wd = cmul(vec2<f32>(-1.154701, 0.000000), cmul(c,c)) + cmul(vec2<f32>(0.577350, 0.000000), cmul(b,b)) + cmul(vec2<f32>(0.577350, 0.000000), cmul(a,a));
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[104u]=na.x; out[105u]=na.y; out[106u]=nb.x; out[107u]=nb.y; out[108u]=nc.x; out[109u]=nc.y; out[110u]=nd.x; out[111u]=nd.y;
            out[224u]=nd.x; out[225u]=nd.y; out[226u]=-nb.x; out[227u]=-nb.y; out[228u]=-nc.x; out[229u]=-nc.y; out[230u]=na.x; out[231u]=na.y;
        }
        {
            var wa = vec2<f32>(0.0,0.0); let wb = cmul(vec2<f32>(1.224745, 0.000000), d) + cmul(vec2<f32>(-0.408248, 0.000000), c) + cmul(vec2<f32>(0.408248, 0.000000), b) + cmul(vec2<f32>(0.408248, 0.000000), a);
            let wc = cmul(vec2<f32>(1.224745, 0.000000), d) + cmul(vec2<f32>(-0.408248, 0.000000), c) + cmul(vec2<f32>(0.408248, 0.000000), b) + cmul(vec2<f32>(0.408248, 0.000000), a); let wd = cmul(vec2<f32>(-1.224745, 0.000000), cmul(d,d)) + cmul(vec2<f32>(0.408248, 0.000000), cmul(c,c)) + cmul(vec2<f32>(0.408248, 0.000000), cmul(b,b)) + cmul(vec2<f32>(0.408248, 0.000000), cmul(a,a));
            let tr = wa + wd; if (tr.x*tr.x+tr.y*tr.y < 0.09) { wa = wa + vec2<f32>(plug,0.0); }
            let dtm = csub(cmul(wa,wd), cmul(wb,wc)); let sd = csqrt(dtm);
            let na=cdiv(wa,sd); let nb=cdiv(wb,sd); let nc=cdiv(wc,sd); let nd=cdiv(wd,sd);
            out[112u]=na.x; out[113u]=na.y; out[114u]=nb.x; out[115u]=nb.y; out[116u]=nc.x; out[117u]=nc.y; out[118u]=nd.x; out[119u]=nd.y;
            out[232u]=nd.x; out[233u]=nd.y; out[234u]=-nb.x; out[235u]=-nb.y; out[236u]=-nc.x; out[237u]=-nc.y; out[238u]=na.x; out[239u]=na.y;
        }
    }
    return out;
}
"#;

const WGSL_2D: &str = r#"
fn variation_su_custom(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {

    let group = u32(get_param(xform_id, variation_id, 0u));
    let avoid = get_param(xform_id, variation_id, 1u) > 0.5;
    let theta = get_param(xform_id, variation_id, 2u) * 0.01745329252;
    let eta = get_param(xform_id, variation_id, 3u) * 0.01745329252;
    let delta = get_param(xform_id, variation_id, 4u);
    let dc_mode = u32(get_param(xform_id, variation_id, 5u));
    let dc_scale = get_param(xform_id, variation_id, 6u);
    let color_speed = get_param(xform_id, variation_id, 7u);

    // Generator counts: SU(2) Custom 6, SU(3) Custom 16, SU(4) Custom 30.
    var cnt = 16u;
    if (group == 0u) { cnt = 6u; }
    else if (group == 2u) { cnt = 30u; }
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

    let bo = 19u + k * 8u;   // init-derived base matrix, 19 = user param count
    let ba = SuMat(
        vec2<f32>(get_param(xform_id, variation_id, bo), get_param(xform_id, variation_id, bo + 1u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 2u), get_param(xform_id, variation_id, bo + 3u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 4u), get_param(xform_id, variation_id, bo + 5u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 6u), get_param(xform_id, variation_id, bo + 7u)));
    var out = su_apply_m(ba, p, cj, cji);
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
fn variation_su_custom(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {

    let group = u32(get_param(xform_id, variation_id, 0u));
    let avoid = get_param(xform_id, variation_id, 1u) > 0.5;
    let theta = get_param(xform_id, variation_id, 2u) * 0.01745329252;
    let eta = get_param(xform_id, variation_id, 3u) * 0.01745329252;
    let delta = get_param(xform_id, variation_id, 4u);
    let dc_mode = u32(get_param(xform_id, variation_id, 5u));
    let dc_scale = get_param(xform_id, variation_id, 6u);
    let color_speed = get_param(xform_id, variation_id, 7u);

    // Generator counts: SU(2) Custom 6, SU(3) Custom 16, SU(4) Custom 30.
    var cnt = 16u;
    if (group == 0u) { cnt = 6u; }
    else if (group == 2u) { cnt = 30u; }
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

    let bo = 19u + k * 8u;   // init-derived base matrix, 19 = user param count
    let ba = SuMat(
        vec2<f32>(get_param(xform_id, variation_id, bo), get_param(xform_id, variation_id, bo + 1u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 2u), get_param(xform_id, variation_id, bo + 3u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 4u), get_param(xform_id, variation_id, bo + 5u)),
        vec2<f32>(get_param(xform_id, variation_id, bo + 6u), get_param(xform_id, variation_id, bo + 7u)));
    let space = u32(get_param(xform_id, variation_id, 8u));
    var out: vec3<f32>;
    if (space == 1u) {
        out = su_apply_m3(ba, p, cj, cji);
    } else {
        out = vec3<f32>(su_apply_m(ba, p.xy, cj, cji), p.z);
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
