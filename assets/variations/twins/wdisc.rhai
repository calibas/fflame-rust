/// Wedge disc — emits `r · (cos a, sin a)` where `a = π / (sqrt(x²+y²) +
/// 1)` and `r = atan2(y, x) / π`. Points with `r > 0` get their `a`
/// reflected via `π − a`. Maps the input plane onto a folded-disc pattern.
///
/// # Authors
/// - Michael Faber

// W-Disc, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "W-Disc", name: "rhai_wdisc", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let pi = 3.14159265358979;
    let inv_pi = 0.31830988618379;
    let a = pi / (sqrt(z.re * z.re + z.im * z.im) + 1.0);
    let r = atan2(z.im, z.re) * inv_pi;
    if (r > 0.0) {
        a = pi - a;
    }
    complex(r * cos(a), r * sin(a))
}
