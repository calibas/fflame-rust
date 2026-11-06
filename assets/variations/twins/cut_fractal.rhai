/// Abs/inversion Julia-style escape stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut Fractal, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    seed: #{
        value: 0, min: 0, max: 1000000,
        caption: "Seed",
        hint: "UI-only in JWildfire; ignored by the renderer.",
    },
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    time: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Time",
        hint: "Animates the fold constant `c`.",
    },
    iters: #{
        value: 30, min: 1, max: 100,
        caption: "Iterations",
        hint: "Number of abs/inversion fold iterations.",
    },
    zoom: #{
        value: 1.0, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale: `point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut Fractal", name: "rhai_cut_fractal", category: "plugin", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let time = p.time;
    let iters = to_int(to_float(p.iters));
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let x = 0.0;
    let y = 0.0;
    if (mode == 0) {
        x = z.re;
        y = z.im;
    } else {
        x = rand() - 0.5;
        y = rand() - 0.5;
    }
    let c = complex(cos(time + 1.5), sin(time + 1.8)) * 0.15 - complex(0.25, 0.25);
    let tt = complex(x * zoom, y * zoom);
    let u = complex(tt.im, tt.re);
    for i in 0..iters {
        u = complex(abs(u.re), abs(u.im)) * (0.5 / u.norm_sqr()) + c;
    }
    let color = abs(u.norm_sqr() - 0.01) * 2.0;
    let hidden = false;
    if (invert == 0) {
        if (color > 0.5) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (color <= 0.5) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(x, y), hide: hide_v }
}
