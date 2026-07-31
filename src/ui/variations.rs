//! Variations panel — lists every variation in the global registry,
//! grouped by category. Shows source label (built-in / API v#) and provides
//! a "Clear Variation Cache" action.

use egui;
use rust_i18n::t;

use crate::variations::{global_registry, VariationCategory};

/// Render the Variations panel.
/// Returns true if the user clicked "Clear Cache" and confirmed.
pub fn render_variations_panel(ui: &mut egui::Ui) -> VariationsPanelResponse {
    let mut response = VariationsPanelResponse::default();

    let registry = global_registry();
    let total = registry.all().iter().count();
    let api_count = registry.all().iter().filter(|v| !v.is_core).count();

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(t!("variations_panel.total", count = total)).strong());
    });
    ui.add_space(4.0);

    {
        let categories = [
            (VariationCategory::Basic2D, "variations_panel.category_basic_2d"),
            (VariationCategory::Advanced2D, "variations_panel.category_advanced_2d"),
            (VariationCategory::Depth3D, "variations_panel.category_depth_3d"),
            (VariationCategory::Rotation3D, "variations_panel.category_rotation_3d"),
            (VariationCategory::Full3D, "variations_panel.category_full_3d"),
            (VariationCategory::Only3D, "variations_panel.category_only_3d"),
            (VariationCategory::Plugin, "variations_panel.category_plugin"),
        ];

        for (cat, label_key) in categories {
            let in_cat: Vec<_> = registry.by_category(cat);
            if in_cat.is_empty() {
                continue;
            }
            egui::CollapsingHeader::new(format!("{} ({})", t!(label_key), in_cat.len()))
                .default_open(false)
                .show(ui, |ui| {
                    for v in in_cat {
                        ui.horizontal(|ui| {
                            ui.label(&v.display_name);
                            ui.weak(format!("({})", v.name));
                            // A variation with no 3D body is DROPPED from a
                            // 3D flame — it keeps its weight and contributes
                            // nothing, which is invisible in the render.
                            // Marking it here is the cheap half of the fix;
                            // the build also logs when it actually happens.
                            if v.wgsl_source_3d.is_none() {
                                ui.weak("· 2D only")
                                    .on_hover_text(t!("variations_panel.two_d_only_hint"));
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let source = if v.is_core {
                                        t!("variations_panel.source_built_in").to_string()
                                    } else {
                                        format!("{} v{}", t!("variations_panel.source_api"), v.version)
                                    };
                                    ui.weak(source);
                                },
                            );
                        });
                        if !v.parameters.is_empty() {
                            ui.indent(format!("params_{}", v.name), |ui| {
                                for p in &v.parameters {
                                    ui.weak(format!(
                                        "  {} = {}",
                                        p.display_name, p.default_value
                                    ));
                                }
                            });
                        }
                    }
                });
        }
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        let label = t!("variations_panel.clear_cache_btn", count = api_count);
        if ui.add_enabled(api_count > 0, egui::Button::new(label.as_ref())).clicked() {
            response.clear_cache_requested = true;
        }
    });

    response
}

#[derive(Default)]
pub struct VariationsPanelResponse {
    pub clear_cache_requested: bool,
}
