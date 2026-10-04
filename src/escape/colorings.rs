//! Coloring definitions — orbit summary → palette position.
//!
//! One `static ColoringDef` per coloring, WGSL inline. The template
//! wraps the returned coordinate with `fract()` so colorings can
//! return unbounded ramps and let the palette cycle. Signature:
//! `coloring_map(sum, state)` where `state` is the orbit accumulator, a
//! `vec4` (meaningful only with `NeedsOrbitAccum`; zero otherwise).

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
        choices: &[],
    }],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    return f32(sum.n) * cparam(0u);
}
"#,
    accum_init: "",
    wgsl_accum: "",
    recommended_bailout: None,
    pick_params: &[],
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
        choices: &[],
    }],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    // |z|^2 at escape is > bailout >= 1, so log2 is safe; the max()
    // guards the first-iteration corner (bailout < 1 configs) without
    // any fast-math-hazard idiom (no self-compare, no self-divide).
    let r2 = max(dot(sum.z, sum.z), 1.0000001);
    // Each iteration near escape takes |z| to its power p, the formula's
    // degree at infinity, so the fraction is a log BASE p: base 2 only
    // for the quadratics, and any other degree left a seam at every
    // band. Exactly the old count at p = 2 (no divide by a log2(2.0)
    // that need not round to 1). No log R in it, so it does not move
    // with the bailout.
    let ll = log2(0.5 * log2(r2));
    let frac = select(ll / log2(params.degree), ll, params.degree == 2.0);
    let mu = f32(sum.n) + 1.0 - frac;
    return mu * cparam(0u);
}
"#,
    accum_init: "",
    wgsl_accum: "",
    recommended_bailout: Some(1.0e4),
    pick_params: &[],
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
///
/// Ultra Fractal's richer traps (survey C9), all appended so a saved
/// trap draws as it did: the trap's centre and rotation; a ring, box,
/// line and diamond with a radius; and the reduction -- the closest
/// approach (the original), the farthest, the mean, or the first
/// approach within a threshold.
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
            max: 7.0,
            tooltip: "The shape the orbit is measured against, at the trap's \
                      centre and rotation: a point, the axes' cross, the unit circle, \
                      a logarithmic spiral (golden by default), or a ring, box, line \
                      or diamond of the given radius.",
            choices: &[
                "Point",
                "Cross (axes)",
                "Unit circle",
                "Logarithmic spiral",
                "Ring",
                "Box",
                "Line",
                "Diamond",
            ],
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette distance per unit of trap distance.",
            choices: &[],
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
            choices: &[],
        },
        EscapeParamDef {
            name: "center_re",
            display_name: "Centre (re)",
            default: 0.0,
            min: -4.0,
            max: 4.0,
            tooltip: "Where the trap sits in the plane.",
            choices: &[],
        },
        EscapeParamDef {
            name: "center_im",
            display_name: "Centre (im)",
            default: 0.0,
            min: -4.0,
            max: 4.0,
            tooltip: "Where the trap sits in the plane.",
            choices: &[],
        },
        EscapeParamDef {
            name: "rotation",
            display_name: "Rotation",
            default: 0.0,
            min: -180.0,
            max: 180.0,
            tooltip: "The trap's rotation in degrees, about its centre.",
            choices: &[],
        },
        EscapeParamDef {
            name: "mode",
            display_name: "Reduction",
            default: 0.0,
            min: 0.0,
            max: 3.0,
            tooltip: "Which of the orbit's distances colours the pixel: the \
                      closest approach, the farthest, their mean, or the first \
                      that came within the threshold (the closest if none did).",
            choices: &["Closest", "Farthest", "Average", "First within threshold"],
        },
        EscapeParamDef {
            name: "threshold",
            display_name: "Threshold",
            default: 0.1,
            min: 0.0001,
            max: 4.0,
            tooltip: "How close an iterate must come to count as caught, for the \
                      First reduction.",
            choices: &[],
        },
        EscapeParamDef {
            name: "radius",
            display_name: "Radius",
            default: 1.0,
            min: 0.001,
            max: 4.0,
            tooltip: "The size of the ring, box and diamond, and the line's offset \
                      from the centre.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    // state = (closest, farthest, sum of distances, first within the
    // threshold, or -1).
    let mode = u32(clamp(cparam(6u), 0.0, 3.0));
    var d = state.x;
    if (mode == 1u) {
        d = state.y;
    } else if (mode == 2u) {
        d = state.z / max(f32(sum.n), 1.0);
    } else if (mode == 3u && state.w >= 0.0) {
        d = state.w;
    }
    return d * cparam(1u);
}
"#,
    accum_init: "vec4<f32>(1e30, 0.0, 0.0, -1.0)",
    wgsl_accum: r#"
fn coloring_accum(z_in: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {
    let shape = u32(clamp(cparam(0u), 0.0, 7.0));
    // Into the trap's frame: about its centre, turned by its rotation.
    let a = -cparam(5u) * 0.017453292;
    let ca = cos(a);
    let sa = sin(a);
    let zc = z_in - vec2<f32>(cparam(3u), cparam(4u));
    // Unturned at rotation 0, not turned by sin 0 = 0: an infinite
    // iterate (the exponential families escape to one) times zero is
    // NaN, and a saved trap must draw as it did.
    let z = select(vec2<f32>(zc.x * ca - zc.y * sa, zc.x * sa + zc.y * ca), zc, cparam(5u) == 0.0);
    let rad = cparam(8u);
    var d: f32;
    switch shape {
        case 0u: { d = length(z); }
        case 1u: { d = min(abs(z.x), abs(z.y)); }
        case 2u: { d = abs(length(z) - 1.0); }
        case 4u: { d = abs(length(z) - rad); }
        case 5u: { d = abs(max(abs(z.x), abs(z.y)) - rad); }
        case 6u: { d = abs(z.y - rad); }
        case 7u: { d = abs(abs(z.x) + abs(z.y) - rad); }
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
    // A sample that says nothing (the spiral's centre) leaves every
    // reduction untouched.
    if (d >= 1e29) {
        return state;
    }
    let first = select(state.w, d, state.w < 0.0 && d < cparam(7u));
    return vec4<f32>(min(state.x, d), max(state.y, d), state.z + d, first);
}
"#,
    recommended_bailout: None,
    pick_params: &[],
};

/// Orbit average — the Kali glow (plan §8: "REQUIRED for NonEscaping;
/// optional everywhere"). Running mean of the distance-to-axes trap
/// function; the classic Kaliset look, and a soft organic wash on
/// escaping formulas.
pub static ORBIT_AVERAGE: ColoringDef = ColoringDef {
    name: "orbit_average",
    display_name: "Orbit Average",
    features: &[ColoringFeature::NeedsOrbitAccum, ColoringFeature::ColorsInterior],
    parameters: &[
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette distance per unit of averaged trap value.",
            choices: &[],
        },
        EscapeParamDef {
            name: "deband",
            display_name: "Deband",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Blends the mean with and without the orbit's last term by how \
                      far through its last step the orbit escaped, so the colour \
                      moves smoothly across an iteration boundary instead of \
                      stepping (Ultra Fractal's smoothing of its Triangle \
                      Inequality Average). Works with the escape radius: the \
                      larger the bailout, the more terms there are to blend.",
            choices: &["Off", "On"],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    // state = (sum of min(|re|, |im|), count, last term, -).
    let mean = select(state.x / max(state.y, 1.0), esc_debanded_mean(sum, state), cparam(1u) > 0.5);
    return mean * cparam(0u);
}
"#,
    accum_init: "vec4<f32>(0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {
    let t = min(abs(z.x), abs(z.y));
    return vec4<f32>(state.x + t, state.y + 1.0, t, state.w);
}
"#,
    recommended_bailout: None,
    pick_params: &[("deband", 1.0)],
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
            choices: &[],
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette distance per unit of averaged stripe value.",
            choices: &[],
        },
        EscapeParamDef {
            name: "deband",
            display_name: "Deband",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Blends the mean with and without the orbit's last term by how \
                      far through its last step the orbit escaped, so the colour \
                      moves smoothly across an iteration boundary instead of \
                      stepping (Ultra Fractal's smoothing of its Triangle \
                      Inequality Average). Works with the escape radius: the \
                      larger the bailout, the more terms there are to blend.",
            choices: &["Off", "On"],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    // state = (sum of stripe terms, count, last term, -).
    let mean = select(state.x / max(state.y, 1.0), esc_debanded_mean(sum, state), cparam(2u) > 0.5);
    return mean * cparam(1u);
}
"#,
    accum_init: "vec4<f32>(0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {
    // Skip exact zero: atan2 at a zero pair is the Metal fast-math
    // hazard (garbage or NaN, see CLAUDE.md) — the branch keeps it
    // from ever being evaluated there, and dropping one sample from
    // a mean is invisible.
    if (dot(z, z) < 1e-30) {
        return state;
    }
    let stripe = 0.5 + 0.5 * sin(cparam(0u) * atan2(z.y, z.x));
    return vec4<f32>(state.x + stripe, state.y + 1.0, stripe, state.w);
}
"#,
    recommended_bailout: Some(1.0e4),
    pick_params: &[("deband", 1.0)],
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
            choices: &[],
        },
        EscapeParamDef {
            name: "offset",
            display_name: "Offset",
            default: 0.0,
            min: -10.0,
            max: 10.0,
            tooltip: "Baseline subtracted from the mean before scaling. A Ducks \
                      julia field can span only ~0.2 around a large mean -- offset \
                      to the field's floor, then scale up, to stretch that range \
                      across the palette (the reference images normalize contrast \
                      this way).",
            choices: &[],
        },
        EscapeParamDef {
            name: "deband",
            display_name: "Deband",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Blends the mean with and without the orbit's last term by how \
                      far through its last step the orbit escaped, so the colour \
                      moves smoothly across an iteration boundary instead of \
                      stepping (Ultra Fractal's smoothing of its Triangle \
                      Inequality Average). Works with the escape radius: the \
                      larger the bailout, the more terms there are to blend.",
            choices: &["Off", "On"],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    // state = (sum of |z|, count, last term, -).
    let mean = select(state.x / max(state.y, 1.0), esc_debanded_mean(sum, state), cparam(2u) > 0.5);
    return (mean - cparam(1u)) * cparam(0u);
}
"#,
    accum_init: "vec4<f32>(0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {
    let t = length(z);
    return vec4<f32>(state.x + t, state.y + 1.0, t, state.w);
}
"#,
    recommended_bailout: None,
    pick_params: &[("deband", 1.0)],
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
            choices: &[],
        },
        EscapeParamDef {
            name: "speed",
            display_name: "Speed shading",
            default: 0.01,
            min: 0.0,
            max: 0.2,
            tooltip: "Palette offset per iteration of convergence time, shading within each basin.",
            choices: &[],
        },
        EscapeParamDef {
            name: "key",
            display_name: "Basin key",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "0: Angle buckets - the true basin index, but only when the roots are evenly spaced on a circle (z^p - 1). 1: General - folds in log|z| so roots that share an angle still separate; use it for every other function.",
            choices: &["Angle buckets", "General"],
        },
        EscapeParamDef {
            name: "key_scale",
            display_name: "Key scale",
            default: 0.25,
            min: -2.0,
            max: 2.0,
            tooltip: "How strongly the General key weighs log|z| against the angle. Tune until neighbouring basins stop sharing a colour.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    // Origin guard throughout: never hand atan2 a zero pair (Metal
    // fast-math hazard, CLAUDE.md).
    var t = 0.0;
    if (dot(sum.z, sum.z) > 1e-30) {
        let tau = 6.28318530718;
        let ang = fract(atan2(sum.z.y, sum.z.x) / tau + 1.0);
        if (cparam(2u) < 0.5) {
            // Angle of the final iterate, bucketed into `roots` equal
            // arcs. A converged orbit sits on a root of z^p - 1, so
            // the bucket IS the basin index.
            let roots = clamp(cparam(0u), 2.0, 12.0);
            t = floor(ang * roots + 0.5) / roots;
        } else {
            // General key. Roots that are NOT evenly spaced on a
            // circle share angle buckets -- z^3 - 2z + 2 has one real
            // and two conjugate roots, and sin z - 1's roots all sit
            // ON the real axis at angle 0 or pi -- so fold in log|z|,
            // which separates them by magnitude. This is a
            // DISCRIMINATOR, not a basin index: distinct roots get
            // distinct colours, but the numbering means nothing, and
            // two roots can still collide (turn Key scale until they
            // do not). Smooth in z, so a basin stays flat rather than
            // dissolving into noise the way a hash would.
            //
            // Only for a converged orbit. A point that never reached a
            // root has no root to key on -- its final iterate is just
            // wherever the cap left it -- and keying on that painted
            // the transcendental families' large non-convergent
            // regions as a smooth rainbow that looked like structure
            // and was not. Those points are the INTERESTING ones for
            // z^3 - 2z + 2 (its critical orbit falls into an
            // attracting 2-cycle, so a whole region converges to no
            // root at all), so they get one flat colour of their own.
            if (sum.converged) {
                let lr = 0.5 * log(max(dot(sum.z, sum.z), 1e-30));
                t = fract(ang + lr * cparam(3u));
            } else {
                t = 0.0;
                return t;
            }
        }
    }
    return t + f32(sum.n) * cparam(1u);
}
"#,
    accum_init: "",
    wgsl_accum: "",
    recommended_bailout: None,
    pick_params: &[],
};

