/// Yuebing (mooncake) radial-pattern stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut Yuebing, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call (the usual stencil mode).",
    },
    zoom: #{
        value: 0.25, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale: `position = point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
    p1: #{
        value: 2.0, min: -50.0, max: 50.0, free: true,
        caption: "P1",
        hint: "Fringe-frequency phase: `duration = sin(p1/2)·p2`.",
    },
    p2: #{
        value: 25.0, min: -100.0, max: 100.0, free: true,
        caption: "P2",
        hint: "Fringe-frequency amplitude: `duration = sin(p1/2)·p2`.",
    },
};

const INFO = #{ title: "Cut Yuebing", name: "rhai_cut_yuebing", category: "plugin", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let p1 = p.p1;
    let p2 = p.p2;
    let x = 0.0;
    let y = 0.0;
    if (mode == 0) {
        x = z.re;
        y = z.im;
    } else {
        x = rand() - 0.5;
        y = rand() - 0.5;
    }
    let posx = x * zoom;
    let posy = y * zoom;
    let duration = sin(p1 / 2.0) * p2;
    let color = sin(posx * posy * 10000.0 * duration);
    let hidden = false;
    if (invert == 0) {
        if (color < 0.3) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (color >= 0.3) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(x, y), hide: hide_v }
}
