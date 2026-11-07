/// Pulls every point toward the origin with a strength that drops off with
/// distance. Produces a magnifying-glass / scrying-orb effect.
///
/// # Authors
/// - Apophysis Plugin Pack

// Scry, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Scry", name: "rhai_scry", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let t = z.norm_sqr();
    let r = 1.0 / (sqrt(t) * (t + 1.0));
    complex(z.re * r, z.im * r)
}
