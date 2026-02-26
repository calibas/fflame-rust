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

    // Note: ScrollArea is managed by the panel viewer (render_palette_library_panel)
    // so cloud palettes section can share the same scroll area.
    {
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
    }
}

// --- Cloud Palettes Section (API feature) ---

/// Fetch cloud palettes from the API
async fn fetch_cloud_palettes(base_url: &str, token: &str) -> Result<Vec<crate::api::types::PaletteResponse>, String> {
    let mut api = crate::api::ApiState::new(base_url);
    api.set_token(token);
    api.list_palettes(None, 1, 100)
        .await
        .map_err(|e| e.to_string())
}

/// Delete a cloud palette from the API
async fn delete_cloud_palette(base_url: &str, token: &str, palette_id: &str) -> Result<String, String> {
    let mut api = crate::api::ApiState::new(base_url);
    api.set_token(token);
    let id = palette_id.to_string();
    api.delete_palette(&id)
        .await
        .map(|_| id)
        .map_err(|e| e.to_string())
}

/// Read API base URL from WASM localStorage. Auth is handled via cookies.
/// Returns (base_url, token) where token is empty (kept for API compatibility with desktop).
#[cfg(target_arch = "wasm32")]
fn get_wasm_palette_credentials() -> Result<(String, String), String> {
    let window = web_sys::window().ok_or("No window")?;
    let storage = window
        .local_storage()
        .map_err(|_| "Failed to access localStorage")?
        .ok_or("No localStorage")?;
    let base_url = storage
        .get_item("fflame_api_base_url")
        .ok()
        .flatten()
        .unwrap_or_else(|| "http://localhost:3000".to_string());
    Ok((base_url, String::new()))
}

