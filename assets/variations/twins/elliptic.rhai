// Elliptic, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Elliptic", name: "rhai_elliptic" };

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
