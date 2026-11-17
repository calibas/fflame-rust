//! Fractal Browser panel - this session's results: batches and files
//!
//! A tabbed panel that provides views (the presets and the online
//! fractals are the Catalog's, `ui::inventory`, docs/projects/catalog.md):
//! - Batch: Random batch generation results (persists until next batch)
//! - Files: Loaded .fflame files

use egui;

use super::fractal_gallery::{FractalConfigGallery, GalleryResponse};
use crate::config::FractalConfig;


/// Which tab is currently active in the browser
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrowserTab {
    #[default]
    Batch,
    Files,
}

/// State for the Fractal Browser panel
pub struct FractalBrowserPanel {
    /// Currently active tab
    current_tab: BrowserTab,

    // --- Presets tab ---

    // --- Batch tab ---
    /// Gallery for batch generation results
    batch_gallery: FractalConfigGallery,

    // --- Files tab ---
    /// Gallery for loaded file contents
    files_gallery: FractalConfigGallery,
    /// Currently loaded file path (if any)
    files_current_path: Option<std::path::PathBuf>,
    /// Error message from last file operation
    files_error_message: Option<String>,
    /// Whether file open dialog was requested
    files_open_requested: bool,
}

impl Default for FractalBrowserPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl FractalBrowserPanel {
    /// Create a new fractal browser panel
    pub fn new() -> Self {
        Self {
            current_tab: BrowserTab::Batch,

            batch_gallery: FractalConfigGallery::default(),

            files_gallery: FractalConfigGallery::default(),
            files_current_path: None,
            files_error_message: None,
            files_open_requested: false,

        }
    }

    /// Get the current tab
    #[cfg(test)]
    pub fn current_tab(&self) -> BrowserTab {
        self.current_tab
    }

    // ========== Batch tab methods ==========

    /// Load batch results (from random generation)
    pub fn load_batch(&mut self, configs: Vec<FractalConfig>) {
        if configs.is_empty() {
            return;
        }
        log::info!("Loaded {} config(s) into batch", configs.len());
        self.batch_gallery = FractalConfigGallery::new(configs);
        // Auto-switch to batch tab
        self.current_tab = BrowserTab::Batch;
    }

    // ========== Files tab methods ==========

    /// Load configs from a file path
    pub fn load_file(&mut self, path: std::path::PathBuf) {
        self.files_error_message = None;

        match FractalConfig::load_multi_from_file(&path) {
            Ok(configs) => {
                if configs.is_empty() {
                    self.files_error_message = Some(t!("file_browser.error_no_configs").to_string());
                } else {
                    log::info!("Loaded {} config(s) from {:?}", configs.len(), path);
                    self.files_gallery = FractalConfigGallery::new(configs);
                    self.files_current_path = Some(path);
                    // Auto-switch to files tab
                    self.current_tab = BrowserTab::Files;
                }
            }
            Err(e) => {
                log::error!("Failed to load file {:?}: {}", path, e);
                self.files_error_message =
                    Some(t!("file_browser.error_load_failed", error = e.to_string()).to_string());
            }
        }
    }

    /// Load configs from JSON string
    #[cfg(target_arch = "wasm32")]
    pub fn load_json(&mut self, json: &str, source_name: &str) {
        self.files_error_message = None;

        match FractalConfig::from_json_multi(json) {
            Ok(configs) => {
                if configs.is_empty() {
                    self.files_error_message =
                        Some(t!("file_browser.error_no_configs_json").to_string());
                } else {
                    log::info!("Loaded {} config(s) from {}", configs.len(), source_name);
                    self.files_gallery = FractalConfigGallery::new(configs);
                    self.files_current_path = None;
                    // Auto-switch to files tab
                    self.current_tab = BrowserTab::Files;
                }
            }
            Err(e) => {
                log::error!("Failed to parse JSON from {}: {}", source_name, e);
                self.files_error_message =
                    Some(t!("file_browser.error_parse_failed", error = e.to_string()).to_string());
            }
        }
    }

