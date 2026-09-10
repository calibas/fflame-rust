//! Animation update logic
//!
//! Extracted from mod.rs to reduce file size and improve maintainability.
//! Handles animation state transitions, playback, and applying animated values to config.

use super::App;
use crate::animation::PlaybackState;

impl App {
    /// Update animation state and apply animated values to config.
    ///
    /// This handles:
    /// - Animation start/stop transitions
    /// - Time updates during playback
    /// - Evaluating animation tracks and applying values
    /// - Restoring base config when animation stops
    ///
    /// Returns whether animation is currently playing (for overwrite mode calculation).
    pub(super) fn update_animation(&mut self, delta_time: f64) -> bool {
        // Detect animation state transitions and update FSM accordingly
        let was_fsm_animating = self.render_mode.is_animating();
        let is_controller_playing = self.animation_controller.state == PlaybackState::Playing;

        // Handle animation start
        self.handle_animation_start(is_controller_playing, was_fsm_animating);

        // Handle animation stop/pause (user-initiated)
        self.handle_animation_stop(is_controller_playing, was_fsm_animating);

        // Process animation playback.
        //
        // The two STATEFUL engines pace differently. A flame frame
        // refines continuously, so advancing the controller every
        // display frame looks right. An escape frame is a chunked
        // render that is not a picture until it SETTLES, and a
        // simulation frame is not the picture for its time until the
        // grid has REACHED that time's step count -- so sampling every
        // display frame would show a smear of partial renders of
        // successive configs (reported for escape as animation "making
        // a mess"), or, for the simulation, a run lagging further
        // behind the playhead the longer it played.
        //
        // Instead: hold the controller while the frame is still coming
        // into being, then advance it by everything that elapsed
        // meanwhile. Playback may run at 1 fps or slower, but every
        // displayed frame is a real frame at a real timestamp -- which
        // is the whole point, because it is also the frame the export
        // will render. Audio, which runs on wall time, stays in sync
        // to within one frame because the controller jumps to wall
        // time at each sample.
        if is_controller_playing {
            let mode = self.config_manager.active_config().render_mode;
            let ready = match mode {
                // `escape_dirty` is the frame loop's "not settled yet"
                // flag: it is cleared only when render() reports final.
                crate::scene::transforms::RenderMode::Escape => !self.escape_dirty,
                // A committed target that has not been cleared means
                // the grid is still walking toward it.
                #[cfg(feature = "engine-sim")]
                crate::scene::transforms::RenderMode::Simulation => {
                    self.sim_timeline_target.is_none()
                }
                _ => true,
            };
            if ready && self.paced_anim_pending == 0.0 {
                // The common case: nothing to wait for, advance now.
                self.advance_animation(delta_time);
            } else if let Some(dt) =
                paced_playback_tick(&mut self.paced_anim_pending, delta_time, ready)
            {
                self.advance_animation(dt);
            }
        }

        is_controller_playing
    }

    /// Handle animation start transition.
    ///
    /// Called when controller starts playing but FSM not yet in animation mode.
    fn handle_animation_start(&mut self, is_playing: bool, was_animating: bool) {
        if is_playing && !was_animating {
            self.render_mode
                .enter_animation(self.config_manager.active_config());
            // Enable animation mode in ConfigManager - UI changes become silent (no undo)
            self.config_manager.set_animation_mode(true);
            // Note: Overwrite mode is automatically enabled during animation (see should_use_overwrite)

            // Start audio playback if sync is enabled
            if self.animation_controller.sync_audio && self.audio_player.has_audio() {
                // Seek audio to current animation time before playing
                self.audio_player.seek(self.animation_controller.current_time);
                if let Err(e) = self.audio_player.play() {
                    log::warn!("Failed to start audio playback: {:?}", e);
                }
            }
        }
    }

    /// Handle animation stop/pause transition.
    ///
    /// Called when FSM was animating but controller is no longer playing.
    /// This catches manual stop/pause clicks from UI - auto-stop is handled in advance_animation().
    fn handle_animation_stop(&mut self, is_playing: bool, was_animating: bool) {
        if was_animating && !is_playing {
            // Escape playback banks wall time against the frame being
            // rendered; on stop that frame is never sampled, so drop
            // it rather than jump by it when playback resumes.
            self.paced_anim_pending = 0.0;
            // Disable animation mode before exit so undo entry creation works
            self.config_manager.set_animation_mode(false);
            self.handle_animation_exit();

            // Sync audio state with animation
            if self.animation_controller.sync_audio && self.audio_player.has_audio() {
                if self.animation_controller.state == PlaybackState::Stopped {
                    self.audio_player.stop();
                } else {
                    // Paused
                    self.audio_player.pause();
                }
            }

            // Only seek to t=0 when STOPPED (not paused)
            // When paused, the fractal should stay at the current timeline position
            if self.animation_controller.state == PlaybackState::Stopped {
                self.seek_to_animation_start();
            }
        }
    }

