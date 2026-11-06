/// Truchet variant with interpolated parameters — same tile algorithm as
/// `truchet`, but the exponent and width are interpolated linearly based on
/// the X-position within each cell (`xp ∈ [0, 1]` via fmod-and-fold). Cells
/// transition smoothly between `(exponent1, width1)` at the left and
/// `(exponent2, width2)` at the right. `inverse = 1` plots the complement
/// (cells outside the arcs) instead.
///
/// # Authors
/// - Tatyana Zabanova
/// - Brad Stefanov
/// - Jesus Sosa

// Truchet 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    exponent1: #{
        value: 1.0, min: -1.0, max: 3.0, free: true,
        caption: "Exponent 1",
        hint: "Lp-norm exponent at the left edge of each cell.",
    },
    exponent2: #{
        value: 2.0, min: -1.0, max: 3.0, free: true,
        caption: "Exponent 2",
        hint: "Lp-norm exponent at the right edge.",
    },
    width1: #{
        value: 0.5, min: -1.0, max: 2.0, free: true,
        caption: "Width 1",
        hint: "Arc width at the left edge of each cell.",
    },
    width2: #{
        value: 0.5, min: -1.0, max: 2.0, free: true,
        caption: "Width 2",
        hint: "Arc width at the right edge.",
    },
    scale: #{
        value: 10.0, min: -100.0, max: 100.0, free: true,
        caption: "Scale",
        hint: "Input pre-scale for the per-cell X-position calculation.",
    },
    seed: #{
        value: 50.0, min: -1000.0, max: 1000.0, free: true,
        caption: "Seed",
        hint: "Hash seed for the per-cell tile-type selection.",
    },
    inverse: #{
        value: false,
        caption: "Inverse",
        hint: "When on, output the complement (cells outside the arc test). When off, standard output.",
    },
};

const INFO = #{ title: "Truchet 2", name: "rhai_truchet2", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let exp1 = p.exponent1;
    let exp2 = p.exponent2;
    let w1 = p.width1;
    let w2 = p.width2;
    let scale = p.scale;
    let seed_p = p.seed;
    let inverse = to_int(to_float(p.inverse));
    let safe_scale = (if abs(scale) < 1.0e-30 { 1.0e-30 } else { scale });
    let xs = z.re / safe_scale;
    let xp_in = abs((xs - to_float(floor(xs))) - 0.5) * 2.0;
    let width = w1 * (1.0 - xp_in) + xp_in * w2;
    width = min(width, 1.0);
    if (width <= 0.0) {
        return complex(z.re, z.im);
    }
    let xp2 = exp1 * (1.0 - xp_in) + xp_in * exp2;
    let n = xp2;
    n = min(n, 2.0);
    if (n <= 0.0) {
        return complex(z.re, z.im);
    }
    let onen = 1.0 / xp2;
    let seed_abs = abs(seed_p);
    let seed2 = sqrt(seed_abs + seed_abs * 0.5 + 1.0e-30) / (seed_abs * 0.5 + 1.0e-30) * 0.25;
    let intx = round_even(z.re);
    let inty = round_even(z.im);
    let x = z.re - intx;
    if (x < 0.0) {
        x = 1.0 + x;
    }
    let y = z.im - inty;
    if (y < 0.0) {
        y = 1.0 + y;
    }
    let tiletype = 0.0;
    if (seed_abs == 1.0) {
        tiletype = 1.0;
    } else if (seed_abs > 0.0) {
        let xrand = round_even(z.re) * seed2;
        let yrand = round_even(z.im) * seed2;
        let niter = xrand + yrand + xrand * yrand;
        let randint = (niter + seed_p) * seed2 * 0.5;
        randint = randint - to_float(floor(randint / 65535.0)) * 65535.0;
        randint = randint * 32747.0 + 12345.0;
        randint = randint - to_float(floor(randint / 65535.0)) * 65535.0;
        tiletype = randint - to_float(floor(randint * 0.5)) * 2.0;
    }
    let r0 = 0.0;
    let r1 = 0.0;
    if (tiletype < 1.0) {
        r0 = pow(pow(abs(x), n) + pow(abs(y), n), onen);
        r1 = pow(pow(abs(x - 1.0), n) + pow(abs(y - 1.0), n), onen);
    } else {
        r0 = pow(pow(abs(x - 1.0), n) + pow(abs(y), n), onen);
        r1 = pow(pow(abs(x), n) + pow(abs(y - 1.0), n), onen);
    }
    let rmax = 0.5 * (pow(2.0, onen) - 1.0) * width;
    let safe_rmax = (if abs(rmax) < 1.0e-30 { 1.0e-30 } else { rmax });
    let r00 = abs(r0 - 0.5) / safe_rmax;
    let r11 = abs(r1 - 0.5) / safe_rmax;
    if (inverse == 0) {
        if (r00 < 1.0 || r11 < 1.0) {
            return complex(x + to_float(floor(z.re)), y + to_float(floor(z.im)));
        }
        return complex(100.0, 100.0);
    }
    if (r00 > 1.0 && r11 > 1.0) {
        return complex(x + to_float(floor(z.re)), y + to_float(floor(z.im)));
    }
    complex(10000.0, 10000.0)
}
