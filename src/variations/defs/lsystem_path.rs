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
        param!("anchored", "Anchored", bool, false, "Vertex-chain mode: connect the images of the anchor point in consecutive cells, instead of drawing each cell's entry-to-exit span. Node-rewriting (space-filling) curves need this — their spans lie on the cell-boundary lattice and overlap each other; the centre chain is the classic self-avoiding drawing."),
        param!("anchor_x", "Anchor X", unlimited_float, 0.5, -2.0, 2.0, "Anchor point x in the curve's unit-displacement frame (the attractor's bounding-box centre, set by the script)."),
        param!("anchor_y", "Anchor Y", unlimited_float, 0.0, -2.0, 2.0, "Anchor point y."),
        param!("thickness", "Thickness", float, 0.0, 0.0, 0.2, "Half-width of the drawn line, in the curve's own units (the whole curve spans 1). Corners are joined seamlessly: each segment is clipped at the mitre line it shares with its neighbours and the wedge the turn opens is filled, so the stroke is covered exactly once — no bright spot at a vertex. Note the density trade — the same samples spread over more area, so a thick line is dimmer; raise Brightness to match."),
        param!("soft", "Soft Edges", bool, false, "Gaussian falloff across the width instead of a flat ribbon. Flat reads as a drawn line; soft reads as a glow."),
        param!("offset_x", "Offset X", unlimited_float, 0.0, -2.0, 2.0, "Move the whole curve along x, in the curve's own units. The path is built in its own frame, so an offset of minus its centre puts the object on the origin — which is what rotation and zoom orbit around. The script sets this to centre the curve."),
        param!("offset_y", "Offset Y", unlimited_float, 0.0, -2.0, 2.0, "Move the whole curve along y."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
// The cell's whole affine, composed from the digits of idx
// (least-significant digit innermost). ONE composition yields BOTH
// endpoints of the cell's segment — entry A·(0,0)+t and exit A·(1,0)+t —
// so numeric error can shrink or shift a segment but never TILT it, and
// consecutive cells meet exactly when the maps are exact.
fn lsystem_path_cell(xform_id: u32, variation_id: u32, idx: u32, iters: u32, n: u32) -> vec4<f32> {
    var ax = 1.0; var aby = 0.0; var cx = 0.0; var dy = 1.0;
    var tx = 0.0; var ty = 0.0;
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
        // Compose M ∘ C (apply M after what is already there).
        let nax = ma * ax + mb * cx;
        let nab = ma * aby + mb * dy;
        let ncx = mc * ax + md * cx;
        let ndy = mc * aby + md * dy;
        let ntx = ma * tx + mb * ty + me;
        let nty = mc * tx + md * ty + mf;
        ax = nax; aby = nab; cx = ncx; dy = ndy; tx = ntx; ty = nty;
    }
    // Entry (A·origin + t) and exit (A·(1,0) + t).
    return vec4<f32>(tx, ty, ax + tx, cx + ty);
}

