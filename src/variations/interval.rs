//! **Interval arithmetic for variation bounds**, shared by the two
//! evaluators that derive a forward bound: [`derive`](super::derive), over
//! a variation's shipped WGSL through naga (desktop only), and
//! [`rhai_bound`](super::rhai_bound), over a Rhai variation's own code
//! (every platform; docs/archive/projects/rhai-one-form.md, route B). Nothing here
//! parses anything, so the web build has it.

use super::bound::Ball;

/// A closed real interval. `lo <= hi` always; either end may be
/// infinite, and neither is ever NaN.
///
/// f64 bounding an f32 computation, widened outward by a relative
/// margin at every operation. The margin is far larger than the f32
/// rounding it covers, which costs a little tightness and removes a
/// whole class of question about directed rounding.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Interval {
    pub lo: f64,
    pub hi: f64,
}

/// Outward widening per operation. `2^-23` is one f32 ulp; eight of
/// them is slack enough that no f32-vs-f64 discrepancy can escape a
/// derived disc, and loose enough to notice only where a bound was
/// already marginal.
pub(crate) const WIDEN: f64 = 8.0 * (1.0 / 8_388_608.0);

/// Every value a 32-bit integer can hold, read either as signed or
/// unsigned.
///
/// The bound for any bitwise result. A hash like
/// `h = (h ^ (h >> 13u)) * 1274126177u` has no useful interval —
/// avalanche is the point — but it has an exact RANGE, and the bodies
/// that use one (the truchet and julia families) immediately divide
/// it down to a fraction, where a full-width 32-bit range becomes an
/// ordinary small number again. Sound whichever way the bits are
/// interpreted, which is why it spans both signs.
#[cfg(not(target_arch = "wasm32"))]
pub(crate) const INT32_RANGE: Interval = Interval { lo: -2147483648.0, hi: 4294967295.0 };

impl Interval {
    pub fn new(lo: f64, hi: f64) -> Self {
        debug_assert!(!lo.is_nan() && !hi.is_nan());
        Self { lo: lo.min(hi), hi: lo.max(hi) }
    }

    pub fn point(v: f64) -> Self {
        Self { lo: v, hi: v }
    }

    pub const UNBOUNDED: Self = Self { lo: f64::NEG_INFINITY, hi: f64::INFINITY };

    pub(crate) fn widen(self) -> Self {
        if !self.lo.is_finite() || !self.hi.is_finite() {
            return self;
        }
        let m = (self.lo.abs().max(self.hi.abs())) * WIDEN + f64::MIN_POSITIVE;
        Self { lo: self.lo - m, hi: self.hi + m }
    }

    /// [`widen`](Self::widen) for the result of one CORRECTLY ROUNDED
    /// operation (`+ - *`, and `/` within WGSL's 2.5 ulp): each end by
    /// its own magnitude. The rounded result of a value is within half
    /// an ulp of it, and rounding is monotone, so the least of the
    /// rounded results is within an ulp of the least exact one -- the
    /// slop at an end scales with that end, not with the other. Scaled
    /// by the larger end, `sqrt(r2) + 1e-6` over `[0, 1]` reached below
    /// zero, and a `pow` the formula language guards by `a < 0.0` took
    /// its negative-base arm (mobiusN, synth). Plus f32's least normal,
    /// for a result flushed to zero.
    pub(crate) fn round_out(self) -> Self {
        if !self.lo.is_finite() || !self.hi.is_finite() {
            return self;
        }
        let ftz = f32::MIN_POSITIVE as f64;
        Self { lo: self.lo - self.lo.abs() * WIDEN - ftz, hi: self.hi + self.hi.abs() * WIDEN + ftz }
    }

    pub fn contains(&self, v: f64) -> bool {
        v >= self.lo && v <= self.hi
    }

    pub(crate) fn finite(&self) -> bool {
        self.lo.is_finite() && self.hi.is_finite()
    }

    pub(crate) fn add(self, o: Self) -> Self {
        Self::new(self.lo + o.lo, self.hi + o.hi).round_out()
    }

