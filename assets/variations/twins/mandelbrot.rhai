/// Random-walk Buddhabrot Mandelbrot. Per-thread state carries the
/// last seed point across iterations, so successive plots walk along
/// the fractal boundary instead of independently sampling. The
/// `skin` parameter controls the walk step; `invert` flips between
/// "plot seeds that escape" and "plot seeds that don't" semantics.
///
/// # Authors
/// - Jed Kelsey
/// - Andreas Maschke

// Mandelbrot, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    iter: #{
        value: 100, min: 1, max: 1000, free: true,
        caption: "Iter",
        hint: "Maximum escape iterations per seed. **GPU-clamped to 250** to stay within the TDR budget when combined with the 10-retry outer loop and per-dispatch thread count. Values above 250 in the .flame XML round-trip but render at 250.",
    },
    xmin: #{
        value: -1.6, min: -10.0, max: 10.0, free: true,
        caption: "X Min",
        hint: "Lower X bound of the random-seed sampling rectangle.",
    },
    xmax: #{
        value: 1.6, min: -10.0, max: 10.0, free: true,
        caption: "X Max",
        hint: "Upper X bound of the random-seed sampling rectangle.",
    },
    ymin: #{
        value: -1.2, min: -10.0, max: 10.0, free: true,
        caption: "Y Min",
        hint: "Lower Y bound of the random-seed sampling rectangle.",
    },
    ymax: #{
        value: 1.2, min: -10.0, max: 10.0, free: true,
        caption: "Y Max",
        hint: "Upper Y bound of the random-seed sampling rectangle.",
    },
    invert: #{
        value: 0, min: 0, max: 1, free: true,
        caption: "Invert",
        hint: "When 1, the acceptance criterion flips: plot seeds that escape *quickly* instead of seeds that don't escape. Treated as a probability — the random check runs each retry.",
    },
    skin: #{
        value: 0.012, min: 0.0, max: 1.0, free: true,
        caption: "Skin",
        hint: "Controls two things: (1) the random-walk step size (next seed is within `skin + 0.001` of the previous), and (2) the acceptance band for normal mode (`iter_count` must be < `0.1 · iter · (1 - skin)`, so smaller skin = stricter near-boundary filter). 1.0 disables the second filter entirely.",
    },
    cx: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "CX",
        hint: "Weight on the final iterate's X added to the seed X for output. 0 = pure-seed output (classic Buddhabrot point cloud); higher = blend with the iterate trajectory.",
    },
    cy: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "CY",
        hint: "Weight on the final iterate's Y.",
    },
    max_points: #{
        value: -1, min: -1, max: 1000000, free: true,
        caption: "Max Points",
        hint: "JWildfire CPU-only: caches the first N visited seeds and replays them. **Not honored on GPU** — value accepted for .flame XML round-trip but the shader always uses fresh randoms. Behavior matches JWildfire's own getGPUCode().",
    },
    seed: #{
        value: 1234, min: 0, max: 1000000, free: true,
        caption: "Seed",
        hint: "JWildfire CPU-only: seeds a per-instance Marsaglia RNG. **Not honored on GPU** — the shader uses the thread-shared RNG. Value accepted for round-trip.",
    },
    rnd_z_range: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Rnd Z Range",
        hint: "Range of the random Z value picked when starting a fresh seed. Final output Z is `random() · rnd_z_range`. 0 = always Z = 0 (flat 2D plot).",
    },
};

const INFO = #{ title: "Mandelbrot", name: "rhai_mandelbrot", category: "plugin", slow: ["iter"] };

fn init(p) {
    #{ x0: 0.0, y0: 0.0, z0: 0.0 }
}

