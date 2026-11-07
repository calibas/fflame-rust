/// Y-flip when x > 0 — leaves the input alone when `x ≤ 0`; flips the sign
/// of Y when `x > 0`. Useful as a building block for left/right asymmetric
/// flames.
///
/// # Authors
/// - Michael Faber

// Flip Y, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Flip Y", name: "rhai_flipy", category: "basic_2d", phase: "any" };

fn variation(z, p) {
    let sy = (if z.re > 0.0 { -1.0 } else { 1.0 });
    complex(z.re, sy * z.im)
}
