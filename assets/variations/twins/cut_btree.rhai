/// Recursive binary-tree band stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut BTree, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    seed: #{
        value: 0, min: 0, max: 1000000,
        caption: "Seed",
        hint: "UI-only in JWildfire; ignored by the renderer.",
    },
    time: #{
        value: 1.0, min: -100.0, max: 100.0, free: true,
        caption: "Time",
        hint: "Branch-level phase (via `fract(time)`).",
    },
    thickness: #{
        value: 5.0, min: 1.0, max: 10.0,
        caption: "Thickness",
        hint: "Branch thickness divisor: larger = thinner branches.",
    },
    zoom: #{
        value: 1.0, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale: `w = point·zoom`.",
    },
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    invert: #{
        value: true,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut BTree", name: "rhai_cut_btree", category: "plugin", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let time = p.time;
    let thickness = p.thickness;
    let zoom = p.zoom;
    let mode = to_int(to_float(p.mode));
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
    let w = complex(x * zoom, y * zoom);
    let tv = complex(fract(time), fract(time));
    let ref_nan_y = (w.im == 0.0);
    let l = complex(w.re * w.re, w.im * w.im) * 5.0 - tv;
    tv = complex(tv.re + 1.0, tv.im + 1.0);
    let t4 = ceil(l);
    let t3 = complex(pow(2.0, t4.re), pow(2.0, t4.im));
    let m = complex(t3.re * tv.re, t3.im * tv.im);
    let t5 = fract(m * w.re);
    w = complex(t5.re - 0.5, t5.im - 0.5);
    let t6 = w.re;
    let t7 = w.im;
    let t0 = complex((if t6 > 0.0 { 1.0 } else if t6 < 0.0 { -1.0 } else { 0.0 }), (if t7 > 0.0 { 1.0 } else if t7 < 0.0 { -1.0 } else { 0.0 })) / 4.0;
    let t10 = complex(0.0, 0.0);
    let t11 = complex(1.0, 1.0);
    let t12 = fract(l);
    let t13 = clamp((t12.re - t10.re) / (t11.re - t10.re), 0.0, 1.0);
    let t14 = clamp((t12.im - t10.im) / (t11.im - t10.im), 0.0, 1.0);
    let t9 = complex(t13 * t13 * (3.0 - 2.0 * t13), t14 * t14 * (3.0 - 2.0 * t14));
    let t8 = complex(t9.re * t0.re, t9.im * t0.im) - w;
    let t1 = complex(abs(t8.re), abs(t8.im));
    let t2 = complex(m.re / t1.re, m.im / t1.im) / (100.0 * thickness);
    let color = t2.im;
    let hidden = false;
    if (invert == 0) {
        if (color > 0.5 && !ref_nan_y) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (color <= 0.5 && !ref_nan_y) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(x, y), hide: hide_v }
}
