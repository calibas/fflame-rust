// Film Grain: a per-pixel grain, the same every frame for a seed.
//
// The Rhai twin of shaders/effects/color/film_grain.wgsl
// (docs/projects/rhai-plugins.md, P2): the same parameters, in the same
// order, so a config cannot tell which it got.

const PARAMS = #{
    intensity: #{ value: 0.1, min: 0.0, max: 1.0, caption: "Intensity" },
    seed: #{ value: 0.0, min: 0.0, max: 256.0, caption: "Seed" },
    blend_mode: #{
        value: 4,
        caption: "Blend Mode",
        choices: ["Normal", "Add", "Multiply", "Screen", "Overlay", "Soft Light", "Hard Light",
                  "Color Dodge", "Color Burn", "Hue", "Saturation", "Color", "Luminosity"],
    },
};
const INFO = #{ title: "Film Grain", id: "film_grain" };

fn effect(c, p, ctx) {
    // The pixel's coordinates, the seed's offset, then the noise hash's
    // first part: the shader's `hash_pixel`, to the operation.
    let pixel = complex(ctx.uv.re * to_float(ctx.width), ctx.uv.im * to_float(ctx.height));
    let at = pixel + complex(p.seed * 127.1, p.seed * 311.7);
    let grain = hash2(at).re;
    #{ color: vec3(grain), blend: p.blend_mode, amount: p.intensity }
}
