/// 4-branch Sierpinski-triangle-like IFS — picks one of four branches
/// uniformly per iteration, each pivoting around the origin or one of the
/// three vertices of an equilateral triangle. Each branch is a
/// rotation+half-scale around its pivot. Based on Paul Bourke's `lace.c`.
///
/// # Authors
/// - Jesus Sosa

// Lace (JS), the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Lace (JS)", name: "rhai_lace_js", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r = 2.0;
    let inv_r = 0.5;
    let sqrt3_2 = 0.8660254037844386;
    let r0 = sqrt(z.re * z.re + z.im * z.im);
    let weight = rand();
    let x = 0.0;
    let y = 0.0;
    let w = 0.0;
    if (weight > 0.75) {
        w = atan2(z.im, z.re - 1.0);
        y = -r0 * cos(w) * inv_r + 1.0;
        x = -r0 * sin(w) * inv_r;
    } else if (weight > 0.50) {
        w = atan2(z.im - sqrt3_2, z.re + 0.5);
        y = -r0 * cos(w) * inv_r - 0.5;
        x = -r0 * sin(w) * inv_r + sqrt3_2;
    } else if (weight > 0.25) {
        w = atan2(z.im + sqrt3_2, z.re + 0.5);
        y = -r0 * cos(w) * inv_r - 0.5;
        x = -r0 * sin(w) * inv_r - sqrt3_2;
    } else {
        w = atan2(z.im, z.re);
        y = -r0 * cos(w) * inv_r;
        x = -r0 * sin(w) * inv_r;
    }
    complex(x, y)
}
