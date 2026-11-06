/// Variant of `circlize` — same L∞-square-to-circle mapping, but `hole` is
/// folded into the variation-weighted radius so the whole formula factors
/// cleanly through the outer weight. Same visual result as `circlize` for
/// matching parameters, with subtly different weight semantics.
///
/// # Authors
/// - Michael Faber

// Circlize 2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    hole: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Hole",
        hint: "Center-hole radial offset (folded into the weighted radius).",
    },
};

const INFO = #{ title: "Circlize 2", name: "rhai_circlize2", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let hole = p.hole;
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
    let safe_side = (if side == 0.0 { 1.0e-30 } else { side });
    let r = side + hole;
    let a = 0.7853981633974483 * perimeter / safe_side - 0.7853981633974483;
    complex(r * cos(a), r * sin(a))
}
