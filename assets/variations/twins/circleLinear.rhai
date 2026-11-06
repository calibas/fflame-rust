/// Grid of linear blobs — divides the input plane into a `2·Sc × 2·Sc` cell
/// grid and per-cell uses deterministic hash noise to either pass through,
/// scale by `K`, or remap to a noise-driven ring radius. `Dens1` and
/// `Dens2` control the density and ratio of the two non-identity branches;
/// `Reverse` swaps which sub-branch gets the scale and which gets the ring
/// map.

// Circle Linear, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    Sc: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Scale",
        hint: "Cell size — each cell occupies a `2·Sc × 2·Sc` region.",
    },
    K: #{
        value: 0.5, min: -10.0, max: 10.0, free: true,
        caption: "K",
        hint: "Linear-scale factor for the non-identity branch.",
    },
    Dens1: #{
        value: 0.5, min: 0.0, max: 1.0, free: true,
        caption: "Density 1",
        hint: "Per-cell density threshold (probability that the cell is 'active' and modifies the output).",
    },
    Dens2: #{
        value: 0.5, min: 0.0, max: 1.0, free: true,
        caption: "Density 2",
        hint: "Sub-density ratio for the ring-vs-scale split inside an active cell.",
    },
    Reverse: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Reverse",
        hint: "When > 0, swaps which sub-branch gets the scale vs the ring map.",
    },
    X: #{
        value: 10.0, min: -100.0, max: 100.0, free: true,
        caption: "X",
        hint: "Unused in the body — preserved for cpp parity and preset compatibility.",
    },
    Y: #{
        value: 10.0, min: -100.0, max: 100.0, free: true,
        caption: "Y",
        hint: "Unused in the body — preserved for cpp parity and preset compatibility.",
    },
    Seed: #{
        value: 0, min: -1000, max: 1000,
        caption: "Seed",
        hint: "Hash seed for the per-cell deterministic noise.",
    },
};

const INFO = #{ title: "Circle Linear", name: "rhai_circlelinear", category: "advanced_2d", phase: "any" };

fn cl_disc_noise(x, y) {
    let n = x + y * 57;
    n = (n << u32(13)) ^ n;
    let h = (n * (n * n * 15731 + 789221) + 1376312589) & 0x7fffffff;
    to_float(h) * (1.0 / 2147483647.0)
}

fn variation(w, p) {
    let sc = p.Sc;
    let k = p.K;
    let dens1 = p.Dens1;
    let dens2 = p.Dens2;
    let reverse = p.Reverse;
    let seed = to_int(to_float(p.Seed));
    let safe_sc = (if abs(sc) < 1.0e-30 { 1.0e-30 } else { sc });
    let m = to_float(floor(0.5 * w.re / safe_sc));
    let n = to_float(floor(0.5 * w.im / safe_sc));
    let mi = to_int(m);
    let ni = to_int(n);
    let x = w.re - (m * 2.0 + 1.0) * sc;
    let y = w.im - (n * 2.0 + 1.0) * sc;
    let u = sqrt(x * x + y * y);
    let v = (0.3 + 0.7 * cl_disc_noise(mi + 10, ni + 3)) * sc;
    let z1 = cl_disc_noise(mi + seed, ni);
    if (z1 < dens1 && u < v) {
        if (reverse > 0.0) {
            if (z1 < dens1 * dens2) {
                x = k * x;
                y = k * y;
            } else {
                let safe_u = (if abs(u) < 1.0e-30 { 1.0e-30 } else { u });
                let z = v / safe_u * (1.0 - k) + k;
                x = z * x;
                y = z * y;
            }
        } else if (z1 > dens1 * dens2) {
            x = k * x;
            y = k * y;
        } else {
            let safe_u = (if abs(u) < 1.0e-30 { 1.0e-30 } else { u });
            let z = v / safe_u * (1.0 - k) + k;
            x = z * x;
            y = z * y;
        }
    }
    complex(x + (m * 2.0 + 1.0) * sc, y + (n * 2.0 + 1.0) * sc)
}