/// Triangle-inequality average (plan §8): at each step, where |z|
/// falls between the triangle-inequality bounds built from |z − c|
/// and |c|. The classic wispy-band coloring, formula-agnostic in this
/// generalized form.
pub static TRIANGLE_INEQUALITY: ColoringDef = ColoringDef {
    name: "triangle_inequality",
    display_name: "Triangle Inequality",
    features: &[ColoringFeature::NeedsOrbitAccum, ColoringFeature::ColorsInterior],
    parameters: &[
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette distance per unit of averaged TIA value.",
            choices: &[],
        },
        EscapeParamDef {
            name: "deband",
            display_name: "Deband",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Blends the mean with and without the orbit's last term by how \
                      far through its last step the orbit escaped, so the colour \
                      moves smoothly across an iteration boundary instead of \
                      stepping (Ultra Fractal's smoothing of its Triangle \
                      Inequality Average). Works with the escape radius: the \
                      larger the bailout, the more terms there are to blend.",
            choices: &["Off", "On"],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    // state = (sum of TIA terms, count, last term, -).
    let mean = select(state.x / max(state.y, 1.0), esc_debanded_mean(sum, state), cparam(1u) > 0.5);
    return mean * cparam(0u);
}
"#,
    accum_init: "vec4<f32>(0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {
    let x = length(z - c);
    let mc = length(c);
    let lo = abs(x - mc);
    let hi = x + mc;
    let span = hi - lo;
    if (span < 1e-12) {
        return state;
    }
    let t = clamp((length(z) - lo) / span, 0.0, 1.0);
    return vec4<f32>(state.x + t, state.y + 1.0, t, state.w);
}
"#,
    recommended_bailout: Some(1.0e4),
    pick_params: &[("deband", 1.0)],
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
            choices: &[],
        },
        EscapeParamDef {
            name: "escape_scale",
            display_name: "Escape scale",
            default: 0.02,
            min: 0.0,
            max: 1.0,
            tooltip: "Palette distance per iteration for pixels that escape instead of cycling.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
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
    recommended_bailout: None,
    pick_params: &[],
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
/// Two cases reach it: the 14 of 26 formulas that define no
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
    parameters: &[
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 0.05,
            min: 0.000001,
            max: 1.0,
            tooltip: "Palette distance per doubling of boundary distance (Log), \
                      or per unit of it (Linear, Square root).",
            choices: &[],
        },
        EscapeParamDef {
            name: "mapping",
            display_name: "Mapping",
            default: 0.0,
            min: 0.0,
            max: 2.0,
            tooltip: "How the distance becomes a palette position (Kalles \
                      Fraktaler's distance colourings): Log, equal steps per \
                      doubling; Linear, the distance itself; Square root, between \
                      the two.",
            choices: &["Log", "Linear", "Square root"],
        },
        EscapeParamDef {
            name: "units",
            display_name: "Units",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Measure the distance in the plane, or in pixels. In pixels \
                      the boundary is always about one pixel away, so the \
                      colours hold still as you zoom (as Fraktaler 3 and Kalles \
                      Fraktaler draw it).",
            choices: &["Plane", "Pixels"],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
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
    var d = max(r * log(r) / deriv, 1e-30);
    if (cparam(2u) > 0.5) {
        // In OUTPUT pixels, as Fraktaler 3 measures it, so antialiasing
        // does not move the colours.
        d = max(d * esc_px_per_unit(), 1e-30);
    }
    let mapping = u32(clamp(cparam(1u), 0.0, 2.0));
    if (mapping == 1u) {
        return d * cparam(0u);
    }
    if (mapping == 2u) {
        return sqrt(d) * cparam(0u);
    }
    return -log2(d) * cparam(0u);
}
"#,
    accum_init: "",
    wgsl_accum: "",
    recommended_bailout: Some(1.0e4),
    pick_params: &[],
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
            choices: &[],
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
            choices: &[],
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette distance per unit of reflection (reflection runs 0..1).",
            choices: &[],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    // Without a compiled derivative `dz` is the constant seed, so
    // `z/dz` would be `z` and the shading would be a smooth function
    // of arg(z): plausible relief that encodes nothing about the
    // surface. Return flat illumination instead — an obviously
    // unshaded image beats a convincing wrong one. This is the case on
    // every perturbed render (the deep rungs do not iterate a
    // derivative) and on the 14 formulas that define no derivative.
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
    recommended_bailout: Some(1.0e4),
    pick_params: &[],
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
            choices: &[],
        },
        EscapeParamDef {
            name: "freq_y",
            display_name: "Frequency Y",
            default: 2.0,
            min: 0.05,
            max: 64.0,
            tooltip: "Vertical frequency of the projected colour source.",
            choices: &[],
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
            choices: &[],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    let z = sum.z;
    let plasma = 0.5
        + 0.25 * sin(6.2831853 * cparam(0u) * z.x)
        + 0.25 * sin(6.2831853 * (cparam(1u) * z.y + 0.3));
    return fract(plasma + state.x * cparam(2u));
}
"#,
    accum_init: "vec4<f32>(0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {
    // Binary branch address: 0.5 into the low bit when this step moved
    // the point. An unmoved point returns from the formula bit-exact,
    // so the comparison is reliable (and it is z against z_prev — two
    // distinct values — not a fast-math-hazard self-compare).
    let moved = select(0.0, 0.5, any(z != z_prev));
    return vec4<f32>(state.x * 0.5 + moved, 0.0, state.zw);
}
"#,
    recommended_bailout: None,
    pick_params: &[],
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
            choices: &[],
        },
        EscapeParamDef {
            name: "target_im",
            display_name: "Target (im)",
            default: 0.0,
            min: -8.0,
            max: 8.0,
            tooltip: "Imaginary part of the target point.",
            choices: &[],
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
            choices: &["Origin", "Infinity (north pole)"],
        },
        EscapeParamDef {
            name: "stride",
            display_name: "Iterate stride",
            default: 1.0,
            min: 1.0,
            max: 64.0,
            tooltip: "Average over every nth orbit point only, which shows the dynamics \
                      of f^n rather than f. 1 is the plain orbit.",
            choices: &[],
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
            choices: &[],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    // state.x sums the accepted distances; state.y counts CALLS, not
    // accepted samples -- with a stride those differ, and dividing by
    // the wrong one scales the mean by the stride. The accepted count
    // is exactly how many k in [0, calls) satisfy k % stride == 0.
    let stride = max(cparam(3u), 1.0);
    let samples = max(ceil(max(state.y, 0.0) / stride), 1.0);
    return (state.x / samples) * cparam(4u);
}
"#,
    accum_init: "vec4<f32>(0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {
    // state.y counts CALLS, which is what the stride test needs; the
    // accepted-sample count is derived in coloring_map, since the call
    // index is the one that cannot be reconstructed afterwards.
    let stride = max(u32(cparam(3u)), 1u);
    let calls = u32(state.y);
    if (stride > 1u && (calls % stride) != 0u) {
        return vec4<f32>(state.x, state.y + 1.0, state.zw);
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
    return vec4<f32>(state.x + d, state.y + 1.0, state.zw);
}
"#,
    recommended_bailout: None,
    pick_params: &[],
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
            choices: &["Angle", "Distance"],
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
            choices: &[],
        },
        EscapeParamDef {
            name: "deband",
            display_name: "Deband",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Blends the mean with and without the orbit's last term by how \
                      far through its last step the orbit escaped, so the colour \
                      moves smoothly across an iteration boundary instead of \
                      stepping (Ultra Fractal's smoothing of its Triangle \
                      Inequality Average). Works with the escape radius: the \
                      larger the bailout, the more terms there are to blend.",
            choices: &["Off", "On"],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    // state.xy is the running SUM of orbit positions, n is how many,
    // and state.zw the last of them.
    let n = max(f32(sum.n), 1.0);
    var avg = state.xy / n;
    if (cparam(2u) > 0.5 && n > 1.5) {
        let prev = (state.xy - state.zw) / (n - 1.0);
        avg = mix(prev, avg, esc_escape_fraction(sum));
    }
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
    accum_init: "vec4<f32>(0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(state.xy + z, z);
}
"#,
    recommended_bailout: None,
    pick_params: &[("deband", 1.0)],
};

