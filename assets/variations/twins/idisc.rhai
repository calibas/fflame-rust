// I-Disc, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "I-Disc", name: "rhai_idisc" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let pi = 3.14159265358979;
    let inv_pi = 0.31830988618379;
    let v = w * inv_pi;
    let a = pi / (sqrt(z.re * z.re + z.im * z.im) + 1.0);
    let r = atan2(z.im, z.re) * v;
    complex(r * cos(a) * inv_w, r * sin(a) * inv_w)
}
