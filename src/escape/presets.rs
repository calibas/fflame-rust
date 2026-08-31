//! Named starting points for each escape formula.
//!
//! GENERATED from the visual-regression configs
//! (`tests/visual/configs/escape/`) by
//! `scripts/gen_escape_presets.py`, then committed. Those configs are
//! rendered and hash-compared on every suite run, so a preset built
//! from one is known to produce a picture — which is the property
//! that matters, and the one hand-invented values would not have.
//!
//! The FIRST preset of a formula is its default: what a switch to
//! that formula applies. Everything a formula needs to look like
//! itself travels together — view, iteration budget, coloring, and
//! both parameter sets — because that is exactly what does not carry
//! over from the formula you were just looking at.

use super::EscapePreset;


pub static MANDELBROT: &[EscapePreset] = &[
    EscapePreset {
        name: "Smooth",
        center_re: "0",
        center_im: "0",
        zoom_log2: 0.0,
        max_iter: 512,
        coloring: "smooth",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.03)],
    },
    EscapePreset {
        name: "Seahorse Valley",
        center_re: "-0.7453",
        center_im: "0.1127",
        zoom_log2: 7.0,
        max_iter: 1024,
        coloring: "smooth",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.01)],
    },
    EscapePreset {
        name: "Distance Estimate",
        center_re: "0",
        center_im: "0",
        zoom_log2: 0.0,
        max_iter: 256,
        coloring: "distance_estimate",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.05)],
    },
    EscapePreset {
        name: "Orbit Trap",
        center_re: "0",
        center_im: "0",
        zoom_log2: 0.0,
        max_iter: 256,
        coloring: "orbit_trap",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.8), ("shape", 0.0)],
    },
    EscapePreset {
        name: "Stripe Average",
        center_re: "0",
        center_im: "0",
        zoom_log2: 0.0,
        max_iter: 256,
        coloring: "stripe_average",
        julia: None,
        formula_params: &[],
        coloring_params: &[("density", 6.0), ("scale", 1.0)],
    },
    EscapePreset {
        name: "Normal Map Relief",
        center_re: "0",
        center_im: "0",
        zoom_log2: 0.0,
        max_iter: 512,
        coloring: "normal_map",
        julia: None,
        formula_params: &[],
        coloring_params: &[("angle", 0.125), ("height", 1.5), ("scale", 1.0)],
    },
    EscapePreset {
        name: "Julia Dragon",
        center_re: "0",
        center_im: "0",
        zoom_log2: 0.0,
        max_iter: 512,
        coloring: "smooth",
        julia: Some((-0.8, 0.156)),
        formula_params: &[],
        coloring_params: &[("scale", 0.02)],
    },
];

pub static MULTIBROT: &[EscapePreset] = &[
    EscapePreset {
        name: "Power 4",
        center_re: "0",
        center_im: "0",
        zoom_log2: 0.0,
        max_iter: 256,
        coloring: "smooth",
        julia: None,
        formula_params: &[("power", 4.0)],
        coloring_params: &[("scale", 0.03)],
    },
];

pub static BURNING_SHIP: &[EscapePreset] = &[
    EscapePreset {
        name: "Classic",
        center_re: "-0.5",
        center_im: "0.5",
        zoom_log2: 0.0,
        max_iter: 256,
        coloring: "smooth",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.03)],
    },
    EscapePreset {
        name: "Celtic",
        center_re: "-0.5",
        center_im: "0",
        zoom_log2: 0.0,
        max_iter: 256,
        coloring: "smooth",
        julia: None,
        formula_params: &[("variant", 3.0)],
        coloring_params: &[("scale", 0.03)],
    },
];

pub static TRICORN: &[EscapePreset] = &[
    EscapePreset {
        name: "Classic",
        center_re: "-0.3",
        center_im: "0",
        zoom_log2: 0.0,
        max_iter: 256,
        coloring: "smooth",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.03)],
    },
];

pub static PHOENIX: &[EscapePreset] = &[
    EscapePreset {
        name: "Classic",
        center_re: "0",
        center_im: "0",
        zoom_log2: -0.3,
        max_iter: 256,
        coloring: "smooth",
        julia: Some((0.5667, 0.0)),
        formula_params: &[("p_re", -0.5)],
        coloring_params: &[("scale", 0.03)],
    },
];

