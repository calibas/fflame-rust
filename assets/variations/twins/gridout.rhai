// Gridout, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Gridout", name: "rhai_gridout" };

fn variation(z, p) {
    let x = round_even(z.re);
    let y = round_even(z.im);
    let ox = z.re;
    let oy = z.im;
    if (y <= 0.0) {
        if (x > 0.0) {
            if (-y >= x) {
                ox = z.re + 1.0;
            } else {
                oy = z.im + 1.0;
            }
        } else if (y <= x) {
            ox = z.re + 1.0;
        } else {
            oy = z.im - 1.0;
        }
    } else if (x > 0.0) {
        if (y >= x) {
            ox = z.re - 1.0;
        } else {
            oy = z.im + 1.0;
        }
    } else if (y > -x) {
        ox = z.re - 1.0;
    } else {
        oy = z.im - 1.0;
    }
    complex(ox, oy)
}
