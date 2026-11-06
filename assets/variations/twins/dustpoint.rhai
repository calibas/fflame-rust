// Dust Point, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Dust Point", name: "rhai_dustpoint" };

fn variation(z, p) {
    let p_sign = (if rand() < 0.5 { 1.0 } else { -1.0 });
    let r = sqrt(max(z.re * z.re + z.im * z.im, 1.0e-30));
    let w = rand();
    if (w < 0.5) {
        return complex(z.re / r - 1.0, p_sign * z.im / r);
    } else if (w < 0.75) {
        return complex(z.re / 3.0, z.im / 3.0);
    }
    complex(z.re / 3.0 + 2.0 / 3.0, z.im / 3.0)
}