pub static MANOWAR: &[EscapePreset] = &[
    EscapePreset {
        name: "Classic",
        center_re: "-0.4",
        center_im: "0",
        zoom_log2: 1.0,
        max_iter: 256,
        coloring: "smooth",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.03)],
    },
];

pub static LAMBDA: &[EscapePreset] = &[
    EscapePreset {
        name: "Parameter Plane",
        center_re: "0.5",
        center_im: "0",
        zoom_log2: -0.5,
        max_iter: 256,
        coloring: "smooth",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.03)],
    },
    EscapePreset {
        name: "Julia, Distance",
        center_re: "0.5",
        center_im: "0",
        zoom_log2: 0.0,
        max_iter: 256,
        coloring: "distance_estimate",
        julia: Some((3.0, 0.05)),
        formula_params: &[],
        coloring_params: &[("scale", 0.05)],
    },
];

pub static LAMBDA_SINE: &[EscapePreset] = &[
    EscapePreset {
        name: "Parameter Plane",
        center_re: "0.0",
        center_im: "0.0",
        zoom_log2: -1.0,
        max_iter: 300,
        coloring: "smooth",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.05)],
    },
    EscapePreset {
        name: "Bouquet",
        center_re: "0.0",
        center_im: "0.0",
        zoom_log2: -2.0,
        max_iter: 400,
        coloring: "smooth",
        julia: Some((0.5, 0.0)),
        formula_params: &[],
        coloring_params: &[("scale", 0.05)],
    },
];

pub static FEATHER: &[EscapePreset] = &[
    EscapePreset {
        name: "Classic",
        center_re: "-0.35",
        center_im: "0",
        zoom_log2: 0.5,
        max_iter: 256,
        coloring: "smooth",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.03)],
    },
];

pub static MCMULLEN: &[EscapePreset] = &[
    EscapePreset {
        name: "Carpet",
        center_re: "0",
        center_im: "0",
        zoom_log2: 0.5,
        max_iter: 128,
        coloring: "smooth",
        julia: Some((0.04, 0.0)),
        formula_params: &[],
        coloring_params: &[("scale", 0.05)],
    },
];

pub static MAGNET: &[EscapePreset] = &[
    EscapePreset {
        name: "Magnet 1",
        center_re: "1",
        center_im: "0",
        zoom_log2: -0.8,
        max_iter: 128,
        coloring: "escape_count",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.03)],
    },
];

pub static NEWTON: &[EscapePreset] = &[
    EscapePreset {
        name: "Cubic Roots",
        center_re: "0",
        center_im: "0",
        zoom_log2: 0.0,
        max_iter: 64,
        coloring: "root_basin",
        julia: None,
        formula_params: &[],
        coloring_params: &[("roots", 3.0), ("speed", 0.01)],
    },
    EscapePreset {
        name: "Relaxed",
        center_re: "0",
        center_im: "0",
        zoom_log2: 0.0,
        max_iter: 64,
        coloring: "root_basin",
        julia: None,
        formula_params: &[("power", 3.0), ("relax_im", 0.6), ("relax_re", 1.35), ("scheme", 0.0)],
        coloring_params: &[("roots", 3.0), ("speed", 0.01)],
    },
];

pub static NOVA: &[EscapePreset] = &[
    EscapePreset {
        name: "Nova 3",
        center_re: "-0.3",
        center_im: "0",
        zoom_log2: 0.5,
        max_iter: 128,
        coloring: "smooth",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.03)],
    },
    EscapePreset {
        name: "Sphere Average",
        center_re: "0.0",
        center_im: "0.0",
        zoom_log2: -1.0,
        max_iter: 60,
        coloring: "sphere_average",
        julia: None,
        formula_params: &[("power", 4.0)],
        coloring_params: &[("at_infinity", 0.0), ("scale", 3.0), ("stride", 1.0), ("target_im", 0.0), ("target_re", 1.0)],
    },
];

