// Density Blur: sparse regions blurred, dense ones kept sharp -- the
// density estimation flam3 does, on the accumulation, before the tone map.
//
// The built-in `density_blur` effect (docs/main/EFFECTS.md, Rhai effects): the
// app runs this file -- on desktop the copy beside the app, so an edit
// shows on the next start. It replaced shaders/effects/density/density_blur.wgsl, whose
// pixels, parameters and order it keeps, so a config cannot tell which
// it got.

const PARAMS = #{
    radius: #{ value: 3.0, min: 0.0, max: 10.0, caption: "Radius" },
    threshold: #{
        value: 0.25, min: 0.0, max: 1.0, caption: "Density Threshold",
        hint: "Density, in multiples of the frame's mean density, below which a pixel blurs at the full radius. 0.25 blurs the parts sparser than a quarter of the average.",
    },
    falloff: #{
        value: 0.5, min: 0.0, max: 1.0, caption: "Falloff",
        hint: "How far above the threshold, in multiples of the mean density, the blur fades out to none.",
    },
};
const INFO = #{ title: "Density Blur", id: "density_blur", category: "density" };

fn smooth(a, b, x) {
    let t = clamp((x - a) / (b - a), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn effect(c, p, ctx) {
    let max_radius = p.radius * ctx.pixel_scale;
    // In multiples of the frame's mean density, so the same pixels blur
    // however long the render has run.
    let density = c.a / max(ctx.density_mean, 0.000001);
    let blur = if density < p.threshold { 1.0 } else { 1.0 - smooth(p.threshold, p.threshold + p.falloff, density) };
    let radius = max_radius * blur;
    let px = 1.0 / to_float(ctx.width);
    let py = 1.0 / to_float(ctx.height);
    let sigma = radius / 2.0;
    let step = max(1.0, radius / 4.0);
    let sum = rgba(0.0);
    let weights = 0.0;
    // 9 x 9 taps at most, `step` apart; none where the pixel keeps its
    // colour, as the shader returns before them.
    if radius >= 0.5 {
        for yi in -4..5 {
            for xi in -4..5 {
                let ox = to_float(xi) * step;
                let oy = to_float(yi) * step;
                let dist = complex(ox, oy).abs();
                if dist <= radius {
                    let w = exp(-(dist * dist) / (2.0 * sigma * sigma));
                    sum = sum + input(ctx.uv + complex(ox * px, oy * py)) * w;
                    weights += w;
                }
            }
        }
    }
    if radius < 0.5 || weights <= 0.0 { c } else { sum / weights }
}
