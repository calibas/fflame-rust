// Handkerchief, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Handkerchief", name: "rhai_handkerchief" };

fn variation(z, p) {
    let r = z.abs();
    let theta = atan2(z.im, z.re);
    complex(r * sin(theta + r), r * cos(theta - r))
}