/// Curvature average (techmatt): the mean turn between consecutive
/// steps of the orbit, `|arg((z_n - z_{n-1}) / (z_{n-1} - z_{n-2}))| / pi`,
/// in 0..1. Smooth where the orbit sweeps, bright where it doubles
/// back -- the texture techmatt Screen-blends over smooth.
pub static CURVATURE_AVERAGE: ColoringDef = ColoringDef {
    name: "curvature_average",
    display_name: "Curvature Average",
    features: &[ColoringFeature::NeedsOrbitAccum, ColoringFeature::ColorsInterior],
    parameters: &[
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette distance per unit of averaged turn (a full reversal is 1).",
            choices: &[],
        },
        EscapeParamDef {
            name: "deband",
            display_name: "Deband",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Blends the mean with and without the orbit's last term by how \
                      far through its last step the orbit escaped, so the colour \
                      moves smoothly across an iteration boundary instead of \
                      stepping.",
            choices: &["Off", "On"],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    // state = (sum of turns, last turn, z_{n-2}). A turn is taken at
    // every call but the first, which has no earlier step to turn
    // from, so there are n - 1 of them.
    let count = max(f32(sum.n) - 1.0, 1.0);
    var mean = state.x / count;
    if (cparam(1u) > 0.5 && count > 1.5) {
        let prev = (state.x - state.y) / (count - 1.0);
        mean = mix(prev, mean, esc_escape_fraction(sum));
    }
    return mean * cparam(0u);
}
"#,
    accum_init: "vec4<f32>(0.0, 0.0, 1e30, 0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {
    // This call's z_prev is the next call's z_{n-2}.
    let next = vec4<f32>(state.x, state.y, z_prev);
    if (state.z > 1e29) {
        return next;
    }
    let a = z - z_prev;
    let b = z_prev - state.zw;
    // A step of zero has no direction: skip it rather than ask atan2
    // about a zero pair (the Metal fast-math hazard, CLAUDE.md).
    if (dot(a, a) < 1e-30 || dot(b, b) < 1e-30) {
        return next;
    }
    // a * conj(b): its argument is the turn from step b to step a.
    let q = vec2<f32>(a.x * b.x + a.y * b.y, a.y * b.x - a.x * b.y);
    let t = abs(atan2(q.y, q.x)) * 0.31830988;
    return vec4<f32>(state.x + t, t, z_prev);
}
"#,
    recommended_bailout: Some(1.0e4),
    pick_params: &[("deband", 1.0)],
};

/// Velocity (techmatt): the mean step length `|z_n - z_{n-1}|` over the
/// orbit. Unbounded -- the last steps of an escaping orbit are as long
/// as the escape radius -- so it is best debanded, and drawn at a
/// small bailout or as a layer.
pub static VELOCITY: ColoringDef = ColoringDef {
    name: "velocity",
    display_name: "Velocity",
    features: &[ColoringFeature::NeedsOrbitAccum, ColoringFeature::ColorsInterior],
    parameters: &[
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 0.25,
            min: 0.001,
            max: 20.0,
            tooltip: "Palette distance per unit of averaged step length.",
            choices: &[],
        },
        EscapeParamDef {
            name: "deband",
            display_name: "Deband",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Blends the mean with and without the orbit's last term by how \
                      far through its last step the orbit escaped, so the colour \
                      moves smoothly across an iteration boundary instead of \
                      stepping.",
            choices: &["Off", "On"],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    // state = (sum of step lengths, count, last step, -).
    let mean = select(state.x / max(state.y, 1.0), esc_debanded_mean(sum, state), cparam(1u) > 0.5);
    return mean * cparam(0u);
}
"#,
    accum_init: "vec4<f32>(0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {
    let t = length(z - z_prev);
    return vec4<f32>(state.x + t, state.y + 1.0, t, state.w);
}
"#,
    recommended_bailout: None,
    pick_params: &[("deband", 1.0)],
};

/// Threads (techmatt): the mean of `exp(-D^2 / w^2)`, `D` the distance
/// from the iterate to the nearer axis -- thin bright filaments where
/// orbits graze the axes. techmatt Adds it over smooth; here it is
/// meant as a texture layer.
pub static THREADS: ColoringDef = ColoringDef {
    name: "threads",
    display_name: "Threads",
    features: &[ColoringFeature::NeedsOrbitAccum, ColoringFeature::ColorsInterior],
    parameters: &[
        EscapeParamDef {
            name: "width",
            display_name: "Thread width",
            default: 0.1,
            min: 0.005,
            max: 2.0,
            tooltip: "How far from an axis an iterate still counts: the w of exp(-D^2 / w^2).",
            choices: &[],
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette distance per unit of averaged thread value.",
            choices: &[],
        },
        EscapeParamDef {
            name: "deband",
            display_name: "Deband",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Blends the mean with and without the orbit's last term by how \
                      far through its last step the orbit escaped, so the colour \
                      moves smoothly across an iteration boundary instead of \
                      stepping.",
            choices: &["Off", "On"],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    // state = (sum of thread terms, count, last term, -).
    let mean = select(state.x / max(state.y, 1.0), esc_debanded_mean(sum, state), cparam(2u) > 0.5);
    return mean * cparam(1u);
}
"#,
    accum_init: "vec4<f32>(0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {
    let d = min(abs(z.x), abs(z.y));
    let w = max(cparam(0u), 1e-6);
    let t = exp(-(d * d) / (w * w));
    return vec4<f32>(state.x + t, state.y + 1.0, t, state.w);
}
"#,
    recommended_bailout: Some(1.0e4),
    pick_params: &[("deband", 1.0)],
};

/// Exponential smoothing (Ultra Fractal): `sum exp(-|z|)` over a
/// diverging orbit, `sum exp(-1/|z - z_prev|)` over a converging one.
/// Each term vanishes as the orbit runs away or settles, so the sum
/// is continuous with no bands -- the standard smooth colouring for
/// Newton and Nova, which otherwise have only Root Basin.
pub static EXPONENTIAL_SMOOTHING: ColoringDef = ColoringDef {
    name: "exponential_smoothing",
    display_name: "Exponential Smoothing",
    features: &[ColoringFeature::NeedsOrbitAccum, ColoringFeature::ColorsInterior],
    parameters: &[
        EscapeParamDef {
            name: "mode",
            display_name: "Orbits",
            default: 2.0,
            min: 0.0,
            max: 2.0,
            tooltip: "Which sum to keep: diverging orbits (exp(-|z|)), converging \
                      ones (exp(-1/|z - z_prev|)), or both, for formulas that do \
                      either.",
            choices: &["Diverging", "Converging", "Both"],
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 0.5,
            min: 0.001,
            max: 20.0,
            tooltip: "Palette distance per unit of the sum.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    return state.x * cparam(1u);
}
"#,
    accum_init: "vec4<f32>(0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {
    let mode = u32(clamp(cparam(0u), 0.0, 2.0));
    var t = 0.0;
    if (mode != 1u) {
        t = t + exp(-length(z));
    }
    if (mode != 0u) {
        // A settled step is 1/0 away from contributing: keep the
        // reciprocal finite and let exp take it to zero.
        t = t + exp(-1.0 / max(length(z - z_prev), 1e-30));
    }
    return vec4<f32>(state.x + t, state.yzw);
}
"#,
    recommended_bailout: None,
    pick_params: &[],
};

