/// N-sided scry — generalizes the `scry` variation to N-sided star/circle
/// hybrids. Computes a max-projection across `sides` rotations, mixes with
/// a circular term via `circle`, optionally folds with a star pattern via
/// `star`. The output is `(x, y) / (r1 · (r2 + 1/w))` — `1/w` appears
/// internally, so the divide-out pattern handles it.
///
/// # Authors
/// - DarkBeam

// Scry 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    sides: #{
        value: 4, min: 1, max: 50,
        caption: "Sides",
        hint: "Polygon side count.",
    },
    star: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Star",
        hint: "Star-fold rotation amount (scaled by `-π/2` internally).",
    },
    circle: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Circle",
        hint: "Circularity mixing factor: 0 = pure star/polygon, 1 = pure circle.",
    },
};

const INFO = #{ title: "Scry 2", name: "rhai_scry2", category: "advanced_2d", phase: "any" };

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
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
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
    let r1 = r2;
    if (i > 1) {
        r2 = r2 * r2;
    } else {
        r2 = abs(r2) * r2;
    }
    let d = r1 * (r2 + inv_w);
    let safe_d = (if abs(d) < 1.0e-30 { 1.0e-30 } else { d });
    let r = 1.0 / safe_d;
    complex(z.re * r * inv_w, z.im * r * inv_w)
}
