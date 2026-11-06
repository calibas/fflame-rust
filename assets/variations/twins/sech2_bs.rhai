// Sech2 BS, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    x1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X1",
        hint: "Scales the argument of `sin(x)` in the internal computation.",
    },
    x2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "X2",
        hint: "Scales the argument of `cos(x)` in the internal computation.",
    },
    y1: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y1",
        hint: "Scales the argument of `sinh(y)` in the internal computation.",
    },
    y2: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "Y2",
        hint: "Scales the argument of `cosh(y)` in the internal computation.",
    },
};

const INFO = #{ title: "Sech2 BS", name: "rhai_sech2_bs" };

fn variation(z, p) {
    let x1 = p.x1;
    let x2 = p.x2;
    let y1 = p.y1;
    let y2 = p.y2;
    let s = sin(z.im * y1);
    let c = cos(z.im * y2);
    let sh = sinh(z.re * x1);
    let ch = cosh(z.re * x2);
    let d = cos(2.0 * z.im) + cosh(2.0 * z.re);
    let inv = 2.0 / (if d == 0.0 { 1.0e-20 } else { d });
    complex(inv * c * ch, -inv * s * sh)
}
