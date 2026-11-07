/// Finite-depth IFS path: plots the depth-k polyline of up to 12 affine
/// maps in visiting order, with the curve parameter as optional direct
/// color. Written by the L-System script; depth is a live parameter.
///
/// # Authors
/// - Fractals for All
/// - Claude Fable 5

// L-System Path, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    iterations: #{
        value: 5, min: 1, max: 12,
        caption: "Iterations",
        hint: "L-system depth: the path has map_count^iterations segments. A live parameter — nothing is re-baked when it changes.",
    },
    map_count: #{
        value: 4, min: 2, max: 12,
        caption: "Map Count",
        hint: "How many of the twelve map slots are in use. Set by the L-System script.",
    },
    connect: #{
        value: true,
        caption: "Connect",
        hint: "Draw the connecting segments between consecutive vertices (the path). Off plots only the vertices.",
    },
    dc: #{
        value: true,
        caption: "Direct Color",
        hint: "Color by the curve parameter t, so the palette runs along the path from start to end (needs the transform's Direct Color at 1).",
    },
    m0_a: #{
        value: 1.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 A",
        hint: "Map 0: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m0_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 B",
        hint: "Map 0: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m0_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 C",
        hint: "Map 0: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m0_d: #{
        value: 1.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 D",
        hint: "Map 0: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m0_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 E",
        hint: "Map 0: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m0_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M0 F",
        hint: "Map 0: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m1_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 A",
        hint: "Map 1: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m1_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 B",
        hint: "Map 1: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m1_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 C",
        hint: "Map 1: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m1_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 D",
        hint: "Map 1: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m1_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 E",
        hint: "Map 1: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m1_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M1 F",
        hint: "Map 1: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m2_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 A",
        hint: "Map 2: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m2_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 B",
        hint: "Map 2: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m2_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 C",
        hint: "Map 2: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m2_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 D",
        hint: "Map 2: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m2_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 E",
        hint: "Map 2: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m2_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M2 F",
        hint: "Map 2: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m3_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 A",
        hint: "Map 3: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m3_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 B",
        hint: "Map 3: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m3_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 C",
        hint: "Map 3: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m3_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 D",
        hint: "Map 3: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m3_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 E",
        hint: "Map 3: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m3_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M3 F",
        hint: "Map 3: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m4_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 A",
        hint: "Map 4: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m4_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 B",
        hint: "Map 4: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m4_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 C",
        hint: "Map 4: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m4_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 D",
        hint: "Map 4: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m4_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 E",
        hint: "Map 4: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m4_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M4 F",
        hint: "Map 4: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m5_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 A",
        hint: "Map 5: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m5_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 B",
        hint: "Map 5: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m5_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 C",
        hint: "Map 5: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m5_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 D",
        hint: "Map 5: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m5_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 E",
        hint: "Map 5: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m5_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M5 F",
        hint: "Map 5: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m6_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 A",
        hint: "Map 6: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m6_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 B",
        hint: "Map 6: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m6_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 C",
        hint: "Map 6: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m6_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 D",
        hint: "Map 6: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m6_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 E",
        hint: "Map 6: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m6_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M6 F",
        hint: "Map 6: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m7_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 A",
        hint: "Map 7: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m7_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 B",
        hint: "Map 7: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m7_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 C",
        hint: "Map 7: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m7_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 D",
        hint: "Map 7: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m7_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 E",
        hint: "Map 7: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m7_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M7 F",
        hint: "Map 7: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m8_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 A",
        hint: "Map 8: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m8_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 B",
        hint: "Map 8: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m8_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 C",
        hint: "Map 8: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m8_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 D",
        hint: "Map 8: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m8_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 E",
        hint: "Map 8: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m8_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M8 F",
        hint: "Map 8: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m9_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 A",
        hint: "Map 9: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m9_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 B",
        hint: "Map 9: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m9_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 C",
        hint: "Map 9: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m9_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 D",
        hint: "Map 9: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m9_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 E",
        hint: "Map 9: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m9_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M9 F",
        hint: "Map 9: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m10_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 A",
        hint: "Map 10: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m10_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 B",
        hint: "Map 10: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m10_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 C",
        hint: "Map 10: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m10_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 D",
        hint: "Map 10: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m10_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 E",
        hint: "Map 10: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m10_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M10 F",
        hint: "Map 10: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m11_a: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 A",
        hint: "Map 11: affine coefficient a (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m11_b: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 B",
        hint: "Map 11: affine coefficient b (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m11_c: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 C",
        hint: "Map 11: affine coefficient c (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m11_d: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 D",
        hint: "Map 11: affine coefficient d (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m11_e: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 E",
        hint: "Map 11: affine coefficient e (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    m11_f: #{
        value: 0.0, min: -4.0, max: 4.0, free: true,
        caption: "M11 F",
        hint: "Map 11: affine coefficient f (x' = a·x + b·y + e, y' = c·x + d·y + f). Normally written by the L-System script, not by hand.",
    },
    anchored: #{
        value: false,
        caption: "Anchored",
        hint: "Vertex-chain mode: connect the images of the anchor point in consecutive cells, instead of drawing each cell's entry-to-exit span. Node-rewriting (space-filling) curves need this — their spans lie on the cell-boundary lattice and overlap each other; the centre chain is the classic self-avoiding drawing.",
    },
    anchor_x: #{
        value: 0.5, min: -2.0, max: 2.0, free: true,
        caption: "Anchor X",
        hint: "Anchor point x in the curve's unit-displacement frame (the attractor's bounding-box centre, set by the script).",
    },
    anchor_y: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Anchor Y",
        hint: "Anchor point y.",
    },
    thickness: #{
        value: 0.0, min: 0.0, max: 0.2,
        caption: "Thickness",
        hint: "Half-width of the drawn line, in the curve's own units (the whole curve spans 1). Corners are joined seamlessly: each segment is clipped at the mitre line it shares with its neighbours and the wedge the turn opens is filled, so the stroke is covered exactly once — no bright spot at a vertex. Note the density trade — the same samples spread over more area, so a thick line is dimmer; raise Brightness to match.",
    },
    soft: #{
        value: false,
        caption: "Soft Edges",
        hint: "Gaussian falloff across the width instead of a flat ribbon. Flat reads as a drawn line; soft reads as a glow.",
    },
    offset_x: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Offset X",
        hint: "Move the whole curve along x, in the curve's own units. The path is built in its own frame, so an offset of minus its centre puts the object on the origin — which is what rotation and zoom orbit around. The script sets this to centre the curve.",
    },
    offset_y: #{
        value: 0.0, min: -2.0, max: 2.0, free: true,
        caption: "Offset Y",
        hint: "Move the whole curve along y.",
    },
};

