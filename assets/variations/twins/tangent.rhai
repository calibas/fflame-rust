/// Real tan-by-cos — X is `sin(x)/cos(y)`, Y is `tan(y)`. Note: not the
/// same as Tan from trig.rs (which is the complex tangent).

// Tangent, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Tangent", name: "rhai_tangent", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let d = cos(z.im);
    let inv = 1.0 / (if d == 0.0 { 1.0e-20 } else { d });
    complex(sin(z.re) * inv, tan(z.im))
}
