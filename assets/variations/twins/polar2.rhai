/// Variant of Polar with log-radius output. Compresses large distances and
/// expands small ones; good for revealing distant structure.
///
/// # Authors
/// - Apophysis Plugin Pack

// Polar2, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Polar2", name: "rhai_polar2", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let PI = 3.14159265359;
    let r2 = z.norm_sqr();
    let new_x = atan2(z.re, z.im) / PI();
    let new_y = 0.5 * ln(r2) / PI();
    complex(new_x, new_y)
}