const INFO = #{ title: "L-System Path", name: "rhai_lsystem_path", category: "advanced_2d", z: "preserve_z" };

fn prepare(p) {
    let out = array(83, 0.0);
    out[0] = to_float(p.iterations);
    out[1] = to_float(p.map_count);
    out[2] = to_float(p.connect);
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
    out[76] = to_float(p.anchored);
    out[77] = p.anchor_x;
    out[78] = p.anchor_y;
    out[79] = p.thickness;
    out[80] = to_float(p.soft);
    out[81] = p.offset_x;
    out[82] = p.offset_y;
    #{ slots: out }
}

fn lsystem_path_cell(p, idx, iters, n) {
    let ax = 1.0;
    let aby = 0.0;
    let cx = 0.0;
    let dy = 1.0;
    let tx = 0.0;
    let ty = 0.0;
    let rem = idx;
    for j_n in 0..to_int(iters) {
        let j = u32(j_n);
        let d = rem % n;
        rem = rem / n;
        let base = u32(4) + d * u32(6);
        let ma = p.slots[to_int(base)];
        let mb = p.slots[to_int(base + u32(1))];
        let mc = p.slots[to_int(base + u32(2))];
        let md = p.slots[to_int(base + u32(3))];
        let me = p.slots[to_int(base + u32(4))];
        let mf = p.slots[to_int(base + u32(5))];
        let nax = ma * ax + mb * cx;
        let nab = ma * aby + mb * dy;
        let ncx = mc * ax + md * cx;
        let ndy = mc * aby + md * dy;
        let ntx = ma * tx + mb * ty + me;
        let nty = mc * tx + md * ty + mf;
        ax = nax;
        aby = nab;
        cx = ncx;
        dy = ndy;
        tx = ntx;
        ty = nty;
    }
    vec4(tx, ty, ax + tx, cx + ty)
}

