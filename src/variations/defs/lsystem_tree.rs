//! `lsystem_tree` - finite-depth L-system plant sampler (original).
//!
//! Draws the depth-n DRAWING of a bracketed plant L-system - what the
//! book pictures show - instead of the infinite-depth attractor the
//! plain transforms converge to. The difference matters exactly when
//! the branch system's similarity dimension reaches 2: the limit of
//! `F=F[+F]F[-F][F]` fills the plane solid, while its depth-5 drawing
//! is the sparse bush every reference reproduces.
//!
//! No geometry is stored. A depth-n plant is, level by level: each of
//! the k^l level-l copies (a composition of l branch maps) draws its
//! stem segments; a rule with no stems - every drawn symbol is a
//! recursion site - draws only the bottom generation on the unit
//! segment. Each sample picks a level weighted by the total length
//! living there (rho^l, rho = sum of branch scales), a start point on a
//! length-weighted random stem, and composes that many branch maps
//! picked by their share of rho - so density is uniform over the whole
//! drawing and Iterations stays a live parameter; nothing is re-baked.
//!
//! The maps and stems come from the L-System Plant script's extraction,
//! in the plant's unit-displacement frame.

use crate::variations::{
    definition::{Feature, VariationDef, VariationParamDef},
    ParamType, VariationCategory, VariationPhase,
};
use crate::param;

