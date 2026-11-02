// Julia: a Mandelbrot or Julia set drawn over the image.
//
// The built-in `julia` effect (docs/main/EFFECTS.md, Rhai effects): the
// app runs this file -- on desktop the copy beside the app, so an edit
// shows on the next start. It replaced shaders/effects/color/julia.wgsl, whose
// pixels, parameters and order it keeps, so a config cannot tell which
// it got.

const PARAMS = #{
    mode: #{ value: 1, caption: "Mode", choices: ["Mandelbrot", "Julia"] },
    julia_cx: #{ value: -0.7, min: -2.0, max: 2.0, caption: "Julia C (Real)" },
    julia_cy: #{ value: 0.27, min: -2.0, max: 2.0, caption: "Julia C (Imag)" },
    max_iter: #{ value: 100, min: 10, max: 500, caption: "Max Iterations" },
    zoom: #{ value: 1.0, min: 0.1, max: 100.0, caption: "Zoom" },
    center_x: #{ value: 0.0, min: -3.0, max: 3.0, caption: "Center X" },
    center_y: #{ value: 0.0, min: -3.0, max: 3.0, caption: "Center Y" },
    color_mode: #{ value: 1, caption: "Color Mode", choices: ["Escape", "Smooth", "Original mask"] },
    intensity: #{ value: 1.0, min: 0.0, max: 1.0, caption: "Intensity" },
    blend_mode: #{
        value: 0,
        caption: "Blend Mode",
        choices: ["Normal", "Add", "Multiply", "Screen", "Overlay", "Soft Light", "Hard Light",
                  "Color Dodge", "Color Burn", "Hue", "Saturation", "Color", "Luminosity"],
    },
};
const INFO = #{ title: "Julia/Mandelbrot", id: "julia" };

// The shader's `hsv_to_rgb`: hue by sextant, its `%` a remainder.
fn hsv_to_rgb(h, s, v) {
    let c = v * s;
    let h6 = h * 6.0;
    let x = c * (1.0 - abs(h6 - 2.0 * trunc(h6 / 2.0) - 1.0));
    let m = v - c;
    let rgb = if h6 < 1.0 {
        vec3(c, x, 0.0)
    } else if h6 < 2.0 {
        vec3(x, c, 0.0)
    } else if h6 < 3.0 {
        vec3(0.0, c, x)
    } else if h6 < 4.0 {
        vec3(0.0, x, c)
    } else if h6 < 5.0 {
        vec3(x, 0.0, c)
    } else {
        vec3(c, 0.0, x)
    };
    rgb + vec3(m)
}

fn effect(c, p, ctx) {
    let max_iter = max(10, p.max_iter);
    let zoom = max(0.1, p.zoom);
    let aspect = to_float(ctx.width) / to_float(ctx.height);
    let px = (ctx.uv.re - 0.5) * 2.0 * aspect / zoom + p.center_x;
    let py = (ctx.uv.im - 0.5) * 2.0 / zoom + p.center_y;
    let julia = p.mode == 1;
    let z = if julia { complex(px, py) } else { complex(0.0, 0.0) };
    let k = if julia { complex(p.julia_cx, p.julia_cy) } else { complex(px, py) };
    // The shader's loop, its `break` the loop's test: the count where
    // |z|^2 > 4, each z's squares taken once.
    let i = 0;
    let x2 = z.re * z.re;
    let y2 = z.im * z.im;
    while i < max_iter && !(x2 + y2 > 4.0) {
        z = complex(x2 - y2 + k.re, 2.0 * z.re * z.im + k.im);
        x2 = z.re * z.re;
        y2 = z.im * z.im;
        i += 1;
    }
    let iter = to_float(i);
    let max_f = to_float(max_iter);
    let z_mag_sq = z.re * z.re + z.im * z.im;
    let rgb = if iter >= max_f {
        vec3(0.0)
    } else if p.color_mode == 1 {
        let log_zn = ln(z_mag_sq) / 2.0;
        let nu = ln(log_zn / ln(2.0)) / ln(2.0);
        let t = (iter + 1.0 - nu) / max_f;
        hsv_to_rgb(fract(t * 5.0), 0.8, 1.0)
    } else if p.color_mode == 2 {
        c.rgb * (iter / max_f)
    } else {
        let t = iter / max_f;
        hsv_to_rgb(fract(t * 3.0), 0.7, t)
    };
    #{ color: rgb, blend: p.blend_mode, amount: p.intensity }
}
