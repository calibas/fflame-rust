/// Declarative slider binding system for ConfigManager
///
/// Provides a builder pattern for creating sliders that automatically wire to
/// ConfigManager for delta-based undo/redo.

use super::delta::{ConfigPath, ConfigValue, UpdateType};
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
    range: std::ops::RangeInclusive<f32>,
    label: String,
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
        Ok(Self {
            manager,
            path,
            current_value,
            range,
            label: label.into(),
            lazy: false,
        })
    }

    /// Enable lazy undo (throttled captures every 500ms)
    pub fn lazy(mut self, lazy: bool) -> Self {
        self.lazy = lazy;
        self
    }

    /// Render the slider and handle updates
    pub fn show(mut self, ui: &mut egui::Ui) -> Result<ConfigSliderResult, ConfigError> {
        let response = ui.add(egui::Slider::new(&mut self.current_value, self.range.clone()).text(&self.label));

        let changed = response.changed();
        let mut should_capture = false;
        let mut update_type = UpdateType::None;

        if changed {
            // Value changed - update config
            update_type = self.manager.update_param(
                self.path.clone(),
                self.current_value.into(),
                self.lazy,
            )?;
            should_capture = true; // ConfigManager already handled lazy throttling

            // On drag end, reset lazy timer to ensure final state is captured
            if response.drag_stopped() && self.lazy {
                self.manager.reset_lazy_undo();
            }
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
    ///     "Exposure"
    /// )?;
    /// ```
    fn config_slider(
        &mut self,
        manager: &mut ConfigManager,
        path: ConfigPath,
        range: std::ops::RangeInclusive<f32>,
        label: &str,
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
    /// if ui.lazy_slider(&mut manager, ConfigPath::Exposure, 0.1..=5.0, "Exposure")?.changed {
    ///     // Handle changes if needed
    /// }
    /// ```
    fn lazy_slider(
        &mut self,
        manager: &mut ConfigManager,
        path: ConfigPath,
        range: std::ops::RangeInclusive<f32>,
        label: &str,
    ) -> Result<ConfigSliderResult, ConfigError> {
        self.config_slider(manager, path, range, label)
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
    ) -> Result<ConfigSliderResult, ConfigError> {
        ConfigSlider::new(manager, path, range, label)?
            .lazy(true)
            .show(self)
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
            update_type = manager.update_param(path.clone(), current_value.into(), true)?;
            should_capture = true;

            // On drag end, reset lazy timer
            if response.drag_stopped() {
                manager.reset_lazy_undo();
            }
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
        assert_eq!(slider.current_value, 1.0); // Default exposure
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
