// The built-in Collatz, the analytic Collatz map
// (2 + 7z - (2 + 5z) cos(pi z)) / 4, as a formula of your own (the Escape
// panel's Edit as Formula). The map has no c: the pixel is the start,
// on either plane. It escapes when |Im z| passes the bailout.

const PARAMS = #{
    bailout: #{ value: 10.0, min: 1.0, caption: "Bailout", hint: "The imaginary part's size past which a pixel has escaped" },
};

const INFO = #{ title: "Collatz", center: complex(1.0, 0.0), maxiter: 64 };

fn start(pixel, p) {
    #{ z: pixel }
}

fn step(s, p) {
    let z = s.z;
    s.z = 0.25 * (2.0 + 7.0 * z - (2.0 + 5.0 * z) * cos(PI() * z));
    s
}

fn escaped(s, p) {
    s.z.im.abs() > p.bailout
}
