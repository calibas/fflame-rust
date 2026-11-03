// How long ago each cell last changed: the models' `.z`, the step it
// last moved, against the step the run is at.
//
// The Rhai twin of the built-in `age` colouring (src/sim/colorings.rs;
// docs/projects/rhai-plugins.md, P4): the same parameters and the same
// picture.

const PARAMS = #{
    window: #{
        value: 64.0, min: 1.0, max: 10000.0, caption: "Window (steps)",
        hint: "How many steps back the palette spans. Anything older than this lands at the far end.",
    },
    invert: #{
        value: 0, choices: ["Recent first", "Oldest first"], caption: "Direction",
        hint: "Whether recent changes take the near end of the palette or the far end.",
    },
    wrap: #{
        value: 0, choices: ["Clamp", "Wrap"], caption: "Wrap",
        hint: "Wrapping repeats the palette every window, which draws growth rings explicitly; clamping fades once.",
    },
};
const INFO = #{
    title: "Age",
    id: "age",
    description: "How long since each cell last changed. Growth rings, spiral arms and coarsening fronts.",
};

fn cell_color(cell, p, ctx) {
    let window = max(p.window, 1.0);
    let elapsed = max(to_float(ctx.step) - cell.state.z, 0.0);
    let t = elapsed / window;
    if p.wrap == 1 {
        t = fract(t);
    } else {
        t = clamp(t, 0.0, 1.0);
    }
    if p.invert == 1 {
        t = 1.0 - t;
    }
    let c = palette(t);
    rgba(c.x, c.y, c.z, 1.0)
}
