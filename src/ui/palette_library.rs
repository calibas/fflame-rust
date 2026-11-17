//! Palette Library panel - browse and manage palette packs

use egui;
use crate::scene::palette::PaletteLibrary;
use crate::resources::LoadState;

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

/// Keep the app's palette library in step with the web's pack fetches.
///
/// Called every frame, before any panel draws (`EguiLayer::render_ui`).
/// It used to run inside the Palette Library panel, which no default
/// layout opens, so on the web an enabled pack was never fetched and
/// its palettes were missing from the Colors panel's dropdown until
/// that panel happened to be opened.
///
/// Starts the fetch of each enabled pack that has not been tried. A
/// pack whose fetch FAILED waits for the panel's Retry: retrying it
/// here would refetch every frame for as long as the app is offline.
#[cfg(target_arch = "wasm32")]
pub fn poll_palette_packs(ctx: &egui::Context, library: &mut PaletteLibrary) {
    // Async fetches complete into the global singleton, not this copy.
    library.sync_from_global();

    for pack_idx in 0..library.pack_count() {
        let needs_auto_load = library
            .get_pack_info(pack_idx)
            .map(|info| {
                info.enabled
                    && matches!(info.load_state, LoadState::NotLoaded)
                    && info.metadata.is_some()
            })
            .unwrap_or(false);

        if needs_auto_load {
            if let Some(file_path) = library.start_pack_load(pack_idx) {
                spawn_pack_fetch(ctx.clone(), pack_idx, file_path);
            }
        }
    }
}

/// A pack's place in a list of packs (the Catalog's palettes): a checkbox
/// that enables it -- on the web, fetching it -- and a collapsing header
/// with its name and load state; open, the body shows its loading, its
/// failure with a Retry, or, enabled and loaded, `palettes`.
pub fn pack_section(
    ui: &mut egui::Ui,
    library: &mut PaletteLibrary,
    pack_idx: usize,
    palettes: impl FnOnce(&mut egui::Ui, &mut PaletteLibrary),
) {
    let Some(info) = library.get_pack_info(pack_idx) else { return };
    let (pack_name, palette_count, is_enabled, load_state) = (info.name().to_string(), info.palette_count(), info.enabled, info.load_state.clone());
    let header_id = ui.make_persistent_id(format!("pack_header_{}", pack_idx));
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), header_id, true)
        .show_header(ui, |ui| {
            let mut enabled = is_enabled;
            if ui.checkbox(&mut enabled, "").on_hover_text(t!("palette_library.tooltip_enable_pack")).changed() {
                enable_pack(ui.ctx(), library, pack_idx, enabled);
            }
            ui.strong(&pack_name);
            match &load_state {
                LoadState::NotLoaded => {
                    ui.weak(format!("({} palettes)", palette_count));
                }
                LoadState::Loading => {
                    ui.spinner();
                    ui.weak(t!("palette_library.loading"));
                }
                LoadState::Loaded => {
                    ui.weak(t!("palette_library.palettes_count", count = palette_count));
                }
                LoadState::Failed(error) => {
                    ui.colored_label(egui::Color32::RED, "⚠");
                    ui.weak(error);
                }
            }
        })
        .body(|ui| match &load_state {
            LoadState::NotLoaded => {
                if !is_enabled {
                    ui.weak(t!("palette_library.pack_disabled"));
                }
            }
            LoadState::Loading => {
                ui.horizontal(|ui| {
                    ui.spinner();
                    ui.label(t!("palette_library.loading"));
                });
            }
            LoadState::Failed(error) => {
                ui.colored_label(egui::Color32::RED, error);
                if ui.button(t!("palette_library.retry")).on_hover_text(t!("palette_library.tooltip_retry")).clicked() {
                    if let Some(info) = library.packs_mut().get_mut(pack_idx) {
                        info.load_state = LoadState::NotLoaded;
                    }
                    enable_pack(ui.ctx(), library, pack_idx, true);
                }
            }
            LoadState::Loaded => {
                if is_enabled {
                    palettes(ui, library);
                } else {
                    ui.weak(t!("palette_library.pack_disabled"));
                }
            }
        });
}

/// Enable or disable a pack; enabling one not loaded loads it (on the
/// web, a fetch).
fn enable_pack(ctx: &egui::Context, library: &mut PaletteLibrary, pack_idx: usize, enabled: bool) {
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = ctx;
        library.set_pack_enabled(pack_idx, enabled);
    }
    #[cfg(target_arch = "wasm32")]
    {
        if let Some(file_path) = library.set_pack_enabled(pack_idx, enabled) {
            spawn_pack_fetch(ctx.clone(), pack_idx, file_path);
        }
    }
}

/// A palette's gradient as a texture, made once and kept in egui's
/// memory under `key` (a content hash names a palette exactly).
pub fn palette_preview(ctx: &egui::Context, key: &str, palette: &crate::scene::palette::Palette, size: [usize; 2]) -> egui::TextureHandle {
    let id = egui::Id::new(("palette_preview", key, size));
    if let Some(t) = ctx.data_mut(|d| d.get_temp::<egui::TextureHandle>(id)) {
        return t;
    }
    let image = PaletteLibrary::generate_preview(palette, size[0], size[1]);
    let t = ctx.load_texture(format!("palette_{key}"), image, egui::TextureOptions::LINEAR);
    ctx.data_mut(|d| d.insert_temp(id, t.clone()));
    t
}
