/// Voronoi-cell snap with hash noise — scans the 3×3 grid of cells around
/// the input, generates 1–`num` deterministic Voronoi site positions per
/// cell via a bit-mixed integer hash, finds the nearest site, and lerps the
/// input toward it by factor `k`. Produces classic Voronoi cellular
/// patterns.
///
/// # Authors
/// - eralex61

// Voron, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    k: #{
        value: 0.99, min: -10.0, max: 10.0, free: true,
        caption: "K",
        hint: "Lerp factor toward the nearest Voronoi site. 1 = snap fully to site; 0 = pass through unchanged.",
    },
    step: #{
        value: 0.25, min: 0.001, max: 10.0, free: true,
        caption: "Step",
        hint: "Voronoi cell size.",
    },
    num: #{
        value: 1, min: 1, max: 5,
        caption: "Num",
        hint: "Maximum sites per cell (1–5). Actual count per cell is hashed from the cell index.",
    },
    xseed: #{
        value: 3, min: -1000, max: 1000,
        caption: "X seed",
        hint: "Hash seed for X-coordinate site generation.",
    },
    yseed: #{
        value: 7, min: -1000, max: 1000,
        caption: "Y seed",
        hint: "Hash seed for Y-coordinate site generation.",
    },
};

const INFO = #{ title: "Voron", name: "rhai_voron", category: "advanced_2d", phase: "any" };

fn voron_noise(x_seed) {
    let n = x_seed;
    n = (n << 13) ^ n;
    let v = n * (n * n * 15731 + 789221) + 1376312589;
    to_float(v & 0x7fffffff) * 4.6566128752457969e-10
}

fn variation(z, p) {
    let k = p.k;
    let step = p.step;
    let num = max(1.0, to_float(p.num));
    let xseed = to_int(to_float(p.xseed));
    let yseed = to_int(to_float(p.yseed));
    let safe_step = (if abs(step) < 1.0e-30 { 1.0e-30 } else { step });
    let rmin = 20.0;
    let x0 = 0.0;
    let y0 = 0.0;
    let m = to_int(to_float(floor(z.re / safe_step)));
    let n = to_int(to_float(floor(z.im / safe_step)));
    for i in -1..2 {
        for j in -1..2 {
            let m1 = m + i;
            let n1 = n + j;
            let kk = to_int(1.0 + to_float(floor(num * voron_noise(19 * m1 + 257 * n1 + xseed))));
            for l in 0..kk {
                let xc = (voron_noise(l + 64 * m1 + 15 * n1 + xseed) + to_float(m1)) * step;
                let yc = (voron_noise(l + 21 * m1 + 33 * n1 + yseed) + to_float(n1)) * step;
                let dx = z.re - xc;
                let dy = z.im - yc;
                let r = sqrt(dx * dx + dy * dy);
                if (r < rmin) {
                    rmin = r;
                    x0 = xc;
                    y0 = yc;
                }
            }
        }
    }
    complex(k * (z.re - x0) + x0, k * (z.im - y0) + y0)
}
