// The signed distance to the matte's edge: signed, as a depth inside,
// or as an outline.
//
// The Rhai twin of the built-in `distance` colouring
// (src/sim/colorings.rs; docs/projects/rhai-plugins.md, P4): the same
// parameters and the same picture.

const PARAMS = #{
    mode: #{
        value: 1, choices: ["Signed", "Depth", "Outline"], caption: "Draw",
        hint: "The matte draws the figure, so these colour the figure by its distance from its own edge. Depth is 0 at the edge and rises inward, full at Scale cells. Outline is brightest on the edge and fades to 0 at Scale cells in. Signed runs the palette from Scale cells outside to Scale cells inside, for a soft matte that shows both sides. Invert the matte to colour the space AROUND a figure instead — a glow.",
    },
    scale: #{
        value: 8.0, min: 0.5, max: 256.0, caption: "Scale (cells)",
        hint: "How many cells of distance span the palette.",
    },
};
const INFO = #{
    title: "Distance",
    id: "distance",
    description: "How far each cell is from the matte's edge, in cells: a glow outward from the figure, an outline along it, or the signed distance through the palette. Needs the matte on.",
};

fn cell_color(cell, p) {
    let d = cell.dist;
    let scale = max(p.scale, 0.000001);
    let mode = clamp(p.mode, 0, 2);
    let t = 0.0;
    if mode == 0 {
        t = clamp(d / (2.0 * scale) + 0.5, 0.0, 1.0);
    } else if mode == 1 {
        t = clamp(d / scale, 0.0, 1.0);
    } else {
        t = clamp(1.0 - abs(d) / scale, 0.0, 1.0);
    }
    let c = palette(t);
    rgba(c.x, c.y, c.z, 1.0)
}
