// Two Face, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Two Face", name: "rhai_twoface" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let r = w;
    if (z.re > 0.0) {
        let denom = z.re * z.re + z.im * z.im;
        r = r / max(denom, 1.0e-30);
    }
    complex(r * z.re * inv_w, r * z.im * inv_w)
}
