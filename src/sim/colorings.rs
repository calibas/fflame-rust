//! Colouring definitions — field state to `(rgb, coverage)`.
//!
//! One `static SimColoringDef` per colouring, mirroring
//! `src/escape/colorings.rs`. Registered in `super::COLORINGS`
//! (append-only). Phase 1 ships `channel` alone; `two_channel`, `age`
//! and `hillshade` follow in phase 2.
//!
//! The template provides:
//!
//! * `cparam(i)` — this colouring's `i`th parameter, declaration order.
//! * `sim_palette(t)` — the shared palette LUT at `t` in [0, 1], bound
//!   exactly as escape binds it.
//! * `x: SimSample` — everything a colouring may read at the sample
//!   point: `x.s` the state; `x.gx`/`x.gy` the gradient of every
//!   channel (`NeedsGradient`); `x.dist` the signed distance to the
//!   matte's edge in cells (`NeedsDistance`); `x.tensor` the structure
//!   tensor of `.x` (`NeedsStructure`). Each is zero unless declared,
//!   and the template skips the reads it would have cost. The resolve
//!   builds one per cell and INTERPOLATES it, so under a magnifying
//!   filter a colouring sees smoothly varying derived quantities
//!   without ever reading a neighbour itself.
//! * `sim_grad_of(x, c)` — the gradient of channel `c` as a vec2.
//!
//! **The output convention is the flame accumulator's**, which is what
//! lets the whole tonemap → effects → readback tail work unchanged:
//! The matte (`SimMatte`, applied in the colour template rather than
//! here) multiplies into whatever coverage a colouring returns, so a
//! colouring never has to think about which cells are background: it
//! answers "what colour is this cell", and the matte answers "is this
//! cell drawn at all".
//!
//! `rgb` is colour and `a` is coverage, 1.0 for a cell that is part of
//! the picture. Because the tonemap reads alpha as a hit count, the
//! mode enters with Linear tonemapping — a flame's Log-calibrated
//! exposure would render a unit-range field black.

use super::{ColoringFeature, SimColoringDef, SimParamDef};

/// Map one channel through the palette.
///
/// The workhorse: every model has at least one channel whose value is
/// the picture. Scale and offset place the interesting part of the
/// range across the palette, because a field that lives in [0, 0.4]
/// otherwise uses less than half of it.
pub static CHANNEL: SimColoringDef = SimColoringDef {
    name: "channel",
    display_name: "Channel",
    description: "One state channel through the palette, with scale and offset.",
    features: &[],
    parameters: &[
        SimParamDef {
            name: "channel",
            display_name: "Channel",
            default: 1.0,
            min: 0.0,
            max: 3.0,
            tooltip: "Which state channel to colour. For Gray–Scott: A is 0, B is 1.",
            choices: &["A / x", "B / y", "Age / z", "Spare / w"],
        },
        SimParamDef {
            name: "scale",
            display_name: "Scale",
            default: 3.0,
            min: -20.0,
            max: 20.0,
            tooltip: "Multiplies the channel before the palette lookup. Gray–Scott's B \
                      rarely exceeds 0.35, so a scale near 3 uses the whole palette.",
            choices: &[],
        },
        SimParamDef {
            name: "offset",
            display_name: "Offset",
            default: 0.0,
            min: -1.0,
            max: 1.0,
            tooltip: "Added after scaling, before the palette lookup.",
            choices: &[],
        },
        SimParamDef {
            name: "wrap",
            display_name: "Wrap",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Off clamps values outside the palette to its ends; on wraps them, \
                      which suits cyclic state (automata phases) rather than concentrations.",
            choices: &["Clamp", "Wrap"],
        },
    ],
    wgsl: r#"
fn sim_color(x: SimSample, p: vec2<i32>) -> vec4<f32> {
    // Channel select without dynamic indexing: a vec4 cannot be
    // indexed by a runtime value in WGSL, and unrolling it is free.
    let which = i32(round(clamp(cparam(0u), 0.0, 3.0)));
    var v = x.s.x;
    if (which == 1) { v = x.s.y; }
    else if (which == 2) { v = x.s.z; }
    else if (which == 3) { v = x.s.w; }

    var t = v * cparam(1u) + cparam(2u);
    if (cparam(3u) >= 0.5) {
        t = fract(t);
    } else {
        t = clamp(t, 0.0, 1.0);
    }
    // Coverage is 1.0: every cell is part of the picture. Models with a
    // notion of empty (growth, percolation) get the `occupancy`
    // colouring in a later phase rather than overloading this one.
    return vec4<f32>(sim_palette(t), 1.0);
}
"#,
};


