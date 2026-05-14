/// Declarative slider binding system for ConfigManager
///
/// Provides a builder pattern for creating sliders that automatically wire to
/// ConfigManager for delta-based undo/redo.

use super::delta::{ConfigPath, UpdateType};
use super::manager::{ConfigError, ConfigManager};

/// Result from a config slider operation
#[derive(Debug, Clone, Copy)]
pub struct ConfigSliderResult {
    /// Whether the value changed (user interacted with slider)
    pub changed: bool,
    /// Whether to capture undo state (throttled via lazy undo)
    pub should_capture: bool,
    /// What kind of update is needed
    pub update_type: UpdateType,
}

/// Builder for creating sliders bound to config parameters
pub struct ConfigSlider<'a> {
    manager: &'a mut ConfigManager,
    path: ConfigPath,
    current_value: f32,
    drag_start_value: Option<f32>,  // Value when drag started
    range: std::ops::RangeInclusive<f32>,
    label: String,
    tooltip: Option<String>,
    lazy: bool,
}

impl<'a> ConfigSlider<'a> {
    /// Create a new slider bound to a config parameter
    ///
    /// # Errors
    /// Returns error if the path doesn't map to an f32 value
    pub fn new(
        manager: &'a mut ConfigManager,
        path: ConfigPath,
        range: std::ops::RangeInclusive<f32>,
        label: impl Into<String>,
    ) -> Result<Self, ConfigError> {
        let current_value: f32 = manager.get_value(&path)?.try_into()?;
        log::trace!("ConfigSlider::new: {} read value {:.3} from ConfigManager", path, current_value);
        Ok(Self {
            manager,
            path,
            current_value,
            drag_start_value: None,
            range,
            label: label.into(),
            tooltip: None,
            lazy: false,
        })
    }

    /// Enable lazy undo (throttled captures every 500ms)
    pub fn lazy(mut self, lazy: bool) -> Self {
        self.lazy = lazy;
        self
    }

    /// Set tooltip text shown on hover
    pub fn tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(tooltip.into());
        self
    }

    /// Render the slider and handle updates
    pub fn show(mut self, ui: &mut egui::Ui) -> Result<ConfigSliderResult, ConfigError> {
        log::trace!("ConfigSlider::show: {} = {:.3}", self.path, self.current_value);

        // Track drag start value for lazy undo
        if self.lazy {
            let id = ui.make_persistent_id(format!("drag_start_{}", self.path));
            let drag_start = ui.data_mut(|d| {
                d.get_persisted::<f32>(id)
            });
            self.drag_start_value = drag_start;
        }

        let response = ui.add(egui::Slider::new(&mut self.current_value, self.range.clone()).text(&self.label));

        // Apply tooltip if set
        if let Some(ref tooltip) = self.tooltip {
            response.clone().on_hover_text(tooltip);
        }

        let changed = response.changed();
        let is_being_dragged = response.dragged();
        let drag_stopped = response.drag_stopped();

        if changed {
            log::trace!("  Slider changed to: {:.3}", self.current_value);
        }
        if drag_stopped {
            log::trace!("  Drag stopped at: {:.3}", self.current_value);
        }

        let mut should_capture = false;
        let mut update_type = UpdateType::None;

        // For lazy sliders: store value when drag starts
        if self.lazy && is_being_dragged && self.drag_start_value.is_none() {
            let id = ui.make_persistent_id(format!("drag_start_{}", self.path));
            ui.data_mut(|d| {
                d.insert_persisted(id, self.current_value);
            });
            self.drag_start_value = Some(self.current_value);
            log::trace!("  Drag started at: {:.3}", self.current_value);
        }

        // Handle value changes during drag (lazy throttled)
        if changed {
            update_type = self.manager.update_param(
                self.path.clone(),
                self.current_value.into())?;
            should_capture = true;
        }

        // Handle drag end: force commit preview to current
        if drag_stopped && self.lazy {
            log::trace!("  Force committing preview on drag end");
            let id = ui.make_persistent_id(format!("drag_start_{}", self.path));
            ui.data_mut(|d| {
                d.remove::<f32>(id);  // Clear drag start value
            });

            // Force commit preview to current (captures final delta if changed)
            update_type = update_type.merge(self.manager.force_commit_preview(&self.path)?);
        }

        Ok(ConfigSliderResult {
            changed,
            should_capture,
            update_type,
        })
    }
}

