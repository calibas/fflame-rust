// Sym NG4, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    sepx: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "sepx",
        hint: "Network-symmetry parameter (see JWildfire SymNetG4).",
    },
    sepy: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "sepy",
        hint: "Network-symmetry parameter (see JWildfire SymNetG4).",
    },
    stepx: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "stepx",
        hint: "Network-symmetry parameter (see JWildfire SymNetG4).",
    },
    stepy: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "stepy",
        hint: "Network-symmetry parameter (see JWildfire SymNetG4).",
    },
};

const INFO = #{ title: "Sym NG4", name: "rhai_sym_ng4" };

fn variation(z, p) {
    let sepx = p.sepx;
    let sepy = p.sepy;
    let stepx = p.stepx;
    let stepy = p.stepy;
    let sx = sepx / 2.0;
    let sy = sepy / 2.0;
    let stpx = stepx / 2.0;
    let stpy = stepy / 2.0;
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
        c = -sx - 2.0 - stpx;
        d = 0.0;
        e = 1.0;
        ff = sy - 1.5 - stpy;
    } else if t1 == 1 {
        a = -1.0;
        b = 0.0;
        c = sx - stpx;
        d = 0.0;
        e = -1.0;
        ff = -sy - 0.5 - stpy;
    } else if t1 == 2 {
        a = 1.0;
        b = 0.0;
        c = -sx + stpx;
        d = 0.0;
        e = -1.0;
        ff = -sy + 0.5 + stpy;
    } else {
        a = -1.0;
        b = 0.0;
        c = sx + 2.0 + stpx;
        d = 0.0;
        e = 1.0;
        ff = sy - 0.5 + stpy;
    }
    let ox = a * zx + b * zy + c;
    let oy = d * zx + e * zy + ff;
    complex(ox, oy)
}