/// Render the Cloud Palettes section in the Palette Library panel.
/// `auth` is `Some((base_url, token))` when signed in, `None` otherwise.
pub fn render_cloud_palettes_section(
    ui: &mut egui::Ui,
    cloud_state: &mut super::CloudPaletteState,
    config_manager: &mut ConfigManager,
    auth: Option<(&str, &str)>,
) {
    // Poll async results
    poll_cloud_palette_results(ui, cloud_state);

    ui.separator();

    let header_id = ui.make_persistent_id("cloud_palettes_header");
    let mut collapsing = egui::collapsing_header::CollapsingState::load_with_default_open(
        ui.ctx(),
        header_id,
        false, // Default collapsed
    );

    collapsing
        .show_header(ui, |ui| {
            ui.strong(t!("palette_library.cloud_section"));

            if cloud_state.loading {
                ui.spinner();
            }
        })
        .body(|ui| {
            // Auto-fetch on first expand
            if !cloud_state.fetched && !cloud_state.loading {
                trigger_cloud_palette_fetch(cloud_state, auth);
            }

            // Toolbar: Refresh button
            ui.horizontal(|ui| {
                let refresh_enabled = !cloud_state.loading;
                if ui.add_enabled(refresh_enabled, egui::Button::new(t!("palette_library.cloud_refresh")))
                    .clicked()
                {
                    trigger_cloud_palette_fetch(cloud_state, auth);
                }
            });

            // Error display
            if let Some(ref error) = cloud_state.error {
                ui.colored_label(egui::Color32::RED, error);
            }

            // Loading state
            if cloud_state.loading && cloud_state.palettes.is_empty() {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(t!("palette_library.cloud_loading"));
                });
                return;
            }

            // Empty state
            if cloud_state.fetched && cloud_state.palettes.is_empty() {
                ui.weak(t!("palette_library.cloud_empty"));
                ui.weak(t!("palette_library.cloud_empty_hint"));
                return;
            }

            // Palette list with gradient previews
            let preview_height = 20.0;
            let preview_width = 200.0;
            let max_name_width = cloud_state.palettes.iter()
                .map(|p| {
                    let name = p.name.as_deref().unwrap_or("Unnamed");
                    name.len() as f32 * 8.0
                })
                .fold(0.0f32, f32::max)
                .max(100.0)
                .min(200.0);

            egui::Grid::new("cloud_palette_grid")
                .num_columns(3) // name, preview, delete button
                .spacing([10.0, 4.0])
                .striped(false)
                .show(ui, |ui| {
                    // Collect palette data first to avoid borrow conflicts
                    let palette_data: Vec<_> = cloud_state.palettes.iter().map(|p| {
                        let name = p.name.clone().unwrap_or_else(|| "Unnamed".to_string());
                        let id = p.id.clone();
                        let palette = crate::api::sync::palette_from_api(p);
                        (id, name, palette)
                    }).collect();

                    for (id, name, palette) in &palette_data {
                        // Generate cached texture for gradient preview
                        let texture_id = egui::Id::new(("cloud_palette_preview", id));
                        let texture = ui.ctx().data_mut(|data| {
                            data.get_temp::<egui::TextureHandle>(texture_id)
                        }).unwrap_or_else(|| {
                            let preview_image = PaletteLibrary::generate_preview(
                                palette,
                                preview_width as usize,
                                preview_height as usize,
                            );
                            let tex = ui.ctx().load_texture(
                                format!("cloud_palette_{}", id),
                                preview_image,
                                egui::TextureOptions::LINEAR,
                            );
                            ui.ctx().data_mut(|data| {
                                data.insert_temp(texture_id, tex.clone());
                            });
                            tex
                        });

                        // Name column (clickable)
                        let (name_rect, name_response) = ui.allocate_exact_size(
                            egui::vec2(max_name_width, preview_height),
                            egui::Sense::click(),
                        );

                        // Preview column (clickable)
                        let (img_rect, img_response) = ui.allocate_exact_size(
                            egui::vec2(preview_width, preview_height),
                            egui::Sense::click(),
                        );

                        // Delete button column
                        let delete_clicked = ui.add_enabled(
                            !cloud_state.deleting,
                            egui::Button::new("X")
                                .small()
                                .fill(egui::Color32::from_rgb(120, 30, 30)),
                        ).clicked();

                        // Hover highlight
                        if name_response.hovered() || img_response.hovered() {
                            let row_rect = name_rect.union(img_rect);
                            ui.painter().rect_filled(
                                row_rect.expand(2.0),
                                2.0,
                                ui.visuals().widgets.hovered.bg_fill,
                            );
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        // Draw name
                        ui.painter().text(
                            name_rect.left_center() + egui::vec2(5.0, 0.0),
                            egui::Align2::LEFT_CENTER,
                            name,
                            egui::FontId::default(),
                            ui.visuals().text_color(),
                        );

                        // Draw gradient preview
                        let image = egui::Image::new(&texture)
                            .fit_to_exact_size(egui::vec2(preview_width, preview_height));
                        ui.put(img_rect, image);

                        // Handle click to load palette
                        if name_response.clicked() || img_response.clicked() {
                            let mut palette_copy = palette.clone();
                            palette_copy.built_in = false;
                            let _ = config_manager.update_param(
                                ConfigPath::Palette,
                                palette_copy.into(),
                            );
                            cloud_state.notification = Some((
                                t!("palette_library.cloud_load_success", name = name).to_string(),
                                false,
                            ));
                        }

                        // Handle delete
                        if delete_clicked {
                            trigger_cloud_palette_delete(cloud_state, id, auth);
                        }

                        ui.end_row();
                    }
                });

            // Show notification
            if let Some((ref msg, is_error)) = cloud_state.notification {
                let color = if is_error { egui::Color32::RED } else { egui::Color32::GREEN };
                ui.colored_label(color, msg);
            }
        });
}

/// Trigger async fetch of cloud palettes.
/// `auth` is `Some((base_url, token))` from SystemSettings; on WASM falls back to localStorage.
fn trigger_cloud_palette_fetch(cloud_state: &mut super::CloudPaletteState, auth: Option<(&str, &str)>) {
    cloud_state.loading = true;
    cloud_state.error = None;
    cloud_state.notification = None;

    // Get credentials: prefer passed-in auth, fall back to WASM localStorage
    let credentials: Result<(String, String), String> = if let Some((base_url, token)) = auth {
        Ok((base_url.to_string(), token.to_string()))
    } else {
        #[cfg(target_arch = "wasm32")]
        { get_wasm_palette_credentials() }
        #[cfg(not(target_arch = "wasm32"))]
        { Err("Not signed in — click Sign In first".to_string()) }
    };

    let (base_url, token) = match credentials {
        Ok(creds) => creds,
        Err(e) => {
            cloud_state.loading = false;
            cloud_state.error = Some(e);
            cloud_state.fetched = true;
            return;
        }
    };

    let result_slot = cloud_state.list_result.clone();

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async move {
        let result = fetch_cloud_palettes(&base_url, &token).await;
        if let Ok(mut slot) = result_slot.lock() {
            *slot = Some(result);
        }
    });

    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || {
        let result = pollster::block_on(fetch_cloud_palettes(&base_url, &token));
        if let Ok(mut slot) = result_slot.lock() {
            *slot = Some(result);
        }
    });
}

