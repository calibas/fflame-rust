// Worley Noise: cellular noise, the distances to the nearest of a
// scattering of moving points.
//
// The built-in `worley_noise` effect (docs/main/EFFECTS.md, Rhai effects): the
// app runs this file -- on desktop the copy beside the app, so an edit
// shows on the next start. It replaced shaders/effects/color/worley_noise.wgsl, whose
// pixels, parameters and order it keeps, so a config cannot tell which
// it got.

const PARAMS = #{
    intensity: #{ value: 0.5, min: 0.0, max: 1.0, caption: "Intensity" },
    scale: #{ value: 8.0, min: 1.0, max: 20.0, caption: "Scale" },
    time: #{ value: 0.0, min: 0.0, max: 1000.0, caption: "Time" },
    mode: #{ value: 0, caption: "Pattern", choices: ["Cells", "Edges", "Organic", "Crystal"] },
    color_mode: #{ value: 1, caption: "Color Mode", choices: ["Grayscale", "Rainbow", "Original"] },
    blend_mode: #{
        value: 4,
        caption: "Blend Mode",
        choices: ["Normal", "Add", "Multiply", "Screen", "Overlay", "Soft Light", "Hard Light",
                  "Color Dodge", "Color Burn", "Hue", "Saturation", "Color", "Luminosity"],
    },
    direction: #{ value: 225.0, min: 0.0, max: 360.0, type: "angle", caption: "Direction" },
};
const INFO = #{ title: "Worley Noise", id: "worley_noise" };

fn smooth(a, b, x) {
    let t = clamp((x - a) / (b - a), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

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
    let pi = 3.14159265359;
    let scale = max(1.0, p.scale);
    let direction = p.direction * pi / 180.0;
    let ax = (ctx.uv.re + cos(direction) * p.time * 0.1) * scale;
    let ay = (ctx.uv.im + sin(direction) * p.time * 0.1) * scale;
    let cell = floor(complex(ax, ay));
    let frac = fract(complex(ax, ay));
    // The nearest and second-nearest points of the 3x3 cells around.
    let d1 = 8.0;
    let d2 = 8.0;
    for j in -1..2 {
        for i in -1..2 {
            let nx = to_float(i);
            let ny = to_float(j);
            let point = hash2(cell + complex(nx, ny));
            let qx = 0.5 + 0.5 * sin(p.time * 0.5 + 6.2831 * point.re);
            let qy = 0.5 + 0.5 * sin(p.time * 0.5 + 6.2831 * point.im);
            let dist = complex(nx + qx - frac.re, ny + qy - frac.im).abs();
            if dist < d1 {
                d2 = d1;
                d1 = dist;
            } else if dist < d2 {
                d2 = dist;
            }
        }
    }
    let pattern = if p.mode == 1 {
        smooth(0.0, 0.15, d2 - d1)
    } else if p.mode == 2 {
        1.0 - smooth(0.0, 0.5, d1)
    } else if p.mode == 3 {
        clamp(d1 * d2 * 2.0, 0.0, 1.0)
    } else {
        d1
    };
    let rgb = if p.color_mode == 1 {
        hsv_to_rgb(fract(d1 + p.time * 0.1), 0.8, pattern)
    } else if p.color_mode == 2 {
        c.rgb * pattern
    } else {
        vec3(pattern)
    };
    #{ color: rgb, blend: p.blend_mode, amount: p.intensity }
}
