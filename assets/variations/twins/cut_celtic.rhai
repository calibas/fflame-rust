/// Triple hex-offset Celtic-knot circle stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut Celtic, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    zoom: #{
        value: 5.0, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale (knot count): `point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut Celtic", name: "rhai_cut_celtic", category: "plugin", phase: "any", replace: true };

fn cut_celtic_circ(uv, r) {
    let d = uv.abs();
    let t1 = d + 0.02;
    let t2 = clamp((r - d) / (t1 - d), 0.0, 1.0);
    (t2 * t2 * (3.0 - 2.0 * t2))
}

fn cut_celtic_celticShit(uv, r) {
    let r1 = 0.38;
    let r2 = 0.45;
    let c1 = cut_celtic_circ(uv, r1);
    let c5 = cut_celtic_circ(uv, r2);
    let c2 = cut_celtic_circ(uv + complex(0.5, -0.288675), r2);
    let c3 = cut_celtic_circ(uv + complex(-0.5, -0.288675), r2);
    let c4 = cut_celtic_circ(uv + complex(0.0, 0.57735), r2);
    let d = c5 - c2 - c3 - c4;
    mix(0.0, d, c1)
}

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
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
    let uv1 = complex(x * 1.0, y * 1.155) * zoom;
    let m = uv1.im - 2.0 * to_float(floor(uv1.im / 2.0));
    uv1 = complex(uv1.re + (if m < 1.0 { 0.0 } else { 1.0 }) * 0.5, uv1.im);
    uv1 = fract(uv1);
    uv1 = complex(uv1.re, uv1.im / 1.155);
    uv1 = complex(uv1.re - 1.155 / 2.0, uv1.im);
    uv1 = complex(uv1.re, uv1.im - 0.5);
    let uv2 = complex(x * 1.0, y * 1.155) * zoom;
    uv2 = complex(uv2.re - 0.5, uv2.im);
    uv2 = complex(uv2.re, uv2.im - 0.288675);
    let n = uv2.im - 2.0 * to_float(floor(uv2.im / 2.0));
    uv2 = complex(uv2.re + (if n < 1.0 { 0.0 } else { 1.0 }) * 0.5, uv2.im);
    uv2 = fract(uv2);
    uv2 = complex(uv2.re, uv2.im / 1.155);
    uv2 = complex(uv2.re - 1.155 / 2.0, uv2.im);
    uv2 = complex(uv2.re, uv2.im - 0.5);
    let uv3 = complex(x * 1.0, y * 1.155) * zoom;
    uv3 = complex(uv3.re + 1.0, uv3.im);
    uv3 = complex(uv3.re, uv3.im - 0.65);
    let o = uv3.im - 2.0 * to_float(floor(uv3.im / 2.0));
    uv3 = complex(uv3.re + (if o < 1.0 { 0.0 } else { 1.0 }) * 0.5, uv3.im);
    uv3 = fract(uv3);
    uv3 = complex(uv3.re, uv3.im / 1.155);
    uv3 = complex(uv3.re - 1.155 / 2.0, uv3.im);
    uv3 = complex(uv3.re, uv3.im - 0.5);
    let color = cut_celtic_celticShit(uv1, 0.5) + cut_celtic_celticShit(uv2, 0.5) + cut_celtic_celticShit(uv3, 0.5);
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
