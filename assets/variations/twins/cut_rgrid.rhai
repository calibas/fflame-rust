/// Rotated-square offset-grid stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut RGrid, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    zoom: #{
        value: 7.0, min: 0.1, max: 50.0,
        caption: "Zoom",
        hint: "Pattern scale (cell count): `point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
    angle: #{
        value: 0, min: 0, max: 90,
        caption: "Angle",
        hint: "Grid rotation angle in degrees.",
    },
};

const INFO = #{ title: "Cut RGrid", name: "rhai_cut_rgrid", category: "plugin", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let zoom = p.zoom;
    let invert = to_int(to_float(p.invert));
    let angle = to_float(p.angle);
    let PI = 3.14159265359;
    let x = 0.0;
    let y = 0.0;
    if (mode == 0) {
        x = z.re;
        y = z.im;
    } else {
        x = rand() - 0.5;
        y = rand() - 0.5;
    }
    let pp = complex(x * zoom, y * zoom);
    let thr = angle * PI() / 180.0;
    let th = thr - (PI() * 2.0) * to_float(floor(thr / (PI() * 2.0)));
    let gridsize = (0.5 + abs(sin(th * 2.0)) * (sqrt(2.0) / 2.0 - 0.5)) * 2.0;
    let flip = 0;
    if (fract(th / PI() + 0.25) > 0.5) {
        pp = complex(pp.re - 0.5, pp.im - 0.5);
        flip = 1;
    }
    pp = pp * gridsize;
    let cp = floor(pp / gridsize);
    let t1 = (pp - gridsize * floor(pp / gridsize));
    let t2 = gridsize / 2.0;
    pp = complex(t1.re - t2, t1.im - t2);
    let modcp = cp - 2.0 * floor(cp / 2.0);
    let t4 = modcp * 2.0;
    let t3 = (complex(t4.re - 1.0, t4.im - 1.0));
    pp = complex(pp.re * t3.re, pp.im * t3.im);
    let ct = cos(th);
    let st = sin(th);
    pp = complex(ct * pp.re + st * pp.im, -st * pp.re + ct * pp.im);
    let w = zoom / 2000.0 * 1.5;
    let t5 = -w;
    let t6 = max(abs(pp.re), abs(pp.im)) - 0.5;
    let t7 = clamp((t6 - t5) / (w - t5), 0.0, 1.0);
    let color = (t7 * t7 * (3.0 - 2.0 * t7));
    if (flip == 1) {
        color = 1.0 - color;
    }
    let t8 = fract(th / PI()) - 0.5;
    if (flip == 1 && color < 0.5 && (abs(pp.re) - abs(pp.im)) * (if t8 > 0.0 { 1.0 } else if t8 < 0.0 { -1.0 } else { 0.0 }) > 0.0) {
        color = 0.4;
    }
    let csum = cp.re + cp.im;
    if (flip == 0 && color < 0.5 && (csum - 2.0 * to_float(floor(csum / 2.0)) - 0.5) > 0.0) {
        color = 0.4;
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
