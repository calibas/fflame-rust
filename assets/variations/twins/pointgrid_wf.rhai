/// Point grid with deterministic jitter — each iteration picks a random
/// cell from an `xcount × ycount` grid spanning `[xmin, xmax] × [ymin,
/// ymax]`, then optionally jitters that cell's position by a deterministic
/// per-cell offset (hashed from `seed` + cell index, scaled by
/// `distortion`). Useful for placing IFS attractors at grid points with a
/// controllable random perturbation.
///
/// # Authors
/// - Andreas Maschke

// Point Grid WF, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    xmin: #{
        value: -3.0, min: -100.0, max: 100.0, free: true,
        caption: "X Min",
        hint: "Left edge of the grid region on the X axis.",
    },
    xmax: #{
        value: 3.0, min: -100.0, max: 100.0, free: true,
        caption: "X Max",
        hint: "Right edge of the grid region on the X axis.",
    },
    xcount: #{
        value: 32, min: 1, max: 1000,
        caption: "X Count",
        hint: "Number of grid cells along X.",
    },
    ymin: #{
        value: -3.0, min: -100.0, max: 100.0, free: true,
        caption: "Y Min",
        hint: "Bottom edge of the grid region on the Y axis.",
    },
    ymax: #{
        value: 3.0, min: -100.0, max: 100.0, free: true,
        caption: "Y Max",
        hint: "Top edge of the grid region on the Y axis.",
    },
    ycount: #{
        value: 32, min: 1, max: 1000,
        caption: "Y Count",
        hint: "Number of grid cells along Y.",
    },
    distortion: #{
        value: 2.3, min: -10.0, max: 10.0, free: true,
        caption: "Distortion",
        hint: "Per-cell jitter amplitude. 0 = no jitter (pure grid); larger = more scatter around each cell center.",
    },
    seed: #{
        value: 1234, min: -100000, max: 100000,
        caption: "Seed",
        hint: "Hash seed for the per-cell jitter — changing it reshuffles the jitter pattern.",
    },
};

const INFO = #{ title: "Point Grid WF", name: "rhai_pointgrid_wf", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let xc = max(1.0, to_float(p.xcount));
    let yc = max(1.0, to_float(p.ycount));
    #{ dx: (p.xmax - p.xmin) / xc, dy: (p.ymax - p.ymin) / yc }
}

fn pg_disc_noise(x, y) {
    let n = x + y * 57;
    n = (n << u32(13)) ^ n;
    let h = (n * (n * n * 15731 + 789221) + 1376312589) & 0x7fffffff;
    to_float(h) * (1.0 / 2147483647.0)
}

fn variation(z, p) {
    let xmin = p.xmin;
    let xcount = max(to_int(to_float(p.xcount)), 1);
    let ymin = p.ymin;
    let ycount = max(to_int(to_float(p.ycount)), 1);
    let distortion = p.distortion;
    let seed = to_int(to_float(p.seed));
    let dx = p.dx;
    let dy = p.dy;
    let x_idx = clamp(to_int(rand() * to_float(xcount)), 0, xcount - 1);
    let y_idx = clamp(to_int(rand() * to_float(ycount)), 0, ycount - 1);
    let x = xmin + dx * to_float(x_idx);
    let y = ymin + dy * to_float(y_idx);
    if (distortion > 0.0) {
        let distx = (0.5 - pg_disc_noise(seed + 1563 + x_idx, y_idx)) * distortion;
        let disty = (0.5 - pg_disc_noise(seed + 6715 + y_idx, x_idx)) * distortion;
        x = x + distx;
        y = y + disty;
    }
    complex(x, y)
}
