// Secant2, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Secant2", name: "rhai_secant2" };

fn variation(z, p) {
    let r = sqrt(z.re * z.re + z.im * z.im) + 1.0e-30;
    let cr = cos(r);
    let icr = 1.0 / (if cr == 0.0 { 1.0e-20 } else { cr });
    let y_offset = (if cr < 0.0 { 1.0 } else { -1.0 });
    complex(z.re, icr + y_offset)
}
