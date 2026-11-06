/// 3-point pivot/overlap IFS triangle — picks one of three sub-
/// transformations uniformly: a Y-sign-flipped polar pivot, a scale-to-
/// origin by 1/3, or a scale-by-1/3 with X offset of 2/3. Together these
/// implement an IFS that draws a Sierpinski-like dust pattern with a
/// circular pivot.
///
/// # Authors
/// - Jesus Sosa

// Dust Point, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Dust Point", name: "rhai_dustpoint", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let p_sign = (if rand() < 0.5 { 1.0 } else { -1.0 });
    let r = sqrt(max(1.0e-30, z.re * z.re + z.im * z.im));
    let w = rand();
    if (w < 0.5) {
        return complex(z.re / r - 1.0, p_sign * z.im / r);
    } else if (w < 0.75) {
        return complex(z.re / 3.0, z.im / 3.0);
    }
    complex(z.re / 3.0 + 2.0 / 3.0, z.im / 3.0)
}
