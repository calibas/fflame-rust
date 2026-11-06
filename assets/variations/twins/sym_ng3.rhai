// Sym NG3, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    sepx: #{
        value: 0.5, min: -100.0, max: 100.0, free: true,
        caption: "sepx",
        hint: "Network-symmetry parameter (see JWildfire SymNetG3).",
    },
    sepy: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "sepy",
        hint: "Network-symmetry parameter (see JWildfire SymNetG3).",
    },
    step: #{
        value: 2.0, min: -100.0, max: 100.0, free: true,
        caption: "step",
        hint: "Network-symmetry parameter (see JWildfire SymNetG3).",
    },
};

const INFO = #{ title: "Sym NG3", name: "rhai_sym_ng3" };

fn variation(z, p) {
    let sepx = p.sepx;
    let sepy = p.sepy;
    let step = p.step;
    let zx = z.re;
    let zy = z.im;
    let idx = to_int(to_float(floor(rand() * 2.0)));
    let a = 0.0;
    let b = 0.0;
    let c = 0.0;
    let d = 0.0;
    let e = 0.0;
    let ff = 0.0;
    let t1 = (idx);
    if t1 == 0 {
        a = 1.0;
        b = 0.0;
        c = -sepx - step / 2.0;
        d = 0.0;
        e = 1.0;
        ff = -sepy - step / 2.0;
    } else {
        a = -1.0;
        b = 0.0;
        c = sepx + step / 2.0;
        d = 0.0;
        e = -1.0;
        ff = sepy + step / 2.0;
    }
    let ox = a * zx + b * zy + c;
    let oy = d * zx + e * zy + ff;
    complex(ox, oy)
}
