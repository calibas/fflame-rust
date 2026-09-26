// The replay in offsets: a forced sample carried as `δ` from a reference
// orbit the CPU ran in f64, through each map's FORWARD difference form
// `F(z + δ) − F(z)` computed without forming the difference. The copy
// of `scene::forward_delta` the shader runs, gated against it
// (`the_shader_forward_forms_are_the_cpu_ones`). See
// docs/projects/deep-zoom-precision.md.
//
// A map is a ROW the CPU fills from the flame's analysis
// (`Backward::forward_rows`), CT_ROW floats per transform, read from
// `cylinders`:
//
//   0 kind (0 affine, 1 kernel, 2 kernel summed with an affine)
//   1 kernel (0 root, 1 spherical, 2 bubble, 3 hemisphere, 4 disc, 5 blob,
//     6 elliptic, 7 splits)
//   2..5 its parameters (root: n, d; blob: high, low, waves)
//   5 the kernel's weight
//   6..10 pre linear part, row-major     10..12 pre translation
//   12..16 the sum's linear part          16..20 post linear part
//   20..24 splits' steps: what crossing x = 0, then y = 0, adds
//
// Translations cancel in a difference, so only the pre's matters: it
// places the reference in the kernel's frame.

const CT_ROW: u32 = 24u;

// Below 0.1 `ln(1 + a)` and `exp(a) − 1` take their series, above it
// the plain functions: the `(1 + a) − 1` correction is exactly what
// Metal's fast-math may fold away. The CPU switches at 0.01 in f64; the
// shader switches later because a GPU's `log` and `exp` are good to an
// f32 ulp of their RESULT, which at 0.01 is 1e-5 of the answer -- measured
// as the roots' worst error in `the_shader_forward_forms_are_the_cpu_ones`
// until the series took over. Nine terms and eight: the next is under
// 1e-10 of the first at 0.1.
fn fd_ln1p(a: f32) -> f32 {
    if (abs(a) < 0.1) {
        return a * (1.0 - a * (0.5 - a * (0.33333334 - a * (0.25 - a * (0.2 - a * (0.16666667 - a * (0.14285715 - a * (0.125 - a * 0.11111111))))))));
    }
    return log(1.0 + a);
}

fn fd_expm1(a: f32) -> f32 {
    if (abs(a) < 0.1) {
        return a * (1.0 + a * (0.5 + a * (0.16666667 + a * (0.041666668 + a * (0.008333334 + a * (0.0013888889 + a * (0.0001984127 + a * 0.0000248016)))))));
    }
    return exp(a) - 1.0;
}

// sin(x) to the precision of x. A GPU's `sin` is accurate to an
// ABSOLUTE error near 1e-7 over a turn (NVIDIA's hardware instruction,
// Metal's fast-math), not a relative one: measured, sin(1e-12) came back
// a thousandth of itself. Every small angle a form takes the sine of
// goes through here; its series is exact to f32 below 0.25.
fn fd_sin(x: f32) -> f32 {
    if (abs(x) < 0.25) {
        let x2 = x * x;
        return x * (1.0 - x2 / 6.0 * (1.0 - x2 / 20.0 * (1.0 - x2 / 42.0 * (1.0 - x2 / 72.0))));
    }
    return sin(x);
}

// e^{a + ib} − 1, every part O(a, b).
fn fd_cexpm1(a: f32, b: f32) -> vec2<f32> {
    let em = fd_expm1(a);
    let h = fd_sin(0.5 * b);
    return vec2<f32>(em * cos(b) - 2.0 * h * h, (em + 1.0) * fd_sin(b));
}

// Whole turns between two angles' actual difference and a formed one:
// nonzero across an atan2 cut.
fn fd_turns(from_: f32, to_: f32, formed: f32) -> f32 {
    return round((to_ - from_ - formed) / 6.28318530717959);
}

// (sin, cos) of x + 2π·q, the whole turns dropped exactly and a half
// turn an exact sign: see `sin_cos_shifted` in forward_delta.rs.
fn fd_sin_cos_shifted(x: f32, q: f32) -> vec2<f32> {
    let frac = q - floor(q);
    if (frac == 0.0) {
        return vec2<f32>(fd_sin(x), cos(x));
    }
    if (frac == 0.5) {
        return vec2<f32>(-fd_sin(x), -cos(x));
    }
    let y = x + 6.28318530717959 * frac;
    return vec2<f32>(sin(y), cos(y));
}

fn fd_cmul(a: vec2<f32>, b: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(a.x * b.x - a.y * b.y, a.x * b.y + a.y * b.x);
}