    /// Hand the timeline's step count to the grid -- or hold it.
    ///
    /// Called after track values have been applied to the config,
    /// from both playback and scrubbing, with how the time that
    /// produced them was moving. `sim::timeline_target_applies` is the
    /// rule: forward always, backward only on a discrete event, since
    /// backward means reseeding and re-running.
    ///
    /// Holding leaves the PREVIOUS commitment in place, so the picture
    /// stays where it was rather than freezing mid-restart.
    #[cfg(feature = "engine-sim")]
    pub(super) fn commit_timeline_sim_target(&mut self, motion: crate::sim::Motion) {
        let config = self.config_manager.active_config();
        if config.render_mode != crate::scene::transforms::RenderMode::Simulation {
            return;
        }
        let target = config.sim.steps;
        // Where the field actually is. A renderer that has not been
        // built yet is at 0, so a first target is always forward.
        let index = self
            .sim_renderer
            .as_ref()
            .map_or(0, |s| s.step_index());
        if crate::sim::timeline_target_applies(index, target, motion) {
            self.sim_timeline_target = Some(target);
        }
    }

    #[cfg(not(feature = "engine-sim"))]
    pub(super) fn commit_timeline_sim_target(&mut self, _motion: crate::sim::Motion) {}

    /// Advance animation playback and apply values.
    ///
    /// Updates animation time, checks for auto-stop, and applies animated values to config.
    fn advance_animation(&mut self, delta_time: f64) {
        // How the clock moved decides whether a BACKWARD simulation
        // step target is applied or held (`playback_motion`), so it is
        // sampled around the update rather than after it.
        let time_before = self.animation_controller.current_time;
        let dir_before = self.animation_controller.direction();
        // Update animation time
        self.animation_controller.update(delta_time);
        let motion = playback_motion(
            time_before,
            dir_before,
            self.animation_controller.current_time,
            self.animation_controller.direction(),
        );

        // Sync audio position with animation time
        if self.animation_controller.sync_audio && self.audio_player.has_audio() {
            self.audio_player.sync_to_time(self.animation_controller.current_time);
        }

        // Check if animation auto-stopped (LoopMode::Once reached end)
        let auto_stopped = self.animation_controller.state != PlaybackState::Playing;
        if auto_stopped {
            self.paced_anim_pending = 0.0;
            // Disable animation mode before exit so undo entry creation works
            self.config_manager.set_animation_mode(false);
            // Animation finished naturally - exit animation mode and create undo snapshot
            self.handle_animation_exit();

            // Stop audio when animation auto-stops
            if self.animation_controller.sync_audio && self.audio_player.has_audio() {
                self.audio_player.stop();
            }

            // Seek to t=0
            self.seek_to_animation_start();
        } else {
            // Animation still playing - evaluate all tracks and apply values to ConfigManager
            self.apply_animated_values();
            self.commit_timeline_sim_target(motion);
        }
    }

    /// Apply animated values from current frame to ConfigManager.
    fn apply_animated_values(&mut self) {
        let frame_values = self.animation_controller.evaluate_frame(Some(&self.signal_manager));

        for (flame_target, path_str, json_value) in frame_values {
            // Parse the string key back to ConfigPath
            if let Some(path) = crate::config::ConfigPath::from_string_key(&path_str) {
                // Convert JSON value to ConfigValue
                if let Some(config_value) = crate::config::json_to_config_value(&json_value, &path)
                {
                    // Apply silently (no undo point) against the track's target flame
                    if let Err(e) = self
                        .config_manager
                        .update_param_silent_on(flame_target, path, config_value)
                    {
                        log::warn!(
                            "Animation: failed to update {:?}/{}: {}",
                            flame_target, path_str, e
                        );
                    }
                }
            } else {
                log::warn!("Animation: unknown path key: {}", path_str);
            }
        }

        // Sync flame from config (animation may have changed transform parameters)
        self.flame = self.config_manager.active_config().flame.clone();
    }

    /// Seek animation to t=0 and apply track values.
    ///
    /// Evaluates all animation tracks at t=0 on top of the current config.
    /// Any edits made during or before animation playback are preserved for
    /// non-animated parameters.
    fn seek_to_animation_start(&mut self) {
        if self.animation_controller.animation.is_some() {
            self.animation_controller.current_time = 0.0;
            self.apply_animated_values();
            // A deliberate landing on one time, so a backward step
            // target applies here: stopping at t = 0 restarts the run
            // rather than leaving the grid wherever playback got to.
            self.commit_timeline_sim_target(crate::sim::Motion::Discrete);
        }

        self.use_overwrite_next_frame = true;
        self.config_manager.request_reset();
    }
}

