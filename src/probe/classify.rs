//! Reducing a shader output to something two GPUs can be compared on.
//!
//! # The two signals, and why they are separate
//!
//! Comparing raw `f32` across vendors is a non-signal. Different GPUs
//! legitimately differ in `sin`/`cos`/`exp`, in whether a multiply-add
//! contracts to an FMA, and in reassociation — all permitted. Hash the
//! bits and every variation goes red on any other GPU, which tells you
//! exactly as much as a golden image that fails everywhere.
//!
//! So each output carries two independent signals:
//!
//! **[`Class`] — the hard signal.** Purely categorical: is it zero,
//! finite, NaN, infinite, past the bad-value threshold. No arithmetic
//! noise can move a value between these buckets, so a class difference
//! is a real behavioural difference, not tolerance. This is what the
//! Metal bugs actually look like — `x != x` false for NaN, `Inf/Inf`
//! landing on 1.0, `atan2(0,0)` returning π/4 where IEEE gives a signed
//! zero. Every one of those is a class change.
//!
//! **The quantised digest — the soft signal.** Magnitudes rounded to a
//! relative 1e-4, hashed. A mismatch means the numbers moved by more
//! than any reasonable vendor difference. Worth reading; not on its own
//! a bug, because a value sitting on a rounding boundary can flip
//! buckets for no interesting reason.
//!
//! Keeping them apart is the whole point. Merged into one hash, the
//! soft signal's false positives would drown the hard signal's real
//! ones, and the report would get ignored the way a noisy test does.

/// What kind of number came out, at a granularity no rounding
/// difference can perturb.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Class {
    /// Exactly `+0.0`.
    PosZero,
    /// Exactly `-0.0`. Distinct from [`Class::PosZero`] on purpose:
    /// `atan2` is specified in terms of the sign of a zero, and that
    /// distinction is precisely what Metal's fast-math loses.
    NegZero,
    /// Non-zero but under 1e-30. Unsigned, because a value this close
    /// to zero can land either side of it from rounding alone, and a
    /// sign here would be noise rather than signal.
    NearZero,
    /// An ordinary positive value.
    Pos,
    /// An ordinary negative value.
    Neg,
    /// Finite but past 1e32 — the threshold `main_template.wgsl`'s
    /// bad-value recovery uses. A value here is one the renderer would
    /// treat as divergent, so crossing into it changes what is drawn.
    HugePos,
    /// Finite but past -1e32. See [`Class::HugePos`].
    HugeNeg,
    PosInf,
    NegInf,
    Nan,
}

/// The project's bad-value threshold, from `main_template.wgsl`.
const HUGE: f32 = 1e32;

/// Below this, sign is rounding noise rather than information.
const NEAR_ZERO: f32 = 1e-30;

impl Class {
    pub fn of(x: f32) -> Self {
        if x.is_nan() {
            return Class::Nan;
        }
        if x.is_infinite() {
            return if x > 0.0 { Class::PosInf } else { Class::NegInf };
        }
        if x == 0.0 {
            // `x > 0.0` is false for both zeros, so read the sign bit —
            // the same reason the `npolar` guard uses `bitcast<u32>`.
            return if x.is_sign_positive() {
                Class::PosZero
            } else {
                Class::NegZero
            };
        }
        let a = x.abs();
        if a > HUGE {
            return if x > 0.0 { Class::HugePos } else { Class::HugeNeg };
        }
        if a < NEAR_ZERO {
            return Class::NearZero;
        }
        if x > 0.0 {
            Class::Pos
        } else {
            Class::Neg
        }
    }

    /// One character, so a whole variation's behaviour is a short
    /// string that a plain `diff` localises to the input that moved.
    pub fn glyph(self) -> char {
        match self {
            Class::PosZero => '0',
            Class::NegZero => 'o',
            Class::NearZero => 'z',
            Class::Pos => 'p',
            Class::Neg => 'm',
            Class::HugePos => 'H',
            Class::HugeNeg => 'h',
            Class::PosInf => 'I',
            Class::NegInf => 'i',
            Class::Nan => 'n',
        }
    }

