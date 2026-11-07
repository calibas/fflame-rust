/// Finite-depth plant drawing: composes up to 12 branch maps down to a
/// chosen level and draws the stem segments living there, so the flame
/// shows the book's depth-n plant rather than the infinite attractor.
/// Written by the L-System Plant script; depth is a live parameter.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// L-System Tree, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    iterations: #{
        value: 5, min: 1, max: 12,
        caption: "Iterations",
        hint: "L-system depth: how many generations of branching the drawing shows. A live parameter - nothing is re-baked when it changes.",
    },
    map_count: #{
        value: 4, min: 1, max: 12,
        caption: "Map Count",
        hint: "How many of the twelve branch-map slots are in use. Set by the L-System Plant script.",
    },
    stem_count: #{
        value: 0, min: 0, max: 8,
        caption: "Stem Count",
        hint: "How many stem segments each copy draws. Zero means a leaf rule - every drawn symbol recurses - which draws only the bottom generation.",
    },
    dc: #{
        value: true,
        caption: "Direct Color",
        hint: "Color by branching level, trunk at the palette's start and the deepest twigs at its end (leaf rules color by visiting order instead). Needs the transform's Direct Color at 1.",
    },
    m0_a: #{
        value: 1.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 A",
        hint: "Branch map 0: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m0_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 B",
        hint: "Branch map 0: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m0_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 C",
        hint: "Branch map 0: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m0_d: #{
        value: 1.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 D",
        hint: "Branch map 0: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m0_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 E",
        hint: "Branch map 0: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m0_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 F",
        hint: "Branch map 0: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m1_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 A",
        hint: "Branch map 1: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m1_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 B",
        hint: "Branch map 1: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m1_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 C",
        hint: "Branch map 1: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m1_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 D",
        hint: "Branch map 1: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m1_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 E",
        hint: "Branch map 1: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m1_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 F",
        hint: "Branch map 1: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m2_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 A",
        hint: "Branch map 2: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m2_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 B",
        hint: "Branch map 2: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m2_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 C",
        hint: "Branch map 2: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m2_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 D",
        hint: "Branch map 2: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m2_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 E",
        hint: "Branch map 2: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m2_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 F",
        hint: "Branch map 2: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m3_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 A",
        hint: "Branch map 3: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m3_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 B",
        hint: "Branch map 3: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m3_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 C",
        hint: "Branch map 3: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m3_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 D",
        hint: "Branch map 3: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m3_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 E",
        hint: "Branch map 3: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m3_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 F",
        hint: "Branch map 3: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m4_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 A",
        hint: "Branch map 4: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m4_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 B",
        hint: "Branch map 4: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m4_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 C",
        hint: "Branch map 4: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m4_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 D",
        hint: "Branch map 4: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m4_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 E",
        hint: "Branch map 4: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m4_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 F",
        hint: "Branch map 4: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m5_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 A",
        hint: "Branch map 5: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m5_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 B",
        hint: "Branch map 5: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m5_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 C",
        hint: "Branch map 5: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m5_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 D",
        hint: "Branch map 5: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m5_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 E",
        hint: "Branch map 5: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m5_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 F",
        hint: "Branch map 5: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m6_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 A",
        hint: "Branch map 6: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m6_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 B",
        hint: "Branch map 6: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m6_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 C",
        hint: "Branch map 6: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m6_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 D",
        hint: "Branch map 6: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m6_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 E",
        hint: "Branch map 6: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m6_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 F",
        hint: "Branch map 6: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m7_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 A",
        hint: "Branch map 7: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m7_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 B",
        hint: "Branch map 7: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m7_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 C",
        hint: "Branch map 7: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m7_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 D",
        hint: "Branch map 7: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m7_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 E",
        hint: "Branch map 7: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m7_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 F",
        hint: "Branch map 7: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m8_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 A",
        hint: "Branch map 8: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m8_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 B",
        hint: "Branch map 8: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m8_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 C",
        hint: "Branch map 8: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m8_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 D",
        hint: "Branch map 8: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m8_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 E",
        hint: "Branch map 8: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m8_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 F",
        hint: "Branch map 8: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m9_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 A",
        hint: "Branch map 9: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m9_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 B",
        hint: "Branch map 9: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m9_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 C",
        hint: "Branch map 9: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m9_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 D",
        hint: "Branch map 9: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m9_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 E",
        hint: "Branch map 9: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m9_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 F",
        hint: "Branch map 9: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m10_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 A",
        hint: "Branch map 10: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m10_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 B",
        hint: "Branch map 10: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m10_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 C",
        hint: "Branch map 10: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m10_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 D",
        hint: "Branch map 10: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m10_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 E",
        hint: "Branch map 10: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m10_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 F",
        hint: "Branch map 10: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m11_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 A",
        hint: "Branch map 11: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m11_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 B",
        hint: "Branch map 11: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m11_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 C",
        hint: "Branch map 11: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m11_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 D",
        hint: "Branch map 11: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m11_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 E",
        hint: "Branch map 11: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    m11_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 F",
        hint: "Branch map 11: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System Plant script, not by hand.",
    },
    s0_x1: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S0 X1",
        hint: "Stem segment 0: start x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s0_y1: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S0 Y1",
        hint: "Stem segment 0: start y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s0_x2: #{
        value: 1.0, min: -4.0, max: 4.0, free: true,
        caption: "S0 X2",
        hint: "Stem segment 0: end x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s0_y2: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S0 Y2",
        hint: "Stem segment 0: end y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s1_x1: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S1 X1",
        hint: "Stem segment 1: start x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s1_y1: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S1 Y1",
        hint: "Stem segment 1: start y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s1_x2: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S1 X2",
        hint: "Stem segment 1: end x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s1_y2: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S1 Y2",
        hint: "Stem segment 1: end y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s2_x1: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S2 X1",
        hint: "Stem segment 2: start x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s2_y1: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S2 Y1",
        hint: "Stem segment 2: start y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s2_x2: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S2 X2",
        hint: "Stem segment 2: end x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s2_y2: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S2 Y2",
        hint: "Stem segment 2: end y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s3_x1: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S3 X1",
        hint: "Stem segment 3: start x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s3_y1: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S3 Y1",
        hint: "Stem segment 3: start y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s3_x2: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S3 X2",
        hint: "Stem segment 3: end x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s3_y2: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S3 Y2",
        hint: "Stem segment 3: end y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s4_x1: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S4 X1",
        hint: "Stem segment 4: start x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s4_y1: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S4 Y1",
        hint: "Stem segment 4: start y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s4_x2: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S4 X2",
        hint: "Stem segment 4: end x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s4_y2: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S4 Y2",
        hint: "Stem segment 4: end y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s5_x1: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S5 X1",
        hint: "Stem segment 5: start x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s5_y1: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S5 Y1",
        hint: "Stem segment 5: start y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s5_x2: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S5 X2",
        hint: "Stem segment 5: end x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s5_y2: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S5 Y2",
        hint: "Stem segment 5: end y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s6_x1: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S6 X1",
        hint: "Stem segment 6: start x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s6_y1: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S6 Y1",
        hint: "Stem segment 6: start y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s6_x2: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S6 X2",
        hint: "Stem segment 6: end x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s6_y2: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S6 Y2",
        hint: "Stem segment 6: end y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s7_x1: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S7 X1",
        hint: "Stem segment 7: start x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s7_y1: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S7 Y1",
        hint: "Stem segment 7: start y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s7_x2: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S7 X2",
        hint: "Stem segment 7: end x, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    s7_y2: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "S7 Y2",
        hint: "Stem segment 7: end y, in the plant's unit-displacement frame. Normally written by the L-System Plant script, not by hand.",
    },
    thickness: #{
        value: 0.0, min: 0.0, max: 0.2,
        caption: "Thickness",
        hint: "Half-width of the drawn stroke at the trunk, in the plant's own units. Twigs thin down automatically with their scale. The same samples spread over more area, so a thick stroke is dimmer - raise Brightness to match.",
    },
    soft: #{
        value: false,
        caption: "Soft Edges",
        hint: "Gaussian falloff across the width instead of a flat stroke. Flat reads as a drawn line; soft reads as a glow.",
    },
    offset_x: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Offset X",
        hint: "Move the whole plant along x, in its own units. The script sets this to centre the plant on the origin, which rotation and zoom orbit around.",
    },
    offset_y: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Offset Y",
        hint: "Move the whole plant along y.",
    },
};

