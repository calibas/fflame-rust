/// 3D version of Hypertile — Möbius reflection through a sphere on the
/// unit-disc boundary. `n` picks which tile (deterministic).
///
/// # Authors
/// - Zueuk

// Hypertile 3D, the built-in variation's twin, translated from its WGSL.

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
        value: 0, min: 0, max: 50,
        caption: "N",
        hint: "Index of which tile to target — deterministic tile selector.",
    },
};

const INFO = #{ title: "Hypertile 3D", name: "rhai_hypertile3d", category: "full_3d", phase: "any" };

fn prepare(p) {
    let pi = 3.14159265358979;
    let pv = to_float(p.p);
    let q = to_float(p.q);
    let n = to_float(p.n);
    let pa = 2.0 * pi / pv;
    let qa = 2.0 * pi / q;
    let r_alt = -(cos(pa) - 1.0) / (cos(pa) + cos(qa));
    let r = 0.0;
    if (r_alt > 0.0) {
        r = 1.0 / sqrt(1.0 + r_alt);
    } else {
        r = 1.0;
    }
    let na = n * pa;
    let cx = r * cos(na);
    let cy = r * sin(na);
    let cx2 = cx * cx;
    let cy2 = cy * cy;
    #{ cx: cx, cy: cy, c2: cx2 + cy2, c2x: 2.0 * cx, c2y: 2.0 * cy, s2x: 1.0 + cx2 - cy2, s2y: 1.0 + cy2 - cx2, s2z: 1.0 - cx2 - cy2 }
}

fn variation(z, p) {
    let cx = p.cx;
    let cy = p.cy;
    let c2 = p.c2;
    let c2x = p.c2x;
    let c2y = p.c2y;
    let s2x = p.s2x;
    let s2y = p.s2y;
    let r2 = z.re * z.re + z.im * z.im;
    let x2cx = c2x * z.re;
    let y2cy = c2y * z.im;
    let denom = c2 * r2 + x2cx - y2cy + 1.0;
    let d = 1.0 / max(1.0e-30, denom);
    complex(d * (z.re * s2x - cx * (y2cy - r2 - 1.0)), d * (z.im * s2y + cy * (-x2cx - r2 - 1.0)))
}

fn variation3(v, p) {
    let cx = p.cx;
    let cy = p.cy;
    let c2 = p.c2;
    let c2x = p.c2x;
    let c2y = p.c2y;
    let s2x = p.s2x;
    let s2y = p.s2y;
    let s2z = p.s2z;
    let r2 = v.x * v.x + v.y * v.y + v.z * v.z;
    let x2cx = c2x * v.x;
    let y2cy = c2y * v.y;
    let denom = c2 * r2 + x2cx - y2cy + 1.0;
    let d = 1.0 / max(1.0e-30, denom);
    vec3(d * (v.x * s2x - cx * (y2cy - r2 - 1.0)), d * (v.y * s2y + cy * (-x2cx - r2 - 1.0)), d * (v.z * s2z))
}
