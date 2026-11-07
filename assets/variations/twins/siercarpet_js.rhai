/// Cross Carpet IFS (Roger Bagula) — picks a random index `l ∈ [1, 2m]` per
/// iteration and applies one of two affine maps based on `l` parity: odd
/// `l` uses a single vertex at angle `2π·i/m`; even `l` uses the midpoint
/// between consecutive vertices. The mapping is `(x/3 + (a − b)/√2, y/3 +
/// (a + b)/√2)`, producing a Sierpinski-style cross carpet with `m` arms.
///
/// # Authors
/// - Jesus Sosa

// Siercarpet (JS), the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    m: #{
        value: 3, min: 3, max: 12,
        caption: "M",
        hint: "Number of arms in the cross (3-12). Each iteration picks one of `2m` vertex-or-midpoint positions around the circle.",
    },
};

const INFO = #{ title: "Siercarpet (JS)", name: "rhai_siercarpet_js", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let m = clamp(to_int(to_float(p.m)), 3, 12);
    let m_f = to_float(m);
    let two_pi_over_m = 6.28318530717959 / m_f;
    let inv_sqrt2 = 0.7071067811865476;
    let l_raw = 1 + to_int(2.0 * m_f * rand());
    let l = clamp(l_raw, 1, 2 * m);
    let a_l = 0.0;
    let b_l = 0.0;
    if ((l & 1) == 1) {
        let i_idx = to_float((l + 1) / 2);
        a_l = cos(two_pi_over_m * i_idx);
        b_l = sin(two_pi_over_m * i_idx);
    } else {
        let i_idx = to_float(l / 2);
        let i_prev = i_idx - 1.0;
        a_l = (cos(two_pi_over_m * i_idx) + cos(two_pi_over_m * i_prev)) * 0.5;
        b_l = (sin(two_pi_over_m * i_idx) + sin(two_pi_over_m * i_prev)) * 0.5;
    }
    let x = z.re / 3.0 + (a_l - b_l) * inv_sqrt2;
    let y = z.im / 3.0 + (a_l + b_l) * inv_sqrt2;
    complex(x, y)
}
