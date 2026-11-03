// Two channels at once: one picks the palette's hue, the other its
// brightness.
//
// The Rhai twin of the built-in `two_channel` colouring
// (src/sim/colorings.rs; docs/projects/rhai-plugins.md, P4): the same
// parameters and the same picture.

// Channel `i` of the state (0-3).

const PARAMS = #{
    hue_channel: #{
        value: 1, choices: ["A / x", "B / y", "Age / z", "Spare / w"], caption: "Colour from",
        hint: "Which channel drives the palette lookup.",
    },
    hue_scale: #{
        value: 3.0, min: -20.0, max: 20.0, caption: "Colour scale",
        hint: "Multiplies that channel before the palette lookup.",
    },
    value_channel: #{
        value: 0, choices: ["A / x", "B / y", "Age / z", "Spare / w"], caption: "Brightness from",
        hint: "Which channel scales brightness.",
    },
    value_scale: #{
        value: 1.0, min: -8.0, max: 8.0, caption: "Brightness scale",
        hint: "Multiplies the brightness channel. Negative inverts it, which is usually what reads best when the two species are complementary.",
    },
    value_offset: #{
        value: 0.0, min: -1.0, max: 2.0, caption: "Brightness offset",
        hint: "Added after scaling. Raise it to keep the dark species visible.",
    },
};
const INFO = #{
    title: "Two Channel",
    id: "two_channel",
    description: "First channel picks the palette colour, second scales its brightness.",
};

fn pick(s, i) {
    let i = clamp(i, 0, 3);
    if i == 1 { s.y } else if i == 2 { s.z } else if i == 3 { s.w } else { s.x }
}

fn cell_color(cell, p) {
    let t = clamp(pick(cell.state, p.hue_channel) * p.hue_scale, 0.0, 1.0);
    let b = clamp(pick(cell.state, p.value_channel) * p.value_scale + p.value_offset, 0.0, 1.0);
    let c = palette(t);
    rgba(c.x * b, c.y * b, c.z * b, 1.0)
}
