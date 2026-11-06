/// RNG-driven blade fan — picks a random radius `r = rand · w · sqrt(x² +
/// y²)`, then emits `(w · x · (cos r + sin r), w · x · (cos r − sin r))`.
/// Both output axes are driven by the input X, so the result spreads along
/// the X axis like blades of a fan.
///
/// # Authors
/// - Z+

// Blade, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Blade", name: "rhai_blade", category: "advanced_2d", phase: "any" };

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
