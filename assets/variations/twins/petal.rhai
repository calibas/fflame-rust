/// Petal shape — `(cos(x)·bx, cos(x)·by)` where `bx, by` are cubed
/// sine/cosine products of `(x, y)`. Produces flower-like radial
/// structures.
///
/// # Authors
/// - Raykoid666

// Petal, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Petal", name: "rhai_petal", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let cx = cos(z.re);
    let cy = cos(z.im);
    let sx = sin(z.re);
    let bx_base = cx * cy;
    let by_base = sx * cy;
    let bx = bx_base * bx_base * bx_base;
    let by = by_base * by_base * by_base;
    complex(cx * bx, cx * by)
}