// Far off every view: what a form answers at a pole, so the sample is
// dropped at the plot.
const FD_POLE: vec2<f32> = vec2<f32>(1.0e30, 1.0e30);

// `h(d, k) = d - k` for `d = sqrt(k^2 + y^2)`, without cancelling: see
// `ifs_analysis::elliptic_h`.
fn fd_eh(d: f32, k: f32, y2: f32) -> f32 {
    if (k >= 0.0) {
        let den = d + k;
        if (!(den > 0.0)) {
            return 0.0;
        }
        return y2 / den;
    }
    return d - k;
}

// Its difference by the rule chosen at `(d, k)`: see
// `forward_delta::elliptic_h_delta`. Past 1e30 at a focus, where the
// map is singular.
fn fd_eh_delta(d: f32, k: f32, y2: f32, dd: f32, dk: f32, dy2: f32, dw: f32, kw: f32) -> f32 {
    if (k >= 0.0) {
        let den = d + k;
        let den_w = dw + kw;
        if (!(den > 0.0) || !(den_w > 0.0)) {
            return 3.0e38;
        }
        return (dy2 * den - y2 * (dd + dk)) / (den_w * den);
    }
    return dd - dk;
}

// elliptic(v + e) - elliptic(v), every term O(e): see
// `forward_delta::elliptic_difference`. The output is (2/pi)(theta, sG):
// theta = atan2(x, C) with C^2 = (xmax - x)(xmax + x), G = ln(1 + m +
// sqrt(m)) with m = xmax - 1, s the sign of y -- each of m, xmax - x and
// xmax + x half a sum of two `fd_eh` terms, differenced by `fd_eh_delta`.
fn fd_elliptic(v: vec2<f32>, e: vec2<f32>) -> vec2<f32> {
    let w = v + e;
    let y2 = v.y * v.y;
    let dy2 = e.y * (v.y + v.y + e.y);
    let kp = v.x + 1.0;
    let km = 1.0 - v.x;
    let d1 = sqrt(kp * kp + y2);
    let d2 = sqrt(km * km + y2);
    let kpw = w.x + 1.0;
    let kmw = 1.0 - w.x;
    let y2w = w.y * w.y;
    let d1w = sqrt(kpw * kpw + y2w);
    let d2w = sqrt(kmw * kmw + y2w);
    let s1 = d1 + d1w;
    let s2 = d2 + d2w;
    if (!(s1 > 0.0) || !(s2 > 0.0)) {
        return FD_POLE;
    }
    let dd1 = (e.x * (kp + kp + e.x) + dy2) / s1;
    let dd2 = (-e.x * (km + km - e.x) + dy2) / s2;
    let a = fd_eh(d1, kp, y2);
    let b = fd_eh(d2, km, y2);
    let a_s = fd_eh(d1, -kp, y2);
    let b_s = fd_eh(d2, -km, y2);
    let da = fd_eh_delta(d1, kp, y2, dd1, e.x, dy2, d1w, kpw);
    let db = fd_eh_delta(d2, km, y2, dd2, -e.x, dy2, d2w, kmw);
    let da_s = fd_eh_delta(d1, -kp, y2, dd1, -e.x, dy2, d1w, -kpw);
    let db_s = fd_eh_delta(d2, -km, y2, dd2, e.x, dy2, d2w, -kmw);
    if (!(abs(da) <= 1e30) || !(abs(db) <= 1e30) || !(abs(da_s) <= 1e30) || !(abs(db_s) <= 1e30)) {
        return FD_POLE;
    }
    // x: theta = atan2(x, C), C = sqrt(P Q).
    let pp = 0.5 * (a + b_s);
    let qq = 0.5 * (a_s + b);
    let dp = 0.5 * (da + db_s);
    let dq = 0.5 * (da_s + db);
    let c = sqrt(pp * qq);
    let cw = sqrt(max(pp + dp, 0.0) * max(qq + dq, 0.0));
    var dc = 0.0;
    if (c + cw > 0.0) {
        dc = (dp * qq + pp * dq + dp * dq) / (c + cw);
    }
    let dtheta = ff_atan2(e.x * c - v.x * dc, v.x * w.x + c * cw);
    // y: s ln(1 + m + sqrt(m)). Across y = 0 the two sides' terms have
    // one sign: small across the segment between the foci, the jump
    // across the rays beyond them.
    let m = 0.5 * (a + b);
    let dm = 0.5 * (da + db);
    let mw = max(m + dm, 0.0);
    let s = sqrt(m);
    let sw = sqrt(mw);
    let neg_v = v.y < 0.0;
    let neg_w = w.y < 0.0;
    var dg: f32;
    if (neg_v == neg_w) {
        var ds = 0.0;
        if (s + sw > 0.0) {
            ds = dm / (s + sw);
        }
        let g = fd_ln1p((dm + ds) / (1.0 + m + s));
        dg = select(g, -g, neg_v);
    } else {
        let sum = fd_ln1p(m + s) + fd_ln1p(mw + sw);
        dg = select(-sum, sum, neg_v);
    }
    return 0.63661977236758 * vec2<f32>(dtheta, dg);
}