fn lsystem_path_point(p, idx, iters, n, anchor) {
    let v = anchor;
    let rem = idx;
    for j_n in 0..to_int(iters) {
        let j = u32(j_n);
        let d = rem % n;
        rem = rem / n;
        let base = u32(4) + d * u32(6);
        let ma = p.slots[to_int(base)];
        let mb = p.slots[to_int(base + u32(1))];
        let mc = p.slots[to_int(base + u32(2))];
        let md = p.slots[to_int(base + u32(3))];
        let me = p.slots[to_int(base + u32(4))];
        let mf = p.slots[to_int(base + u32(5))];
        v = complex(ma * v.re + mb * v.im + me, mc * v.re + md * v.im + mf);
    }
    v
}

fn variation(w, p, ctx) {
    let color = ctx.color;
    let iters = clamp(u32(to_float(p.iterations)), u32(1), u32(12));
    let n = clamp(u32(to_float(p.map_count)), u32(2), u32(12));
    let connect = to_float(p.connect) > 0.5;
    let dc = to_float(p.dc) > 0.5;
    let anchored = to_float(p.anchored) > 0.5;
    let total = u32(1);
    for j_n in 0..to_int(iters) {
        let j = u32(j_n);
        total = min(total * n, u32(16000000));
    }
    let t = rand();
    let thickness = p.thickness;
    let thick = thickness > 0.0;
    let seg_a = complex(0.0, 0.0);
    let seg_b = complex(0.0, 0.0);
    let frac = 0.0;
    let dir_in = complex(0.0, 0.0);
    let dir_out = complex(0.0, 0.0);
    if (anchored) {
        let anchor = complex(p.anchor_x, p.anchor_y);
        let segs = max(total - u32(1), u32(1));
        let ts = t * to_float(segs);
        let idx = min(u32(ts), segs - u32(1));
        seg_a = lsystem_path_point(p, idx, iters, n, anchor);
        seg_b = lsystem_path_point(p, idx + u32(1), iters, n, anchor);
        frac = clamp(ts - to_float(idx), 0.0, 1.0);
        if (thick && idx > u32(0)) {
            dir_in = seg_a - lsystem_path_point(p, idx - u32(1), iters, n, anchor);
        }
        if (thick && idx + u32(2) <= segs) {
            dir_out = lsystem_path_point(p, idx + u32(2), iters, n, anchor) - seg_b;
        }
    } else {
        let idx = min(u32(t * to_float(total)), total - u32(1));
        let cell = lsystem_path_cell(p, idx, iters, n);
        seg_a = complex(cell.x, cell.y);
        seg_b = complex(cell.z, cell.w);
        frac = clamp(t * to_float(total) - to_float(idx), 0.0, 1.0);
        if (thick && idx > u32(0)) {
            let prev = lsystem_path_cell(p, idx - u32(1), iters, n);
            if ((complex(prev.z, prev.w) - seg_a).abs() < thickness) {
                dir_in = complex(prev.z, prev.w) - complex(prev.x, prev.y);
            }
        }
        if (thick && idx + u32(1) < total) {
            let next = lsystem_path_cell(p, idx + u32(1), iters, n);
            if ((complex(next.x, next.y) - seg_b).abs() < thickness) {
                dir_out = complex(next.z, next.w) - complex(next.x, next.y);
            }
        }
    }
    let out = seg_a;
    if (connect) {
        out = mix(seg_a, seg_b, frac);
    }
    if (thick) {
        let soft = to_float(p.soft) > 0.5;
        let dseg = seg_b - seg_a;
        let dlen = dseg.abs();
        let has_seg = connect && dlen > 1.0e-9;
        let r = thickness;
        if (soft || !has_seg) {
            let jit = (rand() + rand() + rand() + rand() - 2.0) * 0.5;
            if (!soft) {
                jit = sqrt(rand());
            }
            if (has_seg) {
                let perp = complex(-dseg.im, dseg.re) / dlen;
                out = out + perp * (r * jit);
            } else {
                let ang = rand() * 6.28318530718;
                out = out + complex(cos(ang), sin(ang)) * (r * abs(jit));
            }
        } else {
            let v = dseg / dlen;
            let perp = complex(-v.im, v.re);
            let n0 = complex(0.0, 0.0);
            let phi0 = 0.0;
            let li = dir_in.abs();
            if (li > 1.0e-9) {
                let u = dir_in / li;
                phi0 = acos(clamp((u.re * v.re + u.im * v.im), -1.0, 1.0));
                if (phi0 > 1.0e-4 && phi0 < 3.1) {
                    let t1 = u + v;
                    n0 = (t1 / t1.abs());
                }
            }
            let n1 = complex(0.0, 0.0);
            let phi1 = 0.0;
            let lo = dir_out.abs();
            if (lo > 1.0e-9) {
                let z = dir_out / lo;
                phi1 = acos(clamp((v.re * z.re + v.im * z.im), -1.0, 1.0));
                if (phi1 > 1.0e-4 && phi1 < 3.1) {
                    let t2 = v + z;
                    n1 = (t2 / t2.abs());
                }
            }
            let cut0 = 0.5 * r * min(8.0, tan(0.5 * phi0));
            let cut1 = 0.5 * r * min(8.0, tan(0.5 * phi1));
            let shaft = max(0.0, 2.0 * dlen - cut0 - cut1);
            let wedge = 0.5 * phi0 * r;
            if (rand() * (shaft + wedge) < wedge) {
                let u = dir_in / li;
                let sgn = (if (u.re * v.im - u.im * v.re) > 0.0 { 1.0 } else { -1.0 });
                let e2 = sgn * complex(-u.im, u.re);
                let beta = -1.5707963268 + phi0 * rand();
                out = seg_a + (cos(beta) * u + sin(beta) * e2) * (r * sqrt(rand()));
            } else {
                let s = 0.0;
                let o = 0.0;
                for k_n in 0..8 {
                    let k = u32(k_n);
                    s = rand() * dlen;
                    o = (rand() * 2.0 - 1.0) * r;
                    let rel = v * s + perp * o;
                    let in0 = n0.norm_sqr() < 0.5 || (rel.re * n0.re + rel.im * n0.im) >= 0.0;
                    let t3 = rel - dseg;
                    let in1 = n1.norm_sqr() < 0.5 || (t3.re * n1.re + t3.im * n1.im) <= 0.0;
                    if (in0 && in1) {
                        break;
                    }
                }
                out = seg_a + v * s + perp * o;
            }
        }
    }
    out = out + complex(p.offset_x, p.offset_y);
    if (dc) {
        color = t;
    }
    #{ z: out, color: color }
}

