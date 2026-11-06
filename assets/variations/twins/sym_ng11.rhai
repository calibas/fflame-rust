/// Network Symmetry Group variations sym_ng1..sym_ng17
///
/// # Authors
/// - Jesus Sosa

// Sym NG11, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    space: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "space",
        hint: "Network-symmetry parameter (see JWildfire SymNetG11).",
    },
    spacex: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "spacex",
        hint: "Network-symmetry parameter (see JWildfire SymNetG11).",
    },
    spacey: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "spacey",
        hint: "Network-symmetry parameter (see JWildfire SymNetG11).",
    },
    stepx: #{
        value: 1.5, min: -100.0, max: 100.0, free: true,
        caption: "stepx",
        hint: "Network-symmetry parameter (see JWildfire SymNetG11).",
    },
    stepy: #{
        value: 1.5, min: -100.0, max: 100.0, free: true,
        caption: "stepy",
        hint: "Network-symmetry parameter (see JWildfire SymNetG11).",
    },
};

const INFO = #{ title: "Sym NG11", name: "rhai_sym_ng11", category: "plugin", phase: "any" };

fn variation(z, p) {
    let space = p.space;
    let spacex = p.spacex;
    let spacey = p.spacey;
    let stepx = p.stepx;
    let stepy = p.stepy;
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
        c = spacex + stepx / 2.0;
        d = 0.0;
        e = 1.0;
        ff = spacey + stepy / 2.0;
    } else if t1 == 1 {
        a = -1.0;
        b = 0.0;
        c = -spacex + stepx / 2.0;
        d = 0.0;
        e = -1.0;
        ff = -spacey + stepy / 2.0;
    } else if t1 == 2 {
        a = -0.0;
        b = 1.0;
        c = spacex + stepx / 2.0;
        d = -1.0;
        e = -0.0;
        ff = -spacey + stepy / 2.0;
    } else if t1 == 3 {
        a = -0.0;
        b = -1.0;
        c = -spacex + stepx / 2.0;
        d = 1.0;
        e = -0.0;
        ff = spacey + stepy / 2.0;
    } else if t1 == 4 {
        a = -1.0;
        b = 0.0;
        c = -spacex - stepx / 2.0;
        d = 0.0;
        e = 1.0;
        ff = spacey - stepy / 2.0;
    } else if t1 == 5 {
        a = -0.0;
        b = -1.0;
        c = -spacex - stepx / 2.0;
        d = -1.0;
        e = -0.0;
        ff = -spacey - stepy / 2.0;
    } else if t1 == 6 {
        a = -0.0;
        b = 1.0;
        c = spacex - stepx / 2.0;
        d = 1.0;
        e = -0.0;
        ff = spacey - stepy / 2.0;
    } else {
        a = 1.0;
        b = 0.0;
        c = spacex - stepx / 2.0;
        d = 0.0;
        e = -1.0;
        ff = -spacey - stepy / 2.0;
    }
    let ox = a * zx + b * zy + c;
    let oy = d * zx + e * zy + ff;
    complex(ox, oy)
}
