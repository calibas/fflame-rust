//! **The forward maps' difference forms**: `F(z + δ) − F(z)` computed
//! without forming the difference, so a sample carried as an offset `δ`
//! from a reference orbit keeps every digit f32 gives it, however deep
//! the view (`docs/projects/deep-zoom-precision.md` §3).
//!
//! The inverse direction's forms, which the escape engine's delta walk
//! uses, are `kernel_difference_gen` in `ifs_analysis.rs`. These go the
//! other way: the render replays a word FORWARD, and a root's forward
//! map is a root, not the power its inverse is. One body, generic over
//! [`Transcendental`], so f64 and `BigFloat` run the same arithmetic;
//! the shader's copy is gated against it.
//!
//! Every term here is O(δ). `ln(1 + a)` and `exp(a) − 1` take series
//! below 0.01 and the plain functions above, rather than the usual
//! `(1 + a) − 1` correction, because that correction is what Metal's
//! fast-math may fold away, and the WGSL copy must be the same rule.

use super::ifs_analysis::{blob_scale_gen, Kernel, Map2};
use super::ifs_real::{cmul, cnorm2, Real, Transcendental};

/// Below this, `ln1p` and `expm1` sum their series.
const SERIES_BELOW: f64 = 0.01;

/// Terms of the series: the next is under 1e-34 of the first at 0.01.
const SERIES_TERMS: u32 = 16;

/// `ln(1 + a)`, to the precision of `a`.
fn ln1p<T: Transcendental>(a: &T) -> T {
    if a.to_f64().abs() < SERIES_BELOW {
        let mut power = a.clone();
        let mut sum = a.zero();
        for k in 1..=SERIES_TERMS {
            let t = power.div(&a.lit(k as f64));
            sum = if k % 2 == 1 { sum.add(&t) } else { sum.sub(&t) };
            power = power.mul(a);
        }
        sum
    } else {
        a.add(&a.one()).ln()
    }
}

/// `exp(a) − 1`, to the precision of `a`.
fn expm1<T: Transcendental>(a: &T) -> T {
    if a.to_f64().abs() < SERIES_BELOW {
        let mut term = a.clone();
        let mut sum = a.zero();
        for k in 1..=SERIES_TERMS {
            sum = sum.add(&term);
            term = term.mul(a).div(&a.lit((k + 1) as f64));
        }
        sum
    } else {
        a.exp().sub(&a.one())
    }
}

/// `e^{a + ib} − 1`, every part O(a, b): `cos b − 1 = −2 sin²(b/2)`.
fn cexpm1<T: Transcendental>(a: &T, b: &T) -> [T; 2] {
    let em = expm1(a);
    let (sb, cb) = b.sin_cos();
    let half = b.div(&b.lit(2.0)).sin();
    let cosm1 = half.mul(&half).mul(&b.lit(-2.0));
    [em.mul(&cb).add(&cosm1), em.add(&a.one()).mul(&sb)]
}

/// The whole turns between two angles' actual difference and the one a
/// difference form computed: nonzero where `v + ε` is across an `atan2`
/// cut from `v`, and the forward map with it.
fn turns<T: Real>(actual_from: f64, actual_to: f64, formed: &T) -> f64 {
    ((actual_to - actual_from - formed.to_f64()) / std::f64::consts::TAU).round()
}

/// `x + 2π·q` taken through sin and cos, with `q`'s whole part dropped
/// exactly and a half turn as an exact sign: where a map is continuous
/// across its cut (a whole number of blob waves, a root of power one),
/// the shift is a whole number of turns, and adding its 2π to a small
/// angle in floating point would round the small angle's digits away.
fn sin_cos_shifted<T: Transcendental>(x: &T, q: f64) -> (T, T) {
    let frac = q - q.floor();
    let (s, c) = x.sin_cos();
    if frac == 0.0 {
        (s, c)
    } else if frac == 0.5 {
        (s.neg(), c.neg())
    } else {
        x.add(&x.lit(std::f64::consts::TAU * frac)).sin_cos()
    }
}

