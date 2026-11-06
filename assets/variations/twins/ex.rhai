// Ex, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Ex", name: "rhai_ex" };

fn variation(z, p) {
    let r = z.abs();
    let theta = atan2(z.im, z.re);
    let n0 = sin(theta + r);
    let n1 = cos(theta - r);
    let m0 = n0 * n0 * n0;
    let m1 = n1 * n1 * n1;
    complex(r * (m0 + m1), r * (m0 - m1))
}
