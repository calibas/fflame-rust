// Vignette: darkens the edges of the image with a smooth radial falloff.
//
// The built-in `vignette` effect (docs/main/EFFECTS.md, Rhai effects): the
// app runs this file -- on desktop the copy beside the app, so an edit
// shows on the next start. It replaced shaders/effects/color/vignette.wgsl, whose
// pixels, parameters and order it keeps, so a config cannot tell which
// it got.

const PARAMS = #{
    intensity: #{ value: 0.5, min: 0.0, max: 1.0, caption: "Intensity" },
    radius: #{ value: 0.5, min: 0.0, max: 1.0, caption: "Radius" },
    softness: #{ value: 0.5, min: 0.0, max: 1.0, caption: "Softness" },
    blend_mode: #{
        value: 0,
        caption: "Blend Mode",
        choices: ["Normal", "Add", "Multiply", "Screen", "Overlay", "Soft Light", "Hard Light",
                  "Color Dodge", "Color Burn", "Hue", "Saturation", "Color", "Luminosity"],
    },
};
const INFO = #{ title: "Vignette", id: "vignette" };

// WGSL's `smoothstep`, as its definition writes it.
fn smooth(a, b, x) {
    let t = clamp((x - a) / (b - a), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn effect(c, p, ctx) {
    // The distance from the centre, its aspect corrected.
    let aspect = to_float(ctx.width) / to_float(ctx.height);
    let d = complex((ctx.uv.re - 0.5) * aspect, ctx.uv.im - 0.5);
    let dist = d.abs();
    let start = p.radius * 0.5 * max(1.0, aspect);
    let end = start + p.softness * 0.5 * max(1.0, aspect);
    let vignette = 1.0 - smooth(start, end, dist);
    #{ color: c.rgb * vignette, blend: p.blend_mode, amount: p.intensity }
}
