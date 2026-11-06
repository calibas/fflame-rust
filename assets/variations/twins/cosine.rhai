/// Complex cosine of `π·x + iy` — output is `(cos(πx)·cosh(y),
/// -sin(πx)·sinh(y))`. Horizontally periodic with vertical exponential
/// growth.

// Cosine, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Cosine", name: "rhai_cosine", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r = z.re * 3.14159265358979;
    complex(cos(r) * cosh(z.im), -sin(r) * sinh(z.im))
}