const INFO = #{ title: "L-System Tree", name: "rhai_lsystem_tree", category: "advanced_2d", z: "preserve_z" };

fn prepare(p) {
    let out = array(112, 0.0);
    out[0] = to_float(p.iterations);
    out[1] = to_float(p.map_count);
    out[2] = to_float(p.stem_count);
    out[3] = to_float(p.dc);
    out[4] = p.m0_a;
    out[5] = p.m0_b;
    out[6] = p.m0_c;
    out[7] = p.m0_d;
    out[8] = p.m0_e;
    out[9] = p.m0_f;
    out[10] = p.m1_a;
    out[11] = p.m1_b;
    out[12] = p.m1_c;
    out[13] = p.m1_d;
    out[14] = p.m1_e;
    out[15] = p.m1_f;
    out[16] = p.m2_a;
    out[17] = p.m2_b;
    out[18] = p.m2_c;
    out[19] = p.m2_d;
    out[20] = p.m2_e;
    out[21] = p.m2_f;
    out[22] = p.m3_a;
    out[23] = p.m3_b;
    out[24] = p.m3_c;
    out[25] = p.m3_d;
    out[26] = p.m3_e;
    out[27] = p.m3_f;
    out[28] = p.m4_a;
    out[29] = p.m4_b;
    out[30] = p.m4_c;
    out[31] = p.m4_d;
    out[32] = p.m4_e;
    out[33] = p.m4_f;
    out[34] = p.m5_a;
    out[35] = p.m5_b;
    out[36] = p.m5_c;
    out[37] = p.m5_d;
    out[38] = p.m5_e;
    out[39] = p.m5_f;
    out[40] = p.m6_a;
    out[41] = p.m6_b;
    out[42] = p.m6_c;
    out[43] = p.m6_d;
    out[44] = p.m6_e;
    out[45] = p.m6_f;
    out[46] = p.m7_a;
    out[47] = p.m7_b;
    out[48] = p.m7_c;
    out[49] = p.m7_d;
    out[50] = p.m7_e;
    out[51] = p.m7_f;
    out[52] = p.m8_a;
    out[53] = p.m8_b;
    out[54] = p.m8_c;
    out[55] = p.m8_d;
    out[56] = p.m8_e;
    out[57] = p.m8_f;
    out[58] = p.m9_a;
    out[59] = p.m9_b;
    out[60] = p.m9_c;
    out[61] = p.m9_d;
    out[62] = p.m9_e;
    out[63] = p.m9_f;
    out[64] = p.m10_a;
    out[65] = p.m10_b;
    out[66] = p.m10_c;
    out[67] = p.m10_d;
    out[68] = p.m10_e;
    out[69] = p.m10_f;
    out[70] = p.m11_a;
    out[71] = p.m11_b;
    out[72] = p.m11_c;
    out[73] = p.m11_d;
    out[74] = p.m11_e;
    out[75] = p.m11_f;
    out[76] = p.s0_x1;
    out[77] = p.s0_y1;
    out[78] = p.s0_x2;
    out[79] = p.s0_y2;
    out[80] = p.s1_x1;
    out[81] = p.s1_y1;
    out[82] = p.s1_x2;
    out[83] = p.s1_y2;
    out[84] = p.s2_x1;
    out[85] = p.s2_y1;
    out[86] = p.s2_x2;
    out[87] = p.s2_y2;
    out[88] = p.s3_x1;
    out[89] = p.s3_y1;
    out[90] = p.s3_x2;
    out[91] = p.s3_y2;
    out[92] = p.s4_x1;
    out[93] = p.s4_y1;
    out[94] = p.s4_x2;
    out[95] = p.s4_y2;
    out[96] = p.s5_x1;
    out[97] = p.s5_y1;
    out[98] = p.s5_x2;
    out[99] = p.s5_y2;
    out[100] = p.s6_x1;
    out[101] = p.s6_y1;
    out[102] = p.s6_x2;
    out[103] = p.s6_y2;
    out[104] = p.s7_x1;
    out[105] = p.s7_y1;
    out[106] = p.s7_x2;
    out[107] = p.s7_y2;
    out[108] = p.thickness;
    out[109] = to_float(p.soft);
    out[110] = p.offset_x;
    out[111] = p.offset_y;
    #{ slots: out }
}

