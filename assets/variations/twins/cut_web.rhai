/// Log-polar spider-web grid stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut Web, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    seed: #{
        value: 1000, min: 0, max: 1000000,
        caption: "Seed",
        hint: "UI-only in JWildfire; ignored by the renderer.",
    },
    time: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Time",
        hint: "Scrolls the log-polar grid radially.",
    },
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    thick: #{
        value: 0.05, min: 0.0, max: 0.9,
        caption: "Thickness",
        hint: "Web strand thickness.",
    },
    invert: #{
        value: true,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut Web", name: "rhai_cut_web", category: "plugin", phase: "any", replace: true };

fn cut_web_eval(p0, c, strength) {
    let p = p0 - c;
    let l = ln(p.abs());
    let ang = atan2(p.im, p.re);
    complex(l, ang) * strength
}

fn cut_web_getColour(p, time, thick) {
    let PI = 3.14159265359;
    let fac = 10.0 / (2.0 * PI());
    let ep = cut_web_eval(p, complex(0.0, 0.0), 1.0);
    let d = ep.re * 0.05 * fac;
    ep = ep + complex(-ep.im, ep.re) * 4.0;
    let t1 = ep * fac;
    let arg = complex(t1.re + time, t1.im + time);
    let modded = arg - 2.0 * floor(arg / 2.0);
    let t3 = complex(modded.re - 1.0, modded.im - 1.0);
    let t2 = complex(abs(t3.re), abs(t3.im));
    let xx = complex(t2.re - thick, t2.im - thick);
    let t5 = -0.5 * d;
    let t4 = complex(t5, t5);
    let t7 = 0.5 * d;
    let t6 = complex(t7, t7);
    let t8 = clamp((xx.re - t4.re) / (t6.re - t4.re), 0.0, 1.0);
    let t9 = clamp((xx.im - t4.im) / (t6.im - t4.im), 0.0, 1.0);
    let si = complex(t8 * t8 * (3.0 - 2.0 * t8), t9 * t9 * (3.0 - 2.0 * t9));
    si.re + si.im
}

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let time = p.time;
    let mode = to_int(to_float(p.mode));
    let thick = p.thick;
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
    let u = complex(x * 0.5, y * 0.5);
    let color = cut_web_getColour(u, time, thick);
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
