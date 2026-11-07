/// Mirrored zig-zag stripe-tile stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut ZigZag, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    xpar: #{
        value: 1.0, min: -50.0, max: 50.0, free: true,
        caption: "X Par",
        hint: "X scale factor applied after zoom.",
    },
    ypar: #{
        value: 2.0, min: -50.0, max: 50.0, free: true,
        caption: "Y Par",
        hint: "Y scale factor applied after zoom (controls zig frequency).",
    },
    zoom: #{
        value: 2.0, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale: `st = point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut ZigZag", name: "rhai_cut_zigzag", category: "plugin", phase: "any", replace: true };

fn cut_zigzag_mirrorTile(st0) {
    let st = st0;
    if (fract(st.im * 0.5) > 0.5) {
        st = complex(st.re + 0.5, st.im);
        st = complex(st.re, 1.0 - st.im);
    }
    fract(st)
}

fn cut_zigzag_fillY(st, pct, antia) {
    let t1 = pct - antia;
    let t2 = st.im;
    let t3 = clamp((t2 - t1) / (pct - t1), 0.0, 1.0);
    (t3 * t3 * (3.0 - 2.0 * t3))
}

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let xpar = p.xpar;
    let ypar = p.ypar;
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let PI = 3.14159265359;
    let xp = 0.0;
    let yp = 0.0;
    if (mode == 0) {
        xp = z.re;
        yp = z.im;
    } else {
        xp = rand() - 0.5;
        yp = rand() - 0.5;
    }
    let st = complex(xp, yp);
    st = st * zoom;
    let t1 = complex(xpar, ypar);
    st = complex(st.re * t1.re, st.im * t1.im);
    st = cut_zigzag_mirrorTile(st);
    let xx = st.re * 2.0;
    let a = to_float(floor(1.0 + sin(xx * PI())));
    let b = to_float(floor(1.0 + sin((xx + 1.0) * PI())));
    let f = fract(xx);
    let color = cut_zigzag_fillY(st, mix(a, b, f), 0.01);
    let hidden = false;
    if (invert == 0) {
        if (color >= 0.5) {
            xp = 0.0;
            yp = 0.0;
            hidden = true;
        }
    } else if (color < 0.5) {
        xp = 0.0;
        yp = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(xp, yp), hide: hide_v }
}
