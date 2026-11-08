/// Conformal map onto an elliptic-coordinate grid. Useful for mathematical-
/// looking, symmetric patterns.
///
/// # Authors
/// - Apophysis Plugin Pack

// Elliptic, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Elliptic", name: "rhai_elliptic", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let PI = 3.14159265359;
    let v = 2.0 / PI();
    let tmp = z.norm_sqr() + 1.0;
    let x2 = 2.0 * z.re;
    let xmax = 0.5 * (sqrt(tmp + x2) + sqrt(tmp - x2));
    let a = z.re / xmax;
    let b = sqrt(max(0.0, 1.0 - a * a));
    let new_x = v * atan2(a, b);
    let new_y = v * ln(xmax + sqrt(max(0.0, xmax - 1.0)));
    if (z.im < 0.0) {
        new_y = -new_y;
    }
    complex(new_x, new_y)
}

fn inverse(z, p, branch) {
    if abs(z.re) > 1.0 {
        return no_preimage();
    }
    let h = 1.5707963267949;
    let e = exp(abs(z.im) * h) - 1.0;
    let s = 2.0 * e / (sqrt(1.0 + 4.0 * e) + 1.0);
    let m = s * s;
    let y = s * sqrt(m + 2.0) * cos(z.re * h);
    complex(sin(z.re * h) * (1.0 + m), if z.im < 0.0 { -y } else { y })
}

fn image_gap(z, p) {
    abs(z.re) - 1.0
}
