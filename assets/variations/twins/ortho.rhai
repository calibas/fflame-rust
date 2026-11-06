// Ortho, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    in_p: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "In",
        hint: "Branch-selector multiplier for the inside-disc Möbius transform.",
    },
    out_p: #{
        value: 0.0, min: -10.0, max: 10.0, free: true,
        caption: "Out",
        hint: "Branch-selector multiplier for the outside-disc Möbius transform.",
    },
};

const INFO = #{ title: "Ortho", name: "rhai_ortho" };

fn variation(z, p) {
    let in_p = p.in_p;
    let out_p = p.out_p;
    let fpx = 0.0;
    let fpy = 0.0;
    let r = z.re * z.re + z.im * z.im;
    if (r < 1.0) {
        if (z.re >= 0.0) {
            let xo = (r + 1.0) / (2.0 * max(z.re, 1.0e-30));
            let ro = sqrt((z.re - xo) * (z.re - xo) + z.im * z.im);
            let theta = atan2(1.0, ro);
            let a = in_p * theta + atan2(z.im, xo - z.re) + theta;
            a = a - to_float(floor(a / (2.0 * theta))) * (2.0 * theta) - theta;
            fpx = xo - cos(a) * ro;
            fpy = sin(a) * ro;
        } else {
            let xo = -(r + 1.0) / (2.0 * min(z.re, -1.0e-30));
            let ro = sqrt((-z.re - xo) * (-z.re - xo) + z.im * z.im);
            let theta = atan2(1.0, ro);
            let a = in_p * theta + atan2(z.im, xo + z.re) + theta;
            a = a - to_float(floor(a / (2.0 * theta))) * (2.0 * theta) - theta;
            fpx = -(xo - cos(a) * ro);
            fpy = sin(a) * ro;
        }
    } else {
        let r_inv = 1.0 / sqrt(r);
        let ta = atan2(z.im, z.re);
        let x = r_inv * cos(ta);
        let y = r_inv * sin(ta);
        if (x >= 0.0) {
            let xo = (x * x + y * y + 1.0) / (2.0 * max(x, 1.0e-30));
            let ro = sqrt((x - xo) * (x - xo) + y * y);
            let theta = atan2(1.0, ro);
            let a = out_p * theta + atan2(y, xo - x) + theta;
            a = a - to_float(floor(a / (2.0 * theta))) * (2.0 * theta) - theta;
            let nx = xo - cos(a) * ro;
            let ny = sin(a) * ro;
            let nr = nx * nx + ny * ny;
            let inv_n = 1.0 / max(nr, 1.0e-30);
            fpx = nx * inv_n;
            fpy = ny * inv_n;
        } else {
            let xo = -(x * x + y * y + 1.0) / (2.0 * min(x, -1.0e-30));
            let ro = sqrt((-x - xo) * (-x - xo) + y * y);
            let theta = atan2(1.0, ro);
            let a = out_p * theta + atan2(y, xo + x) + theta;
            a = a - to_float(floor(a / (2.0 * theta))) * (2.0 * theta) - theta;
            let nx = -(xo - cos(a) * ro);
            let ny = sin(a) * ro;
            let nr = nx * nx + ny * ny;
            let inv_n = 1.0 / max(nr, 1.0e-30);
            fpx = nx * inv_n;
            fpy = ny * inv_n;
        }
    }
    complex(fpx, fpy)
}
