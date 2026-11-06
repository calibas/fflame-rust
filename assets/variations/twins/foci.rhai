// Foci, the built-in variation's twin, translated from its WGSL.

const INFO = #{ title: "Foci", name: "rhai_foci" };

fn variation(z, p) {
    let expx = exp(z.re) * 0.5;
    let expnx = 0.5 * exp(-z.re);
    let tmp = expx + expnx - cos(z.im);
    if (tmp == 0.0) {
        tmp = 1.0e-6;
    }
    tmp = 1.0 / tmp;
    let new_x = (expx - expnx) * tmp;
    let new_y = sin(z.im) * tmp;
    complex(new_x, new_y)
}