/// Trigger async delete of a cloud palette.
/// `auth` is `Some((base_url, token))` from SystemSettings; on WASM falls back to localStorage.
fn trigger_cloud_palette_delete(cloud_state: &mut super::CloudPaletteState, palette_id: &str, auth: Option<(&str, &str)>) {
    cloud_state.deleting = true;
    cloud_state.notification = None;

    // Get credentials
    let credentials: Result<(String, String), String> = if let Some((base_url, token)) = auth {
        Ok((base_url.to_string(), token.to_string()))
    } else {
        #[cfg(target_arch = "wasm32")]
        { get_wasm_palette_credentials() }
        #[cfg(not(target_arch = "wasm32"))]
        { Err("Not signed in — click Sign In first".to_string()) }
    };

    let (base_url, token) = match credentials {
        Ok(creds) => creds,
        Err(e) => {
            cloud_state.deleting = false;
            cloud_state.error = Some(e);
            return;
        }
    };

    let result_slot = cloud_state.delete_result.clone();
    let id = palette_id.to_string();

    #[cfg(target_arch = "wasm32")]
    wasm_bindgen_futures::spawn_local(async move {
        let result = delete_cloud_palette(&base_url, &token, &id).await;
        if let Ok(mut slot) = result_slot.lock() {
            *slot = Some(result);
        }
    });

    #[cfg(not(target_arch = "wasm32"))]
    std::thread::spawn(move || {
        let result = pollster::block_on(delete_cloud_palette(&base_url, &token, &id));
        if let Ok(mut slot) = result_slot.lock() {
            *slot = Some(result);
        }
    });
}

/// Poll async results for cloud palette operations
fn poll_cloud_palette_results(
    ui: &mut egui::Ui,
    cloud_state: &mut super::CloudPaletteState,
) {
    // Poll list result
    if let Ok(mut slot) = cloud_state.list_result.try_lock() {
        if let Some(result) = slot.take() {
            cloud_state.loading = false;
            cloud_state.fetched = true;
            match result {
                Ok(palettes) => {
                    // Clear texture cache when palette list changes
                    for p in &cloud_state.palettes {
                        let texture_id = egui::Id::new(("cloud_palette_preview", &p.id));
                        ui.ctx().data_mut(|data| {
                            data.remove::<egui::TextureHandle>(texture_id);
                        });
                    }
                    cloud_state.palettes = palettes;
                    cloud_state.error = None;
                }
                Err(e) => {
                    cloud_state.error = Some(e);
                }
            }
            ui.ctx().request_repaint();
        }
    }

    // Poll delete result
    if let Ok(mut slot) = cloud_state.delete_result.try_lock() {
        if let Some(result) = slot.take() {
            cloud_state.deleting = false;
            match result {
                Ok(deleted_id) => {
                    // Remove from local list and clear texture cache
                    let texture_id = egui::Id::new(("cloud_palette_preview", &deleted_id));
                    ui.ctx().data_mut(|data| {
                        data.remove::<egui::TextureHandle>(texture_id);
                    });
                    let name = cloud_state.palettes.iter()
                        .find(|p| p.id == deleted_id)
                        .and_then(|p| p.name.clone())
                        .unwrap_or_else(|| "palette".to_string());
                    cloud_state.palettes.retain(|p| p.id != deleted_id);
                    cloud_state.notification = Some((
                        t!("palette_library.cloud_delete_success", name = name).to_string(),
                        false,
                    ));
                }
                Err(e) => {
                    cloud_state.notification = Some((
                        t!("palette_library.cloud_delete_error", error = e).to_string(),
                        true,
                    ));
                }
            }
            ui.ctx().request_repaint();
        }
    }
}
