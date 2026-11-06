/// Network Symmetry Group variations sym_ng1..sym_ng17
///
/// # Authors
/// - Jesus Sosa

// Sym NG5, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    sepx: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "sepx",
        hint: "Network-symmetry parameter (see JWildfire SymNetG5).",
    },
    sepy: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "sepy",
        hint: "Network-symmetry parameter (see JWildfire SymNetG5).",
    },
    stepx: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "stepx",
        hint: "Network-symmetry parameter (see JWildfire SymNetG5).",
    },
};

const INFO = #{ title: "Sym NG5", name: "rhai_sym_ng5", category: "plugin", phase: "any" };

fn variation(z, p) {
    let sepx = p.sepx;
    let sepy = p.sepy;
    let stepx = p.stepx;
    let zx = z.re;
    let zy = z.im;
    let idx = to_int(to_float(floor(rand() * 4.0)));
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
        c = -sepx - 2.0 - stepx / 2.0;
        d = 0.0;
        e = 1.0;
        ff = sepy - 0.5;
    } else if t1 == 1 {
        a = -1.0;
        b = 0.0;
        c = sepx - stepx / 2.0;
        d = 0.0;
        e = -1.0;
        ff = -sepy - 0.5;
    } else if t1 == 2 {
        a = 1.0;
        b = 0.0;
        c = -sepx + stepx / 2.0;
        d = 0.0;
        e = -1.0;
        ff = -sepy - 0.5;
    } else {
        a = -1.0;
        b = 0.0;
        c = 2.0 + sepx + stepx / 2.0;
        d = 0.0;
        e = 1.0;
        ff = sepy - 0.5;
    }
    let ox = a * zx + b * zy + c;
    let oy = d * zx + e * zy + ff;
    complex(ox, oy)
}
