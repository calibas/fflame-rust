// Tunnel: the image wrapped on the inside of a tunnel, flown through.
//
// The built-in `tunnel` effect (docs/main/EFFECTS.md, Rhai effects): the
// app runs this file -- on desktop the copy beside the app, so an edit
// shows on the next start. It replaced shaders/effects/color/tunnel.wgsl, whose
// pixels, parameters and order it keeps, so a config cannot tell which
// it got.

const PARAMS = #{
    speed: #{ value: 1.0, min: 0.0, max: 5.0, caption: "Speed" },
    rotation_speed: #{ value: 0.5, min: 0.0, max: 5.0, caption: "Rotation Speed" },
    distortion: #{ value: 0.0, min: 0.0, max: 1.0, caption: "Distortion" },
    time: #{ value: 0.0, min: 0.0, max: 1000.0, caption: "Time" },
    intensity: #{ value: 1.0, min: 0.0, max: 1.0, caption: "Intensity" },
    blend_mode: #{
        value: 0,
        caption: "Blend Mode",
        choices: ["Normal", "Add", "Multiply", "Screen", "Overlay", "Soft Light", "Hard Light",
                  "Color Dodge", "Color Burn", "Hue", "Saturation", "Color", "Luminosity"],
    },
};
const INFO = #{ title: "Tunnel", id: "tunnel" };

fn smooth(a, b, x) {
    let t = clamp((x - a) / (b - a), 0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

fn effect(c, p, ctx) {
    let pi = 3.14159265359;
    let aspect = to_float(ctx.width) / to_float(ctx.height);
    let x = (ctx.uv.re - 0.5) * aspect;
    let y = ctx.uv.im - 0.5;
    let angle = atan2(y, x) / pi;
    let radius = complex(x, y).abs();
    // The centre, where the tunnel has no wall, keeps the input.
    let wall = max(radius, 0.001);
    let r = wall + p.distortion * sin(angle * pi * 4.0 + p.time) * 0.1;
    let at = fract(complex(angle * 0.5 + 0.5 + p.time * p.rotation_speed * 0.1, 1.0 / r + p.time * p.speed));
    let rgb = input(at).rgb * smooth(0.0, 0.5, radius);
    let amount = if radius < 0.001 { 0.0 } else { p.intensity };
    #{ color: rgb, blend: p.blend_mode, amount: amount }
}
