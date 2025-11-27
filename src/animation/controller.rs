//! Animation playback controller

use super::{Animation, AnimationQualityMode, Interpolation, Keyframe, LoopMode, OscillatorType, PlaybackState, Track, TrackSource};

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

    /// Direction for PingPong mode (1.0 = forward, -1.0 = backward)
    direction: f64,

    /// Quality mode for animation rendering
    pub quality_mode: AnimationQualityMode,
}

impl AnimationController {
    /// Create new controller with no animation loaded
    pub fn new() -> Self {
        Self {
            animation: None,
            state: PlaybackState::Stopped,
            current_time: 0.0,
            speed: 1.0,
            direction: 1.0,
            quality_mode: AnimationQualityMode::default(),
        }
    }

    /// Check if animation should use overwrite mode (vs batched accumulation)
    ///
    /// Returns true for Responsive mode (immediate updates),
    /// false for HighQuality mode (batched accumulation)
    pub fn use_overwrite_mode(&self) -> bool {
        self.quality_mode == AnimationQualityMode::Responsive
    }

    /// Load animation and reset playback
    pub fn load(&mut self, animation: Animation) {
        self.animation = Some(animation);
        self.current_time = 0.0;
        self.state = PlaybackState::Stopped;
        self.direction = 1.0;
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
        self.direction = 1.0;
    }

    /// Check if animation is currently playing
    pub fn is_playing(&self) -> bool {
        self.state == PlaybackState::Playing
    }

    /// Get current playback direction (1.0 = forward, -1.0 = backward)
    pub fn direction(&self) -> f64 {
        self.direction
    }

