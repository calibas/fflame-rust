// Bent, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Bent", name: "rhai_bent" };

fn variation(z, p) {
    let nx = (if z.re >= 0.0 { z.re } else { 2.0 * z.re });
    let ny = (if z.im >= 0.0 { z.im } else { z.im / 2.0 });
    complex(nx, ny)
}