/// Two channels at once: one through the palette, the other as
/// brightness.
///
/// The reaction-diffusion set is the reason it exists. Those models
/// carry two concentrations, and showing one throws away half of what
/// the simulation computed -- a Brusselator's spots and the inhibitor
/// field that spaces them are different pictures.
pub static TWO_CHANNEL: SimColoringDef = SimColoringDef {
    name: "two_channel",
    display_name: "Two Channel",
    description: "First channel picks the palette colour, second scales its brightness.",
    features: &[],
    parameters: &[
        SimParamDef {
            name: "hue_channel",
            display_name: "Colour from",
            default: 1.0,
            min: 0.0,
            max: 3.0,
            tooltip: "Which channel drives the palette lookup.",
            choices: &["A / x", "B / y", "Age / z", "Spare / w"],
        },
        SimParamDef {
            name: "hue_scale",
            display_name: "Colour scale",
            default: 3.0,
            min: -20.0,
            max: 20.0,
            tooltip: "Multiplies that channel before the palette lookup.",
            choices: &[],
        },
        SimParamDef {
            name: "value_channel",
            display_name: "Brightness from",
            default: 0.0,
            min: 0.0,
            max: 3.0,
            tooltip: "Which channel scales brightness.",
            choices: &["A / x", "B / y", "Age / z", "Spare / w"],
        },
        SimParamDef {
            name: "value_scale",
            display_name: "Brightness scale",
            default: 1.0,
            min: -8.0,
            max: 8.0,
            tooltip: "Multiplies the brightness channel. Negative inverts it, which is \
                      usually what reads best when the two species are complementary.",
            choices: &[],
        },
        SimParamDef {
            name: "value_offset",
            display_name: "Brightness offset",
            default: 0.0,
            min: -1.0,
            max: 2.0,
            tooltip: "Added after scaling. Raise it to keep the dark species visible.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn tc_pick(s: vec4<f32>, which: f32) -> f32 {
    // No dynamic vector indexing in WGSL; unrolling is free.
    let i = i32(round(clamp(which, 0.0, 3.0)));
    var v = s.x;
    if (i == 1) { v = s.y; }
    else if (i == 2) { v = s.z; }
    else if (i == 3) { v = s.w; }
    return v;
}

fn sim_color(x: SimSample, p: vec2<i32>) -> vec4<f32> {
    let t = clamp(tc_pick(x.s, cparam(0u)) * cparam(1u), 0.0, 1.0);
    let b = clamp(tc_pick(x.s, cparam(2u)) * cparam(3u) + cparam(4u), 0.0, 1.0);
    return vec4<f32>(sim_palette(t) * b, 1.0);
}
"#,
};


/// Time since a cell last changed, through the palette.
///
/// The colouring the automata and growth models want: a cyclic CA's
/// spiral arms, a growth front's rings and an Ising domain's coarsening
/// are all a story about WHEN, and `channel` on the state itself shows
/// only the final frame of it.
///
/// Reads `.z`, which every model writes as the step at which the cell
/// last changed appreciably. Held rather than counted, so it is a
/// timestamp and this colouring is the thing that turns it into an age.
pub static AGE: SimColoringDef = SimColoringDef {
    name: "age",
    display_name: "Age",
    description: "How long since each cell last changed. Growth rings, spiral arms and \
                  coarsening fronts.",
    features: &[],
    parameters: &[
        SimParamDef {
            name: "window",
            display_name: "Window (steps)",
            default: 64.0,
            min: 1.0,
            max: 10000.0,
            tooltip: "How many steps back the palette spans. Anything older than this \
                      lands at the far end.",
            choices: &[],
        },
        SimParamDef {
            name: "invert",
            display_name: "Direction",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Whether recent changes take the near end of the palette or the far end.",
            choices: &["Recent first", "Oldest first"],
        },
        SimParamDef {
            name: "wrap",
            display_name: "Wrap",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Wrapping repeats the palette every window, which draws growth rings \
                      explicitly; clamping fades once.",
            choices: &["Clamp", "Wrap"],
        },
    ],
    wgsl: r#"
fn sim_color(x: SimSample, p: vec2<i32>) -> vec4<f32> {
    let window = max(cparam(0u), 1.0);
    // Age relative to NOW, so the picture reads the same at step 500
    // and step 50,000 rather than saturating as the run goes on.
    let elapsed = max(f32(sim_step_index()) - x.s.z, 0.0);
    var t = elapsed / window;
    if (cparam(2u) >= 0.5) {
        t = fract(t);
    } else {
        t = clamp(t, 0.0, 1.0);
    }
    if (cparam(1u) >= 0.5) {
        t = 1.0 - t;
    }
    return vec4<f32>(sim_palette(t), 1.0);
}
"#,
};


/// Categorical colour from a cluster label.
///
/// Percolation's labels are CELL INDICES, so they correlate with
/// position: colouring them directly through `channel` draws horizontal
/// stripes, because a label is roughly its row number times the width.
/// Hashing decorrelates them, which is what makes neighbouring clusters
/// legible as different things.
pub static LABEL: SimColoringDef = SimColoringDef {
    name: "label",
    display_name: "Label",
    description: "Hashes a cluster label to a palette position, so adjacent clusters get \
                  unrelated colours instead of a positional gradient.",
    features: &[],
    parameters: &[
        SimParamDef {
            name: "channel",
            display_name: "Label from",
            default: 0.0,
            min: 0.0,
            max: 3.0,
            tooltip: "Which channel holds the label.",
            choices: &["A / x", "B / y", "Age / z", "Spare / w"],
        },
        SimParamDef {
            name: "mask_channel",
            display_name: "Masked by",
            default: 1.0,
            min: 0.0,
            max: 3.0,
            tooltip: "Cells whose mask channel is below 0.5 are drawn as background — \
                      percolation's closed sites, or a growth model's empty space.",
            choices: &["None", "B / y", "Age / z", "Spare / w"],
        },
    ],
    wgsl: r#"
fn lab_pick(s: vec4<f32>, which: f32) -> f32 {
    let i = i32(round(clamp(which, 0.0, 3.0)));
    var v = s.x;
    if (i == 1) { v = s.y; }
    else if (i == 2) { v = s.z; }
    else if (i == 3) { v = s.w; }
    return v;
}

fn sim_color(x: SimSample, p: vec2<i32>) -> vec4<f32> {
    let m = i32(round(clamp(cparam(1u), 0.0, 3.0)));
    if (m != 0 && lab_pick(x.s, cparam(1u)) < 0.5) {
        // Zero coverage: the shared tonemap composites the configured
        // background, exactly as for an empty region of a flame.
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    // The same PCG the rest of the engine uses, so a label maps to a
    // stable colour across runs and grid sizes.
    let h = sim_pcg(u32(max(lab_pick(x.s, cparam(0u)), 0.0)));
    let t = f32(h >> 8u) * (1.0 / 16777216.0);
    return vec4<f32>(sim_palette(t), 1.0);
}
"#,
};


/// Which scale fired, as hue, with the field as brightness: the look
/// later McCabe implementations gave the rule.
///
/// The step writes the firing scale's index into `.y`; this spreads
/// the indices across the palette and darkens by `.x`, so the nested
/// scales read as nested colours.
pub static SCALE_MIX: SimColoringDef = SimColoringDef {
    name: "scale_mix",
    display_name: "Scale Mix",
    description: "Hue from which scale is driving each cell, brightness from the field: \
                  the multi-scale look, where each nesting level has its own colour.",
    features: &[],
    parameters: &[
        SimParamDef {
            name: "scales",
            display_name: "Scales across palette",
            default: 6.0,
            min: 1.0,
            max: 8.0,
            tooltip: "How many scale indices the palette spans. Match the model's scale \
                      count to use the whole palette once.",
            choices: &[],
        },
        SimParamDef {
            name: "value_scale",
            display_name: "Brightness range",
            default: 0.5,
            min: 0.0,
            max: 1.0,
            tooltip: "How much the field darkens the colour. 0 shows the scale alone.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn sim_color(x: SimSample, p: vec2<i32>) -> vec4<f32> {
    let n = max(cparam(0u), 1.0);
    // Scale index to palette position, centred in its band.
    let t = clamp((x.s.y + 0.5) / n, 0.0, 1.0);
    // The field runs roughly [-1, 1]; darken toward the low end.
    let v = clamp(x.s.x * 0.5 + 0.5, 0.0, 1.0);
    let b = mix(1.0, v, cparam(1u));
    return vec4<f32>(sim_palette(t) * b, 1.0);
}
"#,
};


/// Where the agents ARE, rather than where they have been.
///
/// An agent model's step writes the deposit it just collected into
/// `.w`; this draws that. It is a grainier, more immediate picture
/// than the trail -- individual filaments of moving agents rather
/// than the smoothed network they maintain.
pub static OCCUPANCY: SimColoringDef = SimColoringDef {
    name: "occupancy",
    display_name: "Occupancy",
    description: "Agent density this step: the moving population itself, grainier and more \
                  immediate than the trail it leaves behind.",
    features: &[],
    parameters: &[
        SimParamDef {
            name: "scale",
            display_name: "Scale",
            default: 0.2,
            min: 0.001,
            max: 2.0,
            tooltip: "Multiplies the density before the palette lookup.",
            choices: &[],
        },
        SimParamDef {
            name: "soften",
            display_name: "Soften",
            default: 1.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Compresses the bright end, so a cell that happened to take several \
                      agents does not wash out the ones that took one. At 0 the mapping is \
                      linear.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn sim_color(x: SimSample, p: vec2<i32>) -> vec4<f32> {
    let v = max(x.s.w, 0.0) * cparam(0u);
    // A soft knee rather than a clamp: agent counts are spiky, and
    // 1 - exp(-v) keeps a cell with one agent visible next to a cell
    // with ten.
    let t = mix(clamp(v, 0.0, 1.0), 1.0 - exp(-v), cparam(1u));
    return vec4<f32>(sim_palette(t), 1.0);
}
"#,
};


/// The gradient of one channel as a flow picture: direction through
/// the palette, magnitude as brightness.
///
/// The classic vector-field image, and it works on every model
/// because every field has a slope. On a reaction–diffusion pattern it
/// draws the fronts; on the fingering model, the gradient of the
/// pressure (`.y`) points along the flow -- the true velocity also
/// scales with the mobility, so this is its direction exactly and its
/// speed up to a per-cell factor.
///
/// Direction is `ff_atan2` of the gradient -- Metal's `atan2` is wrong
/// at zero pairs, and a flat cell is exactly that -- mapped so that a
/// full turn is the palette once. Brightness is the magnitude scaled
/// and clamped, so flat cells are dark whatever hue the noise in their
/// direction would have given them.
pub static GRADIENT: SimColoringDef = SimColoringDef {
    name: "gradient",
    display_name: "Gradient",
    description: "The slope of a channel as a flow picture: which way it rises, through the \
                  palette, and how steeply, as brightness. Fronts, edges and flow lines.",
    features: &[ColoringFeature::NeedsGradient],
    parameters: &[
        SimParamDef {
            name: "channel",
            display_name: "Channel",
            default: 0.0,
            min: 0.0,
            max: 3.0,
            tooltip: "Which channel's gradient to draw. For fingering, y (the pressure) points \
                      along the flow.",
            choices: &["A / x", "B / y", "Age / z", "Spare / w"],
        },
        SimParamDef {
            name: "scale",
            display_name: "Scale",
            default: 4.0,
            min: 0.1,
            max: 100.0,
            tooltip: "Multiplies the gradient's magnitude before it becomes brightness. A \
                      field that changes by 0.25 per cell at its steepest fills the palette \
                      at 4.",
            choices: &[],
        },
        SimParamDef {
            name: "rotate",
            display_name: "Rotate",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Turns which direction lands at the start of the palette, as a fraction \
                      of a full turn.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn sim_color(x: SimSample, p: vec2<i32>) -> vec4<f32> {
    let c = i32(round(clamp(cparam(0u), 0.0, 3.0)));
    let g = sim_grad_of(x, c);
    let mag = clamp(length(g) * cparam(1u), 0.0, 1.0);
    // A full turn is the palette once; ff_atan2 is IEEE-exact at the
    // zero pair a flat cell hands it.
    let t = fract(ff_atan2(g.y, g.x) / 6.283185307 + 0.5 + cparam(2u));
    return vec4<f32>(sim_palette(t) * mag, 1.0);
}
"#,
};

/// The structure tensor of channel `.x`: the local texture's
/// orientation and how strongly it is oriented.
///
/// The gradient's outer product, smoothed over a 3x3 window, is a 2x2
/// symmetric matrix whose eigenvectors are the texture's along- and
/// across-directions and whose eigenvalues say how much variation each
/// carries. From it: ORIENTATION, the across-direction (a line's
/// normal, defined up to sign, so half a turn is the palette once);
/// COHERENCE, (l1 - l2) / (l1 + l2) in [0, 1], which is 1 on a ridge
/// or a line and 0 on a blob or a flat, and separates the labyrinth's
/// walls from its junctions; ENERGY, the trace, which is the smoothed
/// gradient magnitude squared.
pub static STRUCTURE: SimColoringDef = SimColoringDef {
    name: "structure",
    display_name: "Structure",
    description: "The local texture's grain: which way lines run, how line-like the place is, \
                  or how much is going on. Stripes and labyrinths by direction, walls apart \
                  from junctions.",
    features: &[ColoringFeature::NeedsStructure],
    parameters: &[
        SimParamDef {
            name: "mode",
            display_name: "Draw",
            default: 0.0,
            min: 0.0,
            max: 2.0,
            tooltip: "Orientation colours by the direction lines run (half a turn is the \
                      palette once), bright where the structure is strong. Coherence is how \
                      line-like each place is, 0 to 1. Energy is how much the field varies \
                      there at all.",
            choices: &["Orientation", "Coherence", "Energy"],
        },
        SimParamDef {
            name: "scale",
            display_name: "Scale",
            default: 4.0,
            min: 0.1,
            max: 100.0,
            tooltip: "Multiplies the strength (the square root of the energy) before it \
                      becomes brightness in Orientation mode, or the palette position in \
                      Energy mode.",
            choices: &[],
        },
        SimParamDef {
            name: "rotate",
            display_name: "Rotate",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Turns which orientation lands at the start of the palette, as a fraction \
                      of a half turn.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn sim_color(x: SimSample, p: vec2<i32>) -> vec4<f32> {
    let jxx = x.tensor.x;
    let jxy = x.tensor.y;
    let jyy = x.tensor.z;
    let energy = jxx + jyy;
    let strength = clamp(sqrt(max(energy, 0.0)) * cparam(1u), 0.0, 1.0);
    let mode = i32(round(clamp(cparam(0u), 0.0, 2.0)));
    if (mode == 1) {
        // (l1 - l2) / (l1 + l2), the eigenvalue spread over the trace.
        let spread = sqrt((jxx - jyy) * (jxx - jyy) + 4.0 * jxy * jxy);
        let coherence = select(0.0, spread / energy, energy > 1.0e-12);
        return vec4<f32>(sim_palette(clamp(coherence, 0.0, 1.0)), 1.0);
    }
    if (mode == 2) {
        return vec4<f32>(sim_palette(strength), 1.0);
    }
    // The dominant eigenvector's angle, from the tensor's own
    // half-angle form; defined modulo pi, so half a turn is the
    // palette once.
    let theta = 0.5 * ff_atan2(2.0 * jxy, jxx - jyy);
    let t = fract(theta / 3.14159265 + 0.5 + cparam(2u));
    return vec4<f32>(sim_palette(t) * strength, 1.0);
}
"#,
};

/// The signed distance to the matte's edge, in cells.
///
/// Reads the field phase C of the derived-fields plan built. The
/// matte must be on -- it is what says which cells are the figure --
/// and the renderer builds the distance field for this colouring
/// whatever the matte's own edge setting. With the matte off there is
/// nothing to be distant from and every cell reads 0.
///
/// The matte also CUTS everything outside the figure, so a mode that
/// coloured the outside would never be seen: every mode colours the
/// figure by its own depth, and the space around a cluster is
/// coloured by inverting the matte, which makes that space the figure.
pub static DISTANCE: SimColoringDef = SimColoringDef {
    name: "distance",
    display_name: "Distance",
    description: "How far each cell is from the matte's edge, in cells: a glow outward from \
                  the figure, an outline along it, or the signed distance through the \
                  palette. Needs the matte on.",
    features: &[ColoringFeature::NeedsDistance],
    parameters: &[
        SimParamDef {
            name: "mode",
            display_name: "Draw",
            default: 1.0,
            min: 0.0,
            max: 2.0,
            tooltip: "The matte draws the figure, so these colour the figure by its distance \
                      from its own edge. Depth is 0 at the edge and rises inward, full at \
                      Scale cells. Outline is brightest on the edge and fades to 0 at Scale \
                      cells in. Signed runs the palette from Scale cells outside to Scale \
                      cells inside, for a soft matte that shows both sides. Invert the matte \
                      to colour the space AROUND a figure instead — a glow.",
            choices: &["Signed", "Depth", "Outline"],
        },
        SimParamDef {
            name: "scale",
            display_name: "Scale (cells)",
            default: 8.0,
            min: 0.5,
            max: 256.0,
            tooltip: "How many cells of distance span the palette.",
            choices: &[],
        },
    ],
    wgsl: r#"
fn sim_color(x: SimSample, p: vec2<i32>) -> vec4<f32> {
    let d = x.dist;
    let scale = max(cparam(1u), 1.0e-6);
    let mode = i32(round(clamp(cparam(0u), 0.0, 2.0)));
    var t = 0.0;
    if (mode == 0) {
        t = clamp(d / (2.0 * scale) + 0.5, 0.0, 1.0);
    } else if (mode == 1) {
        t = clamp(d / scale, 0.0, 1.0);
    } else {
        t = clamp(1.0 - abs(d) / scale, 0.0, 1.0);
    }
    return vec4<f32>(sim_palette(t), 1.0);
}
"#,
};

/// Line integral convolution: white noise smeared along the field's
/// flow lines, so the picture IS the streamlines.
///
/// From each cell, walk the direction field forward and back `length`
/// steps of one cell, averaging a per-cell noise value at each stop.
/// Along a streamline the noise correlates and across it does not,
/// which is what draws the lines. The direction is the gradient of a
/// channel, or its perpendicular -- along contours -- with the sign
/// kept continuous step to step, since a contour direction is only
/// defined up to sign.
///
/// This colouring READS THE CELL: it has to walk the field, so it
/// declares `ReadsCell` and is computed at cell resolution under an
/// interpolating resolve, then interpolated -- which for a texture is
/// its nature. The noise is keyed by cell and seed alone, not by the
/// step, so the texture holds still while the run advances. Cost is
/// 2 x length x 4 reads per tap; measured in `phase_d_colouring_cost`.
pub static LIC: SimColoringDef = SimColoringDef {
    name: "lic",
    display_name: "Flow lines (LIC)",
    description: "Noise smeared along the field's flow lines, so the streamlines themselves \
                  are the picture. Line integral convolution.",
    features: &[ColoringFeature::ReadsCell],
    parameters: &[
        SimParamDef {
            name: "channel",
            display_name: "Channel",
            default: 0.0,
            min: 0.0,
            max: 3.0,
            tooltip: "Which channel's gradient gives the direction. For fingering, y (the \
                      pressure) points along the flow.",
            choices: &["A / x", "B / y", "Age / z", "Spare / w"],
        },
        SimParamDef {
            name: "direction",
            display_name: "Follow",
            default: 0.0,
            min: 0.0,
            max: 1.0,
            tooltip: "Contours runs along lines of equal value — the shape of the pattern. \
                      Gradient runs across them, up the slope — the flow.",
            choices: &["Contours", "Gradient"],
        },
        SimParamDef {
            name: "length",
            display_name: "Length (cells)",
            default: 8.0,
            min: 1.0,
            max: 32.0,
            tooltip: "How far each way the noise is smeared. Longer lines are smoother and \
                      cost proportionally more.",
            choices: &[],
        },
        SimParamDef {
            name: "contrast",
            display_name: "Contrast",
            default: 1.0,
            min: 0.1,
            max: 4.0,
            tooltip: "Stretches the averaged noise about its mean. The average of many noise \
                      values is close to a half; this pulls it back out to the palette.",
            choices: &[],
        },
    ],
    wgsl: r#"
// Per-cell white noise keyed by cell and seed, NOT by step: the
// texture must hold still while the run advances.
fn lic_noise(q: vec2<i32>) -> f32 {
    let g = sim_grid();
    let idx = u32(q.y * g.x + q.x);
    let h = sim_pcg(sim_pcg(idx ^ params.seed_lo) ^ params.seed_hi ^ 0x11cu);
    return f32(h >> 8u) * (1.0 / 16777216.0);
}

// The unit direction at a cell: the gradient of the chosen channel,
// or its perpendicular.
fn lic_dir(q: vec2<i32>, c: i32, along: bool) -> vec2<f32> {
    let r = sim_read(q + vec2<i32>(1, 0));
    let l = sim_read(q - vec2<i32>(1, 0));
    let u = sim_read(q + vec2<i32>(0, 1));
    let d = sim_read(q - vec2<i32>(0, 1));
    var gx = r.x - l.x;
    var gy = u.x - d.x;
    if (c == 1) { gx = r.y - l.y; gy = u.y - d.y; }
    else if (c == 2) { gx = r.z - l.z; gy = u.z - d.z; }
    else if (c == 3) { gx = r.w - l.w; gy = u.w - d.w; }
    var v = vec2<f32>(gx, gy);
    let n = length(v);
    if (n < 1.0e-9) {
        return vec2<f32>(0.0, 0.0);
    }
    v = v / n;
    return select(v, vec2<f32>(-v.y, v.x), along);
}

fn sim_color(x: SimSample, p: vec2<i32>) -> vec4<f32> {
    let c = i32(round(clamp(cparam(0u), 0.0, 3.0)));
    let along = cparam(1u) < 0.5;
    let len = i32(round(clamp(cparam(2u), 1.0, 32.0)));
    let g = sim_grid();

    var acc = lic_noise(p);
    var n = 1.0;
    // Forward, then back, each keeping its direction continuous.
    for (var leg = 0; leg < 2; leg = leg + 1) {
        var pos = vec2<f32>(p) + vec2<f32>(0.5, 0.5);
        var prev = vec2<f32>(0.0, 0.0);
        for (var i = 0; i < len; i = i + 1) {
            let q = vec2<i32>(floor(pos));
            if (q.x < 0 || q.y < 0 || q.x >= g.x || q.y >= g.y) {
                break;
            }
            var v = lic_dir(q, c, along);
            if (leg == 1) {
                v = -v;
            }
            if (dot(v, prev) < 0.0) {
                v = -v;
            }
            if (dot(v, v) < 0.5) {
                break;
            }
            prev = v;
            pos = pos + v;
            let s = vec2<i32>(floor(pos));
            if (s.x < 0 || s.y < 0 || s.x >= g.x || s.y >= g.y) {
                break;
            }
            acc = acc + lic_noise(s);
            n = n + 1.0;
        }
    }
    // The mean of n uniforms has spread 0.289 / sqrt(n) about a half;
    // stretch it back to one noise value's spread, then by the
    // contrast. (Unit spread was tried: it clamps half the pixels and
    // the picture is nearly binary.)
    let mean = acc / n;
    let t = clamp(0.5 + (mean - 0.5) * sqrt(n) * cparam(3u), 0.0, 1.0);
    return vec4<f32>(sim_palette(t), 1.0);
}
"#,
};
