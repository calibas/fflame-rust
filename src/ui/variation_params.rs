//! Variation parameter rendering
//!
//! This module provides unified rendering for variation parameters,
//! eliminating the need to duplicate parameter UI code across different
//! variation categories.

use crate::{
    config::{ConfigManager, TransformRef, UpdateType},
    variations::{ParamType, VariationParameter},
};

/// Render parameter controls for an active variation on any pool member.
///
/// `xref` selects which pool (Normal / Linked / Final) and index the
/// transform lives in; the right ConfigPath variant is built via
/// `TransformRef::variation_param_path`.
pub fn render_variation_params(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    xref: TransformRef,
    var_name: &str,
    parameters: &[VariationParameter],
) -> UpdateType {
    let mut max_update = UpdateType::None;

    for param in parameters {
        // Get current value from active config (for live preview).
        let transform = match xref.get(&config_manager.active_config().flame) {
            Some(t) => t,
            None => return max_update, // Pool member missing — bail out.
        };
        let mut param_value = transform.get_variation_param_or_default(
            var_name,
            &param.name,
            &crate::variations::global_registry(),
        );

        let (param_changed, drag_stopped) = match &param.param_type {
            ParamType::Float => render_float_param(ui, param, &mut param_value),
            ParamType::UnlimitedFloat => render_unlimited_float_param(ui, param, &mut param_value),
            ParamType::Integer => render_integer_param(ui, param, &mut param_value),
            ParamType::UnlimitedInteger => render_unlimited_integer_param(ui, param, &mut param_value),
            ParamType::Boolean => render_boolean_param(ui, param, &mut param_value),
            ParamType::Angle => render_angle_param(ui, param, &mut param_value),
            ParamType::Enum { choices } => render_enum_param(ui, param, &mut param_value, choices),
        };

        let path = xref.variation_param_path(var_name.to_string(), param.name.clone());

        if param_changed {
            if let Ok(update_type) = config_manager.update_param(path.clone(), param_value.into()) {
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
fn render_float_param(ui: &mut egui::Ui, param: &VariationParameter, value: &mut f32) -> (bool, bool) {
    let min = param.min_value.unwrap_or(-10.0);
    let max = param.max_value.unwrap_or(10.0);
    let response = ui.add(
        super::VkbSlider::new(value, min..=max)
            .text(&param.display_name)
            .step_by(0.01),
    );
    (response.changed(), response.drag_stopped())
}

/// Render an integer parameter slider
/// Returns (changed, dragged, drag_stopped)
fn render_integer_param(ui: &mut egui::Ui, param: &VariationParameter, value: &mut f32) -> (bool, bool) {
    let min = param.min_value.unwrap_or(1.0) as i32;
    let max = param.max_value.unwrap_or(10.0) as i32;
    let mut int_val = *value as i32;
    let response = ui.add(super::VkbSlider::new(&mut int_val, min..=max).text(&param.display_name));
    *value = int_val as f32;
    (response.changed(), response.drag_stopped())
}

/// Render an unlimited integer parameter (full i32 range)
/// Uses min/max as slider range (default -100 to 100), but allows typing any i32 value
/// Returns (changed, dragged, drag_stopped)
fn render_unlimited_integer_param(ui: &mut egui::Ui, param: &VariationParameter, value: &mut f32) -> (bool, bool) {
    let min = param.min_value.unwrap_or(-100.0) as i32;
    let max = param.max_value.unwrap_or(100.0) as i32;
    let mut int_val = *value as i32;

    let response = ui.add(
        super::VkbSlider::new(&mut int_val, min..=max)
            .text(&param.display_name)
            .clamping(egui::SliderClamping::Never)  // Allow typing values outside slider range
    );

    if response.changed() {
        // Clamp to i32 limits (actual limits: -2,147,483,648 to 2,147,483,647)
        *value = (int_val.clamp(i32::MIN, i32::MAX)) as f32;
    } else {
        *value = int_val as f32;
    }

    (response.changed(), response.drag_stopped())
}

/// Render an angle parameter slider (displays degrees, stores as value)
/// Returns (changed, dragged, drag_stopped)
fn render_angle_param(ui: &mut egui::Ui, param: &VariationParameter, value: &mut f32) -> (bool, bool) {
    let min = param.min_value.unwrap_or(0.0);
    let max = param.max_value.unwrap_or(360.0);
    let response = ui.add(super::VkbSlider::new(value, min..=max).text(&param.display_name).suffix("°"));
    (response.changed(), response.drag_stopped())
}

/// Render an unlimited float parameter (full f32 range)
/// Uses min/max as slider range (default -10.0 to 10.0), but allows typing any f32 value
/// Returns (changed, dragged, drag_stopped)
fn render_unlimited_float_param(ui: &mut egui::Ui, param: &VariationParameter, value: &mut f32) -> (bool, bool) {
    let min = param.min_value.unwrap_or(-10.0);
    let max = param.max_value.unwrap_or(10.0);

    let response = ui.add(
        super::VkbSlider::new(value, min..=max)
            .text(&param.display_name)
            .clamping(egui::SliderClamping::Never)  // Allow typing values outside slider range
    );

    if response.changed() {
        // Clamp to f32 limits (actual limits: -3.4E38 to 3.4E38)
        *value = value.clamp(f32::MIN, f32::MAX);
    }

    (response.changed(), response.drag_stopped())
}

/// Render a boolean parameter (checkbox)
/// Returns (changed, dragged=false, drag_stopped=false)
fn render_boolean_param(ui: &mut egui::Ui, param: &VariationParameter, value: &mut f32) -> (bool, bool) {
    let mut checked = *value > 0.5;
    let response = ui.checkbox(&mut checked, &param.display_name);
    *value = if checked { 1.0 } else { 0.0 };
    (response.changed(), false)  // Checkboxes don't have drag states
}

/// Render an enum parameter (dropdown/combobox)
/// Returns (changed, dragged=false, drag_stopped=false)
fn render_enum_param(ui: &mut egui::Ui, param: &VariationParameter, value: &mut f32, choices: &[String]) -> (bool, bool) {
    if choices.is_empty() {
        // Fallback if no choices provided
        ui.label(format!("{}: (no choices)", param.display_name));
        return (false, false);
    }

    let current_idx = (*value as usize).min(choices.len().saturating_sub(1));
    let mut selected = current_idx;

    let mut changed = false;
    egui::ComboBox::from_label(&param.display_name)
        .selected_text(&choices[selected])
        .show_ui(ui, |ui| {
            for (idx, choice) in choices.iter().enumerate() {
                if ui.selectable_value(&mut selected, idx, choice).clicked() {
                    changed = true;
                }
            }
        });

    if changed {
        *value = selected as f32;
    }

    (changed, false)  // Dropdowns don't have drag states
}

