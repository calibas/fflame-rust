// PDJ, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    a: #{
        value: 0.0, min: -20.0, max: 20.0, free: true,
        caption: "A",
        hint: "Coefficient on the first sine - shapes the X output curve.",
    },
    b: #{
        value: 0.0, min: -20.0, max: 20.0, free: true,
        caption: "B",
        hint: "Coefficient on the first cosine - shapes the X output curve.",
    },
    c: #{
        value: 0.0, min: -20.0, max: 20.0, free: true,
        caption: "C",
        hint: "Coefficient on the second sine - shapes the Y output curve.",
    },
    d: #{
        value: 0.0, min: -20.0, max: 20.0, free: true,
        caption: "D",
        hint: "Coefficient on the second cosine - shapes the Y output curve.",
    },
};

const INFO = #{ title: "PDJ", name: "rhai_pdj" };

fn variation(z, p) {
    let a = p.a;
    let b = p.b;
    let c = p.c;
    let d = p.d;
    complex(sin(a * z.im) - cos(b * z.re), sin(c * z.re) - cos(d * z.im))
}
