//! Palette Library panel - browse and manage palette packs

use egui;
use crate::scene::palette::{PaletteLibrary, Palette};

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
                        // Use grid for automatic alignment
                        egui::Grid::new(format!("palette_grid_{}", pack_idx))
                            .num_columns(2)
                            .spacing([10.0, 4.0])
                            .striped(false)
                            .show(ui, |ui| {
                                for (palette_idx, palette) in pack.palettes.iter().enumerate() {
                                    let preview_height = 20.0;
                                    let preview_width = 200.0;

                                    // Generate texture ID based on pack and palette index
                                    let texture_id = egui::Id::new(("palette_preview", pack_idx, palette_idx));

                                    // Load or get cached texture using egui's memory system
                                    let texture = ui.ctx().data_mut(|data| {
                                        data.get_temp::<egui::TextureHandle>(texture_id)
                                    }).unwrap_or_else(|| {
                                        // Generate preview image
                                        let preview_image = PaletteLibrary::generate_preview(
                                            palette,
                                            preview_width as usize,
                                            preview_height as usize,
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

                                    // Column 1: Palette name (clickable)
                                    let label_response = ui.add(
                                        egui::Label::new(&palette.name)
                                            .sense(egui::Sense::click())
                                    );

                                    if label_response.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }

                                    if label_response.clicked() {
                                        selected_palette = Some(palette.clone());
                                    }

                                    // Column 2: Palette preview (clickable)
                                    let image = egui::Image::new(&texture)
                                        .fit_to_exact_size(egui::vec2(preview_width, preview_height));

                                    let image_response = ui.add(image.sense(egui::Sense::click()));

                                    if image_response.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }

                                    if image_response.clicked() {
                                        selected_palette = Some(palette.clone());
                                    }

                                    // Highlight entire row on hover
                                    if label_response.hovered() || image_response.hovered() {
                                        let row_rect = label_response.rect.union(image_response.rect);
                                        ui.painter().rect_filled(
                                            row_rect.expand(2.0),
                                            2.0,
                                            ui.visuals().widgets.hovered.bg_fill,
                                        );
                                    }

                                    ui.end_row();
                                }
                            });
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
