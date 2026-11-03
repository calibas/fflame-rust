// A multi-scale model's colour memory: each of its eight weights a
// colour of the palette, mixed by weight, brightness from the field.
//
// The Rhai twin of the built-in `scale_memory` colouring
// (src/sim/colorings.rs; docs/projects/rhai-plugins.md, P4): the same
// parameters and the same picture.

const PARAMS = #{
    scales: #{
        value: 6.0, min: 1.0, max: 8.0, caption: "Scales across palette",
        hint: "How many scale indices the palette spans. Match the model's scale count to use the whole palette once.",
    },
    value_scale: #{
        value: 0.5, min: 0.0, max: 1.0, caption: "Brightness range",
        hint: "How much the field sets the brightness. 0 shows the memory alone. At 1, Multiply is Softology's colour times (f + 1) / 2, and Luminance makes (f + 1) / 2 the colour's whole luminance.",
    },
    brightness: #{
        value: 0, choices: ["Multiply", "Luminance (YUV)"], caption: "Brightness from",
        hint: "How the field sets the brightness. Multiply darkens the memory's colour by it, as Softology's does: low regions go black. Luminance (YUV) keeps the colour's hue and saturation and sets its luminance from the field, as Chau's does: every scale's colour stays visible, light and dark, and pale where the field is bright.",
    },
};
const INFO = #{
    title: "Scale Memory",
    id: "scale_memory",
    description: "Each cell coloured by the scales that have been winning it, blended over time: smooth colour regions where scale_mix speckles. Needs the model's Colour memory on; without it the picture is black.",
};

fn cell_color(cell, p) {
    let n = max(p.scales, 1.0);
    let m0 = clamp(cell.m0, 0.0, 1.0);
    let m1 = clamp(cell.m1, 0.0, 1.0);
    let r0 = cell.m0;
    let r1 = cell.m1;
    // The weights as kept, rescaled to the total they had: a weight
    // clamped is not lost.
    let total = clamp(r0.x + r0.y + r0.z + r0.w + (r1.x + r1.y + r1.z + r1.w), 0.0, 1.0);
    let kept = m0.x + m0.y + m0.z + m0.w + (m1.x + m1.y + m1.z + m1.w);
    let k = if kept == total { 1.0 } else { total / max(kept, 0.000001) };
    let c = palette(0.5 / n) * m0.x;
    c = c + palette(1.5 / n) * m0.y;
    c = c + palette(2.5 / n) * m0.z;
    c = c + palette(3.5 / n) * m0.w;
    c = c + palette(4.5 / n) * m1.x;
    c = c + palette(5.5 / n) * m1.y;
    c = c + palette(6.5 / n) * m1.z;
    c = c + palette(7.5 / n) * m1.w;
    c = c * k;
    let v = clamp(cell.state.x * 0.5 + 0.5, 0.0, 1.0);
    if p.brightness == 1 {
        // Luminance (YUV): moved toward the field's value, the hue kept.
        let y = c.x * 0.299 + c.y * 0.587 + c.z * 0.114;
        let lift = mix(y, v, p.value_scale) - y;
        return rgba(clamp(c.x + lift, 0.0, 1.0), clamp(c.y + lift, 0.0, 1.0), clamp(c.z + lift, 0.0, 1.0), 1.0);
    }
    let b = mix(1.0, v, p.value_scale);
    rgba(c.x * b, c.y * b, c.z * b, 1.0)
}