fn variation3(q, p, ctx) {
    let color = ctx.color;
    let iters = clamp(u32(to_float(p.iterations)), u32(1), u32(12));
    let n = clamp(u32(to_float(p.map_count)), u32(2), u32(12));
    let connect = to_float(p.connect) > 0.5;
    let dc = to_float(p.dc) > 0.5;
    let anchored = to_float(p.anchored) > 0.5;
    let total = u32(1);
    for j_n in 0..to_int(iters) {
        let j = u32(j_n);
        total = min(total * n, u32(16000000));
    }
    let t = rand();
    let thickness = p.thickness;
    let thick = thickness > 0.0;
    let seg_a = complex(0.0, 0.0);
    let seg_b = complex(0.0, 0.0);
    let frac = 0.0;
    let dir_in = complex(0.0, 0.0);
    let dir_out = complex(0.0, 0.0);
    if (anchored) {
        let anchor = complex(p.anchor_x, p.anchor_y);
        let segs = max(total - u32(1), u32(1));
        let ts = t * to_float(segs);
        let idx = min(u32(ts), segs - u32(1));
        seg_a = lsystem_path_point(p, idx, iters, n, anchor);
        seg_b = lsystem_path_point(p, idx + u32(1), iters, n, anchor);
        frac = clamp(ts - to_float(idx), 0.0, 1.0);
        if (thick && idx > u32(0)) {
            dir_in = seg_a - lsystem_path_point(p, idx - u32(1), iters, n, anchor);
        }
        if (thick && idx + u32(2) <= segs) {
            dir_out = lsystem_path_point(p, idx + u32(2), iters, n, anchor) - seg_b;
        }
    } else {
        let idx = min(u32(t * to_float(total)), total - u32(1));
        let cell = lsystem_path_cell(p, idx, iters, n);
        seg_a = complex(cell.x, cell.y);
        seg_b = complex(cell.z, cell.w);
        frac = clamp(t * to_float(total) - to_float(idx), 0.0, 1.0);
        if (thick && idx > u32(0)) {
            let prev = lsystem_path_cell(p, idx - u32(1), iters, n);
            if ((complex(prev.z, prev.w) - seg_a).abs() < thickness) {
                dir_in = complex(prev.z, prev.w) - complex(prev.x, prev.y);
            }
        }
        if (thick && idx + u32(1) < total) {
            let next = lsystem_path_cell(p, idx + u32(1), iters, n);
            if ((complex(next.x, next.y) - seg_b).abs() < thickness) {
                dir_out = complex(next.z, next.w) - complex(next.x, next.y);
            }
        }
    }
    let out = seg_a;
    if (connect) {
        out = mix(seg_a, seg_b, frac);
    }
    if (thick) {
        let soft = to_float(p.soft) > 0.5;
        let dseg = seg_b - seg_a;
        let dlen = dseg.abs();
        let has_seg = connect && dlen > 1.0e-9;
        let r = thickness;
        if (soft || !has_seg) {
            let jit = (rand() + rand() + rand() + rand() - 2.0) * 0.5;
            if (!soft) {
                jit = sqrt(rand());
            }
            if (has_seg) {
                let perp = complex(-dseg.im, dseg.re) / dlen;
                out = out + perp * (r * jit);
            } else {
                let ang = rand() * 6.28318530718;
                out = out + complex(cos(ang), sin(ang)) * (r * abs(jit));
            }
        } else {
            let v = dseg / dlen;
            let perp = complex(-v.im, v.re);
            let n0 = complex(0.0, 0.0);
            let phi0 = 0.0;
            let li = dir_in.abs();
            if (li > 1.0e-9) {
                let u = dir_in / li;
                phi0 = acos(clamp((u.re * v.re + u.im * v.im), -1.0, 1.0));
                if (phi0 > 1.0e-4 && phi0 < 3.1) {
                    let t1 = u + v;
                    n0 = (t1 / t1.abs());
                }
            }
            let n1 = complex(0.0, 0.0);
            let phi1 = 0.0;
            let lo = dir_out.abs();
            if (lo > 1.0e-9) {
                let z = dir_out / lo;
                phi1 = acos(clamp((v.re * z.re + v.im * z.im), -1.0, 1.0));
                if (phi1 > 1.0e-4 && phi1 < 3.1) {
                    let t2 = v + z;
                    n1 = (t2 / t2.abs());
                }
            }
            let cut0 = 0.5 * r * min(8.0, tan(0.5 * phi0));
            let cut1 = 0.5 * r * min(8.0, tan(0.5 * phi1));
            let shaft = max(0.0, 2.0 * dlen - cut0 - cut1);
            let wedge = 0.5 * phi0 * r;
            if (rand() * (shaft + wedge) < wedge) {
                let u = dir_in / li;
                let sgn = (if (u.re * v.im - u.im * v.re) > 0.0 { 1.0 } else { -1.0 });
                let e2 = sgn * complex(-u.im, u.re);
                let beta = -1.5707963268 + phi0 * rand();
                out = seg_a + (cos(beta) * u + sin(beta) * e2) * (r * sqrt(rand()));
            } else {
                let s = 0.0;
                let o = 0.0;
                for k_n in 0..8 {
                    let k = u32(k_n);
                    s = rand() * dlen;
                    o = (rand() * 2.0 - 1.0) * r;
                    let rel = v * s + perp * o;
                    let in0 = n0.norm_sqr() < 0.5 || (rel.re * n0.re + rel.im * n0.im) >= 0.0;
                    let t3 = rel - dseg;
                    let in1 = n1.norm_sqr() < 0.5 || (t3.re * n1.re + t3.im * n1.im) <= 0.0;
                    if (in0 && in1) {
                        break;
                    }
                }
                out = seg_a + v * s + perp * o;
            }
        }
    }
    out = out + complex(p.offset_x, p.offset_y);
    if (dc) {
        color = t;
    }
    #{ z: vec3(out.re, out.im, q.z), color: color }
}
