/// Maps each coordinate through `sin`, folding the plane into a
/// [-1, 1] tile. Produces wavy, ribbon-like patterns that repeat
/// outward from the origin.
///
/// # Authors
/// - Scott Draves

// Sinusoidal, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Sinusoidal", name: "rhai_sinusoidal", category: "basic_2d", phase: "any", inverse_branches: 16 };

fn variation(z, p) {
    complex(sin(z.re), sin(z.im))
}

fn inverse(z, p, branch) {
    if abs(z.re) > 1.0 || abs(z.im) > 1.0 {
        return no_preimage();
    }
    let bx = branch % 4;
    let by = branch / 4;
    let sx = asin(z.re);
    let sy = asin(z.im);
    let x = (if (bx & 1) == 0 { sx } else { 3.14159265358979 - sx }) + 6.28318530717959 * (to_float(bx / 2) - 1.0);
    let y = (if (by & 1) == 0 { sy } else { 3.14159265358979 - sy }) + 6.28318530717959 * (to_float(by / 2) - 1.0);
    complex(x, y)
}

fn image_gap(z, p) {
    max(abs(z.re), abs(z.im)) - 1.0
}
