/// Whorled fingerprint-ridge stencil mask (atan accumulation).
///
/// # Authors
/// - Jesus Sosa

// Cut FingerPrint, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    seed: #{
        value: 10000, min: 0, max: 1000000,
        caption: "Seed",
        hint: "Selects the ridge whorl pattern (`floor(7·seed)` seeds the hash walk).",
    },
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    zoom: #{
        value: 20.0, min: 1.0, max: 100.0,
        caption: "Zoom",
        hint: "Pattern scale: `uv = point·zoom`.",
    },
    width: #{
        value: 0.8, min: 0.02, max: 1.0,
        caption: "Width",
        hint: "Ridge width fraction.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut FingerPrint", name: "rhai_cut_fingerprint", category: "plugin", phase: "any", replace: true };

fn cut_fingerprint_hash2(p0) {
    let t1 = complex(63.31, 127.63);
    let t2 = complex(395.467, 213.799);
    let p = complex((p0.re * t1.re + p0.im * t1.im), (p0.re * t2.re + p0.im * t2.im));
    let t3 = fract(complex(sin(p.re), sin(p.im)) * 43141.59265) * 2.0;
    complex(t3.re - 1.0, t3.im - 1.0)
}

fn cut_fingerprint_getColor(uv0, seed, width) {
    let t2 = complex(0.7, 0.5);
    let t1 = (complex(uv0.re * t2.re, uv0.im * t2.im)).abs();
    let t3 = clamp((t1 - 9.0) / (10.0 - 9.0), 0.0, 1.0);
    let bounds = (t3 * t3 * (3.0 - 2.0 * t3));
    let a = 0.0;
    let h = complex(to_float(floor(7.0 * seed)), 0.0);
    for i in 0..50 {
        let t4 = h.re;
        let s = (if t4 > 0.0 { 1.0 } else if t4 < 0.0 { -1.0 } else { 0.0 });
        let t5 = cut_fingerprint_hash2(h);
        let t6 = complex(15.0, 20.0);
        h = complex(t5.re * t6.re, t5.im * t6.im);
        a = a + s * atan2(uv0.re - h.re, uv0.im - h.im);
    }
    let t7 = cut_fingerprint_hash2(h);
    let uv = uv0 + complex(abs(t7.re), abs(t7.im));
    a = a + atan2(uv.im, uv.re);
    let pp = (1.0 - bounds) * width;
    let s = min(0.3, pp);
    let l = uv.abs() + 0.319 * a;
    let m = l - 2.0 * to_float(floor(l / 2.0));
    let t8 = 2.0 - s;
    let t9 = clamp((m - t8) / (2.0 - t8), 0.0, 1.0);
    let t10 = pp + s;
    let t11 = clamp((m - pp) / (t10 - pp), 0.0, 1.0);
    let v = (1.0 - (t9 * t9 * (3.0 - 2.0 * t9))) * (t11 * t11 * (3.0 - 2.0 * t11));
    v
}

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let seed = to_float(p.seed);
    let mode = to_int(to_float(p.mode));
    let zoom = p.zoom;
    let width = p.width;
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
    let uv = complex(x, y) * zoom;
    let color = cut_fingerprint_getColor(uv, seed, width);
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
