/// Flips the Y coordinate inside a circular region sized by the variation's
/// own weight. Outside the circle, points pass through unchanged.
///
/// # Authors
/// - Michael Faber

// Flip Circle, the built-in variation's twin: the points inside the circle of the weight's radius flipped across the real axis.

const INFO = #{ title: "Flip Circle", name: "rhai_flipcircle", category: "advanced_2d", phase: "any" };

fn variation(z, p, ctx) {
    let w = ctx.weight;
    let inside = (z.re * z.re + z.im * z.im) <= w * w;
    if inside { complex(z.re, -z.im) } else { z }
}