/// Decomposition (Ultra Fractal, Fraktaler 3): the angle of z where it
/// escaped, as a palette position. With sectors, the angle is cut into
/// that many flat bands -- 2 is binary decomposition, the sign of
/// `Im z`, whose cells trace the external rays.
pub static DECOMPOSITION: ColoringDef = ColoringDef {
    name: "decomposition",
    display_name: "Decomposition",
    features: &[],
    parameters: &[
        EscapeParamDef {
            name: "sectors",
            display_name: "Sectors",
            default: 0.0,
            min: 0.0,
            max: 16.0,
            tooltip: "0 draws the angle itself; 2 or more cuts it into that many \
                      bands -- 2 is binary decomposition.",
            choices: &[],
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette turns per full turn of the angle.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    // An escaped z is far from the origin, so this atan2 never sees
    // the zero pair Metal's fast-math gets wrong.
    let a = atan2(sum.z.y, sum.z.x) * 0.15915494 + 0.5;
    let m = round(cparam(0u));
    let v = select(a, floor(a * m) / m, m >= 1.5);
    return v * cparam(1u);
}
"#,
    accum_init: "",
    wgsl_accum: "",
    recommended_bailout: Some(4.0),
    pick_params: &[],
};

/// Basic (Ultra Fractal's Basic colouring, its real / imaginary / sum
/// modes): `0.05 (4 + v)` of where the orbit escaped. A classic look
/// that wants the classic bailout of 4, where `v` stays within a few
/// units of zero.
pub static BASIC: ColoringDef = ColoringDef {
    name: "basic",
    display_name: "Basic",
    features: &[],
    parameters: &[
        EscapeParamDef {
            name: "part",
            display_name: "Part",
            default: 0.0,
            min: 0.0,
            max: 2.0,
            tooltip: "Which part of the escaped z: its real part, its imaginary \
                      part, or their sum.",
            choices: &["Real", "Imaginary", "Sum"],
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette distance per unit of 0.05 (4 + v).",
            choices: &[],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    let part = u32(clamp(cparam(0u), 0.0, 2.0));
    var v = sum.z.x;
    if (part == 1u) {
        v = sum.z.y;
    } else if (part == 2u) {
        v = sum.z.x + sum.z.y;
    }
    // The exponential families escape with z infinite, and fract of an
    // infinity is NaN, which draws black: an escape past any meaning
    // reads as zero instead. A NaN fails the comparison, which is the
    // bad-value test main_template.wgsl relies on under Metal's
    // fast-math (CLAUDE.md), unlike a self-compare.
    v = select(0.0, v, abs(v) <= 1e30);
    return 0.05 * (4.0 + v) * cparam(1u);
}
"#,
    accum_init: "",
    wgsl_accum: "",
    recommended_bailout: Some(4.0),
    pick_params: &[],
};

/// Gaussian integer (techmatt, Ultra Fractal): the distance from each
/// iterate to the nearest Gaussian integer -- the lattice point
/// `round(z)` -- reduced over the orbit. Never more than sqrt(2)/2, so
/// it is normalised to 0..1. It colours the interior as well.
pub static GAUSSIAN_INTEGER: ColoringDef = ColoringDef {
    name: "gaussian_integer",
    display_name: "Gaussian Integer",
    features: &[ColoringFeature::NeedsOrbitAccum, ColoringFeature::ColorsInterior],
    parameters: &[
        EscapeParamDef {
            name: "reduction",
            display_name: "Reduction",
            default: 0.0,
            min: 0.0,
            max: 3.0,
            tooltip: "How the orbit's distances become one value: the smallest, \
                      their mean, the largest, or the direction from the lattice \
                      point at the smallest (techmatt's angle at minimum).",
            choices: &["Smallest", "Mean", "Largest", "Angle at smallest"],
        },
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.01,
            max: 20.0,
            tooltip: "Palette distance per unit of the normalised value.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    // state = (smallest distance, sum of distances, largest distance,
    // angle at the smallest); distances normalised by sqrt(2)/2.
    let r = u32(clamp(cparam(0u), 0.0, 3.0));
    var v = state.x;
    if (r == 1u) {
        v = state.y / max(f32(sum.n), 1.0);
    } else if (r == 2u) {
        v = state.z;
    } else if (r == 3u) {
        v = state.w;
    }
    return v * cparam(1u);
}
"#,
    accum_init: "vec4<f32>(1.0, 0.0, 0.0, 0.0)",
    wgsl_accum: r#"
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {
    let g = z - round(z);
    let d = length(g) * 1.4142135;
    var st = vec4<f32>(state.x, state.y + d, max(state.z, d), state.w);
    if (d < state.x) {
        st.x = d;
        // At a lattice point the direction is undefined: 0, and no
        // zero pair reaches atan2.
        st.w = select(0.0, atan2(g.y, g.x) * 0.15915494 + 0.5, d > 1e-6);
    }
    return st;
}
"#,
    recommended_bailout: None,
    pick_params: &[],
};

/// External rays (Fraktaler 3's `flying-fish` example, survey C7): lines
/// where the angle of the escaped z, `T = arg(z) / 2pi`, sits within a
/// band of a whole number of turns -- the external rays the binary
/// decomposition's cells are bounded by. T doubles with each step of a
/// quadratic map, so a fixed band would break at every escape band;
/// narrowing it by `(1/p)^NF`, NF the escape fraction, keeps the lines
/// continuous (F3's `0.05 * pow(0.5, getNF())`, p = 2). 1 on a ray and
/// 0 off it, for a texture layer to draw over another colouring.
pub static EXTERNAL_RAYS: ColoringDef = ColoringDef {
    name: "external_rays",
    display_name: "External Rays",
    features: &[ColoringFeature::Bounded],
    parameters: &[
        EscapeParamDef {
            name: "width",
            display_name: "Width",
            default: 0.05,
            min: 0.001,
            max: 0.5,
            tooltip: "Half the width of a ray, in turns of the escape angle, \
                      where the orbit has just escaped (Fraktaler 3 draws 0.05).",
            choices: &[],
        },
        EscapeParamDef {
            name: "count",
            display_name: "Rays",
            default: 1.0,
            min: 1.0,
            max: 64.0,
            tooltip: "How many rays per turn of the angle: 1 is the one through \
                      angle 0, 2 adds the one through a half turn, and so on.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    // An escaped z is far from the origin: no zero pair reaches atan2.
    let m = max(round(cparam(1u)), 1.0);
    let t = atan2(sum.z.y, sum.z.x) * 0.15915494;
    // Distance in turns from the nearest k/m.
    let off = abs(fract(m * t + 0.5) - 0.5) / m;
    let band = cparam(0u) * pow(1.0 / max(params.degree, 1.0001), esc_escape_fraction(sum));
    return select(0.0, 1.0, off < band);
}
"#,
    accum_init: "",
    wgsl_accum: "",
    recommended_bailout: Some(1.0e4),
    pick_params: &[],
};

/// Rainbow fringe (survey C12), from Fraktaler 3's own example
/// (`examples/rainbow-fringe.f3.toml`, read from source): black on white
/// with a rainbow at the boundary. The hue is the direction of the
/// distance-estimate vector, the saturation and value come from its
/// length `L` in output pixels:
/// `hsv(arg(DE) / 2pi, 1 / (1 + 4L), 2L)`, decoded to linear light and
/// raised to `1/gamma` (the example's gamma is 2).
///
/// Fraktaler 3's DE vector (`hybrid.cc`) is `|z|^2 ln|z| / (z J)`, J the
/// Jacobian of z in pixel coordinates: its length is `|z| ln|z| / |dz|`
/// in pixels (the Milnor estimate without its factor 2), and its
/// direction the CONJUGATE of the screen gradient of |z|^2, since F3's
/// image rows run up as it saves them. In the plane that gradient is
/// `z conj(dz)`; the view rotation turns it onto the screen. So the hue
/// circles the set the way Fraktaler 3's does.
///
/// It needs the derivative orbit, like the distance estimate, and draws
/// flat mid-grey where there is none (the perturbed rungs).
pub static RAINBOW_FRINGE: ColoringDef = ColoringDef {
    name: "rainbow_fringe",
    display_name: "Rainbow Fringe",
    features: &[ColoringFeature::NeedsDerivative, ColoringFeature::DirectColor],
    parameters: &[EscapeParamDef {
        name: "gamma",
        display_name: "Gamma",
        default: 2.0,
        min: 0.1,
        max: 10.0,
        tooltip: "The colour is raised to 1/gamma: higher lifts the fringe's \
                  dark side toward the white. Fraktaler 3's example uses 2.",
        choices: &[],
    }],
    wgsl: r#"
// Fraktaler 3's distance estimate, in output pixels.
fn rainbow_fringe_distance(sum: OrbitSummary) -> f32 {
    let r = max(length(sum.z), 1.0000001);
    return r * log(r) / max(length(sum.dz), 1e-30) * esc_px_per_unit();
}

fn rainbow_fringe_to_linear(c: f32) -> f32 {
    let x = clamp(c, 0.0, 1.0);
    if (x <= 0.04045) {
        return x / 12.92;
    }
    return pow((x + 0.055) / 1.055, 2.4);
}

// The value the relief and auto contrast read: the distance's log, as
// the distance estimate's Log mapping has it.
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    if (!HAS_DERIVATIVE) {
        return 0.0;
    }
    return -log2(max(rainbow_fringe_distance(sum), 1e-30));
}

fn coloring_color(sum: OrbitSummary, state: vec4<f32>, v: f32) -> vec3<f32> {
    // No derivative, no fringe: flat mid-grey, which reads as
    // "unavailable" rather than as a picture.
    if (!HAS_DERIVATIVE) {
        return vec3<f32>(0.21404114);
    }
    let l = rainbow_fringe_distance(sum);
    // The gradient of |z|^2 in the plane, z conj(dz), with dz scaled
    // down first so a huge derivative keeps its direction instead of
    // overflowing; then onto the screen, conj(rotation) times it.
    let m = max(abs(sum.dz.x), abs(sum.dz.y));
    let d = sum.dz / max(m, 1e-30);
    let g = vec2<f32>(sum.z.x * d.x + sum.z.y * d.y, sum.z.y * d.x - sum.z.x * d.y);
    let rot = params.rot_cs;
    let s = vec2<f32>(g.x * rot.x + g.y * rot.y, g.y * rot.x - g.x * rot.y);
    // Fraktaler 3's vector is its conjugate. A zero pair (never for an
    // escaped orbit) reads as hue 0 rather than reaching atan2.
    var hue = 0.0;
    if (max(abs(s.x), abs(s.y)) > 0.0) {
        hue = atan2(-s.y, s.x) / 6.2831855;
        hue = hue - floor(hue);
    }
    let sat = clamp(1.0 / (1.0 + 4.0 * l), 0.0, 1.0);
    let val = clamp(2.0 * l, 0.0, 1.0);
    // hsv2sRGB (lolengine), as Fraktaler 3 has it, then its sRGB decode.
    let k = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(vec3<f32>(hue) + k.xyz) * 6.0 - k.www);
    let srgb = val * mix(k.xxx, clamp(p - k.xxx, vec3<f32>(0.0), vec3<f32>(1.0)), sat);
    let lin = vec3<f32>(
        rainbow_fringe_to_linear(srgb.x),
        rainbow_fringe_to_linear(srgb.y),
        rainbow_fringe_to_linear(srgb.z),
    );
    return pow(lin, vec3<f32>(1.0 / max(cparam(0u), 0.1)));
}
"#,
    accum_init: "",
    wgsl_accum: "",
    recommended_bailout: Some(1.0e4),
    pick_params: &[],
};

/// Infinite waves (survey C13), Kalles Fraktaler's multi-wave colouring
/// (`gl/kf.frag.glsl` `KF_InfiniteWaves`, read from source): no palette,
/// but sinusoids of the iteration value, each driving the hue, the
/// saturation or the brightness. A wave of period `P` is
/// `sin(pi iter / P) / 2 + 1/2`; a NEGATIVE period is the constant
/// `-P / 100`; the waves on each channel are averaged, and a channel
/// with none is 0 -- so with no saturation wave the picture is grey,
/// as in KF2. The iteration value is KF2's `n + 1 - NF` (NF the escape
/// fraction), times `scale` (KF2's 1/Iteration Division) plus `offset`
/// (its Color Offset), after the value transfer (its Color Method).
///
/// Stepped takes the waves at whole iterations (KF2 without Smooth).
/// Blend mixes half the palette back in, indexed as KF2 indexes its
/// palette: a cycle per 1024 of the iteration value. KF2 offers up to
/// 30 waves; six fit the parameter block. A period of 0 is an unused
/// wave. The default three are the ones KF2's bundled
/// `monochrome-de.kfr` carries: 100 on hue, 111 on saturation, 123 on
/// brightness.
pub static INFINITE_WAVES: ColoringDef = ColoringDef {
    name: "infinite_waves",
    display_name: "Infinite Waves",
    features: &[ColoringFeature::DirectColor],
    parameters: &[
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.0001,
            max: 100.0,
            tooltip: "Multiplies the iteration value before the waves read it \
                      (Kalles Fraktaler's 1 / Iteration Division).",
            choices: &[],
        },
        EscapeParamDef {
            name: "offset",
            display_name: "Offset",
            default: 0.0,
            min: -10000.0,
            max: 10000.0,
            tooltip: "Added to the iteration value, which shifts every wave's \
                      phase (Kalles Fraktaler's Color Offset).",
            choices: &[],
        },
        EscapeParamDef {
            name: "smooth",
            display_name: "Waves",
            default: 1.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Smooth follows the continuous iteration value; Stepped \
                      takes it at whole iterations, so every band is flat.",
            choices: &["Stepped", "Smooth"],
        },
        EscapeParamDef {
            name: "blend",
            display_name: "Palette",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Off draws the waves alone. Half mixes the palette in \
                      50/50, a cycle per 1024 of the iteration value, as \
                      Kalles Fraktaler's Blend does.",
            choices: &["Off", "Half"],
        },
        wave_period(0, 100.0),
        wave_channel(0, 0.0),
        wave_period(1, 111.0),
        wave_channel(1, 1.0),
        wave_period(2, 123.0),
        wave_channel(2, 2.0),
        wave_period(3, 0.0),
        wave_channel(3, 0.0),
        wave_period(4, 0.0),
        wave_channel(4, 0.0),
        wave_period(5, 0.0),
        wave_channel(5, 0.0),
    ],
    wgsl: r#"
// Kalles Fraktaler's iteration value.
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    return (f32(sum.n) + 1.0 - esc_escape_fraction(sum)) * cparam(0u) + cparam(1u);
}

