//! Interpolation and easing functions for animation

use serde::{Deserialize, Serialize};

/// Interpolation method between keyframes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum Interpolation {
    /// Jump directly to value (no interpolation)
    Step,

    /// Linear interpolation
    #[default]
    Linear,

    /// Smooth cubic interpolation (Catmull-Rom style)
    Smooth,

    /// Sinusoidal interpolation (smooth S-curve using sine)
    Sinusoidal,

    /// Geometric interpolation — equal RATIO per unit time instead of
    /// equal difference. The curve for anything perceived
    /// multiplicatively, zoom above all: linearly interpolating zoom
    /// 1 → 100 spends its first half getting to 50.5 (five and a half
    /// doublings) and its second half on less than one doubling — a
    /// dive that slams the brakes. Exponential makes every doubling
    /// take the same time, and works identically in both directions
    /// (zooming out is the same curve run backward).
    ///
    /// The value math lives at the keyframe-lerp site, not in
    /// [`Interpolation::apply`] (which stays identity here, like
    /// Linear): the ratio a·(b/a)^t needs both endpoint VALUES, and
    /// `apply` only sees time. Values whose signs differ (or zeros)
    /// fall back to linear — there is no ratio through zero.
    Exponential,
}

/// Easing function for smooth transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum EasingFunction {
    #[default]
    Linear,
    // Quadratic (power of 2)
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    // Cubic (power of 3)
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
    // Sine-based (smoother, more natural)
    EaseInSine,
    EaseOutSine,
    EaseInOutSine,
}

impl EasingFunction {
    /// Apply easing to normalized time [0, 1] → [0, 1]
    pub fn apply(&self, t: f64) -> f64 {
        use std::f64::consts::PI;

        let t = t.clamp(0.0, 1.0);
        match self {
            EasingFunction::Linear => t,

            // Quadratic (same as EaseInQuad/EaseOutQuad for compatibility)
            EasingFunction::EaseIn | EasingFunction::EaseInQuad => t * t,
            EasingFunction::EaseOut | EasingFunction::EaseOutQuad => 1.0 - (1.0 - t) * (1.0 - t),
            EasingFunction::EaseInOut | EasingFunction::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - 2.0 * (1.0 - t) * (1.0 - t)
                }
            }

            // Cubic
            EasingFunction::EaseInCubic => t * t * t,
            EasingFunction::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            EasingFunction::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }

            // Sine-based (very smooth, natural motion)
            EasingFunction::EaseInSine => 1.0 - (t * PI / 2.0).cos(),
            EasingFunction::EaseOutSine => (t * PI / 2.0).sin(),
            EasingFunction::EaseInOutSine => -(((t * PI).cos() - 1.0) / 2.0),
        }
    }
}

impl Interpolation {
    /// Apply interpolation curve to normalized time [0, 1] → [0, 1]
    /// For Step: returns 0.0 or 1.0
    /// For Linear: returns t unchanged
    /// For Smooth/Sinusoidal: applies smoothing curve
    pub fn apply(&self, t: f64) -> f64 {
        use std::f64::consts::PI;

        let t = t.clamp(0.0, 1.0);
        match self {
            Interpolation::Step => if t < 0.5 { 0.0 } else { 1.0 },
            // Exponential is identity in TIME — its curve lives in value
            // space (see geometric_lerp), which apply() cannot reach.
            Interpolation::Linear | Interpolation::Exponential => t,
            // Smooth: Hermite/smoothstep curve (3t² - 2t³)
            Interpolation::Smooth => t * t * (3.0 - 2.0 * t),
            // Sinusoidal: sine-based S-curve
            Interpolation::Sinusoidal => (1.0 - (t * PI).cos()) / 2.0,
        }
    }
}

