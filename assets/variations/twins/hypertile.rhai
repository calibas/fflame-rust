/// Maps the plane onto a {p, q} hyperbolic tiling via Möbius
/// transformation. `n` picks which tile of the tiling is targeted
/// (deterministic).
///
/// # Authors
/// - Zueuk

// Hypertile, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    p: #{
        value: 3, min: 3, max: 50,
        caption: "P",
        hint: "First Schläfli symbol — number of sides per tile (3 = triangle, 4 = square, 5 = pentagon, etc.).",
    },
    q: #{
        value: 7, min: 3, max: 50,
        caption: "Q",
        hint: "Second Schläfli symbol — number of tiles meeting at each vertex. For a valid hyperbolic tiling, `(p − 2)(q − 2) > 4`.",
    },
    n: #{
        value: 1, min: 0, max: 50,
        caption: "N",
        hint: "Index of which tile to target — deterministic tile selector.",
    },
};

const INFO = #{ title: "Hypertile", name: "rhai_hypertile", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn prepare(p) {
    let pi = 3.14159265358979;
    let pv = to_float(p.p);
    let q = to_float(p.q);
    let n = to_float(p.n);
    let pa = 2.0 * pi / pv;
    let qa = 2.0 * pi / q;
    let r_inner = (1.0 - cos(pa)) / (cos(pa) + cos(qa)) + 1.0;
    let r = 0.0;
    if (r_inner > 0.0) {
        r = 1.0 / sqrt(r_inner);
    } else {
        r = 1.0;
    }
    let a = n * pa;
    #{ re: r * cos(a), im: r * sin(a) }
}

fn variation(z, p) {
    let re = p.re;
    let im = p.im;
    let a = z.re + re;
    let b = z.im - im;
    let c = re * z.re - im * z.im + 1.0;
    let d = re * z.im + im * z.re;
    let denom = c * c + d * d;
    let inv = 1.0 / max(1.0e-30, denom);
    complex(inv * (a * c + b * d), inv * (b * c - a * d))
}

fn variation3(v, p) {
    let re = p.re;
    let im = p.im;
    let a = v.x + re;
    let b = v.y - im;
    let c = re * v.x - im * v.y + 1.0;
    let d = re * v.y + im * v.x;
    let denom = c * c + d * d;
    let inv = 1.0 / max(1.0e-30, denom);
    vec3(inv * (a * c + b * d), inv * (b * c - a * d), v.z)
}
