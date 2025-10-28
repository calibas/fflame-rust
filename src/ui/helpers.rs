//! UI helper functions and builders for consistent UI patterns
//!
//! This module provides reusable components that encapsulate common UI patterns
//! to reduce code duplication and ensure consistency across the application.

use egui::Ui;

/// Builder for sliders with automatic change tracking
///
/// Provides a fluent interface for creating sliders that automatically
/// update change flags when modified.
///
/// # Example
/// ```rust,ignore
/// TrackedSlider::new(&mut exposure, 0.1..=5.0)
///     .text("Exposure")
///     .hover_text("Controls image brightness")
///     .show(ui, &mut exposure_changed);
/// ```
pub struct TrackedSlider<'a, T: egui::emath::Numeric> {
    value: &'a mut T,
    range: std::ops::RangeInclusive<T>,
    text: Option<String>,
    logarithmic: bool,
    suffix: Option<&'static str>,
    hover_text: Option<String>,
}

impl<'a, T: egui::emath::Numeric> TrackedSlider<'a, T> {
    /// Create a new tracked slider
    pub fn new(value: &'a mut T, range: std::ops::RangeInclusive<T>) -> Self {
        Self {
            value,
            range,
            text: None,
            logarithmic: false,
            suffix: None,
            hover_text: None,
        }
    }

    /// Set the label text for the slider
    pub fn text(mut self, text: impl Into<String>) -> Self {
        self.text = Some(text.into());
        self
    }

    /// Enable logarithmic scaling
    pub fn logarithmic(mut self, log: bool) -> Self {
        self.logarithmic = log;
        self
    }

    /// Add a suffix to the displayed value (e.g., "°", "%", "ms")
    pub fn suffix(mut self, suffix: &'static str) -> Self {
        self.suffix = Some(suffix);
        self
    }

    /// Add hover text (tooltip) to the slider
    pub fn hover_text(mut self, text: impl Into<String>) -> Self {
        self.hover_text = Some(text.into());
        self
    }

    /// Render the slider and update the change flag if modified
    pub fn show(self, ui: &mut Ui, changed: &mut bool) {
        let mut slider = egui::Slider::new(self.value, self.range);

        if let Some(text) = self.text {
            slider = slider.text(text);
        }
        if self.logarithmic {
            slider = slider.logarithmic(true);
        }
        if let Some(suffix) = self.suffix {
            slider = slider.suffix(suffix);
        }

        let mut response = ui.add(slider);

        if let Some(hover) = self.hover_text {
            response = response.on_hover_text(hover);
        }

        if response.changed() {
            *changed = true;
        }
    }
}

/// Builder for drag values with automatic change tracking
///
/// Similar to TrackedSlider but for drag values (click and drag to modify).
pub struct TrackedDragValue<'a, T: egui::emath::Numeric> {
    value: &'a mut T,
    speed: f64,
    range: Option<std::ops::RangeInclusive<T>>,
    prefix: Option<String>,
    suffix: Option<&'static str>,
}

impl<'a, T: egui::emath::Numeric> TrackedDragValue<'a, T> {
    /// Create a new tracked drag value
    pub fn new(value: &'a mut T) -> Self {
        Self {
            value,
            speed: 0.01,
            range: None,
            prefix: None,
            suffix: None,
        }
    }

    /// Set the drag speed (how fast the value changes when dragging)
    pub fn speed(mut self, speed: f64) -> Self {
        self.speed = speed;
        self
    }

    /// Constrain the value to a specific range
    pub fn range(mut self, range: std::ops::RangeInclusive<T>) -> Self {
        self.range = Some(range);
        self
    }

    /// Add a prefix to the displayed value (e.g., "X:", "Speed:")
    pub fn prefix(mut self, prefix: impl Into<String>) -> Self {
        self.prefix = Some(prefix.into());
        self
    }

    /// Add a suffix to the displayed value
    pub fn suffix(mut self, suffix: &'static str) -> Self {
        self.suffix = Some(suffix);
        self
    }

    /// Render the drag value and update the change flag if modified
    pub fn show(self, ui: &mut Ui, changed: &mut bool) {
        if let Some(prefix) = self.prefix {
            ui.label(prefix);
        }

        let mut drag = egui::DragValue::new(self.value).speed(self.speed);

        if let Some(range) = self.range {
            drag = drag.range(range);
        }
        if let Some(suffix) = self.suffix {
            drag = drag.suffix(suffix);
        }

        if ui.add(drag).changed() {
            *changed = true;
        }
    }
}

