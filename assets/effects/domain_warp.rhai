// Domain Warp: the image pushed along a field of noise warped by noise,
// as Inigo Quilez describes it.
//
// The built-in `domain_warp` effect (docs/main/EFFECTS.md, Rhai effects): the
// app runs this file -- on desktop the copy beside the app, so an edit
// shows on the next start. It replaced shaders/effects/color/domain_warp.wgsl, whose
// pixels, parameters and order it keeps, so a config cannot tell which
// it got.

const PARAMS = #{
    intensity: #{ value: 0.1, min: 0.0, max: 0.5, caption: "Intensity" },
    scale: #{ value: 3.0, min: 0.5, max: 10.0, caption: "Scale" },
    octaves: #{ value: 4, min: 1, max: 6, caption: "Octaves" },
    time: #{ value: 0.0, min: 0.0, max: 1000.0, caption: "Time" },
    blend_mode: #{
        value: 0,
        caption: "Blend Mode",
        choices: ["Normal", "Add", "Multiply", "Screen", "Overlay", "Soft Light", "Hard Light",
                  "Color Dodge", "Color Burn", "Hue", "Saturation", "Color", "Luminosity"],
    },
    direction: #{ value: 225.0, min: 0.0, max: 360.0, type: "angle", caption: "Direction" },
};
const INFO = #{ title: "Domain Warp", id: "domain_warp" };

// The gradient (the hash, from -1 to 1) at a lattice point, along an
// offset from it.
fn grad(lattice, ox, oy) {
    let g = hash2(lattice);
    (g.re * 2.0 - 1.0) * ox + (g.im * 2.0 - 1.0) * oy
}

// Gradient noise with a smooth fade.
fn noise(x, y) {
    let i = floor(complex(x, y));
    let f = fract(complex(x, y));
    let ux = f.re * f.re * (3.0 - 2.0 * f.re);
    let uy = f.im * f.im * (3.0 - 2.0 * f.im);
    let a = grad(i, f.re, f.im);
    let b = grad(i + complex(1.0, 0.0), f.re - 1.0, f.im);
    let c = grad(i + complex(0.0, 1.0), f.re, f.im - 1.0);
    let d = grad(i + complex(1.0, 1.0), f.re - 1.0, f.im - 1.0);
    mix(mix(a, b, ux), mix(c, d, ux), uy)
}

fn fbm(x, y, octaves) {
    let value = 0.0;
    let amplitude = 0.5;
    let px = x;
    let py = y;
    for o in 0..octaves {
        value += amplitude * noise(px, py);
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
    let tx = cos(direction) * p.time * 0.1;
    let ty = sin(direction) * p.time * 0.1;
    let x = ctx.uv.re * scale;
    let y = ctx.uv.im * scale;
    let qx = fbm(x + 0.0 + tx, y + 0.0 + ty, octaves);
    let qy = fbm(x + 5.2 + tx * 1.2, y + 1.3 + ty * 1.2, octaves);
    let rx = fbm(x + 4.0 * qx + 1.7, y + 4.0 * qy + 9.2, octaves);
    let ry = fbm(x + 4.0 * qx + 8.3, y + 4.0 * qy + 2.8, octaves);
    let at = complex(clamp(ctx.uv.re + rx * p.intensity, 0.0, 1.0), clamp(ctx.uv.im + ry * p.intensity, 0.0, 1.0));
    #{ color: input(at).rgb, blend: p.blend_mode, amount: p.intensity * 2.0 }
}