fn variation(z, p, ctx) {
    let color = ctx.color;
    let iters = clamp(u32(to_float(p.iterations)), u32(1), u32(12));
    let k = clamp(u32(to_float(p.map_count)), u32(1), u32(12));
    let s = min(u32(to_float(p.stem_count)), u32(8));
    let dc = to_float(p.dc) > 0.5;
    let thickness = p.thickness;
    let soft = to_float(p.soft) > 0.5;
    let sig = array(12, 0.0);
    let rho = 0.0;
    for i_n in 0..to_int(k) {
        let i = u32(i_n);
        let base = u32(4) + i * u32(6);
        let ma = p.slots[to_int(base)];
        let mc = p.slots[to_int(base + u32(2))];
        sig[i] = sqrt(ma * ma + mc * mc);
        rho = rho + sig[i];
    }
    let level = iters;
    if (s > u32(0)) {
        let total = 0.0;
        let w = 1.0;
        for l_n in 0..to_int(iters) {
            let l = u32(l_n);
            total = total + w;
            w = w * rho;
        }
        let u = rand() * total;
        level = iters - u32(1);
        w = 1.0;
        for l_n in 0..to_int(iters) {
            let l = u32(l_n);
            if (u < w) {
                level = l;
                break;
            }
            u = u - w;
            w = w * rho;
        }
    }
    let pa = complex(0.0, 0.0);
    let pb = complex(1.0, 0.0);
    if (s > u32(0)) {
        let slen = 0.0;
        for j_n in 0..to_int(s) {
            let j = u32(j_n);
            let sb = u32(76) + j * u32(4);
            let dxj = p.slots[to_int(sb + u32(2))] - p.slots[to_int(sb)];
            let dyj = p.slots[to_int(sb + u32(3))] - p.slots[to_int(sb + u32(1))];
            slen = slen + sqrt(dxj * dxj + dyj * dyj);
        }
        let us = rand() * max(1.0e-9, slen);
        let pick = s - u32(1);
        for j_n in 0..to_int(s) {
            let j = u32(j_n);
            let sb = u32(76) + j * u32(4);
            let dxj = p.slots[to_int(sb + u32(2))] - p.slots[to_int(sb)];
            let dyj = p.slots[to_int(sb + u32(3))] - p.slots[to_int(sb + u32(1))];
            let lj = sqrt(dxj * dxj + dyj * dyj);
            if (us < lj) {
                pick = j;
                break;
            }
            us = us - lj;
        }
        let sb = u32(76) + pick * u32(4);
        pa = complex(p.slots[to_int(sb)], p.slots[to_int(sb + u32(1))]);
        pb = complex(p.slots[to_int(sb + u32(2))], p.slots[to_int(sb + u32(3))]);
    }
    let out = mix(pa, pb, rand());
    if (thickness > 0.0) {
        let dseg = pb - pa;
        let dl = dseg.abs();
        if (dl > 1.0e-9) {
            let perp = complex(-dseg.im, dseg.re) / dl;
            let jit = rand() * 2.0 - 1.0;
            if (soft) {
                jit = (rand() + rand() + rand() + rand() - 2.0) * 0.5;
            }
            out = out + perp * (thickness * jit);
        }
    }
    let tcol = 0.0;
    for j2_n in 0..to_int(level) {
        let j2 = u32(j2_n);
        let u2 = rand() * rho;
        let i2 = k - u32(1);
        for q_n in 0..to_int(k) {
            let q = u32(q_n);
            if (u2 < sig[q]) {
                i2 = q;
                break;
            }
            u2 = u2 - sig[q];
        }
        let base = u32(4) + i2 * u32(6);
        let ma = p.slots[to_int(base)];
        let mb = p.slots[to_int(base + u32(1))];
        let mc = p.slots[to_int(base + u32(2))];
        let md = p.slots[to_int(base + u32(3))];
        let me = p.slots[to_int(base + u32(4))];
        let mf = p.slots[to_int(base + u32(5))];
        out = complex(ma * out.re + mb * out.im + me, mc * out.re + md * out.im + mf);
        tcol = (tcol + to_float(i2)) / to_float(k);
    }
    out = out + complex(p.offset_x, p.offset_y);
    if (dc) {
        if (s > u32(0)) {
            color = to_float(level) / to_float(iters);
        } else {
            color = tcol;
        }
    }
    #{ z: out, color: color }
}