    /// Load configs that are already parsed into the Files tab -- a
    /// multi-flame `.flame` from File > Open or Import Apophysis.
    /// `path` is the file they came from, for the tab's header.
    pub fn load_configs(
        &mut self,
        configs: Vec<FractalConfig>,
        source_name: &str,
        path: Option<std::path::PathBuf>,
    ) {
        self.files_error_message = None;
        self.files_current_path = path;

        if configs.is_empty() {
            self.files_error_message =
                Some(t!("file_browser.error_no_configs_provided").to_string());
        } else {
            log::info!("Loaded {} config(s) from {}", configs.len(), source_name);
            self.files_gallery = FractalConfigGallery::new(configs);
            // Auto-switch to files tab
            self.current_tab = BrowserTab::Files;
        }
    }

    /// Check if file open dialog was requested
    pub fn take_open_file_request(&mut self) -> bool {
        std::mem::take(&mut self.files_open_requested)
    }

    // ========== Previews ==========

    /// The gallery drawn this frame, if any, every gallery's flag taken
    /// (`FractalConfigGallery::take_shown`): only it makes previews, so
    /// a closed panel or another tab makes none.
    fn shown_gallery(&mut self) -> Option<&mut FractalConfigGallery> {
        let mut shown = None;
        let galleries = [&mut self.batch_gallery, &mut self.files_gallery];
        for g in galleries {
            if g.take_shown() {
                shown = Some(g);
            }
        }
        shown
    }

    /// Desktop: make one preview for the gallery drawn this frame (one
    /// call a frame, after the UI). Returns whether one was made.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn generate_one_preview<F>(&mut self, ctx: &egui::Context, render_fn: F) -> bool
    where
        F: FnOnce(&FractalConfig) -> image::RgbaImage,
    {
        self.shown_gallery().is_some_and(|g| g.generate_one_preview(ctx, render_fn))
    }

    /// WASM: spawn the next preview of the gallery drawn this frame, one
    /// at a time.
    #[cfg(target_arch = "wasm32")]
    pub fn start_next_preview(
        &mut self,
        device: &egui_wgpu::wgpu::Device,
        queue: &egui_wgpu::wgpu::Queue,
        ctx: &egui::Context,
    ) {
        if let Some(g) = self.shown_gallery() {
            g.start_next_preview(device, queue, ctx);
        }
    }

    // ========== Rendering ==========

    /// Render the panel
    pub fn render(&mut self, ui: &mut egui::Ui) -> GalleryResponse {
        // Tab bar. Wrapped: tabs with counts exceed a phone-width
        // panel, and an unwrappable row sets the panel's minimum width.
        ui.horizontal_wrapped(|ui| {
            // Show batch count if any
            let batch_label = if self.batch_gallery.len() > 0 {
                format!("{} ({})", t!("browser.tab_batch"), self.batch_gallery.len())
            } else {
                t!("browser.tab_batch").to_string()
            };
            if ui
                .selectable_label(self.current_tab == BrowserTab::Batch, batch_label)
                .clicked()
            {
                self.current_tab = BrowserTab::Batch;
            }

            // Show file count if any
            let files_label = if self.files_gallery.len() > 0 {
                format!("{} ({})", t!("browser.tab_files"), self.files_gallery.len())
            } else {
                t!("browser.tab_files").to_string()
            };
            if ui
                .selectable_label(self.current_tab == BrowserTab::Files, files_label)
                .clicked()
            {
                self.current_tab = BrowserTab::Files;
            }

            // The online fractals moved to the Catalog.
            if ui.button(t!("browser.online_in_catalog")).on_hover_text(t!("browser.online_in_catalog_tip")).clicked() {
                super::inventory::request_open(crate::inventory::Family::Fractals, super::inventory::Place::Online);
            }
        });

        ui.separator();

        // Render current tab content
        match self.current_tab {
            BrowserTab::Batch => self.render_batch_tab(ui),
            BrowserTab::Files => self.render_files_tab(ui),
        }
    }

