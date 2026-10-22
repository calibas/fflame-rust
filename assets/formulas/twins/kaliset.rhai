// The built-in Kaliset, |z| / |z|^2 -/+ c (each part's absolute value),
// as a formula of your own (the Escape panel's Edit as Formula). It
// never escapes: the colourings that read the orbit draw it.

const PARAMS = #{
    plus_c: #{
        value: 0,
        caption: "Add c (instead of subtract)",
        hint: "0: z <- |z|/<z,z> - c (classic). 1: the + c branch.",
        choices: ["Subtract c (classic)", "Add c"],
    },
};

const INFO = #{ title: "Kaliset", center: complex(0.0, 0.0), maxiter: 24 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: pixel, c: pixel }
    }
}

fn step(s, p) {
    let r2 = s.z.norm_sqr();
    // The built-in's guard: zero at the origin.
    let folded = if r2 > 1e-30 { s.z.fold() / r2 } else { complex(0.0, 0.0) };
    s.z = if p.plus_c == 1 { folded + s.c } else { folded - s.c };
    s
}