// K(v + e) − K(v), the kernel's forward difference along `arm`. `q` is
// the row's 20..24, splits' steps.
fn fd_kernel(k: u32, p: vec3<f32>, q: vec4<f32>, v: vec2<f32>, e: vec2<f32>, arm: u32) -> vec2<f32> {
    let w = v + e;
    let x = dot(v, v);
    let vd = dot(v, e);
    let t = vd + vd + dot(e, e);
    if (k == 0u) {
        // Root: K(v)·((1 + ε/v)^{(d/|n|, 1/n)} − 1).
        if (!(x > 0.0) || !(dot(w, w) > 0.0)) {
            return FD_POLE;
        }
        let n = p.x;
        let d = p.y;
        let u = vec2<f32>(dot(e, v), e.y * v.x - e.x * v.y) / x;
        let lnmod = 0.5 * fd_ln1p(u.x + u.x + dot(u, u));
        let argu = ff_atan2(u.y, 1.0 + u.x);
        let tv = ff_atan2(v.y, v.x);
        let tw = ff_atan2(w.y, w.x);
        let q = fd_turns(tv, tw, argu) / n;
        let a = lnmod * (d / abs(n));
        let b = argu / n;
        var ratio: vec2<f32>;
        if (q == floor(q)) {
            ratio = fd_cexpm1(a, b);
        } else {
            let sc = fd_sin_cos_shifted(b, q);
            let ea = exp(a);
            ratio = vec2<f32>(ea * sc.y - 1.0, ea * sc.x);
        }
        let rr = pow(sqrt(x), d / abs(n));
        let ang = (tv + 6.28318530717959 * f32(arm)) / n;
        return fd_cmul(rr * vec2<f32>(cos(ang), sin(ang)), ratio);
    }
    if (k == 1u || k == 2u) {
        // A·v/(|v|² + B): spherical (1, 1e-6), bubble (4, 4).
        let scale = select(4.0, 1.0, k == 1u);
        let bb = select(4.0, 1e-6, k == 1u);
        let xb = x + bb;
        let den = xb * (x + t + bb);
        if (!(abs(den) > 0.0)) {
            return FD_POLE;
        }
        return (e * xb - v * t) * (scale / den);
    }
    if (k == 3u) {
        // Hemisphere, v/s: ε/s' − v·t/(s·s'·(s + s')).
        let s = sqrt(x + 1.0);
        let s1 = sqrt(x + t + 1.0);
        return e / s1 - v * (t / (s * s1 * (s + s1)));
    }
    if (k == 6u) {
        return fd_elliptic(v, e);
    }
    if (k == 7u) {
        // Splits: e, plus the step between the quadrants of v and v + e
        // -- zero within one, the row's numbers across.
        let bv = vec2<f32>(select(0.0, 1.0, v.x >= 0.0), select(0.0, 1.0, v.y >= 0.0));
        let bw = vec2<f32>(select(0.0, 1.0, w.x >= 0.0), select(0.0, 1.0, w.y >= 0.0));
        let db = bw - bv;
        return e + db.x * q.xy + db.y * q.zw;
    }
    // Disc and blob measure θ from +y: the change in θ is minus the
    // angle from v to v + ε.
    if (!(x > 0.0)) {
        return FD_POLE;
    }
    let cross = v.x * e.y - v.y * e.x;
    let dformed = -ff_atan2(cross, x + vd);
    let tv = ff_atan2(v.x, v.y);
    let tw = ff_atan2(w.x, w.y);
    let m = fd_turns(tv, tw, dformed);
    let dtheta = dformed + 6.28318530717959 * m;
    if (k == 4u) {
        // Disc, (θ/π)(sin πr, cos πr).
        let r = sqrt(x);
        let r1 = sqrt(x + t);
        if (!(r + r1 > 0.0)) {
            return FD_POLE;
        }
        let dr = t / (r + r1);
        let pi = 3.14159265358979;
        let rm = pi * (r + 0.5 * dr);
        let h = fd_sin(0.5 * pi * dr);
        let dsin = 2.0 * cos(rm) * h;
        let dcos = -2.0 * sin(rm) * h;
        let s1 = vec2<f32>(sin(pi * r1), cos(pi * r1));
        return (dtheta / pi) * s1 + (tv / pi) * vec2<f32>(dsin, dcos);
    }
    // Blob, s(θ)·(v.y, v.x).
    let high = p.x;
    let low = p.y;
    let waves = p.z;
    let tw_eff = tv + dtheta;
    let s_w = low + 0.5 * (high - low) * (sin(waves * tw_eff) + 1.0);
    let half = 0.5 * waves;
    let sin_half = fd_sin_cos_shifted(half * dformed, waves * m * 0.5).x;
    let ds = (high - low) * cos(half * (tv + tw_eff)) * sin_half;
    return s_w * e.yx + ds * v.yx;
}

