// Frothy Basin, Fractint's type (Wesley Loewer's, after the riddled
// basins James Alexander drew): z^2 + conj((-1 + a i) z), a map with
// three attractors -- each settling on one of three lines, y = a/2 and
// its turns by 120 degrees -- whose basins froth into one another.
// Ported from Xfractint 20.04p16 (common/miscfrac.c: froth_setup and
// calcfroth, the floating-point path; the defaults, the view and the
// bailout from common/fractalp.c).
//
// A pixel stops when its orbit lands within 1e-6 of an attractor's line
// (`froth_landed`, Fractint's test), or leaves the bailout's circle.
// The test reads the iterate alone -- nothing carried but z -- so deep
// zoom derives its equations. The Frothy Basin colouring
// (`frothy-basin-coloring.rhai`) colours the basins as Fractint does;
// its `a` and Mapping are this formula's, to set alike.

const PARAMS = #{
    a: #{ value: 1.028713768218725, caption: "a", hint: "The map's parameter. Up to 1.0287137682 (Fractint's critical value) there are three basins; past it, two" },
    mapping: #{ value: 0, caption: "Mapping", choices: ["Once (3 basins)", "Twice (6 basins)"], hint: "Map once a pass, or twice: twice splits each basin in two (Fractint's first parameter, 1 or 2)" },
    bailout: #{ value: 7.0, min: 7.0, caption: "Bailout", hint: "The squared radius past which a pixel has left (at least 7, as Fractint keeps it)" },
};

const INFO = #{ title: "Frothy Basin", center: complex(0.0, -0.255), magn: 0.9523809523809523, maxiter: 150 };

fn start(pixel, p, ctx) {
    #{ z: pixel }
}

// Whether `z` has landed on an attractor: Fractint's test, which sets
// marks along the top line by the line's own map -- x^2 - x - 3a^2/4,
// from its minimum at x = 0.5 -- and turns them 120 degrees either way
// for the other two lines. Written as one comparison, for `escaped`: a
// point on a line counts unless it lies between the third and fourth
// marks, which only a three-basin picture (a at most the critical value)
// keeps waiting. Which attractor it is, the colouring tells.
fn froth_landed(z, a) {
    let close = 1.0e-6;
    let slope = sqrt(3.0);
    // Fractint's critical a, 1.0287137682187249: a parameter is kept in
    // f32, which rounds that value itself just past it, so the test
    // allows the rounding -- at the default there are three basins.
    let three = a.abs() <= 1.02871388;
    let k = 0.75 * a * a;
    let t1 = 0.25 - 0.5 - k;
    let t2 = t1 * t1 - t1 - k;
    let t3 = t2 * t2 - t2 - k;
    let t4 = t3 * t3 - t3 - k;
    let y0 = a / 2.0;
    let s = slope / 2.0;
    let x = z.re;
    let y = z.im;
    let top = (y0 - y).abs() < close && x >= t1 && x <= t2;
    let right = (slope * x - a - y).abs() < close && x <= -0.5 * t1 + y0 * s && x >= -0.5 * t2 + y0 * s;
    let left = (-slope * x - a - y).abs() < close && x <= -0.5 * t1 - y0 * s && x >= -0.5 * t2 - y0 * s;
    (top && (!three || x <= t3 || x >= t4))
        || (!top && right && (!three || x >= -0.5 * t3 + y0 * s || x <= -0.5 * t4 + y0 * s))
        || (!top && !right && left && (!three || x >= -0.5 * t3 - y0 * s || x <= -0.5 * t4 - y0 * s))
}

// One map, z^2 + conj((-1 + a i) z), in Fractint's arithmetic and its
// order: the orbits are chaotic, and the order of a rounding moves a
// count.
fn froth_map(z, a) {
    let x = z.re;
    let y = z.im;
    complex(x * x - y * y - x - a * y, y + ((x + x) * y - a * x))
}

fn step(s, p) {
    s.z = froth_map(s.z, p.a);
    if p.mapping == 1 {
        s.z = froth_map(s.z, p.a);
    }
    s
}

fn escaped(s, p) {
    froth_landed(s.z, p.a) || s.z.norm_sqr() > p.bailout
}
