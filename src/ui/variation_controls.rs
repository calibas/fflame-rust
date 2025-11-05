//! Variation category rendering
//!
//! This module provides a unified interface for rendering variation controls
//! organized by category, eliminating duplication across Basic 2D, Advanced 2D,
//! 3D Depth, 3D Rotation, and Full 3D variations.

use crate::{
    config::{ConfigManager, ConfigPath, UpdateType},
    variations::VariationCategory,
};
use super::variation_params::render_variation_params;

/// Render all variations in a specific category
///
/// This function handles:
/// - Section header with category label
/// - Variation weight sliders (-10.0 to 10.0 range) with lazy undo support
/// - Automatic parameter display when variation is active (weight > 1e-6)
/// - Indented parameter controls with lazy undo support
///
/// # Arguments
/// * `ui` - The egui UI context
/// * `config_manager` - The configuration manager
/// * `transform_index` - Index of the transform being edited
/// * `category` - Which category of variations to render
/// * `category_label` - Display label for the category (e.g., "Basic 2D Variations")
///
/// # Returns
/// The highest UpdateType from all variation changes
///
/// # Example
/// ```rust,ignore
/// let update = render_variation_category(
///     ui,
///     config_manager,
///     i,
///     VariationCategory::Basic2D,
///     "Basic 2D Variations"
/// );
/// max_update = max_update.max(update);
/// ```
pub fn render_variation_category(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    transform_index: usize,
    category: VariationCategory,
    category_label: &str,
) -> UpdateType {
    ui.separator();
    ui.label(category_label);

    let mut max_update = UpdateType::None;
    let variations = crate::variations::global_registry().by_category(category);

    for var_info in variations {
        // Get current value from active config (for live preview)
        let transform = &config_manager.active_config().flame.transforms[transform_index];
        let mut value = transform.get_variation(&var_info.name);

        // Apophysis allows any float value including negative weights
        // Slider range: -10.0 to 10.0 (covers 99% of use cases)
        // Users can type values beyond this range (clamped to f32 limits)
        let response = ui.add(
            egui::Slider::new(&mut value, -10.0..=10.0)
                .text(&var_info.display_name)
                .clamp_to_range(false)  // Allow typing values outside slider range
        );

        if response.changed() {
            // Clamp to f32 limits (actual limits: -3.4E38 to 3.4E38)
            value = value.clamp(f32::MIN, f32::MAX);

            // Update via ConfigManager with lazy undo only during drag
            let path = ConfigPath::TransformVariation {
                index: transform_index,
                variation: var_info.name.clone(),
            };

            if let Ok(update_type) = config_manager.update_param(path, value.into(), response.dragged()) {
                max_update = max_update.max(update_type);
            }
        }

        if response.drag_stopped() {
            let path = ConfigPath::TransformVariation {
                index: transform_index,
                variation: var_info.name.clone(),
            };
            let _ = config_manager.force_commit_preview(&path);
        }

        // Show parameters if variation is active and has parameters
        if value.abs() > 1e-6 && !var_info.parameters.is_empty() {
            ui.indent(format!("params_{}", var_info.name), |ui| {
                let param_update = render_variation_params(
                    ui,
                    config_manager,
                    transform_index,
                    &var_info.name,
                    &var_info.parameters,
                );
                max_update = max_update.max(param_update);
            });
        }
    }

    max_update
}
