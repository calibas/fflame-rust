// A label per value: each whole number of a channel (a cluster's index,
// a species) its own colour, hashed from the palette.
//
// The Rhai twin of the built-in `label` colouring (src/sim/colorings.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters and the same
// picture.

// Channel `i` of the state (0-3).

const PARAMS = #{
    channel: #{
        value: 0, choices: ["A / x", "B / y", "Age / z", "Spare / w"], caption: "Label from",
        hint: "Which channel holds the label.",
    },
    mask_channel: #{
        value: 1, choices: ["None", "B / y", "Age / z", "Spare / w"], caption: "Masked by",
        hint: "Cells whose mask channel is below 0.5 are drawn as background — percolation's closed sites, or a growth model's empty space.",
    },
};
const INFO = #{
    title: "Label",
    id: "label",
    description: "Hashes a cluster label to a palette position, so adjacent clusters get unrelated colours instead of a positional gradient.",
};

fn pick(s, i) {
    let i = clamp(i, 0, 3);
    if i == 1 { s.y } else if i == 2 { s.z } else if i == 3 { s.w } else { s.x }
}

fn cell_color(cell, p) {
    // A mask channel below a half is empty: the background shows.
    if clamp(p.mask_channel, 0, 3) != 0 && pick(cell.state, p.mask_channel) < 0.5 {
        return rgba(0.0, 0.0, 0.0, 0.0);
    }
    // The framework's PCG hash of the label, to a place on the palette.
    let h = pcg(to_int(max(pick(cell.state, p.channel), 0.0)));
    let c = palette(to_float(h >> 8) * (1.0 / 16777216.0));
    rgba(c.x, c.y, c.z, 1.0)
}
