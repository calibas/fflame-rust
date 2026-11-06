/// Power warp — emits `r^(x/r) · (y/r, x/r)` where `r = sqrt(x²+y²)`. The
/// exponent depends on the input's angle (`cos θ`), and the output
/// coordinates are swapped relative to the input (a 90° rotation; cpp's xy-
/// swap quirk, preserved).

// Power, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Power", name: "rhai_power", category: "advanced_2d", phase: "any" };

fn variation(z, p) {
    let r = sqrt(z.re * z.re + z.im * z.im) + 1.0e-30;
    let exponent = z.re / r;
    let m = pow(r, exponent);
    complex(m * z.im / r, m * z.re / r)
}
