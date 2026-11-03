// Channel: one state channel through the palette, with scale and offset.
//
// The Rhai twin of the built-in `channel` colouring (src/sim/colorings.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters, in the same
// order, and the same pixels.

const PARAMS = #{
    channel: #{
        value: 1, caption: "Channel",
        hint: "Which state channel to colour. For Gray–Scott: A is 0, B is 1.",
        choices: ["A / x", "B / y", "Age / z", "Spare / w"],
    },
    scale: #{
        value: 3.0, min: -20.0, max: 20.0, caption: "Scale",
        hint: "Multiplies the channel before the palette lookup. Gray–Scott's B rarely exceeds 0.35, so a scale near 3 uses the whole palette.",
    },
    offset: #{ value: 0.0, min: -1.0, max: 1.0, caption: "Offset", hint: "Added after scaling, before the palette lookup." },
    wrap: #{
        value: 0, caption: "Wrap",
        hint: "Off clamps values outside the palette to its ends; on wraps them, which suits cyclic state (automata phases) rather than concentrations.",
        choices: ["Clamp", "Wrap"],
    },
};
const INFO = #{
    title: "Channel",
    id: "channel",
    description: "One state channel through the palette, with scale and offset.",
};

fn cell_color(cell, p) {
    let s = cell.state;
    let v = if p.channel == 1 { s.y } else if p.channel == 2 { s.z } else if p.channel == 3 { s.w } else { s.x };
    let scaled = v * p.scale + p.offset;
    let t = if p.wrap == 1 { fract(scaled) } else { clamp(scaled, 0.0, 1.0) };
    palette(t)
}
