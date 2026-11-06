/// Variant of secant with a sign-dependent constant offset on Y. Passes X
/// through; Y becomes `1/cos(r) ± 1` depending on the sign of `cos(r)`.
/// Produces banded patterns with a sharp jump at the cos-sign boundary.

// Secant2, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Secant2", name: "rhai_secant2", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r = sqrt(z.re * z.re + z.im * z.im) + 1.0e-30;
    let cr = cos(r);
    let icr = 1.0 / (if cr == 0.0 { 1.0e-20 } else { cr });
    let y_offset = (if cr < 0.0 { 1.0 } else { -1.0 });
    complex(z.re, icr + y_offset)
}
