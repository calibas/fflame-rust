/// 3-branch Hadamard IFS — picks one of three affine maps uniformly per
/// iteration: half-scale identity, or one of two half-scale rotation-plus-
/// offset maps. Produces a Hadamard fractal pattern. Based on Paul Bourke's
/// `roger18.c`.
///
/// # Authors
/// - Jesus Sosa

// Hadamard (JS), the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Hadamard (JS)", name: "rhai_hadamard_js", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r1 = rand();
    let x = 0.0;
    let y = 0.0;
    if (r1 < 0.333) {
        x = z.re * 0.5;
        y = z.im * 0.5;
    } else {
        let r2 = rand();
        if (r2 < 0.666) {
            x = z.im * 0.5;
            y = -z.re * 0.5 - 0.5;
        } else {
            x = -z.im * 0.5 - 0.5;
            y = z.re * 0.5;
        }
    }
    complex(x, y)
}
