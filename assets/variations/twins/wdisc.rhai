// W-Disc, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "W-Disc", name: "rhai_wdisc" };

fn variation(z, p) {
    let pi = 3.14159265358979;
    let inv_pi = 0.31830988618379;
    let a = pi / (sqrt(z.re * z.re + z.im * z.im) + 1.0);
    let r = atan2(z.im, z.re) * inv_pi;
    if (r > 0.0) {
        a = pi - a;
    }
    complex(r * cos(a), r * sin(a))
}