    fn render_batch_tab(&mut self, ui: &mut egui::Ui) -> GalleryResponse {
        if self.batch_gallery.len() > 0 {
            self.batch_gallery.render(ui)
        } else {
            // Empty state
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(50.0);
                    ui.label(t!("browser.batch_empty_title"));
                    ui.add_space(10.0);
                    ui.label(t!("browser.batch_empty_hint"));
                });
            });
            GalleryResponse::default()
        }
    }

    fn render_files_tab(&mut self, ui: &mut egui::Ui) -> GalleryResponse {
        // Toolbar: Open file button + current file info
        ui.horizontal(|ui| {
            if ui.button(t!("file_browser.open_file")).clicked() {
                self.files_open_requested = true;
            }

            ui.separator();

            if let Some(path) = &self.files_current_path {
                let filename = path.file_name().and_then(|n| n.to_str()).unwrap_or("Unknown");
                ui.label(t!("file_browser.file_icon", filename = filename));
            } else if self.files_gallery.len() > 0 {
                ui.label(t!("file_browser.loaded_from_json"));
            } else {
                ui.label(t!("file_browser.no_file_loaded"));
            }
        });

        // Show error message if any
        if let Some(error) = &self.files_error_message {
            ui.separator();
            ui.colored_label(egui::Color32::RED, format!("⚠ {}", error));
        }

        ui.separator();

        // Show gallery if we have configs
        if self.files_gallery.len() > 0 {
            self.files_gallery.render(ui)
        } else {
            // Empty state
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(50.0);
                    ui.label(t!("file_browser.empty_title"));
                    ui.add_space(10.0);
                    ui.label(t!("file_browser.empty_hint"));
                    ui.add_space(10.0);
                    ui.label(t!("file_browser.empty_description"));
                });
            });
            GalleryResponse::default()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A bundled preset, by name.
    fn preset(name: &str) -> crate::config::FractalConfig {
        crate::resources::load_presets_with_fallback()
            .into_iter()
            .find(|p| p.flame.name == name)
            .unwrap_or_else(|| panic!("no preset {name}"))
    }

    /// The bundled presets fall in the categories their pictures are: a
    /// flame by its render mode; an escape picture 3D when it is a solid --
    /// a built-in, or a Rhai solid told from its source, which nothing has
    /// compiled when the browser sorts them -- and every category has some.
    #[test]
    fn the_bundled_presets_fall_in_their_categories() {
        use crate::scene::presets::PresetCategory as C;
        for (name, want) in [
            ("Julian Disc", C::Flame2d),
            ("Cup (3D)", C::Flame3d),
            ("Sierpinski Gasket", C::Escape2d),
            ("Frothy Basin", C::Escape2d),
            ("KIFS Menger", C::Escape3d),
            ("Mandelbulb", C::Escape3d),
            ("Halayka: sin Z + C sin Z", C::Escape3d),
            ("Bristorbrot", C::Escape3d),
            ("Gray-Scott Coral", C::Simulation),
        ] {
            assert_eq!(C::of(&preset(name)), want, "{name}");
        }
        let presets = crate::resources::load_presets_with_fallback();
        for category in C::ALL {
            assert!(presets.iter().any(|p| C::of(p) == category), "{category:?} has no presets");
        }
    }

    /// A Rhai solid the config keeps but does not draw -- another formula
    /// picked, the source kept for undo -- is a plane picture; a terrain
    /// over a plane is 3D.
    #[test]
    fn a_kept_solid_is_not_3d_and_a_terrain_is() {
        use crate::scene::presets::PresetCategory as C;
        let mut kept = preset("Halayka: sin Z + C sin Z");
        assert!(kept.escape.custom_formula.is_some());
        kept.escape.formula = "mandelbrot".to_string();
        assert_eq!(C::of(&kept), C::Escape2d);
        let mut terrain = preset("Douady Rabbit");
        terrain.escape.formula = "mandelbrot".to_string();
        terrain.escape.terrain.enabled = true;
        assert_eq!(C::of(&terrain), C::Escape3d);
    }

    /// A multi-flame `.flame` lands in Files and leaves the Random tab's
    /// batch alone. It used to go through `load_batch`, replacing the
    /// last random batch and opening the Random tab, while
    /// `load_configs` -- the Files loader -- had no caller.
    #[test]
    fn parsed_configs_from_a_file_go_to_files_and_keep_the_batch() {
        let mut panel = FractalBrowserPanel::new();
        panel.load_batch(vec![FractalConfig::default(), FractalConfig::default()]);
        assert_eq!(panel.current_tab(), BrowserTab::Batch);

        let path = std::path::PathBuf::from("several.flame");
        panel.load_configs(
            vec![FractalConfig::default(), FractalConfig::default(), FractalConfig::default()],
            "several.flame",
            Some(path.clone()),
        );
        assert_eq!(panel.current_tab(), BrowserTab::Files);
        assert_eq!(panel.files_gallery.len(), 3);
        assert_eq!(panel.files_current_path, Some(path));
        assert_eq!(panel.batch_gallery.len(), 2, "the random batch was replaced");
    }
}
