//! An animation's frames as configs -- what a video is made of before any
//! pixel: the tracks bound to the config, the signals they read, and each
//! frame's config and motion-blur moments. The video export renders them
//! into FFmpeg (`export::export_animation`) and Python into whatever it
//! likes (`wasm/python`), so a frame is the same config from both; what
//! renders it and where the pixels go is theirs.

use std::collections::HashMap;

use super::export::{apply_animation_values, export_signals, moment_configs, tracks_missing_signals};
use super::{Animation, AnimationController};
use crate::config::FractalConfig;
use crate::signal::{Signal, SignalManager};

/// An animation over its base config, ready to give its frames.
pub struct FrameSource {
    base: FractalConfig,
    /// The animation as bound, for a walk of its own ([`FrameSource::fresh`]).
    bound: Animation,
    controller: AnimationController,
    /// The signals the tracks read, and a manager over them (none
    /// without any). Smoothing's state is the controller's, so a manager
    /// built afresh from the same signals reads the same.
    signal_map: HashMap<String, Signal>,
    signals: Option<SignalManager>,
    fps: u32,
    frames: u32,
    /// What the animation asks for that this render cannot give: a track
    /// that lost its target, a signal not handed over. Each surface
    /// reports them its own way.
    pub warnings: Vec<String>,
}

impl FrameSource {
    /// `animation` over `base` at `fps`, reading the `given` signals and
    /// rendering the animation's own generators.
    ///
    /// A track the app marked broken holds a stale index, and binding
    /// afresh would resolve that index onto whatever item now sits there,
    /// so it is left out, as it is left out of the preview.
    pub fn new(animation: &Animation, base: FractalConfig, fps: u32, given: &HashMap<String, Signal>) -> Self {
        let fps = fps.max(1);
        let mut bound = animation.clone();
        let mut warnings: Vec<String> = bound
            .remove_broken_tracks()
            .into_iter()
            .map(|t| format!("the track on `{}` lost its target (deleted) and is left out", t.target))
            .collect();
        bound.bind_to_config(&base);
        let signals = export_signals(animation, given);
        for (target, signal) in tracks_missing_signals(animation, &signals) {
            warnings.push(format!(
                "the track on `{target}` reads signal `{signal}`, which this render does not have \
                 (a live signal never reaches an export), so it holds still"
            ));
        }
        let frames = (animation.duration * fps as f64).ceil() as u32;
        let controller = Self::controller(&bound, fps);
        FrameSource { base, bound, controller, signals: Self::manager(&signals), signal_map: signals, fps, frames, warnings }
    }

    fn manager(signals: &HashMap<String, Signal>) -> Option<SignalManager> {
        (!signals.is_empty()).then(|| {
            let mut manager = SignalManager::new();
            for signal in signals.values() {
                manager.insert(signal.clone());
            }
            manager
        })
    }

    /// A controller over `animation` stepping one frame at a time. Signal
    /// smoothing is scaled by the step, and a controller's is 1/60 s until
    /// something calls `update` -- which nothing rendering frames does --
    /// so a 30 fps video smoothed half as much per second as the preview.
    fn controller(animation: &Animation, fps: u32) -> AnimationController {
        let mut controller = AnimationController::new();
        controller.load(animation.clone());
        controller.set_step(1.0 / fps as f64);
        controller
    }

    /// The same frames walked afresh, signal smoothing (which keeps state)
    /// starting clean: a check over every frame before the video, which
    /// must not disturb the video's own walk.
    pub fn fresh(&self) -> FrameSource {
        FrameSource {
            base: self.base.clone(),
            bound: self.bound.clone(),
            controller: Self::controller(&self.bound, self.fps),
            signals: Self::manager(&self.signal_map),
            signal_map: self.signal_map.clone(),
            fps: self.fps,
            frames: self.frames,
            warnings: Vec::new(),
        }
    }

    pub fn frames(&self) -> u32 {
        self.frames
    }

    pub fn fps(&self) -> u32 {
        self.fps
    }

    pub fn duration(&self) -> f64 {
        self.bound.duration
    }

    /// The config the frames are made from.
    pub fn base(&self) -> &FractalConfig {
        &self.base
    }

    /// When frame `frame` is shown.
    pub fn time(&self, frame: u32) -> f64 {
        frame as f64 / self.fps as f64
    }

