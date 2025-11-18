//! Palette Library panel - browse and manage palette packs

use egui;
use crate::scene::palette::{PaletteLibrary, Palette};
use std::collections::HashMap;

/// Render the Palette Library panel
/// Returns Some(palette) if user selected a new palette (cloned)
pub fn render_palette_library(
    ui: &mut egui::Ui,
    library: &mut PaletteLibrary,
) -> Option<Palette> {
    let mut selected_palette: Option<Palette> = None;

    // Search box (future feature)
    ui.horizontal(|ui| {
        ui.label("Search:");
        ui.add_enabled(false, egui::TextEdit::singleline(&mut String::new()).hint_text("Coming soon..."));
    });

    ui.separator();

    // Scrollable area for pack list
    egui::ScrollArea::vertical().show(ui, |ui| {
        // Iterate through all packs
        for pack_idx in 0..library.pack_count() {
            // Get pack info before borrowing mutably
            let (pack_name, pack_description, palette_count, is_enabled) = {
                if let Some(pack) = library.get_pack(pack_idx) {
                    (pack.pack_name.clone(), pack.description.clone(), pack.palettes.len(), library.is_pack_enabled(pack_idx))
                } else {
                    continue;
                }
            };

            // Pack header with checkbox
            ui.horizontal(|ui| {
                let mut enabled = is_enabled;
                if ui.checkbox(&mut enabled, "").changed() {
                    library.set_pack_enabled(pack_idx, enabled);
                }

                ui.strong(&pack_name);
                ui.label(format!("({} palettes)", palette_count));
            });

            // Show palettes if pack is enabled
            if is_enabled {
                if let Some(pack) = library.get_pack(pack_idx) {
                    ui.indent(format!("pack_{}", pack_idx), |ui| {
                        for (palette_idx, palette) in pack.palettes.iter().enumerate() {
                            // Clickable palette entry
                            ui.vertical(|ui| {
                                // Generate preview on-demand
                                let preview_height = 10;
                                let available_width = ui.available_width();
                                let preview_width = (available_width - 20.0).max(100.0) as usize;

                                // Generate texture ID based on pack and palette index
                                let texture_id = egui::Id::new(("palette_preview", pack_idx, palette_idx));

                                // Load or get cached texture using egui's memory system
                                let texture = ui.ctx().data_mut(|data| {
                                    data.get_temp::<egui::TextureHandle>(texture_id)
                                }).unwrap_or_else(|| {
                                    // Generate preview image
                                    let preview_image = PaletteLibrary::generate_preview(
                                        palette,
                                        preview_width,
                                        preview_height,
                                    );

                                    // Load and cache texture
                                    let tex = ui.ctx().load_texture(
                                        format!("palette_{}_{}", pack_idx, palette_idx),
                                        preview_image,
                                        egui::TextureOptions::LINEAR,
                                    );

                                    // Store in egui memory
                                    ui.ctx().data_mut(|data| {
                                        data.insert_temp(texture_id, tex.clone());
                                    });

                                    tex
                                });

                                // Preview image
                                let image = egui::Image::new(&texture)
                                    .fit_to_exact_size(egui::vec2(preview_width as f32, preview_height as f32));

                                if ui.add(image.sense(egui::Sense::click())).clicked() {
                                    selected_palette = Some(palette.clone());
                                }

                                // Palette name
                                ui.label(&palette.name);
                            });

                            ui.add_space(4.0);
                        }
                    });
                }
            }

            ui.separator();
        }

        // If no packs loaded, show message
        if library.pack_count() == 0 {
            ui.label("No palette packs loaded.");
            ui.label("Place .json files in assets/palettes/packs/");
        }
    });

    selected_palette
}