pub static NOVARETTI: &[EscapePreset] = &[
    EscapePreset {
        name: "Classic",
        center_re: "0",
        center_im: "0",
        zoom_log2: -2.0,
        max_iter: 128,
        coloring: "escape_count",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.02)],
    },
    EscapePreset {
        name: "Period",
        center_re: "0",
        center_im: "0",
        zoom_log2: -2.0,
        max_iter: 200,
        coloring: "period",
        julia: None,
        formula_params: &[],
        coloring_params: &[("escape_scale", 0.01), ("scale", 0.13)],
    },
    EscapePreset {
        name: "Julia Trap",
        center_re: "0",
        center_im: "0",
        zoom_log2: 0.0,
        max_iter: 10,
        coloring: "orbit_trap",
        julia: Some((0.3, 0.2)),
        formula_params: &[],
        coloring_params: &[("scale", 2.5), ("shape", 0.0)],
    },
];

pub static COLLATZ: &[EscapePreset] = &[
    EscapePreset {
        name: "Classic",
        center_re: "1",
        center_im: "0",
        zoom_log2: 1.5,
        max_iter: 64,
        coloring: "smooth",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.1)],
    },
];

pub static DUCKS: &[EscapePreset] = &[
    EscapePreset {
        name: "Parameter Plane",
        center_re: "0",
        center_im: "0",
        zoom_log2: -0.5,
        max_iter: 80,
        coloring: "magnitude_average",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 2.0)],
    },
    EscapePreset {
        name: "Julia",
        center_re: "0",
        center_im: "0",
        zoom_log2: 0.2,
        max_iter: 80,
        coloring: "magnitude_average",
        julia: Some((0.1, -0.62)),
        formula_params: &[],
        coloring_params: &[("offset", 1.648), ("scale", 11.64)],
    },
    EscapePreset {
        name: "Secant",
        center_re: "0",
        center_im: "0",
        zoom_log2: 0.2,
        max_iter: 80,
        coloring: "magnitude_average",
        julia: Some((0.1, -0.62)),
        formula_params: &[("variant", 2.0)],
        coloring_params: &[("offset", 1.857), ("scale", 5.17)],
    },
];

pub static KALISET: &[EscapePreset] = &[
    EscapePreset {
        name: "Glow",
        center_re: "0",
        center_im: "0",
        zoom_log2: 0.5,
        max_iter: 24,
        coloring: "orbit_average",
        julia: Some((0.6, 0.4)),
        formula_params: &[],
        coloring_params: &[("scale", 0.5)],
    },
];

pub static ORIGAMI: &[EscapePreset] = &[
    EscapePreset {
        name: "Butterfly",
        center_re: "0.0",
        center_im: "0.0",
        zoom_log2: -1.0,
        max_iter: 32,
        coloring: "position_map",
        julia: None,
        formula_params: &[("seed", 8.0), ("spread", 2.0)],
        coloring_params: &[("address_mix", 1.5), ("freq_x", 5.0), ("freq_y", 1.5)],
    },
    EscapePreset {
        name: "Relief",
        center_re: "0.0",
        center_im: "0.0",
        zoom_log2: -1.0,
        max_iter: 32,
        coloring: "position_map",
        julia: None,
        formula_params: &[("seed", 8.0), ("spread", 2.0)],
        coloring_params: &[("address_mix", 1.5), ("freq_x", 5.0), ("freq_y", 1.5)],
    },
    EscapePreset {
        name: "Soft Relief",
        center_re: "0.0",
        center_im: "0.0",
        zoom_log2: -1.0,
        max_iter: 32,
        coloring: "position_map",
        julia: None,
        formula_params: &[("seed", 8.0), ("spread", 2.0)],
        coloring_params: &[("address_mix", 1.5), ("freq_x", 5.0), ("freq_y", 1.5)],
    },
];

pub static LATTES: &[EscapePreset] = &[
    EscapePreset {
        name: "Variant 0",
        center_re: "0.0",
        center_im: "0.0",
        zoom_log2: -1.0,
        max_iter: 5,
        coloring: "sphere_average",
        julia: None,
        formula_params: &[("a_im", 0.866025), ("a_re", -0.5), ("variant", 0.0)],
        coloring_params: &[("at_infinity", 0.0), ("scale", 2.0), ("stride", 1.0), ("target_im", -0.2), ("target_re", 0.35)],
    },
    EscapePreset {
        name: "Variant 2",
        center_re: "0.0",
        center_im: "0.0",
        zoom_log2: -1.0,
        max_iter: 5,
        coloring: "sphere_average",
        julia: None,
        formula_params: &[("a_im", 0.866025), ("a_re", -0.5), ("variant", 2.0)],
        coloring_params: &[("at_infinity", 0.0), ("scale", 2.0), ("stride", 1.0), ("target_im", -0.2), ("target_re", 0.35)],
    },
];