// hsv2rgb (lolengine), as Kalles Fraktaler has it: display space.
fn infinite_waves_hsv(c: vec3<f32>) -> vec3<f32> {
    let k = vec4<f32>(1.0, 2.0 / 3.0, 1.0 / 3.0, 3.0);
    let p = abs(fract(c.xxx + k.xyz) * 6.0 - k.www);
    return c.z * mix(k.xxx, clamp(p - k.xxx, vec3<f32>(0.0), vec3<f32>(1.0)), c.y);
}

fn coloring_color(sum: OrbitSummary, state: vec4<f32>, v: f32) -> vec3<f32> {
    let iter = select(floor(v), v, cparam(2u) > 0.5);
    var sums = vec3<f32>(0.0);
    var counts = vec3<f32>(0.0);
    for (var i = 0u; i < 6u; i = i + 1u) {
        let period = cparam(4u + 2u * i);
        if (period == 0.0) {
            continue;
        }
        // sin(pi iter / period), with the argument reduced to a turn
        // first so a large count keeps its phase in f32.
        var g = sin(6.2831855 * fract(iter / (2.0 * period))) * 0.5 + 0.5;
        if (period < 0.0) {
            g = -period / 100.0;
        }
        let ch = u32(clamp(cparam(5u + 2u * i), 0.0, 2.0));
        sums[ch] = sums[ch] + g;
        counts[ch] = counts[ch] + 1.0;
    }
    var rgb = infinite_waves_hsv(sums / max(counts, vec3<f32>(1.0)));
    if (cparam(3u) > 0.5) {
        rgb = mix(rgb, esc_palette_srgb(fract(v / 1024.0)), 0.5);
    }
    // Display space, decoded as the palette is.
    return pow(max(rgb, vec3<f32>(0.0)), vec3<f32>(2.2));
}
"#,
    accum_init: "",
    wgsl_accum: "",
    recommended_bailout: None,
    pick_params: &[],
};

const fn wave_period(i: usize, default: f32) -> EscapeParamDef {
    const NAMES: [&str; 6] = ["wave1", "wave2", "wave3", "wave4", "wave5", "wave6"];
    const LABELS: [&str; 6] = ["Wave 1", "Wave 2", "Wave 3", "Wave 4", "Wave 5", "Wave 6"];
    EscapeParamDef {
        name: NAMES[i],
        display_name: LABELS[i],
        default,
        min: -100.0,
        max: 100000.0,
        tooltip: "The wave's period in iterations (a full cycle is twice \
                  it). Negative is a constant, -period/100. 0 is no wave.",
        choices: &[],
    }
}

const fn wave_channel(i: usize, default: f32) -> EscapeParamDef {
    const NAMES: [&str; 6] = ["wave1_channel", "wave2_channel", "wave3_channel", "wave4_channel", "wave5_channel", "wave6_channel"];
    const LABELS: [&str; 6] = ["Wave 1 drives", "Wave 2 drives", "Wave 3 drives", "Wave 4 drives", "Wave 5 drives", "Wave 6 drives"];
    EscapeParamDef {
        name: NAMES[i],
        display_name: LABELS[i],
        default,
        min: 0.0,
        max: 2.0,
        tooltip: "Which part of the colour the wave drives. The waves on \
                  each are averaged; one with none is 0 (no saturation \
                  wave is grey).",
        choices: &["Hue", "Saturation", "Brightness"],
    }
}

/// Itinerary (survey C8), from techmatt's engine (`iterate.rs`
/// `Address`, `mode.rs` `itinerary`, read from source): the orbit's
/// angular address. Each iterate lands in one of `sectors` equal
/// sectors, counted counter-clockwise from the negative real axis
/// (`floor((atan2(y, x) + pi) / 2pi * k)`), and the sectors are the
/// digits of one base-k fraction. Two pixels whose orbits take the same
/// route through the sectors get nearly the same number however long
/// they run, so the field follows the fractal's own self-similar
/// lamination rather than its escape time.
///
/// Head (the default) is the first `depth` symbols; Tail rolls, keeping
/// the last `depth` before the orbit stopped
/// (`fract(v * k) + s * k^-depth`). Both open on z1. techmatt opens a
/// head on z0 where z0 is a constant (the parameter plane), which
/// prepends one constant digit: an affine change the frame stretch it
/// always applies removes, so stretched the two are the same.
///
/// **In f32 the address holds 12 base-4 symbols exactly** (24 bits;
/// techmatt keeps 26 in f64). techmatt's itinerary mode is this as a
/// texture layer, Add at 0.5, over smooth with Equalize: Auto contrast
/// stretches the layer to its own range in the frame, which is what
/// keeps the texture as you zoom -- until the frame's addresses share
/// so many leading symbols that 12 leave too few to tell apart.
pub static ITINERARY: ColoringDef = ColoringDef {
    name: "itinerary",
    display_name: "Itinerary",
    features: &[ColoringFeature::NeedsOrbitAccum, ColoringFeature::ColorsInterior],
    parameters: &[
        EscapeParamDef {
            name: "scale",
            display_name: "Scale",
            default: 1.0,
            min: 0.001,
            max: 100.0,
            tooltip: "Palette turns across the whole address range, 0 to 1.",
            choices: &[],
        },
        EscapeParamDef {
            name: "sectors",
            display_name: "Sectors",
            default: 4.0,
            min: 2.0,
            max: 16.0,
            tooltip: "How many equal angular sectors the plane is cut into: \
                      each step's sector is the next digit of the address, in \
                      base sectors.",
            choices: &[],
        },
        EscapeParamDef {
            name: "depth",
            display_name: "Symbols",
            default: 12.0,
            min: 1.0,
            max: 24.0,
            tooltip: "How many symbols the address holds. f32 holds 24 bits: \
                      12 symbols at 4 sectors, 8 at 8. Past that the deepest \
                      symbols round.",
            choices: &[],
        },
        EscapeParamDef {
            name: "window",
            display_name: "Window",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Head: the first symbols the orbit spells. Tail: the last \
                      ones before it stopped, which puts the structure where \
                      the orbits are long.",
            choices: &["Head", "Tail"],
        },
    ],
    wgsl: r#"
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    return state.x * cparam(0u);
}
"#,
    accum_init: "vec4<f32>(0.0, 0.0, 0.0, 0.0)",
    wgsl_accum: r#"
