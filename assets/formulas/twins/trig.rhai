// The built-in Sine / Cosine, sin z + c or cos z + c, as a formula of
// your own (the Escape panel's Edit as Formula). It escapes when |Im z|
// passes the bailout, where sin and cos grow like e^|Im z|.

const PARAMS = #{
    variant: #{
        value: 0,
        caption: "Function",
        hint: "0: sin z + c, 1: cos z + c.",
        choices: ["sin z + c", "cos z + c"],
    },
    bailout: #{ value: 50.0, min: 1.0, caption: "Bailout", hint: "The imaginary part's size past which a pixel has escaped" },
};

const INFO = #{ title: "Sine / Cosine", center: complex(0.0, 0.0), maxiter: 128 };

fn start(pixel, p, ctx) {
    if ctx.julia {
        #{ z: pixel, c: ctx.c }
    } else {
        #{ z: complex(0.0, 0.0), c: pixel }
    }
}

fn step(s, p) {
    let w = if p.variant == 0 { sin(s.z) } else { cos(s.z) };
    s.z = w + s.c;
    s
}

fn escaped(s, p) {
    s.z.im.abs() > p.bailout
}
