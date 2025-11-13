//! Interpolation and easing functions for animation

use serde::{Deserialize, Serialize};

/// Interpolation method between keyframes
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Interpolation {
    /// Jump directly to value (no interpolation)
    Step,

    /// Linear interpolation
    Linear,

    /// Smooth cubic interpolation
    Smooth,
}

/// Easing function for smooth transitions
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum EasingFunction {
    Linear,
    EaseIn,
    EaseOut,
    EaseInOut,
    EaseInQuad,
    EaseOutQuad,
    EaseInOutQuad,
    EaseInCubic,
    EaseOutCubic,
    EaseInOutCubic,
}

impl EasingFunction {
    /// Apply easing to normalized time [0, 1] → [0, 1]
    pub fn apply(&self, t: f64) -> f64 {
        let t = t.clamp(0.0, 1.0);
        match self {
            EasingFunction::Linear => t,
            EasingFunction::EaseIn => t * t,
            EasingFunction::EaseOut => 1.0 - (1.0 - t) * (1.0 - t),
            EasingFunction::EaseInOut => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - 2.0 * (1.0 - t) * (1.0 - t)
                }
            }
            EasingFunction::EaseInQuad => t * t,
            EasingFunction::EaseOutQuad => 1.0 - (1.0 - t) * (1.0 - t),
            EasingFunction::EaseInOutQuad => {
                if t < 0.5 {
                    2.0 * t * t
                } else {
                    1.0 - 2.0 * (1.0 - t) * (1.0 - t)
                }
            }
            EasingFunction::EaseInCubic => t * t * t,
            EasingFunction::EaseOutCubic => 1.0 - (1.0 - t).powi(3),
            EasingFunction::EaseInOutCubic => {
                if t < 0.5 {
                    4.0 * t * t * t
                } else {
                    1.0 - (-2.0 * t + 2.0).powi(3) / 2.0
                }
            }
        }
    }
}

/// Interpolate between two f64 values
pub fn lerp_f64(a: f64, b: f64, t: f64) -> f64 {
    a + (b - a) * t
}

/// Interpolate between two u32 values
pub fn lerp_u32(a: u32, b: u32, t: f64) -> u32 {
    (a as f64 + (b as f64 - a as f64) * t).round() as u32
}

/// Interpolate between two bool values (threshold at 0.5)
pub fn lerp_bool(a: bool, b: bool, t: f64) -> bool {
    if t < 0.5 { a } else { b }
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
    fn test_lerp() {
        assert_eq!(lerp_f64(0.0, 10.0, 0.0), 0.0);
        assert_eq!(lerp_f64(0.0, 10.0, 0.5), 5.0);
        assert_eq!(lerp_f64(0.0, 10.0, 1.0), 10.0);
    }

    #[test]
    fn test_lerp_u32() {
        assert_eq!(lerp_u32(0, 100, 0.0), 0);
        assert_eq!(lerp_u32(0, 100, 0.5), 50);
        assert_eq!(lerp_u32(0, 100, 1.0), 100);
    }
}
