/// Apollonian gasket fold (inversion iteration) stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut Apollonian, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    levels: #{
        value: 4, min: 1, max: 10,
        caption: "Levels",
        hint: "Number of inversion-fold iterations.",
    },
    zoom: #{
        value: 2.0, min: 0.1, max: 50.0,
        caption: "Zoom",
        hint: "Pattern scale: `point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut Apollonian", name: "rhai_cut_apollonian", category: "plugin", phase: "any", replace: true };

fn cut_apollonian_apollo(xy, n) {
    let scale = 1.0;
    let pp = xy;
    for i in 0..n {
        let t2 = pp * 0.5;
        let t1 = fract(complex(t2.re + 0.5, t2.im + 0.5)) * 2.0;
        pp = complex(t1.re - 1.0, t1.im - 1.0);
        let k = 1.34 / pp.norm_sqr();
        pp = pp * k;
        scale = scale * k;
    }
    let d = 0.25 * abs(pp.im) / scale;
    let t3 = clamp((d - 0.001) / (0.002 - 0.001), 0.0, 1.0);
    d = (t3 * t3 * (3.0 - 2.0 * t3));
    d
}

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let levels = to_int(to_float(p.levels));
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
    let pt = complex(x * zoom, y * zoom);
    let col = cut_apollonian_apollo(pt, levels);
    let hidden = false;
    if (invert == 0) {
        if (col > 0.0) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (col <= 0.0) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(x, y), hide: hide_v }
}