/// `K(v + ε) − K(v)` for the forward kernel along `arm` (only a root has
/// arms). `None` at a pole: `v` or `v + ε` where the kernel is not
/// defined, or both at the origin.
pub fn kernel_forward_difference_gen<T: Transcendental>(k: &Kernel, v: &[T; 2], e: &[T; 2], arm: u32) -> Option<[T; 2]> {
    use std::f64::consts::{PI, TAU};
    let w = [v[0].add(&e[0]), v[1].add(&e[1])];
    let x = cnorm2(v);
    // `|v + ε|² − |v|² = 2v·ε + |ε|²`, which cancels nothing.
    let dot = v[0].mul(&e[0]).add(&v[1].mul(&e[1]));
    let t = dot.add(&dot).add(&cnorm2(e));
    match *k {
        Kernel::Root { n, d } => {
            let xv = x.to_f64();
            if !(xv > 0.0) || !(cnorm2(&w).to_f64() > 0.0) {
                return None;
            }
            let nf = n as f64;
            // u = ε/v, and K(v+ε)/K(v) = (1 + u)^{(d/|n|, 1/n)}: the
            // modulus to d/|n| and the angle over n.
            let u = [
                e[0].mul(&v[0]).add(&e[1].mul(&v[1])).div(&x),
                e[1].mul(&v[0]).sub(&e[0].mul(&v[1])).div(&x),
            ];
            let lnmod = ln1p(&u[0].add(&u[0]).add(&cnorm2(&u))).div(&x.lit(2.0));
            let argu = T::atan2(&u[1], &u[0].add(&u[0].one()));
            let tv = T::atan2(&v[1], &v[0]);
            let tw = T::atan2(&w[1], &w[0]);
            // Across the cut the angle gains m whole turns, and the
            // root's angle m/n of one: an exact rotation, and none at
            // all when n divides m.
            let m = turns(tv.to_f64(), tw.to_f64(), &argu);
            let q = m / nf;
            let a = lnmod.mul(&x.lit(d / nf.abs()));
            let b = argu.div(&x.lit(nf));
            let ratio = if q == q.floor() {
                cexpm1(&a, &b)
            } else {
                let (sb, cb) = sin_cos_shifted(&b, q);
                let ea = a.exp();
                [ea.mul(&cb).sub(&a.one()), ea.mul(&sb)]
            };
            // K(v) along the arm, as `kernel_forward_gen` has it.
            let rr = x.sqrt().powf(&x.lit(d / nf.abs()));
            let ang = tv.add(&x.lit(TAU * arm as f64)).div(&x.lit(nf));
            let (sa, ca) = ang.sin_cos();
            Some(cmul(&[rr.mul(&ca), rr.mul(&sa)], &ratio))
        }
        Kernel::Spherical | Kernel::Bubble => {
            // A·v/(|v|² + B): spherical is (1, 1e-6), bubble (4, 4).
            let (scale, b) = if matches!(k, Kernel::Spherical) { (1.0, 1e-6) } else { (4.0, 4.0) };
            let xb = x.add(&x.lit(b));
            let xwb = x.add(&t).add(&x.lit(b));
            let den = xb.mul(&xwb);
            if !(den.to_f64().abs() > 0.0) {
                return None;
            }
            let f = x.lit(scale).div(&den);
            Some([
                e[0].mul(&xb).sub(&v[0].mul(&t)).mul(&f),
                e[1].mul(&xb).sub(&v[1].mul(&t)).mul(&f),
            ])
        }
        Kernel::Hemisphere => {
            // v/s with s = √(|v|²+1): ε/s' − v·t/(s·s'·(s + s')).
            let s = x.add(&x.one()).sqrt();
            let s1 = x.add(&t).add(&x.one()).sqrt();
            let g = t.div(&s.mul(&s1).mul(&s.add(&s1)));
            Some([e[0].div(&s1).sub(&v[0].mul(&g)), e[1].div(&s1).sub(&v[1].mul(&g))])
        }
        Kernel::Disc | Kernel::Blob { .. } => {
            // Both measure θ from +y: θ = atan2(x, y) = π/2 − φ, so the
            // change in θ is minus the change in the usual angle, which
            // is the angle from v to v + ε: atan2(v × ε, |v|² + v·ε).
            if !(x.to_f64() > 0.0) {
                return None;
            }
            let cross = v[0].mul(&e[1]).sub(&v[1].mul(&e[0]));
            let dtheta_formed = T::atan2(&cross, &x.add(&dot)).neg();
            let tv = T::atan2(&v[0], &v[1]);
            let tw = T::atan2(&w[0], &w[1]);
            let m = turns(tv.to_f64(), tw.to_f64(), &dtheta_formed);
            let dtheta = dtheta_formed.add(&x.lit(std::f64::consts::TAU * m));
            match *k {
                Kernel::Disc => {
                    // (θ/π)(sin πr, cos πr).
                    let r = x.sqrt();
                    let r1 = x.add(&t).sqrt();
                    let rsum = r.add(&r1);
                    if !(rsum.to_f64() > 0.0) {
                        return None;
                    }
                    let dr = t.div(&rsum);
                    let pi = x.lit(PI);
                    let (s1, c1) = pi.mul(&r1).sin_cos();
                    let (sm, cm) = pi.mul(&r.add(&dr.div(&x.lit(2.0)))).sin_cos();
                    let h = pi.mul(&dr).div(&x.lit(2.0)).sin();
                    let dsin = cm.mul(&h).mul(&x.lit(2.0));
                    let dcos = sm.mul(&h).mul(&x.lit(-2.0));
                    let dt = dtheta.div(&pi);
                    let th = tv.div(&pi);
                    Some([dt.mul(&s1).add(&th.mul(&dsin)), dt.mul(&c1).add(&th.mul(&dcos))])
                }
                Kernel::Blob { high, low, waves } => {
                    // s(θ)·(v.y, v.x): s(θ')·swap(ε) + (s(θ') − s(θ))·swap(v).
                    let tw_eff = tv.add(&dtheta);
                    let s_w = blob_scale_gen(high, low, waves, &tw_eff);
                    // sin(waves·Δθ/2) with Δθ = formed + 2πm: the turns
                    // shift it by waves·m/2 of a turn, taken exactly.
                    let half = x.lit(waves / 2.0);
                    let (sin_half, _) = sin_cos_shifted(&half.mul(&dtheta_formed), waves * m / 2.0);
                    let ds = x.lit(high - low).mul(&half.mul(&tv.add(&tw_eff)).cos()).mul(&sin_half);
                    Some([s_w.mul(&e[1]).add(&ds.mul(&v[1])), s_w.mul(&e[0]).add(&ds.mul(&v[0]))])
                }
                _ => unreachable!(),
            }
        }
        Kernel::Elliptic => elliptic_difference(v, e, &w),
        // A translation on each quadrant: ε, plus the step between the
        // quadrants of `v` and `v + ε` -- exactly zero within one, and a
        // difference of the table's own numbers across.
        Kernel::Splits { x: sx, y: sy, .. } => {
            let qv = Kernel::splits_quadrant([v[0].to_f64(), v[1].to_f64()]);
            let qw = Kernel::splits_quadrant([w[0].to_f64(), w[1].to_f64()]);
            let bx = (qw & 1) as f64 - (qv & 1) as f64;
            let by = ((qw >> 1) & 1) as f64 - ((qv >> 1) & 1) as f64;
            Some([
                e[0].add(&e[0].lit(bx * sx[0] + by * sy[0])),
                e[1].add(&e[1].lit(bx * sx[1] + by * sy[1])),
            ])
        }
    }
}

