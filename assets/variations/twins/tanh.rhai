/// Complex hyperbolic tangent, applied to `z·π/4`. Compresses extreme
/// values toward ±1.
///
/// # Authors
/// - cothe

// Tanh, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Tanh", name: "rhai_tanh", category: "advanced_2d", phase: "any", z: "preserve_z" };

fn variation(z, p) {
    let pi_2 = 1.5707963267948966;
    let s2 = sinh(z.re * pi_2);
    let c2 = cosh(z.re * pi_2);
    let sin2 = sin(z.im * pi_2);
    let cos2 = cos(z.im * pi_2);
    let d = c2 + cos2;
    let inv = 1.0 / (if d == 0.0 { 1.0e-20 } else { d });
    complex(inv * s2, inv * sin2)
}

fn variation3(v, p) {
    let pi_2 = 1.5707963267948966;
    let s2 = sinh(v.x * pi_2);
    let c2 = cosh(v.x * pi_2);
    let sin2 = sin(v.y * pi_2);
    let cos2 = cos(v.y * pi_2);
    let d = c2 + cos2;
    let inv = 1.0 / (if d == 0.0 { 1.0e-20 } else { d });
    vec3(inv * s2, inv * sin2, 0.0)
}
