/// Interlocking jigsaw-piece tile stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut Jigsaw, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    seed: #{
        value: 1000, min: 0, max: 1000000,
        caption: "Seed",
        hint: "UI-only in JWildfire; ignored by the renderer.",
    },
    time: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Time",
        hint: "Scrolls the tiling along X.",
    },
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    zoom: #{
        value: 4.0, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale (tile count): `point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut Jigsaw", name: "rhai_cut_jigsaw", category: "plugin", phase: "any", replace: true };

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
        x = rand() - 0.5;
        y = rand() - 0.5;
    }
    let u = complex(x * zoom, y * zoom) + complex(1.0, 0.0) * time;
    let iv = ceil(u);
    let t1 = (iv - 2.0 * floor(iv / 2.0));
    let mv = complex(t1.re - 0.5, t1.im - 0.5);
    u = u - (complex(iv.re - 0.5, iv.im - 0.5));
    let dd = (if (mv.re * u.re + mv.im * u.im) < 0.0 { -mv } else { mv });
    let t3 = u - dd;
    let t2 = complex(abs(t3.re), abs(t3.im));
    let t4 = complex(1.0, 1.0);
    let f = (t2.re * t4.re + t2.im * t4.im) * 0.7 - 0.65;
    let c = (u - dd * 0.2).abs() - 0.2;
    let t5 = complex(27.0, 57.0);
    let dsc = sin((iv.re * t5.re + iv.im * t5.im)) * 1.0e5;
    let q = (fract(dsc) - 0.5) * dd.re < 0.0;
    let tt = (if q { max(f, 0.1 - c) } else { f });
    let sval = min(tt, c);
    let color = 1.0 - sval * 1.0e4 / 8.0;
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
