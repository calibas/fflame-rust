/// Inverse polar — emits `(1 + y) · (sin(π·x), cos(π·x))`. Treats X as the
/// angle (in units of π) and `1 + y` as the radius — the inverse of the
/// `polar` variation.

// Inv Polar, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Inv Polar", name: "rhai_invpolar", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let pi = 3.14159265358979;
    let ny = 1.0 + z.im;
    complex(ny * sin(z.re * pi), ny * cos(z.re * pi))
}
