// Hue Cycle: every colour's hue turned by an angle.
//
// The built-in `hue_shift` effect (docs/main/EFFECTS.md, Rhai effects): the
// app runs this file -- on desktop the copy beside the app, so an edit
// shows on the next start. It replaced shaders/effects/color/hue_cycle.wgsl, whose
// pixels, parameters and order it keeps, so a config cannot tell which
// it got.

const PARAMS = #{
    offset: #{ value: 0.0, min: 0.0, max: 360.0, type: "angle", caption: "Hue Offset" },
    intensity: #{ value: 1.0, min: 0.0, max: 1.0, caption: "Intensity" },
    blend_mode: #{
        value: 0,
        caption: "Blend Mode",
        choices: ["Normal", "Add", "Multiply", "Screen", "Overlay", "Soft Light", "Hard Light",
                  "Color Dodge", "Color Burn", "Hue", "Saturation", "Color", "Luminosity"],
    },
};
const INFO = #{ title: "Hue Shift", id: "hue_shift" };

// Hue, saturation and value, each 0 to 1.
fn rgb_to_hsv(rgb) {
    let c_max = max(rgb.r, max(rgb.g, rgb.b));
    let c_min = min(rgb.r, min(rgb.g, rgb.b));
    let delta = c_max - c_min;
    let h = 0.0;
    let s = 0.0;
    if delta > 0.0001 {
        s = delta / c_max;
        if c_max == rgb.r {
            h = (rgb.g - rgb.b) / delta;
            if rgb.g < rgb.b {
                h += 6.0;
            }
        } else if c_max == rgb.g {
            h = 2.0 + (rgb.b - rgb.r) / delta;
        } else {
            h = 4.0 + (rgb.r - rgb.g) / delta;
        }
        h /= 6.0;
    }
    vec3(h, s, c_max)
}

fn hsv_to_rgb(hsv) {
    let h = hsv.x * 6.0;
    let s = hsv.y;
    let v = hsv.z;
    let i = floor(h);
    let f = h - i;
    let p = v * (1.0 - s);
    let q = v * (1.0 - s * f);
    let t = v * (1.0 - s * (1.0 - f));
    // `h` is in [0, 6): its sextant is `i` itself.
    if i == 0.0 {
        vec3(v, t, p)
    } else if i == 1.0 {
        vec3(q, v, p)
    } else if i == 2.0 {
        vec3(p, v, t)
    } else if i == 3.0 {
        vec3(p, q, v)
    } else if i == 4.0 {
        vec3(t, p, v)
    } else {
        vec3(v, p, q)
    }
}

fn effect(c, p, ctx) {
    let hsv = rgb_to_hsv(c.rgb);
    hsv.x = fract(hsv.x + p.offset / 360.0);
    #{ color: hsv_to_rgb(hsv), blend: p.blend_mode, amount: p.intensity }
}
