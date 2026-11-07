/// Angle-radius disc inversion — emits `(r·cos a, r·sin a)` where `a = π /
/// (sqrt(x²+y²) + 1)` and `r = atan2(y, x) · w/π`. Swaps the roles of
/// radius and angle in the output relative to a standard polar-to-cartesian
/// mapping.
///
/// # Authors
/// - Michael Faber

// I-Disc, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "I-Disc", name: "rhai_idisc", category: "advanced_2d", phase: "any" };

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
