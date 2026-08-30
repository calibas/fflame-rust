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
fn coloring_map(sum: OrbitSummary, state: vec2<f32>) -> f32 {
    return f32(sum.n) * cparam(0u);
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
fn coloring_map(sum: OrbitSummary, state: vec2<f32>) -> f32 {
    // |z|^2 at escape is > bailout >= 1, so log2 is safe; the max()
    // guards the first-iteration corner (bailout < 1 configs) without
    // any fast-math-hazard idiom (no self-compare, no self-divide).
    let r2 = max(dot(sum.z, sum.z), 1.0000001);
    let mu = f32(sum.n) + 1.0 - log2(0.5 * log2(r2));
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
///
/// Shape 3 is a logarithmic spiral, golden by default:
/// `r = log|z| / (4 log g) - arg(z)/2pi`, distance `|r - round(r)|`
/// (after Nylander's golden-ratio spiral trap). Unlike the other
/// three it is not a distance in z-units but in TURNS, doubled to
/// span 0..1; `scale` maps it onto the palette as before.
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
            max: 3.0,
            tooltip: "0: point at origin, 1: coordinate axes (cross), 2: unit circle, \
                      3: logarithmic spiral (golden by default).",
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette distance per unit of trap distance.",
        },
        EscapeParamDef {
            name: "growth",
            display_name: "Spiral growth",
            default: 1.618_034,
            min: 1.05,
            max: 8.0,
            tooltip: "Spiral shape 3 only: the factor the spiral widens by per \
                      QUARTER turn. The default is the golden ratio, which makes \
                      it the golden spiral; 2 gives a doubling-per-quarter-turn \
                      spiral, and values near 1 wind tightly.",
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec2<f32>) -> f32 {
    return state.x * cparam(1u);
}
"#,
    accum_init: "vec2<f32>(1e30, 0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec2<f32>) -> vec2<f32> {
    let shape = u32(clamp(cparam(0u), 0.0, 3.0));
    var d: f32;
    switch shape {
        case 0u: { d = length(z); }
        case 1u: { d = min(abs(z.x), abs(z.y)); }
        case 2u: { d = abs(length(z) - 1.0); }
        default: {
            // Logarithmic spiral, golden by default.
            //
            //   r = log|z| / (4 log g)  -  arg(z) / 2pi
            //
            // is "how many turns out along the spiral this point sits"
            // — g per QUARTER turn is the golden spiral's definition,
            // hence the 4 — so the distance to the nearest arm is how
            // far r sits from a whole number. Halved turns are the
            // farthest a point can be, so the result is doubled to
            // span 0..1 like the other shapes' distances.
            let r2 = dot(z, z);
            if (r2 < 1e-30) {
                // The spiral winds infinitely at the origin, so there
                // is no finite distance to report — and atan2 at a
                // zero pair is the Metal fast-math hazard (pi/4 for
                // same-sign zeros, NaN for mixed; see CLAUDE.md). A
                // huge value leaves the running minimum untouched,
                // which is exactly "this sample says nothing".
                d = 1e30;
            } else {
                let g = max(cparam(2u), 1.05);
                // log|z| = 0.5*log(r2), so the 4 log g below is 8.
                let turns = log(r2) / (8.0 * log(g))
                    - atan2(z.y, z.x) * 0.159154943;
                d = 2.0 * abs(turns - round(turns));
            }
        }
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
fn coloring_map(sum: OrbitSummary, state: vec2<f32>) -> f32 {
    // state.x = sum of min(|re|, |im|) over the orbit, state.y = count.
    let mean = state.x / max(state.y, 1.0);
    return mean * cparam(0u);
}
"#,
    accum_init: "vec2<f32>(0.0, 0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec2<f32>) -> vec2<f32> {
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
fn coloring_map(sum: OrbitSummary, state: vec2<f32>) -> f32 {
    let mean = state.x / max(state.y, 1.0);
    return mean * cparam(1u);
}
"#,
    accum_init: "vec2<f32>(0.0, 0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec2<f32>) -> vec2<f32> {
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

/// Magnitude average — mean of |z| over the orbit. THE Ducks
/// coloring: Monnier's post colors "according to the mean of the
/// magnitude of z, summed over all iterations", and the scaly
/// paisley look is this statistic on a non-escaping log map. Soft
/// luminance wash on escaping formulas.
pub static MAGNITUDE_AVERAGE: ColoringDef = ColoringDef {
    name: "magnitude_average",
    display_name: "Magnitude Average",
    features: &[ColoringFeature::NeedsOrbitAccum, ColoringFeature::ColorsInterior],
    parameters: &[
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 100.0,
            tooltip: "Palette distance per unit of averaged |z| (above the offset).",
        },
        EscapeParamDef {
            name: "offset",
            display_name: "Offset",
            default: 0.0,
            min: -10.0,
            max: 10.0,
            tooltip: "Baseline subtracted from the mean before scaling. A Ducks                       julia field can span only ~0.2 around a large mean -- offset                       to the field's floor, then scale up, to stretch that range                       across the palette (the reference images normalize contrast                       this way).",
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec2<f32>) -> f32 {
    // state.x = sum of |z| over the orbit, state.y = count.
    let mean = state.x / max(state.y, 1.0);
    return (mean - cparam(1u)) * cparam(0u);
}
"#,
    accum_init: "vec2<f32>(0.0, 0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec2<f32>) -> vec2<f32> {
    return state + vec2<f32>(length(z), 1.0);
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
fn coloring_map(sum: OrbitSummary, state: vec2<f32>) -> f32 {
    // Angle of the final iterate, bucketed into `roots` equal arcs.
    // A converged orbit sits on a root of z^p - 1, so the bucket IS
    // the basin index. Origin guard: never hand atan2 a zero pair
    // (Metal fast-math hazard).
    var t = 0.0;
    if (dot(sum.z, sum.z) > 1e-30) {
        let tau = 6.28318530718;
        let ang = fract(atan2(sum.z.y, sum.z.x) / tau + 1.0);
        let roots = clamp(cparam(0u), 2.0, 12.0);
        t = floor(ang * roots + 0.5) / roots;
    }
    return t + f32(sum.n) * cparam(1u);
}
"#,
    accum_init: "",
    wgsl_accum: "",
};

/// Triangle-inequality average (plan §8): at each step, where |z|
/// falls between the triangle-inequality bounds built from |z − c|
/// and |c|. The classic wispy-band coloring, formula-agnostic in this
/// generalized form.
pub static TRIANGLE_INEQUALITY: ColoringDef = ColoringDef {
    name: "triangle_inequality",
    display_name: "Triangle Inequality",
    features: &[ColoringFeature::NeedsOrbitAccum, ColoringFeature::ColorsInterior],
    parameters: &[EscapeParamDef {
        name: "scale",
        display_name: "Scale",
        default: 1.0,
        min: 0.01,
        max: 20.0,
        tooltip: "Palette distance per unit of averaged TIA value.",
    }],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec2<f32>) -> f32 {
    let mean = state.x / max(state.y, 1.0);
    return mean * cparam(0u);
}
"#,
    accum_init: "vec2<f32>(0.0, 0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec2<f32>) -> vec2<f32> {
    let x = length(z - c);
    let mc = length(c);
    let lo = abs(x - mc);
    let hi = x + mc;
    let span = hi - lo;
    if (span < 1e-12) {
        return state;
    }
    let t = clamp((length(z) - lo) / span, 0.0, 1.0);
    return state + vec2<f32>(t, 1.0);
}
"#,
};

