/// L∞-square perimeter to circle mapping — treats the input as a point
/// on the L∞ unit square (the Chebyshev-distance "square"), and maps it
/// onto a circle by treating its perimeter distance as the output angle.
/// `hole` adds a central radial offset, creating an inner gap.
///
/// # Authors
/// - Apophysis Plugin Pack

// Circlize, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    hole: #{
        value: 0.4, min: -10.0, max: 10.0, free: true,
        caption: "Hole",
        hint: "Center-hole radial offset. Larger values produce a bigger central gap.",
    },
};

const INFO = #{ title: "Circlize", name: "rhai_circlize", category: "advanced_2d", phase: "any" };

fn variation(z, p, ctx) {
    let hole = p.hole;
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let absx = abs(z.re);
    let absy = abs(z.im);
    let perimeter = 0.0;
    let side = 0.0;
    if (absx >= absy) {
        if (z.re >= absy) {
            perimeter = absx + z.im;
        } else {
            perimeter = 5.0 * absx - z.im;
        }
        side = absx;
    } else {
        if (z.im >= absx) {
            perimeter = 3.0 * absy - z.re;
        } else {
            perimeter = 7.0 * absy + z.re;
        }
        side = absy;
    }
    let var4_pi = w * 1.2732395447351627;
    let safe_side = (if side == 0.0 { 1.0e-30 } else { side });
    let r = var4_pi * side + hole;
    let a = 0.7853981633974483 * perimeter / safe_side - 0.7853981633974483;
    let fx = r * cos(a);
    let fy = r * sin(a);
    complex(fx * inv_w, fy * inv_w)
}
