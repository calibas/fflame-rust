//! Animation playback controller

use super::{Animation, EasingFunction, Interpolation, Keyframe, LoopMode, PlaybackState, Track, TrackSource};
use crate::config::{EditingTarget, FractalConfig};
use crate::signal::SignalManager;

/// A frame's values: `(flame, ConfigPath key, value)`, applied in order.
type Values = Vec<(EditingTarget, String, serde_json::Value)>;

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

    /// Evaluate all tracks at current time, live: on `base`, the config
    /// the values will be applied to.
    ///
    /// Returns vec of `(flame_target, path_string, value)` triples for
    /// parameters that should be updated. The flame_target tells the
    /// apply path which flame (Main or a specific subflame) the path
    /// resolves against.
    ///
    /// The one evaluation that records the script targets' rests
    /// (`script::Rest`): the live config is the only one that holds what
    /// the scripts wrote. The video export's base holds the app's last
    /// values and never its own, so its evaluations read the rests and
    /// leave them.
    pub fn evaluate_frame(&mut self, signal_manager: Option<&SignalManager>, base: &FractalConfig) -> Values {
        let (values, rests) = self.evaluate_inner(self.current_time, signal_manager, base);
        if let (Some(animation), Some(rests)) = (self.animation.as_mut(), rests) {
            animation.script_rest = rests;
        }
        values
    }

    /// Evaluate all tracks at a specific time (for export, no signal
    /// support), on `base`, the config the values will be applied to.
    ///
    /// Returns vec of `(flame_target, path_string, value)` triples.
    /// Signal tracks are skipped when no SignalManager is provided.
    pub fn evaluate_at_time(&self, time: f64, base: &FractalConfig) -> Values {
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

        let (scripts, _) = self.script_values(base, &values, time, None);
        values.extend(scripts);
        values
    }

    /// Evaluate all tracks at a specific time with signal support, on
    /// `base`, the config the values will be applied to.
    ///
    /// This version supports Signal tracks when a SignalManager is provided.
    pub fn evaluate_at_time_with_signals(&mut self, time: f64, signal_manager: Option<&SignalManager>, base: &FractalConfig) -> Values {
        self.evaluate_inner(time, signal_manager, base).0
    }

    /// Every track at `time`, and the script targets' rests as this
    /// evaluation found them (`None` without an animation).
    fn evaluate_inner(&mut self, time: f64, signal_manager: Option<&SignalManager>, base: &FractalConfig) -> (Values, Option<std::collections::HashMap<String, super::script::Rest>>) {
        let Some(ref animation) = self.animation else {
            return (Vec::new(), None);
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

        if self.animation.is_none() {
            return (values, None);
        }
        let (scripts, rests) = self.script_values(base, &values, time, signal_manager);
        values.extend(scripts);
        (values, Some(rests))
    }

    /// The script tracks' values at `time` (docs/archive/projects/rhai-tracks.md),
    /// in track order, after the keyframe and signal tracks' `under`, and
    /// the script targets' rests as they now stand.
    ///
    /// A script reads the value under the scripts: `base` with each script
    /// target put back to its rest where `base` still holds what scripts
    /// wrote (else what `base` holds is the new rest: someone changed it),
    /// then `under` applied. Its outputs combine with that by their modes,
    /// so an `"add"` adds to the same value every frame, live or exported.
    /// A later track writing the same target replaces an earlier one.
    ///
    /// A stateful script's steps read the config under the scripts at
    /// their own times: the rested base with the keyframe and signal tracks
    /// at that time (`held_values`; `signals` as the frame has them).
    fn script_values(&self, base: &FractalConfig, under: &[(EditingTarget, String, serde_json::Value)], time: f64, signals: Option<&SignalManager>) -> (Values, std::collections::HashMap<String, super::script::Rest>) {
        use super::script::{self, Rest};
        let Some(animation) = self.animation.as_ref() else {
            return (Vec::new(), Default::default());
        };
        let mut rests = animation.script_rest.clone();
        let scripts: Vec<_> = animation
            .tracks
            .iter()
            .filter(|t| !t.bound.broken)
            .filter_map(|t| match &t.source {
                TrackSource::Script { source, params } => Some((source, params)),
                _ => None,
            })
            .collect();
        if scripts.is_empty() {
            return (Vec::new(), rests);
        }
        let mut restore = Vec::new();
        for (key, r) in rests.iter_mut() {
            let Some(current) = script::read(base, key) else { continue };
            if r.holds_written(current) {
                restore.push((EditingTarget::Main, key.clone(), serde_json::json!(r.rest)));
            } else {
                r.rest = current;
            }
        }
        let mut config = base.clone();
        super::export::apply_animation_values(&mut config, &restore);
        let rested = std::rc::Rc::new(config.clone());
        super::export::apply_animation_values(&mut config, under);
        let config = std::rc::Rc::new(config);
        let signals_at = |t: f64| {
            let values = signals.map(|m| m.signal_names().into_iter().filter_map(|n| Some((n.clone(), m.get_live_value(&n, t)? as f64))).collect()).unwrap_or_default();
            std::rc::Rc::new(values)
        };
        let at_time = |t: f64| {
            let mut c = (*rested).clone();
            super::export::apply_animation_values(&mut c, &self.held_values(t, signals));
            script::Moment { under: std::rc::Rc::new(c), signals: signals_at(t), any_signal: false }
        };
        let dependencies = {
            use std::hash::{Hash, Hasher};
            let others: Vec<&Track> = animation.tracks.iter().filter(|t| !matches!(t.source, TrackSource::Script { .. })).collect();
            let mut h = std::collections::hash_map::DefaultHasher::new();
            serde_json::to_string(&others).unwrap_or_default().hash(&mut h);
            animation.duration.to_bits().hash(&mut h);
            h.finish()
        };
        let frame = script::Frame {
            time,
            duration: animation.duration,
            at: script::Moment { under: std::rc::Rc::clone(&config), signals: signals_at(time), any_signal: false },
            rested: &rested,
            at_time: &at_time,
            dependencies,
        };
        let mut values = Vec::new();
        for (source, params) in scripts {
            match script::evaluate(source, params, &frame) {
                Ok(outputs) => {
                    script::note_error(source, None);
                    for o in outputs {
                        let Some(below) = script::read(&config, &o.key) else { continue };
                        values.push((EditingTarget::Main, o.key, serde_json::json!(o.mode.combine(below, o.value))));
                    }
                }
                Err(e) => script::note_error(source, Some(e)),
            }
        }
        // What the config will hold once they are applied, through the one
        // setter, clamps and all: what the next frame compares against.
        let mut written = (*config).clone();
        super::export::apply_animation_values(&mut written, &values);
        for (_, key, _) in &values {
            let (Some(w), Some(rest)) = (script::read(&written, key), script::read(&rested, key)) else { continue };
            rests.entry(key.clone()).or_insert(Rest { rest, written: w }).written = w;
        }
        (values, rests)
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
            // Evaluated after the others (`script_values`).
            TrackSource::Script { .. } => None,
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
            TrackSource::Script { .. } => None,
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
    pub fn evaluate_moment(&self, time: f64, signal_manager: Option<&SignalManager>, base: &FractalConfig) -> Values {
        let mut values = self.held_values(time, signal_manager);
        let (scripts, _) = self.script_values(base, &values, time, signal_manager);
        values.extend(scripts);
        values
    }

    /// The keyframe and signal tracks at `time`, leaving the controller as
    /// it was: a smoothed signal holds the frame's smoothed value. What a
    /// motion-blur moment and a script's step see under the scripts.
    fn held_values(&self, time: f64, signal_manager: Option<&SignalManager>) -> Values {
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
                TrackSource::Script { .. } => None,
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
        let values = controller.evaluate_at_time(0.0, &crate::config::FractalConfig::default());
        let val = values.iter().find(|(_, k, _)| k == "Test").unwrap().2.as_f64().unwrap();
        assert_eq!(val, 0.0);

        // At middle
        let values = controller.evaluate_at_time(5.0, &crate::config::FractalConfig::default());
        let val = values.iter().find(|(_, k, _)| k == "Test").unwrap().2.as_f64().unwrap();
        assert_eq!(val, 5.0);

        // At end
        let values = controller.evaluate_at_time(10.0, &crate::config::FractalConfig::default());
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
            c.evaluate_at_time(t, &crate::config::FractalConfig::default())
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
        let values = controller.evaluate_at_time_with_signals(0.0, Some(&signal_manager), &crate::config::FractalConfig::default());
        let val = values.iter().find(|(_, k, _)| k == "Test").unwrap().2.as_f64().unwrap();
        assert!((val - 0.0).abs() < 0.01);

        // At t=0.2, signal=1.0, output=10.0
        let values = controller.evaluate_at_time_with_signals(0.2, Some(&signal_manager), &crate::config::FractalConfig::default());
        let val = values.iter().find(|(_, k, _)| k == "Test").unwrap().2.as_f64().unwrap();
        assert!((val - 10.0).abs() < 0.01);

        // At t=0.1, signal=0.5, output=5.0
        let values = controller.evaluate_at_time_with_signals(0.1, Some(&signal_manager), &crate::config::FractalConfig::default());
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
                let values = controller.evaluate_at_time_with_signals(k as f64 / fps as f64, Some(&signals), &crate::config::FractalConfig::default());
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
            let a = plain.evaluate_at_time_with_signals(t, Some(&signals), &crate::config::FractalConfig::default());
            let b = blurred.evaluate_at_time_with_signals(t, Some(&signals), &crate::config::FractalConfig::default());
            assert_eq!(a, b, "frame {frame}: the moments changed what followed");
            for dt in [-0.01, 0.01] {
                let moment = blurred.evaluate_moment(t + dt, Some(&signals), &crate::config::FractalConfig::default());
                assert_eq!(value(&moment, "Exposure"), value(&b, "Exposure"), "the smoothed track holds the frame's value");
                assert!((value(&moment, "Gamma") - (t + dt)).abs() < 1e-9, "the keyframes are read at the moment");
            }
        }
    }

    // ---- Script tracks (docs/archive/projects/rhai-tracks.md, step 1) ----

    fn zoom_of(config: &FractalConfig) -> f64 {
        super::super::script::read(config, &super::super::script::resolve_target("zoom").unwrap()).unwrap()
    }

    fn with_tracks(tracks: Vec<Track>) -> Animation {
        let mut a = Animation::new("scripts".into(), 4.0);
        for t in tracks {
            a.add_track(t);
        }
        a
    }

    const ADD_HALF_T: &str = "const TARGETS = #{ zoom: \"add\" };\nfn value(t, p, ctx) { #{ zoom: 0.5 * t } }";

    /// **An `"add"` adds to the same value every frame, live and in the
    /// export.** Played live, each frame applied to the config the next
    /// one reads, the zoom is its rest plus the script's value, never the
    /// last frame's plus it again. The export's base is the app's config
    /// after playback, holding the last value written; carrying the rests
    /// in the animation, it gives the frames playback gave -- and a zoom
    /// the user set after playback is the new rest.
    #[test]
    fn an_added_value_does_not_compound_live_or_exported() {
        use super::super::export::apply_animation_values;
        let base = FractalConfig::default();
        let rest = zoom_of(&base);
        let mut live = AnimationController::new();
        live.load(with_tracks(vec![Track::script(ADD_HALF_T.into()).unwrap()]));
        let mut config = base.clone();
        let times = [0.0, 0.5, 1.0, 1.0, 1.0, 2.5, 0.25];
        for t in times {
            live.current_time = t;
            let v = live.evaluate_frame(None, &config);
            apply_animation_values(&mut config, &v);
            assert!((zoom_of(&config) - (rest + 0.5 * t)).abs() < 1e-5, "live at {t}: {}", zoom_of(&config));
        }
        let mut export = AnimationController::new();
        export.load(live.animation.clone().unwrap());
        for t in times {
            let mut frame = config.clone();
            apply_animation_values(&mut frame, &export.evaluate_at_time(t, &config));
            assert!((zoom_of(&frame) - (rest + 0.5 * t)).abs() < 1e-5, "exported at {t}: {}", zoom_of(&frame));
        }
        let mut edited = config.clone();
        apply_animation_values(&mut edited, &[(EditingTarget::Main, "Zoom".into(), serde_json::json!(3.0))]);
        let mut frame = edited.clone();
        apply_animation_values(&mut frame, &export.evaluate_at_time(1.0, &edited));
        assert!((zoom_of(&frame) - 3.5).abs() < 1e-5, "the user's zoom is the new rest: {}", zoom_of(&frame));
    }

    /// A script goes on top of the keyframes: it reads, and adds to, the
    /// keyframed value. Two scripts on one target: the later one wins.
    #[test]
    fn a_script_adjusts_the_keyframes_and_the_later_script_wins() {
        use super::super::export::apply_animation_values;
        let base = FractalConfig::default();
        let keyed = Track::linear("Zoom".into(), serde_json::json!(1.0), serde_json::json!(2.0), 4.0);
        let c = |tracks: Vec<Track>, t: f64| {
            let mut ctl = AnimationController::new();
            ctl.load(with_tracks(tracks));
            let mut frame = base.clone();
            apply_animation_values(&mut frame, &ctl.evaluate_at_time(t, &base));
            zoom_of(&frame)
        };
        assert!((c(vec![keyed.clone(), Track::script(ADD_HALF_T.into()).unwrap()], 2.0) - (1.5 + 1.0)).abs() < 1e-5);
        let reads = "const TARGETS = #{ rotation: \"set\" };\nfn value(t, p, ctx) { #{ rotation: ctx.get(\"zoom\") } }";
        let mut ctl = AnimationController::new();
        ctl.load(with_tracks(vec![keyed.clone(), Track::script(reads.into()).unwrap()]));
        let v = ctl.evaluate_at_time(2.0, &base);
        let rotation = v.iter().find(|(_, k, _)| k == "Rotation").expect("a rotation").2.as_f64().unwrap();
        assert!((rotation - 1.5).abs() < 1e-5, "ctx reads the keyframed zoom: {rotation}");
        let set_two = "const TARGETS = #{ zoom: \"set\" };\nfn value(t, p, ctx) { #{ zoom: 2.0 } }";
        let set_three = "const TARGETS = #{ zoom: \"set\" };\nfn value(t, p, ctx) { #{ zoom: 3.0 } }";
        assert_eq!(c(vec![Track::script(set_two.into()).unwrap(), Track::script(set_three.into()).unwrap()], 1.0), 3.0);
    }

    /// A script track round-trips through a `.anim` with its parameters,
    /// and a broken one, or one that fails, writes nothing.
    #[test]
    fn a_script_track_round_trips_and_a_broken_one_is_skipped() {
        let mut track = Track::script(ADD_HALF_T.into()).unwrap();
        if let TrackSource::Script { params, .. } = &mut track.source {
            params.insert("depth".into(), 0.25);
        }
        let json = with_tracks(vec![track]).to_json().unwrap();
        let loaded = Animation::from_json(&json).unwrap();
        let TrackSource::Script { source, params } = &loaded.tracks[0].source else { panic!("a script track") };
        assert_eq!((source.as_str(), params.get("depth")), (ADD_HALF_T, Some(&0.25)));
        assert_eq!(loaded.tracks[0].target, "Zoom");
        let base = FractalConfig::default();
        let mut broken = loaded.clone();
        broken.tracks[0].bound.broken = true;
        let mut ctl = AnimationController::new();
        ctl.load(broken);
        assert!(ctl.evaluate_at_time(1.0, &base).is_empty());
        let fails = "const TARGETS = #{ zoom: \"set\" };\nfn value(t, p, ctx) { #{ zoom: 1.0 / 0.0 } }";
        let mut ctl = AnimationController::new();
        ctl.load(with_tracks(vec![Track::script(fails.into()).unwrap()]));
        assert!(ctl.evaluate_at_time(1.0, &base).is_empty());
        assert!(super::super::script::error_of(fails).is_some_and(|e| e.contains("finite")));
    }

    // ---- Script tracks with state (rhai-tracks.md, step 2) ----

    /// A spring following the keyframed zoom: lag and overshoot, which no
    /// formula of `t` alone gives.
    const SPRING: &str = r#"
const PARAMS = #{
    stiffness: #{ value: 40.0, min: 1.0, max: 400.0 },
    damping: #{ value: 6.0, min: 0.0, max: 50.0 },
};
const TARGETS = #{ rotation: "set" };
fn start(p, ctx) { #{ x: ctx.get("zoom"), v: 0.0 } }
fn step(s, dt, p, ctx) {
    let a = p.stiffness * (ctx.get("zoom") - s.x) - p.damping * s.v;
    s.v += a * dt;
    s.x += s.v * dt;
    s
}
fn value(t, s, p, ctx) { #{ rotation: s.x } }
"#;

    fn spring_animation() -> Animation {
        let mut a = with_tracks(vec![
            Track::linear("Zoom".into(), serde_json::json!(1.0), serde_json::json!(3.0), 2.0),
            Track::script(SPRING.into()).unwrap(),
        ]);
        a.duration = 6.0;
        a
    }

    fn rotation_at(ctl: &AnimationController, t: f64, base: &FractalConfig) -> f64 {
        ctl.evaluate_at_time(t, base).iter().find(|(_, k, _)| k == "Rotation").expect("a rotation").2.as_f64().unwrap()
    }

    /// **The same time gives the same state**, however it is reached:
    /// played forward at 60 fps, at 30, with frames dropped, scrubbed
    /// backward from the end, or jumped to cold on a fresh controller.
    /// And it is a state: it lags the zoom and overshoots it.
    #[test]
    fn a_state_is_the_same_however_its_time_is_reached() {
        let base = FractalConfig::default();
        let mut sixty = AnimationController::new();
        sixty.load(spring_animation());
        let mut at_sixty = Vec::new();
        for k in 0..=360 {
            at_sixty.push(rotation_at(&sixty, k as f64 / 60.0, &base));
        }
        let mut thirty = AnimationController::new();
        thirty.load(spring_animation());
        for k in 0..=180 {
            assert_eq!(rotation_at(&thirty, k as f64 / 30.0, &base), at_sixty[2 * k], "30 fps frame {k}");
        }
        let mut dropping = AnimationController::new();
        dropping.load(spring_animation());
        for k in (0..=360).step_by(7) {
            assert_eq!(rotation_at(&dropping, k as f64 / 60.0, &base), at_sixty[k], "with frames dropped, frame {k}");
        }
        for k in (0..=360).rev().step_by(11) {
            assert_eq!(rotation_at(&sixty, k as f64 / 60.0, &base), at_sixty[k], "scrubbed back, frame {k}");
        }
        let mut cold = AnimationController::new();
        cold.load(spring_animation());
        assert_eq!(rotation_at(&cold, 4.5, &base), at_sixty[270]);
        // A motion-blur moment either side of a frame leaves its state.
        for offset in [-0.007, 0.004] {
            let _ = sixty.evaluate_moment(1.0 + offset, None, &base);
            assert_eq!(rotation_at(&sixty, 1.0, &base), at_sixty[60], "after a moment at {offset}");
        }
        // Lag: at 1 s the keyframed zoom is 2; the spring is behind it.
        assert!(at_sixty[60] < 2.0, "it lags: {}", at_sixty[60]);
        // Overshoot: past 2 s the zoom holds at 3; the spring passes it.
        let peak = at_sixty[120..].iter().cloned().fold(f64::MIN, f64::max);
        assert!(peak > 3.0, "it overshoots: {peak}");
        assert!((at_sixty[360] - 3.0).abs() < 0.05, "and settles: {}", at_sixty[360]);
    }

    /// An edit a state depends on replays it: a changed keyframe track,
    /// a changed parameter, and a changed value it reads from the base.
    #[test]
    fn an_edit_a_state_depends_on_replays_it() {
        let base = FractalConfig::default();
        let mut ctl = AnimationController::new();
        ctl.load(spring_animation());
        let before = rotation_at(&ctl, 1.0, &base);
        if let TrackSource::Keyframes { keyframes } = &mut ctl.animation.as_mut().unwrap().tracks[0].source {
            keyframes[1].value = serde_json::json!(5.0);
        }
        let keyed = rotation_at(&ctl, 1.0, &base);
        assert_ne!(before, keyed, "a keyframe edit replays");
        let mut fresh = AnimationController::new();
        fresh.load(ctl.animation.clone().unwrap());
        assert_eq!(rotation_at(&fresh, 1.0, &base), keyed);
        if let TrackSource::Script { params, .. } = &mut ctl.animation.as_mut().unwrap().tracks[1].source {
            params.insert("stiffness".into(), 100.0);
        }
        assert_ne!(rotation_at(&ctl, 1.0, &base), keyed, "a parameter edit replays");
        // Without the keyframes the spring follows the base's zoom.
        let mut free = with_tracks(vec![Track::script(SPRING.into()).unwrap()]);
        free.duration = 6.0;
        let mut ctl = AnimationController::new();
        ctl.load(free);
        let mut moved = base.clone();
        super::super::export::apply_animation_values(&mut moved, &[(EditingTarget::Main, "Zoom".into(), serde_json::json!(2.5))]);
        let a = rotation_at(&ctl, 3.0, &base);
        let b = rotation_at(&ctl, 3.0, &moved);
        assert!((a - zoom_of(&base)).abs() < 1e-3 && (b - 2.5).abs() < 1e-3, "it follows the base it reads: {a} {b}");
    }
}
