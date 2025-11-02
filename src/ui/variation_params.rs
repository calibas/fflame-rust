//! Variation parameter rendering
//!
//! This module provides unified rendering for variation parameters,
//! eliminating the need to duplicate parameter UI code across different
//! variation categories.

use crate::{
    config::{ConfigManager, ConfigPath, UpdateType},
    variations::{ParamType, VariationParameter},
};

/// Render parameter controls for an active variation
///
/// This function handles all parameter types (Float, Integer, Angle) and
/// automatically updates the config via ConfigManager with lazy undo support.
///
/// # Arguments
/// * `ui` - The egui UI context
/// * `config_manager` - The configuration manager
/// * `transform_index` - Index of the transform being edited
/// * `var_name` - Name of the variation (e.g., "julian", "blob")
/// * `parameters` - List of parameters for this variation
///
/// # Returns
/// The highest UpdateType from all parameter changes
///
/// # Example
/// ```rust,ignore
/// if value.abs() > 1e-6 && !var_info.parameters.is_empty() {
///     ui.indent(format!("params_{}", var_info.name), |ui| {
///         let update = render_variation_params(ui, config_manager, i, &var_info.name, &var_info.parameters);
///         max_update = max_update.max(update);
///     });
/// }
/// ```
pub fn render_variation_params(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    transform_index: usize,
    var_name: &str,
    parameters: &[VariationParameter],
) -> UpdateType {
    let mut max_update = UpdateType::None;

    for param in parameters {
        // Get current value from active config (for live preview)
        let transform = &config_manager.active_config().flame.transforms[transform_index];
        let mut param_value = transform.get_variation_param_or_default(
            var_name,
            &param.name,
            &crate::variations::global_registry(),
        );

        let (param_changed, dragged, drag_stopped) = match param.param_type {
            ParamType::Float => render_float_param(ui, param, &mut param_value),
            ParamType::Integer => render_integer_param(ui, param, &mut param_value),
            ParamType::Angle => render_angle_param(ui, param, &mut param_value),
        };

        let path = ConfigPath::TransformVariationParam {
            index: transform_index,
            variation: var_name.to_string(),
            param: param.name.clone(),
        };

        if param_changed {
            // Update via ConfigManager with lazy undo only during drag
            if let Ok(update_type) = config_manager.update_param(path.clone(), param_value.into(), dragged) {
                max_update = max_update.max(update_type);
            }
        }

        if drag_stopped {
            let _ = config_manager.force_commit_preview(&path);
        }
    }

    max_update
}

/// Render a float parameter slider
/// Returns (changed, dragged, drag_stopped)
fn render_float_param(ui: &mut egui::Ui, param: &VariationParameter, value: &mut f32) -> (bool, bool, bool) {
    let min = param.min_value.unwrap_or(-10.0);
    let max = param.max_value.unwrap_or(10.0);
    let response = ui.add(
        egui::Slider::new(value, min..=max)
            .text(&param.display_name)
            .step_by(0.01),
    );
    (response.changed(), response.dragged(), response.drag_stopped())
}

/// Render an integer parameter slider
/// Returns (changed, dragged, drag_stopped)
fn render_integer_param(ui: &mut egui::Ui, param: &VariationParameter, value: &mut f32) -> (bool, bool, bool) {
    let min = param.min_value.unwrap_or(1.0) as i32;
    let max = param.max_value.unwrap_or(10.0) as i32;
    let mut int_val = *value as i32;
    let response = ui.add(egui::Slider::new(&mut int_val, min..=max).text(&param.display_name));
    *value = int_val as f32;
    (response.changed(), response.dragged(), response.drag_stopped())
}

/// Render an angle parameter slider (displays degrees, stores as value)
/// Returns (changed, dragged, drag_stopped)
fn render_angle_param(ui: &mut egui::Ui, param: &VariationParameter, value: &mut f32) -> (bool, bool, bool) {
    let min = param.min_value.unwrap_or(0.0);
    let max = param.max_value.unwrap_or(360.0);
    let response = ui.add(egui::Slider::new(value, min..=max).text(&param.display_name).suffix("°"));
    (response.changed(), response.dragged(), response.drag_stopped())
}
