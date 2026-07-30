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
        // Get current value from the active editing target (for live
        // preview). Routes through `active_flame()` — when editing a
        // subflame, that's the subflame's slot; otherwise it's the
        // main flame.
        let transform = match xref.get(config_manager.active_flame()) {
            Some(t) => t,
            None => return max_update, // Pool member missing — bail out.
        };
        let stored_value = transform.get_variation_param_or_default(
            var_name,
            &param.name,
            &crate::variations::global_registry(),
        );
        let mut param_value = stored_value;

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

        // Writing only on `changed()` isn't enough on its own: a widget that
        // quantizes or clamps the value it was handed reports `changed()` on
        // the frame it's first drawn, so merely EXPANDING a variation's
        // parameter section would rewrite the flame (a step of 0.01 rounded
        // Thickness 0.006 → 0.01, doubling the rendered line width). Require
        // an actual difference so opening a panel can never edit a flame.
        if param_changed && param_value != stored_value {
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

/// Attach a hover tooltip to a widget response when the param has a
/// description. No-op when description is None (no tooltip rendered).
fn with_hover(response: egui::Response, param: &VariationParameter) -> egui::Response {
    if let Some(desc) = &param.description {
        response.on_hover_text(desc)
    } else {
        response
    }
}

/// Render a float parameter slider
/// Returns (changed, dragged, drag_stopped)
fn render_float_param(ui: &mut egui::Ui, param: &VariationParameter, value: &mut f32) -> (bool, bool) {
    let min = param.min_value.unwrap_or(-10.0);
    let max = param.max_value.unwrap_or(10.0);
    let response = with_hover(ui.add(
        super::VkbSlider::new(value, min..=max)
            .text(&param.display_name)
            // No step_by: a fixed step rounds the STORED value (0.006 →
            // 0.01 just by drawing the panel) and disables VkbSlider's own
            // drag snapping, which derives a nice_step from the range —
            // 0.001 for a 0..0.2 param — and applies it only while
            // dragging, leaving typed values exact.
            .clamping(egui::SliderClamping::Edits),
    ), param);
    (response.changed(), response.drag_stopped())
}

/// Render an integer parameter slider
/// Returns (changed, dragged, drag_stopped)
fn render_integer_param(ui: &mut egui::Ui, param: &VariationParameter, value: &mut f32) -> (bool, bool) {
    let min = param.min_value.unwrap_or(1.0) as i32;
    let max = param.max_value.unwrap_or(10.0) as i32;
    let mut int_val = *value as i32;
    let response = with_hover(
        ui.add(
            super::VkbSlider::new(&mut int_val, min..=max)
                .text(&param.display_name)
                .clamping(egui::SliderClamping::Edits),
        ),
        param,
    );
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

    let response = with_hover(ui.add(
        super::VkbSlider::new(&mut int_val, min..=max)
            .text(&param.display_name)
            .clamping(egui::SliderClamping::Never)  // Allow typing values outside slider range
    ), param);

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
    let response = with_hover(
        ui.add(
            super::VkbSlider::new(value, min..=max)
                .text(&param.display_name)
                .suffix("°")
                .clamping(egui::SliderClamping::Edits),
        ),
        param,
    );
    (response.changed(), response.drag_stopped())
}

/// Render an unlimited float parameter (full f32 range)
/// Uses min/max as slider range (default -10.0 to 10.0), but allows typing any f32 value
/// Returns (changed, dragged, drag_stopped)
fn render_unlimited_float_param(ui: &mut egui::Ui, param: &VariationParameter, value: &mut f32) -> (bool, bool) {
    let min = param.min_value.unwrap_or(-10.0);
    let max = param.max_value.unwrap_or(10.0);

    let response = with_hover(ui.add(
        super::VkbSlider::new(value, min..=max)
            .text(&param.display_name)
            .clamping(egui::SliderClamping::Never)  // Allow typing values outside slider range
    ), param);

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
    let response = with_hover(ui.checkbox(&mut checked, &param.display_name), param);
    *value = if checked { 1.0 } else { 0.0 };
    (response.changed(), false)  // Checkboxes don't have drag states
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{ConfigManager, FractalConfig, TransformRef};

    /// Store `thickness` on a flame, draw the parameter section once, and
    /// report what's left in the config afterwards.
    fn stored_after_drawing(thickness: f32) -> f32 {
        let mut config = FractalConfig::default();
        if config.flame.transforms.is_empty() {
            config.flame.transforms.push(Default::default());
        }
        let xform = &mut config.flame.transforms[0];
        xform.variations.insert("lsystem_path_3D".to_string(), 1.0);
        xform
            .variation_params
            .insert("lsystem_path_3D.thickness".to_string(), thickness);

        let mut manager = ConfigManager::new(config);
        let registry = crate::variations::global_registry();
        let params = registry.get("lsystem_path_3D").unwrap().parameters.clone();

        egui::__run_test_ui(|ui| {
            render_variation_params(
                ui,
                &mut manager,
                TransformRef::Normal(0),
                "lsystem_path_3D",
                &params,
            );
        });

        manager.active_config().flame.transforms[0].variation_params
            ["lsystem_path_3D.thickness"]
    }

    /// Drawing a parameter section must never edit the flame.
    ///
    /// Regression: the Float slider carried `step_by(0.01)`, which rounds
    /// the value it is handed, so merely expanding "L-System Path 3D"
    /// rewrote Thickness 0.006 → 0.01 and doubled the rendered line width.
    /// The export path, which never draws the panel, kept 0.006 — which is
    /// what made the two disagree.
    #[test]
    fn drawing_params_keeps_values_finer_than_a_slider_step() {
        assert_eq!(stored_after_drawing(0.006), 0.006);
    }

    /// The UI min/max are a suggested editing range, not a constraint on
    /// what a flame may hold — scripts and `.flame` imports both write
    /// values outside it. egui's default `SliderClamping::Always` clamps
    /// "even existing ones", so displaying such a param used to silently
    /// rewrite it; `Edits` restricts what the user can enter instead.
    #[test]
    fn drawing_params_keeps_values_outside_the_slider_range() {
        // Thickness is declared 0.0..=0.2.
        assert_eq!(stored_after_drawing(0.5), 0.5);
    }
}

/// Render an enum parameter (dropdown/combobox)
/// Returns (changed, dragged=false, drag_stopped=false)
fn render_enum_param(ui: &mut egui::Ui, param: &VariationParameter, value: &mut f32, choices: &[&'static str]) -> (bool, bool) {
    if choices.is_empty() {
        // Fallback if no choices provided
        ui.label(format!("{}: (no choices)", param.display_name));
        return (false, false);
    }

    let current_idx = (*value as usize).min(choices.len().saturating_sub(1));
    let mut selected = current_idx;

    let mut changed = false;
    let combo = egui::ComboBox::from_label(&param.display_name)
        .selected_text(choices[selected])
        .show_ui(ui, |ui| {
            for (idx, choice) in choices.iter().enumerate() {
                if ui.selectable_value(&mut selected, idx, *choice).clicked() {
                    changed = true;
                }
            }
        });
    // Attach hover tooltip to the closed combobox response.
    let _ = with_hover(combo.response, param);

    if changed {
        *value = selected as f32;
    }

    (changed, false)  // Dropdowns don't have drag states
}

