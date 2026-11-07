/// Interlocking-chain sine-field stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut Chains, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    shiftX: #{
        value: 0.0, min: -50.0, max: 50.0, free: true,
        caption: "Shift X",
        hint: "X offset added to `uv` after scaling.",
    },
    shiftY: #{
        value: 0.0, min: -50.0, max: 50.0, free: true,
        caption: "Shift Y",
        hint: "Y offset added to `uv` after scaling.",
    },
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    zoom: #{
        value: 2.5, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale: `uv = point·zoom + shift`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut Chains", name: "rhai_cut_chains", category: "plugin", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let shiftx = p.shiftX;
    let shifty = p.shiftY;
    let mode = to_int(to_float(p.mode));
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let x = 0.0;
    let y = 0.0;
    let cx = 0.0;
    let cy = 0.0;
    if (mode == 0) {
        x = z.re;
        y = z.im;
    } else {
        x = rand();
        y = rand();
        cx = 0.5;
        cy = 0.5;
    }
    let u = complex(x * zoom, y * zoom) + complex(shiftx, shifty);
    let wy = cos(u.im * 12.0);
    let wx = sin(u.re * 15.0);
    let w = cos(u.re * 30.0);
    let t1 = -0.25;
    let t2 = mix(wy + wx, wx * 1.4, w * 3.0);
    let t3 = clamp((t2 - t1) / (0.25 - t1), 0.0, 1.0);
    let color = (t3 * t3 * (3.0 - 2.0 * t3));
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
    #{ z: complex(x - cx, y - cy), hide: hide_v }
}
