/// Apollonian Carpet — 6-branch random IFS using complex inversion
/// (branches 0 and 5) and four scaled translates by `r = 1/(1 + √2)`
/// (branches 1-4). Produces the classic Apollonian Carpet fractal. Based on
/// Paul Bourke's `roger17.c`.
///
/// # Authors
/// - Jesus Sosa

// Apocarpet (JS), the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Apocarpet (JS)", name: "rhai_apocarpet_js", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r = 1.0 / (1.0 + sqrt(2.0));
    let denom = z.re * z.re + z.im * z.im;
    let safe_denom = (if abs(denom) < 1.0e-30 { 1.0e-30 } else { denom });
    let weight = to_int(6.0 * rand());
    let x = 0.0;
    let y = 0.0;
    if (weight <= 0) {
        x = 2.0 * z.re * z.im / safe_denom;
        y = (z.re * z.re - z.im * z.im) / safe_denom;
    } else if (weight == 1) {
        x = z.re * r - r;
        y = z.im * r - r;
    } else if (weight == 2) {
        x = z.re * r + r;
        y = z.im * r + r;
    } else if (weight == 3) {
        x = z.re * r + r;
        y = z.im * r - r;
    } else if (weight == 4) {
        x = z.re * r - r;
        y = z.im * r + r;
    } else {
        x = (z.re * z.re - z.im * z.im) / safe_denom;
        y = 2.0 * z.re * z.im / safe_denom;
    }
    complex(x, y)
}
