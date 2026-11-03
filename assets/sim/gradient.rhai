// A channel's gradient: its direction as hue, its size as brightness.
//
// The Rhai twin of the built-in `gradient` colouring
// (src/sim/colorings.rs; docs/projects/rhai-plugins.md, P4): the same
// parameters and the same picture.

const PARAMS = #{
    channel: #{
        value: 0, choices: ["A / x", "B / y", "Age / z", "Spare / w"], caption: "Channel",
        hint: "Which channel's gradient to draw. For fingering, y (the pressure) points along the flow.",
    },
    scale: #{
        value: 4.0, min: 0.1, max: 100.0, caption: "Scale",
        hint: "Multiplies the gradient's magnitude before it becomes brightness. A field that changes by 0.25 per cell at its steepest fills the palette at 4.",
    },
    rotate: #{
        value: 0.0, min: 0.0, max: 1.0, caption: "Rotate",
        hint: "Turns which direction lands at the start of the palette, as a fraction of a full turn.",
    },
};
const INFO = #{
    title: "Gradient",
    id: "gradient",
    description: "The slope of a channel as a flow picture: which way it rises, through the palette, and how steeply, as brightness. Fronts, edges and flow lines.",
};

fn cell_color(cell, p) {
    let c = clamp(p.channel, 0, 3);
    let gx = cell.gx.x;
    let gy = cell.gy.x;
    if c == 1 {
        gx = cell.gx.y;
        gy = cell.gy.y;
    } else if c == 2 {
        gx = cell.gx.z;
        gy = cell.gy.z;
    } else if c == 3 {
        gx = cell.gx.w;
        gy = cell.gy.w;
    }
    let mag = clamp(sqrt(gx * gx + gy * gy) * p.scale, 0.0, 1.0);
    let t = fract(atan2(gy, gx) / 6.283185307 + 0.5 + p.rotate);
    let col = palette(t);
    rgba(col.x * mag, col.y * mag, col.z * mag, 1.0)
}
