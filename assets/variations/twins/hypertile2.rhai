/// Variant of Hypertile1 that applies the rotation last instead of first.
/// Equivalent math up to ordering, but the per-iteration distribution looks
/// visually distinct.
///
/// # Authors
/// - Zueuk

// Hypertile 2, the built-in variation's twin, translated from its WGSL.

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

const INFO = #{ title: "Hypertile 2", name: "rhai_hypertile2", category: "advanced_2d", phase: "any", z: "preserve_z" };

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
    let a = z.re + r;
    let b = z.im;
    let c = r * z.re + 1.0;
    let d = r * z.im;
    let mx = a * c + b * d;
    let my = b * c - a * d;
    let denom = c * c + d * d;
    let inv = 1.0 / max(1.0e-30, denom);
    let ang = to_float(floor(rand() * to_float(p.p))) * pa;
    let cosa = cos(ang);
    let sina = sin(ang);
    complex(inv * (mx * cosa + my * sina), inv * (my * cosa - mx * sina))
}

fn variation3(v, p) {
    let pa = p.pa;
    let r = p.r;
    let a = v.x + r;
    let b = v.y;
    let c = r * v.x + 1.0;
    let d = r * v.y;
    let mx = a * c + b * d;
    let my = b * c - a * d;
    let denom = c * c + d * d;
    let inv = 1.0 / max(1.0e-30, denom);
    let ang = to_float(floor(rand() * to_float(p.p))) * pa;
    let cosa = cos(ang);
    let sina = sin(ang);
    vec3(inv * (mx * cosa + my * sina), inv * (my * cosa - mx * sina), v.z)
}
