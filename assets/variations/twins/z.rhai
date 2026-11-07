/// Pure radial boost using Michael Faber's *Lost Variations* four-shape
/// blender. Computes a per-angle `total` by summing contributions from up
/// to four named shapes — `hypergon` (regular polygon with adjustable inner
/// radius), `star` (star polygon with slope-defined arms), `lituus` (polar
/// spiral `(a/π + 1)^(-lituus_a)`), and `super` (Gielis super-shape) — each
/// gated by its weight parameter (zero = disabled) and shaped by its own
/// family of sub-parameters. Output is then `(r_in + total) · (cos a, sin
/// a)`: the input point shoved outward along its own angle.
///
/// # Authors
/// - Michael Faber

// Z (Faber), the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    hypergon: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Hypergon",
        hint: "Weight of the hypergon (regular-polygon) shape's contribution to the per-angle radius blender. 0 disables this shape.",
    },
    hypergon_n: #{
        value: 4, min: 1, max: 50,
        caption: "Hypergon N",
        hint: "Hypergon symmetry count (number of sides). Used as the modular reduction divisor for the per-sector angle lookup.",
    },
    hypergon_r: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Hypergon R",
        hint: "Hypergon inner-radius parameter. Combined with the per-sector angle to decide whether the shape edge is convex or hollow at that angle.",
    },
    star: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Star",
        hint: "Weight of the star-polygon shape's contribution. 0 disables this shape.",
    },
    star_n: #{
        value: 5, min: 1, max: 50,
        caption: "Star N",
        hint: "Star symmetry count (number of points).",
    },
    star_slope: #{
        value: 2.0, min: -10.0, max: 10.0, free: true,
        caption: "Star Slope",
        hint: "Star arm slope angle (radians). Internally pre-computed as `tan(star_slope)` and used as the slope of each star arm.",
    },
    lituus: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Lituus",
        hint: "Weight of the lituus (polar spiral) shape's contribution. 0 disables this shape.",
    },
    lituus_a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Lituus A",
        hint: "Lituus exponent. Internally negated; the contribution is `(a/π + 1)^(-lituus_a)` — a classical polar spiral.",
    },
    super_shape: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Super",
        hint: "Weight of the Gielis super-shape's contribution. 0 disables this shape.",
    },
    super_m: #{
        value: 1.0, min: -50.0, max: 50.0, free: true,
        caption: "Super M",
        hint: "Super-shape symmetry count (petal count). Internally divided by 4 to feed the Gielis sin/cos formula.",
    },
    super_n1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Super N1",
        hint: "Super-shape outer exponent. Internally `-1/(super_n1 + ε)` is used as the radial blow-up power — small values produce sharper points.",
    },
    super_n2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Super N2",
        hint: "Super-shape cosine-term exponent (controls one half of the petal asymmetry).",
    },
    super_n3: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Super N3",
        hint: "Super-shape sine-term exponent.",
    },
};

const INFO = #{ title: "Z (Faber)", name: "rhai_z", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let hypergon_r = p.hypergon_r;
    let hypergon_n = to_float(p.hypergon_n);
    let star_n = to_float(p.star_n);
    let star_slope = p.star_slope;
    let lituus_a = p.lituus_a;
    let super_m = p.super_m;
    let super_n1 = p.super_n1;
    #{ hypergon_d: sqrt(1.0 + hypergon_r * hypergon_r), d1: hypergon_n, lituus_a_neg: -lituus_a, d3: star_n, star_slope_t: tan(star_slope), super_m4: super_m * 0.25, super_n1_neg: -1.0 / (super_n1 + 1.0e-7) }
}

fn z_blend_total(a, hypergon, star, lituus, super_w, super_n2, super_n3, hypergon_d, hypergon_n, lituus_a_neg, star_n, star_slope_t, super_m4, super_n1_neg) {
    let pi = 3.14159265358979;
    let two_pi = 6.28318530717959;
    let total = 0.0;
    if (hypergon != 0.0 && hypergon_n > 0.0) {
        let safe_n = max(1.0, hypergon_n);
        let temp1 = (abs(a) - to_float(floor(abs(a) / (two_pi / safe_n))) * (two_pi / safe_n)) - pi / safe_n;
        let t = tan(temp1);
        let temp2 = t * t + 1.0;
        let h_d_sq = hypergon_d * hypergon_d;
        if (temp2 >= h_d_sq) {
            total = total + hypergon;
        } else {
            let safe_t2 = max(1.0e-30, temp2);
            total = total + hypergon * (hypergon_d - sqrt(max(0.0, h_d_sq - temp2))) / sqrt(safe_t2);
        }
    }
    if (star != 0.0 && star_n > 0.0) {
        let safe_n = max(1.0, star_n);
        let inner = (abs(a) - to_float(floor(abs(a) / (two_pi / safe_n))) * (two_pi / safe_n)) - pi / safe_n;
        let temp1 = tan(abs(inner));
        let denom = (temp1 + star_slope_t) * (temp1 + star_slope_t);
        let safe_d = max(1.0e-30, abs(denom));
        total = total + star * sqrt(star_slope_t * star_slope_t * (1.0 + temp1 * temp1) / safe_d);
    }
    if (lituus != 0.0) {
        let base = a / pi + 1.0;
        total = total + lituus * pow(max(1.0e-30, abs(base)), lituus_a_neg);
    }
    if (super_w != 0.0) {
        let ang = a * super_m4;
        let s = sin(ang);
        let c = cos(ang);
        let inner = pow(max(1.0e-30, abs(c)), super_n2) + pow(max(1.0e-30, abs(s)), super_n3);
        total = total + super_w * pow(max(1.0e-30, inner), super_n1_neg);
    }
    total
}

fn variation(z, p) {
    let hypergon = p.hypergon;
    let star = p.star;
    let lituus = p.lituus;
    let super_w = p.super_shape;
    let super_n2 = p.super_n2;
    let super_n3 = p.super_n3;
    let hypergon_d = p.hypergon_d;
    let hypergon_n = p.d1;
    let lituus_a_neg = p.lituus_a_neg;
    let star_n = p.d3;
    let star_slope_t = p.star_slope_t;
    let super_m4 = p.super_m4;
    let super_n1_neg = p.super_n1_neg;
    let a = atan2(z.im, z.re);
    let r_in = sqrt(z.re * z.re + z.im * z.im);
    let total = z_blend_total(a, hypergon, star, lituus, super_w, super_n2, super_n3, hypergon_d, hypergon_n, lituus_a_neg, star_n, star_slope_t, super_m4, super_n1_neg);
    let r = r_in + total;
    complex(r * cos(a), r * sin(a))
}
