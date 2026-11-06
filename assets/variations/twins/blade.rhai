// Blade, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Blade", name: "rhai_blade" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let r = rand() * w * sqrt(z.re * z.re + z.im * z.im);
    let sinr = sin(r);
    let cosr = cos(r);
    let fx = w * z.re * (cosr + sinr);
    let fy = w * z.re * (cosr - sinr);
    complex(fx * inv_w, fy * inv_w)
}
