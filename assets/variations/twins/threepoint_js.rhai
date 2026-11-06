/// 3-branch Sierpinski-triangle IFS — picks one of three affine maps
/// uniformly per iteration (rotated half-scale, XY swap, mirrored half-
/// scale) to trace a Sierpinski-like triangle attractor. Based on Roger
/// Bagula's IFS.
///
/// # Authors
/// - Jesus Sosa

// Three Point IFS (JS), the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Three Point IFS (JS)", name: "rhai_threepoint_js", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r1 = rand();
    let x = 0.0;
    let y = 0.0;
    if (r1 < 0.333) {
        x = z.re * 0.5 - z.im * 0.5 + 0.5;
        y = -z.re * 0.5 - z.im * 0.5 + 0.5;
    } else {
        let r2 = rand();
        if (r2 < 0.666) {
            x = z.im;
            y = z.re;
        } else {
            x = -z.im * 0.5 + 0.5;
            y = -z.re * 0.5 + 0.5;
        }
    }
    complex(x, y)
}
