// Smooth colouring, as a Rhai colouring.
//
// A colouring is up to three functions, run beside the formula: `start`
// gives each pixel the state it keeps, `step` updates it after each of
// the formula's passes, and `finish` turns it into the palette position
// (the index; it wraps). Each takes the formula's `z`. This one needs no
// state: the escape count, made continuous by how far past the bailout
// the last `z` went.

const PARAMS = #{
    scale: #{ value: 0.02, caption: "Scale", hint: "Palette positions per iteration" },
};

const INFO = #{ title: "Smooth", region: "outside" };

fn finish(s, z, p, ctx) {
    let n = to_float(ctx.numiter);
    (n + 1.0 - ln(ln(z.abs())) / ln(2.0)) * p.scale
}
