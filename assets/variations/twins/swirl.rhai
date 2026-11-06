// Swirl, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Swirl", name: "rhai_swirl" };

fn variation(z, p) {
    let r2 = z.norm_sqr();
    let s = sin(r2);
    let c = cos(r2);
    complex(z.re * s - z.im * c, z.re * c + z.im * s)
}
