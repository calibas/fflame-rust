/// 3-branch inverse-tree IFS — picks one of three branches uniformly per
/// iteration: half-scale identity, or per-axis `1/(coord + 1)` inverse (one
/// axis at a time). Produces an inverse-tree fractal. Based on Paul
/// Bourke's `trifraction2`.
///
/// # Authors
/// - Jesus Sosa

// Inv-Tree (JS), the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Inv-Tree (JS)", name: "rhai_invtree_js", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r1 = rand();
    let x = 0.0;
    let y = 0.0;
    if (r1 < 0.333) {
        x = z.re * 0.5;
        y = z.im * 0.5;
    } else {
        let r2 = rand();
        let safe_x_p1 = (if abs(z.re + 1.0) < 1.0e-30 { 1.0e-30 } else { z.re + 1.0 });
        let safe_y_p1 = (if abs(z.im + 1.0) < 1.0e-30 { 1.0e-30 } else { z.im + 1.0 });
        if (r2 < 0.666) {
            x = 1.0 / safe_x_p1;
            y = z.im / safe_y_p1;
        } else {
            x = z.re / safe_x_p1;
            y = 1.0 / safe_y_p1;
        }
    }
    complex(x, y)
}
