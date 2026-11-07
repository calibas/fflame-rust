/// Hyperbolic {p, q} tiling — generates a hyperbolic tiling whose faces are
/// regular `p`-gons meeting `q`-fold at each vertex (e.g., {3,7} = the
/// order-7 triangular tiling). Reflects the input through a hyperbolic
/// shift transform tuned by the `(p, q)` Schläfli pair, then rotates to one
/// of `p` random angular sectors per iteration.
///
/// # Authors
/// - Tatyana Zabanova
/// - Brad Stefanov

// Hyper Shift 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    p: #{
        value: 3, min: 2, max: 50,
        caption: "P",
        hint: "Polygon side count — the {p, q} tiling has regular `p`-gon faces.",
    },
    q: #{
        value: 7, min: 2, max: 50,
        caption: "Q",
        hint: "Vertices per polygon — `q` of them meet at each vertex of the tiling.",
    },
};

const INFO = #{ title: "Hyper Shift 2", name: "rhai_hypershift2", category: "full_3d", phase: "any", replace: true };

fn prepare(p) {
    let pi = 3.14159265358979;
    let pi_2 = 1.5707963267948966;
    let pv = max(2.0, to_float(p.p));
    let q = max(2.0, to_float(p.q));
    let pq = pi / q;
    let pp = pi / pv;
    let spq = sin(pq);
    let spp = sin(pp);
    let shift_num = sin(pi_2 - pq - pp);
    let shift_den = sqrt(max(1.0e-30, 1.0 - spq * spq - spp * spp));
    let shift = shift_num / shift_den;
    let s_p2pp = sin(pi_2 + pp);
    let r_inner = s_p2pp * s_p2pp / (spq * spq) - 1.0;
    let scale2_a = 1.0 / sqrt(max(1.0e-30, r_inner));
    let scale2 = scale2_a * (s_p2pp / spq - 1.0);
    let scale = 1.0 - shift * shift;
    #{ shift: shift, scale2: scale2, scale: scale, d3: 1.0 / pv }
}

fn variation(z, p) {
    let p_u = max(2.0, to_float(p.p));
    let shift = p.shift;
    let scale2 = p.scale2;
    let scale = p.scale;
    let pi = 3.14159265358979;
    let FX = z.re * scale2;
    let FY = z.im * scale2;
    let denom1 = max(1.0e-30, FX * FX + FY * FY);
    let rad = 1.0 / denom1;
    let xp = rad * FX + shift;
    let yp = rad * FY;
    let denom2 = max(1.0e-30, xp * xp + yp * yp);
    rad = scale / denom2;
    let angle = (to_float(floor(rand() * p_u)) * 2.0 + 1.0) * pi / p_u;
    let X = rad * xp + shift;
    let Y = rad * yp;
    let cosa = cos(angle);
    let sina = sin(angle);
    complex(cosa * X - sina * Y, sina * X + cosa * Y)
}

fn variation3(v, p) {
    let p_u = max(2.0, to_float(p.p));
    let shift = p.shift;
    let scale2 = p.scale2;
    let scale = p.scale;
    let pi = 3.14159265358979;
    let FX = v.x * scale2;
    let FY = v.y * scale2;
    let denom1 = max(1.0e-30, FX * FX + FY * FY);
    let rad = 1.0 / denom1;
    let xp = rad * FX + shift;
    let yp = rad * FY;
    let denom2 = max(1.0e-30, xp * xp + yp * yp);
    rad = scale / denom2;
    let angle = (to_float(floor(rand() * p_u)) * 2.0 + 1.0) * pi / p_u;
    let X = rad * xp + shift;
    let Y = rad * yp;
    let cosa = cos(angle);
    let sina = sin(angle);
    vec3(cosa * X - sina * Y, sina * X + cosa * Y, v.z * rad)
}
