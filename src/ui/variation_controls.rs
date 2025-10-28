//! Variation category rendering
//!
//! This module provides a unified interface for rendering variation controls
//! organized by category, eliminating duplication across Basic 2D, Advanced 2D,
//! 3D Depth, 3D Rotation, and Full 3D variations.

use crate::{
    scene::transforms::Transform,
    variations::VariationCategory,
};
use super::variation_params::render_variation_params;

/// Render all variations in a specific category
///
/// This function handles:
/// - Section header with category label
/// - Variation weight sliders (0.0-2.0 range)
/// - Automatic parameter display when variation is active (weight > 1e-6)
/// - Indented parameter controls with appropriate labeling
///
/// # Arguments
/// * `ui` - The egui UI context
/// * `transform` - The transform to modify
/// * `category` - Which category of variations to render
/// * `category_label` - Display label for the category (e.g., "Basic 2D Variations")
/// * `flame_changed` - Flag to set when variations are modified
///
/// # Example
/// ```rust,ignore
/// render_variation_category(
///     ui,
///     transform,
///     VariationCategory::Basic2D,
///     "Basic 2D Variations",
///     flame_changed
/// );
/// ```
pub fn render_variation_category(
    ui: &mut egui::Ui,
    transform: &mut Transform,
    category: VariationCategory,
    category_label: &str,
    flame_changed: &mut bool,
) {
    ui.separator();
    ui.label(category_label);

    let variations = crate::variations::global_registry().by_category(category);

    for var_info in variations {
        let mut value = transform.get_variation(&var_info.name);

        if ui
            .add(egui::Slider::new(&mut value, 0.0..=2.0).text(&var_info.display_name))
            .changed()
        {
            transform.set_variation(&var_info.name, value);
            *flame_changed = true;
        }

        // Show parameters if variation is active and has parameters
        if value.abs() > 1e-6 && !var_info.parameters.is_empty() {
            ui.indent(format!("params_{}", var_info.name), |ui| {
                render_variation_params(ui, transform, &var_info.name, &var_info.parameters, flame_changed);
            });
        }
    }
}
