/// Lazy undo system for throttling undo captures during continuous UI interactions.
///
/// **Problem:** Sliders and drag interactions trigger `flame_changed = true` every frame,
/// filling the 50-state undo buffer in less than 1 second at 60 FPS.
///
/// **Solution:** Only capture undo state once per second during drag, always on release.
///
/// **Usage:**
/// ```rust,ignore
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
    /// ```rust,ignore
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
    /// ```rust,ignore
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

/// Result from a lazy slider operation
#[derive(Debug, Clone, Copy)]
pub struct LazySliderResult {
    /// Whether the value changed (user interacted with slider)
    pub changed: bool,
    /// Whether to capture undo state (throttled)
    pub should_capture: bool,
}

/// Extension trait for egui::Ui to add lazy slider methods
///
/// This eliminates the need for manual flag management - just call `ui.lazy_slider()`
/// and it returns both whether the value changed AND whether to capture undo.
pub trait LazyUndoUi {
    /// Add a slider with lazy undo capture.
    ///
    /// **Usage:**
    /// ```rust,ignore
    /// let result = ui.lazy_slider(&mut lazy_undo, &mut value, 0.0..=1.0, "Exposure");
    /// if result.should_capture {
    ///     app.capture_state();
    /// }
    /// if result.changed {
    ///     needs_update = true;
    /// }
    /// ```
    fn lazy_slider<Num: egui::emath::Numeric>(
        &mut self,
        lazy_undo: &mut LazyUndoHelper,
        value: &mut Num,
        range: std::ops::RangeInclusive<Num>,
        text: &str,
    ) -> LazySliderResult;

    /// Add a drag value with lazy undo capture.
    fn lazy_drag_value<Num: egui::emath::Numeric>(
        &mut self,
        lazy_undo: &mut LazyUndoHelper,
        value: &mut Num,
    ) -> LazySliderResult;
}

impl LazyUndoUi for egui::Ui {
    fn lazy_slider<Num: egui::emath::Numeric>(
        &mut self,
        lazy_undo: &mut LazyUndoHelper,
        value: &mut Num,
        range: std::ops::RangeInclusive<Num>,
        text: &str,
    ) -> LazySliderResult {
        let response = self.add(egui::Slider::new(value, range).text(text));
        let changed = response.changed();
        let should_capture = lazy_undo.should_capture_for_widget(&response);

        LazySliderResult {
            changed,
            should_capture,
        }
    }

    fn lazy_drag_value<Num: egui::emath::Numeric>(
        &mut self,
        lazy_undo: &mut LazyUndoHelper,
        value: &mut Num,
    ) -> LazySliderResult {
        let response = self.add(egui::DragValue::new(value));
        let changed = response.changed();
        let should_capture = lazy_undo.should_capture_for_widget(&response);

        LazySliderResult {
            changed,
            should_capture,
        }
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
