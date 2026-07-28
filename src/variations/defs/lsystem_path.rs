//! `lsystem_path` — finite-depth IFS path sampler (original).
//!
//! Draws the depth-k polyline of an IFS whose maps trace a curve in
//! visiting order — the finite-depth L-system look (the iconic Hilbert
//! maze, a depth-6 Koch flake) that the plain transforms cannot show:
//! their attractor is the INFINITE-depth limit, and for a space-filling
//! curve that limit is a featureless filled square.
//!
//! No geometry is stored. Vertex `i` of the depth-k curve is the
//! composition of k affine maps selected by the base-n digits of `i`
//! (least-significant digit innermost), applied to the curve's start.
//! Each sample draws a uniform `t`, computes vertex `⌊t·nᵏ⌋` and its
//! successor by two such compositions, and lerps between them — so the
//! whole polyline, `nᵏ` segments, costs ~2k small matrix multiplies per
//! sample and twelve maps of parameters. Depth is a live parameter;
//! nothing is re-baked when it changes.
//!
//! The maps come from the L-System script's constructions (edge or node
//! rewriting), which store them in visiting order; `t` doubles as the
//! curve parameter, exposed as direct color when `dc` is on, so the
//! palette runs along the path.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Finite-depth IFS path: plots the depth-k polyline of up to 12 affine
/// maps in visiting order, with the curve parameter as optional direct
/// color. Written by the L-System script; depth is a live parameter.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static LSYSTEM_PATH: VariationDef = VariationDef {
    name: "lsystem_path",
    aliases: &[],
    display_name: "L-System Path",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("iterations", "Iterations", int, 5.0, 1.0, 12.0, "L-system depth: the path has map_count^iterations segments. A live parameter — nothing is re-baked when it changes."),
        param!("map_count", "Map Count", int, 4.0, 2.0, 12.0, "How many of the twelve map slots are in use. Set by the L-System script."),
        param!("connect", "Connect", bool, true, "Draw the connecting segments between consecutive vertices (the path). Off plots only the vertices."),
        param!("dc", "Direct Color", bool, true, "Color by the curve parameter t, so the palette runs along the path from start to end (needs the transform's Direct Color at 1)."),
        param!("m0_a", "M0 A", unlimited_float, 1.0, -4.0, 4.0, "Map 0: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m0_b", "M0 B", unlimited_float, 0.0, -4.0, 4.0, "Map 0: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m0_c", "M0 C", unlimited_float, 0.0, -4.0, 4.0, "Map 0: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m0_d", "M0 D", unlimited_float, 1.0, -4.0, 4.0, "Map 0: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m0_e", "M0 E", unlimited_float, 0.0, -4.0, 4.0, "Map 0: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m0_f", "M0 F", unlimited_float, 0.0, -4.0, 4.0, "Map 0: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m1_a", "M1 A", unlimited_float, 0.0, -4.0, 4.0, "Map 1: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m1_b", "M1 B", unlimited_float, 0.0, -4.0, 4.0, "Map 1: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m1_c", "M1 C", unlimited_float, 0.0, -4.0, 4.0, "Map 1: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m1_d", "M1 D", unlimited_float, 0.0, -4.0, 4.0, "Map 1: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m1_e", "M1 E", unlimited_float, 0.0, -4.0, 4.0, "Map 1: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m1_f", "M1 F", unlimited_float, 0.0, -4.0, 4.0, "Map 1: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m2_a", "M2 A", unlimited_float, 0.0, -4.0, 4.0, "Map 2: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m2_b", "M2 B", unlimited_float, 0.0, -4.0, 4.0, "Map 2: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m2_c", "M2 C", unlimited_float, 0.0, -4.0, 4.0, "Map 2: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m2_d", "M2 D", unlimited_float, 0.0, -4.0, 4.0, "Map 2: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m2_e", "M2 E", unlimited_float, 0.0, -4.0, 4.0, "Map 2: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m2_f", "M2 F", unlimited_float, 0.0, -4.0, 4.0, "Map 2: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m3_a", "M3 A", unlimited_float, 0.0, -4.0, 4.0, "Map 3: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m3_b", "M3 B", unlimited_float, 0.0, -4.0, 4.0, "Map 3: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m3_c", "M3 C", unlimited_float, 0.0, -4.0, 4.0, "Map 3: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m3_d", "M3 D", unlimited_float, 0.0, -4.0, 4.0, "Map 3: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m3_e", "M3 E", unlimited_float, 0.0, -4.0, 4.0, "Map 3: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m3_f", "M3 F", unlimited_float, 0.0, -4.0, 4.0, "Map 3: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m4_a", "M4 A", unlimited_float, 0.0, -4.0, 4.0, "Map 4: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m4_b", "M4 B", unlimited_float, 0.0, -4.0, 4.0, "Map 4: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m4_c", "M4 C", unlimited_float, 0.0, -4.0, 4.0, "Map 4: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m4_d", "M4 D", unlimited_float, 0.0, -4.0, 4.0, "Map 4: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m4_e", "M4 E", unlimited_float, 0.0, -4.0, 4.0, "Map 4: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m4_f", "M4 F", unlimited_float, 0.0, -4.0, 4.0, "Map 4: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m5_a", "M5 A", unlimited_float, 0.0, -4.0, 4.0, "Map 5: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m5_b", "M5 B", unlimited_float, 0.0, -4.0, 4.0, "Map 5: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m5_c", "M5 C", unlimited_float, 0.0, -4.0, 4.0, "Map 5: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m5_d", "M5 D", unlimited_float, 0.0, -4.0, 4.0, "Map 5: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m5_e", "M5 E", unlimited_float, 0.0, -4.0, 4.0, "Map 5: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m5_f", "M5 F", unlimited_float, 0.0, -4.0, 4.0, "Map 5: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m6_a", "M6 A", unlimited_float, 0.0, -4.0, 4.0, "Map 6: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m6_b", "M6 B", unlimited_float, 0.0, -4.0, 4.0, "Map 6: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m6_c", "M6 C", unlimited_float, 0.0, -4.0, 4.0, "Map 6: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m6_d", "M6 D", unlimited_float, 0.0, -4.0, 4.0, "Map 6: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m6_e", "M6 E", unlimited_float, 0.0, -4.0, 4.0, "Map 6: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m6_f", "M6 F", unlimited_float, 0.0, -4.0, 4.0, "Map 6: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m7_a", "M7 A", unlimited_float, 0.0, -4.0, 4.0, "Map 7: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m7_b", "M7 B", unlimited_float, 0.0, -4.0, 4.0, "Map 7: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m7_c", "M7 C", unlimited_float, 0.0, -4.0, 4.0, "Map 7: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m7_d", "M7 D", unlimited_float, 0.0, -4.0, 4.0, "Map 7: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m7_e", "M7 E", unlimited_float, 0.0, -4.0, 4.0, "Map 7: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m7_f", "M7 F", unlimited_float, 0.0, -4.0, 4.0, "Map 7: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m8_a", "M8 A", unlimited_float, 0.0, -4.0, 4.0, "Map 8: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m8_b", "M8 B", unlimited_float, 0.0, -4.0, 4.0, "Map 8: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m8_c", "M8 C", unlimited_float, 0.0, -4.0, 4.0, "Map 8: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m8_d", "M8 D", unlimited_float, 0.0, -4.0, 4.0, "Map 8: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m8_e", "M8 E", unlimited_float, 0.0, -4.0, 4.0, "Map 8: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m8_f", "M8 F", unlimited_float, 0.0, -4.0, 4.0, "Map 8: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m9_a", "M9 A", unlimited_float, 0.0, -4.0, 4.0, "Map 9: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m9_b", "M9 B", unlimited_float, 0.0, -4.0, 4.0, "Map 9: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m9_c", "M9 C", unlimited_float, 0.0, -4.0, 4.0, "Map 9: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m9_d", "M9 D", unlimited_float, 0.0, -4.0, 4.0, "Map 9: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m9_e", "M9 E", unlimited_float, 0.0, -4.0, 4.0, "Map 9: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m9_f", "M9 F", unlimited_float, 0.0, -4.0, 4.0, "Map 9: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m10_a", "M10 A", unlimited_float, 0.0, -4.0, 4.0, "Map 10: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m10_b", "M10 B", unlimited_float, 0.0, -4.0, 4.0, "Map 10: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m10_c", "M10 C", unlimited_float, 0.0, -4.0, 4.0, "Map 10: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m10_d", "M10 D", unlimited_float, 0.0, -4.0, 4.0, "Map 10: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m10_e", "M10 E", unlimited_float, 0.0, -4.0, 4.0, "Map 10: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m10_f", "M10 F", unlimited_float, 0.0, -4.0, 4.0, "Map 10: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m11_a", "M11 A", unlimited_float, 0.0, -4.0, 4.0, "Map 11: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m11_b", "M11 B", unlimited_float, 0.0, -4.0, 4.0, "Map 11: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m11_c", "M11 C", unlimited_float, 0.0, -4.0, 4.0, "Map 11: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m11_d", "M11 D", unlimited_float, 0.0, -4.0, 4.0, "Map 11: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m11_e", "M11 E", unlimited_float, 0.0, -4.0, 4.0, "Map 11: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
        param!("m11_f", "M11 F", unlimited_float, 0.0, -4.0, 4.0, "Map 11: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn lsystem_path_vertex(xform_id: u32, variation_id: u32, idx: u32, iters: u32, n: u32) -> vec2<f32> {
    // Digits least-significant first: the LSD picks the innermost
    // (deepest) map, the MSD the outermost top-level cell.
    var v = vec2<f32>(0.0, 0.0);
    var rem = idx;
    for (var j = 0u; j < iters; j = j + 1u) {
        let d = rem % n;
        rem = rem / n;
        let base = 4u + d * 6u;
        let ma = get_param(xform_id, variation_id, base);
        let mb = get_param(xform_id, variation_id, base + 1u);
        let mc = get_param(xform_id, variation_id, base + 2u);
        let md = get_param(xform_id, variation_id, base + 3u);
        let me = get_param(xform_id, variation_id, base + 4u);
        let mf = get_param(xform_id, variation_id, base + 5u);
        v = vec2<f32>(ma * v.x + mb * v.y + me, mc * v.x + md * v.y + mf);
    }
    return v;
}

fn variation_lsystem_path(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let iters = clamp(u32(get_param(xform_id, variation_id, 0u)), 1u, 12u);
    let n = clamp(u32(get_param(xform_id, variation_id, 1u)), 2u, 12u);
    let connect = get_param(xform_id, variation_id, 2u) > 0.5;
    let dc = get_param(xform_id, variation_id, 3u) > 0.5;

    // Total cells, capped so f32(t)·total keeps sub-cell precision.
    var total = 1u;
    for (var j = 0u; j < iters; j = j + 1u) {
        total = min(total * n, 16000000u);
    }
    let t = rng_nextf(rng);
    let idx = min(u32(t * f32(total)), total - 1u);

    var out = lsystem_path_vertex(xform_id, variation_id, idx, iters, n);
    if (connect) {
        var nxt: vec2<f32>;
        if (idx + 1u < total) {
            nxt = lsystem_path_vertex(xform_id, variation_id, idx + 1u, iters, n);
        } else {
            // The curve's own endpoint in the unit-displacement frame.
            nxt = vec2<f32>(1.0, 0.0);
        }
        let frac = clamp(t * f32(total) - f32(idx), 0.0, 1.0);
        out = mix(out, nxt, frac);
    }
    if (dc) {
        *vc = t;
    }
    return out;
}
"#;

const WGSL_3D: &str = r#"
fn lsystem_path_vertex(xform_id: u32, variation_id: u32, idx: u32, iters: u32, n: u32) -> vec2<f32> {
    var v = vec2<f32>(0.0, 0.0);
    var rem = idx;
    for (var j = 0u; j < iters; j = j + 1u) {
        let d = rem % n;
        rem = rem / n;
        let base = 4u + d * 6u;
        let ma = get_param(xform_id, variation_id, base);
        let mb = get_param(xform_id, variation_id, base + 1u);
        let mc = get_param(xform_id, variation_id, base + 2u);
        let md = get_param(xform_id, variation_id, base + 3u);
        let me = get_param(xform_id, variation_id, base + 4u);
        let mf = get_param(xform_id, variation_id, base + 5u);
        v = vec2<f32>(ma * v.x + mb * v.y + me, mc * v.x + md * v.y + mf);
    }
    return v;
}

fn variation_lsystem_path(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let iters = clamp(u32(get_param(xform_id, variation_id, 0u)), 1u, 12u);
    let n = clamp(u32(get_param(xform_id, variation_id, 1u)), 2u, 12u);
    let connect = get_param(xform_id, variation_id, 2u) > 0.5;
    let dc = get_param(xform_id, variation_id, 3u) > 0.5;

    var total = 1u;
    for (var j = 0u; j < iters; j = j + 1u) {
        total = min(total * n, 16000000u);
    }
    let t = rng_nextf(rng);
    let idx = min(u32(t * f32(total)), total - 1u);

    var out = lsystem_path_vertex(xform_id, variation_id, idx, iters, n);
    if (connect) {
        var nxt: vec2<f32>;
        if (idx + 1u < total) {
            nxt = lsystem_path_vertex(xform_id, variation_id, idx + 1u, iters, n);
        } else {
            nxt = vec2<f32>(1.0, 0.0);
        }
        let frac = clamp(t * f32(total) - f32(idx), 0.0, 1.0);
        out = mix(out, nxt, frac);
    }
    if (dc) {
        *vc = t;
    }
    // The path lives in the xy plane; z rides along unchanged.
    return vec3<f32>(out, p.z);
}
"#;
