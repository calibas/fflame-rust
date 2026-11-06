// Sym NG12, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    space: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "space",
        hint: "Network-symmetry parameter (see JWildfire SymNetG12).",
    },
    spacex: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "spacex",
        hint: "Network-symmetry parameter (see JWildfire SymNetG12).",
    },
    spacey: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "spacey",
        hint: "Network-symmetry parameter (see JWildfire SymNetG12).",
    },
};

const INFO = #{ title: "Sym NG12", name: "rhai_sym_ng12" };

fn variation(z, p) {
    let space = p.space;
    let spacex = p.spacex;
    let spacey = p.spacey;
    let zx = z.re + (space);
    let zy = z.im + (space);
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
        c = spacex;
        d = 0.0;
        e = 1.0;
        ff = spacey;
    } else if t1 == 1 {
        a = -1.0;
        b = 0.0;
        c = -spacex;
        d = 0.0;
        e = -1.0;
        ff = -spacey;
    } else if t1 == 2 {
        a = -0.0;
        b = 1.0;
        c = spacex;
        d = -1.0;
        e = -0.0;
        ff = -spacey;
    } else if t1 == 3 {
        a = -0.0;
        b = -1.0;
        c = -spacex;
        d = 1.0;
        e = -0.0;
        ff = spacey;
    } else if t1 == 4 {
        a = -1.0;
        b = 0.0;
        c = -spacex;
        d = 0.0;
        e = 1.0;
        ff = spacey;
    } else if t1 == 5 {
        a = -0.0;
        b = -1.0;
        c = -spacex;
        d = -1.0;
        e = -0.0;
        ff = -spacey;
    } else if t1 == 6 {
        a = -0.0;
        b = 1.0;
        c = spacex;
        d = 1.0;
        e = -0.0;
        ff = spacey;
    } else {
        a = 1.0;
        b = 0.0;
        c = spacex;
        d = 0.0;
        e = -1.0;
        ff = -spacey;
    }
    let ox = a * zx + b * zy + c;
    let oy = d * zx + e * zy + ff;
    complex(ox, oy)
}
