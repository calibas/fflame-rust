/// 3D version of Hypertile2 — tile centered on the real axis, with per-
/// iteration random XY rotation applied after the Möbius warp.
///
/// # Authors
/// - Zueuk

// Hypertile 3D 2, the built-in variation's twin, translated from its WGSL.

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

const INFO = #{ title: "Hypertile 3D 2", name: "rhai_hypertile3d2", category: "full_3d", phase: "any" };

fn prepare(p) {
    let pi = 3.14159265358979;
    let pv = to_float(p.p);
    let q = to_float(p.q);
    let pa = 2.0 * pi / pv;
    let qa = 2.0 * pi / q;
    let r_alt = -(cos(pa) - 1.0) / (cos(pa) + cos(qa));
    let r = 0.0;
    if (r_alt > 0.0) {
        r = 1.0 / sqrt(1.0 + r_alt);
    } else {
        r = 1.0;
    }
    let cx = r;
    let cx2 = cx * cx;
    #{ pa: pa, cx: cx, c2: cx2, c2x: 2.0 * cx, s2x: 1.0 + cx2, s2y: 1.0 - cx2, s2z: 1.0 - cx2 }
}

fn variation(z, p) {
    let pa = p.pa;
    let cx = p.cx;
    let c2 = p.c2;
    let c2x = p.c2x;
    let s2x = p.s2x;
    let s2y = p.s2y;
    let r2 = z.re * z.re + z.im * z.im;
    let x2cx = c2x * z.re;
    let x = z.re * s2x - cx * (-r2 - 1.0);
    let y = z.im * s2y;
    let denom = c2 * r2 + x2cx + 1.0;
    let inv = 1.0 / max(1.0e-30, denom);
    let ang = to_float(floor(rand() * to_float(p.p))) * pa;
    let cosa = cos(ang);
    let sina = sin(ang);
    complex(inv * (x * cosa + y * sina), inv * (y * cosa - x * sina))
}

fn variation3(v, p) {
    let pa = p.pa;
    let cx = p.cx;
    let c2 = p.c2;
    let c2x = p.c2x;
    let s2x = p.s2x;
    let s2y = p.s2y;
    let s2z = p.s2z;
    let r2 = v.x * v.x + v.y * v.y + v.z * v.z;
    let x2cx = c2x * v.x;
    let x = v.x * s2x - cx * (-r2 - 1.0);
    let y = v.y * s2y;
    let denom = c2 * r2 + x2cx + 1.0;
    let inv = 1.0 / max(1.0e-30, denom);
    let ang = to_float(floor(rand() * to_float(p.p))) * pa;
    let cosa = cos(ang);
    let sina = sin(ang);
    vec3(inv * (x * cosa + y * sina), inv * (y * cosa - x * sina), inv * (v.z * s2z))
}