fn variation3(v, p, ctx) {
    let color = ctx.color;
    let iters = clamp(u32(to_float(p.iterations)), u32(1), u32(12));
    let k = clamp(u32(to_float(p.map_count)), u32(1), u32(12));
    let s = min(u32(to_float(p.stem_count)), u32(8));
    let dc = to_float(p.dc) > 0.5;
    let thickness = p.thickness;
    let soft = to_float(p.soft) > 0.5;
    let sig = array(12, 0.0);
    let rho = 0.0;
    for i_n in 0..to_int(k) {
        let i = u32(i_n);
        let base = u32(4) + i * u32(6);
        let ma = p.slots[to_int(base)];
        let mc = p.slots[to_int(base + u32(2))];
        sig[i] = sqrt(ma * ma + mc * mc);
        rho = rho + sig[i];
    }
    let level = iters;
    if (s > u32(0)) {
        let total = 0.0;
        let w = 1.0;
        for l_n in 0..to_int(iters) {
            let l = u32(l_n);
            total = total + w;
            w = w * rho;
        }
        let u = rand() * total;
        level = iters - u32(1);
        w = 1.0;
        for l_n in 0..to_int(iters) {
            let l = u32(l_n);
            if (u < w) {
                level = l;
                break;
            }
            u = u - w;
            w = w * rho;
        }
    }
    let pa = complex(0.0, 0.0);
    let pb = complex(1.0, 0.0);
    if (s > u32(0)) {
        let slen = 0.0;
        for j_n in 0..to_int(s) {
            let j = u32(j_n);
            let sb = u32(76) + j * u32(4);
            let dxj = p.slots[to_int(sb + u32(2))] - p.slots[to_int(sb)];
            let dyj = p.slots[to_int(sb + u32(3))] - p.slots[to_int(sb + u32(1))];
            slen = slen + sqrt(dxj * dxj + dyj * dyj);
        }
        let us = rand() * max(1.0e-9, slen);
        let pick = s - u32(1);
        for j_n in 0..to_int(s) {
            let j = u32(j_n);
            let sb = u32(76) + j * u32(4);
            let dxj = p.slots[to_int(sb + u32(2))] - p.slots[to_int(sb)];
            let dyj = p.slots[to_int(sb + u32(3))] - p.slots[to_int(sb + u32(1))];
            let lj = sqrt(dxj * dxj + dyj * dyj);
            if (us < lj) {
                pick = j;
                break;
            }
            us = us - lj;
        }
        let sb = u32(76) + pick * u32(4);
        pa = complex(p.slots[to_int(sb)], p.slots[to_int(sb + u32(1))]);
        pb = complex(p.slots[to_int(sb + u32(2))], p.slots[to_int(sb + u32(3))]);
    }
    let out = mix(pa, pb, rand());
    if (thickness > 0.0) {
        let dseg = pb - pa;
        let dl = dseg.abs();
        if (dl > 1.0e-9) {
            let perp = complex(-dseg.im, dseg.re) / dl;
            let jit = rand() * 2.0 - 1.0;
            if (soft) {
                jit = (rand() + rand() + rand() + rand() - 2.0) * 0.5;
            }
            out = out + perp * (thickness * jit);
        }
    }
    let tcol = 0.0;
    for j2_n in 0..to_int(level) {
        let j2 = u32(j2_n);
        let u2 = rand() * rho;
        let i2 = k - u32(1);
        for q_n in 0..to_int(k) {
            let q = u32(q_n);
            if (u2 < sig[q]) {
                i2 = q;
                break;
            }
            u2 = u2 - sig[q];
        }
        let base = u32(4) + i2 * u32(6);
        let ma = p.slots[to_int(base)];
        let mb = p.slots[to_int(base + u32(1))];
        let mc = p.slots[to_int(base + u32(2))];
        let md = p.slots[to_int(base + u32(3))];
        let me = p.slots[to_int(base + u32(4))];
        let mf = p.slots[to_int(base + u32(5))];
        out = complex(ma * out.re + mb * out.im + me, mc * out.re + md * out.im + mf);
        tcol = (tcol + to_float(i2)) / to_float(k);
    }
    out = out + complex(p.offset_x, p.offset_y);
    if (dc) {
        if (s > u32(0)) {
            color = to_float(level) / to_float(iters);
        } else {
            color = tcol;
        }
    }
    #{ z: vec3(out.re, out.im, v.z), color: color }
}
