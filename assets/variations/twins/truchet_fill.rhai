/// Truchet-style tile fill — divides the plane into unit cells and draws
/// arc patterns through each cell with hash-based per-cell orientation.
/// Produces interlocking curved-tile patterns.
///
/// # Authors
/// - Tatyana Zabanova

// Truchet Fill, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    exponent: #{
        value: 2.0, min: 0.001, max: 2.0, free: true,
        caption: "Exponent",
        hint: "Lp-norm exponent for the cell distance metric — controls how rounded or cornered the arcs are. Clamped to [0.001, 2.0].",
    },
    arc_width: #{
        value: 0.5, min: 0.001, max: 1.0, free: true,
        caption: "Arc Width",
        hint: "Arc thickness within each cell. Clamped to [0.001, 1.0].",
    },
    seed: #{
        value: 0.0, min: 0.0, max: 100.0, free: true,
        caption: "Seed",
        hint: "Hash seed for per-cell tile orientation. 0 = all same orientation; 1 = all flipped; other values produce a varied pattern.",
    },
};

const INFO = #{ title: "Truchet Fill", name: "rhai_truchet_fill", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let exp_in = p.exponent;
    let arc_in = p.arc_width;
    let seed = p.seed;
    let exp_clamped = clamp(exp_in, 0.001, 2.0);
    let safe_seed = (if abs(seed) < 1.0e-30 { 1.0e-30 } else { seed });
    #{ d0: exp_clamped, onen: 1.0 / exp_clamped, width: clamp(arc_in, 0.001, 1.0), seed2: sqrt(max(0.0, seed * 1.5)) / (safe_seed * 0.5) * 0.25 }
}

fn variation(z, p, ctx) {
    let seed = p.seed;
    let exponent = p.d0;
    let onen = p.onen;
    let width = p.width;
    let seed2 = p.seed2;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let rmax = 0.5 * (pow(2.0, onen) - 1.0) * width;
    let safe_rmax = (if abs(rmax) < 1.0e-30 { 1.0e-30 } else { rmax });
    let scale = inv_w;
    let modbase = 65535.0;
    let multiplier = 32747.0;
    let offset = 12345.0;
    let x = z.re * scale;
    let y = z.im * scale;
    let intx = round_even(x);
    let inty = round_even(y);
    let r = x - intx;
    x = (if r < 0.0 { 1.0 + r } else { r });
    r = y - inty;
    y = (if r < 0.0 { 1.0 + r } else { r });
    let tiletype = 0.0;
    if (seed == 0.0) {
        tiletype = 0.0;
    } else if (seed == 1.0) {
        tiletype = 1.0;
    } else {
        let xrand = round_even(abs(z.re)) * seed2;
        let yrand = round_even(abs(z.im)) * seed2;
        let niter = xrand + yrand + xrand * yrand;
        let randint = (seed + niter) * seed2 * 0.5;
        randint = randint * multiplier + offset;
        randint = randint - to_float(floor(randint / modbase)) * modbase;
        tiletype = randint - to_float(floor(randint / 2.0)) * 2.0;
    }
    let r0 = 0.0;
    let r1 = 0.0;
    if (tiletype < 1.0) {
        r0 = pow(pow(abs(x), exponent) + pow(abs(y), exponent), onen);
        r1 = pow(pow(abs(x - 1.0), exponent) + pow(abs(y - 1.0), exponent), onen);
    } else {
        r0 = pow(pow(abs(x - 1.0), exponent) + pow(abs(y), exponent), onen);
        r1 = pow(pow(abs(x), exponent) + pow(abs(y - 1.0), exponent), onen);
    }
    let x1 = 0.0;
    let y1 = 0.0;
    let r00 = abs(r0 - 0.5) / safe_rmax;
    if (r00 < 1.0) {
        x1 = 2.0 * (x + to_float(floor(z.re)));
        y1 = 2.0 * (y + to_float(floor(z.im)));
    }
    let fpx_inc = 0.0;
    let fpy_inc = 0.0;
    let r11 = abs(r1 - 0.5) / safe_rmax;
    if (r11 < 1.0) {
        fpx_inc = x1 + 2.0 * (x + to_float(floor(z.re))) - z.re;
        fpy_inc = y1 + 2.0 * (y + to_float(floor(z.im))) - z.im;
    } else {
        fpx_inc = x1 - z.re;
        fpy_inc = y1 - z.im;
    }
    complex(fpx_inc * inv_w, fpy_inc * inv_w)
}
