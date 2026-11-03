// Kaleidoscope: one slice of the image folded into mirrored segments.
//
// The built-in `kaleidoscope` effect (docs/main/EFFECTS.md, Rhai effects): the
// app runs this file -- on desktop the copy beside the app, so an edit
// shows on the next start. It replaced shaders/effects/color/kaleidoscope.wgsl, whose
// pixels, parameters and order it keeps, so a config cannot tell which
// it got.

const PARAMS = #{
    segments: #{ value: 6.0, min: 2.0, max: 16.0, caption: "Segments" },
    rotation: #{ value: 0.0, min: 0.0, max: 360.0, type: "angle", caption: "Source Angle" },
    zoom: #{ value: 1.0, min: 0.1, max: 3.0, caption: "Zoom" },
    intensity: #{ value: 1.0, min: 0.0, max: 1.0, caption: "Intensity" },
    blend_mode: #{
        value: 0,
        caption: "Blend Mode",
        choices: ["Normal", "Add", "Multiply", "Screen", "Overlay", "Soft Light", "Hard Light",
                  "Color Dodge", "Color Burn", "Hue", "Saturation", "Color", "Luminosity"],
    },
    square_mode: #{ value: 1, caption: "Square Mode", choices: ["Screen ratio (stretched)", "Square"] },
    edge_offset: #{ value: 0, min: 0, max: 2000, caption: "Edge Offset" },
};
const INFO = #{ title: "Kaleidoscope", id: "kaleidoscope" };

fn effect(c, p, ctx) {
    let pi = 3.14159265359;
    let two_pi = 6.28318530718;
    let segments = max(2.0, p.segments);
    let rotation = p.rotation * pi / 180.0;
    let zoom = max(0.1, p.zoom);
    let square = p.square_mode > 0;
    let edge_offset = to_float(p.edge_offset) * ctx.pixel_scale;
    let aspect = to_float(ctx.width) / to_float(ctx.height);

    // In square mode the folding happens in a square space, so circles
    // stay circles.
    let x = (ctx.uv.re - 0.5) / zoom;
    let y = (ctx.uv.im - 0.5) / zoom;
    if square {
        x = x * aspect;
    }
    let radius = complex(x, y).abs();
    let angle = atan2(y, x);
    // Folded into one segment, and mirrored within it.
    let segment = two_pi / segments;
    angle = angle - segment * floor(angle / segment);
    if angle > segment * 0.5 {
        angle = segment - angle;
    }
    // Which slice of the image is sampled, after the folding.
    angle = angle + rotation;
    let rx = cos(angle) * radius;
    let ry = sin(angle) * radius;
    if square {
        rx = rx / aspect;
    }

    // Shifted toward the centre, away from the slice, so the samples
    // stay on the image.
    let dx = -cos(rotation);
    let dy = -sin(rotation);
    let u = rx + 0.5 + dx * edge_offset * (1.0 / to_float(ctx.width));
    let v = ry + 0.5 + dy * edge_offset * (1.0 / to_float(ctx.height));
    let kaleido = input(complex(clamp(u, 0.001, 0.999), clamp(v, 0.001, 0.999)));
    #{ color: kaleido.rgb, blend: p.blend_mode, amount: p.intensity }
}