    pub(crate) fn sub(self, o: Self) -> Self {
        Self::new(self.lo - o.hi, self.hi - o.lo).round_out()
    }

    pub(crate) fn mul(self, o: Self) -> Self {
        // The four corners: the extremes of a product over a box are
        // always at one of them.
        let c = [self.lo * o.lo, self.lo * o.hi, self.hi * o.lo, self.hi * o.hi];
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for v in c {
            if v.is_nan() {
                // 0 · ∞. The only way to get one here is an interval
                // that already reached infinity, so the honest answer
                // is that nothing is known.
                return Self::UNBOUNDED;
            }
            lo = lo.min(v);
            hi = hi.max(v);
        }
        Self::new(lo, hi).round_out()
    }

    /// `self / o`, and the place a pole announces itself.
    ///
    /// An interval containing zero in the denominator makes the
    /// quotient unbounded, and that propagates to a refusal naming a
    /// pole. It is the honest answer for `curl` (`1/(re² + im²)`) and
    /// for anything that divides by a coordinate: those maps really
    /// are unbounded, and the render survives them only because
    /// bad-value recovery discards the samples afterwards.
    pub(crate) fn div(self, o: Self) -> Self {
        if o.lo <= 0.0 && o.hi >= 0.0 {
            return Self::UNBOUNDED;
        }
        let c = [self.lo / o.lo, self.lo / o.hi, self.hi / o.lo, self.hi / o.hi];
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        for v in c {
            if v.is_nan() {
                return Self::UNBOUNDED;
            }
            lo = lo.min(v);
            hi = hi.max(v);
        }
        Self::new(lo, hi).round_out()
    }

    pub(crate) fn neg(self) -> Self {
        Self::new(-self.hi, -self.lo)
    }

    pub(crate) fn union(self, o: Self) -> Self {
        Self::new(self.lo.min(o.lo), self.hi.max(o.hi))
    }

    pub(crate) fn abs(self) -> Self {
        if self.lo >= 0.0 {
            self
        } else if self.hi <= 0.0 {
            self.neg()
        } else {
            // Straddles zero: the low end is zero, the high end is
            // whichever side reaches further.
            Self::new(0.0, self.lo.abs().max(self.hi.abs()))
        }
    }

    /// A monotone function applied to both ends.
    pub(crate) fn monotone(self, f: impl Fn(f64) -> f64) -> Self {
        self.monotone_exact(f).widen()
    }

    /// [`monotone`](Self::monotone) for an `f` the shader computes
    /// exactly (`floor`, `round`): no rounding slop to allow for.
    pub(crate) fn monotone_exact(self, f: impl Fn(f64) -> f64) -> Self {
        let (a, b) = (f(self.lo), f(self.hi));
        if a.is_nan() || b.is_nan() {
            return Self::UNBOUNDED;
        }
        Self::new(a, b)
    }

    /// `sin` or `cos` over the interval: `[-1, 1]` unless the span is
    /// short enough to track, and short enough is generously defined.
    ///
    /// The full-range answer is always sound, which matters more than
    /// it might: trig of a huge argument is garbage on every platform
    /// and differently so (`docs/accepted-divergences.txt`), and
    /// `[-1, 1]` is right whatever the GPU actually computed.
    pub(crate) fn trig(self, f: impl Fn(f64) -> f64, quarter_offset: f64) -> Self {
        if !self.finite() || self.hi - self.lo >= std::f64::consts::TAU {
            return Self::new(-1.0, 1.0);
        }
        // Sample the ends and every extremum of the period inside.
        let mut lo = f(self.lo).min(f(self.hi));
        let mut hi = f(self.lo).max(f(self.hi));
        // Extrema of sin sit at π/2 + kπ; of cos at kπ. `quarter_offset`
        // selects which.
        let pi = std::f64::consts::PI;
        let first = ((self.lo - quarter_offset) / pi).ceil();
        let mut k = first;
        while quarter_offset + k * pi <= self.hi {
            let v = f(quarter_offset + k * pi);
            lo = lo.min(v);
            hi = hi.max(v);
            k += 1.0;
            if k - first > 8.0 {
                return Self::new(-1.0, 1.0);
            }
        }
        Self::new(lo, hi).widen()
    }
}

