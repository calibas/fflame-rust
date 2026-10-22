// The built-in Origami (Butterfly), McCabe's folded paper after
// McDonald's `foldPoint`, as a formula of your own (the Escape panel's
// Edit as Formula). Each pass folds the point once, about one of 64
// lines built once per image (`prepare`): line j's endpoints are two
// seeded random points folded through the lines before it, so every
// crease lands on the current wad. Nothing escapes; the paper's creases
// draw the Creases relief and terrain (`creases`).

const PARAMS = #{
    seed: #{ value: 7.0, min: 0.0, max: 512.0, caption: "Line seed", hint: "Chooses the random fold arrangement. Every value is a different folding; scrub it to explore the family." },
    spread: #{ value: 2.0, min: 0.05, max: 4.0, caption: "Line spread", hint: "Half-size of the square the fold endpoints are drawn from, before being folded onto the wad. 2.0 matches the default view." },
};

const INFO = #{ title: "Origami (Butterfly)", center: complex(0.0, 0.0), maxiter: 32 };

// The built-in's integer hash, on 32-bit patterns.
fn hash(x) {
    let h = x;
    h = h ^ (h >> 16);
    h = h * 0x7feb352d;
    h = h ^ (h >> 15);
    h = h * 0x846ca68b;
    h ^ (h >> 16)
}

// A hash's high 24 bits as a number in [0, 1).
fn unit(x) {
    to_float(hash(x) >> 8) * (1.0 / 16777216.0)
}

// The seed's stream: its whole part, clamped as the built-in's, by
// Knuth's constant.
fn stream(seed) {
    let s = if seed < 0.0 { 0.0 } else if seed > 4096.0 { 4096.0 } else { seed };
    to_int(s) * 2654435761
}

// The fold: q reflected about the line through a and b, but only from
// the negative side; a degenerate segment folds nothing.
fn fold(q, a, b) {
    let ab = b - a;
    let d2 = ab.norm_sqr();
    if ab.re * (q.im - a.im) - ab.im * (q.re - a.re) > 0.0 || d2 < 1e-12 {
        q
    } else {
        let t = ((q.re - a.re) * ab.re + (q.im - a.im) * ab.im) / d2;
        (a + ab * t) * 2.0 - q
    }
}

// The same fold in f32, an operation at a time: the line table is the
// built-in's, built on the CPU in f32, and each line's endpoints fold
// through every earlier line, so a last-place difference would move
// later lines whole.
fn fold32(q, a, b) {
    let abx = to_f32(b.re - a.re);
    let aby = to_f32(b.im - a.im);
    let d2 = to_f32(to_f32(abx * abx) + to_f32(aby * aby));
    let side = to_f32(to_f32(abx * to_f32(q.im - a.im)) - to_f32(aby * to_f32(q.re - a.re)));
    if side > 0.0 || d2 < to_f32(1e-12) {
        q
    } else {
        let t = to_f32(to_f32(to_f32(to_f32(q.re - a.re) * abx) + to_f32(to_f32(q.im - a.im) * aby)) / d2);
        complex(to_f32(to_f32(a.re + to_f32(abx * t)) * 2.0 - q.re), to_f32(to_f32(a.im + to_f32(aby * t)) * 2.0 - q.im))
    }
}

// An endpoint's coordinate: a hashed number in [-1, 1) times the spread.
fn coord(x, spread) {
    to_f32((unit(x) * 2.0 - 1.0) * spread)
}

fn prepare(p) {
    let s = stream(p.seed);
    let lines = array(256, 0.0);
    for j in 0..64 {
        let a = complex(coord(j * 4 + s, p.spread), coord(j * 4 + 1 + s, p.spread));
        let b = complex(coord(j * 4 + 2 + s, p.spread), coord(j * 4 + 3 + s, p.spread));
        for k in 0..j {
            let la = complex(lines[k * 4], lines[k * 4 + 1]);
            let lb = complex(lines[k * 4 + 2], lines[k * 4 + 3]);
            a = fold32(a, la, lb);
            b = fold32(b, la, lb);
        }
        lines[j * 4] = a.re;
        lines[j * 4 + 1] = a.im;
        lines[j * 4 + 2] = b.re;
        lines[j * 4 + 3] = b.im;
    }
    #{ lines: lines }
}

fn start(pixel, p) {
    #{ z: pixel }
}

// One fold a pass; past the 64th the point stays.
fn step(s, p, ctx) {
    if ctx.pass < 64 {
        let k = ctx.pass * 4;
        s.z = fold(s.z, complex(p.lines[k], p.lines[k + 1]), complex(p.lines[k + 2], p.lines[k + 3]));
    }
    s
}

// The paper's creases: fold i a crease weighing decay^i, the paper
// rising (or falling) in a straight line with the point's distance from
// it, measured in the wad, the same slope both sides; every image of a
// crease keeps its fold's way. The folded point is m p0 + t for an
// orthogonal m (its columns c0, c1), so the gradient of a distance from
// a line is m^T times its normal.
fn creases(p0, p, ctx) {
    let reach = ctx.crease_reach * p.spread;
    let s = stream(p.seed);
    let folds = if ctx.maxiter < 64 { ctx.maxiter } else { 64 };
    let q = p0;
    let c0 = complex(1.0, 0.0);
    let c1 = complex(0.0, 1.0);
    let weight = 1.0;
    let h = 0.0;
    let grad = complex(0.0, 0.0);
    for i in 0..folds {
        let a = complex(p.lines[i * 4], p.lines[i * 4 + 1]);
        let b = complex(p.lines[i * 4 + 2], p.lines[i * 4 + 3]);
        let ab = b - a;
        let d2 = ab.norm_sqr();
        let side = ab.re * (q.im - a.im) - ab.im * (q.re - a.re);
        if d2 >= 1e-12 {
            let dir = 1.0;
            if ctx.crease_pattern == 1 && (i & 1) == 1 {
                dir = -1.0;
            }
            if ctx.crease_pattern == 2 && (hash(s + 256 + i) >> 31) == 1 {
                dir = -1.0;
            }
            let len = sqrt(d2);
            let nx = -ab.im / len;
            let ny = ab.re / len;
            let dist = side / len;
            let w = weight * dir * ctx.crease_sign;
            if reach > 0.0 && abs(dist) >= reach {
                h += w * reach;
            } else {
                h += w * abs(dist);
                let sg = if dist > 0.0 { 1.0 } else if dist < 0.0 { -1.0 } else { 0.0 };
                grad += (w * sg) * complex(nx * c0.re + ny * c0.im, nx * c1.re + ny * c1.im);
            }
            if side <= 0.0 {
                q = fold(q, a, b);
                let ux = ab.re / len;
                let uy = ab.im / len;
                let r00 = 2.0 * ux * ux - 1.0;
                let r01 = 2.0 * ux * uy;
                let r11 = 2.0 * uy * uy - 1.0;
                let n0 = complex(r00 * c0.re + r01 * c0.im, r01 * c0.re + r11 * c0.im);
                let n1 = complex(r00 * c1.re + r01 * c1.im, r01 * c1.re + r11 * c1.im);
                c0 = n0;
                c1 = n1;
            }
        }
        weight *= ctx.crease_decay;
    }
    #{ height: h, slope: grad }
}
