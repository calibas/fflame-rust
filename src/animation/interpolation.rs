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
            Interpolation::Linear => t,
            // Smooth: Hermite/smoothstep curve (3t² - 2t³)
            Interpolation::Smooth => t * t * (3.0 - 2.0 * t),
            // Sinusoidal: sine-based S-curve
            Interpolation::Sinusoidal => (1.0 - (t * PI).cos()) / 2.0,
        }
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
}