// state: x the address, y the symbols spelled so far, z the weight the
// next head symbol carries, w the tail's bottom place k^-depth. The two
// place values are built by division on the first step rather than by
// pow, which GPUs approximate: at a power-of-two k they are then exact,
// as the address is.
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {
    let k = max(floor(cparam(1u)), 2.0);
    let depth = max(floor(cparam(2u)), 1.0);
    var st = state;
    if (st.y == 0.0) {
        st.z = 1.0 / k;
        var bottom = 1.0;
        for (var i = 0.0; i < depth; i = i + 1.0) {
            bottom = bottom / k;
        }
        st.w = bottom;
    }
    // The sector, counter-clockwise from the negative real axis. The
    // origin reads as angle 0 (techmatt's +0 seed) rather than reaching
    // atan2 with a zero pair (CLAUDE.md, Metal).
    var turns = 0.5;
    if (max(abs(z.x), abs(z.y)) > 0.0) {
        turns = (atan2(z.y, z.x) + 3.14159265) / 6.2831855;
    }
    let sector = clamp(floor(turns * k), 0.0, k - 1.0);
    var out = st;
    if (cparam(3u) > 0.5) {
        // Tail: shift the window up a place, drop the oldest symbol,
        // write this one at the bottom.
        out.x = fract(st.x * k) + sector * st.w;
    } else if (st.y < depth) {
        out.x = st.x + sector * st.z;
        out.z = st.z / k;
    }
    out.y = st.y + 1.0;
    return out;
}
"#,
    recommended_bailout: None,
    pick_params: &[],
};

/// Visions of Chaos orbit traps, ported from Softology's own shader
/// listings (softology.pro `Mandelbrot_{Circles,Crosses,Rings,Squares,
/// Stalks}_Orbit_Traps.txt`, read from source; the blog post
/// "Orbit Traps", 2011, describes them).
///
/// Every listing is the same loop: step, stop if the new iterate
/// escapes (so the escaping iterate is never tested), take it, test the
/// trap, and stop at the FIRST iterate the trap catches. A caught pixel
/// is grey by how close that iterate came; one that escaped uncaught
/// takes the CPM smooth colouring, `i + 1 - log2(ln|z|)` through a
/// 256-entry palette indexed `mod 255` (entry 255 is never used, and
/// 254 blends into 0); the interior is black.
///
/// The shapes, as coded:
/// - Circles: `d < size` from the trap centre; grey `1 - d/size`.
/// - Crosses: `|x| < size`, else `|y| < size` (x first), at the origin;
///   grey `1 - d/size`.
/// - Rings: `min < d < max` from the centre; grey a triangle, 1 midway.
/// - Squares: inside the square of half-size `size` about the centre;
///   `d = (|x - X| + |y - Y|) / 2`, grey `1 - d/size`.
/// - Stalks: `||z| - |c|| <= radius`, from z3 on; grey
///   `min(1, sqrt(1 - ||z| - |c||/radius))`. (The listing accumulates
///   `ztot = sqrt(ztot) + ...`, but it stops at the first catch, so the
///   sum only ever holds one term.)
///
/// Kept: circles and rings measure from (trap Y, trap X) -- the listing
/// swaps the names -- so with its trap at X 0, Y 0.5 they sit at
/// (0.5, 0), while squares sit at (0, 0.5) as named.
///
/// Not kept: VoC stops iterating at the catch; here the orbit runs on
/// with the catch frozen, which costs time, not colour. VoC averages
/// its supersamples in display space, the escape renderer in linear
/// light. And the uncaught exterior follows this renderer's escape
/// count `n`, VoC's loop index plus one.
pub static VOC_TRAPS: ColoringDef = ColoringDef {
    name: "voc_traps",
    display_name: "Visions of Chaos Traps",
    features: &[ColoringFeature::NeedsOrbitAccum, ColoringFeature::ColorsInterior, ColoringFeature::DirectColor],
    parameters: &[
        EscapeParamDef {
            name: "shape",
            display_name: "Trap",
            default: 0.0,
            min: 0.0,
            max: 4.0,
            tooltip: "Which of Visions of Chaos's orbit traps: circles, crosses, \
                      rings, squares or stalks.",
            choices: &["Circles", "Crosses", "Rings", "Squares", "Stalks"],
        },
        EscapeParamDef {
            name: "trap_x",
            display_name: "Trap X",
            default: 0.0,
            min: -4.0,
            max: 4.0,
            tooltip: "The trap's centre, as Visions of Chaos names it. Circles \
                      and rings read it with X and Y swapped, as the original \
                      does, so the default puts them at (0.5, 0) and squares at \
                      (0, 0.5). Crosses and stalks do not use it.",
            choices: &[],
        },
        EscapeParamDef {
            name: "trap_y",
            display_name: "Trap Y",
            default: 0.5,
            min: -4.0,
            max: 4.0,
            tooltip: "The trap's centre, as Visions of Chaos names it (see Trap X).",
            choices: &[],
        },
        EscapeParamDef {
            name: "circle_size",
            display_name: "Circle radius",
            default: 0.5,
            min: 0.001,
            max: 4.0,
            tooltip: "Circles: how far from the centre an iterate is caught.",
            choices: &[],
        },
        EscapeParamDef {
            name: "cross_size",
            display_name: "Cross width",
            default: 0.05,
            min: 0.001,
            max: 4.0,
            tooltip: "Crosses: how near an axis an iterate is caught.",
            choices: &[],
        },
        EscapeParamDef {
            name: "ring_min",
            display_name: "Ring inner radius",
            default: 0.4,
            min: 0.0,
            max: 4.0,
            tooltip: "Rings: the inner edge of the band that catches.",
            choices: &[],
        },
        EscapeParamDef {
            name: "ring_max",
            display_name: "Ring outer radius",
            default: 0.5,
            min: 0.001,
            max: 4.0,
            tooltip: "Rings: the outer edge of the band that catches.",
            choices: &[],
        },
        EscapeParamDef {
            name: "square_size",
            display_name: "Square half-size",
            default: 0.4,
            min: 0.001,
            max: 4.0,
            tooltip: "Squares: half the side of the square that catches.",
            choices: &[],
        },
        EscapeParamDef {
            name: "stalk_radius",
            display_name: "Stalk width",
            default: 0.05,
            min: 0.001,
            max: 4.0,
            tooltip: "Stalks: how near |z| must come to |c| to be caught.",
            choices: &[],
        },
    ],
    wgsl: r#"
// Caught: the grey. Escaped uncaught: Visions of Chaos's smooth palette
// position, for the relief and auto contrast to read.
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    if (state.x > 0.5) {
        return state.y;
    }
    let r = max(length(sum.z), 1.0000001);
    let real = f32(sum.n) - log2(log(r));
    return (real - 255.0 * floor(real / 255.0)) / 256.0;
}

fn coloring_color(sum: OrbitSummary, state: vec4<f32>, v: f32) -> vec3<f32> {
    if (state.x > 0.5) {
        let g = clamp(state.y, 0.0, 1.0);
        return vec3<f32>(pow(g, 2.2));
    }
    if (!sum.escaped) {
        return vec3<f32>(0.0);
    }
    // CPM smooth colours: i + 1 - log(log|z|)/log 2, i the loop index
    // the escape broke on -- this renderer's n less one.
    let r = max(length(sum.z), 1.0000001);
    let real = f32(sum.n) - log2(log(r));
    let m = real - 255.0 * floor(real / 255.0);
    let colval = floor(m);
    let colval2 = select(colval + 1.0, 0.0, colval + 1.0 >= 255.0);
    let tween = real - trunc(real);
    // Palette entries at their texel centres, blended by hand: entry 254
    // blends into entry 0, which a single sample cannot do.
    let a = esc_palette_srgb((colval + 0.5) / 256.0);
    let b = esc_palette_srgb((colval2 + 0.5) / 256.0);
    return pow(max(mix(a, b, tween), vec3<f32>(0.0)), vec3<f32>(2.2));
}
"#,
    accum_init: "vec4<f32>(0.0, 0.0, 0.0, 0.0)",
    wgsl_accum: r#"
// state: x caught (1) or not, y the grey at the catch, z the loop index
// of this step (0 for z1).
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {
    var st = state;
    let i = st.z;
    st.z = st.z + 1.0;
    // Caught already: frozen, as the original stops there. The escaping
    // iterate is never tested: the original breaks before it.
    if (state.x > 0.5 || dot(z, z) > params.bailout) {
        return st;
    }
    let shape = u32(clamp(cparam(0u), 0.0, 4.0));
    let tx = cparam(1u);
    let ty = cparam(2u);
    var caught = false;
    var grey = 0.0;
    switch shape {
        case 0u: {
            // Circles, from (ty, tx) as the listing has it.
            let size = cparam(3u);
            let d = sqrt((z.x - ty) * (z.x - ty) + (z.y - tx) * (z.y - tx));
            if (d < size) {
                caught = true;
                grey = 1.0 - d / size;
            }
        }
        case 1u: {
            // Crosses: the real axis's band first.
            let size = cparam(4u);
            if (abs(z.x) < size) {
                caught = true;
                grey = 1.0 - abs(z.x) / size;
            } else if (abs(z.y) < size) {
                caught = true;
                grey = 1.0 - abs(z.y) / size;
            }
        }
        case 2u: {
            // Rings, from (ty, tx); a triangle across the band.
            let lo = cparam(5u);
            let hi = cparam(6u);
            let d = sqrt((z.x - ty) * (z.x - ty) + (z.y - tx) * (z.y - tx));
            if (d < hi && d > lo) {
                caught = true;
                var p = (d - lo) / (hi - lo);
                if (p > 0.5) {
                    p = 0.5 - (p - 0.5);
                }
                grey = p * 2.0;
            }
        }
        case 3u: {
            // Squares, about (tx, ty) as named.
            let size = cparam(7u);
            if (z.x > tx - size && z.x < tx + size && z.y > ty - size && z.y < ty + size) {
                caught = true;
                grey = 1.0 - ((abs(z.x - tx) + abs(z.y - ty)) / 2.0) / size;
            }
        }
        default: {
            // Stalks: |z| within the radius of |c|, from z3 on.
            let rad = cparam(8u);
            let m = length(z);
            let rc = length(c);
            if (m <= rc + rad && m >= rc - rad && i > 1.0) {
                caught = true;
                grey = min(1.0, sqrt(1.0 - abs(m - rc) / rad));
            }
        }
    }
    if (caught) {
        st.x = 1.0;
        st.y = grey;
    }
    return st;
}
"#,
    recommended_bailout: None,
    pick_params: &[],
};

