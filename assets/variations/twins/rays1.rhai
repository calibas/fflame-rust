/// Cotangent + (2/π)² ray spread — computes `u = cot(sqrt(x²+y²)) + w ·
/// (2/π)²`, then emits `(u·t/x, u·t/y)` where `t = x² + y²`. Produces
/// concentric-ring ray patterns driven by the cotangent's pole structure.
///
/// # Authors
/// - Raykoid666

// Rays 1, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Rays 1", name: "rhai_rays1", category: "advanced_2d", phase: "any", replace: true };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let two_over_pi_sq = 0.40528473456935106;
    let t = z.re * z.re + z.im * z.im;
    let s = sqrt(max(1.0e-30, t));
    let tan_s = tan(s);
    let safe_tan = (if abs(tan_s) < 1.0e-30 { 1.0e-30 } else { tan_s });
    let u = 1.0 / safe_tan + w * two_over_pi_sq;
    let safe_x = (if abs(z.re) < 1.0e-30 { 1.0e-30 } else { z.re });
    let safe_y = (if abs(z.im) < 1.0e-30 { 1.0e-30 } else { z.im });
    let fx = w * u * t / safe_x;
    let fy = w * u * t / safe_y;
    complex(fx * inv_w, fy * inv_w)
}
