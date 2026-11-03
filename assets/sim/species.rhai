// Four (or three) species at once: the balance between them as hue, how
// far from even as brightness.
//
// The Rhai twin of the built-in `species` colouring
// (src/sim/colorings.rs; docs/projects/rhai-plugins.md, P4): the same
// parameters and the same picture.

const PARAMS = #{
    scale: #{
        value: 1.0, min: 0.1, max: 10.0, caption: "Scale",
        hint: "Multiplies the lead before it becomes brightness. The lead is the length of (A − C, B − D), at most about 1.4.",
    },
    rotate: #{
        value: 0.0, min: 0.0, max: 1.0, caption: "Rotate",
        hint: "Turns which field lands at the start of the palette, as a fraction of the whole.",
    },
    fields: #{
        value: 0, choices: ["Four", "Three"], caption: "Fields",
        hint: "Four places the channels a quarter turn apart. Three places x, y, z a third of a turn apart and ignores w — for the lattice with a memory in its fourth channel.",
    },
};
const INFO = #{
    title: "Four-way mix",
    id: "species",
    description: "The four channels a quarter turn apart around the palette: the hue is which leads, the brightness by how much, and balanced cells go dark. For the coupled Turing lattice.",
};

fn cell_color(cell, p) {
    let s = cell.state;
    let cx = s.x - s.z;
    let cy = s.y - s.w;
    if p.fields == 1 {
        cx = s.x - 0.5 * (s.y + s.z);
        cy = 0.8660254 * (s.y - s.z);
    }
    let hue = fract(atan2(cy, cx) / 6.283185307 + p.rotate);
    let bright = clamp(sqrt(cx * cx + cy * cy) * p.scale, 0.0, 1.0);
    let c = palette(hue);
    rgba(c.x * bright, c.y * bright, c.z * bright, 1.0)
}
