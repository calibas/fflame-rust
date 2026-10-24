// The Bristorbrot: the Mandelbrot and Julia sets of Doug Bristor's
// numbers ("Bristorbrot / Doug Bristor, bristorbrot.org"; his viewer's
// code, which this follows, is MIT). Use it from Escape mode ("Use as
// solid").
//
// The numbers have a real part and ranked imaginary units e1, e2, e3
// (i, j, k), and multiply by one rule: e_a e_a = -1, and e_a e_b =
// sgn(b - a) e_b for a != b -- the product is a signed copy of its
// right-hand factor, so i j = j and j i = -i. Not quaternions (no i j =
// k); not associative either, so the two cubes (zz)z and z(zz) differ.
// Its square in the three parts (real, i, j) is the 3D Bristorbrot Jos
// Leys named in 2009: (x^2 - y^2 - z^2, y (2x - z), z (2x + y)).
//
// The iteration is four-dimensional; the solid is a 3D section of it,
// the window (real, i, cos a j + sin a k) at the Section Roll a, set
// Section Offset along the axis it hides. Roll and offset 0 is the 3D
// Bristorbrot. The Mandelbrot set iterates z = f(z) + c from z = c, the
// point; the Julia set from z = the point, with c the constant.
//
// The distance, 0.5 |z| ln|z| / |dz|, two ways (Distance). Bristor's,
// as his viewer reads it: |dz| the running derivative of a conformal
// set, n |z|^(n-1) a step. But the product stretches space by up to
// 3.3 |z| (the cubes 6.7 and 7.2 |z|^2), so the reading is long -- by
// the stretches' ratio compounded over the orbit -- and the Julia sets,
// dust finer than a pixel almost everywhere, show their core as thin
// ribbons, as his pictures do. The Jacobian: the orbit carries its
// whole derivative along each of the window's three axes, and |dz| is
// its Frobenius norm, never less than the stretch, so the dust shows
// grown by a pixel, fuller. Either is capped at a tenth of the ball: a
// Julia orbit through 0, where the map's derivative is none, reads any
// distance, and uncapped Bristor's reading carried rays through the
// Julia sets (one in thirteen for the square, 13 of 33 for the cubes);
// capped, neither did. Both are rescaled with the weight of the c term past
// 1e10, as Bristor's march is, so a deep orbit stays in f32.
//
// It shows a product of your own over a `quat` -- four parts, scalar
// first -- written out part by part, since `*` between quaternions is
// Hamilton's.

const PARAMS = #{
    formula: #{ value: 0, caption: "Formula", choices: ["z^2 + c", "(zz)z + c", "z(zz) + c"], hint: "The square, or one of the two cubes, which differ: the product is not associative" },
    julia: #{ value: 0, caption: "Variant", choices: ["Mandelbrot", "Julia"], hint: "The Mandelbrot set (c the point) or a Julia set (c the constant below)" },
    c: #{ value: quat(-0.7, -0.35, 0.05, 0.0), caption: "Julia C", hint: "The Julia set's constant: real, i, j, k. Bristor's for the square is (-0.7, -0.35, 0.05, 0), for the cubes (0.15, 0.4, 0.05, 0)" },
    roll: #{ value: 0.0, min: -180.0, max: 180.0, caption: "Section Roll", hint: "Degrees: the 3D window into the 4D iteration turns from j toward k. 0 is the 3D Bristorbrot" },
    w: #{ value: 0.0, min: -2.0, max: 2.0, caption: "Section Offset", hint: "How far the window sits along the axis it hides" },
    estimate: #{ value: 0, caption: "Distance", choices: ["Bristor's", "Jacobian"], hint: "Bristor's: his viewer's running derivative, which reads long, so the Julia sets thin to ribbons as his pictures show them. Jacobian: the orbit's whole stretch, so the dust shows grown by a pixel, fuller -- and three derivatives a step to carry" },
};

