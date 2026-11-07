/// Random-circle sampler — repeatedly samples a random `(rx, ry)` point in
/// the rectangle `[-X, X] × [-Y, Y]`, checks whether the point falls inside
/// an 'active' cell (cell density ≤ `Dens` AND distance from cell center ≤
/// a hashed per-cell radius). Returns the first accepted sample. Rejection-
/// sampling loop is capped at 32 iterations to avoid WGSL non-uniform-
/// control-flow issues; the last sample is returned if none accept.

// Circle Rand, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    Sc: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale",
        hint: "Cell size — each cell occupies a `2·Sc × 2·Sc` region.",
    },
    Dens: #{
        value: 0.5, min: 0.0, max: 1.0, free: true,
        caption: "Density",
        hint: "Per-cell density threshold (probability that a cell is 'active' and accepts a sample).",
    },
    X: #{
        value: 10.0, min: -100.0, max: 100.0, free: true,
        caption: "X",
        hint: "Bounding-rectangle X half-extent (samples drawn from `[-X, X]`).",
    },
    Y: #{
        value: 10.0, min: -100.0, max: 100.0, free: true,
        caption: "Y",
        hint: "Bounding-rectangle Y half-extent.",
    },
    Seed: #{
        value: 0, min: -1000, max: 1000,
        caption: "Seed",
        hint: "Hash seed for the per-cell deterministic noise.",
    },
};

const INFO = #{ title: "Circle Rand", name: "rhai_circlerand", category: "advanced_2d", phase: "any" };

fn cr_disc_noise(x, y) {
    let n = x + y * 57;
    n = (n << u32(13)) ^ n;
    let h = (n * (n * n * 15731 + 789221) + 1376312589) & 0x7fffffff;
    to_float(h) * (1.0 / 2147483647.0)
}

fn variation(z, p) {
    let sc = p.Sc;
    let dens = p.Dens;
    let xp = p.X;
    let yp = p.Y;
    let seed = to_int(to_float(p.Seed));
    let safe_sc = (if abs(sc) < 1.0e-30 { 1.0e-30 } else { sc });
    let ox = 0.0;
    let oy = 0.0;
    for i in 0..32 {
        let rx = xp * (1.0 - 2.0 * rand());
        let ry = yp * (1.0 - 2.0 * rand());
        let m = to_float(floor(0.5 * rx / safe_sc));
        let n = to_float(floor(0.5 * ry / safe_sc));
        let mi = to_int(m);
        let ni = to_int(n);
        let dx = rx - (m * 2.0 + 1.0) * sc;
        let dy = ry - (n * 2.0 + 1.0) * sc;
        let u = sqrt(dx * dx + dy * dy);
        ox = dx + (m * 2.0 + 1.0) * sc;
        oy = dy + (n * 2.0 + 1.0) * sc;
        let n_seed = cr_disc_noise(mi + seed, ni);
        let n_radius = (0.3 + 0.7 * cr_disc_noise(mi + 10, ni + 3)) * sc;
        if (n_seed <= dens && u <= n_radius) {
            break;
        }
    }
    complex(ox, oy)
}
