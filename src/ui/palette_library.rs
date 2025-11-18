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
                        // Calculate max name width for this pack
                        let max_name_width = pack.palettes.iter()
                            .map(|p| p.name.len() as f32 * 8.0) // Rough estimate: 8px per character
                            .fold(0.0f32, f32::max)
                            .max(100.0) // Minimum
                            .min(200.0); // Maximum to prevent excessive width

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

                                    // Allocate space for the row first to get rect for background
                                    // Use calculated max width for first column (all rows will align)
                                    let (name_rect, name_response) = ui.allocate_exact_size(
                                        egui::vec2(max_name_width, preview_height),
                                        egui::Sense::click()
                                    );

                                    // Move to next column
                                    let (img_rect, img_response) = ui.allocate_exact_size(
                                        egui::vec2(preview_width, preview_height),
                                        egui::Sense::click()
                                    );

                                    // Draw highlight FIRST (behind everything)
                                    if name_response.hovered() || img_response.hovered() {
                                        let row_rect = name_rect.union(img_rect);
                                        ui.painter().rect_filled(
                                            row_rect.expand(2.0),
                                            2.0,
                                            ui.visuals().widgets.hovered.bg_fill,
                                        );
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }

                                    // Draw text label on top of background
                                    ui.painter().text(
                                        name_rect.left_center() + egui::vec2(5.0, 0.0),
                                        egui::Align2::LEFT_CENTER,
                                        &palette.name,
                                        egui::FontId::default(),
                                        ui.visuals().text_color(),
                                    );

                                    // Draw image on top of background
                                    let image = egui::Image::new(&texture)
                                        .fit_to_exact_size(egui::vec2(preview_width, preview_height));

                                    ui.put(img_rect, image);

                                    // Handle clicks on either element
                                    if name_response.clicked() || img_response.clicked() {
                                        let mut palette_copy = palette.clone();

                                        // Always create a custom copy with unique name
                                        // (Pack palettes have built_in=true, user palettes have built_in=false)
                                        let base_name = &palette.name;
                                        let mut new_name = format!("{} (Custom)", base_name);
                                        let mut counter = 2;

                                        // Ensure unique name
                                        while library.palettes().iter().any(|p| p.name == new_name) {
                                            new_name = format!("{} (Custom {})", base_name, counter);
                                            counter += 1;
                                        }

                                        palette_copy.name = new_name;
                                        palette_copy.built_in = false;
                                        selected_palette = Some(palette_copy);
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
