/// Cosine-of-tangent rays — uses `1/cos((t+ε)·tan(1/t+ε))` on the squared
/// radius `t`. Creates intricate ray patterns radiating from the origin.
///
/// # Authors
/// - Raykoid666

// Rays2, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Rays2", name: "rhai_rays2", category: "advanced_2d", phase: "any", replace: true };

fn variation(z, p) {
    let eps = 1.0e-30;
    let t = z.re * z.re + z.im * z.im + eps;
    let u = 1.0 / cos((t + eps) * tan(1.0 / t + eps));
    let scale = u * t / 10.0;
    complex(scale / (if z.re == 0.0 { eps } else { z.re }), scale / (if z.im == 0.0 { eps } else { z.im }))
}