/// Why a body could not be bounded.
///
/// Every variant names something specific, because the phase-1
/// deliverable is a census of what stops the evaluator and the census
/// is only worth having if the reasons are distinguishable.
#[derive(Debug, Clone, PartialEq)]
pub enum Refusal {
    /// The variation is not in the registry, or has no 2D body.
    NoBody,
    /// A statement kind phase 1 does not model. Phase 2 takes the
    /// branches and loops.
    Construct(&'static str),
    /// An intrinsic with no interval rule yet. Named so the survey's
    /// frequency list says which to add next.
    Intrinsic(String),
    /// A call to something not in the module — should not happen, and
    /// says so if it does.
    UnknownCall(String),
    /// The body divides by an interval containing zero. The map is
    /// genuinely unbounded there; see [`Interval::div`].
    Pole,
    /// `sqrt`/`log`/`acos` of an interval reaching outside its
    /// domain. A body that guards its own domain (`sqrt(max(0, x))`)
    /// never lands here.
    Domain(&'static str),
    /// The body reads or writes per-thread state, so its output
    /// depends on what a previous iteration stored.
    State,
    /// The body did arithmetic on something the evaluator carries
    /// without a value. Names which.
    Opaque(&'static str),
    /// The returned point depends on something the evaluator carries
    /// without a value (a direct colour, a raw RNG word). Names which.
    Reaches(&'static str),
    /// The body reads an init-derived parameter slot, whose value is
    /// computed by a `wgsl_init` function that the probe module does
    /// not carry.
    InitSlot,
    /// A loop whose exit condition never became definitely true
    /// within the unroll cap.
    LoopUnsettled,
    /// The value came out with a shape the evaluator did not expect.
    Shape(&'static str),
    /// The result was unbounded without a single identifiable cause.
    Unbounded,
}

impl std::fmt::Display for Refusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NoBody => write!(f, "no 2D body"),
            Self::Construct(c) => write!(f, "uses {c}, which is phase 2"),
            Self::Intrinsic(n) => write!(f, "no interval rule for `{n}`"),
            Self::UnknownCall(n) => write!(f, "calls `{n}`, which is not in the module"),
            Self::Pole => write!(f, "divides by a range containing zero (a pole)"),
            Self::Domain(d) => write!(f, "leaves the domain of {d}"),
            Self::State => write!(f, "reads per-thread state"),
            Self::Opaque(w) => write!(f, "does arithmetic on {w}"),
            Self::Reaches(w) => write!(f, "its answer depends on {w}"),
            Self::InitSlot => write!(f, "reads an init-derived parameter slot"),
            Self::LoopUnsettled => write!(f, "a loop that did not settle within the cap"),
            Self::Shape(s) => write!(f, "unexpected value shape: {s}"),
            Self::Unbounded => write!(f, "the result is unbounded"),
        }
    }
}

/// `x²` over a range, which is never negative however the range
/// straddles zero.
pub(crate) fn square(x: Interval) -> Interval {
    let a = x.lo * x.lo;
    let b = x.hi * x.hi;
    let lo = if x.lo <= 0.0 && x.hi >= 0.0 { 0.0 } else { a.min(b) };
    Interval::new(lo, a.max(b)).widen()
}

/// The angles `atan2(y, x)` takes over a box, and only those.
///
/// **This used to return the whole turn, always.** Sound, and the
/// single loosest rule in the evaluator: every radial variation
/// computes an angle and then a sine and cosine of it, so a full-turn
/// angle makes the image an annulus however small the input. Measured
/// on `julian`, it was the whole reason a bound there could not shrink
/// — the arm the random draw picks was blamed for years and is not at
/// fault.
///
/// `atan2` is continuous except across the negative real axis, so
/// away from that cut the extremes are at the box's corners. Two
/// cases have to give up and take the turn: a box holding the origin,
/// where every angle really does occur, and one straddling the cut,
/// where the range wraps and an interval cannot say so.
///
/// Note the ORDER: `atan2(y, x)` takes the ordinate first, which is
/// how the shader calls it and the opposite of how a box is usually
/// written.
pub(crate) fn atan2_iv(y: Interval, x: Interval) -> Interval {
    let whole = Interval::new(-std::f64::consts::PI, std::f64::consts::PI);
    let spans_y = y.lo <= 0.0 && y.hi >= 0.0;
    if spans_y && x.lo <= 0.0 && x.hi >= 0.0 {
        return whole; // holds the origin
    }
    if spans_y && x.hi < 0.0 {
        return whole; // straddles the branch cut
    }
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for yy in [y.lo, y.hi] {
        for xx in [x.lo, x.hi] {
            let a = yy.atan2(xx);
            if !a.is_finite() {
                return whole;
            }
            lo = lo.min(a);
            hi = hi.max(a);
        }
    }
    if lo > hi {
        return whole;
    }
    Interval::new(lo, hi).widen()
}

/// The formula language's `pow` (`uf_powf`): 1 at a zero exponent; a
/// negative base to a whole exponent as `pow(-a, b)`, negated at an odd
/// one; else WGSL's `pow`. The hull of its non-negative base's `pow` and,
/// for a base reaching below zero, the negative part to each whole
/// exponent in range (a fractional one there is NaN, which no bound need
/// hold).
pub(crate) fn powf_iv(a: Interval, b: Interval) -> Result<Interval, Refusal> {
    let mut acc: Option<Interval> = None;
    let mut add = |r: Interval| acc = Some(acc.map_or(r, |x: Interval| x.union(r)));
    if a.hi >= 0.0 {
        add(pow_iv(Interval::new(a.lo.max(0.0), a.hi), b)?);
    }
    if a.lo < 0.0 {
        let (n0, n1) = (b.lo.ceil(), b.hi.floor());
        if n1 - n0 > 64.0 {
            return Err(Refusal::Domain("pow with a negative base"));
        }
        let neg = Interval::new(a.lo, a.hi.min(0.0));
        let mut n = n0;
        while n <= n1 {
            add(pow_iv(neg, Interval::point(n))?);
            n += 1.0;
        }
    }
    if b.contains(0.0) {
        add(Interval::point(1.0));
    }
    acc.ok_or(Refusal::Domain("pow with a negative base"))
}

pub(crate) fn sqrt_iv(x: Interval) -> Result<Interval, Refusal> {
    if x.hi < 0.0 {
        return Err(Refusal::Domain("sqrt"));
    }
    // A lower end below zero is clamped rather than refused: it is
    // almost always a rounding artefact of an expression that is
    // mathematically non-negative (`dot(p, p)`), and clamping is
    // sound because the true value cannot be negative there.
    // Never below zero, however widened: no square root is.
    let r = Interval::new(x.lo.max(0.0).sqrt(), x.hi.max(0.0).sqrt()).widen();
    Ok(Interval::new(r.lo.max(0.0), r.hi))
}

pub(crate) fn pow_iv(mut b: Interval, e: Interval) -> Result<Interval, Refusal> {
    // **A lower end negative by no more than the widening could have
    // made it is treated as zero**, the same call `sqrt_iv` makes and
    // for the same reason. `widen` is absolute, so a mathematically
    // non-negative `dot(p, p)` of [0, 4] comes back as [-eps, 4+eps],
    // and the julia family raises exactly that to a fractional power.
    // Refusing it cost `juliascope` and `julia3D` six corpus flames
    // between them.
    //
    // Where the interval genuinely straddles zero by more than the
    // slop this does not fire, and a truly negative base is NaN on
    // the GPU — a value no disc contains and bad-value recovery
    // removes from the render anyway.
    if b.lo < 0.0 && b.hi > 0.0 && -b.lo <= b.hi * 4.0 * WIDEN {
        b.lo = 0.0;
    }
    if b.lo < 0.0 {
        // A negative base is NaN on the GPU unless the exponent is a
        // whole number -- and when it is, the answer is ordinary:
        // even powers fold to the positive side, odd ones keep the
        // sign and stay monotone. Refusing the whole case cost the
        // julia family, which raises a signed radius to a parameter
        // that is usually an integer.
        if e.lo == e.hi && e.lo.fract() == 0.0 && e.lo.abs() < 64.0 {
            let n = e.lo as i32;
            if n >= 0 && n % 2 == 0 {
                let a = b.lo.abs().powi(n);
                let c = b.hi.abs().powi(n);
                let lo = if b.lo <= 0.0 && b.hi >= 0.0 { 0.0 } else { a.min(c) };
                return Ok(Interval::new(lo, a.max(c)).widen());
            }
            if n >= 0 {
                return Ok(Interval::new(b.lo.powi(n), b.hi.powi(n)).widen());
            }
            // A negative power is a division, so a base spanning zero
            // is a pole like any other.
            if b.lo <= 0.0 && b.hi >= 0.0 {
                return Err(Refusal::Pole);
            }
            let (a, c) = (b.lo.powi(n), b.hi.powi(n));
            return Ok(Interval::new(a.min(c), a.max(c)).widen());
        }
        return Err(Refusal::Domain("pow with a negative base"));
    }
    let c = [
        b.lo.max(0.0).powf(e.lo),
        b.lo.max(0.0).powf(e.hi),
        b.hi.max(0.0).powf(e.lo),
        b.hi.max(0.0).powf(e.hi),
    ];
    let mut lo = f64::INFINITY;
    let mut hi = f64::NEG_INFINITY;
    for v in c {
        if v.is_nan() {
            return Err(Refusal::Domain("pow"));
        }
        lo = lo.min(v);
        hi = hi.max(v);
    }
    if !lo.is_finite() || !hi.is_finite() {
        return Err(Refusal::Unbounded);
    }
    Ok(Interval::new(lo, hi).widen())
}

/// `x < y` over two ranges: definitely, definitely not, or unknown.
pub(crate) fn cmp_lt(x: Interval, y: Interval) -> Option<bool> {
    if x.hi < y.lo {
        Some(true)
    } else if x.lo >= y.hi {
        Some(false)
    } else {
        None
    }
}

pub(crate) fn cmp_le(x: Interval, y: Interval) -> Option<bool> {
    if x.hi <= y.lo {
        Some(true)
    } else if x.lo > y.hi {
        Some(false)
    } else {
        None
    }
}

/// Which arm of a many-valued variation to bound, as `(index, count)`.
///
/// The body picks its arm with `floor(count · rng_nextf())`, so
/// pinning the draw to that arm's slice pins the arm. The slice is
/// taken a hundredth in from each end: the evaluator widens every
/// interval it produces, and an arm boundary is exactly where that
/// widening would cross into the next arm and hand back two.
pub type Branch = (u32, u32);

pub(crate) fn rng_for(branch: Option<Branch>) -> Interval {
    match branch {
        Some((k, n)) if n > 0 => {
            let n = n as f64;
            Interval::new((k as f64 + 0.01) / n, (k as f64 + 0.99) / n)
        }
        _ => Interval::new(0.0, 1.0),
    }
}

pub(crate) fn box_to_disc(out: [Interval; 2]) -> Result<Ball, Refusal> {
    let (x, y) = (out[0], out[1]);
    if !x.finite() || !y.finite() {
        return Err(Refusal::Unbounded);
    }
    let cx = 0.5 * (x.lo + x.hi);
    let cy = 0.5 * (y.lo + y.hi);
    let hx = 0.5 * (x.hi - x.lo);
    let hy = 0.5 * (y.hi - y.lo);
    Ok(Ball::new([cx, cy], (hx * hx + hy * hy).sqrt()))
}

