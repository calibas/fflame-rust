// Which scale won, as hue, and the field's value as brightness: a
// multi-scale model's `.y` and `.x`.
//
// The Rhai twin of the built-in `scale_mix` colouring
// (src/sim/colorings.rs; docs/projects/rhai-plugins.md, P4): the same
// parameters and the same picture.

const PARAMS = #{
    scales: #{
        value: 6.0, min: 1.0, max: 8.0, caption: "Scales across palette",
        hint: "How many scale indices the palette spans. Match the model's scale count to use the whole palette once.",
    },
    value_scale: #{
        value: 0.5, min: 0.0, max: 1.0, caption: "Brightness range",
        hint: "How much the field darkens the colour. 0 shows the scale alone.",
    },
};
const INFO = #{
    title: "Scale Mix",
    id: "scale_mix",
    description: "Hue from which scale is driving each cell, brightness from the field: the multi-scale look, where each nesting level has its own colour.",
};

fn cell_color(cell, p) {
    let n = max(p.scales, 1.0);
    let t = clamp((cell.state.y + 0.5) / n, 0.0, 1.0);
    let v = clamp(cell.state.x * 0.5 + 0.5, 0.0, 1.0);
    let b = mix(1.0, v, p.value_scale);
    let c = palette(t);
    rgba(c.x * b, c.y * b, c.z * b, 1.0)
}
