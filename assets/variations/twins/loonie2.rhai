/// N-sided loonie — generalizes the standard `loonie` warp to N-sided
/// star/circle hybrids. Computes a maximum projection across `sides`
/// rotations, then mixes with a circular term via `circle` and optionally
/// folds with a star pattern via `star`. Inside the squared-weight
/// threshold the input scales outward; outside, it passes through.
///
/// # Authors
/// - DarkBeam

// Loonie 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    sides: #{
        value: 4, min: 1, max: 50,
        caption: "Sides",
        hint: "Polygon side count (≥ 1).",
    },
    star: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Star",
        hint: "Star-fold rotation amount (scaled by −π/2 internally).",
    },
    circle: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Circle",
        hint: "Circularity mixing factor: 0 = pure star/polygon shape, 1 = pure circle.",
    },
};

const INFO = #{ title: "Loonie 2", name: "rhai_loonie2", category: "advanced_2d", phase: "any" };

fn prepare(p) {
    let sides = max(1.0, to_float(p.sides));
    let star = p.star;
    let circle_p = p.circle;
    let two_pi = 6.28318530717959;
    let half_pi = 1.5707963267948966;
    let a = two_pi / sides;
    let as_ = -half_pi * star;
    let ac = half_pi * circle_p;
    #{ sina: sin(a), cosa: cos(a), sins: sin(as_), coss: cos(as_), sinc: sin(ac), cosc: cos(ac) }
}

fn variation(z, p, ctx) {
    let sides_in = max(1.0, to_float(p.sides));
    let sina = p.sina;
    let cosa = p.cosa;
    let sins = p.sins;
    let coss = p.coss;
    let sinc = p.sinc;
    let cosc = p.cosc;
    let w = ctx.weight;
    let sqrvvar = w * w;
    let xrt = z.re;
    let yrt = z.im;
    let r2 = xrt * coss + abs(yrt) * sins;
    let circle_v = sqrt(z.re * z.re + z.im * z.im);
    let sides_i = to_int(sides_in);
    let i = 0;
    for k in 0..sides_i - 1 {
        let swp = xrt * cosa - yrt * sina;
        yrt = xrt * sina + yrt * cosa;
        xrt = swp;
        r2 = max(r2, xrt * coss + abs(yrt) * sins);
        i = k + 1;
    }
    r2 = r2 * cosc + circle_v * sinc;
    if (i > 1) {
        r2 = r2 * r2;
    } else {
        r2 = abs(r2) * r2;
    }
    if (r2 > 0.0 && r2 < sqrvvar) {
        let scale = sqrt(max(0.0, sqrvvar / max(1.0e-30, r2) - 1.0));
        return complex(scale * z.re, scale * z.im);
    } else if (r2 < 0.0) {
        let inv_scale = 1.0 / max(1.0e-30, sqrt(max(1.0e-30, sqrvvar / max(1.0e-30, -r2) - 1.0)));
        let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
        return complex(inv_scale * z.re * inv_w, inv_scale * z.im * inv_w);
    }
    z
}