/// Was this playback tick a CONTINUOUS advance, or a discrete jump?
///
/// It decides whether a backward simulation step target is applied or
/// held (`sim::timeline_target_applies`). Going back means reseeding
/// and re-running, so applying a falling target on every frame of a
/// smooth backward run restarts the simulation on every frame -- which
/// is what ping-pong's whole backward leg would do, and what a track
/// authored to count DOWN would do under ordinary forward playback.
///
/// Two things count as discrete, and they are the two moments where a
/// restart is the RIGHT picture:
///
/// - a `PingPong` turnaround at the start, where the direction flips
///   from backward to forward. (The turnaround at the END is not: it
///   begins the backward leg, which is exactly what must be held.)
/// - a `Loop` wrap, where time falls back to the beginning while still
///   running forward. One restart per cycle, for a run starting over.
///
/// Everything else is continuous, including the whole backward leg.
pub(super) fn playback_motion(
    time_before: f64,
    dir_before: f64,
    time_after: f64,
    dir_after: f64,
) -> crate::sim::Motion {
    use crate::sim::Motion;
    let turned_forward = dir_before < 0.0 && dir_after > 0.0;
    let wrapped = time_after < time_before && dir_after > 0.0;
    if turned_forward || wrapped {
        Motion::Discrete
    } else {
        Motion::Continuous
    }
}

/// Paced playback: accumulate wall time while the current frame is
/// still coming into being, and hand it over in one piece once it is
/// ready. Returns the time to advance the controller by, or None while
/// the frame is not finished.
///
/// Serves both stateful engines -- an escape render that has not
/// settled, and a simulation grid still stepping toward this frame's
/// target. Named for the behaviour rather than the engine because it
/// is now shared.
///
/// A free function, not a method: the rule IS the design, and it
/// deserves a test that needs neither a GPU nor an App.
pub(super) fn paced_playback_tick(pending: &mut f64, delta: f64, settled: bool) -> Option<f64> {
    *pending += delta.max(0.0);
    if settled && *pending > 0.0 {
        Some(std::mem::take(pending))
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::paced_playback_tick;

    /// The two moments a restart is the right picture, and the whole
    /// backward leg that is not.
    #[test]
    fn only_a_turnaround_or_a_wrap_counts_as_a_discrete_jump() {
        use crate::app::animation_update::playback_motion;
        use crate::sim::Motion;

        // Ordinary forward playback.
        assert_eq!(playback_motion(1.0, 1.0, 1.1, 1.0), Motion::Continuous);

        // PingPong hits the END: direction flips to backward and time
        // starts falling. This BEGINS the backward leg -- the thing
        // that must be held, not applied.
        assert_eq!(playback_motion(9.9, 1.0, 9.95, -1.0), Motion::Continuous);
        // ...and every frame of that leg is continuous too.
        assert_eq!(playback_motion(5.0, -1.0, 4.9, -1.0), Motion::Continuous);
        // The turnaround at the START is the discrete one.
        assert_eq!(playback_motion(0.05, -1.0, 0.05, 1.0), Motion::Discrete);

        // Loop wraps: time falls back while still running forward.
        assert_eq!(playback_motion(9.95, 1.0, 0.05, 1.0), Motion::Discrete);

        // A backward track under FORWARD playback is continuous --
        // time is not what is going backwards, the TRACK is, and the
        // hold rule reads the target rather than the clock.
        assert_eq!(playback_motion(3.0, 1.0, 3.1, 1.0), Motion::Continuous);
    }

    /// Settle-then-jump: the controller is sampled once per COMPLETED
    /// escape frame, and advanced by everything that elapsed while it
    /// rendered. Sampling every display frame instead is what made
    /// escape playback a smear of partial renders of successive
    /// configs.
    #[test]
    fn escape_playback_samples_only_on_settled_frames() {
        let mut pending = 0.0;
        // Frame still rendering: bank the time, do not advance.
        assert_eq!(paced_playback_tick(&mut pending, 0.016, false), None);
        assert_eq!(paced_playback_tick(&mut pending, 0.016, false), None);
        // It settles: advance by everything banked, in one jump.
        let dt = paced_playback_tick(&mut pending, 0.016, true).expect("sample");
        assert!((dt - 0.048).abs() < 1e-9, "dt = {dt}");
        assert_eq!(pending, 0.0, "the bank empties on sample");

        // A settled frame with no elapsed time is not a sample point:
        // advancing by zero would re-render an identical frame.
        assert_eq!(paced_playback_tick(&mut pending, 0.0, true), None);

        // Long renders keep banking, however many frames they take --
        // playback slows down, it does not skip or blend.
        let mut pending = 0.0;
        for _ in 0..600 {
            assert_eq!(paced_playback_tick(&mut pending, 0.016, false), None);
        }
        let dt = paced_playback_tick(&mut pending, 0.016, true).expect("sample");
        assert!((dt - 9.616).abs() < 1e-6, "dt = {dt}");
    }
}
