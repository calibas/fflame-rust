/// Heart-shaped Möbius warp — applies a Möbius-like inversion `(bx·x,
/// by·y)` where `bx = 4/(r²+4)` and `by = (6+2·ratio)/(r²+4)`, then rotates
/// the result by `π/4 + π/8·angle`. The Y axis is flipped where the rotated
/// X is negative, producing a heart-shaped silhouette.
///
/// # Authors
/// - Xyrus02

// X Heart, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    angle: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Angle",
        hint: "Rotation angle of the heart shape (scaled by π/8 internally and offset from π/4).",
    },
    ratio: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Ratio",
        hint: "Y-axis stretch factor (added to a base of 6 to control heart roundness).",
    },
};

const INFO = #{ title: "X Heart", name: "rhai_xheart", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let pi_4 = 0.7853981633974483;
    let pi_8 = 0.39269908169872414;
    let ang = pi_4 + pi_8 * p.angle;
    #{ rat: 6.0 + 2.0 * p.ratio, cosa: cos(ang), sina: sin(ang) }
}

fn variation(z, p) {
    let rat = p.rat;
    let cosa = p.cosa;
    let sina = p.sina;
    let r2_4 = z.re * z.re + z.im * z.im + 4.0;
    if (r2_4 == 0.0) {
        r2_4 = 1.0;
    }
    let bx = 4.0 / r2_4;
    let by = rat / r2_4;
    let x = cosa * (bx * z.re) - sina * (by * z.im);
    let y = sina * (bx * z.re) + cosa * (by * z.im);
    if (x > 0.0) {
        return complex(x, y);
    }
    complex(x, -y)
}