    /// Frame `frame`'s config: the tracks at its time on the base.
    /// Evaluated in order, as smoothing keeps state.
    pub fn config(&mut self, frame: u32) -> FractalConfig {
        let time = self.time(frame);
        let values = match &self.signals {
            Some(s) => self.controller.evaluate_at_time_with_signals(time, Some(s), &self.base),
            None => self.controller.evaluate_at_time(time, &self.base),
        };
        let mut config = self.base.clone();
        apply_animation_values(&mut config, &values);
        config
    }

    /// Frame `frame`'s config and its motion-blur moments (none without
    /// motion blur), the moments taken after the frame so its signal
    /// smoothing has advanced once (`evaluate_moment`).
    pub fn frame(&mut self, frame: u32) -> (FractalConfig, Vec<FractalConfig>) {
        let config = self.config(frame);
        let moments = moment_configs(&self.base, &self.controller, self.time(frame), self.signals.as_ref());
        (config, moments)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A walk afresh gives the frames the video's walk gives, signal
    /// smoothing and all: the deep-zoom check walks every frame before the
    /// video does, and must neither see other frames nor disturb the
    /// video's.
    #[test]
    fn a_fresh_walk_gives_the_same_frames() {
        let animation = Animation::from_json(
            r#"{
                "name": "walk", "duration": 1.0, "loop_mode": "Loop",
                "tracks": {
                    "Zoom": { "source": { "type": "Keyframes", "keyframes": [
                        { "time": 0.0, "value": 1.0, "easing": "Linear" },
                        { "time": 1.0, "value": 1.6, "easing": "Linear" } ] },
                        "interpolation": "Linear" },
                    "Rotation": { "source": { "type": "Signal", "signal_name": "sine_2hz",
                        "min_output": -0.4, "max_output": 0.4, "smoothing": 0.5 },
                        "interpolation": "Linear" }
                },
                "generators": [{ "name": "sine_2hz", "waveform": "Sine", "frequency": 2.0 }]
            }"#,
        )
        .expect("animation");
        let mut source = FrameSource::new(&animation, crate::resources::presets::create_default_preset(), 12, &HashMap::new());
        assert!(source.warnings.is_empty(), "{:?}", source.warnings);
        assert_eq!(source.frames(), 12);
        let walk = |s: &mut FrameSource| -> Vec<serde_json::Value> {
            (0..s.frames()).map(|f| serde_json::to_value(s.config(f)).expect("config")).collect()
        };
        let checked = walk(&mut source.fresh());
        let video = walk(&mut source);
        assert_eq!(checked, video);
        assert!(video.windows(2).all(|w| w[0] != w[1]), "every frame moves");
    }

    /// **A video gives the frames playback gave**, script tracks and all
    /// (docs/projects/rhai-tracks.md): the shipped examples -- an added
    /// breath, a spring that keeps a state, smoothed jitter -- over a
    /// keyframed zoom, played live with each frame applied to the config
    /// the next reads, then exported from the config playback left.
    #[test]
    fn a_video_of_script_tracks_gives_the_frames_playback_gave() {
        use crate::animation::{Animation, AnimationController, Track};
        let example = |f: &str| crate::storage::formula_store::EXAMPLES.iter().find(|(n, _)| *n == f).unwrap().1.to_string();
        let mut animation = Animation::new("scripts".into(), 2.0);
        animation.add_track(Track::linear("Zoom".into(), serde_json::json!(1.0), serde_json::json!(2.0), 2.0));
        animation.add_track(Track::script(example("track-springy-zoom.rhai")).unwrap());
        animation.add_track(Track::script(example("track-breathing.rhai")).unwrap());
        animation.add_track(Track::script(example("track-smoothed-jitter.rhai")).unwrap());
        let fps = 24;
        let mut live = AnimationController::new();
        live.load(animation);
        let mut config = crate::resources::presets::create_default_preset();
        let mut played = Vec::new();
        for frame in 0..=(2 * fps) {
            live.current_time = frame as f64 / fps as f64;
            let values = live.evaluate_frame(None, &config);
            apply_animation_values(&mut config, &values);
            played.push(serde_json::to_value(&config).unwrap());
        }
        let mut source = FrameSource::new(live.animation.as_ref().unwrap(), config.clone(), fps, &HashMap::new());
        for frame in 0..source.frames() {
            let exported = serde_json::to_value(source.config(frame)).unwrap();
            assert_eq!(exported, played[frame as usize], "frame {frame}");
        }
        assert_ne!(played[6], played[18], "the frames move");
    }
}
