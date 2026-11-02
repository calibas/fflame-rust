// Sharpen: an unsharp mask on the accumulation -- the colour pushed away
// from a Gaussian blur of itself.
//
// The Rhai twin of shaders/effects/density/sharpen.wgsl
// (docs/projects/rhai-plugins.md, P2): the same parameters, in the same
// order, so a config cannot tell which it got.

const PARAMS = #{
    amount: #{ value: 1.0, min: 0.0, max: 2.0, caption: "Amount" },
    radius: #{ value: 1.0, min: 0.5, max: 5.0, caption: "Radius" },
};
const INFO = #{ title: "Sharpen", id: "sharpen", category: "density" };

fn effect(c, p, ctx) {
    let radius = p.radius * ctx.pixel_scale;
    let px = 1.0 / to_float(ctx.width);
    let py = 1.0 / to_float(ctx.height);
    let sigma = radius / 2.0;
    let k = to_int(ceil(radius));
    let sum = rgba(0.0);
    let weights = 0.0;
    for y in -k..(k + 1) {
        for x in -k..(k + 1) {
            let fx = to_float(x);
            let fy = to_float(y);
            let dist = complex(fx, fy).abs();
            if dist <= radius {
                let w = exp(-(dist * dist) / (2.0 * sigma * sigma));
                sum = sum + input(ctx.uv + complex(fx * px, fy * py)) * w;
                weights += w;
            }
        }
    }
    let blurred = sum / weights;
    let sharp = c + (c - blurred) * p.amount;
    let out = rgba(clamp(sharp.rgb, 0.0, 1.0), c.a);
    if p.amount < 0.01 { c } else { out }
}
