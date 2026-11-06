// Loonie, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Loonie", name: "rhai_loonie" };

fn variation(z, p) {
    let r2 = z.norm_sqr();
    if (r2 < 1.0 && r2 != 0.0) {
        let r = sqrt(1.0 / r2 - 1.0);
        complex(z.re * r, z.im * r)
    } else {
        z
    }
}
