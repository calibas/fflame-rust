// Simplex Noise: fractal simplex noise, as colour, as a distortion of the
// image, or as a mask.
//
// The built-in `simplex_noise` effect (docs/main/EFFECTS.md, Rhai effects): the
// app runs this file -- on desktop the copy beside the app, so an edit
// shows on the next start. It replaced shaders/effects/color/simplex_noise.wgsl, whose
// pixels, parameters and order it keeps, so a config cannot tell which
// it got.

const PARAMS = #{
    intensity: #{ value: 0.5, min: 0.0, max: 1.0, caption: "Intensity" },
    scale: #{ value: 5.0, min: 0.5, max: 20.0, caption: "Scale" },
    octaves: #{ value: 4, min: 1, max: 6, caption: "Octaves" },
    time: #{ value: 0.0, min: 0.0, max: 1000.0, caption: "Time" },
    mode: #{ value: 0, caption: "Mode", choices: ["Color", "Distort", "Mask"] },
    blend_mode: #{
        value: 4,
        caption: "Blend Mode",
        choices: ["Normal", "Add", "Multiply", "Screen", "Overlay", "Soft Light", "Hard Light",
                  "Color Dodge", "Color Burn", "Hue", "Saturation", "Color", "Luminosity"],
    },
    direction: #{ value: 225.0, min: 0.0, max: 360.0, type: "angle", caption: "Direction" },
};
const INFO = #{ title: "Simplex Noise", id: "simplex_noise" };

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

// One corner's share: its falloff to the fourth times its gradient (the
// hash, from -1 to 1) along the offset.
fn corner(lattice, ox, oy) {
    let t = 0.5 - (ox * ox + oy * oy);
    if t >= 0.0 {
        let g = hash2(lattice);
        let t2 = t * t;
        t2 * t2 * ((g.re * 2.0 - 1.0) * ox + (g.im * 2.0 - 1.0) * oy)
    } else {
        0.0
    }
}

fn simplex(x, y) {
    // (sqrt(3) - 1) / 2 and (3 - sqrt(3)) / 6, as the shader's constants
    // fold.
    let f2 = 0.36602540378443865;
    let g2 = 0.21132486540518713;
    let s = (x + y) * f2;
    let i = floor(complex(x + s, y + s));
    let t = (i.re + i.im) * g2;
    let x0 = x - (i.re - t);
    let y0 = y - (i.im - t);
    let i1x = if x0 > y0 { 1.0 } else { 0.0 };
    let i1y = if x0 > y0 { 0.0 } else { 1.0 };
    let n0 = corner(i, x0, y0);
    let n1 = corner(i + complex(i1x, i1y), x0 - i1x + g2, y0 - i1y + g2);
    let n2 = corner(i + complex(1.0, 1.0), x0 - 1.0 + 2.0 * g2, y0 - 1.0 + 2.0 * g2);
    70.0 * (n0 + n1 + n2)
}

fn fbm(x, y, octaves) {
    let value = 0.0;
    let amplitude = 0.5;
    let px = x;
    let py = y;
    for o in 0..octaves {
        value += amplitude * simplex(px, py);
        // The shader's rotation, column-major: (0.8, -0.6), (0.6, 0.8).
        let nx = (0.8 * px + 0.6 * py) * 2.02;
        let ny = (-0.6 * px + 0.8 * py) * 2.02;
        px = nx;
        py = ny;
        amplitude *= 0.5;
    }
    value
}

fn effect(c, p, ctx) {
    let pi = 3.14159265359;
    let scale = max(0.5, p.scale);
    let octaves = clamp(p.octaves, 1, 6);
    let direction = p.direction * pi / 180.0;
    let nx = ctx.uv.re * scale + cos(direction) * p.time * 0.1;
    let ny = ctx.uv.im * scale + sin(direction) * p.time * 0.1;
    let noise = fbm(nx, ny, octaves);
    let noise2 = fbm(nx + 5.2, ny + 1.3, octaves);
    // Distort: the image itself at the warped position, unblended.
    let warped = input(complex(clamp(ctx.uv.re + noise * p.intensity * 0.1, 0.0, 1.0), clamp(ctx.uv.im + noise2 * p.intensity * 0.1, 0.0, 1.0)));
    let rgb = if p.mode == 2 {
        c.rgb * smooth(-0.5, 0.5, noise)
    } else {
        hsv_to_rgb(fract(noise * 0.5 + 0.5 + p.time * 0.1), 0.8, noise2 * 0.5 + 0.5)
    };
    let color = if p.mode == 1 { warped } else { rgba(rgb, c.a) };
    let blend = if p.mode == 1 { -1 } else { p.blend_mode };
    #{ color: color, blend: blend, amount: p.intensity }
}
