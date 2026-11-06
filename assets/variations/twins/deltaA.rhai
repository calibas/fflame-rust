/// Radial ratio + half angle difference — emits `r · (cos a, sin a)` where
/// `r = sqrt(y² + (x+1)²) / sqrt(y² + (x−1)²)` (ratio of distances to two
/// foci at `(∓1, 0)`) and `a` is half the angular difference between the
/// focus-angles. Produces conformal-mapping-style patterns.
///
/// # Authors
/// - Michael Faber

// Delta A, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Delta A", name: "rhai_deltaa", category: "advanced_2d", phase: "any" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let denom = sqrt(max(1.0e-30, z.im * z.im + (z.re - 1.0) * (z.re - 1.0)));
    let num = sqrt(max(1.0e-30, z.im * z.im + (z.re + 1.0) * (z.re + 1.0)));
    let avgr = w * (num / denom);
    let avga = (atan2(z.im, z.re - 1.0) - atan2(z.im, z.re + 1.0)) * 0.5;
    complex(avgr * cos(avga) * inv_w, avgr * sin(avga) * inv_w)
}