const INFO = #{ title: "Bristorbrot (Rhai)", radius: 2.5 };

// The product: e_a e_a = -1, e_a e_b = sgn(b - a) e_b. Parts (real, i,
// j, k) as the quaternion's (w, x, y, z).
fn bmul(a, b) {
    quat(a.w * b.w - a.x * b.x - a.y * b.y - a.z * b.z,
         a.w * b.x + a.x * b.w - b.x * (a.y + a.z),
         a.w * b.y + a.y * b.w + b.y * (a.x - a.z),
         a.w * b.z + a.z * b.w + b.z * (a.x + a.y))
}

// The map, without c.
fn bmap(z, formula) {
    if formula == 0 {
        bmul(z, z)
    } else if formula == 1 {
        bmul(bmul(z, z), z)
    } else {
        bmul(z, bmul(z, z))
    }
}

// The map's derivative along `v`: the product rule, which needs no
// associativity -- the square's z v + v z, and through it the cubes'.
fn bmap_along(z, v, formula) {
    let sq = bmul(z, v) + bmul(v, z);
    if formula == 0 {
        sq
    } else if formula == 1 {
        bmul(sq, z) + bmul(bmul(z, z), v)
    } else {
        bmul(v, bmul(z, z)) + bmul(z, sq)
    }
}

fn start(pos, p) {
    let a = p.roll * 0.017453292519943295;
    let cr = cos(a);
    let sr = sin(a);
    let z = quat(pos.x, pos.y, cr * pos.z - sr * p.w, sr * pos.z + cr * p.w);
    // The c term's share: the Mandelbrot set's c is the point.
    let k = if p.julia == 0 { 1.0 } else { 0.0 };
    // The derivative along the window's axes starts as the axes.
    #{ z: z, c: z * k + p.c * (1.0 - k), k: k, ez: quat(0.0, 0.0, cr, sr),
       m1: quat(1.0, 0.0, 0.0, 0.0), m2: quat(0.0, 1.0, 0.0, 0.0), m3: quat(0.0, 0.0, cr, sr), wt: 1.0, dr: 1.0,
       sd: 1.0, sw: 1.0 }
}

fn step(s, p) {
    let z = s.z;
    if p.estimate == 1 {
        let g = s.k * s.wt;
        s.m1 = bmap_along(z, s.m1, p.formula) + quat(g, 0.0, 0.0, 0.0);
        s.m2 = bmap_along(z, s.m2, p.formula) + quat(0.0, g, 0.0, 0.0);
        s.m3 = bmap_along(z, s.m3, p.formula) + s.ez * g;
    }
    // Bristor's running derivative, n |z|^(n-1) a step.
    let n = if p.formula == 0 { 2.0 } else { 3.0 };
    s.sd = n * length(z) ** (n - 1.0) * s.sd + s.k * s.sw;
    if s.sd > 1.0e10 {
        s.sw = s.sw / s.sd;
        s.sd = 1.0;
    }
    s.z = bmap(z, p.formula) + s.c;
    let big = max(length(s.m1), max(length(s.m2), length(s.m3)));
    if big > 1.0e10 {
        s.m1 = s.m1 / big;
        s.m2 = s.m2 / big;
        s.m3 = s.m3 / big;
        s.wt = s.wt / big;
    }
    // For the Escape Level colouring: how far the orbit has stretched.
    s.dr = s.sd / max(s.sw, 1.0e-30);
    s
}

fn escaped(s, p) {
    length(s.z) > 1.0e4
}

fn distance(s, p) {
    let r = length(s.z);
    if r > 1.0e4 {
        let j = sqrt(dot(s.m1, s.m1) + dot(s.m2, s.m2) + dot(s.m3, s.m3));
        let d = if p.estimate == 1 { s.wt / max(j, 1.0e-30) } else { s.sw / max(s.sd, 1.0e-30) };
        min(0.5 * r * ln(r) * d, 0.25)
    } else {
        0.0
    }
}
