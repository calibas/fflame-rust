// The built-in Manowar (Fractint's), z^2 + m + c with m the previous z,
// as a formula of your own (the Escape panel's Edit as Formula). Both z
// and m start at the pixel; m is carried from pass to pass, which deep
// zoom follows too.

const PARAMS = #{
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout", hint: "The squared radius past which a pixel has escaped" },
};

const INFO = #{ title: "Manowar", center: complex(-0.4, 0.0), maxiter: 256 };

fn start(pixel, p, ctx) {
    let c = if ctx.julia { ctx.c } else { pixel };
    #{ z: pixel, c: c, m: pixel }
}

fn step(s, p) {
    #{ z: s.z * s.z + s.m + s.c, m: s.z }
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}

fn degree(p) {
    2.0
}
