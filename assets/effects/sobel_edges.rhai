// Edge Glow: the image's edges, found by the Sobel operator on its
// luminance, drawn glowing over it.
//
// The Rhai twin of shaders/effects/color/sobel_edges.wgsl
// (docs/projects/rhai-plugins.md, P2): the same parameters, in the same
// order, so a config cannot tell which it got.

const PARAMS = #{
    intensity: #{ value: 1.0, min: 0.0, max: 2.0, caption: "Intensity" },
    threshold: #{ value: 0.1, min: 0.0, max: 1.0, caption: "Threshold" },
    glow: #{ value: 0.5, min: 0.0, max: 1.0, caption: "Glow" },
    preserve_color: #{ value: 0.5, min: 0.0, max: 1.0, caption: "Preserve Color" },
    blend_mode: #{
        value: 1,
        caption: "Blend Mode",
        choices: ["Normal", "Add", "Multiply", "Screen", "Overlay", "Soft Light", "Hard Light",
                  "Color Dodge", "Color Burn", "Hue", "Saturation", "Color", "Luminosity"],
    },
};
const INFO = #{ title: "Edge Glow", id: "sobel_edges" };

// The blend library's luminance (Rec. 709).
fn lum(uv) {
    let s = input(uv);
    s.r * 0.2126 + s.g * 0.7152 + s.b * 0.0722
}

fn smooth(a, b, x) {
    let t = clamp((x - a) / (b - a), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn effect(c, p, ctx) {
    let tx = 1.0 / to_float(ctx.width);
    let ty = 1.0 / to_float(ctx.height);
    let uv = ctx.uv;
    let n0 = lum(uv + complex(-tx, -ty));
    let n1 = lum(uv + complex(0.0, -ty));
    let n2 = lum(uv + complex(tx, -ty));
    let n3 = lum(uv + complex(-tx, 0.0));
    let n5 = lum(uv + complex(tx, 0.0));
    let n6 = lum(uv + complex(-tx, ty));
    let n7 = lum(uv + complex(0.0, ty));
    let n8 = lum(uv + complex(tx, ty));
    let gx = n2 + 2.0 * n5 + n8 - (n0 + 2.0 * n3 + n6);
    let gy = n0 + 2.0 * n1 + n2 - (n6 + 2.0 * n7 + n8);
    let edge = smooth(p.threshold * 0.5, p.threshold, sqrt(gx * gx + gy * gy));
    let tint = mix(vec3(1.0), c.rgb, p.preserve_color);
    #{ color: tint * ((1.0 + p.glow * 2.0) * edge), blend: p.blend_mode, amount: p.intensity }
}