fn lsystem_path_point(xform_id: u32, variation_id: u32, idx: u32, iters: u32, n: u32, anchor: vec2<f32>) -> vec2<f32> {
    // The digit composition applied straight to the anchor point.
    var v = anchor;
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
    let anchored = get_param(xform_id, variation_id, 76u) > 0.5;

    // Total cells, capped so f32(t)·total keeps sub-cell precision.
    var total = 1u;
    for (var j = 0u; j < iters; j = j + 1u) {
        total = min(total * n, 16000000u);
    }
    let t = rng_nextf(rng);

    // Thickness up front: a seamless join needs the neighbouring segment
    // directions, and each costs another walk down the map chain — wasted
    // work on the overwhelmingly common thickness = 0.
    let thickness = get_param(xform_id, variation_id, 79u);
    let thick = thickness > 0.0;

    var seg_a: vec2<f32>;
    var seg_b: vec2<f32>;
    var frac: f32 = 0.0;
    // Directions of the segments arriving at seg_a and leaving seg_b.
    // Zero where there is none — the ends of the curve.
    var dir_in = vec2<f32>(0.0, 0.0);
    var dir_out = vec2<f32>(0.0, 0.0);
    if (anchored) {
        // Vertex chain through the anchor's image in each cell — the
        // classic space-filling drawing (anchor = attractor centre gives
        // cell centres). Cell spans would lie on the boundary lattice
        // and OVERLAP each other; centres are self-avoiding.
        let anchor = vec2<f32>(
            get_param(xform_id, variation_id, 77u),
            get_param(xform_id, variation_id, 78u));
        let segs = max(total - 1u, 1u);
        let ts = t * f32(segs);
        let idx = min(u32(ts), segs - 1u);
        seg_a = lsystem_path_point(xform_id, variation_id, idx, iters, n, anchor);
        seg_b = lsystem_path_point(xform_id, variation_id, idx + 1u, iters, n, anchor);
        frac = clamp(ts - f32(idx), 0.0, 1.0);
        if (thick && idx > 0u) {
            dir_in = seg_a - lsystem_path_point(xform_id, variation_id, idx - 1u, iters, n, anchor);
        }
        if (thick && idx + 2u <= segs) {
            dir_out = lsystem_path_point(xform_id, variation_id, idx + 2u, iters, n, anchor) - seg_b;
        }
    } else {
        let idx = min(u32(t * f32(total)), total - 1u);
        let cell = lsystem_path_cell(xform_id, variation_id, idx, iters, n);
        seg_a = cell.xy;
        seg_b = cell.zw;
        frac = clamp(t * f32(total) - f32(idx), 0.0, 1.0);
        // Cell spans need not chain, so a neighbour only counts as one
        // where it actually meets this span — a join across a gap would
        // be density hanging in mid-air.
        if (thick && idx > 0u) {
            let prev = lsystem_path_cell(xform_id, variation_id, idx - 1u, iters, n);
            if (distance(prev.zw, seg_a) < thickness) {
                dir_in = prev.zw - prev.xy;
            }
        }
        if (thick && idx + 1u < total) {
            let next = lsystem_path_cell(xform_id, variation_id, idx + 1u, iters, n);
            if (distance(next.xy, seg_b) < thickness) {
                dir_out = next.zw - next.xy;
            }
        }
    }

    var out = seg_a;
    if (connect) {
        out = mix(seg_a, seg_b, frac);
    }

    // Thicken PERPENDICULAR to the segment: an offset along it would only
    // slide the sample along ground the t-sweep already covers.
    if (thick) {
        let soft = get_param(xform_id, variation_id, 80u) > 0.5;
        let dseg = seg_b - seg_a;
        let dlen = length(dseg);
        let has_seg = connect && dlen > 1e-9;
        let r = thickness;

        if (soft || !has_seg) {
            // A gaussian shaft has no hard edge, so its own tails round
            // the corners; a join there would only pile density on
            // density. Same for vertices-only, which is all corner.
            var jit = (rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0) * 0.5;
            if (!soft) {
                jit = sqrt(rng_nextf(rng));
            }
            if (has_seg) {
                let perp = vec2<f32>(-dseg.y, dseg.x) / dlen;
                out = out + perp * (r * jit);
            } else {
                let ang = rng_nextf(rng) * 6.28318530718;
                out = out + vec2<f32>(cos(ang), sin(ang)) * (r * abs(jit));
            }
        } else {
            // Seamless join. The stroke around a polyline splits exactly
            // into two kinds of piece: each segment's rectangle CLIPPED at
            // the mitre line it shares with its neighbours, and the wedge
            // that the turn opens on the OUTSIDE of each corner. Together
            // they tile it — every point covered once.
            //
            // Getting this wrong is what the earlier attempts looked like:
            // an unclipped rectangle plus a full disc covers the corner
            // twice over (a visible circle), and clipping the rectangle
            // only against the disc still leaves the two rectangles
            // crossing outside it, since the corner square reaches r*sqrt2
            // while the disc stops at r (the faint crescents).
            let v = dseg / dlen;
            let perp = vec2<f32>(-v.y, v.x);

            // The mitre line at a corner has normal u + v: the one
            // direction with a positive component along both segments, so
            // it separates the arriving shaft from the leaving one.
            var n0 = vec2<f32>(0.0, 0.0);
            var phi0 = 0.0;
            let li = length(dir_in);
            if (li > 1e-9) {
                let u = dir_in / li;
                phi0 = acos(clamp(dot(u, v), -1.0, 1.0));
                if (phi0 > 1e-4 && phi0 < 3.1) {
                    n0 = normalize(u + v);
                }
            }
            var n1 = vec2<f32>(0.0, 0.0);
            var phi1 = 0.0;
            let lo = length(dir_out);
            if (lo > 1e-9) {
                let z = dir_out / lo;
                phi1 = acos(clamp(dot(v, z), -1.0, 1.0));
                if (phi1 > 1e-4 && phi1 < 3.1) {
                    n1 = normalize(v + z);
                }
            }

            // Areas, all divided by r. The mitre takes a triangle with
            // legs r and r*tan(phi/2) off each end; the wedge it leaves
            // outside has area (phi/2)*r^2. tan is clamped because a
            // near-reversal would otherwise claim more than the segment.
            let cut0 = 0.5 * r * min(tan(0.5 * phi0), 8.0);
            let cut1 = 0.5 * r * min(tan(0.5 * phi1), 8.0);
            let shaft = max(2.0 * dlen - cut0 - cut1, 0.0);
            let wedge = 0.5 * phi0 * r;

            if (rng_nextf(rng) * (shaft + wedge) < wedge) {
                // Basis (u, e2) with e2 pointing OUTSIDE the turn, so the
                // wedge is beta in [-pi/2, phi0 - pi/2]: from the far edge
                // of the arriving shaft round to the near edge of this one.
                let u = dir_in / li;
                let sgn = select(-1.0, 1.0, (u.x * v.y - u.y * v.x) > 0.0);
                let e2 = sgn * vec2<f32>(-u.y, u.x);
                let beta = -1.5707963268 + phi0 * rng_nextf(rng);
                out = seg_a + (cos(beta) * u + sin(beta) * e2) * (r * sqrt(rng_nextf(rng)));
            } else {
                // Uniform over the rectangle minus whatever the mitres
                // take. Rejection keeps it uniform in AREA — sampling a
                // shortened span directly would not — and the cuts are a
                // percent or two of the rectangle at sensible thicknesses.
                // Falling out of the loop keeps the last sample rather
                // than spinning.
                var s = 0.0;
                var o = 0.0;
                for (var k = 0u; k < 8u; k = k + 1u) {
                    s = rng_nextf(rng) * dlen;
                    o = (rng_nextf(rng) * 2.0 - 1.0) * r;
                    let rel = v * s + perp * o;
                    let in0 = dot(n0, n0) < 0.5 || dot(rel, n0) >= 0.0;
                    let in1 = dot(n1, n1) < 0.5 || dot(rel - dseg, n1) <= 0.0;
                    if (in0 && in1) {
                        break;
                    }
                }
                out = seg_a + v * s + perp * o;
            }
        }
    }

    // Move the whole curve. The path is built in its own frame, so an
    // offset of minus its centre puts the object on the origin — which is
    // what rotation and zoom orbit around.
    out = out + vec2<f32>(
        get_param(xform_id, variation_id, 81u),
        get_param(xform_id, variation_id, 82u));

    if (dc) {
        *vc = t;
    }
    return out;
}
"#;

