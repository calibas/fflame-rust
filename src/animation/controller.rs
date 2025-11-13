//! Animation playback controller

use super::{Animation, EasingFunction, Interpolation, Keyframe, LoopMode, PlaybackState, Track};
use crate::config::{ConfigPath, ConfigValue};
use std::collections::HashMap;

/// Controls animation playback and evaluates frame values
pub struct AnimationController {
    /// Currently loaded animation (if any)
    pub animation: Option<Animation>,

    /// Playback state
    pub state: PlaybackState,

    /// Current time in seconds from animation start
    pub current_time: f64,

    /// Playback speed multiplier (1.0 = normal speed)
    pub speed: f64,
}

impl AnimationController {
    /// Create new controller with no animation loaded
    pub fn new() -> Self {
        Self {
            animation: None,
            state: PlaybackState::Stopped,
            current_time: 0.0,
            speed: 1.0,
        }
    }

    /// Load animation and reset playback
    pub fn load(&mut self, animation: Animation) {
        self.animation = Some(animation);
        self.current_time = 0.0;
        self.state = PlaybackState::Stopped;
    }

    /// Start playback
    pub fn play(&mut self) {
        if self.animation.is_some() {
            self.state = PlaybackState::Playing;
        }
    }

    /// Pause playback
    pub fn pause(&mut self) {
        self.state = PlaybackState::Paused;
    }

    /// Stop playback and reset to start
    pub fn stop(&mut self) {
        self.state = PlaybackState::Stopped;
        self.current_time = 0.0;
    }

    /// Update animation time (called each frame)
    pub fn update(&mut self, delta_time: f64) {
        if self.state != PlaybackState::Playing {
            return;
        }

        let Some(ref animation) = self.animation else {
            return;
        };

        self.current_time += delta_time * self.speed;

        // Handle loop modes
        match animation.loop_mode {
            LoopMode::Once => {
                if self.current_time >= animation.duration {
                    self.current_time = animation.duration;
                    self.state = PlaybackState::Stopped;
                }
            }
            LoopMode::Loop => {
                if self.current_time >= animation.duration {
                    self.current_time = self.current_time % animation.duration;
                }
            }
            LoopMode::PingPong => {
                // TODO: Implement ping-pong (requires direction tracking)
                // For now, just loop
                if self.current_time >= animation.duration {
                    self.current_time = self.current_time % animation.duration;
                }
            }
        }
    }

    /// Seek to specific time
    pub fn seek(&mut self, time: f64) {
        if let Some(ref animation) = self.animation {
            self.current_time = time.clamp(0.0, animation.duration);
        }
    }

    /// Evaluate all tracks at current time
    ///
    /// Returns map of ConfigPath → ConfigValue for parameters that should be updated
    pub fn evaluate_frame(&self) -> HashMap<String, serde_json::Value> {
        let Some(ref animation) = self.animation else {
            return HashMap::new();
        };

        let mut values = HashMap::new();

        for (path_str, track) in &animation.tracks {
            if let Some(value) = Self::evaluate_track(track, self.current_time) {
                values.insert(path_str.clone(), value);
            }
        }

        values
    }

    /// Evaluate single track at specific time
    fn evaluate_track(track: &Track, time: f64) -> Option<serde_json::Value> {
        if track.keyframes.is_empty() {
            return None;
        }

        // Single keyframe: constant value
        if track.keyframes.len() == 1 {
            return Some(track.keyframes[0].value.clone());
        }

        // Before first keyframe: hold first value
        if time <= track.keyframes[0].time {
            return Some(track.keyframes[0].value.clone());
        }

        // After last keyframe: hold last value
        let last_idx = track.keyframes.len() - 1;
        if time >= track.keyframes[last_idx].time {
            return Some(track.keyframes[last_idx].value.clone());
        }

        // Find surrounding keyframes
        for i in 0..track.keyframes.len() - 1 {
            let kf0 = &track.keyframes[i];
            let kf1 = &track.keyframes[i + 1];

            if time >= kf0.time && time <= kf1.time {
                // Interpolate between kf0 and kf1
                return Some(Self::interpolate_keyframes(kf0, kf1, time, track.interpolation));
            }
        }

        None
    }

