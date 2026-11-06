// TQ Mirror, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "A",
        hint: "Y threshold for the central-region diagonal-mirror branch.",
    },
    b: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "B",
        hint: "X offset added to the central-region's right-side gate.",
    },
    c: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "C",
        hint: "Y offset added to the central-region's top-side gate.",
    },
    d: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "D",
        hint: "X offset added to the outer-boundary swap gate.",
    },
    e: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "E",
        hint: "Y offset added to the outer-boundary swap gate.",
    },
    f: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "F",
        hint: "Additive X shift in the third-quadrant branch (scaled by weight).",
    },
    g: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "G",
        hint: "Additive Y shift in the third-quadrant branch.",
    },
    h: #{
        value: 1.0, min: -10.0, max: 10.0, free: true,
        caption: "H",
        hint: "Scale factor on the central-region diagonal-mirror output.",
    },
    type: #{
        value: false,
        caption: "Type",
        hint: "When on, pass the input through. When off, swap (x ↔ y).",
    },
};

const INFO = #{ title: "TQ Mirror", name: "rhai_tqmirror" };

fn variation(z, p, ctx) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    let e = p.e;
    let f = p.f;
    let g = p.g;
    let h = p.h;
    let type_p = to_float(p.type);
    let w = ctx.weight;
    let inv_w = 1.0 / (if abs(w) < 1.0e-30 { 1.0e-30 } else { w });
    let fx = 0.0;
    let fy = 0.0;
    if (d + z.re < 0.0 || e + z.im < 0.0) {
        if (type_p != 0.0) {
            fx = z.re;
            fy = z.im;
        } else {
            fx = z.im;
            fy = z.re;
        }
    } else if (z.re < 0.0 && z.im < 0.0) {
        fx = z.re + w * f;
        fy = z.im + w * g;
    } else if (z.re < w && z.im < a && z.re + b > 0.0 && z.im + c > 0.0) {
        fx = -z.im * h;
        fy = -z.re * h;
    } else {
        fx = z.re;
        fy = z.im;
    }
    complex(fx * inv_w, fy * inv_w)
}
