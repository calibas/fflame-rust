/// Maps the plane onto a {p, q} hyperbolic tiling — random tile per
/// iteration. The rotation picking the tile is applied first.
///
/// # Authors
/// - Zueuk

// Hypertile 1, the built-in variation's twin, translated from its WGSL.

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
};

const INFO = #{ title: "Hypertile 1", name: "rhai_hypertile1", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn prepare(p) {
    let pi = 3.14159265358979;
    let pv = to_float(p.p);
    let q = to_float(p.q);
    let pa = 2.0 * pi / pv;
    let qa = 2.0 * pi / q;
    let r2 = 1.0 - (cos(pa) - 1.0) / (cos(pa) + cos(qa));
    let r = 0.0;
    if (r2 > 0.0) {
        r = 1.0 / sqrt(r2);
    } else {
        r = 1.0;
    }
    #{ pa: pa, r: r }
}

fn variation(z, p) {
    let pa = p.pa;
    let r = p.r;
    let ang = to_float(floor(rand() * to_float(p.p))) * pa;
    let cosa = cos(ang);
    let sina = sin(ang);
    let re = r * cosa;
    let im = r * sina;
    let a = z.re + re;
    let b = z.im - im;
    let c = re * z.re - im * z.im + 1.0;
    let d = re * z.im + im * z.re;
    let denom = c * c + d * d;
    let inv = 1.0 / max(1.0e-30, denom);
    complex(inv * (a * c + b * d), inv * (b * c - a * d))
}

fn variation3(v, p) {
    let pa = p.pa;
    let r = p.r;
    let ang = to_float(floor(rand() * to_float(p.p))) * pa;
    let cosa = cos(ang);
    let sina = sin(ang);
    let re = r * cosa;
    let im = r * sina;
    let a = v.x + re;
    let b = v.y - im;
    let c = re * v.x - im * v.y + 1.0;
    let d = re * v.y + im * v.x;
    let denom = c * c + d * d;
    let inv = 1.0 / max(1.0e-30, denom);
    vec3(inv * (a * c + b * d), inv * (b * c - a * d), v.z)
}
