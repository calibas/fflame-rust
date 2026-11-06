/// RNG twin trigonal — picks a random `r = rand · w · sqrt(x² + y²)`,
/// computes `diff = log₁₀(sin² r) + cos r` (forced to −30 when degenerate),
/// then emits `(w · x · diff, w · x · (diff − sin r · π))`. The log term
/// creates twin trigonometric interference patterns.
///
/// # Authors
/// - Z+

// Twin Trian, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Twin Trian", name: "rhai_twintrian", category: "advanced_2d", phase: "any" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let pi = 3.14159265358979;
    let inv_log10 = 0.43429448190325176;
    let r = rand() * w * (sqrt(z.re * z.re + z.im * z.im) + 1.0e-30);
    let sinr = sin(r);
    let cosr = cos(r);
    let diff = ln(max(sinr * sinr, 1.0e-30)) * inv_log10 + cosr;
    if (abs(diff) < 1.0e-30) {
        diff = -30.0;
    }
    let fx = w * z.re * diff;
    let fy = w * z.re * (diff - sinr * pi);
    complex(fx * inv_w, fy * inv_w)
}
