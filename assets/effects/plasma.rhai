// Plasma: the demoscene's plasma, a sum of sines, travelling in a direction.
//
// The built-in `plasma` effect (docs/main/EFFECTS.md, Rhai effects): the
// app runs this file -- on desktop the copy beside the app, so an edit
// shows on the next start. It replaced shaders/effects/color/plasma.wgsl, whose
// pixels, parameters and order it keeps, so a config cannot tell which
// it got.

const PARAMS = #{
    intensity: #{ value: 0.3, min: 0.0, max: 1.0, caption: "Intensity" },
    scale: #{ value: 3.0, min: 0.1, max: 100.0, caption: "Scale" },
    speed: #{ value: 1.0, min: 0.0, max: 10.0, caption: "Speed" },
    time: #{ value: 0.0, min: 0.0, max: 1000.0, caption: "Time" },
    blend_mode: #{
        value: 1,
        caption: "Blend Mode",
        choices: ["Normal", "Add", "Multiply", "Screen", "Overlay", "Soft Light", "Hard Light",
                  "Color Dodge", "Color Burn", "Hue", "Saturation", "Color", "Luminosity"],
    },
    direction: #{ value: 225.0, min: 0.0, max: 360.0, type: "angle", caption: "Direction" },
};
const INFO = #{ title: "Plasma", id: "plasma" };

fn effect(c, p, ctx) {
    let pi = 3.14159265359;
    let scale = max(0.5, p.scale);
    let time = p.time * p.speed;
    let direction = p.direction * pi / 180.0;
    // Each pixel the same animation offset in time along the direction,
    // so the pattern travels and keeps its loop.
    let x = ctx.uv.re * scale;
    let y = ctx.uv.im * scale;
    let dir_phase = x * cos(direction) + y * sin(direction);
    let t = time + dir_phase;
    let v = sin(x + t);
    v += sin((y + t) * 0.5);
    v += sin((x + y + t) * 0.5);
    let cx = x + 0.5 * sin(t * 0.25);
    let cy = y + 0.5 * cos(t * 0.5);
    v += sin(sqrt(cx * cx + cy * cy + 1.0) + t);
    v = v * 0.25;
    let rgb = vec3(sin(v * pi) * 0.5 + 0.5, sin(v * pi + 2.094) * 0.5 + 0.5, sin(v * pi + 4.189) * 0.5 + 0.5);
    #{ color: rgb, blend: p.blend_mode, amount: p.intensity }
}
