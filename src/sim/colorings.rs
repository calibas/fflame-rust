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
