/// 64-step rotation-parity spiral stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut Spiral, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    time: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Time",
        hint: "Rotation angle per spiral step.",
    },
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    zoom: #{
        value: 2.5, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale: `uv = point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut Spiral", name: "rhai_cut_spiral", category: "plugin", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let time = p.time;
    let mode = to_int(to_float(p.mode));
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let x = 0.0;
    let y = 0.0;
    if (mode == 0) {
        x = z.re;
        y = z.im;
    } else {
        x = 2.0 * rand() - 1.0;
        y = 2.0 * rand() - 1.0;
    }
    let uv = complex(x * zoom, y * zoom);
    let a = 1.1 * sin(time);
    let b = 1.1 * cos(time);
    let l = 0;
    for i in 0..64 {
        uv = complex(a * uv.re + b * uv.im, -b * uv.re + a * uv.im);
        if (uv.im > 1.0) {
            break;
        }
        l = l ^ 1;
    }
    let color = to_float(l);
    let hidden = false;
    if (invert == 0) {
        if (color > 0.0) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (color <= 0.0) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(x, y), hide: hide_v }
}
