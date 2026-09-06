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
//! * `grad` — the central-difference gradient of channel `.x`. Only
//!   computed for colourings that declare `ColoringFeature::NeedsGradient`;
//!   everything else receives zero and the template skips the four
//!   neighbour reads it would have cost.
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

use super::{SimColoringDef, SimParamDef};

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
fn sim_color(s: vec4<f32>, grad: vec2<f32>, p: vec2<i32>) -> vec4<f32> {
    // Channel select without dynamic indexing: a vec4 cannot be
    // indexed by a runtime value in WGSL, and unrolling it is free.
    let which = i32(round(clamp(cparam(0u), 0.0, 3.0)));
    var v = s.x;
    if (which == 1) { v = s.y; }
    else if (which == 2) { v = s.z; }
    else if (which == 3) { v = s.w; }

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

fn sim_color(s: vec4<f32>, grad: vec2<f32>, p: vec2<i32>) -> vec4<f32> {
    let t = clamp(tc_pick(s, cparam(0u)) * cparam(1u), 0.0, 1.0);
    let b = clamp(tc_pick(s, cparam(2u)) * cparam(3u) + cparam(4u), 0.0, 1.0);
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
fn sim_color(s: vec4<f32>, grad: vec2<f32>, p: vec2<i32>) -> vec4<f32> {
    let window = max(cparam(0u), 1.0);
    // Age relative to NOW, so the picture reads the same at step 500
    // and step 50,000 rather than saturating as the run goes on.
    let elapsed = max(f32(sim_step_index()) - s.z, 0.0);
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

fn sim_color(s: vec4<f32>, grad: vec2<f32>, p: vec2<i32>) -> vec4<f32> {
    let m = i32(round(clamp(cparam(1u), 0.0, 3.0)));
    if (m != 0 && lab_pick(s, cparam(1u)) < 0.5) {
        // Zero coverage: the shared tonemap composites the configured
        // background, exactly as for an empty region of a flame.
        return vec4<f32>(0.0, 0.0, 0.0, 0.0);
    }
    // The same PCG the rest of the engine uses, so a label maps to a
    // stable colour across runs and grid sizes.
    let h = sim_pcg(u32(max(lab_pick(s, cparam(0u)), 0.0)));
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
fn sim_color(s: vec4<f32>, grad: vec2<f32>, p: vec2<i32>) -> vec4<f32> {
    let n = max(cparam(0u), 1.0);
    // Scale index to palette position, centred in its band.
    let t = clamp((s.y + 0.5) / n, 0.0, 1.0);
    // The field runs roughly [-1, 1]; darken toward the low end.
    let v = clamp(s.x * 0.5 + 0.5, 0.0, 1.0);
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
fn sim_color(s: vec4<f32>, grad: vec2<f32>, p: vec2<i32>) -> vec4<f32> {
    let v = max(s.w, 0.0) * cparam(0u);
    // A soft knee rather than a clamp: agent counts are spiky, and
    // 1 - exp(-v) keeps a cell with one agent visible next to a cell
    // with ten.
    let t = mix(clamp(v, 0.0, 1.0), 1.0 - exp(-v), cparam(1u));
    return vec4<f32>(sim_palette(t), 1.0);
}
"#,
};
