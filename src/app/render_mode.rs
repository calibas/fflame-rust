//! Rendering mode state machine
//!
//! Manages transitions between different rendering modes (Normal, Animating, Overwrite)
//! and automatically creates undo snapshots at appropriate transition points.
//!
//! ## States
//!
//! - **Normal**: Standard editing mode. Changes create undo entries via ConfigManager.
//! - **Animating**: Animation playback. Changes are applied silently (no undo entries).
//!   When exiting, creates a single undo entry for the entire animation session.
//! - **Overwrite**: Live preview mode. Rapid parameter changes bypass normal undo coalescing
//!   for immediate visual feedback.
//!
//! ## Transitions
//!
//! ```text
//! ┌──────────┐  enter_animation()   ┌────────────┐
//! │  Normal  │ ──────────────────→  │  Animating │
//! │          │ ←──────────────────  │            │
//! └──────────┘  exit_animation()    └────────────┘
//!      ↑↓
//!  enter/exit_overwrite()
//!      ↑↓
//! ┌──────────┐
//! │ Overwrite│
//! └──────────┘
//! ```
//!
//! Note: Cannot enter Overwrite while Animating (animation takes priority).

use crate::config::FractalConfig;

/// Rendering mode states
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderModeState {
    /// Normal editing mode - changes tracked via ConfigManager
    Normal,
    /// Animation playback - changes applied silently, snapshot on exit
    Animating,
    /// Live preview/overwrite mode - rapid changes for immediate feedback
    Overwrite,
}

impl Default for RenderModeState {
    fn default() -> Self {
        Self::Normal
    }
}

/// Rendering mode state machine
///
/// Tracks the current rendering mode and manages transitions.
/// Stores snapshots needed for undo when exiting special modes.
#[derive(Default)]
pub struct RenderModeFSM {
    /// Current state
    state: RenderModeState,
    /// Config snapshot taken when entering Animating mode
    /// Used to create undo entry when exiting
    pre_animation_snapshot: Option<FractalConfig>,
    /// Config snapshot taken when entering Overwrite mode (if needed)
    pre_overwrite_snapshot: Option<FractalConfig>,
    /// Frames since exiting a low-density mode (Animating or Overwrite)
    /// Used to continue brightness boost while buffer rebuilds
    frames_since_low_density_exit: u32,
    /// Whether we were in a low-density mode before transitioning to Normal
    was_in_low_density_mode: bool,
}

/// Result of a state transition
#[derive(Debug)]
pub enum TransitionResult {
    /// No transition occurred (already in target state or invalid transition)
    NoChange,
    /// Transition occurred, no undo action needed
    Transitioned,
    /// Transition occurred, caller should create undo entry with these configs
    CreateUndo {
        before: FractalConfig,
        after: FractalConfig,
        description: String,
    },
}

impl RenderModeFSM {
    /// Create a new FSM in Normal state
    pub fn new() -> Self {
        Self::default()
    }

    /// Get the current state
    pub fn state(&self) -> RenderModeState {
        self.state
    }

    /// Check if currently in Normal mode
    pub fn is_normal(&self) -> bool {
        self.state == RenderModeState::Normal
    }

    /// Check if currently animating
    pub fn is_animating(&self) -> bool {
        self.state == RenderModeState::Animating
    }

    /// Check if currently in overwrite mode
    pub fn is_overwrite(&self) -> bool {
        self.state == RenderModeState::Overwrite
    }

    /// Enter animation mode
    ///
    /// Saves a snapshot of the current config for later undo creation.
    /// Does nothing if already animating.
    pub fn enter_animation(&mut self, current_config: &FractalConfig) -> TransitionResult {
        match self.state {
            RenderModeState::Animating => {
                // Already animating, no change
                TransitionResult::NoChange
            }
            RenderModeState::Normal | RenderModeState::Overwrite => {
                // Save snapshot before animation starts
                self.pre_animation_snapshot = Some(current_config.clone());
                self.state = RenderModeState::Animating;
                log::debug!("RenderMode: Entered Animating (saved pre-animation snapshot)");
                TransitionResult::Transitioned
            }
        }
    }

    /// Exit animation mode
    ///
    /// Returns a TransitionResult indicating whether an undo entry should be created.
    /// The caller is responsible for actually creating the undo entry via ConfigManager.
    pub fn exit_animation(&mut self, current_config: &FractalConfig) -> TransitionResult {
        if self.state != RenderModeState::Animating {
            return TransitionResult::NoChange;
        }

        self.state = RenderModeState::Normal;

        // Create undo entry if we have a pre-animation snapshot
        if let Some(before) = self.pre_animation_snapshot.take() {
            log::debug!("RenderMode: Exited Animating (creating undo snapshot)");
            TransitionResult::CreateUndo {
                before,
                after: current_config.clone(),
                description: "Animation".to_string(),
            }
        } else {
            log::warn!("RenderMode: Exited Animating but no pre-animation snapshot found");
            TransitionResult::Transitioned
        }
    }

    /// Enter overwrite mode
    ///
    /// Used for live preview of parameter changes.
    /// Cannot enter overwrite while animating.
    pub fn enter_overwrite(&mut self, _current_config: &FractalConfig) -> TransitionResult {
        match self.state {
            RenderModeState::Animating => {
                // Animation takes priority, don't enter overwrite
                TransitionResult::NoChange
            }
            RenderModeState::Overwrite => {
                // Already in overwrite
                TransitionResult::NoChange
            }
            RenderModeState::Normal => {
                self.state = RenderModeState::Overwrite;
                log::trace!("RenderMode: Entered Overwrite");
                TransitionResult::Transitioned
            }
        }
    }

