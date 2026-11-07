/// Hexagonal nearest-cell dot stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut HexDots, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    size: #{
        value: 0.5, min: 0.01, max: 1.0,
        caption: "Size",
        hint: "Dot threshold: larger keeps more of each hex cell.",
    },
    zoom: #{
        value: 4.0, min: 0.1, max: 50.0,
        caption: "Zoom",
        hint: "Pattern scale: `uv = point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut HexDots", name: "rhai_cut_hexdots", category: "plugin", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let size = p.size;
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
    let u = complex(x * zoom, y * zoom);
    let s = complex(1.0, 1.732);
    let t1 = floor(complex(u.re / s.re, u.im / s.im));
    let a = (u - complex(s.re * t1.re, s.im * t1.im)) * 2.0 - s;
    let ub = u + s * 0.5;
    let t2 = floor(complex(ub.re / s.re, ub.im / s.im));
    let b = (ub - complex(s.re * t2.re, s.im * t2.im)) * 2.0 - s;
    let col = 0.8 * min(a.norm_sqr(), b.norm_sqr());
    let hidden = false;
    if (invert == 0) {
        if (col > size) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (col <= size) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(x, y), hide: hide_v }
}
