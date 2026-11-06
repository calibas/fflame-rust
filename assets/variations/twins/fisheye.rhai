// Fisheye, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Fisheye", name: "rhai_fisheye" };

fn variation(z, p) {
    let r0 = sqrt(z.re * z.re + z.im * z.im);
    let r = 2.0 * r0 / (r0 + 1.0);
    let denom = r0 + 1.0e-6;
    complex(r * z.im / denom, r * z.re / denom)
}
