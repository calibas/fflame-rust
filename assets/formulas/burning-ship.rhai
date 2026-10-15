// The Burning Ship and its relatives: each part of z folded (or not)
// before it is squared.

const PARAMS = #{
    fold_re: #{ value: true, caption: "Fold the real part" },
    fold_im: #{ value: true, caption: "Fold the imaginary part" },
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout" },
};

const INFO = #{ title: "Burning Ship", center: complex(-0.5, -0.5), maxiter: 500 };

fn start(pixel, p) {
    #{ z: complex(0.0, 0.0), c: pixel }
}

fn step(s, p) {
    let x = if p.fold_re { abs(s.z.re) } else { s.z.re };
    let y = if p.fold_im { abs(s.z.im) } else { s.z.im };
    s.z = complex(x, y) ** 2 + s.c;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}
