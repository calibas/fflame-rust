// Flip Y, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Flip Y", name: "rhai_flipy" };

fn variation(z, p) {
    let sy = (if z.re > 0.0 { -1.0 } else { 1.0 });
    complex(z.re, sy * z.im)
}
