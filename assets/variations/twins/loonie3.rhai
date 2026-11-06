// Loonie 3, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Loonie 3", name: "rhai_loonie3" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let sqrvvar = w * w;
    let r2 = 2.0 * sqrvvar;
    if (z.re > 1.0e-6) {
        let s = (z.re * z.re + z.im * z.im) / z.re;
        r2 = s * s;
    }
    if (r2 < sqrvvar) {
        let scale = sqrt(max(sqrvvar / max(r2, 1.0e-30) - 1.0, 0.0));
        return complex(scale * z.re, scale * z.im);
    }
    z
}
