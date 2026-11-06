/// Band Symmetry Group variations sym_bg1..sym_bg7
///
/// # Authors
/// - Jesus Sosa

// Sym BG6, the built-in variation's twin, translated from its WGSL.

const PARAMS = #{
    stepx: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "stepx",
        hint: "Band-symmetry parameter (see JWildfire SymBandG6).",
    },
    stepy: #{
        value: 0.0, min: -100.0, max: 100.0, free: true,
        caption: "stepy",
        hint: "Band-symmetry parameter (see JWildfire SymBandG6).",
    },
};

const INFO = #{ title: "Sym BG6", name: "rhai_sym_bg6", category: "plugin", phase: "any" };

fn variation(z, p) {
    let stepx = p.stepx;
    let stepy = p.stepy;
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
        c = -stepx / 2.0 - 1.0;
        d = 0.0;
        e = 1.0;
        ff = -stepy / 2.0 - 0.5;
    } else if t1 == 1 {
        a = -1.0;
        b = 0.0;
        c = stepx / 2.0 + 1.0;
        d = 0.0;
        e = 1.0;
        ff = stepy / 2.0 - 0.5;
    } else if t1 == 2 {
        a = 1.0;
        b = 0.0;
        c = -stepx / 2.0 - 1.0;
        d = 0.0;
        e = -1.0;
        ff = -stepy / 2.0 + 0.5;
    } else {
        a = -1.0;
        b = 0.0;
        c = stepx / 2.0 + 1.0;
        d = 0.0;
        e = -1.0;
        ff = stepy / 2.0 + 0.5;
    }
    let ox = a * zx + b * zy + c;
    let oy = d * zx + e * zy + ff;
    complex(ox, oy)
}
