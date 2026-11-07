/// Halfway-translate + conditional resample — first applies a halfway
/// translation of the input toward the target point `(X, Y)`. If the
/// translated point lands in a sparse cell (`noise(M+seed, N) > Dens`) or
/// outside the cell's circular radius, returns it directly. Otherwise
/// resamples a new point on a random circle within an active cell (up to 32
/// rejection attempts).

// Circle Trans 1, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    Sc: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale",
        hint: "Cell size — each cell occupies a `2·Sc × 2·Sc` region.",
    },
    Dens: #{
        value: 0.5, min: 0.0, max: 1.0, free: true,
        caption: "Density",
        hint: "Per-cell density threshold (probability that a cell is 'active' and triggers resampling).",
    },
    X: #{
        value: 10.0, min: -100.0, max: 100.0, free: true,
        caption: "X",
        hint: "X coordinate of the halfway-translate target point. Also bounds the resample rectangle (`|X|`).",
    },
    Y: #{
        value: 10.0, min: -100.0, max: 100.0, free: true,
        caption: "Y",
        hint: "Y coordinate of the halfway-translate target point. Also bounds the resample rectangle.",
    },
    Seed: #{
        value: 0, min: -1000, max: 1000,
        caption: "Seed",
        hint: "Hash seed for the per-cell deterministic noise.",
    },
};

const INFO = #{ title: "Circle Trans 1", name: "rhai_circletrans1", category: "advanced_2d", phase: "any" };

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
    let two_pi = 6.28318530717959;
    let ux = (z.re - xp) * 0.5 + xp;
    let uy = (z.im - yp) * 0.5 + yp;
    let m0 = to_float(floor(0.5 * ux / safe_sc));
    let n0 = to_float(floor(0.5 * uy / safe_sc));
    let mi0 = to_int(m0);
    let ni0 = to_int(n0);
    let dx0 = ux - (m0 * 2.0 + 1.0) * sc;
    let dy0 = uy - (n0 * 2.0 + 1.0) * sc;
    let u0 = sqrt(dx0 * dx0 + dy0 * dy0);
    let n0_seed = cr_disc_noise(mi0 + seed, ni0);
    let n0_radius = (0.3 + 0.7 * cr_disc_noise(mi0 + 10, ni0 + 3)) * sc;
    if (n0_seed > dens || u0 > n0_radius) {
        return complex(ux, uy);
    }
    let ox = 0.0;
    let oy = 0.0;
    let abs_xp = abs(xp);
    let abs_yp = abs(yp);
    for i in 0..32 {
        let rx = abs_xp * (1.0 - 2.0 * rand());
        let ry = abs_yp * (1.0 - 2.0 * rand());
        let m = to_float(floor(0.5 * rx / safe_sc));
        let n = to_float(floor(0.5 * ry / safe_sc));
        let mi = to_int(m);
        let ni = to_int(n);
        let alpha = two_pi * rand();
        let u_radius = 0.3 + 0.7 * cr_disc_noise(mi + 10, ni + 3);
        let dx = u_radius * cos(alpha);
        let dy = u_radius * sin(alpha);
        ox = dx + (m * 2.0 + 1.0) * sc;
        oy = dy + (n * 2.0 + 1.0) * sc;
        if (cr_disc_noise(mi + seed, ni) <= dens) {
            break;
        }
    }
    complex(ox, oy)
}
