//! Animation playback controller

use super::{Animation, EasingFunction, Interpolation, Keyframe, LoopMode, PlaybackState, Track, TrackSource};
use crate::config::EditingTarget;
use crate::signal::SignalManager;

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

    /// Smoothed signal values for tracks with smoothing enabled
    /// Key: (track_index, signal_name), Value: smoothed value
    signal_smoothed: std::collections::HashMap<(usize, String), f32>,

    /// Delta time from the most recent update() call (for frame-rate independent smoothing)
    last_delta_time: f64,

    /// Whether to sync audio playback with animation timeline
    pub sync_audio: bool,
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
            signal_smoothed: std::collections::HashMap::new(),
            last_delta_time: 1.0 / 60.0,
            sync_audio: false,
        }
    }

    /// Load animation and reset playback
    pub fn load(&mut self, animation: Animation) {
        self.animation = Some(animation);
        self.current_time = 0.0;
        self.state = PlaybackState::Stopped;
        self.direction = 1.0;
        self.signal_smoothed.clear();
    }

    /// Set the time step signal smoothing assumes, for a caller that
    /// evaluates at fixed times instead of through `update` -- the video
    /// exporter, whose frames are 1/fps apart. Without it the step stays
    /// at its initial 1/60 s whatever the frame rate.
    pub fn set_step(&mut self, dt: f64) {
        self.last_delta_time = dt;
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

        self.last_delta_time = delta_time;
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

    /// Evaluate all tracks at current time.
    ///
    /// Returns vec of `(flame_target, path_string, value)` triples for
    /// parameters that should be updated. The flame_target tells the
    /// apply path which flame (Main or a specific subflame) the path
    /// resolves against.
    pub fn evaluate_frame(&mut self, signal_manager: Option<&SignalManager>) -> Vec<(EditingTarget, String, serde_json::Value)> {
        self.evaluate_at_time_with_signals(self.current_time, signal_manager)
    }

    /// Evaluate all tracks at a specific time (for export, no signal support).
    ///
    /// Returns vec of `(flame_target, path_string, value)` triples.
    /// Signal tracks are skipped when no SignalManager is provided.
    pub fn evaluate_at_time(&self, time: f64) -> Vec<(EditingTarget, String, serde_json::Value)> {
        let Some(ref animation) = self.animation else {
            return Vec::new();
        };

        let mut values = Vec::new();

        // Evaluate regular tracks (skip Signal tracks without manager)
        for track in &animation.tracks {
            if matches!(track.source, TrackSource::Signal { .. }) {
                continue; // Skip signal tracks when no manager available
            }
            // Its target was deleted; the index it holds names another
            // item now (`TrackBinding::broken`).
            if track.bound.broken {
                continue;
            }
            if let Some(value) = Self::evaluate_track_pure(track, time) {
                values.push((track.flame_target, track.target.clone(), value));
            }
        }

        values
    }

    /// Evaluate all tracks at a specific time with signal support.
    ///
    /// This version supports Signal tracks when a SignalManager is provided.
    pub fn evaluate_at_time_with_signals(
        &mut self,
        time: f64,
        signal_manager: Option<&SignalManager>,
    ) -> Vec<(EditingTarget, String, serde_json::Value)> {
        let Some(ref animation) = self.animation else {
            return Vec::new();
        };

        // Clone track info to avoid borrow conflicts with signal smoothing
        // Broken tracks are skipped (`TrackBinding::broken`); the index
        // stays the track's position, which smoothing state is keyed by.
        let track_info: Vec<_> = animation
            .tracks
            .iter()
            .enumerate()
            .filter(|(_, track)| !track.bound.broken)
            .map(|(idx, track)| (idx, track.flame_target, track.target.clone(), track.source.clone(), track.interpolation))
            .collect();

        let duration = animation.duration;

        let mut values = Vec::new();

        // Evaluate regular tracks
        for (track_idx, flame_target, target, source, interpolation) in track_info {
            if let Some(value) = self.evaluate_source(
                &source,
                interpolation,
                time,
                duration,
                signal_manager,
                Some(track_idx),
            ) {
                values.push((flame_target, target, value));
            }
        }

        values
    }

    /// Evaluate single track at specific time (pure - no state mutation)
    /// Used for non-signal tracks where no smoothing is needed.
    fn evaluate_track_pure(track: &Track, time: f64) -> Option<serde_json::Value> {
        match &track.source {
            TrackSource::Keyframes { keyframes } => {
                Self::evaluate_keyframes_pure(keyframes, time, track.interpolation)
            }
            TrackSource::Signal { .. } => {
                None // Signal tracks require SignalManager
            }
        }
    }

    /// Evaluate a track source with signal support (may mutate smoothing state)
    fn evaluate_source(
        &mut self,
        source: &TrackSource,
        interpolation: Interpolation,
        time: f64,
        duration: f64,
        signal_manager: Option<&SignalManager>,
        track_idx: Option<usize>,
    ) -> Option<serde_json::Value> {
        match source {
            TrackSource::Keyframes { keyframes } => {
                Self::evaluate_keyframes_pure(keyframes, time, interpolation)
            }
            TrackSource::Signal {
                signal_name, min_output, max_output, smoothing,
                start_time, end_time, fade_in, fade_in_easing, fade_out, fade_out_easing,
            } => {
                self.evaluate_signal(
                    signal_name,
                    *min_output,
                    *max_output,
                    *smoothing,
                    *start_time,
                    *end_time,
                    *fade_in,
                    *fade_in_easing,
                    *fade_out,
                    *fade_out_easing,
                    time,
                    duration,
                    signal_manager,
                    track_idx,
                )
            }
        }
    }

    /// Evaluate signal-driven track with timing and fade support
    fn evaluate_signal(
        &mut self,
        signal_name: &str,
        min_output: f64,
        max_output: f64,
        smoothing: f64,
        start_time: f64,
        end_time: f64,
        fade_in: f64,
        fade_in_easing: EasingFunction,
        fade_out: f64,
        fade_out_easing: EasingFunction,
        time: f64,
        duration: f64,
        signal_manager: Option<&SignalManager>,
        track_idx: Option<usize>,
    ) -> Option<serde_json::Value> {
        let effective_end = Self::signal_window(start_time, end_time, time, duration)?;

        let manager = signal_manager?;
        // Use get_live_value which checks live producers first, then stored signals
        let raw_value = manager.get_live_value(signal_name, time)?;

        // Apply smoothing if enabled
        let value = if smoothing > 0.0 && track_idx.is_some() {
            let key = (track_idx.unwrap(), signal_name.to_string());
            let prev = self.signal_smoothed.get(&key).copied().unwrap_or(raw_value);

            // Frame-rate independent exponential smoothing:
            // alpha = 1 - smoothing^(dt / reference_dt)
            // At 60fps (reference), smoothing=0.9 gives alpha=0.1 (same as before)
            // At 30fps, alpha increases so the result matches what 60fps would produce
            let reference_dt: f64 = 1.0 / 60.0;
            let dt = self.last_delta_time.max(0.001); // clamp to avoid division issues
            let alpha = (1.0 - smoothing.clamp(0.0, 0.99).powf(dt / reference_dt)) as f32;
            let smoothed = prev + alpha * (raw_value - prev);
            self.signal_smoothed.insert(key, smoothed);
            smoothed
        } else {
            raw_value
        };

        Some(Self::signal_output(
            value, min_output, max_output, start_time, effective_end,
            fade_in, fade_in_easing, fade_out, fade_out_easing, time,
        ))
    }

    /// A signal track's window: its end (0 meaning the animation's), or
    /// None outside it, where the track is inactive.
    fn signal_window(start_time: f64, end_time: f64, time: f64, duration: f64) -> Option<f64> {
        // Resolve end_time sentinel (0.0 = use animation duration)
        let effective_end = if end_time <= 0.0 { duration } else { end_time };
        // Outside active window → track inactive
        (time >= start_time && time <= effective_end).then_some(effective_end)
    }

    /// A signal's value, faded at the window's ends and mapped to the
    /// track's output range.
    #[allow(clippy::too_many_arguments)]
    fn signal_output(
        value: f32,
        min_output: f64,
        max_output: f64,
        start_time: f64,
        effective_end: f64,
        fade_in: f64,
        fade_in_easing: EasingFunction,
        fade_out: f64,
        fade_out_easing: EasingFunction,
        time: f64,
    ) -> serde_json::Value {
        // Calculate fade factor (0.0 = fully faded, 1.0 = fully active)
        let mut fade_factor = 1.0;

        // Fade in
        if fade_in > 0.0 && time < start_time + fade_in {
            let t = ((time - start_time) / fade_in).clamp(0.0, 1.0);
            fade_factor *= fade_in_easing.apply(t);
        }

        // Fade out
        if fade_out > 0.0 && time > effective_end - fade_out {
            let t = ((effective_end - time) / fade_out).clamp(0.0, 1.0);
            fade_factor *= fade_out_easing.apply(t);
        }

        // Map signal value (assumed 0-1 range) to output range, applying fade
        let mapped = min_output + fade_factor * (max_output - min_output) * value as f64;
        serde_json::json!(mapped)
    }

    /// Every track at a motion-blur moment (`Animation::moments`), leaving
    /// the controller as it was. A frame's own evaluation advances signal
    /// smoothing once; its moments must not advance it again, so a
    /// smoothed signal track holds the frame's smoothed value across the
    /// shutter, and an unsmoothed one is read at the moment.
    pub fn evaluate_moment(
        &self,
        time: f64,
        signal_manager: Option<&SignalManager>,
    ) -> Vec<(EditingTarget, String, serde_json::Value)> {
        let Some(ref animation) = self.animation else {
            return Vec::new();
        };
        let mut values = Vec::new();
        for (track_idx, track) in animation.tracks.iter().enumerate() {
            if track.bound.broken {
                continue;
            }
            let value = match &track.source {
                TrackSource::Keyframes { keyframes } => {
                    Self::evaluate_keyframes_pure(keyframes, time, track.interpolation)
                }
                TrackSource::Signal {
                    signal_name, min_output, max_output, smoothing,
                    start_time, end_time, fade_in, fade_in_easing, fade_out, fade_out_easing,
                } => (|| {
                    let effective_end = Self::signal_window(*start_time, *end_time, time, animation.duration)?;
                    let raw = signal_manager?.get_live_value(signal_name, time)?;
                    let value = if *smoothing > 0.0 {
                        self.signal_smoothed.get(&(track_idx, signal_name.clone())).copied().unwrap_or(raw)
                    } else {
                        raw
                    };
                    Some(Self::signal_output(
                        value, *min_output, *max_output, *start_time, effective_end,
                        *fade_in, *fade_in_easing, *fade_out, *fade_out_easing, time,
                    ))
                })(),
            };
            if let Some(value) = value {
                values.push((track.flame_target, track.target.clone(), value));
            }
        }
        values
    }

    /// Evaluate keyframe track at specific time (pure function)
    fn evaluate_keyframes_pure(keyframes: &[Keyframe], time: f64, interpolation: Interpolation) -> Option<serde_json::Value> {
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
            Interpolation::Exponential => {
                // Equal ratio per unit time — the zoom curve. Easing
                // still applies (it shapes time; the geometry shapes
                // value). Non-numeric or sign-crossing values fall back
                // inside geometric_lerp / lerp_json_value.
                let t_eased = kf0.easing.apply(t);
                match (kf0.value.as_f64(), kf1.value.as_f64()) {
                    (Some(a), Some(b)) => {
                        let v = super::interpolation::geometric_lerp(a, b, t_eased);
                        serde_json::Number::from_f64(v)
                            .map(serde_json::Value::Number)
                            .unwrap_or_else(|| kf1.value.clone())
                    }
                    _ => Self::lerp_json_value(&kf0.value, &kf1.value, t_eased),
                }
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
    use super::super::Track;

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
        let mut controller = AnimationController::new();
        let track = Track::linear(
            "Test".into(),
            serde_json::json!(0.0),
            serde_json::json!(10.0),
            10.0,
        );

        let mut animation = Animation::new("Test".into(), 10.0);
        animation.add_track(track);
        controller.load(animation);

        // At start
        let values = controller.evaluate_at_time(0.0);
        let val = values.iter().find(|(_, k, _)| k == "Test").unwrap().2.as_f64().unwrap();
        assert_eq!(val, 0.0);

        // At middle
        let values = controller.evaluate_at_time(5.0);
        let val = values.iter().find(|(_, k, _)| k == "Test").unwrap().2.as_f64().unwrap();
        assert_eq!(val, 5.0);

        // At end
        let values = controller.evaluate_at_time(10.0);
        let val = values.iter().find(|(_, k, _)| k == "Test").unwrap().2.as_f64().unwrap();
        assert_eq!(val, 10.0);
    }

    /// An exponential Zoom track evaluates geometrically end to end —
    /// through Track/Animation/Controller, not just the bare lerp.
    #[test]
    fn exponential_track_interpolates_by_ratio() {
        let mut controller = AnimationController::new();
        let mut track = Track::linear(
            "Zoom".into(),
            serde_json::json!(1.0),
            serde_json::json!(100.0),
            10.0,
        );
        track.interpolation = Interpolation::Exponential;
        let mut animation = Animation::new("Dive".into(), 10.0);
        animation.add_track(track);
        controller.load(animation);

        let at = |c: &AnimationController, t: f64| -> f64 {
            c.evaluate_at_time(t)
                .iter()
                .find(|(_, k, _)| k == "Zoom")
                .unwrap()
                .2
                .as_f64()
                .unwrap()
        };

        // Halfway = 10 (half the doublings), NOT the linear 50.5.
        assert!((at(&controller, 5.0) - 10.0).abs() < 1e-6);
        // Quarter points continue the equal-ratio ladder.
        assert!((at(&controller, 2.5) - 100f64.powf(0.25)).abs() < 1e-6);
        // Endpoints are exact.
        assert!((at(&controller, 0.0) - 1.0).abs() < 1e-9);
        assert!((at(&controller, 10.0) - 100.0).abs() < 1e-9);
    }

    #[test]
    fn test_signal_track() {
        use crate::signal::{Signal, SignalManager, SignalType};

        let mut controller = AnimationController::new();
        let track = Track::signal("Test".into(), "energy".into(), 0.0, 10.0);

        let mut animation = Animation::new("Test".into(), 10.0);
        animation.add_track(track);
        controller.load(animation);

        // Create signal manager with test signal
        let mut signal_manager = SignalManager::new();
        signal_manager.insert(Signal::new(
            "energy".into(),
            10.0, // 10 Hz
            SignalType::Continuous,
            vec![0.0, 0.5, 1.0, 0.5, 0.0], // Peaks at 0.2s
        ));

        // Test signal evaluation at different times
        // At t=0.0, signal=0.0, output=0.0
        let values = controller.evaluate_at_time_with_signals(0.0, Some(&signal_manager));
        let val = values.iter().find(|(_, k, _)| k == "Test").unwrap().2.as_f64().unwrap();
        assert!((val - 0.0).abs() < 0.01);

        // At t=0.2, signal=1.0, output=10.0
        let values = controller.evaluate_at_time_with_signals(0.2, Some(&signal_manager));
        let val = values.iter().find(|(_, k, _)| k == "Test").unwrap().2.as_f64().unwrap();
        assert!((val - 10.0).abs() < 0.01);

        // At t=0.1, signal=0.5, output=5.0
        let values = controller.evaluate_at_time_with_signals(0.1, Some(&signal_manager));
        let val = values.iter().find(|(_, k, _)| k == "Test").unwrap().2.as_f64().unwrap();
        assert!((val - 5.0).abs() < 0.01);
    }

    /// Smoothing evaluated frame by frame, as the video exporter does,
    /// lands in the same place at 30 and 60 fps once the step is the
    /// frame interval. With the controller's initial 1/60 s step a
    /// 30 fps run smoothed half as much per second: about 0.97 rather
    /// than 0.999 here.
    #[test]
    fn smoothing_is_frame_rate_independent_with_the_frame_step() {
        use crate::signal::{Signal, SignalManager, SignalType};

        // 0 until t = 0.5 s, then 1; 100 Hz over 1 s.
        let data: Vec<f32> = (0..=100).map(|i| if i < 50 { 0.0 } else { 1.0 }).collect();
        let mut signals = SignalManager::new();
        signals.insert(Signal::new("step".into(), 100.0, SignalType::Continuous, data));

        let run = |fps: u32| -> f64 {
            let mut controller = AnimationController::new();
            let mut animation = Animation::new("Smooth".into(), 1.0);
            animation.add_track(Track::signal_with_smoothing("Exposure".into(), "step".into(), 0.0, 1.0, 0.8));
            controller.load(animation);
            controller.set_step(1.0 / fps as f64);
            let mut last = f64::NAN;
            for k in 0..=fps {
                let values = controller.evaluate_at_time_with_signals(k as f64 / fps as f64, Some(&signals));
                last = values.iter().find(|(_, key, _)| key == "Exposure").unwrap().2.as_f64().unwrap();
            }
            last
        };

        let (at30, at60) = (run(30), run(60));
        assert!(at60 > 0.99, "the step has settled at 60 fps: {at60}");
        assert!((at30 - at60).abs() < 1e-3, "30 fps {at30} vs 60 fps {at60}");
    }

    /// A frame's motion-blur moments are evaluated without touching the
    /// controller: the frames after come out the same whether or not the
    /// moments were taken. A smoothed signal track holds its frame value
    /// across the shutter; a keyframe track is read at the moment.
    #[test]
    fn a_moment_leaves_the_smoothing_as_it_was() {
        use crate::signal::{Signal, SignalManager, SignalType};

        let data: Vec<f32> = (0..=100).map(|i| i as f32 / 100.0).collect();
        let mut signals = SignalManager::new();
        signals.insert(Signal::new("ramp".into(), 100.0, SignalType::Continuous, data));
        let make = || {
            let mut controller = AnimationController::new();
            let mut animation = Animation::new("Moments".into(), 1.0);
            animation.add_track(Track::signal_with_smoothing("Exposure".into(), "ramp".into(), 0.0, 1.0, 0.8));
            animation.add_track(Track::linear("Gamma".into(), serde_json::json!(0.0), serde_json::json!(1.0), 1.0));
            controller.load(animation);
            controller
        };
        let value = |values: &[(EditingTarget, String, serde_json::Value)], key: &str| {
            values.iter().find(|(_, k, _)| k == key).unwrap().2.as_f64().unwrap()
        };
        let (mut plain, mut blurred) = (make(), make());
        // From the second frame: the first moment of the first falls
        // before the keyframes, which hold their first value there.
        for frame in 1..30 {
            let t = frame as f64 / 30.0;
            let a = plain.evaluate_at_time_with_signals(t, Some(&signals));
            let b = blurred.evaluate_at_time_with_signals(t, Some(&signals));
            assert_eq!(a, b, "frame {frame}: the moments changed what followed");
            for dt in [-0.01, 0.01] {
                let moment = blurred.evaluate_moment(t + dt, Some(&signals));
                assert_eq!(value(&moment, "Exposure"), value(&b, "Exposure"), "the smoothed track holds the frame's value");
                assert!((value(&moment, "Gamma") - (t + dt)).abs() < 1e-9, "the keyframes are read at the moment");
            }
        }
    }

}