/// `h(d', k') − h(d, k)` for [`elliptic_h`], by the rule it chose at
/// `(d, k)`, so nothing cancels: `y²/(d + k)` differences as
/// `(Δy²·(d + k) − y²·(Δd + Δk)) / ((d' + k')(d + k))`, and `d − k` as
/// `Δd − Δk`. `None` where the first's denominator vanishes at either
/// end: a focus, where the map is singular.
#[allow(clippy::too_many_arguments)]
fn elliptic_h_delta<T: Real>(d: &T, k: &T, y2: &T, dd: &T, dk: &T, dy2: &T, dw: &T, kw: &T) -> Option<T> {
    if k.to_f64() >= 0.0 {
        let den = d.add(k);
        let den_w = dw.add(kw);
        if !(den.to_f64() > 0.0) || !(den_w.to_f64() > 0.0) {
            return None;
        }
        Some(dy2.mul(&den).sub(&y2.mul(&dd.add(dk))).div(&den_w.mul(&den)))
    } else {
        Some(dd.sub(dk))
    }
}

/// **Elliptic's forward difference** (tracker C6), every term O(ε).
///
/// Its output is `(2/π)(θ, σG)`: `θ = atan2(x, C)` with `C² = (xmax −
/// x)(xmax + x)`, and `G = ln(1 + m + √m)` with `m = xmax − 1`, `σ` the
/// sign of `y`. Every one of `m`, `xmax − x`, `xmax + x` is half a sum of
/// two [`elliptic_h`] terms, and each of those is differenced by
/// [`elliptic_h_delta`]. Then `Δθ = atan2(ε.x·C − x·ΔC, x·x' + C·C')`
/// with `ΔC = (ΔP·Q + P·ΔQ + ΔP·ΔQ)/(C + C')`, and on one side of `y = 0`
/// `ΔG = ln(1 + (Δm + Δ√m)/(1 + m + √m))`, `Δ√m = Δm/(√m + √m')`. Across
/// it `σ'G' − σG` is a sum of two same-signed terms: small across the
/// segment between the foci, where the map is continuous, and the jump
/// across the rays beyond them.
fn elliptic_difference<T: Transcendental>(v: &[T; 2], e: &[T; 2], w: &[T; 2]) -> Option<[T; 2]> {
    let one = v[0].one();
    let half = v[0].lit(0.5);
    let y2 = v[1].mul(&v[1]);
    // y'² − y² and the distances' differences, as the norms' own.
    let dy2 = e[1].mul(&v[1].add(&v[1]).add(&e[1]));
    let kp = v[0].add(&one);
    let km = one.sub(&v[0]);
    let d1 = kp.mul(&kp).add(&y2).sqrt();
    let d2 = km.mul(&km).add(&y2).sqrt();
    let kpw = w[0].add(&one);
    let kmw = one.sub(&w[0]);
    let y2w = w[1].mul(&w[1]);
    let d1w = kpw.mul(&kpw).add(&y2w).sqrt();
    let d2w = kmw.mul(&kmw).add(&y2w).sqrt();
    let s1 = d1.add(&d1w);
    let s2 = d2.add(&d2w);
    if !(s1.to_f64() > 0.0) || !(s2.to_f64() > 0.0) {
        return None;
    }
    let ex = e[0].clone();
    let dd1 = ex.mul(&kp.add(&kp).add(&ex)).add(&dy2).div(&s1);
    let dd2 = ex.neg().mul(&km.add(&km).sub(&ex)).add(&dy2).div(&s2);
    let p = super::ifs_analysis::elliptic_parts(v);
    let da = elliptic_h_delta(&d1, &kp, &y2, &dd1, &ex, &dy2, &d1w, &kpw)?;
    let db = elliptic_h_delta(&d2, &km, &y2, &dd2, &ex.neg(), &dy2, &d2w, &kmw)?;
    let da_s = elliptic_h_delta(&d1, &kp.neg(), &y2, &dd1, &ex.neg(), &dy2, &d1w, &kpw.neg())?;
    let db_s = elliptic_h_delta(&d2, &km.neg(), &y2, &dd2, &ex, &dy2, &d2w, &kmw.neg())?;
    let clamp = |t: T| if t.to_f64() < 0.0 { t.zero() } else { t };
    // The x output: θ = atan2(x, C), C = √(P·Q).
    let pp = p.a.add(&p.b_s).mul(&half);
    let qq = p.a_s.add(&p.b).mul(&half);
    let dp = da.add(&db_s).mul(&half);
    let dq = da_s.add(&db).mul(&half);
    let c = pp.mul(&qq).sqrt();
    let cw = clamp(pp.add(&dp)).mul(&clamp(qq.add(&dq))).sqrt();
    let csum = c.add(&cw);
    let dc = if csum.to_f64() > 0.0 {
        dp.mul(&qq).add(&pp.mul(&dq)).add(&dp.mul(&dq)).div(&csum)
    } else {
        c.zero()
    };
    let dtheta = T::atan2(&ex.mul(&c).sub(&v[0].mul(&dc)), &v[0].mul(&w[0]).add(&c.mul(&cw)));
    // The y output: σ·ln(1 + m + √m).
    let m = p.a.add(&p.b).mul(&half);
    let dm = da.add(&db).mul(&half);
    let mw = clamp(m.add(&dm));
    let (s, sw) = (m.sqrt(), mw.sqrt());
    let neg_v = v[1].to_f64() < 0.0;
    let neg_w = w[1].to_f64() < 0.0;
    let dg = if neg_v == neg_w {
        let ssum = s.add(&sw);
        let ds = if ssum.to_f64() > 0.0 { dm.div(&ssum) } else { dm.zero() };
        let g = ln1p(&dm.add(&ds).div(&one.add(&m).add(&s)));
        if neg_v {
            g.neg()
        } else {
            g
        }
    } else {
        let g = ln1p(&m.add(&s));
        let gw = ln1p(&mw.add(&sw));
        // σ'G' − σG with σ' = −σ: −σ(G + G').
        let sum = g.add(&gw);
        if neg_v {
            sum
        } else {
            sum.neg()
        }
    };
    let k = v[0].lit(2.0 / std::f64::consts::PI);
    Some([dtheta.mul(&k), dg.mul(&k)])
}

