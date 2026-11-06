// Diamond, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Diamond", name: "rhai_diamond" };

fn variation(z, p) {
    let r = z.abs();
    let theta = atan2(z.re, z.im);
    complex(sin(theta) * cos(r), cos(theta) * sin(r))
}