/// Extension trait for egui::Ui to add config slider methods
///
/// Provides convenient methods for creating config-bound sliders without
/// manually building ConfigSlider.
pub trait ConfigSliderUi {
    /// Add a config-bound slider (lazy undo enabled by default)
    ///
    /// # Example
    /// ```ignore
    /// let result = ui.config_slider(
    ///     &mut config_manager,
    ///     ConfigPath::Exposure,
    ///     0.1..=5.0,
    ///     "Exposure",
    ///     Some("Tooltip text")
    /// )?;
    /// ```
    fn config_slider(
        &mut self,
        manager: &mut ConfigManager,
        path: ConfigPath,
        range: std::ops::RangeInclusive<f32>,
        label: &str,
        tooltip: Option<&str>,
    ) -> Result<ConfigSliderResult, ConfigError>;

    /// Add a config-bound drag value (lazy undo enabled by default)
    fn config_drag_value(
        &mut self,
        manager: &mut ConfigManager,
        path: ConfigPath,
        speed: f32,
        label: &str,
    ) -> Result<ConfigSliderResult, ConfigError>;
}

/// Extension trait to simplify ConfigSlider usage with lazy undo
///
/// This trait provides shorthand methods that match the ergonomics of the old
/// LazyUndoHelper pattern, making migration easier.
pub trait LazyUndoUi: ConfigSliderUi {
    /// Add a slider with lazy undo (500ms throttle)
    ///
    /// # Example
    /// ```ignore
    /// if ui.lazy_slider(&mut manager, ConfigPath::Exposure, 0.1..=5.0, "Exposure", None)?.changed {
    ///     // Handle changes if needed
    /// }
    /// ```
    fn lazy_slider(
        &mut self,
        manager: &mut ConfigManager,
        path: ConfigPath,
        range: std::ops::RangeInclusive<f32>,
        label: &str,
        tooltip: Option<&str>,
    ) -> Result<ConfigSliderResult, ConfigError> {
        self.config_slider(manager, path, range, label, tooltip)
    }

    /// Add a drag value with lazy undo (500ms throttle)
    fn lazy_drag(
        &mut self,
        manager: &mut ConfigManager,
        path: ConfigPath,
        speed: f32,
        label: &str,
    ) -> Result<ConfigSliderResult, ConfigError> {
        self.config_drag_value(manager, path, speed, label)
    }
}

impl ConfigSliderUi for egui::Ui {
    fn config_slider(
        &mut self,
        manager: &mut ConfigManager,
        path: ConfigPath,
        range: std::ops::RangeInclusive<f32>,
        label: &str,
        tooltip: Option<&str>,
    ) -> Result<ConfigSliderResult, ConfigError> {
        let mut slider = ConfigSlider::new(manager, path, range, label)?
            .lazy(true);
        if let Some(tip) = tooltip {
            slider = slider.tooltip(tip);
        }
        slider.show(self)
    }

    fn config_drag_value(
        &mut self,
        manager: &mut ConfigManager,
        path: ConfigPath,
        speed: f32,
        label: &str,
    ) -> Result<ConfigSliderResult, ConfigError> {
        // Get current value
        let mut current_value: f32 = manager.get_value(&path)?.try_into()?;

        // Create drag value with label
        let response = if label.is_empty() {
            self.add(egui::DragValue::new(&mut current_value).speed(speed))
        } else {
            self.horizontal(|ui| {
                ui.label(label);
                ui.add(egui::DragValue::new(&mut current_value).speed(speed))
            })
            .inner
        };

        let changed = response.changed();
        let mut should_capture = false;
        let mut update_type = UpdateType::None;

        if changed {
            // Value changed - update config with lazy undo
            update_type = manager.update_param(path.clone(), current_value.into())?;
            should_capture = true;
        }

        Ok(ConfigSliderResult {
            changed,
            should_capture,
            update_type,
        })
    }
}

impl LazyUndoUi for egui::Ui {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::FractalConfig;

    #[test]
    fn test_config_slider_creation() {
        let config = FractalConfig::default();
        let mut manager = ConfigManager::new(config);

        // Should be able to create slider for f32 path
        let slider = ConfigSlider::new(
            &mut manager,
            ConfigPath::Exposure,
            0.1..=5.0,
            "Exposure",
        );
        assert!(slider.is_ok());

        let slider = slider.unwrap();
        assert_eq!(slider.current_value, crate::config::defaults::DEFAULT_EXPOSURE);
        assert!(!slider.lazy); // Default is not lazy
    }

    #[test]
    fn test_config_slider_lazy() {
        let config = FractalConfig::default();
        let mut manager = ConfigManager::new(config);

        let slider = ConfigSlider::new(
            &mut manager,
            ConfigPath::Exposure,
            0.1..=5.0,
            "Exposure",
        )
        .unwrap()
        .lazy(true);

        assert!(slider.lazy);
    }
}
