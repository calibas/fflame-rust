/// Classic exponential variation — emits `exp(x − 1) · (cos(π·y),
/// sin(π·y))`. Same shape as `exp2` but with a slightly different
/// exponential argument (subtracts 1 so the unit radius lies at x = 1
/// instead of x = 0).
///
/// # Authors
/// - Scott Draves

// Exponential, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Exponential", name: "rhai_exponential", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let pi = 3.14159265358979;
    let r = pi * z.im;
    let d = exp(z.re - 1.0);
    complex(cos(r) * d, sin(r) * d)
}
