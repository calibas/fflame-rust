/// Sheared wavy-tile checkerboard-illusion stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut TileIllusion, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    time: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Time",
        hint: "Horizontal shear offset alternating per row (drives the illusion).",
    },
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    zoom: #{
        value: 15.0, min: -100.0, max: 100.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale: `uv = point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut TileIllusion", name: "rhai_cut_tileillusion", category: "plugin", phase: "any", replace: true };

fn cut_tileillusion_f(x) {
    x + 0.1 * sin(1.6 * x)
}

fn cut_tileillusion_solve(x0i, x1i, y) {
    let x0 = x0i;
    let x1 = x1i;
    let y0 = cut_tileillusion_f(x0);
    let y1 = cut_tileillusion_f(x1);
    if (y1 < y0) {
        let x2 = x1;
        x1 = x0;
        x0 = x2;
        let y2 = y1;
        y1 = y0;
        y0 = y2;
    }
    let xn = 0.0;
    let yn = 0.0;
    for i in 0..20 {
        xn = x0 + (x1 - x0) / (y1 - y0) * (y - y0);
        yn = cut_tileillusion_f(xn);
        if (yn > y) {
            x1 = xn;
            y1 = yn;
        } else {
            x0 = xn;
            y0 = yn;
        }
    }
    xn
}

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
    let uv = complex(x * zoom, y * zoom);
    let fuvy = cut_tileillusion_f(uv.im);
    let y0 = cut_tileillusion_solve(uv.im - 3.0, uv.im, to_float(floor(fuvy)));
    let y1 = cut_tileillusion_solve(uv.im, uv.im + 3.0, to_float(floor(fuvy)) + 1.0);
    uv = complex(uv.re, fuvy);
    uv = complex(uv.re / (y1 - y0), uv.im);
    let fy = to_float(floor(uv.im));
    let s = fy - 2.0 * to_float(floor(fy / 2.0));
    let color = 0.0;
    if (fract(uv.im) > 0.05) {
        let t1 = s - 0.5;
        uv = complex(uv.re + time * (if t1 > 0.0 { 1.0 } else if t1 < 0.0 { -1.0 } else { 0.0 }), uv.im);
        let cc = to_float(floor(uv.re)) + to_float(floor(uv.im));
        color = cc - 2.0 * to_float(floor(cc / 2.0));
    }
    let hidden = false;
    if (invert == 0) {
        if (color == 0.0) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (color > 0.0) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(x, y), hide: hide_v }
}
