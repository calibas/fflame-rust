// Halayka's quaternion sets: the quaternion Julia set with its step
// replaced (S. Halayka, "Some visually interesting non-standard quaternion
// fractal sets", Chaos, Solitons and Fractals 41, 2009). Use it from Escape
// mode ("Use as solid").
//
// The point (x, y, z) is the quaternion Z = (x, y, z, w), scalar part
// first, w the slice; it is stepped a few times (Iterations, 8 in the
// paper) and is in the set while |Z| stays within the threshold (4). So
// these are the shapes of a few iterations, smooth, not the limit fractal:
// raise Iterations to sharpen them.
//
// It shows quaternions in Rhai: `quat(w, x, y, z)`, `*` Hamilton's
// product, `inv`, and `sin`, `cos`, `exp`, `sinh`, `cosh` -- each the
// complex function of `t + i r` (t the scalar part, r the vector part's
// length) carried back along the vector part, Halayka's equations
// (17)-(21). `z.complex()` is that `t + i r`.
//
// The distance: each step stretches space by at most |1 + C| (or |C|)
// times the larger of |g'| along the plane and |Im g| / r across it, so
// `dr` bounds how far the orbit has been stretched, and a point whose
// orbit passes the threshold at step k is at least (|Z_k| - 4) / dr_k
// from the set: a first-order reading, which holds near the surface, so
// no step outside is longer than a tenth of the box. Inside there is no
// such bound -- an orbit that settles shrinks `dr`, and the reading
// (4 - |Z_k|) / dr_k says units where the surface is a tenth away -- so the
// complement and the surface (Halayka's figures 6 and 8) are marched
// through the set in steps of a fortieth of the box, the reading taking
// over within one: a feature thinner than a step may be passed, as the
// paper's lattice passes what is finer than its spacing, and the surface
// is drawn a step thick on its escaped side, so a step from inside stops
// in it. All are cut to the paper's box.
//
// Draw: the set; the complement, the box less the set; or the surface
// between them, which is what the paper draws -- an isosurface of the
// lattice has no faces where the set meets the lattice's edge, so it is
// open at the box, and both of its sides show.

const PARAMS = #{
    step: #{ value: 1, caption: "Step", choices: ["Z^n + C", "sin Z + C sin Z", "cos Z + C cos Z", "exp(Z^2) + C", "inv(sinh Z) + C inv(sinh Z)", "C inv(sinh Z) cosh Z"], hint: "Which of Halayka's steps: figures 1-2, 3-4, 5, 6, 7 and 8" },
    c: #{ value: quat(0.3, 0.5, 0.4, 0.2), caption: "C", hint: "The constant, scalar part first: Halayka's (0.3, 0.5, 0.4, 0.2)" },
    power: #{ value: 2.0, min: 2.0, max: 8.0, caption: "Power", hint: "The n of Z^n + C" },
    threshold: #{ value: 4.0, min: 1.0, max: 16.0, caption: "Threshold", hint: "A point whose |Z| passes it is outside the set (Halayka's 4)" },
    w: #{ value: 0.0, min: -2.0, max: 2.0, caption: "Slice", hint: "The quaternion's last part, which the 3D slice pins" },
    draw: #{ value: 0, caption: "Draw", choices: ["The set", "The complement", "The surface"], hint: "The set, cut to the box; the box less the set, the points that escape (figures 6 and 8); or the surface between them, open at the box and seen from either side, as the paper's crude surface draws them" },
    box: #{ value: 1.5, min: 0.1, max: 8.0, caption: "Box", hint: "The half-width of the box the shape is cut to (Halayka's lattice, -1.5 to 1.5)" },
};

// The box's corner: 1.5 times the square root of 3.
const INFO = #{ title: "Halayka's Quaternions (Rhai)", radius: 2.6 };

// How far a function of the plane stretches: along the plane by its
// derivative `gd`, across it by its imaginary part over the vector part's
// length.
fn stretch(gd, g, r) {
    max(gd.abs(), g.im.abs() / (r + 1e-30))
}

// The distance to a box of half-width `b` about the origin, negative
// inside.
fn box_distance(v, b) {
    let q = abs(v) - vec3(b);
    length(max(q, 0.0)) + min(max(q.x, max(q.y, q.z)), 0.0)
}

fn start(pos, p) {
    #{ z: quat(pos.x, pos.y, pos.z, p.w), at: pos, dr: 1.0, near: 1.0e30, out: false, d: 0.0 }
}

fn step(s, p) {
    let z = s.z;
    let zc = z.complex();
    let r = zc.im;
    let k = 0.0;
    if p.step == 0 {
        k = p.power * length(z) ** (p.power - 1.0);
        s.z = pow(z, p.power) + p.c;
    } else if p.step == 1 {
        k = length(1.0 + p.c) * stretch(cos(zc), sin(zc), r);
        s.z = sin(z) + p.c * sin(z);
    } else if p.step == 2 {
        k = length(1.0 + p.c) * stretch(-sin(zc), cos(zc), r);
        s.z = cos(z) + p.c * cos(z);
    } else if p.step == 3 {
        let e = exp(zc * zc);
        k = stretch(2.0 * zc * e, e, r);
        s.z = exp(z * z) + p.c;
    } else if p.step == 4 {
        let sh = sinh(zc);
        k = length(1.0 + p.c) * stretch(cosh(zc) / (sh * sh), 1.0 / sh, r);
        s.z = inv(sinh(z)) + p.c * inv(sinh(z));
    } else {
        let sh = sinh(zc);
        k = length(p.c) * stretch(1.0 / (sh * sh), cosh(zc) / sh, r);
        s.z = p.c * (inv(sinh(z)) * cosh(z));
    }
    s.dr = s.dr * k;
    let m = length(s.z);
    // A critical point (exp(Z^2)'s origin) stretches by nothing: the
    // derivative's floor keeps the estimate a number.
    let dr = max(s.dr, 1e-30);
    // Written so a point the step has no number for -- inv(sinh Z) at
    // Z = 0 -- is outside, as the paper's lattice has it.
    if !(m <= p.threshold) {
        s.out = true;
        let d = (m - p.threshold) / dr;
        s.d = if d >= 0.0 { d } else { 1.0e30 };
    } else {
        s.near = min(s.near, (p.threshold - m) / dr);
    }
    s
}

// The orbit stops where it first leaves.
fn escaped(s, p) {
    s.out
}

fn distance(s, p) {
    // Half: the stretch is the steps' largest, the distance a first-order
    // reading of it, which holds near the surface; farther, and past a
    // critical point, no step is longer than a tenth of the box.
    let step = 0.025 * p.box;
    let outside = if s.out { min(0.5 * s.d, 0.1 * p.box) } else { 0.0 };
    let inside = if s.out { 0.0 } else { min(0.25 * s.near, step) };
    // The surface: a shell a step thick on the escaped side, which a step
    // from inside the set cannot cross.
    let set = if p.draw == 0 {
        outside
    } else if p.draw == 1 {
        inside
    } else if s.out {
        max(outside - step, 0.0)
    } else {
        inside
    };
    max(set, box_distance(s.at, p.box))
}