/// Direct orbit traps (survey C10), from techmatt's engine
/// (`direct_trap.rs`, the `direct_trap_*` modes in `mode.rs`, read from
/// source); Ultra Fractal's Direct Orbit Traps are the same idea with
/// more options. It never makes a value. Every iterate that comes within
/// `threshold` of the shape takes a sample from the palette and
/// composites it into the pixel, in the order the orbit made them, so the
/// colour is the stack of every near miss -- the lacy, overlapping look.
///
/// Per near miss, as techmatt has it: `key = transform(d / threshold)`
/// picks the sample (a close approach reads from the bottom of the
/// palette) and feathers it, `alpha = opacity (1 - key)`; then, per
/// channel in linear light, `blend(standing, sample)` (or the sample
/// under the standing colour, Top down) mixed in by `alpha`. It starts
/// from black or white, paints the interior too, and tests the escaping
/// iterate as well (the trap is checked before the escape).
///
/// Kept: the palette is read clamped, not wrapped, on the same
/// `i/(N-1)` table positions; a threshold of 0 takes the shape's own
/// default (techmatt's per-shape calibration); and a Screen cross is
/// held to opacity 0.15 and threshold 0.08, where techmatt measured it
/// blowing out to white. The defaults are his `direct_trap_ring`.
///
/// The palette is sampled inside the loop, so a palette edit
/// re-iterates (`PaletteInLoop`).
pub static DIRECT_TRAPS: ColoringDef = ColoringDef {
    name: "direct_traps",
    display_name: "Direct Orbit Traps",
    features: &[
        ColoringFeature::NeedsOrbitAccum,
        ColoringFeature::ColorsInterior,
        ColoringFeature::DirectColor,
        ColoringFeature::PaletteInLoop,
    ],
    parameters: &[
        EscapeParamDef {
            name: "shape",
            display_name: "Trap shape",
            default: 1.0,
            min: 0.0,
            max: 7.0,
            tooltip: "The shape each iterate is measured against, at the origin: \
                      a point (beads), a ring of the given radius (overlapping \
                      scales), the axes' cross, the cross with its diagonals, \
                      diamond and square contours, a four-pointed astroid, or the \
                      real axis alone (horizontal bands).",
            choices: &["Point", "Ring", "Cross", "Hypercross", "Diamond", "Box", "Astroid", "Lines"],
        },
        EscapeParamDef {
            name: "radius",
            display_name: "Ring radius",
            default: 1.0,
            min: 0.01,
            max: 4.0,
            tooltip: "The ring's radius (the other shapes do not use it).",
            choices: &[],
        },
        EscapeParamDef {
            name: "threshold",
            display_name: "Threshold",
            default: 0.0597,
            min: 0.0,
            max: 4.0,
            tooltip: "How near an iterate must come to paint. 0 takes the shape's \
                      own default, calibrated so every shape paints about the \
                      same share of a frame: point 0.6, ring 0.078, cross 0.1, \
                      hypercross 0.074, diamond 0.8, box 0.55, astroid 1.06, \
                      lines 0.127.",
            choices: &[],
        },
        EscapeParamDef {
            name: "opacity",
            display_name: "Opacity",
            default: 0.45,
            min: 0.0,
            max: 1.0,
            tooltip: "How strongly each near miss paints, before its feathering \
                      by closeness. A Screen cross is held to 0.15, where it \
                      would otherwise blow out to white.",
            choices: &[],
        },
        EscapeParamDef {
            name: "blend",
            display_name: "Blend",
            default: 2.0,
            min: 0.0,
            max: 5.0,
            tooltip: "How each sample merges with the colour so far: Screen \
                      brightens (start from black), Multiply darkens (start from \
                      white), Overlay does both, Min clamps, Add lifts by the same \
                      amount everywhere, Normal replaces.",
            choices: &["Normal", "Multiply", "Screen", "Overlay", "Min", "Add"],
        },
        EscapeParamDef {
            name: "order",
            display_name: "Order",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Bottom up: each near miss goes over the ones before it. Top \
                      down: the earliest stays on top.",
            choices: &["Bottom up", "Top down"],
        },
        EscapeParamDef {
            name: "start",
            display_name: "Start",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "The colour the samples land on, where no near miss paints.",
            choices: &["Black", "White"],
        },
        EscapeParamDef {
            name: "transform",
            display_name: "Curve",
            default: 0.0,
            min: 0.0,
            max: 3.0,
            tooltip: "A curve on the closeness before it picks the sample and its \
                      feathering.",
            choices: &["Linear", "Square root", "Log", "S-curve"],
        },
    ],
    wgsl: r#"
// The value the relief and auto contrast read: the colour's luminance.
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    return dot(state.xyz, vec3<f32>(0.2126, 0.7152, 0.0722));
}

fn coloring_color(sum: OrbitSummary, state: vec4<f32>, v: f32) -> vec3<f32> {
    return max(state.xyz, vec3<f32>(0.0));
}
"#,
    accum_init: "vec4<f32>(vec3<f32>(select(0.0, 1.0, cparam(6u) > 0.5)), 0.0)",
    wgsl_accum: r#"
// Distance from an iterate to the shape, at the origin.
fn direct_traps_distance(shape: u32, z: vec2<f32>, radius: f32) -> f32 {
    let re = abs(z.x);
    let im = abs(z.y);
    switch shape {
        case 0u: { return length(z); }
        case 1u: { return abs(length(z) - radius); }
        case 3u: {
            let diagonal = min(abs(z.x - z.y), abs(z.x + z.y)) * 0.70710678;
            return min(min(re, im), diagonal);
        }
        case 4u: { return re + im; }
        case 5u: { return max(re, im); }
        case 6u: {
            // (|x|^(2/3) + |y|^(2/3))^(3/2), with the zeros written out
            // rather than handed to pow.
            let a = select(0.0, pow(re, 2.0 / 3.0), re > 0.0);
            let b = select(0.0, pow(im, 2.0 / 3.0), im > 0.0);
            return select(0.0, pow(a + b, 1.5), a + b > 0.0);
        }
        case 7u: { return im; }
        default: { return min(re, im); }
    }
}

// techmatt's per-shape thresholds, each calibrated to paint about the
// share of a frame the cross does at 0.1.
fn direct_traps_default_threshold(shape: u32) -> f32 {
    switch shape {
        case 0u: { return 0.60; }
        case 1u: { return 0.078; }
        case 3u: { return 0.074; }
        case 4u: { return 0.80; }
        case 5u: { return 0.55; }
        case 6u: { return 1.06; }
        case 7u: { return 0.127; }
        default: { return 0.10; }
    }
}

fn direct_traps_blend(blend: u32, under: f32, over: f32) -> f32 {
    switch blend {
        case 0u: { return over; }
        case 1u: { return under * over; }
        case 3u: {
            return select(1.0 - 2.0 * (1.0 - under) * (1.0 - over), 2.0 * under * over, under < 0.5);
        }
        case 4u: { return min(under, over); }
        case 5u: { return min(under + over, 1.0); }
        default: { return 1.0 - (1.0 - under) * (1.0 - over); }
    }
}

// The palette at `key`, CLAMPED as techmatt's colormap lookup is (not
// wrapped): position key (N - 1) over the table's N entries, which sit
// at i/(N-1), read with the sampler's linear filter at texel centres.
fn direct_traps_sample(key: f32) -> vec3<f32> {
    let k = clamp(esc_palette_curve(key), 0.0, 1.0);
    let n = f32(textureDimensions(palette_texture).x);
    let u = (k * (n - 1.0) + 0.5) / n;
    let srgb = textureSampleLevel(palette_texture, palette_sampler, vec2<f32>(u, 0.5), 0.0).rgb;
    return pow(max(srgb, vec3<f32>(0.0)), vec3<f32>(2.2));
}

// state.xyz: the colour so far, in linear light.
fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {
    let shape = u32(clamp(cparam(0u), 0.0, 7.0));
    let blend = u32(clamp(cparam(4u), 0.0, 5.0));
    var threshold = select(direct_traps_default_threshold(shape), cparam(2u), cparam(2u) > 0.0);
    var opacity = clamp(cparam(3u), 0.0, 1.0);
    if (blend == 2u && shape == 2u) {
        opacity = min(opacity, 0.15);
        threshold = min(threshold, 0.08);
    }
    threshold = max(threshold, 1e-12);
    let d = direct_traps_distance(shape, z, cparam(1u));
    // Negated, so a non-finite distance paints nothing (CLAUDE.md).
    if (!(d < threshold)) {
        return state;
    }
    var key = clamp(d / threshold, 0.0, 1.0);
    switch u32(clamp(cparam(7u), 0.0, 3.0)) {
        case 1u: { key = sqrt(key); }
        case 2u: { key = log2(1.0 + key); }
        case 3u: { key = key * key * (3.0 - 2.0 * key); }
        default: {}
    }
    let sample = direct_traps_sample(key);
    let alpha = opacity * (1.0 - key);
    let top_down = cparam(5u) > 0.5;
    var col = state.xyz;
    for (var k = 0u; k < 3u; k = k + 1u) {
        let blended = select(
            direct_traps_blend(blend, col[k], sample[k]),
            direct_traps_blend(blend, sample[k], col[k]),
            top_down,
        );
        col[k] = blended * alpha + col[k] * (1.0 - alpha);
    }
    return vec4<f32>(col, state.w);
}
"#,
    // techmatt's escape radius, 2^16, squared.
    recommended_bailout: Some(4_294_967_296.0),
    pick_params: &[],
};

