/// Summed-distance kaleidoscope stencil mask (N orbiting centers).
///
/// # Authors
/// - Jesus Sosa

// Cut Kaleido, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    seed: #{
        value: 1000, min: 0, max: 1000000,
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
        hint: "Rotates the orbiting centers and modulates the ring frequency.",
    },
    n: #{
        value: 6, min: 2, max: 20,
        caption: "N",
        hint: "Number of orbiting distance centers.",
    },
    zoom: #{
        value: 0.5, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale: `uv = point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut Kaleido", name: "rhai_cut_kaleido", category: "plugin", phase: "any", replace: true };

fn cut_kaleido_distToColor(d) {
    0.0 - cos(d * 13.0)
}

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let time = p.time;
    let n = clamp(to_int(to_float(p.n)), 2, 20);
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
    let uv = complex(x * zoom, y * zoom);
    let ci = array(20, complex(0.0, 0.0));
    for i in 0..n {
        let fi = 2.0 * 3.14 * (to_float(i) + 0.02 * time) / to_float(n);
        ci[i] = complex(0.5 * sin(fi), 0.5 * cos(fi));
    }
    let d = 1.0;
    let k = 100.0 + 10.0 * sin(time / 5.0);
    for i in 0..n {
        d = d + sin(k * (uv - ci[i]).abs());
    }
    let color = cut_kaleido_distToColor(d / to_float(n));
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