fn lin(m: &[[f64; 2]; 2], d: [f64; 2]) -> [f64; 2] {
    [m[0][0] * d[0] + m[0][1] * d[1], m[1][0] * d[0] + m[1][1] * d[1]]
}

/// `F(z + δ) − F(z)` for a forward map along `arm`: an affine's linear
/// part, or the kernel's form between the map's affines -- which enter
/// only through their linear parts, since a translation cancels.
///
/// `None` where the kernel's form is (a pole), and for the inverse
/// variants, which the forward replay never runs.
pub fn map_forward_difference(m: &Map2, z: [f64; 2], d: [f64; 2], arm: u32) -> Option<[f64; 2]> {
    match m {
        Map2::Affine(a) => Some(lin(&a.m, d)),
        Map2::Nonlinear(n) => {
            let k = kernel_forward_difference_gen(&n.kernel, &n.pre.apply(z), &lin(&n.pre.m, d), arm)?;
            Some(lin(&n.post.m, [n.w * k[0], n.w * k[1]]))
        }
        Map2::Sum(s) => {
            let e = lin(&s.pre.m, d);
            let k = kernel_forward_difference_gen(&s.kernel, &s.pre.apply(z), &e, arm)?;
            let a = lin(&s.lin.m, e);
            Some(lin(&s.post.m, [a[0] + s.kw * k[0], a[1] + s.kw * k[1]]))
        }
        Map2::NonlinearInverse(_) | Map2::SumInverse(_) => None,
    }
}

