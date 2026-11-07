/// Sine/cosine interference stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut SinCos, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    seed: #{
        value: 1000, min: 0, max: 1000000,
        caption: "Seed",
        hint: "UI-only in JWildfire (reseeds and rederives `time` from the wall clock). Ignored by the renderer.",
    },
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call (the usual stencil mode).",
    },
    time: #{
        value: 0.9, min: -100.0, max: 100.0, free: true,
        caption: "Time",
        hint: "Advection phase: the band field scrolls along X by `time·0.5`.",
    },
    zoom: #{
        value: 0.5, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale: `uv = point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut. Note: at `invert=0` the lit band is hidden and the dark gaps are kept.",
    },
};

const INFO = #{ title: "Cut SinCos", name: "rhai_cut_sincos", category: "plugin", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let time = p.time;
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
    uv = complex(uv.re + (1.5), uv.im);
    uv = complex(uv.re, uv.im + (cos(uv.re * 10.0) * 0.15));
    uv = complex(uv.re, uv.im * (1.5));
    let r = 7.3;
    let e = uv.re - time * 0.5;
    let t = e + abs(uv.im);
    let f = (1.0 + sin(uv.re * 3.0)) * 0.4 + 0.1;
    let d = f - abs(fract(t * r) - 0.5) * 2.0;
    let t1 = 0.05 / max(0.0, d + 0.5);
    let t2 = d + 0.1;
    let t3 = clamp((t2 - 0.0) / (t1 - 0.0), 0.0, 1.0);
    let s = (t3 * t3 * (3.0 - 2.0 * t3));
    let color = sqrt(s);
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
