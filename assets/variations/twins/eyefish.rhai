// Eyefish, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Eyefish", name: "rhai_eyefish" };

fn variation(z, p) {
    let r_xy = z.abs() + 1.0;
    let scale = 2.0 / r_xy;
    complex(scale * z.re, scale * z.im)
}
