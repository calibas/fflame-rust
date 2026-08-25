//! Coloring definitions — orbit summary → palette position.
//!
//! One `static ColoringDef` per coloring, WGSL inline. The template
//! wraps the returned coordinate with `fract()` so colorings can
//! return unbounded ramps and let the palette cycle. Signature:
//! `coloring_map(z, n, escaped, state)` where `state` is the orbit
//! accumulator (meaningful only with `NeedsOrbitAccum`).

use super::{ColoringDef, ColoringFeature, EscapeParamDef};

/// Discrete escape count: the classic banded look. `t = n · scale`.
pub static ESCAPE_COUNT: ColoringDef = ColoringDef {
    name: "escape_count",
    display_name: "Escape Count",
    features: &[],
    parameters: &[EscapeParamDef {
        name: "scale",
        display_name: "Scale",
        default: 0.05,
        min: 0.001,
        max: 1.0,
        tooltip: "Palette distance per iteration band. Smaller = broader bands.",
    }],
    wgsl: r#"
fn coloring_map(z: vec2<f32>, n: u32, escaped: bool, converged: bool, state: vec2<f32>) -> f32 {
    return f32(n) * cparam(0u);
}
"#,
    accum_init: "",
    wgsl_accum: "",
};

/// Smooth (continuous) iteration count — the standard fractional
/// escape-time formula `mu = n + 1 - log2(log2 |z|)`, which cancels the
/// banding of the discrete count for any quadratic-growth formula.
pub static SMOOTH: ColoringDef = ColoringDef {
    name: "smooth",
    display_name: "Smooth Iteration",
    features: &[],
    parameters: &[EscapeParamDef {
        name: "scale",
        display_name: "Scale",
        default: 0.05,
        min: 0.001,
        max: 1.0,
        tooltip: "Palette distance per iteration. Smaller = broader gradient.",
    }],
    wgsl: r#"
fn coloring_map(z: vec2<f32>, n: u32, escaped: bool, converged: bool, state: vec2<f32>) -> f32 {
    // |z|^2 at escape is > bailout >= 1, so log2 is safe; the max()
    // guards the first-iteration corner (bailout < 1 configs) without
    // any fast-math-hazard idiom (no self-compare, no self-divide).
    let r2 = max(dot(z, z), 1.0000001);
    let mu = f32(n) + 1.0 - log2(0.5 * log2(r2));
    return mu * cparam(0u);
}
"#,
    accum_init: "",
    wgsl_accum: "",
};

/// Orbit trap: minimum distance the orbit ever came to a trap shape
/// (plan §8 — "composable with any formula; small SDF enum").
/// Colors interior pixels too — trapped orbits are the interesting
/// ones.
pub static ORBIT_TRAP: ColoringDef = ColoringDef {
    name: "orbit_trap",
    display_name: "Orbit Trap",
    features: &[ColoringFeature::NeedsOrbitAccum, ColoringFeature::ColorsInterior],
    parameters: &[
        EscapeParamDef {
            name: "shape",
            display_name: "Trap shape",
            default: 0.0,
            min: 0.0,
            max: 2.0,
            tooltip: "0: point at origin, 1: coordinate axes (cross), 2: unit circle.",
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette distance per unit of trap distance.",
        },
    ],
    wgsl: r#"
fn coloring_map(z: vec2<f32>, n: u32, escaped: bool, converged: bool, state: vec2<f32>) -> f32 {
    return state.x * cparam(1u);
}
"#,
    accum_init: "vec2<f32>(1e30, 0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, state: vec2<f32>) -> vec2<f32> {
    let shape = u32(clamp(cparam(0u), 0.0, 2.0));
    var d: f32;
    switch shape {
        case 0u: { d = length(z); }
        case 1u: { d = min(abs(z.x), abs(z.y)); }
        default: { d = abs(length(z) - 1.0); }
    }
    return vec2<f32>(min(state.x, d), state.y);
}
"#,
};

