// The structure tensor of `.x`: the local orientation as hue and the
// energy as brightness, or the coherence, or the energy alone.
//
// The Rhai twin of the built-in `structure` colouring
// (src/sim/colorings.rs; docs/projects/rhai-plugins.md, P4): the same
// parameters and the same picture.

const PARAMS = #{
    mode: #{
        value: 0, choices: ["Orientation", "Coherence", "Energy"], caption: "Draw",
        hint: "Orientation colours by the direction lines run (half a turn is the palette once), bright where the structure is strong. Coherence is how line-like each place is, 0 to 1. Energy is how much the field varies there at all.",
    },
    scale: #{
        value: 4.0, min: 0.1, max: 100.0, caption: "Scale",
        hint: "Multiplies the strength (the square root of the energy) before it becomes brightness in Orientation mode, or the palette position in Energy mode.",
    },
    rotate: #{
        value: 0.0, min: 0.0, max: 1.0, caption: "Rotate",
        hint: "Turns which orientation lands at the start of the palette, as a fraction of a half turn.",
    },
};
const INFO = #{
    title: "Structure",
    id: "structure",
    description: "The local texture's grain: which way lines run, how line-like the place is, or how much is going on. Stripes and labyrinths by direction, walls apart from junctions.",
};

fn cell_color(cell, p) {
    let jxx = cell.tensor.x;
    let jxy = cell.tensor.y;
    let jyy = cell.tensor.z;
    let energy = jxx + jyy;
    let strength = clamp(sqrt(max(energy, 0.0)) * p.scale, 0.0, 1.0);
    let mode = clamp(p.mode, 0, 2);
    if mode == 1 {
        let spread = sqrt((jxx - jyy) * (jxx - jyy) + 4.0 * jxy * jxy);
        let coherence = if energy > 0.000000000001 { spread / energy } else { 0.0 };
        let c = palette(clamp(coherence, 0.0, 1.0));
        return rgba(c.x, c.y, c.z, 1.0);
    }
    if mode == 2 {
        let c = palette(strength);
        return rgba(c.x, c.y, c.z, 1.0);
    }
    let theta = 0.5 * atan2(2.0 * jxy, jxx - jyy);
    let c = palette(fract(theta / 3.14159265 + 0.5 + p.rotate));
    rgba(c.x * strength, c.y * strength, c.z * strength, 1.0)
}