/// Interior / period coloring (plan §8, §5.8, §5.17): pixels whose
/// orbit settles into a cycle are colored by the detected cycle
/// length k (Geisler's "Oscillating Tower" signature); escaped pixels
/// get an escape-count wash; undetected interiors stay at the palette
/// origin.
pub static PERIOD: ColoringDef = ColoringDef {
    name: "period",
    display_name: "Period",
    features: &[ColoringFeature::NeedsPeriod, ColoringFeature::ColorsInterior],
    parameters: &[
        EscapeParamDef {
            name: "scale",
            display_name: "Period scale",
            default: 0.1,
            min: 0.001,
            max: 1.0,
            tooltip: "Palette distance per unit of detected cycle length.",
        },
        EscapeParamDef {
            name: "escape_scale",
            display_name: "Escape scale",
            default: 0.02,
            min: 0.0,
            max: 1.0,
            tooltip: "Palette distance per iteration for pixels that escape instead of cycling.",
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec2<f32>) -> f32 {
    if (sum.period > 0u) {
        return f32(sum.period) * cparam(0u);
    }
    if (sum.escaped) {
        return f32(sum.n) * cparam(1u);
    }
    return 0.0;
}
"#,
    accum_init: "",
    wgsl_accum: "",
};

/// Exterior distance estimation (plan §8): `d = |z|·ln|z| / |dz|`
/// from the derivative orbit, mapped through −log2 so equal palette
/// steps mean equal zoom depths of boundary distance. Needs a formula
/// that supplies a derivative (Mandelbrot, Multibrot, Lambda);
/// elsewhere |dz| stays at its seed and the coloring degrades to a
/// |z|·ln|z| wash.
pub static DISTANCE_ESTIMATE: ColoringDef = ColoringDef {
    name: "distance_estimate",
    display_name: "Distance Estimate",
    features: &[ColoringFeature::NeedsDerivative],
    parameters: &[EscapeParamDef {
        name: "scale",
        display_name: "Scale",
        default: 0.05,
        min: 0.001,
        max: 1.0,
        tooltip: "Palette distance per doubling of boundary distance.",
    }],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec2<f32>) -> f32 {
    // Escaped only; |z| > 1 at escape so ln|z| > 0.
    let r = max(length(sum.z), 1.0000001);
    let deriv = max(length(sum.dz), 1e-30);
    let d = max(r * log(r) / deriv, 1e-30);
    return -log2(d) * cparam(0u);
}
"#,
    accum_init: "",
    wgsl_accum: "",
};