    pub fn from_glyph(c: char) -> Option<Self> {
        Some(match c {
            '0' => Class::PosZero,
            'o' => Class::NegZero,
            'z' => Class::NearZero,
            'p' => Class::Pos,
            'm' => Class::Neg,
            'H' => Class::HugePos,
            'h' => Class::HugeNeg,
            'I' => Class::PosInf,
            'i' => Class::NegInf,
            'n' => Class::Nan,
            _ => return None,
        })
    }
}

/// One probed output value, reduced to both signals.
#[derive(Debug, Clone, Copy)]
pub struct Sample {
    pub class: Class,
    /// The quantised magnitude, or `None` when the value has no
    /// meaningful magnitude (zero, infinite, NaN).
    pub quantised: Option<i64>,
}

/// Relative granularity of the soft signal. Chosen well above the
/// vendor differences it must tolerate (transcendental implementations
/// differ in the last few ulp, ~1e-7 relative for `f32`) and well below
/// a difference worth reading.
const QUANT_RELATIVE: f64 = 1e-4;

impl Sample {
    pub fn of(x: f32) -> Self {
        let class = Class::of(x);
        let quantised = match class {
            Class::Nan | Class::PosInf | Class::NegInf | Class::PosZero | Class::NegZero => None,
            _ => Some(quantise(x)),
        };
        Sample { class, quantised }
    }
}

/// Round to a relative grid, so the step scales with the value.
///
/// A fixed absolute grid would be hopeless across the range these
/// outputs span — the same absolute step is far below the noise at
/// 1e30 and far above the whole value at 1e-20.
fn quantise(x: f32) -> i64 {
    let x = x as f64;
    let a = x.abs();
    // log-domain grid: index = round(ln|x| / ln(1 + step)), signed.
    let index = (a.ln() / (1.0 + QUANT_RELATIVE).ln()).round() as i64;
    if x < 0.0 {
        -index
    } else {
        index
    }
}

/// Every class, in the fixed order the presence mask uses.
pub const ALPHABET: [Class; 10] = [
    Class::PosZero,
    Class::NegZero,
    Class::NearZero,
    Class::Pos,
    Class::Neg,
    Class::HugePos,
    Class::HugeNeg,
    Class::PosInf,
    Class::NegInf,
    Class::Nan,
];

/// Marks a class the group did not produce.
pub const ABSENT: char = '.';

/// Which classes a group of samples contains, as a fixed-width string:
/// the class's glyph where present, [`ABSENT`] where not.
///
/// This is how the parameter sweep collapses 27 inputs into one field.
/// Recording a class per input would multiply that report by 27; picking
/// a single "most notable" class needs a ranking, and any ranking is
/// wrong somewhere — ordering zero above ordinary finites made every
/// variation that returns zero at one input read as returning zero
/// everywhere, which flagged 1103 sweep entries as producing no output
/// when they produce plenty.
///
/// A presence mask needs no ranking and loses nothing about *which*
/// kinds occurred. What it does lose is which input produced which, and
/// the split between components — both stated in the report.
pub fn class_mask(samples: impl IntoIterator<Item = Class>) -> String {
    let mut seen = [false; ALPHABET.len()];
    for c in samples {
        if let Some(i) = ALPHABET.iter().position(|a| *a == c) {
            seen[i] = true;
        }
    }
    ALPHABET
        .iter()
        .zip(seen)
        .map(|(c, present)| if present { c.glyph() } else { ABSENT })
        .collect()
}

