/// Lazy undo system for throttling undo captures during continuous UI interactions.
///
/// **Problem:** Sliders and drag interactions trigger `flame_changed = true` every frame,
/// filling the 50-state undo buffer in less than 1 second at 60 FPS.
///
/// **Solution:** Only capture undo state once per second during drag, always on release.
///
/// **Usage:**
/// ```rust
/// // In UI code (where you have egui::Ui):
/// let mut lazy = LazyUndoHelper::new();
///
/// let response = ui.add(egui::Slider::new(&mut value, 0.0..=1.0));
/// if lazy.should_capture_for_widget(&response) {
///     *flame_changed = true;
/// }
/// ```
use web_time::Instant;

/// Helper for implementing lazy undo captures in UI interactions.
///
/// Tracks drag state and throttles undo captures to once per second while dragging,
/// always capturing on release.
pub struct LazyUndoHelper {
    /// Time when we last captured an undo point during this drag
    last_capture_time: Option<Instant>,
    /// Whether we're currently dragging
    is_dragging: bool,
    /// Throttle interval (1 second)
    throttle_duration: std::time::Duration,
}

impl LazyUndoHelper {
    /// Create a new lazy undo helper with default 1-second throttle
    pub fn new() -> Self {
        Self {
            last_capture_time: None,
            is_dragging: false,
            throttle_duration: std::time::Duration::from_secs(1),
        }
    }

    /// Create with custom throttle duration
    pub fn with_throttle(throttle_secs: u64) -> Self {
        Self {
            last_capture_time: None,
            is_dragging: false,
            throttle_duration: std::time::Duration::from_secs(throttle_secs),
        }
    }

    /// Check if we should capture undo state based on widget response.
    ///
    /// **Returns `true` when:**
    /// - First frame of drag (drag started)
    /// - During drag, if 1+ seconds elapsed since last capture
    /// - Frame when drag ends (always capture final state)
    ///
    /// **Usage:**
    /// ```rust
    /// let response = ui.add(egui::Slider::new(&mut value, 0.0..=1.0));
    /// if lazy.should_capture_for_widget(&response) {
    ///     *flame_changed = true;
    /// }
    /// ```
    pub fn should_capture_for_widget(&mut self, response: &egui::Response) -> bool {
        let now = Instant::now();

        // Detect drag state transitions
        let was_dragging = self.is_dragging;
        self.is_dragging = response.dragged();

        if self.is_dragging {
            // Currently dragging
            if !was_dragging {
                // Drag just started - always capture
                self.last_capture_time = Some(now);
                return true;
            } else {
                // Continuing drag - throttle by time
                if let Some(last_time) = self.last_capture_time {
                    let elapsed = now.duration_since(last_time);
                    if elapsed >= self.throttle_duration {
                        // Time to capture again
                        self.last_capture_time = Some(now);
                        return true;
                    }
                } else {
                    // No last capture time (shouldn't happen, but handle gracefully)
                    self.last_capture_time = Some(now);
                    return true;
                }
            }
        } else if was_dragging {
            // Drag just ended - always capture final state
            self.last_capture_time = None;
            return true;
        }

        // No drag activity or already captured recently
        false
    }

    /// Manual API for custom drag detection (when not using egui::Response).
    ///
    /// **Usage:**
    /// ```rust
    /// let mut lazy = LazyUndoHelper::new();
    ///
    /// if mouse_down {
    ///     if lazy.should_capture_for_drag_state(true) {
    ///         *flame_changed = true;
    ///     }
    /// } else if was_mouse_down {
    ///     if lazy.should_capture_for_drag_state(false) {
    ///         *flame_changed = true;
    ///     }
    /// }
    /// ```
    pub fn should_capture_for_drag_state(&mut self, currently_dragging: bool) -> bool {
        let now = Instant::now();
        let was_dragging = self.is_dragging;
        self.is_dragging = currently_dragging;

        if currently_dragging {
            if !was_dragging {
                // Drag started
                self.last_capture_time = Some(now);
                return true;
            } else {
                // Continuing drag - throttle
                if let Some(last_time) = self.last_capture_time {
                    let elapsed = now.duration_since(last_time);
                    if elapsed >= self.throttle_duration {
                        self.last_capture_time = Some(now);
                        return true;
                    }
                }
            }
        } else if was_dragging {
            // Drag ended
            self.last_capture_time = None;
            return true;
        }

        false
    }

    /// Reset the helper state (useful when switching UI contexts)
    pub fn reset(&mut self) {
        self.last_capture_time = None;
        self.is_dragging = false;
    }
}

impl Default for LazyUndoHelper {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn test_drag_start_capture() {
        let mut helper = LazyUndoHelper::new();

        // First drag should capture
        assert!(helper.should_capture_for_drag_state(true));
    }

    #[test]
    fn test_drag_continue_throttle() {
        let mut helper = LazyUndoHelper::with_throttle(0); // 0 second throttle for testing

        // Start drag
        assert!(helper.should_capture_for_drag_state(true));

        // Sleep to ensure time advances (on some systems Instant::now() can be same)
        thread::sleep(std::time::Duration::from_millis(1));

        // Continue drag should capture (throttle expired due to sleep)
        assert!(helper.should_capture_for_drag_state(true));
    }

    #[test]
    fn test_drag_end_capture() {
        let mut helper = LazyUndoHelper::new();

        // Start drag
        assert!(helper.should_capture_for_drag_state(true));

        // Continue drag (within throttle)
        assert!(!helper.should_capture_for_drag_state(true));

        // End drag - should always capture
        assert!(helper.should_capture_for_drag_state(false));
    }

    #[test]
    fn test_reset() {
        let mut helper = LazyUndoHelper::new();

        // Start drag
        helper.should_capture_for_drag_state(true);

        // Reset
        helper.reset();

        // Should treat next drag as new
        assert!(helper.should_capture_for_drag_state(true));
    }
}