/// Orbit average — the Kali glow (plan §8: "REQUIRED for NonEscaping;
/// optional everywhere"). Running mean of the distance-to-axes trap
/// function; the classic Kaliset look, and a soft organic wash on
/// escaping formulas.
pub static ORBIT_AVERAGE: ColoringDef = ColoringDef {
    name: "orbit_average",
    display_name: "Orbit Average",
    features: &[ColoringFeature::NeedsOrbitAccum, ColoringFeature::ColorsInterior],
    parameters: &[EscapeParamDef {
        name: "scale",
        display_name: "Scale",
        default: 1.0,
        min: 0.01,
        max: 20.0,
        tooltip: "Palette distance per unit of averaged trap value.",
    }],
    wgsl: r#"
fn coloring_map(z: vec2<f32>, n: u32, escaped: bool, converged: bool, state: vec2<f32>) -> f32 {
    // state.x = sum of min(|re|, |im|) over the orbit, state.y = count.
    let mean = state.x / max(state.y, 1.0);
    return mean * cparam(0u);
}
"#,
    accum_init: "vec2<f32>(0.0, 0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, state: vec2<f32>) -> vec2<f32> {
    return state + vec2<f32>(min(abs(z.x), abs(z.y)), 1.0);
}
"#,
};

/// Stripe average — mean of `0.5 + 0.5·sin(density·arg z)` over the
/// orbit (plan §8). The banding classic; spectacular on Ducks-family
/// maps, soft angular striping everywhere else.
pub static STRIPE_AVERAGE: ColoringDef = ColoringDef {
    name: "stripe_average",
    display_name: "Stripe Average",
    features: &[ColoringFeature::NeedsOrbitAccum, ColoringFeature::ColorsInterior],
    parameters: &[
        EscapeParamDef {
            name: "density",
            display_name: "Stripe density",
            default: 4.0,
            min: 0.5,
            max: 32.0,
            tooltip: "Angular frequency of the stripes (sin(density * arg z)).",
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette distance per unit of averaged stripe value.",
        },
    ],
    wgsl: r#"
fn coloring_map(z: vec2<f32>, n: u32, escaped: bool, converged: bool, state: vec2<f32>) -> f32 {
    let mean = state.x / max(state.y, 1.0);
    return mean * cparam(1u);
}
"#,
    accum_init: "vec2<f32>(0.0, 0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, state: vec2<f32>) -> vec2<f32> {
    // Skip exact zero: atan2 at a zero pair is the Metal fast-math
    // hazard (garbage or NaN, see CLAUDE.md) — the branch keeps it
    // from ever being evaluated there, and dropping one sample from
    // a mean is invisible.
    if (dot(z, z) < 1e-30) {
        return state;
    }
    let stripe = 0.5 + 0.5 * sin(cparam(0u) * atan2(z.y, z.x));
    return state + vec2<f32>(stripe, 1.0);
}
"#,
};

/// Root basin — for Convergent formulas over `zᵖ − 1`: which root the
/// orbit landed on (angle-bucketed final z) shaded by convergence
/// speed. On anything else it degrades to an angle-of-final-z wash.
pub static ROOT_BASIN: ColoringDef = ColoringDef {
    name: "root_basin",
    display_name: "Root Basin",
    features: &[ColoringFeature::ColorsInterior],
    parameters: &[
        EscapeParamDef {
            name: "roots",
            display_name: "Root count",
            default: 3.0,
            min: 2.0,
            max: 12.0,
            tooltip: "Number of basins to bucket the final angle into - match the formula's power.",
        },
        EscapeParamDef {
            name: "speed",
            display_name: "Speed shading",
            default: 0.01,
            min: 0.0,
            max: 0.2,
            tooltip: "Palette offset per iteration of convergence time, shading within each basin.",
        },
    ],
    wgsl: r#"
fn coloring_map(z: vec2<f32>, n: u32, escaped: bool, converged: bool, state: vec2<f32>) -> f32 {
    // Angle of the final iterate, bucketed into `roots` equal arcs.
    // A converged orbit sits on a root of z^p - 1, so the bucket IS
    // the basin index. Origin guard: never hand atan2 a zero pair
    // (Metal fast-math hazard).
    var t = 0.0;
    if (dot(z, z) > 1e-30) {
        let tau = 6.28318530718;
        let ang = fract(atan2(z.y, z.x) / tau + 1.0);
        let roots = clamp(cparam(0u), 2.0, 12.0);
        t = floor(ang * roots + 0.5) / roots;
    }
    return t + f32(n) * cparam(1u);
}
"#,
    accum_init: "",
    wgsl_accum: "",
};