fn fd_lin(o: u32, d: vec2<f32>) -> vec2<f32> {
    return vec2<f32>(cylinders[o] * d.x + cylinders[o + 1u] * d.y, cylinders[o + 2u] * d.x + cylinders[o + 3u] * d.y);
}

// F(z + δ) − F(z) for the map in the row at `o`, along `arm`.
fn ct_fwd_diff(o: u32, z: vec2<f32>, d: vec2<f32>, arm: u32) -> vec2<f32> {
    let kind = u32(cylinders[o]);
    if (kind == 0u) {
        return fd_lin(o + 16u, d);
    }
    let v = fd_lin(o + 6u, z) + vec2<f32>(cylinders[o + 10u], cylinders[o + 11u]);
    let e = fd_lin(o + 6u, d);
    let kp = vec3<f32>(cylinders[o + 2u], cylinders[o + 3u], cylinders[o + 4u]);
    let kq = vec4<f32>(cylinders[o + 20u], cylinders[o + 21u], cylinders[o + 22u], cylinders[o + 23u]);
    let kd = fd_kernel(u32(cylinders[o + 1u]), kp, kq, v, e, arm);
    var mid = cylinders[o + 5u] * kd;
    if (kind == 2u) {
        mid = fd_lin(o + 12u, e) + mid;
    }
    return fd_lin(o + 16u, mid);
}

// **The final transforms, in offsets** (`scene::final_map`): a section
// of the table at `cylinders[7]` -- their count, then CT_FINAL_ROW floats
// a step, `[kind, pre linear (4), pre translation (2), w, shift, post
// linear (4)]`, kind 0 an affine (its linear part in the pre's place),
// 1 `bipolar`. `cylinders[7]` is 0 for a flame without.
const CT_FINAL_ROW: u32 = 13u;

// bipolar(v + e) - bipolar(v), without forming the difference: see
// `final_map::bipolar_diff`. With a = e/(v +- 1), the log term changes by
// (ln|1 + a+| - ln|1 + a-|)/pi and the angle term by
// (arg(1 + a-) - arg(1 + a+))/pi. Across the angle's wrap the point lands
// the whole strip away, far off a view deep enough for offsets: FD_POLE.
fn fd_bipolar(v: vec2<f32>, e: vec2<f32>, shift: f32) -> vec2<f32> {
    let pi = 3.14159265358979;
    let half_pi = 1.5707963267948966;
    let bp = vec2<f32>(v.x + 1.0, v.y);
    let bm = vec2<f32>(v.x - 1.0, v.y);
    let np = dot(bp, bp);
    let nm = dot(bm, bm);
    if (!(np > 0.0) || !(nm > 0.0)) {
        return FD_POLE;
    }
    let ap = vec2<f32>(dot(e, bp), e.y * bp.x - e.x * bp.y) / np;
    let am = vec2<f32>(dot(e, bm), e.y * bm.x - e.x * bm.y) / nm;
    let dx = 0.5 * (fd_ln1p(2.0 * ap.x + dot(ap, ap)) - fd_ln1p(2.0 * am.x + dot(am, am))) / pi;
    let dy = (ff_atan2(am.y, 1.0 + am.x) - ff_atan2(ap.y, 1.0 + ap.x)) / pi;
    // The reference's own output angle, as `bipolar` wraps it: where the
    // seam is.
    var y = 0.5 * ff_atan2(2.0 * v.y, dot(v, v) - 1.0) - half_pi * shift;
    if (y > half_pi) {
        y = -half_pi + (y + half_pi) % pi;
    } else if (y < -half_pi) {
        y = half_pi - (half_pi - y) % pi;
    }
    if (!(abs(2.0 / pi * y + dy) <= 1.0)) {
        return FD_POLE;
    }
    return vec2<f32>(dx, dy);
}