/// Analytic normal shading — the "fake 3D" relief.
///
/// The normal comes from the derivative rather than from neighbouring
/// pixels. [Chéritat's derivation](https://www.math.univ-toulouse.fr/~cheritat/wiki-draw/index.php/Mandelbrot_set):
/// the normal is `(x, y, 1)/sqrt(2)` where `(x, y)` is normal to the
/// potential line, and since the potential is `2^-n log|z_n|` one
/// pulls the radial direction back through `dz/dc`. That is `z/dz`.
///
/// Verbatim from the reference implementation on
/// [Wikimedia Commons](https://commons.wikimedia.org/wiki/File:Mandelbrot_set_-_Normal_mapping.png)
/// (the source behind Wikibooks' bump-mapping article):
///
/// ```c
/// u = Z / dC;  u = u / cabs(u);
/// double h2 = 1.5;                      // height of the light
/// double angle = 45.0 / 360.0;          // direction, in turns
/// double complex v = cexp(2.0 * angle * M_PI * I);
/// reflection = cdot(u, v) + h2;
/// reflection = reflection / (1.0 + h2);
/// if (reflection < 0.0) reflection = 0.0;
/// ```
///
/// Ported unchanged, with `angle` and `height` exposed. The defaults
/// are that snippet's own 45 degrees and 1.5.
///
/// WHY NOT FINITE DIFFERENCES: the other way to fake relief is to
/// light the gradient of the iteration count across neighbouring
/// pixels, which works for every formula rather than the 11 with a
/// derivative. It also goes noisy the moment sampling is jittered,
/// which is what Kalles Fraktaler's changelog records fixing by adding
/// analytic differences. This is the version that survives temporal
/// or stochastic sampling; see docs/projects/escape-new-families.md.
pub static NORMAL_MAP: ColoringDef = ColoringDef {
    name: "normal_map",
    display_name: "Normal Map (3D relief)",
    features: &[ColoringFeature::NeedsDerivative, ColoringFeature::Bounded],
    parameters: &[
        EscapeParamDef {
            name: "angle",
            display_name: "Light angle",
            default: 0.125,
            min: 0.0,
            max: 1.0,
            tooltip: "Direction the light comes from, in TURNS (0.125 = 45°), \
                      measured counter-clockwise from the +x axis.",
        },
        EscapeParamDef {
            name: "height",
            display_name: "Light height",
            default: 1.5,
            min: 0.0,
            max: 8.0,
            tooltip: "How high the light sits above the plane. Low values rake \
                      across the surface and exaggerate relief; high values \
                      flatten it toward even illumination.",
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette distance per unit of reflection (reflection runs 0..1).",
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec2<f32>) -> f32 {
    // Without a compiled derivative `dz` is the constant seed, so
    // `z/dz` would be `z` and the shading would be a smooth function
    // of arg(z): plausible relief that encodes nothing about the
    // surface. Return flat illumination instead — an obviously
    // unshaded image beats a convincing wrong one. This is the case on
    // every perturbed render (the deep rungs do not iterate a
    // derivative) and on the 12 formulas that define no derivative.
    if (!HAS_DERIVATIVE) {
        return cparam(2u);
    }
    let dzl = dot(sum.dz, sum.dz);
    if (dzl < 1e-30) {
        return cparam(2u);
    }
    // u = z / dz, normalised. Complex division by hand: the escape
    // helpers are not in scope inside a coloring.
    let inv = 1.0 / dzl;
    var u = vec2<f32>(
        (sum.z.x * sum.dz.x + sum.z.y * sum.dz.y) * inv,
        (sum.z.y * sum.dz.x - sum.z.x * sum.dz.y) * inv,
    );
    let ul = length(u);
    if (ul < 1e-30) {
        return cparam(2u);
    }
    u = u / ul;
    // Light direction from an angle in TURNS, so no atan2 anywhere:
    // this coloring never evaluates one, and the Metal zero-pair
    // hazard cannot arise.
    let a = cparam(0u) * 6.283185307;
    let v = vec2<f32>(cos(a), sin(a));
    let h2 = cparam(1u);
    var reflection = dot(u, v) + h2;
    reflection = reflection / (1.0 + h2);
    return max(reflection, 0.0) * cparam(2u);
}
"#,
    accum_init: "",
    wgsl_accum: "",
};

/// Position average — the mean POSITION of the orbit, not the mean
/// magnitude.
///
/// The distinction matters for folding maps. McCabe's Butterfly
/// Origami colours each point by *"a weighted average of that list of
/// positions"* — the orbit points themselves — and that average is a
/// 2-D vector whose ANGLE carries the creased, layered-paper
/// structure. Averaging |z| instead (see [`MAGNITUDE_AVERAGE`])
/// collapses that vector to a length and renders the same orbit as
/// concentric contour rings: a kaleidoscope rather than folded paper.
/// Prototyped side by side before this was written.
///
/// `mode` picks which component of the average position becomes the
/// palette coordinate. The source's full mapping is hue AND
/// brightness from the one vector; a palette here is one-dimensional,
/// so the angle is offered as the default (it is the half that
/// carries the seams) and the magnitude as the alternative. Reaching
/// the full 2-D mapping would need a coloring that writes RGB
/// directly, which the escape template does not have.
///
/// The average is UNWEIGHTED. McCabe weights each fold, but a weight
/// per step needs the iteration index inside the accumulator, and
/// only the formula side has that today (`FormulaFeature::NeedsIndex`).
pub static POSITION_AVERAGE: ColoringDef = ColoringDef {
    name: "position_average",
    display_name: "Position Average",
    features: &[ColoringFeature::NeedsOrbitAccum, ColoringFeature::ColorsInterior],
    parameters: &[
        EscapeParamDef {
            name: "mode",
            display_name: "Component",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "0: angle of the average position (carries the fold seams), \
                      1: its distance from the origin.",
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 64.0,
            tooltip: "Palette distance per unit. A whole turn of the angle \
                      spans the palette once at 1.0, but the average position \
                      often sweeps only a narrow arc across a given view -- \
                      0.09 of a turn on the shipped Origami -- so raising this \
                      is how the structure becomes visible.",
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec2<f32>) -> f32 {
    // state is the running SUM of orbit positions; n is how many.
    let n = max(f32(sum.n), 1.0);
    let avg = state / n;
    if (cparam(0u) < 0.5) {
        // Angle. The origin guard matters: atan2 at a zero pair is
        // the Metal fast-math hazard (pi/4 for same-sign zeros, NaN
        // for mixed), and an average position of exactly zero is
        // reachable wherever the orbit is symmetric about it.
        if (dot(avg, avg) < 1e-30) {
            return 0.0;
        }
        return (atan2(avg.y, avg.x) * 0.159154943 + 0.5) * cparam(1u);
    }
    return length(avg) * cparam(1u);
}
"#,
    accum_init: "vec2<f32>(0.0, 0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec2<f32>) -> vec2<f32> {
    return state + z;
}
"#,
};
