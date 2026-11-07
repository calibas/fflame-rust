/// Exp/sin polar inversion — outputs `(w/(2π) · exp(y) · sin x, w/(2π) ·
/// exp(y) · cos x)`. The inverse of `polar` — converts log-polar
/// coordinates back to cartesian.

// Unpolar, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Unpolar", name: "rhai_unpolar", category: "advanced_2d", phase: "any" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let inv_two_pi = 0.15915494309189535;
    let r = exp(z.im);
    let s = sin(z.re);
    let c = cos(z.re);
    let vvar_2 = w * inv_two_pi;
    let fx = vvar_2 * r * s;
    let fy = vvar_2 * r * c;
    complex(fx * inv_w, fy * inv_w)
}
