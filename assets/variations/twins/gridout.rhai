/// Snaps points onto a discrete grid by following the nearest cell-edge
/// direction. Produces a chunky, tile-like output.
///
/// # Authors
/// - Michael Faber
/// - DarkBeam

// Gridout, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Gridout", name: "rhai_gridout", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let x = round_even(z.re);
    let y = round_even(z.im);
    let ox = z.re;
    let oy = z.im;
    if (y <= 0.0) {
        if (x > 0.0) {
            if (-y >= x) {
                ox = z.re + 1.0;
            } else {
                oy = z.im + 1.0;
            }
        } else if (y <= x) {
            ox = z.re + 1.0;
        } else {
            oy = z.im - 1.0;
        }
    } else if (x > 0.0) {
        if (y >= x) {
            ox = z.re - 1.0;
        } else {
            oy = z.im + 1.0;
        }
    } else if (y > -x) {
        ox = z.re - 1.0;
    } else {
        oy = z.im - 1.0;
    }
    complex(ox, oy)
}