/// Simple button with change tracking
///
/// Returns true and sets the change flag if clicked.
///
/// # Example
/// ```rust,ignore
/// if tracked_button(ui, "Reset", &mut reset_requested) {
///     // Handle reset
/// }
/// ```
pub fn tracked_button(ui: &mut Ui, text: &str, changed: &mut bool) -> bool {
    if ui.button(text).clicked() {
        *changed = true;
        true
    } else {
        false
    }
}

/// Checkbox with change tracking
///
/// # Example
/// ```rust,ignore
/// tracked_checkbox(ui, &mut enabled, "Enable feature", &mut settings_changed);
/// ```
pub fn tracked_checkbox(ui: &mut Ui, value: &mut bool, text: &str, changed: &mut bool) {
    if ui.checkbox(value, text).changed() {
        *changed = true;
    }
}

/// Angle slider that displays degrees but stores radians
///
/// This is a common pattern in the UI where we want to show user-friendly
/// degree values but store radians internally for calculations.
///
/// # Example
/// ```rust,ignore
/// angle_slider_radians(ui, &mut rotation, "Rotation", &mut view_changed);
/// ```
pub fn angle_slider_radians(ui: &mut Ui, radians: &mut f32, label: &str, changed: &mut bool) {
    let mut degrees = radians.to_degrees();
    if ui
        .add(egui::Slider::new(&mut degrees, -180.0..=180.0).text(label).suffix("°"))
        .changed()
    {
        *radians = degrees.to_radians();
        *changed = true;
    }
}

/// Zoom controls with consistent step size and layout
///
/// Provides zoom in/out buttons and a drag value for precise control.
///
/// # Example
/// ```rust,ignore
/// ui.label("Zoom");
/// zoom_controls(ui, zoom, view_changed);
/// ```
pub fn zoom_controls(ui: &mut Ui, zoom: &mut f32, changed: &mut bool) {
    ui.horizontal(|ui| {
        if tracked_button(ui, "➕ Zoom In", changed) {
            *zoom *= 1.5;
        }
        if tracked_button(ui, "➖ Zoom Out", changed) {
            *zoom /= 1.5;
        }
        ui.label("Value:");
        if ui.add(egui::DragValue::new(zoom).speed(0.01).range(0.01..=10000.0)).changed() {
            *changed = true;
        }
    });
}

/// Horizontal row of selectable labels (radio button style)
///
/// Useful for mode selection (e.g., 2D/3D, Linear/Logarithmic).
///
/// # Example
/// ```rust,ignore
/// let modes = [("2D", RenderMode::TwoD), ("3D", RenderMode::ThreeD)];
/// selectable_row(ui, &mut render_mode, &modes, &mut mode_changed);
/// ```
pub fn selectable_row<T: PartialEq + Copy>(
    ui: &mut Ui,
    current: &mut T,
    options: &[(&str, T)],
    changed: &mut bool,
) {
    ui.horizontal(|ui| {
        for (label, value) in options {
            if ui.selectable_label(*current == *value, *label).clicked() {
                *current = *value;
                *changed = true;
            }
        }
    });
}

/// Color picker with change tracking
///
/// Wraps egui's color_edit_button_rgb with automatic change tracking.
pub fn tracked_color_picker(ui: &mut Ui, color: &mut [f32; 3], changed: &mut bool) {
    if ui.color_edit_button_rgb(color).changed() {
        *changed = true;
    }
}

/// Text edit with change tracking
///
/// Single-line text editor with automatic change tracking.
pub fn tracked_text_edit(ui: &mut Ui, text: &mut String, changed: &mut bool) {
    if ui.text_edit_singleline(text).changed() {
        *changed = true;
    }
}

/// Collapsing header that tracks its open state
///
/// Convenience wrapper for CollapsingHeader with default_open option.
pub fn collapsing_section<R>(
    ui: &mut Ui,
    label: &str,
    default_open: bool,
    add_contents: impl FnOnce(&mut Ui) -> R,
) -> Option<R> {
    egui::CollapsingHeader::new(label).default_open(default_open).show(ui, add_contents).body_returned
}
