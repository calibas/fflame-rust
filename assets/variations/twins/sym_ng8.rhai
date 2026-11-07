/// Network Symmetry Group variations sym_ng1..sym_ng17
///
/// # Authors
/// - Jesus Sosa

// Sym NG8, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    sepx: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "sepx",
        hint: "Network-symmetry parameter (see JWildfire SymNetG8).",
    },
    sepy: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "sepy",
        hint: "Network-symmetry parameter (see JWildfire SymNetG8).",
    },
    skewx: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "skewx",
        hint: "Network-symmetry parameter (see JWildfire SymNetG8).",
    },
    skewy: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "skewy",
        hint: "Network-symmetry parameter (see JWildfire SymNetG8).",
    },
};

const INFO = #{ title: "Sym NG8", name: "rhai_sym_ng8", category: "plugin", phase: "any" };

fn variation(z, p) {
    let sepx = p.sepx;
    let sepy = p.sepy;
    let skewx = p.skewx;
    let skewy = p.skewy;
    let zx = z.re;
    let zy = z.im;
    let idx = to_int(to_float(floor(rand() * 8.0)));
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
        c = -1.0 - sepx / 2.0 - skewx / 2.0;
        d = 0.0;
        e = -1.0;
        ff = -sepy / 2.0 - skewy / 2.0;
    } else if t1 == 1 {
        a = -1.0;
        b = 0.0;
        c = 1.0 + sepx / 2.0 - skewx / 2.0;
        d = 0.0;
        e = 1.0;
        ff = sepy / 2.0 - skewy / 2.0;
    } else if t1 == 2 {
        a = -1.0;
        b = 0.0;
        c = 1.0 + sepx / 2.0 - skewx / 2.0;
        d = 0.0;
        e = -1.0;
        ff = -sepy / 2.0 - skewy / 2.0;
    } else if t1 == 3 {
        a = 1.0;
        b = 0.0;
        c = -1.0 - sepx / 2.0 - skewx / 2.0;
        d = 0.0;
        e = 1.0;
        ff = sepy / 2.0 - skewy / 2.0;
    } else if t1 == 4 {
        a = 1.0;
        b = 0.0;
        c = -1.0 - sepx / 2.0 + skewx / 2.0;
        d = 0.0;
        e = -1.0;
        ff = -sepy / 2.0 + skewy / 2.0;
    } else if t1 == 5 {
        a = -1.0;
        b = 0.0;
        c = 1.0 + sepx / 2.0 + skewx / 2.0;
        d = 0.0;
        e = 1.0;
        ff = sepy / 2.0 + skewy / 2.0;
    } else if t1 == 6 {
        a = -1.0;
        b = 0.0;
        c = 1.0 + sepx / 2.0 + skewx / 2.0;
        d = 0.0;
        e = -1.0;
        ff = -sepy / 2.0 + skewy / 2.0;
    } else {
        a = 1.0;
        b = 0.0;
        c = -1.0 - sepx / 2.0 + skewx / 2.0;
        d = 0.0;
        e = 1.0;
        ff = sepy / 2.0 + skewy / 2.0;
    }
    let ox = a * zx + b * zy + c;
    let oy = d * zx + e * zy + ff;
    complex(ox, oy)
}