/// Geometric (equal-ratio) interpolation between two values, for
/// [`Interpolation::Exponential`]. Falls back to linear when the
/// endpoints do not share a nonzero sign — a ratio through zero does
/// not exist, and a silent linear segment beats a NaN.
pub fn geometric_lerp(a: f64, b: f64, t: f64) -> f64 {
    if (a > 0.0 && b > 0.0) || (a < 0.0 && b < 0.0) {
        a * (b / a).powf(t)
    } else {
        a + (b - a) * t
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_easing() {
        assert_eq!(EasingFunction::Linear.apply(0.0), 0.0);
        assert_eq!(EasingFunction::Linear.apply(0.5), 0.5);
        assert_eq!(EasingFunction::Linear.apply(1.0), 1.0);
    }

    #[test]
    fn test_ease_in() {
        let result = EasingFunction::EaseIn.apply(0.5);
        assert!(result < 0.5); // Should accelerate (slower at start)
    }

    #[test]
    fn test_ease_out() {
        let result = EasingFunction::EaseOut.apply(0.5);
        assert!(result > 0.5); // Should decelerate (faster at start)
    }

    #[test]
    fn test_ease_in_sine() {
        let result = EasingFunction::EaseInSine.apply(0.5);
        assert!(result < 0.5); // Should accelerate (slower at start)
        // At t=0, should be 0
        assert!((EasingFunction::EaseInSine.apply(0.0) - 0.0).abs() < 1e-10);
        // At t=1, should be 1
        assert!((EasingFunction::EaseInSine.apply(1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_ease_out_sine() {
        let result = EasingFunction::EaseOutSine.apply(0.5);
        assert!(result > 0.5); // Should decelerate (faster at start)
        assert!((EasingFunction::EaseOutSine.apply(0.0) - 0.0).abs() < 1e-10);
        assert!((EasingFunction::EaseOutSine.apply(1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_ease_in_out_sine() {
        let result = EasingFunction::EaseInOutSine.apply(0.5);
        // At midpoint, should be exactly 0.5 for symmetric easing
        assert!((result - 0.5).abs() < 1e-10);
        assert!((EasingFunction::EaseInOutSine.apply(0.0) - 0.0).abs() < 1e-10);
        assert!((EasingFunction::EaseInOutSine.apply(1.0) - 1.0).abs() < 1e-10);
    }

    #[test]
    fn test_interpolation_linear() {
        assert_eq!(Interpolation::Linear.apply(0.0), 0.0);
        assert_eq!(Interpolation::Linear.apply(0.5), 0.5);
        assert_eq!(Interpolation::Linear.apply(1.0), 1.0);
    }

    #[test]
    fn test_interpolation_step() {
        assert_eq!(Interpolation::Step.apply(0.0), 0.0);
        assert_eq!(Interpolation::Step.apply(0.49), 0.0);
        assert_eq!(Interpolation::Step.apply(0.5), 1.0);
        assert_eq!(Interpolation::Step.apply(1.0), 1.0);
    }

    #[test]
    fn test_interpolation_smooth() {
        // Smooth should be 0 at 0, 1 at 1, and 0.5 at 0.5
        assert!((Interpolation::Smooth.apply(0.0) - 0.0).abs() < 1e-10);
        assert!((Interpolation::Smooth.apply(0.5) - 0.5).abs() < 1e-10);
        assert!((Interpolation::Smooth.apply(1.0) - 1.0).abs() < 1e-10);
        // Should be slower at edges (derivative = 0 at t=0 and t=1)
        let early = Interpolation::Smooth.apply(0.1);
        assert!(early < 0.1); // Slower at start
    }

    #[test]
    fn test_interpolation_sinusoidal() {
        assert!((Interpolation::Sinusoidal.apply(0.0) - 0.0).abs() < 1e-10);
        assert!((Interpolation::Sinusoidal.apply(0.5) - 0.5).abs() < 1e-10);
        assert!((Interpolation::Sinusoidal.apply(1.0) - 1.0).abs() < 1e-10);
    }

    /// Equal ratio per unit time: the midpoint of 1 → 4 is 2 (one
    /// doubling done, one to go), not the linear 2.5.
    #[test]
    fn geometric_lerp_moves_by_equal_ratio() {
        assert!((geometric_lerp(1.0, 4.0, 0.0) - 1.0).abs() < 1e-12);
        assert!((geometric_lerp(1.0, 4.0, 0.5) - 2.0).abs() < 1e-12);
        assert!((geometric_lerp(1.0, 4.0, 1.0) - 4.0).abs() < 1e-12);

        // The motivating case: zoom 1 → 100 linearly spends its first
        // half on five and a half doublings. Geometrically the halfway
        // value is 10 — half the doublings done, half to go.
        assert!((geometric_lerp(1.0, 100.0, 0.5) - 10.0).abs() < 1e-9);
    }

    /// "Both ways": zooming out is the same curve run backward, exactly.
    #[test]
    fn geometric_lerp_is_symmetric_in_direction() {
        for t in [0.0, 0.2, 0.5, 0.8, 1.0] {
            let dive = geometric_lerp(1.0, 50.0, t);
            let pull = geometric_lerp(50.0, 1.0, 1.0 - t);
            assert!(
                (dive - pull).abs() < 1e-9 * dive,
                "t={t}: dive {dive} != reversed pull {pull}"
            );
        }
        // Negative pairs work in their own sign domain.
        assert!((geometric_lerp(-1.0, -4.0, 0.5) - -2.0).abs() < 1e-12);
    }

    /// No ratio exists through zero — those pairs fall back to linear
    /// instead of producing NaN.
    #[test]
    fn geometric_lerp_falls_back_to_linear_through_zero() {
        assert!((geometric_lerp(0.0, 4.0, 0.5) - 2.0).abs() < 1e-12);
        assert!((geometric_lerp(-2.0, 2.0, 0.5) - 0.0).abs() < 1e-12);
        assert!(geometric_lerp(-1.0, 3.0, 0.25).is_finite());
    }
}
