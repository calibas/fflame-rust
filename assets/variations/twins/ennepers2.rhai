// Ennepers2, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Coefficient on the X factor.",
    },
    b: #{
        value: 0.3333, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "Coefficient on the Y factor.",
    },
    c: #{
        value: 0.075, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Square-root correction strength applied to both X and Y outputs.",
    },
};

const INFO = #{ title: "Ennepers2", name: "rhai_ennepers2" };

fn variation(z, p) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let r2 = 1.0 / max(z.re * z.re + z.im * z.im, 1.0e-30);
    let ax = a * z.re;
    let by = b * z.im;
    let dxy = ax * ax - by * by;
    complex(z.re * (a * a - dxy * r2 - c * sqrt(abs(z.re))), z.im * (b * b - dxy * r2 - c * sqrt(abs(z.im))))
}

fn variation3(v, p) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let r2 = 1.0 / max(v.x * v.x + v.y * v.y, 1.0e-30);
    let ax = a * v.x;
    let by = b * v.y;
    let dxy = ax * ax - by * by;
    vec3(v.x * (a * a - dxy * r2 - c * sqrt(abs(v.x))), v.y * (b * b - dxy * r2 - c * sqrt(abs(v.y))), dxy * 0.5 * sqrt(r2))
}
