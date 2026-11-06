// Squarize, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Squarize", name: "rhai_squarize" };

fn variation(z, p) {
    let two_pi = 6.28318530717959;
    let inv_pi = 0.31830988618379;
    let s = sqrt(z.re * z.re + z.im * z.im);
    let a = atan2(z.im, z.re);
    if (a < 0.0) {
        a = a + two_pi;
    }
    let q = 4.0 * s * a * inv_pi;
    if (q <= s) {
        complex(s, q)
    } else if (q <= 3.0 * s) {
        complex(2.0 * s - q, s)
    } else if (q <= 5.0 * s) {
        complex(-s, 4.0 * s - q)
    } else if (q <= 7.0 * s) {
        complex(-(6.0 * s - q), -s)
    } else {
        complex(s, -(8.0 * s - q))
    }
}