    /// Update animation time (called each frame)
    pub fn update(&mut self, delta_time: f64) {
        if self.state != PlaybackState::Playing {
            return;
        }

        let Some(ref animation) = self.animation else {
            return;
        };

        self.current_time += delta_time * self.speed * self.direction;

        // Handle loop modes
        match animation.loop_mode {
            LoopMode::Once => {
                if self.current_time >= animation.duration {
                    // Animation finished - reset to start
                    self.current_time = 0.0;
                    self.state = PlaybackState::Stopped;
                    self.direction = 1.0;
                } else if self.current_time < 0.0 {
                    self.current_time = 0.0;
                    self.state = PlaybackState::Stopped;
                    self.direction = 1.0;
                }
            }
            LoopMode::Loop => {
                if self.current_time >= animation.duration {
                    self.current_time = self.current_time % animation.duration;
                } else if self.current_time < 0.0 {
                    self.current_time = animation.duration + (self.current_time % animation.duration);
                }
            }
            LoopMode::PingPong => {
                if self.current_time >= animation.duration {
                    // Hit end, reverse direction
                    self.current_time = animation.duration - (self.current_time - animation.duration);
                    self.direction = -1.0;
                } else if self.current_time < 0.0 {
                    // Hit start, reverse direction
                    self.current_time = -self.current_time;
                    self.direction = 1.0;
                }
                // Clamp in case of overshoot
                self.current_time = self.current_time.clamp(0.0, animation.duration);
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
    /// Returns vec of (path_string, value) pairs for parameters that should be updated
    pub fn evaluate_frame(&self) -> Vec<(String, serde_json::Value)> {
        self.evaluate_at_time(self.current_time)
    }

    /// Evaluate all tracks at a specific time (for export)
    ///
    /// This is the core evaluation function used by both real-time playback
    /// and animation export. Returns vec of (path_string, value) pairs.
    pub fn evaluate_at_time(&self, time: f64) -> Vec<(String, serde_json::Value)> {
        let Some(ref animation) = self.animation else {
            return Vec::new();
        };

        let mut values = Vec::new();

        // Evaluate regular tracks
        for (path_str, track) in &animation.tracks {
            if let Some(value) = self.evaluate_track(track, time) {
                values.push((path_str.clone(), value));
            }
        }

        // Evaluate circular tracks (output X and Y)
        for circular in &animation.circular_tracks {
            let (x, y) = circular.evaluate(time);
            values.push((circular.target_x.clone(), serde_json::json!(x)));
            values.push((circular.target_y.clone(), serde_json::json!(y)));
        }

        values
    }

    /// Evaluate single track at specific time
    fn evaluate_track(&self, track: &Track, time: f64) -> Option<serde_json::Value> {
        match &track.source {
            TrackSource::Keyframes { keyframes } => {
                self.evaluate_keyframes(keyframes, time, track.interpolation)
            }
            TrackSource::Oscillator { oscillator_type, center, amplitude, frequency, phase } => {
                Some(self.evaluate_oscillator(*oscillator_type, *center, *amplitude, *frequency, *phase, time))
            }
        }
    }

    /// Evaluate oscillator at given time
    fn evaluate_oscillator(
        &self,
        oscillator_type: OscillatorType,
        center: f64,
        amplitude: f64,
        frequency: f64,
        phase: f64,
        time: f64,
    ) -> serde_json::Value {
        use std::f64::consts::PI;

        let t = time * frequency + phase;
        let wave = match oscillator_type {
            OscillatorType::Sine => (t * 2.0 * PI).sin(),
            OscillatorType::Triangle => {
                // Triangle wave: linear up from -1 to 1, then down
                let t_mod = t - t.floor(); // 0 to 1
                if t_mod < 0.5 {
                    4.0 * t_mod - 1.0 // -1 to 1
                } else {
                    3.0 - 4.0 * t_mod // 1 to -1
                }
            }
            OscillatorType::Sawtooth => {
                // Sawtooth: linear ramp from -1 to 1, then reset
                2.0 * (t - t.floor()) - 1.0
            }
            OscillatorType::Square => {
                // Square wave: -1 or 1
                if t.fract() < 0.5 { -1.0 } else { 1.0 }
            }
        };

        serde_json::json!(center + amplitude * wave)
    }

    /// Evaluate keyframe track at specific time
    fn evaluate_keyframes(&self, keyframes: &[Keyframe], time: f64, interpolation: Interpolation) -> Option<serde_json::Value> {
        if keyframes.is_empty() {
            return None;
        }

        // Single keyframe: constant value
        if keyframes.len() == 1 {
            return Some(keyframes[0].value.clone());
        }

        // Before first keyframe: hold first value
        if time <= keyframes[0].time {
            return Some(keyframes[0].value.clone());
        }

        // After last keyframe: hold last value
        let last_idx = keyframes.len() - 1;
        if time >= keyframes[last_idx].time {
            return Some(keyframes[last_idx].value.clone());
        }

        // Find surrounding keyframes
        for i in 0..keyframes.len() - 1 {
            let kf0 = &keyframes[i];
            let kf1 = &keyframes[i + 1];

            if time >= kf0.time && time <= kf1.time {
                // Interpolate between kf0 and kf1
                return Some(Self::interpolate_keyframes(kf0, kf1, time, interpolation));
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
            Interpolation::Linear => {
                // Apply easing function from keyframe, then linear interpolation
                let t_eased = kf0.easing.apply(t);
                Self::lerp_json_value(&kf0.value, &kf1.value, t_eased)
            }
            Interpolation::Smooth => {
                // Apply easing, then smooth (Hermite) curve
                let t_eased = kf0.easing.apply(t);
                let t_smooth = interpolation.apply(t_eased);
                Self::lerp_json_value(&kf0.value, &kf1.value, t_smooth)
            }
            Interpolation::Sinusoidal => {
                // Apply easing, then sinusoidal S-curve
                let t_eased = kf0.easing.apply(t);
                let t_sine = interpolation.apply(t_eased);
                Self::lerp_json_value(&kf0.value, &kf1.value, t_sine)
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
    use super::super::{Track, CircularTrack, OscillatorType};

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
        // When LoopMode::Once finishes, time resets to 0.0 for consistent behavior
        assert_eq!(controller.current_time, 0.0);
        assert_eq!(controller.state, PlaybackState::Stopped);
    }

    #[test]
    fn test_loop_mode_loop() {
        let mut controller = AnimationController::new();
        let mut animation = Animation::new("Test".into(), 5.0);
        animation.loop_mode = LoopMode::Loop;
        controller.load(animation);
        controller.play();

        controller.update(7.0); // 7 % 5 = 2
        assert_eq!(controller.current_time, 2.0);
        assert_eq!(controller.state, PlaybackState::Playing);
    }

    #[test]
    fn test_loop_mode_pingpong() {
        let mut controller = AnimationController::new();
        let mut animation = Animation::new("Test".into(), 5.0);
        animation.loop_mode = LoopMode::PingPong;
        controller.load(animation);
        controller.play();

        // Move to end
        controller.update(5.0);
        assert_eq!(controller.current_time, 5.0);
        assert_eq!(controller.direction, -1.0);

        // Move backward
        controller.update(2.0);
        assert_eq!(controller.current_time, 3.0);

        // Continue backward to start
        controller.update(4.0);
        // Should bounce at 0 and go forward
        assert!(controller.current_time >= 0.0);
        assert_eq!(controller.direction, 1.0);
    }

    #[test]
    fn test_track_evaluation_keyframes() {
        let controller = AnimationController::new();
        let track = Track::linear(
            serde_json::json!(0.0),
            serde_json::json!(10.0),
            10.0,
        );

        // At start
        let val = controller.evaluate_track(&track, 0.0).unwrap();
        assert_eq!(val.as_f64().unwrap(), 0.0);

        // At middle
        let val = controller.evaluate_track(&track, 5.0).unwrap();
        assert_eq!(val.as_f64().unwrap(), 5.0);

        // At end
        let val = controller.evaluate_track(&track, 10.0).unwrap();
        assert_eq!(val.as_f64().unwrap(), 10.0);
    }

    #[test]
    fn test_oscillator_sine() {
        let controller = AnimationController::new();
        let track = Track::oscillator(OscillatorType::Sine, 5.0, 2.0, 1.0);

        // At t=0, sine(0) = 0, so value = center = 5.0
        let val = controller.evaluate_track(&track, 0.0).unwrap();
        assert!((val.as_f64().unwrap() - 5.0).abs() < 0.01);

        // At t=0.25, sine(π/2) = 1, so value = 5.0 + 2.0 = 7.0
        let val = controller.evaluate_track(&track, 0.25).unwrap();
        assert!((val.as_f64().unwrap() - 7.0).abs() < 0.01);

        // At t=0.5, sine(π) = 0, so value = 5.0
        let val = controller.evaluate_track(&track, 0.5).unwrap();
        assert!((val.as_f64().unwrap() - 5.0).abs() < 0.01);

        // At t=0.75, sine(3π/2) = -1, so value = 5.0 - 2.0 = 3.0
        let val = controller.evaluate_track(&track, 0.75).unwrap();
        assert!((val.as_f64().unwrap() - 3.0).abs() < 0.01);
    }

    #[test]
    fn test_oscillator_square() {
        let controller = AnimationController::new();
        let track = Track::oscillator(OscillatorType::Square, 0.0, 1.0, 1.0);

        // First half of cycle: -1
        let val = controller.evaluate_track(&track, 0.25).unwrap();
        assert_eq!(val.as_f64().unwrap(), -1.0);

        // Second half of cycle: +1
        let val = controller.evaluate_track(&track, 0.75).unwrap();
        assert_eq!(val.as_f64().unwrap(), 1.0);
    }

    #[test]
    fn test_circular_track() {
        let circular = CircularTrack::new(
            "X".to_string(),
            "Y".to_string(),
            0.0, 0.0,  // center
            1.0,       // radius
            1.0,       // 1 revolution per second
        );

        // At t=0, angle=0, x=1, y=0
        let (x, y) = circular.evaluate(0.0);
        assert!((x - 1.0).abs() < 0.01);
        assert!(y.abs() < 0.01);

        // At t=0.25, angle=π/2, x=0, y=1
        let (x, y) = circular.evaluate(0.25);
        assert!(x.abs() < 0.01);
        assert!((y - 1.0).abs() < 0.01);

        // At t=0.5, angle=π, x=-1, y=0
        let (x, y) = circular.evaluate(0.5);
        assert!((x + 1.0).abs() < 0.01);
        assert!(y.abs() < 0.01);
    }

    #[test]
    fn test_evaluate_frame_with_circular() {
        let mut controller = AnimationController::new();
        let mut animation = Animation::new("Test".into(), 10.0);

        animation.add_circular_track(CircularTrack::new(
            "PanX".to_string(),
            "PanY".to_string(),
            0.0, 0.0,
            1.0,
            1.0,
        ));

        controller.load(animation);
        controller.current_time = 0.0;

        let values = controller.evaluate_frame();
        assert_eq!(values.len(), 2); // X and Y

        // Find X and Y values
        let x = values.iter().find(|(k, _)| k == "PanX").map(|(_, v)| v.as_f64().unwrap());
        let y = values.iter().find(|(k, _)| k == "PanY").map(|(_, v)| v.as_f64().unwrap());

        assert!(x.is_some());
        assert!(y.is_some());
        assert!((x.unwrap() - 1.0).abs() < 0.01);
        assert!(y.unwrap().abs() < 0.01);
    }
}
