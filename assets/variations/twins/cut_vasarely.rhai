/// Vasarely op-art bulge-grid stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut Vasarely, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call (the usual stencil mode).",
    },
    zoom: #{
        value: 1.0, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale: `uv = point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
    size: #{
        value: 0.5, min: 0.001, max: 50.0, free: true,
        caption: "Size",
        hint: "Central-bulge radius: `b = max(0, 2 − l/size)`.",
    },
};

const INFO = #{ title: "Cut Vasarely", name: "rhai_cut_vasarely", category: "plugin", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let size = p.size;
    let x = 0.0;
    let y = 0.0;
    if (mode == 0) {
        x = z.re;
        y = z.im;
    } else {
        x = 2.0 * rand() - 1.0;
        y = 2.0 * rand() - 1.0;
    }
    let uv = complex(x, y) * zoom;
    let l = uv.abs();
    let b = max(0.0, 2.0 - l / size);
    let cond = abs(uv.re) < 1.0;
    let color = 0.0;
    color = (if cond { color - 0.5 - sin(l * sin(atan2(uv.im, uv.re) + 1.57 + 4.0 * b * b) * 150.0) } else { color });
    color = color - (if cond { color - 0.5 - sin(l * sin(atan2(uv.im, uv.re) + 1.57 + 4.0 * b * b) * 150.0) } else { color });
    let hidden = false;
    if (invert == 0) {
        if (color < 0.1) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (color >= 0.1) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(x, y), hide: hide_v }
}
