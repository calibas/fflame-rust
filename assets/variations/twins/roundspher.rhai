/// Rounded spherical inversion — softens the standard spherical inversion
/// `(x, y)/r²` by adding `(2/π)²` to the reciprocal-of-radius term,
/// yielding `(w·x, w·y) / (1 + (2/π)²·r²)`. Smooths out the singularity at
/// the origin.
///
/// # Authors
/// - Raykoid666

// Round Spher, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Round Spher", name: "rhai_roundspher", category: "advanced_2d", phase: "any" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let two_over_pi_sq = 0.40528473456935106;
    let d = max(1.0e-30, z.re * z.re + z.im * z.im);
    let e = 1.0 / d + two_over_pi_sq;
    let safe_e = (if abs(e) < 1.0e-30 { 1.0e-30 } else { e });
    let fx = w * (w / d * z.re / safe_e);
    let fy = w * (w / d * z.im / safe_e);
    complex(fx * inv_w, fy * inv_w)
}