const WGSL_3D: &str = r#"
fn lsystem_path_cell(xform_id: u32, variation_id: u32, idx: u32, iters: u32, n: u32) -> vec4<f32> {
    var ax = 1.0; var aby = 0.0; var cx = 0.0; var dy = 1.0;
    var tx = 0.0; var ty = 0.0;
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
        let nax = ma * ax + mb * cx;
        let nab = ma * aby + mb * dy;
        let ncx = mc * ax + md * cx;
        let ndy = mc * aby + md * dy;
        let ntx = ma * tx + mb * ty + me;
        let nty = mc * tx + md * ty + mf;
        ax = nax; aby = nab; cx = ncx; dy = ndy; tx = ntx; ty = nty;
    }
    return vec4<f32>(tx, ty, ax + tx, cx + ty);
}

fn lsystem_path_point(xform_id: u32, variation_id: u32, idx: u32, iters: u32, n: u32, anchor: vec2<f32>) -> vec2<f32> {
    // The digit composition applied straight to the anchor point.
    var v = anchor;
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
    let anchored = get_param(xform_id, variation_id, 76u) > 0.5;

    var total = 1u;
    for (var j = 0u; j < iters; j = j + 1u) {
        total = min(total * n, 16000000u);
    }
    let t = rng_nextf(rng);

    // Thickness up front: a seamless join needs the neighbouring segment
    // directions, and each costs another walk down the map chain — wasted
    // work on the overwhelmingly common thickness = 0.
    let thickness = get_param(xform_id, variation_id, 79u);
    let thick = thickness > 0.0;

    var seg_a: vec2<f32>;
    var seg_b: vec2<f32>;
    var frac: f32 = 0.0;
    // Directions of the segments arriving at seg_a and leaving seg_b.
    // Zero where there is none — the ends of the curve.
    var dir_in = vec2<f32>(0.0, 0.0);
    var dir_out = vec2<f32>(0.0, 0.0);
    if (anchored) {
        // Vertex chain through the anchor's image in each cell — the
        // classic space-filling drawing (anchor = attractor centre gives
        // cell centres). Cell spans would lie on the boundary lattice
        // and OVERLAP each other; centres are self-avoiding.
        let anchor = vec2<f32>(
            get_param(xform_id, variation_id, 77u),
            get_param(xform_id, variation_id, 78u));
        let segs = max(total - 1u, 1u);
        let ts = t * f32(segs);
        let idx = min(u32(ts), segs - 1u);
        seg_a = lsystem_path_point(xform_id, variation_id, idx, iters, n, anchor);
        seg_b = lsystem_path_point(xform_id, variation_id, idx + 1u, iters, n, anchor);
        frac = clamp(ts - f32(idx), 0.0, 1.0);
        if (thick && idx > 0u) {
            dir_in = seg_a - lsystem_path_point(xform_id, variation_id, idx - 1u, iters, n, anchor);
        }
        if (thick && idx + 2u <= segs) {
            dir_out = lsystem_path_point(xform_id, variation_id, idx + 2u, iters, n, anchor) - seg_b;
        }
    } else {
        let idx = min(u32(t * f32(total)), total - 1u);
        let cell = lsystem_path_cell(xform_id, variation_id, idx, iters, n);
        seg_a = cell.xy;
        seg_b = cell.zw;
        frac = clamp(t * f32(total) - f32(idx), 0.0, 1.0);
        // Cell spans need not chain, so a neighbour only counts as one
        // where it actually meets this span — a join across a gap would
        // be density hanging in mid-air.
        if (thick && idx > 0u) {
            let prev = lsystem_path_cell(xform_id, variation_id, idx - 1u, iters, n);
            if (distance(prev.zw, seg_a) < thickness) {
                dir_in = prev.zw - prev.xy;
            }
        }
        if (thick && idx + 1u < total) {
            let next = lsystem_path_cell(xform_id, variation_id, idx + 1u, iters, n);
            if (distance(next.xy, seg_b) < thickness) {
                dir_out = next.zw - next.xy;
            }
        }
    }

    var out = seg_a;
    if (connect) {
        out = mix(seg_a, seg_b, frac);
    }

    // Thicken PERPENDICULAR to the segment: an offset along it would only
    // slide the sample along ground the t-sweep already covers.
    if (thick) {
        let soft = get_param(xform_id, variation_id, 80u) > 0.5;
        let dseg = seg_b - seg_a;
        let dlen = length(dseg);
        let has_seg = connect && dlen > 1e-9;
        let r = thickness;

        if (soft || !has_seg) {
            // A gaussian shaft has no hard edge, so its own tails round
            // the corners; a join there would only pile density on
            // density. Same for vertices-only, which is all corner.
            var jit = (rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0) * 0.5;
            if (!soft) {
                jit = sqrt(rng_nextf(rng));
            }
            if (has_seg) {
                let perp = vec2<f32>(-dseg.y, dseg.x) / dlen;
                out = out + perp * (r * jit);
            } else {
                let ang = rng_nextf(rng) * 6.28318530718;
                out = out + vec2<f32>(cos(ang), sin(ang)) * (r * abs(jit));
            }
        } else {
            // Seamless join. The stroke around a polyline splits exactly
            // into two kinds of piece: each segment's rectangle CLIPPED at
            // the mitre line it shares with its neighbours, and the wedge
            // that the turn opens on the OUTSIDE of each corner. Together
            // they tile it — every point covered once.
            //
            // Getting this wrong is what the earlier attempts looked like:
            // an unclipped rectangle plus a full disc covers the corner
            // twice over (a visible circle), and clipping the rectangle
            // only against the disc still leaves the two rectangles
            // crossing outside it, since the corner square reaches r*sqrt2
            // while the disc stops at r (the faint crescents).
            let v = dseg / dlen;
            let perp = vec2<f32>(-v.y, v.x);

            // The mitre line at a corner has normal u + v: the one
            // direction with a positive component along both segments, so
            // it separates the arriving shaft from the leaving one.
            var n0 = vec2<f32>(0.0, 0.0);
            var phi0 = 0.0;
            let li = length(dir_in);
            if (li > 1e-9) {
                let u = dir_in / li;
                phi0 = acos(clamp(dot(u, v), -1.0, 1.0));
                if (phi0 > 1e-4 && phi0 < 3.1) {
                    n0 = normalize(u + v);
                }
            }
            var n1 = vec2<f32>(0.0, 0.0);
            var phi1 = 0.0;
            let lo = length(dir_out);
            if (lo > 1e-9) {
                let z = dir_out / lo;
                phi1 = acos(clamp(dot(v, z), -1.0, 1.0));
                if (phi1 > 1e-4 && phi1 < 3.1) {
                    n1 = normalize(v + z);
                }
            }

            // Areas, all divided by r. The mitre takes a triangle with
            // legs r and r*tan(phi/2) off each end; the wedge it leaves
            // outside has area (phi/2)*r^2. tan is clamped because a
            // near-reversal would otherwise claim more than the segment.
            let cut0 = 0.5 * r * min(tan(0.5 * phi0), 8.0);
            let cut1 = 0.5 * r * min(tan(0.5 * phi1), 8.0);
            let shaft = max(2.0 * dlen - cut0 - cut1, 0.0);
            let wedge = 0.5 * phi0 * r;

            if (rng_nextf(rng) * (shaft + wedge) < wedge) {
                // Basis (u, e2) with e2 pointing OUTSIDE the turn, so the
                // wedge is beta in [-pi/2, phi0 - pi/2]: from the far edge
                // of the arriving shaft round to the near edge of this one.
                let u = dir_in / li;
                let sgn = select(-1.0, 1.0, (u.x * v.y - u.y * v.x) > 0.0);
                let e2 = sgn * vec2<f32>(-u.y, u.x);
                let beta = -1.5707963268 + phi0 * rng_nextf(rng);
                out = seg_a + (cos(beta) * u + sin(beta) * e2) * (r * sqrt(rng_nextf(rng)));
            } else {
                // Uniform over the rectangle minus whatever the mitres
                // take. Rejection keeps it uniform in AREA — sampling a
                // shortened span directly would not — and the cuts are a
                // percent or two of the rectangle at sensible thicknesses.
                // Falling out of the loop keeps the last sample rather
                // than spinning.
                var s = 0.0;
                var o = 0.0;
                for (var k = 0u; k < 8u; k = k + 1u) {
                    s = rng_nextf(rng) * dlen;
                    o = (rng_nextf(rng) * 2.0 - 1.0) * r;
                    let rel = v * s + perp * o;
                    let in0 = dot(n0, n0) < 0.5 || dot(rel, n0) >= 0.0;
                    let in1 = dot(n1, n1) < 0.5 || dot(rel - dseg, n1) <= 0.0;
                    if (in0 && in1) {
                        break;
                    }
                }
                out = seg_a + v * s + perp * o;
            }
        }
    }

    // Move the whole curve. The path is built in its own frame, so an
    // offset of minus its centre puts the object on the origin — which is
    // what rotation and zoom orbit around.
    out = out + vec2<f32>(
        get_param(xform_id, variation_id, 81u),
        get_param(xform_id, variation_id, 82u));

    if (dc) {
        *vc = t;
    }
    // The path lives in the xy plane; z rides along unchanged.
    return vec3<f32>(out, p.z);
}
"#;
