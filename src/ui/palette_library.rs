//! Palette Library panel - browse and manage palette packs

use egui;
use crate::scene::palette::PaletteLibrary;
use crate::config::{ConfigManager, ConfigPath};
use crate::resources::LoadState;
use rust_i18n::t;

/// Spawn async pack fetch (WASM only)
#[cfg(target_arch = "wasm32")]
fn spawn_pack_fetch(ctx: egui::Context, pack_idx: usize, file_path: String) {
    use crate::scene::palette::global_palette_library;
    use crate::resources::palettes::load_pack;

    wasm_bindgen_futures::spawn_local(async move {
        match load_pack(&file_path).await {
            Ok(pack) => {
                if let Ok(mut library) = global_palette_library().write() {
                    library.set_pack_loaded(pack_idx, pack);
                }
            }
            Err(e) => {
                if let Ok(mut library) = global_palette_library().write() {
                    library.set_pack_failed(pack_idx, e.to_string());
                }
            }
        }
        // Request repaint to update UI
        ctx.request_repaint();
    });
}

/// Render the Palette Library panel
/// When a palette is selected, it's set directly via ConfigManager
pub fn render_palette_library(
    ui: &mut egui::Ui,
    library: &mut PaletteLibrary,
    config_manager: &mut ConfigManager,
    open_palette_editor: &mut bool,
) {
    // WASM: Sync from global singleton (async fetches update global, not local)
    #[cfg(target_arch = "wasm32")]
    library.sync_from_global();

    // WASM: Auto-load enabled packs that haven't been loaded yet (e.g., on startup)
    #[cfg(target_arch = "wasm32")]
    {
        for pack_idx in 0..library.pack_count() {
            let needs_auto_load = library.get_pack_info(pack_idx)
                .map(|info| info.enabled && !info.is_loaded() && !info.is_loading() && info.metadata.is_some())
                .unwrap_or(false);

            if needs_auto_load {
                if let Some(file_path) = library.start_pack_load(pack_idx) {
                    spawn_pack_fetch(ui.ctx().clone(), pack_idx, file_path);
                }
            }
        }
    }

    // Quick access to palette editor
    if ui.button(t!("palette_library.open_editor")).clicked() {
        *open_palette_editor = true;
    }
    ui.separator();

    // Scrollable area for pack list
    egui::ScrollArea::vertical().show(ui, |ui| {
        // Iterate through all packs
        for pack_idx in 0..library.pack_count() {
            // Get pack info (includes load state)
            let (pack_name, palette_count, is_enabled, load_state) = {
                if let Some(info) = library.get_pack_info(pack_idx) {
                    let name = info.name().to_string();
                    let count = info.palette_count();
                    let enabled = info.enabled;
                    let state = info.load_state.clone();
                    (name, count, enabled, state)
                } else {
                    continue;
                }
            };

            // Pack header with checkbox and collapsing control
            let header_id = ui.make_persistent_id(format!("pack_header_{}", pack_idx));

            // Checkbox for enabling/disabling pack
            let mut enabled = is_enabled;
            ui.horizontal(|ui| {
                if ui.checkbox(&mut enabled, "")
                    .on_hover_text(t!("palette_library.tooltip_enable_pack"))
                    .changed()
                {
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        library.set_pack_enabled(pack_idx, enabled);
                    }
                    #[cfg(target_arch = "wasm32")]
                    {
                        if let Some(file_path) = library.set_pack_enabled(pack_idx, enabled) {
                            spawn_pack_fetch(ui.ctx().clone(), pack_idx, file_path);
                        }
                    }
                }
            });

            // Collapsing header for expand/collapse
            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                header_id,
                true // Default to open
            )
            .show_header(ui, |ui| {
                ui.strong(&pack_name);

                // Show load state indicator
                match &load_state {
                    LoadState::NotLoaded => {
                        ui.weak(format!("({} palettes)", palette_count));
                    }
                    LoadState::Loading => {
                        ui.spinner();
                        ui.weak(t!("palette_library.loading"));
                    }
                    LoadState::Loaded => {
                        ui.label(t!("palette_library.palettes_count", count = palette_count));
                    }
                    LoadState::Failed(error) => {
                        ui.colored_label(egui::Color32::RED, "⚠");
                        ui.weak(error);
                    }
                }
            })
            .body(|ui| {
                // Handle different load states
                match &load_state {
                    LoadState::NotLoaded => {
                        // Nothing to show - pack will load when enabled
                    }
                    LoadState::Loading => {
                        ui.horizontal(|ui| {
                            ui.spinner();
                            ui.label(t!("palette_library.loading"));
                        });
                    }
                    LoadState::Failed(error) => {
                        ui.colored_label(egui::Color32::RED, error);
                        if ui.button(t!("palette_library.retry"))
                            .on_hover_text(t!("palette_library.tooltip_retry"))
                            .clicked()
                        {
                            // Reset state and retry by toggling enabled
                            if let Some(info) = library.packs_mut().get_mut(pack_idx) {
                                info.load_state = LoadState::NotLoaded;
                            }
                            #[cfg(not(target_arch = "wasm32"))]
                            {
                                library.set_pack_enabled(pack_idx, true);
                            }
                            #[cfg(target_arch = "wasm32")]
                            {
                                if let Some(file_path) = library.set_pack_enabled(pack_idx, true) {
                                    spawn_pack_fetch(ui.ctx().clone(), pack_idx, file_path);
                                }
                            }
                        }
                    }
                    LoadState::Loaded => {
                        // Show palettes only if pack is enabled and loaded
                        if is_enabled {
                            if let Some(pack) = library.get_pack(pack_idx) {
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

                                            // Check if this is the Custom pack
                                            let is_custom_pack = library.custom_pack_index() == Some(pack_idx);

                                            // Generate texture ID based on pack and palette index
                                            // For Custom pack: include generation counter to invalidate cache on save/delete
                                            let texture_id = if is_custom_pack {
                                                egui::Id::new(("palette_preview", pack_idx, palette_idx, library.generation()))
                                            } else {
                                                egui::Id::new(("palette_preview", pack_idx, palette_idx))
                                            };

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
                                                // Set palette directly - create an editable copy
                                                let mut palette_copy = palette.clone();
                                                palette_copy.built_in = false;

                                                let _ = config_manager.update_param(
                                                    ConfigPath::Palette,
                                                    palette_copy.into()
                                                );
                                            }

                                            ui.end_row();
                                        }
                                    });
                            }
                        }
                    }
                }
            });

            ui.separator();
        }

        // If no packs loaded, show message
        if library.pack_count() == 0 {
            ui.label(t!("palette_library.no_packs"));
            ui.label(t!("palette_library.no_packs_hint"));
        }
    });
}
