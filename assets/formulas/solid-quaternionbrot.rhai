// The Quaternionbrot: the Mandelbrot and Julia sets of q^n + c over
// Hamilton's quaternions, as Doug Bristor's viewer sets them beside his
// Bristorbrot (bristorbrot.org; solid-bristorbrot.rhai). Use it from
// Escape mode ("Use as solid").
//
// The iteration is four-dimensional; the solid is a 3D section of it,
// the window (real, i, cos a j + sin a k) at the Section Roll a, set
// Section Offset along the axis it hides. Every turn of i, j and k
// about the real axis is a symmetry of the quaternions, so at offset 0
// the Mandelbrot set is the plane's, turned about its real axis, and the
// roll changes nothing; the offset is what cuts it otherwise. The
// Mandelbrot set iterates q = q^n + c from q = c, the point; the Julia
// set from q = the point, with c the constant.
//
// The distance is the classic one, 0.5 |q| ln|q| / dr: unlike the
// Bristorian product, q^n stretches space by exactly n |q|^(n-1) at
// most, so the running derivative bounds the orbit's (rescaled with the
// weight of the c term past 1e10, so a deep orbit stays in f32). A
// Julia orbit through 0, where q^n's derivative is none, reads any
// distance, so a reading is at most a tenth of the ball.
//
// It shows `quat` arithmetic: `*` Hamilton's product, `pow(q, n)` the
// complex power of t + i r carried along the vector part (for a whole n
// the product n times over).

const PARAMS = #{
    power: #{ value: 2.0, min: 2.0, max: 8.0, caption: "Power", hint: "The n of q^n + c: Bristor's are 2 and 3" },
    julia: #{ value: 0, caption: "Variant", choices: ["Mandelbrot", "Julia"], hint: "The Mandelbrot set (c the point) or a Julia set (c the constant below)" },
    c: #{ value: quat(-0.2, 0.65, 0.0, 0.0), caption: "Julia C", hint: "The Julia set's constant, scalar part first. Bristor's for q^2 + c is (-0.2, 0.65, 0, 0), for q^3 + c (0.35, 0.62, 0, 0)" },
    roll: #{ value: 0.0, min: -180.0, max: 180.0, caption: "Section Roll", hint: "Degrees: the 3D window into the 4D iteration turns from j toward k" },
    w: #{ value: 0.0, min: -2.0, max: 2.0, caption: "Section Offset", hint: "How far the window sits along the axis it hides: 0 is the plane's set turned about its real axis" },
};

const INFO = #{ title: "Quaternionbrot (Rhai)", radius: 2.5 };

fn start(pos, p) {
    let a = p.roll * 0.017453292519943295;
    let cr = cos(a);
    let sr = sin(a);
    let q = quat(pos.x, pos.y, cr * pos.z - sr * p.w, sr * pos.z + cr * p.w);
    // The c term's share: the Mandelbrot set's c is the point.
    let k = if p.julia == 0 { 1.0 } else { 0.0 };
    #{ z: q, c: q * k + p.c * (1.0 - k), k: k, dr: 1.0, wt: 1.0 }
}

fn step(s, p) {
    let r = length(s.z);
    s.dr = p.power * r ** (p.power - 1.0) * s.dr + s.k * s.wt;
    if s.dr > 1.0e10 {
        s.wt = s.wt / s.dr;
        s.dr = 1.0;
    }
    // The square as the product, exact; any other power by `pow`.
    let f = if p.power == 2.0 { s.z * s.z } else { pow(s.z, p.power) };
    s.z = f + s.c;
    s
}

fn escaped(s, p) {
    length(s.z) > 1.0e4
}

fn distance(s, p) {
    let r = length(s.z);
    if r > 1.0e4 {
        min(0.5 * r * ln(r) * s.wt / max(s.dr, 1.0e-30), 0.25)
    } else {
        0.0
    }
}
