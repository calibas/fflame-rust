/// Enneper's-surface parametric mapping — `(x(1 − x²/3 + y²), y(1 − y²/3 +
/// x²))`. Inspired by Alfred Enneper's classical minimal surface.
///
/// # Authors
/// - Raykoid666

// Ennepers, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Ennepers", name: "rhai_ennepers", category: "advanced_2d", phase: "any", replace: true };

fn variation(z, p) {
    let x2 = z.re * z.re;
    let y2 = z.im * z.im;
    complex(z.re * (1.0 - x2 / 3.0 + y2), z.im * (1.0 - y2 / 3.0 + x2))
}
