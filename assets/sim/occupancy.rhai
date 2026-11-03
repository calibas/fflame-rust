// A density in `.w` (an agent model's deposit, a vapour), linear or
// softened to saturate.
//
// The Rhai twin of the built-in `occupancy` colouring
// (src/sim/colorings.rs; docs/projects/rhai-plugins.md, P4): the same
// parameters and the same picture.

const PARAMS = #{
    scale: #{
        value: 0.2, min: 0.001, max: 2.0, caption: "Scale",
        hint: "Multiplies the density before the palette lookup.",
    },
    soften: #{
        value: 1.0, min: 0.0, max: 1.0, caption: "Soften",
        hint: "Compresses the bright end, so a cell that happened to take several agents does not wash out the ones that took one. At 0 the mapping is linear.",
    },
};
const INFO = #{
    title: "Occupancy",
    id: "occupancy",
    description: "Agent density this step: the moving population itself, grainier and more immediate than the trail it leaves behind.",
};

fn cell_color(cell, p) {
    let v = max(cell.state.w, 0.0) * p.scale;
    let t = mix(clamp(v, 0.0, 1.0), 1.0 - exp(-v), p.soften);
    let c = palette(t);
    rgba(c.x, c.y, c.z, 1.0)
}