/// Ultra Fractal's Image Trap (common.ulb `ColorTrapImage`), drawn the
/// way its Direct Orbit Traps colouring draws a colour trap
/// (Standard.ulb `Standard_DirectOrbitTraps`), with common.ulb's
/// `TrapTransform` for the trap's position. All read from source, as is
/// UF's documentation of the built-ins they call.
/// - Each iterate z, passed through the trap position
///   (`(z - centre) * rotation / scale`, then aspect and skew), reads the
///   image with `Image.getColor`. The image spans (-1,-1) to (1,1), with
///   (-1,-1) its bottom-left corner. With Keep proportions
///   (`ImageWrapper.NormalizePixel`), the shorter side is shrunk to keep
///   the image's own proportions. Outside the image it is fully
///   transparent.
/// - That colour merges into the colour so far with `FullMerge`:
///   `compose(bottom, blend(top, mergeX(bottom, top), alpha(bottom)),
///   opacity)`. Bottom-up puts each new colour on top; top-down puts it
///   underneath. It starts from the base colour, opaque.
/// - The colour is UF's, in display values, with its alpha. Top-down
///   can leave it partly transparent, and the background then shows
///   through, as a layer below does in UF (`DirectAlpha`).
/// - As UF's loop section, it sees only the iterates that did not bail
///   out (`SkipsEscapingIterate`).
///
/// The image is the config's simulation texture
/// (docs/projects/sim-textures.md). It is opaque, so the trap is its
/// rectangle.
///
/// Not ported:
/// - UF reads the image bicubically; this uses the bilinear sampler.
/// - The merge modes UF documents without a formula: Overlay, Hard and
///   Soft Light, and the HSL modes.
/// - TrapTransform's per-iteration steps (drift, rotation step, skew
///   step) and "follows initial z", which need the iteration's index or
///   start.
pub static IMAGE_TRAP: ColoringDef = ColoringDef {
    name: "image_trap",
    display_name: "Image Trap",
    features: &[
        ColoringFeature::NeedsOrbitAccum,
        ColoringFeature::ColorsInterior,
        ColoringFeature::DirectColor,
        ColoringFeature::TextureInLoop,
        ColoringFeature::DirectAlpha,
        ColoringFeature::SkipsEscapingIterate,
    ],
    parameters: &[
        EscapeParamDef {
            name: "trap_re",
            display_name: "Trap center (re)",
            default: 0.0,
            min: -4.0,
            max: 4.0,
            tooltip: "Where the image sits in the complex plane: its centre.",
            choices: &[],
        },
        EscapeParamDef {
            name: "trap_im",
            display_name: "Trap center (im)",
            default: 0.0,
            min: -4.0,
            max: 4.0,
            tooltip: "Where the image sits in the complex plane: its centre.",
            choices: &[],
        },
        EscapeParamDef {
            name: "trap_scale",
            display_name: "Trap scale",
            default: 1.0,
            min: 0.01,
            max: 10.0,
            tooltip: "The image's size. At 1 it spans -1 to 1; smaller shrinks it.",
            choices: &[],
        },
        EscapeParamDef {
            name: "trap_rotation",
            display_name: "Rotation",
            default: 0.0,
            min: -180.0,
            max: 180.0,
            tooltip: "The image's angle, in degrees.",
            choices: &[],
        },
        EscapeParamDef {
            name: "trap_aspect",
            display_name: "Aspect ratio",
            default: 1.0,
            min: 0.1,
            max: 10.0,
            tooltip: "Squeezes the image vertically above 1, stretches it below.",
            choices: &[],
        },
        EscapeParamDef {
            name: "trap_skew",
            display_name: "Skew",
            default: 0.0,
            min: -80.0,
            max: 80.0,
            tooltip: "Slants the image's vertical axis, in degrees.",
            choices: &[],
        },
        EscapeParamDef {
            name: "proportions",
            display_name: "Proportions",
            default: 1.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Keep the image's own proportions, or stretch it to a square.",
            choices: &["Stretch to square", "Keep proportions"],
        },
        EscapeParamDef {
            name: "opacity",
            display_name: "Trap merge opacity",
            default: 0.2,
            min: 0.0,
            max: 1.0,
            tooltip: "How strongly each iterate's colour merges. Low values stack \
                      many translucent copies; 1 lets one copy cover the rest.",
            choices: &[],
        },
        EscapeParamDef {
            name: "order",
            display_name: "Trap merge order",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Bottom-up merges each new iterate's colour on top of the \
                      ones before; top-down merges it underneath, so the earliest \
                      stays on top. Top-down leaves the colour partly transparent \
                      where iterates miss the image, and the background shows.",
            choices: &["Bottom-up", "Top-down"],
        },
        EscapeParamDef {
            name: "merge",
            display_name: "Trap color merge",
            default: 0.0,
            min: 0.0,
            max: 10.0,
            tooltip: "How an iterate's colour combines with the colour so far, as \
                      Ultra Fractal's layer merge modes do.",
            choices: &[
                "Normal",
                "Multiply",
                "Screen",
                "Darken",
                "Lighten",
                "Difference",
                "Addition",
                "Subtraction",
                "Red",
                "Green",
                "Blue",
            ],
        },
        EscapeParamDef {
            name: "base_r",
            display_name: "Base color red",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "The colour the iterates' colours merge onto.",
            choices: &[],
        },
        EscapeParamDef {
            name: "base_g",
            display_name: "Base color green",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "The colour the iterates' colours merge onto.",
            choices: &[],
        },
        EscapeParamDef {
            name: "base_b",
            display_name: "Base color blue",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "The colour the iterates' colours merge onto.",
            choices: &[],
        },
    ],
    wgsl: r#"
// The value the relief and auto contrast read: the colour's luminance.
fn coloring_map(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    return dot(state.xyz, vec3<f32>(0.2126, 0.7152, 0.0722));
}

// UF's colour is in display values; the output is linear light.
fn coloring_color(sum: OrbitSummary, state: vec4<f32>, v: f32) -> vec3<f32> {
    return pow(clamp(state.xyz, vec3<f32>(0.0), vec3<f32>(1.0)), vec3<f32>(2.2));
}

fn coloring_alpha(sum: OrbitSummary, state: vec4<f32>) -> f32 {
    return state.w;
}
"#,
    // The base colour, opaque: UF's `rgb()`.
    accum_init: "vec4<f32>(cparam(10u), cparam(11u), cparam(12u), 1.0)",
    wgsl_accum: r#"
// The config's texture, declared here so only this colouring's iterate
// passes bind it (the layouts carry the slot; see TextureInLoop).
@group(0) @binding(14) var esc_texture: texture_2d<f32>;

// common.ulb TrapTransform: (z - centre) * rotation * recip(scale), then
// the aspect on the imaginary part, then the skew on the real part.
fn image_trap_position(z: vec2<f32>) -> vec2<f32> {
    let r = radians(cparam(3u));
    let rot = vec2<f32>(cos(r), sin(r));
    let d = z - vec2<f32>(cparam(0u), cparam(1u));
    var p = vec2<f32>(d.x * rot.x - d.y * rot.y, d.x * rot.y + d.y * rot.x) / cparam(2u);
    p.y = p.y * cparam(4u);
    let k = radians(cparam(5u));
    p.x = p.x * cos(k) - p.y * sin(k);
    return p;
}

// Image.getColor: the image over (-1,-1)..(1,1), (-1,-1) its bottom-left
// corner, transparent outside; ImageWrapper.NormalizePixel first, for
// Keep proportions. The negated test lets a non-finite point miss.
fn image_trap_color(zt: vec2<f32>) -> vec4<f32> {
    let dims = vec2<f32>(textureDimensions(esc_texture));
    var p = zt;
    if (cparam(6u) > 0.5) {
        if (dims.x > dims.y) {
            p.y = p.y * dims.x / dims.y;
        } else {
            p.x = p.x * dims.y / dims.x;
        }
    }
    if (!(abs(p.x) <= 1.0 && abs(p.y) <= 1.0)) {
        return vec4<f32>(0.0);
    }
    let uv = vec2<f32>((p.x + 1.0) * 0.5, (1.0 - p.y) * 0.5);
    return textureSampleLevel(esc_texture, palette_sampler, uv, 0.0);
}

// mergeX(bottom, top): UF's layer merge modes, the ones its help defines
// exactly. The result's alpha is the top's.
fn image_trap_mergex(mode: u32, b: vec3<f32>, t: vec3<f32>) -> vec3<f32> {
    switch mode {
        case 1u: { return b * t; }
        case 2u: { return vec3<f32>(1.0) - (vec3<f32>(1.0) - b) * (vec3<f32>(1.0) - t); }
        case 3u: { return min(b, t); }
        case 4u: { return max(b, t); }
        case 5u: { return abs(b - t); }
        case 6u: { return min(b + t, vec3<f32>(1.0)); }
        case 7u: { return max(b - t, vec3<f32>(0.0)); }
        case 8u: { return vec3<f32>(t.x, b.y, b.z); }
        case 9u: { return vec3<f32>(b.x, t.y, b.z); }
        case 10u: { return vec3<f32>(b.x, b.y, t.z); }
        default: { return t; }
    }
}

// compose(b, t, o): t over b at t's alpha times o, alphas counted.
fn image_trap_compose(b: vec4<f32>, t: vec4<f32>, o: f32) -> vec4<f32> {
    let at = t.w * o;
    let a = at + b.w * (1.0 - at);
    let rgb = t.xyz * at + b.xyz * (b.w * (1.0 - at));
    return vec4<f32>(select(vec3<f32>(0.0), rgb / max(a, 1e-30), a > 0.0), a);
}

// ColorMerge.FullMerge: compose(b, blend(t, mergeX(b, t), alpha(b)), o).
fn image_trap_full_merge(b: vec4<f32>, t: vec4<f32>, o: f32) -> vec4<f32> {
    let mode = u32(clamp(cparam(9u), 0.0, 10.0));
    let merged = vec4<f32>(image_trap_mergex(mode, b.xyz, t.xyz), t.w);
    return image_trap_compose(b, mix(t, merged, b.w), o);
}

fn coloring_accum(z: vec2<f32>, z_prev: vec2<f32>, c: vec2<f32>, state: vec4<f32>) -> vec4<f32> {
    let current = image_trap_color(image_trap_position(z));
    let o = clamp(cparam(7u), 0.0, 1.0);
    if (cparam(8u) > 0.5) {
        return image_trap_full_merge(current, state, o);
    }
    return image_trap_full_merge(state, current, o);
}
"#,
    recommended_bailout: None,
    pick_params: &[],
};
