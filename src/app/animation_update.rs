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

        // Process animation playback
        if is_controller_playing {
            self.advance_animation(delta_time);
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
        }
    }

    /// Handle animation stop/pause transition.
    ///
    /// Called when FSM was animating but controller is no longer playing.
    /// This catches manual stop/pause clicks from UI - auto-stop is handled in advance_animation().
    fn handle_animation_stop(&mut self, is_playing: bool, was_animating: bool) {
        if was_animating && !is_playing {
            // Disable animation mode before exit so undo entry creation works
            self.config_manager.set_animation_mode(false);
            self.handle_animation_exit();

            // Only restore base config when animation is STOPPED (not paused)
            // When paused, the fractal should stay at the current timeline position
            if self.animation_controller.state == PlaybackState::Stopped {
                self.restore_base_config();
            }
        }
    }

    /// Advance animation playback and apply values.
    ///
    /// Updates animation time, checks for auto-stop, and applies animated values to config.
    fn advance_animation(&mut self, delta_time: f64) {
        // Update animation time
        self.animation_controller.update(delta_time);

        // Check if animation auto-stopped (LoopMode::Once reached end)
        let auto_stopped = self.animation_controller.state != PlaybackState::Playing;
        if auto_stopped {
            // Disable animation mode before exit so undo entry creation works
            self.config_manager.set_animation_mode(false);
            // Animation finished naturally - exit animation mode and create undo snapshot
            self.handle_animation_exit();

            // Restore base config when animation stops (returns to original state)
            self.restore_base_config();
        } else {
            // Animation still playing - evaluate all tracks and apply values to ConfigManager
            self.apply_animated_values();
        }
    }

    /// Apply animated values from current frame to ConfigManager.
    fn apply_animated_values(&mut self) {
        let frame_values = self.animation_controller.evaluate_frame();

        for (path_str, json_value) in frame_values {
            // Parse the string key back to ConfigPath
            if let Some(path) = crate::config::ConfigPath::from_string_key(&path_str) {
                // Convert JSON value to ConfigValue
                if let Some(config_value) = crate::config::json_to_config_value(&json_value, &path)
                {
                    // Apply silently (no undo point)
                    if let Err(e) = self.config_manager.update_param_silent(path, config_value) {
                        log::warn!("Animation: failed to update {}: {}", path_str, e);
                    }
                }
            } else {
                log::warn!("Animation: unknown path key: {}", path_str);
            }
        }

        // Sync flame from config (animation may have changed transform parameters)
        self.flame = self.config_manager.active_config().flame.clone();
    }

    /// Restore base config after animation stops.
    fn restore_base_config(&mut self) {
        if let Some(ref animation) = self.animation_controller.animation {
            if let Some(ref base_config) = animation.base_config {
                // Load the base config silently (the undo entry was already created by handle_animation_exit)
                if let Err(e) = self.config_manager.load_config_silent(base_config.clone()) {
                    log::error!("Failed to restore base config: {}", e);
                }
                self.flame = base_config.flame.clone();
                self.use_overwrite_next_frame = true;

                // Reset accumulation to start fresh (same as Reset Accumulation button)
                self.config_manager.request_reset();
            }
        }
    }
}
