/// Variant of Rays2 with `sqrt(cos(sin(...)·sin(...)))` and tangent on Y.
/// Denser ray pattern with sharper structure.
///
/// # Authors
/// - Raykoid666

// Rays3, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Rays3", name: "rhai_rays3", category: "advanced_2d", phase: "any", z: "preserve_z", replace: true };

fn variation(z, p) {
    let eps = 1.0e-30;
    let t = z.re * z.re + z.im * z.im + eps;
    let t2 = t * t;
    let u = 1.0 / sqrt(max(1.0e-20, cos(sin(t2 + eps) * sin(1.0 / t2 + eps))));
    complex((u * cos(t) * t / 10.0) / (if z.re == 0.0 { eps } else { z.re }), (u * tan(t) * t / 10.0) / (if z.im == 0.0 { eps } else { z.im }))
}

fn variation3(v, p) {
    let eps = 1.0e-30;
    let t = v.x * v.x + v.y * v.y + eps;
    let t2 = t * t;
    let u = 1.0 / sqrt(max(1.0e-20, cos(sin(t2 + eps) * sin(1.0 / t2 + eps))));
    vec3((u * cos(t) * t / 10.0) / (if v.x == 0.0 { eps } else { v.x }), (u * tan(t) * t / 10.0) / (if v.y == 0.0 { eps } else { v.y }), v.z)
}
