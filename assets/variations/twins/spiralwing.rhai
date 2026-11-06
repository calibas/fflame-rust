// Spiral Wing, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Spiral Wing", name: "rhai_spiralwing" };

fn variation(z, p) {
    let c1 = z.re * z.re;
    let c2 = z.im * z.im;
    let d = 1.0 / (c1 + c2 + 1.0e-30);
    let s2 = sin(c2);
    complex(d * cos(c1) * s2, d * sin(c1) * s2)
}
