/// Sine-warped tiled distance-band stencil mask.
///
/// # Authors
/// - Jesus Sosa

// Cut SWarp, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    seed: #{
        value: 1000, min: 0, max: 1000000,
        caption: "Seed",
        hint: "UI-only in JWildfire; ignored by the renderer.",
    },
    mode: #{
        value: 1, choices: ["Affine Input", "Random Point"],
        caption: "Mode",
        hint: "Affine Input samples the incoming point; Random Point samples a fresh uniform point each call.",
    },
    time: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "Time",
        hint: "Animates the sine warp and band phase.",
    },
    type: #{
        value: 0, choices: ["Euclidean", "Chebyshev", "Hexagonal", "Octagonal"],
        caption: "Type",
        hint: "Distance metric for the band field.",
    },
    zoom: #{
        value: 1.0, min: -50.0, max: 50.0, free: true,
        caption: "Zoom",
        hint: "Pattern scale: `uv = point·zoom`.",
    },
    invert: #{
        value: false,
        caption: "Invert",
        hint: "Invert the cut — keep what would be hidden and vice versa.",
    },
};

const INFO = #{ title: "Cut SWarp", name: "rhai_cut_swarp", category: "plugin", phase: "any", replace: true };

fn cut_swarp_dist(p0, btype) {
    let distance = 0.0;
    if (btype == 0) {
        distance = p0.abs();
    }
    let p = complex(abs(p0.re), abs(p0.im));
    if (btype == 1) {
        distance = max(p.re, p.im);
    }
    if (btype == 2) {
        distance = max(p.re * 0.8660254 + p.im * 0.5, p.im);
    }
    if (btype == 3) {
        distance = max(max(p.re, p.im), (p.re + p.im) * 0.7071);
    }
    distance
}

fn variation(z, p, ctx) {
    let hide_v = ctx.hide;
    let mode = to_int(to_float(p.mode));
    let time = p.time;
    let btype = to_int(to_float(p.type));
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
    let t3 = uv * 11.0;
    let t2 = complex(t3.re + time, t3.im + time);
    let t1 = uv * 5.0 + complex(cos(t2.re), cos(t2.im));
    let v0 = complex(t1.re + time, t1.im + time);
    let v1 = complex(sin(v0.re), sin(v0.im));
    let t4 = complex(0.5, 0.5);
    let t0 = (v1.re * t4.re + v1.im * t4.im);
    let t5 = t0 * 0.02;
    uv = complex(uv.re + t5, uv.im + t5);
    uv = complex(uv.re, uv.im + sin(uv.re * 7.0 + cos(uv.re * 4.0 + time * 2.0) * 1.0 + time) * 0.05);
    let s = to_float(ceil(uv.re)) * 2.0 - 1.0;
    let m = 6.0;
    uv = complex(uv.re, uv.im + s / m);
    let n = 5.0;
    let k = sin(s * 3.14159 / 2.0 + cut_swarp_dist(uv, btype) * n * m * 3.14159 / 2.0);
    let color = sqrt(max(0.0, k));
    let hidden = false;
    if (invert == 0) {
        if (color > 0.5) {
            x = 0.0;
            y = 0.0;
            hidden = true;
        }
    } else if (color <= 0.5) {
        x = 0.0;
        y = 0.0;
        hidden = true;
    }
    hide_v = hidden;
    #{ z: complex(x, y), hide: hide_v }
}