pub static BARNSLEY: &[EscapePreset] = &[
    EscapePreset {
        name: "M3",
        center_re: "0.3",
        center_im: "0",
        zoom_log2: 0.0,
        max_iter: 128,
        coloring: "smooth",
        julia: None,
        formula_params: &[("variant", 2.0)],
        coloring_params: &[("scale", 0.05)],
    },
    EscapePreset {
        name: "M1 Julia",
        center_re: "0",
        center_im: "0",
        zoom_log2: 0.0,
        max_iter: 128,
        coloring: "escape_count",
        julia: Some((0.6, 1.1)),
        formula_params: &[],
        coloring_params: &[("scale", 0.05)],
    },
];

pub static CACTUS: &[EscapePreset] = &[
    EscapePreset {
        name: "Classic",
        center_re: "0",
        center_im: "0",
        zoom_log2: 0.5,
        max_iter: 128,
        coloring: "smooth",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.05)],
    },
];

pub static EXPONENTIAL: &[EscapePreset] = &[
    EscapePreset {
        name: "Classic",
        center_re: "-1",
        center_im: "0",
        zoom_log2: -1.0,
        max_iter: 128,
        coloring: "smooth",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.05)],
    },
];

pub static LITTLEWOOD: &[EscapePreset] = &[
    EscapePreset {
        name: "Classic",
        center_re: "0",
        center_im: "0",
        zoom_log2: 0.5,
        max_iter: 48,
        coloring: "escape_count",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.05)],
    },
];

pub static SPIDER: &[EscapePreset] = &[
    EscapePreset {
        name: "Classic",
        center_re: "-0.4",
        center_im: "0",
        zoom_log2: 0.0,
        max_iter: 256,
        coloring: "smooth",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.03)],
    },
];

pub static TETRATION: &[EscapePreset] = &[
    EscapePreset {
        name: "Classic",
        center_re: "0",
        center_im: "0",
        zoom_log2: -1.5,
        max_iter: 128,
        coloring: "smooth",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.05)],
    },
    EscapePreset {
        name: "Period",
        center_re: "0",
        center_im: "0",
        zoom_log2: -1.5,
        max_iter: 200,
        coloring: "period",
        julia: None,
        formula_params: &[],
        coloring_params: &[("escape_scale", 0.005), ("scale", 0.13)],
    },
];

pub static TRIG: &[EscapePreset] = &[
    EscapePreset {
        name: "Sine",
        center_re: "0",
        center_im: "0",
        zoom_log2: -1.0,
        max_iter: 128,
        coloring: "smooth",
        julia: None,
        formula_params: &[],
        coloring_params: &[("scale", 0.05)],
    },
];

pub static WEIERSTRASS: &[EscapePreset] = &[
    EscapePreset {
        name: "Hillshade",
        center_re: "0.0",
        center_im: "0.0",
        zoom_log2: 0.0,
        max_iter: 24,
        coloring: "field_hillshade",
        julia: None,
        formula_params: &[("a", 0.55), ("b", 2.0), ("generator", 0.0), ("phase", 0.0)],
        coloring_params: &[("azimuth", 315.0), ("elevation", 45.0), ("relief", 2.0), ("scale", 0.6)],
    },
];

pub static MARKUS_LYAPUNOV: &[EscapePreset] = &[
    EscapePreset {
        name: "A-B Sequence",
        center_re: "3.2",
        center_im: "3.2",
        zoom_log2: 1.3,
        max_iter: 600,
        coloring: "field_diverging",
        julia: None,
        formula_params: &[("seq_bits", 2.0), ("seq_len", 2.0), ("warmup", 50.0)],
        coloring_params: &[("scale", 6.0)],
    },
];

pub static STANDARD_MAP_FTLE: &[EscapePreset] = &[
    EscapePreset {
        name: "Classic",
        center_re: "3.14159265",
        center_im: "0.0",
        zoom_log2: -0.65,
        max_iter: 400,
        coloring: "field_diverging",
        julia: None,
        formula_params: &[("k", 1.0)],
        coloring_params: &[("scale", 12.0)],
    },
];