/// Fold samples into the two report fields: the class string and the
/// quantised digest.
pub fn summarise(samples: &[Sample]) -> (String, u64) {
    let glyphs: String = samples.iter().map(|s| s.class.glyph()).collect();

    // FNV-1a: stable across platforms and across Rust versions, which
    // `DefaultHasher` explicitly is not — and this digest is committed
    // and compared between machines.
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    let mut eat = |b: u8| {
        h ^= b as u64;
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    };
    for s in samples {
        eat(s.class.glyph() as u8);
        // Distinguish "no magnitude" from a magnitude that happens to
        // quantise to zero, so an Inf and a 1.0 cannot collide.
        match s.quantised {
            None => eat(0xff),
            Some(q) => {
                eat(0x01);
                for b in q.to_le_bytes() {
                    eat(b);
                }
            }
        }
    }
    (glyphs, h)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn signed_zeros_are_distinguished() {
        // The whole reason `npolar` broke: IEEE gives atan2 a different
        // answer for +0 and -0, and a classifier that folds them
        // together cannot see the bug it exists to catch.
        assert_eq!(Class::of(0.0), Class::PosZero);
        assert_eq!(Class::of(-0.0), Class::NegZero);
        assert_ne!(Class::of(0.0), Class::of(-0.0));
    }

    #[test]
    fn the_metal_divergences_all_show_up_as_class_changes() {
        // atan2(0,0): IEEE +0 vs Metal's pi/4.
        assert_ne!(Class::of(0.0), Class::of(std::f32::consts::FRAC_PI_4));
        // Inf/Inf: IEEE NaN vs Metal's 1.0.
        assert_ne!(Class::of(f32::NAN), Class::of(1.0));
        // A value that escaped past the bad-value threshold vs one that
        // did not.
        assert_ne!(Class::of(1e33), Class::of(1e30));
    }

    #[test]
    fn ulp_noise_does_not_change_the_class_or_the_digest() {
        // What a different vendor's `sin` legitimately does.
        let a = 0.841_470_98_f32;
        let b = f32::from_bits(a.to_bits() + 3);
        assert_ne!(a, b, "test needs two genuinely different floats");
        assert_eq!(Class::of(a), Class::of(b));
        assert_eq!(
            summarise(&[Sample::of(a)]).1,
            summarise(&[Sample::of(b)]).1,
            "a few ulp must not move the soft signal either"
        );
    }

    #[test]
    fn a_real_numeric_difference_does_change_the_digest() {
        let (_, a) = summarise(&[Sample::of(1.0)]);
        let (_, b) = summarise(&[Sample::of(1.01)]);
        assert_ne!(a, b, "1% is far past tolerance and must be caught");
    }

    #[test]
    fn near_zero_is_unsigned_so_a_noise_sign_flip_is_not_a_failure() {
        assert_eq!(Class::of(1e-35), Class::of(-1e-35));
        assert_eq!(Class::of(1e-35), Class::NearZero);
    }

    #[test]
    fn inf_and_a_finite_value_cannot_collide_in_the_digest() {
        assert_ne!(
            summarise(&[Sample::of(f32::INFINITY)]).1,
            summarise(&[Sample::of(1.0)]).1
        );
    }

    #[test]
    fn the_mask_is_fixed_width_and_shows_every_class_present() {
        let mask = class_mask([Class::Pos, Class::Nan, Class::Pos]);
        assert_eq!(mask.chars().count(), ALPHABET.len());
        assert!(mask.contains('p') && mask.contains('n'));
        assert!(!mask.contains('m'), "a class not produced must not appear");
        assert_eq!(class_mask(std::iter::empty()), ".".repeat(ALPHABET.len()));
    }

    #[test]
    fn a_group_that_is_mostly_finite_does_not_read_as_all_zero() {
        // The failure the mask replaced: ranking zero above ordinary
        // values made one zero input mask 26 real results.
        let mask = class_mask([Class::Pos, Class::PosZero, Class::Neg]);
        assert!(mask.contains('p') && mask.contains('m') && mask.contains('0'));
    }

    #[test]
    fn the_mask_is_order_independent() {
        assert_eq!(
            class_mask([Class::Nan, Class::Pos]),
            class_mask([Class::Pos, Class::Nan]),
            "which input came first must not change the record"
        );
    }

    #[test]
    fn glyphs_round_trip() {
        for c in [
            Class::PosZero,
            Class::NegZero,
            Class::NearZero,
            Class::Pos,
            Class::Neg,
            Class::HugePos,
            Class::HugeNeg,
            Class::PosInf,
            Class::NegInf,
            Class::Nan,
        ] {
            assert_eq!(Class::from_glyph(c.glyph()), Some(c));
        }
    }

    #[test]
    fn the_digest_is_stable_across_runs_and_builds() {
        // FNV-1a over a fixed input has one right answer. If this ever
        // changes, every committed report is invalidated — which is
        // exactly the sort of silent drift a pinned value catches.
        let samples: Vec<Sample> = [0.0f32, -0.0, 1.0, -1.0, f32::NAN, f32::INFINITY]
            .iter()
            .map(|&x| Sample::of(x))
            .collect();
        let (glyphs, digest) = summarise(&samples);
        assert_eq!(glyphs, "0opmnI");
        assert_eq!(digest, 0xc7fc_ee6a_b5ff_a040);
    }
}
