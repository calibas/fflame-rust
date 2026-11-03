// Bilateral Blur: a blur that keeps edges, each tap weighed by its
// distance and by how far its luminance is from the centre's.
//
// The built-in `bilateral_blur` effect (docs/main/EFFECTS.md, Rhai effects): the
// app runs this file -- on desktop the copy beside the app, so an edit
// shows on the next start. It replaced shaders/effects/density/bilateral_blur.wgsl, whose
// pixels, parameters and order it keeps, so a config cannot tell which
// it got.

const PARAMS = #{
    radius: #{ value: 3.0, min: 1.0, max: 10.0, caption: "Radius" },
    sigma_spatial: #{ value: 3.0, min: 1.0, max: 10.0, caption: "Spatial Sigma" },
    sigma_range: #{ value: 0.1, min: 0.05, max: 0.5, caption: "Range Sigma" },
};
const INFO = #{ title: "Bilateral Blur", id: "bilateral_blur", category: "density" };

fn gaussian(x, sigma) {
    exp(-(x * x) / (2.0 * sigma * sigma))
}

fn lum(c) {
    c.r * 0.2126 + c.g * 0.7152 + c.b * 0.0722
}

fn effect(c, p, ctx) {
    let radius = to_int(clamp(p.radius * ctx.pixel_scale, 1.0, 10.0));
    let sigma_spatial = max(0.1, p.sigma_spatial * ctx.pixel_scale);
    let sigma_range = max(0.01, p.sigma_range);
    let px = 1.0 / to_float(ctx.width);
    let py = 1.0 / to_float(ctx.height);
    let centre = lum(c);
    let sum = rgba(0.0);
    let weights = 0.0;
    for y in -radius..(radius + 1) {
        for x in -radius..(radius + 1) {
            let fx = to_float(x);
            let fy = to_float(y);
            let s = input(ctx.uv + complex(fx * px, fy * py));
            let w = gaussian(complex(fx, fy).abs(), sigma_spatial) * gaussian(abs(centre - lum(s)), sigma_range);
            sum = sum + s * w;
            weights += w;
        }
    }
    sum / max(weights, 0.0001)
}