/// Finite-depth plant drawing: composes up to 12 branch maps down to a
/// chosen level and draws the stem segments living there, so the flame
/// shows the book's depth-n plant rather than the infinite attractor.
/// Written by the L-System Plant script; depth is a live parameter.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5
pub static LSYSTEM_TREE: VariationDef = VariationDef {
    name: "lsystem_tree",
    aliases: &[],
    display_name: "L-System Tree",
    category: VariationCategory::Advanced2D,
    phase: VariationPhase::Normal,
    features: &[Feature::NeedsRng, Feature::WritesColor],
    init_param_count: 0,
    wgsl_init: None,
    state_count: 0,
    wgsl_state_init: None,
    parameters: &[
        param!("iterations", "Iterations", int, 5.0, 1.0, 12.0, "L-system depth: how many generations of branching the drawing shows. A live parameter - nothing is re-baked when it changes."),
        param!("map_count", "Map Count", int, 4.0, 1.0, 12.0, "How many of the twelve branch-map slots are in use. Set by the L-System Plant script."),
        param!("stem_count", "Stem Count", int, 0.0, 0.0, 8.0, "How many stem segments each copy draws. Zero means a leaf rule - every drawn symbol recurses - which draws only the bottom generation."),
        param!("dc", "Direct Color", bool, true, "Color by branching level, trunk at the palette's start and the deepest twigs at its end (leaf rules color by visiting order instead). Needs the transform's Direct Color at 1."),
        param!("m0_a", "M0 A", unlimited_float, 1.0, -4.0, 4.0, "Branch map 0: affine coefficient a (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m0_b", "M0 B", unlimited_float, 0.0, -4.0, 4.0, "Branch map 0: affine coefficient b (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m0_c", "M0 C", unlimited_float, 0.0, -4.0, 4.0, "Branch map 0: affine coefficient c (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m0_d", "M0 D", unlimited_float, 1.0, -4.0, 4.0, "Branch map 0: affine coefficient d (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m0_e", "M0 E", unlimited_float, 0.0, -4.0, 4.0, "Branch map 0: affine coefficient e (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m0_f", "M0 F", unlimited_float, 0.0, -4.0, 4.0, "Branch map 0: affine coefficient f (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m1_a", "M1 A", unlimited_float, 0.0, -4.0, 4.0, "Branch map 1: affine coefficient a (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m1_b", "M1 B", unlimited_float, 0.0, -4.0, 4.0, "Branch map 1: affine coefficient b (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m1_c", "M1 C", unlimited_float, 0.0, -4.0, 4.0, "Branch map 1: affine coefficient c (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m1_d", "M1 D", unlimited_float, 0.0, -4.0, 4.0, "Branch map 1: affine coefficient d (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m1_e", "M1 E", unlimited_float, 0.0, -4.0, 4.0, "Branch map 1: affine coefficient e (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m1_f", "M1 F", unlimited_float, 0.0, -4.0, 4.0, "Branch map 1: affine coefficient f (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m2_a", "M2 A", unlimited_float, 0.0, -4.0, 4.0, "Branch map 2: affine coefficient a (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m2_b", "M2 B", unlimited_float, 0.0, -4.0, 4.0, "Branch map 2: affine coefficient b (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m2_c", "M2 C", unlimited_float, 0.0, -4.0, 4.0, "Branch map 2: affine coefficient c (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m2_d", "M2 D", unlimited_float, 0.0, -4.0, 4.0, "Branch map 2: affine coefficient d (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m2_e", "M2 E", unlimited_float, 0.0, -4.0, 4.0, "Branch map 2: affine coefficient e (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m2_f", "M2 F", unlimited_float, 0.0, -4.0, 4.0, "Branch map 2: affine coefficient f (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m3_a", "M3 A", unlimited_float, 0.0, -4.0, 4.0, "Branch map 3: affine coefficient a (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m3_b", "M3 B", unlimited_float, 0.0, -4.0, 4.0, "Branch map 3: affine coefficient b (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m3_c", "M3 C", unlimited_float, 0.0, -4.0, 4.0, "Branch map 3: affine coefficient c (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m3_d", "M3 D", unlimited_float, 0.0, -4.0, 4.0, "Branch map 3: affine coefficient d (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m3_e", "M3 E", unlimited_float, 0.0, -4.0, 4.0, "Branch map 3: affine coefficient e (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m3_f", "M3 F", unlimited_float, 0.0, -4.0, 4.0, "Branch map 3: affine coefficient f (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m4_a", "M4 A", unlimited_float, 0.0, -4.0, 4.0, "Branch map 4: affine coefficient a (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m4_b", "M4 B", unlimited_float, 0.0, -4.0, 4.0, "Branch map 4: affine coefficient b (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m4_c", "M4 C", unlimited_float, 0.0, -4.0, 4.0, "Branch map 4: affine coefficient c (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m4_d", "M4 D", unlimited_float, 0.0, -4.0, 4.0, "Branch map 4: affine coefficient d (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m4_e", "M4 E", unlimited_float, 0.0, -4.0, 4.0, "Branch map 4: affine coefficient e (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m4_f", "M4 F", unlimited_float, 0.0, -4.0, 4.0, "Branch map 4: affine coefficient f (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m5_a", "M5 A", unlimited_float, 0.0, -4.0, 4.0, "Branch map 5: affine coefficient a (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m5_b", "M5 B", unlimited_float, 0.0, -4.0, 4.0, "Branch map 5: affine coefficient b (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m5_c", "M5 C", unlimited_float, 0.0, -4.0, 4.0, "Branch map 5: affine coefficient c (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m5_d", "M5 D", unlimited_float, 0.0, -4.0, 4.0, "Branch map 5: affine coefficient d (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m5_e", "M5 E", unlimited_float, 0.0, -4.0, 4.0, "Branch map 5: affine coefficient e (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m5_f", "M5 F", unlimited_float, 0.0, -4.0, 4.0, "Branch map 5: affine coefficient f (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m6_a", "M6 A", unlimited_float, 0.0, -4.0, 4.0, "Branch map 6: affine coefficient a (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m6_b", "M6 B", unlimited_float, 0.0, -4.0, 4.0, "Branch map 6: affine coefficient b (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m6_c", "M6 C", unlimited_float, 0.0, -4.0, 4.0, "Branch map 6: affine coefficient c (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m6_d", "M6 D", unlimited_float, 0.0, -4.0, 4.0, "Branch map 6: affine coefficient d (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m6_e", "M6 E", unlimited_float, 0.0, -4.0, 4.0, "Branch map 6: affine coefficient e (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m6_f", "M6 F", unlimited_float, 0.0, -4.0, 4.0, "Branch map 6: affine coefficient f (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m7_a", "M7 A", unlimited_float, 0.0, -4.0, 4.0, "Branch map 7: affine coefficient a (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m7_b", "M7 B", unlimited_float, 0.0, -4.0, 4.0, "Branch map 7: affine coefficient b (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m7_c", "M7 C", unlimited_float, 0.0, -4.0, 4.0, "Branch map 7: affine coefficient c (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m7_d", "M7 D", unlimited_float, 0.0, -4.0, 4.0, "Branch map 7: affine coefficient d (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m7_e", "M7 E", unlimited_float, 0.0, -4.0, 4.0, "Branch map 7: affine coefficient e (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m7_f", "M7 F", unlimited_float, 0.0, -4.0, 4.0, "Branch map 7: affine coefficient f (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m8_a", "M8 A", unlimited_float, 0.0, -4.0, 4.0, "Branch map 8: affine coefficient a (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m8_b", "M8 B", unlimited_float, 0.0, -4.0, 4.0, "Branch map 8: affine coefficient b (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m8_c", "M8 C", unlimited_float, 0.0, -4.0, 4.0, "Branch map 8: affine coefficient c (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m8_d", "M8 D", unlimited_float, 0.0, -4.0, 4.0, "Branch map 8: affine coefficient d (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m8_e", "M8 E", unlimited_float, 0.0, -4.0, 4.0, "Branch map 8: affine coefficient e (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m8_f", "M8 F", unlimited_float, 0.0, -4.0, 4.0, "Branch map 8: affine coefficient f (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m9_a", "M9 A", unlimited_float, 0.0, -4.0, 4.0, "Branch map 9: affine coefficient a (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m9_b", "M9 B", unlimited_float, 0.0, -4.0, 4.0, "Branch map 9: affine coefficient b (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m9_c", "M9 C", unlimited_float, 0.0, -4.0, 4.0, "Branch map 9: affine coefficient c (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m9_d", "M9 D", unlimited_float, 0.0, -4.0, 4.0, "Branch map 9: affine coefficient d (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m9_e", "M9 E", unlimited_float, 0.0, -4.0, 4.0, "Branch map 9: affine coefficient e (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m9_f", "M9 F", unlimited_float, 0.0, -4.0, 4.0, "Branch map 9: affine coefficient f (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m10_a", "M10 A", unlimited_float, 0.0, -4.0, 4.0, "Branch map 10: affine coefficient a (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m10_b", "M10 B", unlimited_float, 0.0, -4.0, 4.0, "Branch map 10: affine coefficient b (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m10_c", "M10 C", unlimited_float, 0.0, -4.0, 4.0, "Branch map 10: affine coefficient c (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m10_d", "M10 D", unlimited_float, 0.0, -4.0, 4.0, "Branch map 10: affine coefficient d (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m10_e", "M10 E", unlimited_float, 0.0, -4.0, 4.0, "Branch map 10: affine coefficient e (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m10_f", "M10 F", unlimited_float, 0.0, -4.0, 4.0, "Branch map 10: affine coefficient f (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m11_a", "M11 A", unlimited_float, 0.0, -4.0, 4.0, "Branch map 11: affine coefficient a (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m11_b", "M11 B", unlimited_float, 0.0, -4.0, 4.0, "Branch map 11: affine coefficient b (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m11_c", "M11 C", unlimited_float, 0.0, -4.0, 4.0, "Branch map 11: affine coefficient c (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m11_d", "M11 D", unlimited_float, 0.0, -4.0, 4.0, "Branch map 11: affine coefficient d (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m11_e", "M11 E", unlimited_float, 0.0, -4.0, 4.0, "Branch map 11: affine coefficient e (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("m11_f", "M11 F", unlimited_float, 0.0, -4.0, 4.0, "Branch map 11: affine coefficient f (x' = a\u{b7}x + b\u{b7}y + e, y' = c\u{b7}x + d\u{b7}y + f). Normally written by the L-System Plant script, not by hand."),
        param!("s0_x1", "S0 X1", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 0: start x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s0_y1", "S0 Y1", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 0: start y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s0_x2", "S0 X2", unlimited_float, 1.0, -4.0, 4.0, "Stem segment 0: end x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s0_y2", "S0 Y2", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 0: end y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s1_x1", "S1 X1", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 1: start x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s1_y1", "S1 Y1", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 1: start y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s1_x2", "S1 X2", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 1: end x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s1_y2", "S1 Y2", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 1: end y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s2_x1", "S2 X1", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 2: start x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s2_y1", "S2 Y1", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 2: start y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s2_x2", "S2 X2", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 2: end x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s2_y2", "S2 Y2", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 2: end y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s3_x1", "S3 X1", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 3: start x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s3_y1", "S3 Y1", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 3: start y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s3_x2", "S3 X2", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 3: end x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s3_y2", "S3 Y2", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 3: end y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s4_x1", "S4 X1", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 4: start x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s4_y1", "S4 Y1", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 4: start y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s4_x2", "S4 X2", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 4: end x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s4_y2", "S4 Y2", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 4: end y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s5_x1", "S5 X1", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 5: start x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s5_y1", "S5 Y1", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 5: start y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s5_x2", "S5 X2", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 5: end x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s5_y2", "S5 Y2", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 5: end y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s6_x1", "S6 X1", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 6: start x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s6_y1", "S6 Y1", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 6: start y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s6_x2", "S6 X2", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 6: end x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s6_y2", "S6 Y2", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 6: end y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s7_x1", "S7 X1", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 7: start x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s7_y1", "S7 Y1", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 7: start y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s7_x2", "S7 X2", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 7: end x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("s7_y2", "S7 Y2", unlimited_float, 0.0, -4.0, 4.0, "Stem segment 7: end y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand."),
        param!("thickness", "Thickness", float, 0.0, 0.0, 0.2, "Half-width of the drawn stroke at the trunk, in the plant's own units. Twigs thin down automatically with their scale. The same samples spread over more area, so a thick stroke is dimmer - raise Brightness to match."),
        param!("soft", "Soft Edges", bool, false, "Gaussian falloff across the width instead of a flat stroke. Flat reads as a drawn line; soft reads as a glow."),
        param!("offset_x", "Offset X", unlimited_float, 0.0, -2.0, 2.0, "Move the whole plant along x, in its own units. The script sets this to centre the plant on the origin, which rotation and zoom orbit around."),
        param!("offset_y", "Offset Y", unlimited_float, 0.0, -2.0, 2.0, "Move the whole plant along y."),
    ],
    wgsl_2d: WGSL_2D,
    wgsl_3d: WGSL_3D,
};

const WGSL_2D: &str = r#"
fn variation_lsystem_tree(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec2<f32> {
    let iters = clamp(u32(get_param(xform_id, variation_id, 0u)), 1u, 12u);
    let k = clamp(u32(get_param(xform_id, variation_id, 1u)), 1u, 12u);
    let s = min(u32(get_param(xform_id, variation_id, 2u)), 8u);
    let dc = get_param(xform_id, variation_id, 3u) > 0.5;
    let thickness = get_param(xform_id, variation_id, 108u);
    let soft = get_param(xform_id, variation_id, 109u) > 0.5;

    // Per-branch contraction (length of the heading column) and the sum
    // rho: the factor by which the total drawn length grows per level.
    var sig: array<f32, 12>;
    var rho = 0.0;
    for (var i = 0u; i < k; i = i + 1u) {
        let base = 4u + i * 6u;
        let ma = get_param(xform_id, variation_id, base);
        let mc = get_param(xform_id, variation_id, base + 2u);
        sig[i] = sqrt(ma * ma + mc * mc);
        rho = rho + sig[i];
    }

    // Level. A rule with stems draws them at every level 0..iters-1,
    // weighted by the total length living there (rho^l); a leaf rule
    // (no stems - every drawn symbol is a recursion site) draws only
    // the bottom generation.
    var level = iters;
    if (s > 0u) {
        var total = 0.0;
        var w = 1.0;
        for (var l = 0u; l < iters; l = l + 1u) {
            total = total + w;
            w = w * rho;
        }
        var u = rng_nextf(rng) * total;
        level = iters - 1u;
        w = 1.0;
        for (var l = 0u; l < iters; l = l + 1u) {
            if (u < w) {
                level = l;
                break;
            }
            u = u - w;
            w = w * rho;
        }
    }

    // Start point: a random point of a random stem segment, length-
    // weighted so density is uniform along the drawing. Leaf rules use
    // the unit segment the frame spans.
    var pa = vec2<f32>(0.0, 0.0);
    var pb = vec2<f32>(1.0, 0.0);
    if (s > 0u) {
        var slen = 0.0;
        for (var j = 0u; j < s; j = j + 1u) {
            let sb = 76u + j * 4u;
            let dxj = get_param(xform_id, variation_id, sb + 2u) - get_param(xform_id, variation_id, sb);
            let dyj = get_param(xform_id, variation_id, sb + 3u) - get_param(xform_id, variation_id, sb + 1u);
            slen = slen + sqrt(dxj * dxj + dyj * dyj);
        }
        var us = rng_nextf(rng) * max(slen, 1e-9);
        var pick = s - 1u;
        for (var j = 0u; j < s; j = j + 1u) {
            let sb = 76u + j * 4u;
            let dxj = get_param(xform_id, variation_id, sb + 2u) - get_param(xform_id, variation_id, sb);
            let dyj = get_param(xform_id, variation_id, sb + 3u) - get_param(xform_id, variation_id, sb + 1u);
            let lj = sqrt(dxj * dxj + dyj * dyj);
            if (us < lj) {
                pick = j;
                break;
            }
            us = us - lj;
        }
        let sb = 76u + pick * 4u;
        pa = vec2<f32>(get_param(xform_id, variation_id, sb), get_param(xform_id, variation_id, sb + 1u));
        pb = vec2<f32>(get_param(xform_id, variation_id, sb + 2u), get_param(xform_id, variation_id, sb + 3u));
    }
    var out = mix(pa, pb, rng_nextf(rng));

    // Thickness perpendicular to the segment, in ITS frame, before the
    // branch maps go on - so twigs thin down with their scale and the
    // trunk stays the widest stroke.
    if (thickness > 0.0) {
        let dseg = pb - pa;
        let dl = length(dseg);
        if (dl > 1e-9) {
            let perp = vec2<f32>(-dseg.y, dseg.x) / dl;
            var jit = rng_nextf(rng) * 2.0 - 1.0;
            if (soft) {
                jit = (rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0) * 0.5;
            }
            out = out + perp * (thickness * jit);
        }
    }

    // Compose `level` branch maps, each picked with probability
    // sig/rho - its share of the length - so every copy gets samples
    // proportional to what it draws. tcol tracks the visiting-order
    // fraction (outermost digit most significant) for leaf colouring.
    var tcol = 0.0;
    for (var j2 = 0u; j2 < level; j2 = j2 + 1u) {
        var u2 = rng_nextf(rng) * rho;
        var i2 = k - 1u;
        for (var q = 0u; q < k; q = q + 1u) {
            if (u2 < sig[q]) {
                i2 = q;
                break;
            }
            u2 = u2 - sig[q];
        }
        let base = 4u + i2 * 6u;
        let ma = get_param(xform_id, variation_id, base);
        let mb = get_param(xform_id, variation_id, base + 1u);
        let mc = get_param(xform_id, variation_id, base + 2u);
        let md = get_param(xform_id, variation_id, base + 3u);
        let me = get_param(xform_id, variation_id, base + 4u);
        let mf = get_param(xform_id, variation_id, base + 5u);
        out = vec2<f32>(ma * out.x + mb * out.y + me, mc * out.x + md * out.y + mf);
        tcol = (tcol + f32(i2)) / f32(k);
    }

    out = out + vec2<f32>(
        get_param(xform_id, variation_id, 110u),
        get_param(xform_id, variation_id, 111u));

    if (dc) {
        if (s > 0u) {
            *vc = f32(level) / f32(iters);
        } else {
            *vc = tcol;
        }
    }
    return out;
}
"#;

const WGSL_3D: &str = r#"
fn variation_lsystem_tree(p: vec3<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>, vc: ptr<function, f32>) -> vec3<f32> {
    let iters = clamp(u32(get_param(xform_id, variation_id, 0u)), 1u, 12u);
    let k = clamp(u32(get_param(xform_id, variation_id, 1u)), 1u, 12u);
    let s = min(u32(get_param(xform_id, variation_id, 2u)), 8u);
    let dc = get_param(xform_id, variation_id, 3u) > 0.5;
    let thickness = get_param(xform_id, variation_id, 108u);
    let soft = get_param(xform_id, variation_id, 109u) > 0.5;

    // Per-branch contraction (length of the heading column) and the sum
    // rho: the factor by which the total drawn length grows per level.
    var sig: array<f32, 12>;
    var rho = 0.0;
    for (var i = 0u; i < k; i = i + 1u) {
        let base = 4u + i * 6u;
        let ma = get_param(xform_id, variation_id, base);
        let mc = get_param(xform_id, variation_id, base + 2u);
        sig[i] = sqrt(ma * ma + mc * mc);
        rho = rho + sig[i];
    }

    // Level. A rule with stems draws them at every level 0..iters-1,
    // weighted by the total length living there (rho^l); a leaf rule
    // (no stems - every drawn symbol is a recursion site) draws only
    // the bottom generation.
    var level = iters;
    if (s > 0u) {
        var total = 0.0;
        var w = 1.0;
        for (var l = 0u; l < iters; l = l + 1u) {
            total = total + w;
            w = w * rho;
        }
        var u = rng_nextf(rng) * total;
        level = iters - 1u;
        w = 1.0;
        for (var l = 0u; l < iters; l = l + 1u) {
            if (u < w) {
                level = l;
                break;
            }
            u = u - w;
            w = w * rho;
        }
    }

    // Start point: a random point of a random stem segment, length-
    // weighted so density is uniform along the drawing. Leaf rules use
    // the unit segment the frame spans.
    var pa = vec2<f32>(0.0, 0.0);
    var pb = vec2<f32>(1.0, 0.0);
    if (s > 0u) {
        var slen = 0.0;
        for (var j = 0u; j < s; j = j + 1u) {
            let sb = 76u + j * 4u;
            let dxj = get_param(xform_id, variation_id, sb + 2u) - get_param(xform_id, variation_id, sb);
            let dyj = get_param(xform_id, variation_id, sb + 3u) - get_param(xform_id, variation_id, sb + 1u);
            slen = slen + sqrt(dxj * dxj + dyj * dyj);
        }
        var us = rng_nextf(rng) * max(slen, 1e-9);
        var pick = s - 1u;
        for (var j = 0u; j < s; j = j + 1u) {
            let sb = 76u + j * 4u;
            let dxj = get_param(xform_id, variation_id, sb + 2u) - get_param(xform_id, variation_id, sb);
            let dyj = get_param(xform_id, variation_id, sb + 3u) - get_param(xform_id, variation_id, sb + 1u);
            let lj = sqrt(dxj * dxj + dyj * dyj);
            if (us < lj) {
                pick = j;
                break;
            }
            us = us - lj;
        }
        let sb = 76u + pick * 4u;
        pa = vec2<f32>(get_param(xform_id, variation_id, sb), get_param(xform_id, variation_id, sb + 1u));
        pb = vec2<f32>(get_param(xform_id, variation_id, sb + 2u), get_param(xform_id, variation_id, sb + 3u));
    }
    var out = mix(pa, pb, rng_nextf(rng));

    // Thickness perpendicular to the segment, in ITS frame, before the
    // branch maps go on - so twigs thin down with their scale and the
    // trunk stays the widest stroke.
    if (thickness > 0.0) {
        let dseg = pb - pa;
        let dl = length(dseg);
        if (dl > 1e-9) {
            let perp = vec2<f32>(-dseg.y, dseg.x) / dl;
            var jit = rng_nextf(rng) * 2.0 - 1.0;
            if (soft) {
                jit = (rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) + rng_nextf(rng) - 2.0) * 0.5;
            }
            out = out + perp * (thickness * jit);
        }
    }

    // Compose `level` branch maps, each picked with probability
    // sig/rho - its share of the length - so every copy gets samples
    // proportional to what it draws. tcol tracks the visiting-order
    // fraction (outermost digit most significant) for leaf colouring.
    var tcol = 0.0;
    for (var j2 = 0u; j2 < level; j2 = j2 + 1u) {
        var u2 = rng_nextf(rng) * rho;
        var i2 = k - 1u;
        for (var q = 0u; q < k; q = q + 1u) {
            if (u2 < sig[q]) {
                i2 = q;
                break;
            }
            u2 = u2 - sig[q];
        }
        let base = 4u + i2 * 6u;
        let ma = get_param(xform_id, variation_id, base);
        let mb = get_param(xform_id, variation_id, base + 1u);
        let mc = get_param(xform_id, variation_id, base + 2u);
        let md = get_param(xform_id, variation_id, base + 3u);
        let me = get_param(xform_id, variation_id, base + 4u);
        let mf = get_param(xform_id, variation_id, base + 5u);
        out = vec2<f32>(ma * out.x + mb * out.y + me, mc * out.x + md * out.y + mf);
        tcol = (tcol + f32(i2)) / f32(k);
    }

    out = out + vec2<f32>(
        get_param(xform_id, variation_id, 110u),
        get_param(xform_id, variation_id, 111u));

    if (dc) {
        if (s > 0u) {
            *vc = f32(level) / f32(iters);
        } else {
            *vc = tcol;
        }
    }
    // The drawing lives in the xy plane; z rides along unchanged.
    return vec3<f32>(out, p.z);
}
"#;