    /// Exit overwrite mode
    ///
    /// Returns to Normal mode.
    pub fn exit_overwrite(&mut self) -> TransitionResult {
        if self.state != RenderModeState::Overwrite {
            return TransitionResult::NoChange;
        }

        self.state = RenderModeState::Normal;
        log::trace!("RenderMode: Exited Overwrite");
        TransitionResult::Transitioned
    }

    /// Check if we should apply changes silently (no undo tracking)
    ///
    /// Returns true when animating - animation changes shouldn't create
    /// individual undo entries.
    pub fn should_apply_silent(&self) -> bool {
        self.state == RenderModeState::Animating
    }

    /// Check if we should use overwrite mode for accumulation
    ///
    /// Returns true when in Overwrite or Animating mode - both need
    /// immediate visual feedback without waiting for accumulation.
    pub fn should_use_overwrite(&self) -> bool {
        matches!(self.state, RenderModeState::Overwrite | RenderModeState::Animating)
    }

    /// Update the brightness boost state each frame.
    ///
    /// Call this every frame to track how long since we exited a low-density mode.
    /// The `is_iterating` parameter indicates if normal accumulation is happening.
    pub fn update_brightness_state(&mut self, is_iterating: bool) {
        let is_low_density = self.should_use_overwrite();

        if is_low_density {
            // In Animating or Overwrite mode - buffer has low density
            self.was_in_low_density_mode = true;
            self.frames_since_low_density_exit = 0;
        } else if self.was_in_low_density_mode {
            // Just exited to Normal mode
            if is_iterating {
                // Normal accumulation is building density - count frames until buffer is full
                self.frames_since_low_density_exit += 1;
                // After ~8 frames, buffer has enough density (matches 8x boost factor)
                if self.frames_since_low_density_exit > 8 {
                    self.was_in_low_density_mode = false;
                }
            }
            // If not iterating (stopped at max_iterations), keep boost until we start iterating again
        }
    }

    /// Check if brightness boost should be applied.
    ///
    /// Returns true when the accumulation buffer has low density:
    /// - Currently in Animating or Overwrite mode
    /// - Recently exited and buffer is still rebuilding
    pub fn needs_brightness_boost(&self) -> bool {
        self.should_use_overwrite() || self.was_in_low_density_mode
    }

    /// Get debug info for brightness state
    pub fn brightness_debug(&self) -> (bool, u32, bool) {
        (self.was_in_low_density_mode, self.frames_since_low_density_exit, self.needs_brightness_boost())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FractalConfig;

    fn make_config() -> FractalConfig {
        FractalConfig::default()
    }

    #[test]
    fn test_initial_state() {
        let fsm = RenderModeFSM::new();
        assert!(fsm.is_normal());
        assert!(!fsm.is_animating());
        assert!(!fsm.is_overwrite());
    }

    #[test]
    fn test_enter_exit_animation() {
        let mut fsm = RenderModeFSM::new();
        let config = make_config();

        // Enter animation
        let result = fsm.enter_animation(&config);
        assert!(matches!(result, TransitionResult::Transitioned));
        assert!(fsm.is_animating());

        // Enter again should be no-op
        let result = fsm.enter_animation(&config);
        assert!(matches!(result, TransitionResult::NoChange));

        // Exit animation
        let result = fsm.exit_animation(&config);
        assert!(matches!(result, TransitionResult::CreateUndo { .. }));
        assert!(fsm.is_normal());
    }

    #[test]
    fn test_enter_exit_overwrite() {
        let mut fsm = RenderModeFSM::new();
        let config = make_config();

        // Enter overwrite
        let result = fsm.enter_overwrite(&config);
        assert!(matches!(result, TransitionResult::Transitioned));
        assert!(fsm.is_overwrite());

        // Exit overwrite
        let result = fsm.exit_overwrite();
        assert!(matches!(result, TransitionResult::Transitioned));
        assert!(fsm.is_normal());
    }

    #[test]
    fn test_cannot_enter_overwrite_while_animating() {
        let mut fsm = RenderModeFSM::new();
        let config = make_config();

        fsm.enter_animation(&config);
        assert!(fsm.is_animating());

        // Try to enter overwrite - should fail
        let result = fsm.enter_overwrite(&config);
        assert!(matches!(result, TransitionResult::NoChange));
        assert!(fsm.is_animating()); // Still animating
    }

    #[test]
    fn test_should_apply_silent() {
        let mut fsm = RenderModeFSM::new();
        let config = make_config();

        assert!(!fsm.should_apply_silent()); // Normal

        fsm.enter_overwrite(&config);
        assert!(!fsm.should_apply_silent()); // Overwrite doesn't silence

        fsm.exit_overwrite();
        fsm.enter_animation(&config);
        assert!(fsm.should_apply_silent()); // Animation silences
    }

    #[test]
    fn test_should_use_overwrite() {
        let mut fsm = RenderModeFSM::new();
        let config = make_config();

        assert!(!fsm.should_use_overwrite()); // Normal

        fsm.enter_overwrite(&config);
        assert!(fsm.should_use_overwrite()); // Overwrite mode

        fsm.exit_overwrite();
        fsm.enter_animation(&config);
        assert!(fsm.should_use_overwrite()); // Animation also uses overwrite
    }
}
