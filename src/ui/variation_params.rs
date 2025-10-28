//! Variation parameter rendering
//!
//! This module provides unified rendering for variation parameters,
//! eliminating the need to duplicate parameter UI code across different
//! variation categories.

use crate::{
    scene::transforms::Transform,
    variations::{ParamType, VariationParameter},
};

/// Render parameter controls for an active variation
///
/// This function handles all parameter types (Float, Integer, Angle) and
/// automatically updates the transform when parameters are modified.
///
/// # Arguments
/// * `ui` - The egui UI context
/// * `transform` - The transform containing the variation parameters
/// * `var_name` - Name of the variation (e.g., "julian", "blob")
/// * `parameters` - List of parameters for this variation
/// * `flame_changed` - Flag to set when parameters are modified
///
/// # Example
/// ```rust,ignore
/// if value.abs() > 1e-6 && !var_info.parameters.is_empty() {
///     ui.indent(format!("params_{}", var_info.name), |ui| {
///         render_variation_params(ui, transform, &var_info.name, &var_info.parameters, flame_changed);
///     });
/// }
/// ```
pub fn render_variation_params(
    ui: &mut egui::Ui,
    transform: &mut Transform,
    var_name: &str,
    parameters: &[VariationParameter],
    flame_changed: &mut bool,
) {
    for param in parameters {
        let mut param_value = transform.get_variation_param_or_default(
            var_name,
            &param.name,
            &crate::variations::global_registry(),
        );

        let param_changed = match param.param_type {
            ParamType::Float => render_float_param(ui, param, &mut param_value),
            ParamType::Integer => render_integer_param(ui, param, &mut param_value),
            ParamType::Angle => render_angle_param(ui, param, &mut param_value),
        };

        if param_changed {
            transform.set_variation_param(var_name, &param.name, param_value);
            *flame_changed = true;
        }
    }
}

/// Render a float parameter slider
fn render_float_param(ui: &mut egui::Ui, param: &VariationParameter, value: &mut f32) -> bool {
    let min = param.min_value.unwrap_or(-10.0);
    let max = param.max_value.unwrap_or(10.0);
    ui.add(
        egui::Slider::new(value, min..=max)
            .text(&param.display_name)
            .step_by(0.01),
    )
    .changed()
}

/// Render an integer parameter slider
fn render_integer_param(ui: &mut egui::Ui, param: &VariationParameter, value: &mut f32) -> bool {
    let min = param.min_value.unwrap_or(1.0) as i32;
    let max = param.max_value.unwrap_or(10.0) as i32;
    let mut int_val = *value as i32;
    let changed = ui
        .add(egui::Slider::new(&mut int_val, min..=max).text(&param.display_name))
        .changed();
    *value = int_val as f32;
    changed
}

/// Render an angle parameter slider (displays degrees, stores as value)
fn render_angle_param(ui: &mut egui::Ui, param: &VariationParameter, value: &mut f32) -> bool {
    let min = param.min_value.unwrap_or(0.0);
    let max = param.max_value.unwrap_or(360.0);
    ui.add(egui::Slider::new(value, min..=max).text(&param.display_name).suffix("°"))
        .changed()
}