    /// Interpolate between two keyframes
    fn interpolate_keyframes(
        kf0: &Keyframe,
        kf1: &Keyframe,
        time: f64,
        interpolation: Interpolation,
    ) -> serde_json::Value {
        // Calculate normalized time [0, 1] between keyframes
        let duration = kf1.time - kf0.time;
        let t = if duration > 0.0 {
            (time - kf0.time) / duration
        } else {
            0.0
        };

        match interpolation {
            Interpolation::Step => {
                // Jump to next value at halfway point
                if t < 0.5 {
                    kf0.value.clone()
                } else {
                    kf1.value.clone()
                }
            }
            Interpolation::Linear | Interpolation::Smooth => {
                // Apply easing function
                let t_eased = kf0.easing.apply(t);

                // Interpolate based on value type
                Self::lerp_json_value(&kf0.value, &kf1.value, t_eased)
            }
        }
    }

    /// Linear interpolation between JSON values
    fn lerp_json_value(a: &serde_json::Value, b: &serde_json::Value, t: f64) -> serde_json::Value {
        use serde_json::Value;

        match (a, b) {
            (Value::Number(a), Value::Number(b)) => {
                // Try as f64
                if let (Some(a_f), Some(b_f)) = (a.as_f64(), b.as_f64()) {
                    let result = a_f + (b_f - a_f) * t;
                    return Value::Number(serde_json::Number::from_f64(result).unwrap());
                }
                // Try as u64
                if let (Some(a_u), Some(b_u)) = (a.as_u64(), b.as_u64()) {
                    let result = (a_u as f64 + (b_u as f64 - a_u as f64) * t).round() as u64;
                    return Value::Number(result.into());
                }
                // Fallback: step interpolation
                if t < 0.5 { a.clone() } else { b.clone() }.into()
            }
            (Value::Bool(a), Value::Bool(b)) => {
                // Boolean: threshold at 0.5
                Value::Bool(if t < 0.5 { *a } else { *b })
            }
            _ => {
                // Non-numeric types: step interpolation
                if t < 0.5 {
                    a.clone()
                } else {
                    b.clone()
                }
            }
        }
    }
}

impl Default for AnimationController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_controller_update() {
        let mut controller = AnimationController::new();
        let animation = Animation::new("Test".into(), 10.0);
        controller.load(animation);

        controller.play();
        assert_eq!(controller.state, PlaybackState::Playing);

        controller.update(1.0);
        assert_eq!(controller.current_time, 1.0);

        controller.update(2.0);
        assert_eq!(controller.current_time, 3.0);
    }

    #[test]
    fn test_loop_mode_once() {
        let mut controller = AnimationController::new();
        let animation = Animation::new("Test".into(), 5.0);
        controller.load(animation);
        controller.play();

        controller.update(6.0); // Exceed duration
        assert_eq!(controller.current_time, 5.0);
        assert_eq!(controller.state, PlaybackState::Stopped);
    }

    #[test]
    fn test_track_evaluation() {
        use super::super::Track;

        let track = Track::linear(
            serde_json::json!(0.0),
            serde_json::json!(10.0),
            10.0,
        );

        // At start
        let val = AnimationController::evaluate_track(&track, 0.0).unwrap();
        assert_eq!(val.as_f64().unwrap(), 0.0);

        // At middle
        let val = AnimationController::evaluate_track(&track, 5.0).unwrap();
        assert_eq!(val.as_f64().unwrap(), 5.0);

        // At end
        let val = AnimationController::evaluate_track(&track, 10.0).unwrap();
        assert_eq!(val.as_f64().unwrap(), 10.0);
    }
}
