/// Truchet tile pattern — divides the input plane into a unit-square grid;
/// each cell is one of two truchet types selected by a deterministic per-
/// cell hash of `(intx, inty)`. Within each cell, two arc segments connect
/// opposite corners; an emitted point falls on one of those arcs (within
/// `arc_width` of the unit distance). Arc shape controlled by the Lp-norm
/// `exponent`.
///
/// # Authors
/// - TyrantWave

// Truchet, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    extended: #{
        value: false,
        caption: "Extended",
        hint: "When on, use the slow iterated-LCG hash for tile-type selection (more cells produce distinct patterns). When off, the fast single-step LCG.",
    },
    exponent: #{
        value: 2.0, min: 0.001, max: 2.0, free: true,
        caption: "Exponent",
        hint: "Lp-norm exponent for the arc shape (clamped to [0.001, 2]). 2 = circular arcs; lower values produce more angular shapes.",
    },
    arc_width: #{
        value: 0.5, min: 0.001, max: 1.0, free: true,
        caption: "Arc Width",
        hint: "Width of the arc segments (clamped to [0.001, 1]).",
    },
    rotation: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Rotation",
        hint: "Pre-rotation angle applied to the input, in radians.",
    },
    size: #{
        value: 1.0, min: 0.001, max: 10.0, free: true,
        caption: "Size",
        hint: "Output scale factor for each cell (clamped to [0.001, 10]).",
    },
    seed: #{
        value: 50.0, min: -1000.0, max: 1000.0, free: true,
        caption: "Seed",
        hint: "Hash seed for the per-cell tile-type selection.",
    },
    direct_color: #{
        value: false,
        caption: "Direct Color",
        hint: "When on, writes `clamp(r0, 0, 1)` to the color register on the r0 arc and `clamp(1 − r1, 0, 1)` on the r1 arc. Visible color requires the transform's Direct Color slider > 0.",
    },
};

const INFO = #{ title: "Truchet", name: "rhai_truchet", category: "advanced_2d", phase: "any" };

fn variation(z, p, ctx) {
    let color = ctx.color;
    let extended = to_int(to_float(p.extended));
    let n = clamp(p.exponent, 0.001, 2.0);
    let width = clamp(p.arc_width, 0.001, 1.0);
    let tdeg = p.rotation;
    let size = clamp(p.size, 0.001, 10.0);
    let seed_p = p.seed;
    let direct_color = to_int(to_float(p.direct_color));
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let onen = 1.0 / n;
    let seed_abs = abs(seed_p);
    let seed2 = sqrt(seed_abs + seed_abs * 0.5 + 1.0e-30) / (seed_abs * 0.5 + 1.0e-30) * 0.25;
    let r0_init = -tdeg;
    let rmax = 0.5 * (pow(2.0, onen) - 1.0) * width;
    let safe_rmax = (if abs(rmax) < 1.0e-30 { 1.0e-30 } else { rmax });
    let scale = (cos(r0_init) - sin(r0_init)) * inv_w;
    let xs = z.re * scale;
    let ys = z.im * scale;
    let intx = round_even(xs);
    let inty = round_even(ys);
    let rx = xs - intx;
    let ry = ys - inty;
    let x = (if rx < 0.0 { 1.0 + rx } else { rx });
    let y = (if ry < 0.0 { 1.0 + ry } else { ry });
    let tiletype = 0.0;
    if (seed_abs == 1.0) {
        tiletype = 1.0;
    } else if (seed_abs > 0.0) {
        if (extended == 0) {
            let xrand = round_even(z.re) * seed2;
            let yrand = round_even(z.im) * seed2;
            let niter = xrand + yrand + xrand * yrand;
            let randint = (niter + seed_p) * seed2 * 0.5;
            randint = randint * 32747.0 + 12345.0;
            randint = randint - to_float(floor(randint / 65535.0)) * 65535.0;
            tiletype = randint - to_float(floor(randint * 0.5)) * 2.0;
        } else {
            let seed_floor = to_float(floor(seed_abs));
            let xrand = round_even(z.re);
            let yrand = round_even(z.im);
            let niter = abs(xrand + yrand + xrand * yrand);
            niter = min(1000.0, niter);
            let niter_i = to_int(niter);
            let randint = seed_floor + niter;
            for i in 0..1000 {
                if (i >= niter_i) {
                    break;
                }
                randint = randint * 32747.0 + 12345.0;
                randint = randint - to_float(floor(randint / 65535.0)) * 65535.0;
            }
            tiletype = randint - to_float(floor(randint * 0.5)) * 2.0;
        }
    }
    let r0 = 0.0;
    let r1 = 0.0;
    if (extended == 0) {
        if (tiletype < 1.0) {
            r0 = pow(pow(abs(x), n) + pow(abs(y), n), onen);
            r1 = pow(pow(abs(x - 1.0), n) + pow(abs(y - 1.0), n), onen);
        } else {
            r0 = pow(pow(abs(x - 1.0), n) + pow(abs(y), n), onen);
            r1 = pow(pow(abs(x), n) + pow(abs(y - 1.0), n), onen);
        }
    } else if (tiletype == 1.0) {
        r0 = pow(pow(abs(x), n) + pow(abs(y), n), onen);
        r1 = pow(pow(abs(x - 1.0), n) + pow(abs(y - 1.0), n), onen);
    } else {
        r0 = pow(pow(abs(x - 1.0), n) + pow(abs(y), n), onen);
        r1 = pow(pow(abs(x), n) + pow(abs(y - 1.0), n), onen);
    }
    let ox = 0.0;
    let oy = 0.0;
    let cell_x = x + to_float(floor(z.re));
    let cell_y = y + to_float(floor(z.im));
    if (abs(r0 - 0.5) / safe_rmax < 1.0) {
        ox = ox + size * cell_x;
        oy = oy + size * cell_y;
        if (direct_color != 0) {
            color = clamp(r0, 0.0, 1.0);
        }
    }
    if (abs(r1 - 0.5) / safe_rmax < 1.0) {
        ox = ox + size * cell_x;
        oy = oy + size * cell_y;
        if (direct_color != 0) {
            color = clamp(1.0 - r1, 0.0, 1.0);
        }
    }
    #{ z: complex(ox * inv_w, oy * inv_w), color: color }
}
