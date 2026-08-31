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
        min: 0.000001,
        max: 1.0,
        tooltip: "Palette distance per iteration band. Smaller = broader bands. Deep views need very small values -- the slider reaches 1e-6.",
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
        min: 0.000001,
        max: 1.0,
        tooltip: "Palette distance per iteration. Smaller = broader gradient. Deep views need very small values -- the slider reaches 1e-6.",
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
            min: 0.000001,
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
/// steps mean equal zoom depths of boundary distance.
///
/// NEEDS A COMPILED DERIVATIVE, and says so rather than pretending.
/// Without one `dz` stays at its seed of 1, so `d` collapses to
/// `|z|·ln|z|` — a smooth function of the escape radius alone, which
/// renders as a perfectly plausible banded exterior that is not a
/// distance estimate at all. That is the failure mode this project
/// keeps finding and keeps refusing: a confident wrong answer is worse
/// than a visibly missing one. So the coloring returns a flat value
/// instead, exactly as [`NORMAL_MAP`] returns flat light.
///
/// Two cases reach it: the 13 of 25 formulas that define no
/// derivative, and EVERY perturbed render — the deep rungs do not
/// iterate a derivative orbit at all, so a Mandelbrot dive past
/// `PERTURB_MIN_ZOOM` loses it even though the formula has one. The
/// escape panel says which case you are in.
///
/// A finite-difference distance estimate would cover both (the
/// relief-shading pass already differences the value field for the
/// same reason), and is the obvious way to lift this limitation
/// later; it is not what "distance estimate" has meant here so far.
pub static DISTANCE_ESTIMATE: ColoringDef = ColoringDef {
    name: "distance_estimate",
    display_name: "Distance Estimate",
    features: &[ColoringFeature::NeedsDerivative],
    parameters: &[EscapeParamDef {
        name: "scale",
        display_name: "Scale",
        default: 0.05,
        min: 0.000001,
        max: 1.0,
        tooltip: "Palette distance per doubling of boundary distance.",
    }],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec2<f32>) -> f32 {
    // No derivative compiled => dz is the constant seed, so this
    // would reduce to |z|.ln|z|: a smooth function of the escape
    // radius that looks like a distance estimate and is not one.
    // Flat instead. 0.5 rather than 0.0 deliberately -- pixels that
    // do not escape are painted by the template, not by this
    // function, so returning the palette's bottom would make the
    // exterior blend into the interior and read as "everything is in
    // the set" rather than "this coloring is unavailable".
    if (!HAS_DERIVATIVE) {
        return 0.5;
    }
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
/// Position map — project a colour source onto the folded paper.
///
/// McDonald's port of McCabe's origami colours each pixel by LOOKING
/// UP A SOURCE IMAGE at the orbit's final position ("project an image
/// onto the paper, like tie-dye, then unfold"). The engine's palette
/// is one-dimensional, so the source here is a procedural plasma —
/// two sine waves in x and y — whose frequencies stand in for the
/// image's detail scale. Folds pack many copies of the plane onto the
/// wad, so a moderate frequency already shatters into the bead-chain
/// and rosette ornament of the published images.
///
/// `address_mix` blends in the orbit's BRANCH ADDRESS: a binary
/// fraction accumulating, per iteration, whether the step moved the
/// point (for origami: which folds reflected it — the facet
/// identity). This channel is what survives zooming. The smooth part
/// cannot: an all-isometry orbit makes the final position Lipschitz
/// in the pixel, so its contrast dies linearly with window size —
/// measured dead by zoom ~5 — while the address is piecewise-constant
/// with a jump at every crease, and was still structured at zoom 22.
/// The f32 accumulator retains the last ~24 branch choices, which are
/// the fine ones; the coarse folds are visible in the smooth part
/// anyway.
///
/// On always-moving formulas (every escape-time map) the address is
/// the constant 0.111…₂ and `address_mix` does nothing — this
/// coloring is for folding/conditional formulas.
pub static POSITION_MAP: ColoringDef = ColoringDef {
    name: "position_map",
    display_name: "Position Map",
    features: &[
        ColoringFeature::NeedsOrbitAccum,
        ColoringFeature::ColorsInterior,
        ColoringFeature::Bounded,
    ],
    parameters: &[
        EscapeParamDef {
            name: "freq_x",
            display_name: "Frequency X",
            default: 3.0,
            min: 0.05,
            max: 64.0,
            tooltip: "Horizontal frequency of the projected colour source, in \
                      cycles per unit of the folded plane.",
        },
        EscapeParamDef {
            name: "freq_y",
            display_name: "Frequency Y",
            default: 2.0,
            min: 0.05,
            max: 64.0,
            tooltip: "Vertical frequency of the projected colour source.",
        },
        EscapeParamDef {
            name: "address_mix",
            display_name: "Address mix",
            default: 0.0,
            min: 0.0,
            max: 8.0,
            tooltip: "How strongly the orbit's branch address (which \
                      iterations moved the point) shifts the palette. This is \
                      the channel that keeps detail alive under deep zoom; 0 \
                      is the pure projected source.",
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec2<f32>) -> f32 {
    let z = sum.z;
    let plasma = 0.5
        + 0.25 * sin(6.2831853 * cparam(0u) * z.x)
        + 0.25 * sin(6.2831853 * (cparam(1u) * z.y + 0.3));
    return fract(plasma + state.x * cparam(2u));
}
"#,
    accum_init: "vec2<f32>(0.0, 0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec2<f32>) -> vec2<f32> {
    // Binary branch address: 0.5 into the low bit when this step moved
    // the point. An unmoved point returns from the formula bit-exact,
    // so the comparison is reliable (and it is z against z_prev — two
    // distinct values — not a fast-math-hazard self-compare).
    let moved = select(0.0, 0.5, any(z != z_prev));
    return vec2<f32>(state.x * 0.5 + moved, 0.0);
}
"#,
};

/// Sphere average — the orbit's mean CHORDAL distance to a chosen
/// point of the Riemann sphere.
///
/// From [algorithmic-worlds](https://www.algorithmic-worlds.net/blog/blog.php?Post=20141005):
/// *"Each point on the sphere corresponds to an orbit, and is
/// essentially colored according to the mean distance of the orbit to
/// a given point on the sphere."* The images there iterate a Nova map
/// at exponent 4, which this engine already ships — so that pairing
/// needs only this coloring.
///
/// The distance is the CHORDAL metric, which is the natural one on the
/// sphere and has a closed form in the plane:
///
/// ```text
///   d(z, t) = 2 |z - t| / (sqrt(1+|z|^2) sqrt(1+|t|^2))
/// ```
///
/// so no 3-vector is ever built. It is bounded by 2 and treats
/// infinity as an ordinary point — `d(z, inf) = 2/sqrt(1+|z|^2)` —
/// which is the whole reason to use it here rather than `|z - t|`:
/// on a map whose Julia set is the entire sphere (Lattès), orbits pass
/// arbitrarily far out, and a plane metric would saturate on them
/// while the sphere metric stays honest.
///
/// THE SOURCE IS VAGUE, AND THIS IS NOT A PORT. It says "essentially
/// colored" and states no metric, iteration count or normalization;
/// those are choices made here. Chordal distance and a plain mean are
/// the natural readings, and the result matches the published look,
/// but nothing about this is pinned to his implementation.
///
/// `stride` is his other idea, generalized: *"One can picture the nth
/// iteration of the map by taking into account only every nth point in
/// the orbit when computing the average distance."* Sampling every
/// nth iterate shows the dynamics of `f^n` instead of `f`. It needs
/// the iteration index inside the ACCUMULATOR, which no coloring has
/// — so the index rides the spare state slot, counted per call.
pub static SPHERE_AVERAGE: ColoringDef = ColoringDef {
    name: "sphere_average",
    display_name: "Sphere Average",
    features: &[ColoringFeature::NeedsOrbitAccum, ColoringFeature::ColorsInterior],
    parameters: &[
        EscapeParamDef {
            name: "target_re",
            display_name: "Target (re)",
            default: 0.0,
            min: -8.0,
            max: 8.0,
            tooltip: "The sphere point distances are measured to, as a complex number. \
                      The origin and 1 are the usual choices; a point ON the attractor \
                      picks out where the orbit spends its time.",
        },
        EscapeParamDef {
            name: "target_im",
            display_name: "Target (im)",
            default: 0.0,
            min: -8.0,
            max: 8.0,
            tooltip: "Imaginary part of the target point.",
        },
        EscapeParamDef {
            name: "at_infinity",
            display_name: "Target infinity",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Measure to the sphere's north pole instead: d = 2/sqrt(1+|z|^2). \
                      Infinity is an ordinary point in the chordal metric, so this is a \
                      legitimate target rather than a special case.",
        },
        EscapeParamDef {
            name: "stride",
            display_name: "Iterate stride",
            default: 1.0,
            min: 1.0,
            max: 64.0,
            tooltip: "Average over every nth orbit point only, which shows the dynamics \
                      of f^n rather than f. 1 is the plain orbit.",
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 64.0,
            tooltip: "Palette distance per unit of mean chordal distance. The metric is \
                      bounded by 2, so the whole useful range lands inside one palette \
                      turn at 1.0.",
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec2<f32>) -> f32 {
    // state.x sums the accepted distances; state.y counts CALLS, not
    // accepted samples -- with a stride those differ, and dividing by
    // the wrong one scales the mean by the stride. The accepted count
    // is exactly how many k in [0, calls) satisfy k % stride == 0.
    let stride = max(cparam(3u), 1.0);
    let samples = max(ceil(max(state.y, 0.0) / stride), 1.0);
    return (state.x / samples) * cparam(4u);
}
"#,
    accum_init: "vec2<f32>(0.0, 0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec2<f32>) -> vec2<f32> {
    // state.y counts CALLS, which is what the stride test needs; the
    // accepted-sample count is derived in coloring_map. Two floats of
    // state cannot hold both, and the call index is the one that
    // cannot be reconstructed afterwards.
    let stride = max(u32(cparam(3u)), 1u);
    let calls = u32(state.y);
    if (stride > 1u && (calls % stride) != 0u) {
        return vec2<f32>(state.x, state.y + 1.0);
    }
    let zz = dot(z, z);
    var d: f32;
    if (cparam(2u) >= 0.5) {
        // Chordal distance to infinity (the north pole).
        d = 2.0 * inverseSqrt(1.0 + zz);
    } else {
        let t = vec2<f32>(cparam(0u), cparam(1u));
        let dz = z - t;
        d = 2.0 * sqrt(dot(dz, dz)) * inverseSqrt((1.0 + zz) * (1.0 + dot(t, t)));
    }
    return vec2<f32>(state.x + d, state.y + 1.0);
}
"#,
};

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
                      a twentieth of a turn is typical on Origami -- so \
                      raising this is how the structure becomes visible.",
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