// F(z + d) - F(z) for the final step whose row is at `o`.
fn ct_final_diff(o: u32, z: vec2<f32>, d: vec2<f32>) -> vec2<f32> {
    let e = fd_lin(o + 1u, d);
    if (u32(cylinders[o]) == 0u) {
        return e;
    }
    let v = fd_lin(o + 1u, z) + vec2<f32>(cylinders[o + 5u], cylinders[o + 6u]);
    let k = fd_bipolar(v, e, cylinders[o + 8u]);
    if (k.x == FD_POLE.x) {
        return FD_POLE;
    }
    return fd_lin(o + 9u, cylinders[o + 7u] * k);
}

// **The replay's last steps, in offsets.** `p` is the sample after the
// first `m` symbols of the word whose record is at `b`, in absolute f32,
// where f32 still resolves it; `blk` is the word's block of references
// (`scene::cylinder::pack_words`). It takes the nearest reference,
// carries its offset through the remaining symbols' forward difference
// forms, and comes out RELATIVE to the view centre: the reference's end,
// less the plan's centre, was formed in f64, and the table's `shift`
// moves the plan's centre to the view's. No big number meets a small
// one on the GPU. A flame's final transforms are its last offset
// steps, each chain holding the reference before each (`scene::final_map`).
fn ct_offsets(p: vec2<f32>, b: u32, blk: u32, m: u32, len: u32) -> vec2<f32> {
    let fin = u32(cylinders[7]);
    let finals = select(0u, min(u32(cylinders[fin]), 4u), fin != 0u);
    let per = 2u * (len - m) + 2u * finals + 2u;
    // `backward::MAX_CHAINS` is 4; bounded, like the word's length, so a
    // mismatched table cannot loop the GPU for long.
    let chains = min(u32(cylinders[blk + 1u]), 16u);
    var best = blk + 2u;
    var near = 3.0e38;
    for (var j = 0u; j < chains; j = j + 1u) {
        let o = blk + 2u + j * per;
        let g = p - vec2<f32>(cylinders[o], cylinders[o + 1u]);
        let dd = dot(g, g);
        if (dd < near) {
            near = dd;
            best = o;
        }
    }
    return ct_offsets_from(b, best, p - vec2<f32>(cylinders[best], cylinders[best + 1u]), m, len);
}

// The same from chain `j` of the word's block, with the offset `d` from
// its base: a conditional draw's sample, formed from its piece's centre
// (`ct_conditional`), whose chain is its piece's.
fn ct_offsets_chain(b: u32, blk: u32, j: u32, d: vec2<f32>, m: u32, len: u32) -> vec2<f32> {
    let fin = u32(cylinders[7]);
    let finals = select(0u, min(u32(cylinders[fin]), 4u), fin != 0u);
    let per = 2u * (len - m) + 2u * finals + 2u;
    let chains = min(u32(cylinders[blk + 1u]), 16u);
    return ct_offsets_from(b, blk + 2u + min(j, chains - 1u) * per, d, m, len);
}

// Chain `best`'s offset steps from `d0`: the word's, then the finals'.
fn ct_offsets_from(b: u32, best: u32, d0: vec2<f32>, m: u32, len: u32) -> vec2<f32> {
    let fin = u32(cylinders[7]);
    let finals = select(0u, min(u32(cylinders[fin]), 4u), fin != 0u);
    var d = d0;
    let rows = u32(cylinders[2]);
    for (var k = m; k < len; k = k + 1u) {
        let at = best + 2u * (k - m);
        let sym = u32(cylinders[b + 4u + k]);
        d = ct_fwd_diff(rows + (sym & 255u) * CT_ROW, vec2<f32>(cylinders[at], cylinders[at + 1u]), d, sym >> 8u);
    }
    for (var j = 0u; j < finals; j = j + 1u) {
        let at = best + 2u * (len - m) + 2u * j;
        d = ct_final_diff(fin + 1u + j * CT_FINAL_ROW, vec2<f32>(cylinders[at], cylinders[at + 1u]), d);
    }
    let end = best + 2u * (len - m) + 2u * finals;
    return vec2<f32>(cylinders[end], cylinders[end + 1u]) + d + vec2<f32>(cylinders[4], cylinders[5]);
}