/// Floats per map row in the shader's table (`replay_delta.wgsl`).
pub const ROW_FLOATS: usize = 24;

/// A forward map as the shader's row: see `replay_delta.wgsl` for the
/// layout. An inverse variant, which the replay never runs, is written
/// as the identity.
pub fn forward_row(m: &Map2) -> [f32; ROW_FLOATS] {
    let mut row = [0.0f32; ROW_FLOATS];
    let put = |row: &mut [f32; ROW_FLOATS], at: usize, a: &[[f64; 2]; 2]| {
        row[at] = a[0][0] as f32;
        row[at + 1] = a[0][1] as f32;
        row[at + 2] = a[1][0] as f32;
        row[at + 3] = a[1][1] as f32;
    };
    let kernel = |row: &mut [f32; ROW_FLOATS], k: &Kernel| {
        let (id, p) = match *k {
            Kernel::Root { n, d } => (0.0, [n as f64, d, 0.0]),
            Kernel::Spherical => (1.0, [0.0; 3]),
            Kernel::Bubble => (2.0, [0.0; 3]),
            Kernel::Hemisphere => (3.0, [0.0; 3]),
            Kernel::Disc => (4.0, [0.0; 3]),
            Kernel::Blob { high, low, waves } => (5.0, [high, low, waves]),
            Kernel::Elliptic => (6.0, [0.0; 3]),
            // Its steps, past the three slots: a difference needs only
            // what crossing each axis adds.
            Kernel::Splits { x, y, .. } => {
                row[20] = x[0] as f32;
                row[21] = x[1] as f32;
                row[22] = y[0] as f32;
                row[23] = y[1] as f32;
                (7.0, [0.0; 3])
            }
        };
        row[1] = id;
        row[2] = p[0] as f32;
        row[3] = p[1] as f32;
        row[4] = p[2] as f32;
    };
    match m {
        Map2::Affine(a) => put(&mut row, 16, &a.m),
        Map2::Nonlinear(n) => {
            row[0] = 1.0;
            kernel(&mut row, &n.kernel);
            row[5] = n.w as f32;
            put(&mut row, 6, &n.pre.m);
            row[10] = n.pre.t[0] as f32;
            row[11] = n.pre.t[1] as f32;
            put(&mut row, 16, &n.post.m);
        }
        Map2::Sum(s) => {
            row[0] = 2.0;
            kernel(&mut row, &s.kernel);
            row[5] = s.kw as f32;
            put(&mut row, 6, &s.pre.m);
            row[10] = s.pre.t[0] as f32;
            row[11] = s.pre.t[1] as f32;
            put(&mut row, 12, &s.lin.m);
            put(&mut row, 16, &s.post.m);
        }
        Map2::NonlinearInverse(_) | Map2::SumInverse(_) => put(&mut row, 16, &[[1.0, 0.0], [0.0, 1.0]]),
    }
    row
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::escape::bigfloat::BigFloat;
    use crate::scene::ifs_analysis::kernel_forward_gen;

    const LIMBS: usize = 8;

    fn big2(v: [f64; 2]) -> [BigFloat; 2] {
        [BigFloat::from_f64(v[0], LIMBS), BigFloat::from_f64(v[1], LIMBS)]
    }

    /// Every forward kernel the analysis accepts, with the arms worth
    /// reaching: a root of each sign of distance, of several powers, and
    /// a negative power.
    fn kernels() -> Vec<(Kernel, Vec<u32>)> {
        vec![
            (Kernel::Root { n: 2, d: 1.0 }, vec![0, 1]),
            (Kernel::Root { n: 3, d: 1.0 }, vec![0, 2]),
            (Kernel::Root { n: 2, d: -1.0 }, vec![0, 1]),
            (Kernel::Root { n: 15, d: -1.0 }, vec![0, 7, 14]),
            (Kernel::Root { n: 8, d: -1.0 }, vec![3]),
            (Kernel::Root { n: -2, d: 1.0 }, vec![0, 1]),
            (Kernel::Root { n: 5, d: 2.5 }, vec![4]),
            (Kernel::Spherical, vec![0]),
            (Kernel::Bubble, vec![0]),
            (Kernel::Hemisphere, vec![0]),
            (Kernel::Disc, vec![0]),
            (Kernel::Blob { high: 1.2, low: 0.4, waves: 3.0 }, vec![0]),
            (Kernel::Blob { high: 0.9, low: 0.1, waves: 2.5 }, vec![0]),
            // The rings pass the segment between the foci (−0.3, −0.9 on
            // the x axis) and the ray beyond them (−1.7).
            (Kernel::Elliptic, vec![0]),
            (Kernel::Splits { base: [-0.4, 0.1], x: [0.8, 0.3], y: [-0.2, 0.9] }, vec![0]),
        ]
    }

    /// Points on rings of three radii, including the negative x axis
    /// (a root's cut) and the negative y axis (disc's and blob's).
    fn points() -> Vec<[f64; 2]> {
        let mut pts = Vec::new();
        for r in [0.3, 0.9, 1.7] {
            for i in 0..12 {
                let a = std::f64::consts::TAU * i as f64 / 12.0 + 0.1;
                pts.push([r * a.cos(), r * a.sin()]);
            }
            pts.push([-r, 1e-9]);
            pts.push([-r, -1e-9]);
            pts.push([1e-9, -r]);
            pts.push([-1e-9, -r]);
        }
        pts
    }

    /// **Gate 1 of deep-zoom-precision.md: the forward forms are EXACT.**
    ///
    /// `D(v, ε)` in f64 against `K(v+ε) − K(v)` taken in `BigFloat` at
    /// 512 bits, at ε from 1e-30 to 1e-1 of `|v|`, in four directions --
    /// judged relative to `|D|`, which is the whole point: a form that
    /// is only right to f64's ABSOLUTE precision is useless where ε is
    /// thirty orders below the position beside it. The points on the
    /// cuts send some ε across them, so the jump is reproduced too.
    #[test]
    fn the_forward_forms_are_exact() {
        let mut worst = 0.0f64;
        let mut worst_where = String::new();
        let mut checked = 0usize;
        let mut crossed = 0usize;
        for (k, arms) in kernels() {
            for &arm in &arms {
                for v in points() {
                    let scale = v[0].hypot(v[1]);
                    for p in 1..=30 {
                        let mag = scale * 10f64.powi(-p);
                        for dir in [[1.0, 0.0], [0.0, 1.0], [0.6, -0.8], [-0.3, -0.954]] {
                            let e = [mag * dir[0], mag * dir[1]];
                            let w = [v[0] + e[0], v[1] + e[1]];
                            if w == v {
                                continue;
                            }
                            let Some(got) = kernel_forward_difference_gen(&k, &v, &e, arm) else { continue };
                            let (bv, be) = (big2(v), big2(e));
                            let bw = [bv[0].add(&be[0]), bv[1].add(&be[1])];
                            let fv = kernel_forward_gen(&k, &bv, arm);
                            let fw = kernel_forward_gen(&k, &bw, arm);
                            let want = [fw[0].sub(&fv[0]).to_f64(), fw[1].sub(&fv[1]).to_f64()];
                            let norm = want[0].hypot(want[1]);
                            if !(norm > 0.0) || !norm.is_finite() {
                                continue;
                            }
                            if norm > 1e3 * mag * 1e3 {
                                crossed += 1;
                            }
                            let err = (got[0] - want[0]).hypot(got[1] - want[1]) / norm;
                            if !(err <= worst) {
                                worst = err;
                                worst_where = format!("{k:?} arm {arm} at {v:?} with |ε| {mag:.2e} dir {dir:?}");
                            }
                            checked += 1;
                        }
                    }
                }
            }
        }
        println!("  {checked} differences ({crossed} across a cut); worst relative error {worst:.2e} at {worst_where}");
        assert!(checked > 20_000, "only {checked} points were checkable");
        assert!(crossed > 0, "no difference crossed a cut, so the jump went untested");
        assert!(
            worst < 1e-13,
            "worst relative error {worst:.2e} at {worst_where} -- a form that loses digits of ε has a cancellation in it"
        );
    }

    /// What the forms are for, as a measurement: the direct subtraction
    /// in f64 has no correct digit left once ε is below f64's spacing at
    /// `v`, where the form still has them all.
    #[test]
    fn the_direct_forward_subtraction_loses_everything() {
        let k = Kernel::Root { n: 2, d: -1.0 };
        let v = [0.268, -0.111];
        let e = [3e-18, -4e-18];
        let w = [v[0] + e[0], v[1] + e[1]];
        let fv = kernel_forward_gen(&k, &v, 1);
        let fw = kernel_forward_gen(&k, &w, 1);
        let direct = [fw[0] - fv[0], fw[1] - fv[1]];
        let form = kernel_forward_difference_gen(&k, &v, &e, 1).expect("defined");
        let (bv, be) = (big2(v), big2(e));
        let bw = [bv[0].add(&be[0]), bv[1].add(&be[1])];
        let bfv = kernel_forward_gen(&k, &bv, 1);
        let bfw = kernel_forward_gen(&k, &bw, 1);
        let want = [bfw[0].sub(&bfv[0]).to_f64(), bfw[1].sub(&bfv[1]).to_f64()];
        let norm = want[0].hypot(want[1]);
        let err = |g: [f64; 2]| (g[0] - want[0]).hypot(g[1] - want[1]) / norm;
        println!("  direct {:.2e}, form {:.2e}", err(direct), err(form));
        assert!(err(direct) > 0.5, "the direct subtraction kept digits: {:.2e}", err(direct));
        assert!(err(form) < 1e-14, "the form lost digits: {:.2e}", err(form));
    }

    /// The shader harness for the forward forms: `ff_atan2` as
    /// `utilities.wgsl` has it, `replay_delta.wgsl`, and one job per
    /// thread -- `(z, δ)` then `(arm, row offset)`.
    fn harness() -> String {
        let util = std::fs::read_to_string("shaders/core/utilities.wgsl").expect("utilities.wgsl");
        let a = util.find("fn ff_atan2(").expect("ff_atan2");
        let end = a + util[a..].find("\n}\n").expect("its end") + 3;
        let delta = std::fs::read_to_string("shaders/core/replay_delta.wgsl").expect("replay_delta.wgsl");
        let head = "@group(0) @binding(0) var<storage, read> cylinders: array<f32>;\n\
                    @group(0) @binding(1) var<storage, read> jobs: array<vec4<f32>>;\n\
                    @group(0) @binding(2) var<storage, read_write> out: array<vec2<f32>>;\n";
        let main = "@compute @workgroup_size(64)\n\
                    fn main(@builtin(global_invocation_id) id: vec3<u32>) {\n\
                        let i = id.x;\n\
                        if (i >= arrayLength(&out)) { return; }\n\
                        let a = jobs[2u * i];\n\
                        let b = jobs[2u * i + 1u];\n\
                        out[i] = ct_fwd_diff(u32(b.y), a.xy, a.zw, u32(b.x));\n\
                    }\n";
        format!("{head}{}\n{delta}\n{main}", &util[a..end])
    }

    fn device() -> (wgpu::Device, wgpu::Queue) {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .expect("adapter");
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor { label: Some("forward forms"), ..Default::default() }))
            .expect("device")
    }

    /// Run `jobs` -- `(z, δ, arm)` against the map whose row is at offset
    /// 0 of `rows` -- through the shader's forms.
    fn run(device: &wgpu::Device, queue: &wgpu::Queue, rows: &[f32], jobs: &[([f32; 2], [f32; 2], u32)]) -> Vec<[f32; 2]> {
        use wgpu::util::DeviceExt;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("forward forms"),
            source: wgpu::ShaderSource::Wgsl(harness().into()),
        });
        let pipe = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("forward forms"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let packed: Vec<f32> = jobs.iter().flat_map(|(z, d, arm)| [z[0], z[1], d[0], d[1], *arm as f32, 0.0, 0.0, 0.0]).collect();
        let st = wgpu::BufferUsages::STORAGE;
        let init = |bytes: &[u8], usage| device.create_buffer_init(&wgpu::util::BufferInitDescriptor { label: None, contents: bytes, usage });
        let rows_buf = init(bytemuck::cast_slice(rows), st);
        let jobs_buf = init(bytemuck::cast_slice(&packed), st);
        let bytes = (jobs.len() * 8) as u64;
        let out = device.create_buffer(&wgpu::BufferDescriptor { label: None, size: bytes, usage: st | wgpu::BufferUsages::COPY_SRC, mapped_at_creation: false });
        let stage = device.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: bytes,
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipe.get_bind_group_layout(0),
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: rows_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: jobs_buf.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 2, resource: out.as_entire_binding() },
            ],
        });
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor { label: None, timestamp_writes: None });
            pass.set_pipeline(&pipe);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups((jobs.len() as u32).div_ceil(64), 1, 1);
        }
        enc.copy_buffer_to_buffer(&out, 0, &stage, 0, bytes);
        queue.submit(Some(enc.finish()));
        stage.slice(..).map_async(wgpu::MapMode::Read, |_| {});
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        let got: Vec<[f32; 2]> = bytemuck::cast_slice::<u8, [f32; 2]>(&stage.slice(..).get_mapped_range()).to_vec();
        got
    }

    /// **Gate 2 of deep-zoom-precision.md: the shader's forward forms are
    /// the CPU's.** One fixture flame per kernel -- julian of several
    /// powers and both signs of distance, every arm worth reaching, and
    /// spherical, bubble, hemisphere, disc and blob -- its transform 0's
    /// forward map as the shader's row, and the WGSL form against
    /// `map_forward_difference` in f64 on the SAME f32 inputs, at offsets
    /// from 1e-1 to 1e-12 of the point. Judged relative to the
    /// difference itself: f32 rounding and each form's conditioning,
    /// nothing more.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_shader_forward_forms_are_the_cpu_ones() {
        use crate::scene::ifs_analysis::analyse_2d_maps;
        use crate::scene::transforms::{Flame, Transform};
        let guard = crate::variations::global_registry();
        let reg = &*guard;
        let (device, queue) = device();
        let jul = |power: f32, dist: f32| {
            let mut t = Transform::default();
            (t.a, t.b, t.c, t.d, t.e, t.f) = (0.7071, 0.7071, -0.7071, 0.7071, 0.2, -0.1);
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("flatten", 1.0);
            t.set_variation("julian", 1.0);
            t.set_variation_param("julian", "power", power);
            t.set_variation_param("julian", "dist", dist);
            t
        };
        let kern = |name: &str, params: &[(&str, f32)]| {
            let mut t = Transform::default();
            (t.a, t.b, t.c, t.d, t.e, t.f) = (0.9, 0.3, -0.3, 0.9, 0.15, -0.2);
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation(name, 0.6);
            for (p, v) in params {
                t.set_variation_param(name, p, *v);
            }
            t
        };
        let mut affine = Transform::default();
        (affine.a, affine.d, affine.e) = (0.5, 0.5, 0.3);
        affine.variations.clear();
        affine.variation_order.clear();
        affine.set_variation("linear", 1.0);
        let cases: Vec<(&str, Transform, Vec<u32>)> = vec![
            ("root n=2 d=1", jul(2.0, 1.0), vec![0, 1]),
            ("root n=3 d=1", jul(3.0, 1.0), vec![0, 1, 2]),
            ("root n=-2 d=1", jul(-2.0, 1.0), vec![0, 1]),
            ("root n=2 d=-1", jul(2.0, -1.0), vec![0, 1]),
            ("root n=15 d=-1", jul(15.0, -1.0), vec![0, 7, 14]),
            ("root n=8 d=-1", jul(8.0, -1.0), vec![3]),
            ("spherical", kern("spherical", &[]), vec![0]),
            ("bubble", kern("bubble", &[]), vec![0]),
            ("hemisphere", kern("hemisphere", &[]), vec![0]),
            ("disc", kern("disc", &[]), vec![0]),
            ("blob", kern("blob", &[("high", 1.2), ("low", 0.4), ("waves", 3.0)]), vec![0]),
            ("elliptic", kern("elliptic", &[]), vec![0]),
            (
                "splits",
                kern("splits", &[("x", 0.4), ("y", -0.3), ("lshear", 0.1), ("rshear", -0.2), ("ushear", 0.15), ("dshear", 0.05)]),
                vec![0],
            ),
            // Summed with an affine, as bipolar-elliptic-splits2 has it:
            // folded into the post-affine (`transform_map_2d_ordered`).
            (
                "linear + splits",
                {
                    let mut t = kern("splits", &[("x", 1.0), ("y", 0.099)]);
                    t.set_variation("linear", 0.2);
                    t
                },
                vec![0],
            ),
        ];
        let mut worst_all = 0.0f64;
        for (name, t, arms) in cases {
            let mut flame = Flame::default();
            flame.transforms = vec![t, affine.clone()];
            let ifs = match analyse_2d_maps(&flame, reg) {
                Ok(i) => i,
                Err(e) => panic!("{name}: the fixture does not analyse: {e:?}"),
            };
            let map = &ifs.maps[0].forward;
            let rows = forward_row(map).to_vec();
            let mut jobs: Vec<([f32; 2], [f32; 2], u32)> = Vec::new();
            for &arm in &arms {
                for rad in [0.2f32, 0.55, 1.1] {
                    for i in 0..16 {
                        let a = std::f32::consts::TAU * i as f32 / 16.0 + 0.05;
                        let z = [rad * a.cos(), rad * a.sin()];
                        for p in 1..=12 {
                            let mag = rad * 10f32.powi(-p);
                            for dir in [[1.0f32, 0.0], [0.0, 1.0], [0.6, -0.8], [-0.3, -0.954]] {
                                jobs.push((z, [mag * dir[0], mag * dir[1]], arm));
                            }
                        }
                    }
                }
            }
            let got = run(&device, &queue, &rows, &jobs);
            let (mut worst, mut n) = (0.0f64, 0usize);
            let mut worst_where = String::new();
            for ((z, d, arm), g) in jobs.iter().zip(&got) {
                let zf = [z[0] as f64, z[1] as f64];
                let df = [d[0] as f64, d[1] as f64];
                let Some(want) = map_forward_difference(map, zf, df, *arm) else { continue };
                let norm = want[0].hypot(want[1]);
                if !(norm > 0.0) || !norm.is_finite() || norm > 1e20 {
                    continue;
                }
                let err = (g[0] as f64 - want[0]).hypot(g[1] as f64 - want[1]) / norm;
                n += 1;
                if !(err <= worst) {
                    worst = err;
                    worst_where = format!("arm {arm} at {z:?} with δ {d:?}: shader {g:?}, cpu {want:?}");
                }
            }
            println!("  {name}: {n} differences, worst relative error {worst:.2e} ({worst_where})");
            assert!(n > 1000, "{name}: only {n} checkable");
            worst_all = worst_all.max(worst);
        }
        assert!(worst_all < 1e-5, "worst relative error {worst_all:.2e}");
    }
}
