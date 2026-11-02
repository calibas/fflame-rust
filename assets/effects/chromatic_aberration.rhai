// Chromatic Aberration: the red and blue channels drawn apart, as a lens
// that does not bring every colour to one focus.
//
// The built-in `chromatic_aberration` effect (docs/main/EFFECTS.md, Rhai effects): the
// app runs this file -- on desktop the copy beside the app, so an edit
// shows on the next start. It replaced shaders/effects/color/chromatic_aberration.wgsl, whose
// pixels, parameters and order it keeps, so a config cannot tell which
// it got.

const PARAMS = #{
    amount: #{ value: 2.0, min: 0.0, max: 20.0, caption: "Amount" },
    radial: #{ value: true, caption: "Radial" },
    intensity: #{ value: 1.0, min: 0.0, max: 1.0, caption: "Intensity" },
    blend_mode: #{
        value: 0,
        caption: "Blend Mode",
        choices: ["Normal", "Add", "Multiply", "Screen", "Overlay", "Soft Light", "Hard Light",
                  "Color Dodge", "Color Burn", "Hue", "Saturation", "Color", "Luminosity"],
    },
};
const INFO = #{ title: "Chromatic Aberration", id: "chromatic_aberration" };

fn effect(c, p, ctx) {
    // In logical points: the same look at any render scale.
    let amount = p.amount * ctx.pixel_scale;
    let px = 1.0 / to_float(ctx.width);
    let py = 1.0 / to_float(ctx.height);
    let offset = if p.radial {
        // Away from the centre, stronger towards the edges.
        let to_edge = ctx.uv - complex(0.5, 0.5);
        let dist = to_edge.abs() * 2.0;
        let v = to_edge + complex(0.0001, 0.0001);
        let n = v / v.abs();
        complex(n.re * amount * px * dist, n.im * amount * py * dist)
    } else {
        complex(amount * px, 0.0)
    };
    let r = input(ctx.uv + offset).r;
    let b = input(ctx.uv - offset).b;
    #{ color: vec3(r, c.g, b), blend: p.blend_mode, amount: p.intensity }
}