fn variation(z, p) {
    let iter_param = u32(to_float(p.iter));
    let iter_safe = min(max(iter_param, u32(1)), u32(250));
    let xmin = p.xmin;
    let xmax = p.xmax;
    let ymin = p.ymin;
    let ymax = p.ymax;
    let invert_p = to_float(p.invert);
    let skin = p.skin;
    let cx = p.cx;
    let cy = p.cy;
    let rnd_z_range = p.rnd_z_range;
    let x0 = this.x0;
    let y0 = this.y0;
    let z0 = this.z0;
    let iter_f = to_float(iter_safe);
    let inverted = rand() < invert_p;
    let curr_iter = (if inverted { u32(0) } else { iter_safe });
    let x1 = x0;
    let y1 = y0;
    let x = x0;
    let y = y0;
    let k = u32(0);
    while true {
        let still_searching = (inverted && curr_iter < iter_safe) || (!inverted && (curr_iter >= iter_safe || (skin < 1.0 && to_float(curr_iter) < 0.1 * iter_f * (1.0 - skin))));
        if (k >= u32(10) || !still_searching) {
            break;
        }
        k = k + u32(1);
        if (x0 == 0.0 && y0 == 0.0) {
            x0 = (xmax - xmin) * rand() + xmin;
            y0 = (ymax - ymin) * rand() + ymin;
            z0 = rand() * rnd_z_range;
        } else {
            x0 = (skin + 0.001) * (rand() - 0.5) + x0;
            y0 = (skin + 0.001) * (rand() - 0.5) + y0;
        }
        x1 = x0;
        y1 = y0;
        x = x0;
        y = y0;
        curr_iter = u32(0);
        while true {
            if (curr_iter >= iter_safe || (x * x + y * y >= 4.0)) {
                break;
            }
            let xtemp = x * x - y * y + x0;
            y = 2.0 * x * y + y0;
            x = xtemp;
            curr_iter = curr_iter + u32(1);
        }
        let escaped_too_deep = curr_iter >= iter_safe;
        let escaped_too_fast = (skin < 1.0) && (to_float(curr_iter) < 0.1 * iter_f * (1.0 - skin));
        if (escaped_too_deep || skin == 1.0 || escaped_too_fast) {
            x0 = 0.0;
            y0 = 0.0;
        }
    }
    if (k >= u32(10)) {
        let far = 50000.0 - rand() * 100000.0;
        x1 = far;
        y1 = far;
    }
    this.x0 = x0;
    this.y0 = y0;
    this.z0 = z0;
    complex(x1 + cx * x, y1 + cy * y)
}

fn variation3(v, p) {
    let iter_param = u32(to_float(p.iter));
    let iter_safe = min(max(iter_param, u32(1)), u32(250));
    let xmin = p.xmin;
    let xmax = p.xmax;
    let ymin = p.ymin;
    let ymax = p.ymax;
    let invert_p = to_float(p.invert);
    let skin = p.skin;
    let cx = p.cx;
    let cy = p.cy;
    let rnd_z_range = p.rnd_z_range;
    let x0 = this.x0;
    let y0 = this.y0;
    let z0 = this.z0;
    let iter_f = to_float(iter_safe);
    let inverted = rand() < invert_p;
    let curr_iter = (if inverted { u32(0) } else { iter_safe });
    let x1 = x0;
    let y1 = y0;
    let x = x0;
    let y = y0;
    let k = u32(0);
    while true {
        let still_searching = (inverted && curr_iter < iter_safe) || (!inverted && (curr_iter >= iter_safe || (skin < 1.0 && to_float(curr_iter) < 0.1 * iter_f * (1.0 - skin))));
        if (k >= u32(10) || !still_searching) {
            break;
        }
        k = k + u32(1);
        if (x0 == 0.0 && y0 == 0.0) {
            x0 = (xmax - xmin) * rand() + xmin;
            y0 = (ymax - ymin) * rand() + ymin;
            z0 = rand() * rnd_z_range;
        } else {
            x0 = (skin + 0.001) * (rand() - 0.5) + x0;
            y0 = (skin + 0.001) * (rand() - 0.5) + y0;
        }
        x1 = x0;
        y1 = y0;
        x = x0;
        y = y0;
        curr_iter = u32(0);
        while true {
            if (curr_iter >= iter_safe || (x * x + y * y >= 4.0)) {
                break;
            }
            let xtemp = x * x - y * y + x0;
            y = 2.0 * x * y + y0;
            x = xtemp;
            curr_iter = curr_iter + u32(1);
        }
        let escaped_too_deep = curr_iter >= iter_safe;
        let escaped_too_fast = (skin < 1.0) && (to_float(curr_iter) < 0.1 * iter_f * (1.0 - skin));
        if (escaped_too_deep || skin == 1.0 || escaped_too_fast) {
            x0 = 0.0;
            y0 = 0.0;
        }
    }
    if (k >= u32(10)) {
        let far = 50000.0 - rand() * 100000.0;
        x1 = far;
        y1 = far;
    }
    this.x0 = x0;
    this.y0 = y0;
    this.z0 = z0;
    vec3(x1 + cx * x, y1 + cy * y, z0)
}
