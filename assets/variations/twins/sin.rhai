/// Treats the input as a complex number and applies the sine function.
/// Output is `sin(x)*cosh(y), cos(x)*sinh(y)` — horizontally periodic,
/// growing vertically away from the real axis.
///
/// # Authors
/// - cothe

// Sin, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Sin", name: "rhai_sin", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let s = sin(z.re);
    let c = cos(z.re);
    let sh = sinh(z.im);
    let ch = cosh(z.im);
    complex(s * ch, c * sh)
}
