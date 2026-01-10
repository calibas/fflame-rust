//! Fractal Browser panel - unified browser for presets, batch results, and files
//!
//! A tabbed panel that provides three views:
//! - Presets: Built-in presets from assets/presets.fflame
//! - Batch: Random batch generation results (persists until next batch)
//! - Files: Loaded .fflame files

use egui;
use rust_i18n::t;

use super::fractal_gallery::{FractalConfigGallery, GalleryResponse};
use crate::config::FractalConfig;
use crate::scene::presets::global_preset_library;

/// Which tab is currently active in the browser
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrowserTab {
    #[default]
    Presets,
    Batch,
    Files,
}

/// State for the Fractal Browser panel
pub struct FractalBrowserPanel {
    /// Currently active tab
    current_tab: BrowserTab,

    // --- Presets tab ---
    /// Gallery for presets
    presets_gallery: FractalConfigGallery,
    /// Whether preset thumbnails need generation
    presets_needs_generation: bool,

    // --- Batch tab ---
    /// Gallery for batch generation results
    batch_gallery: FractalConfigGallery,
    /// Whether batch thumbnails need generation
    batch_needs_generation: bool,

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
        // Load presets from global library
        let preset_library = global_preset_library();
        let presets: Vec<FractalConfig> = preset_library.presets().iter().cloned().collect();

        Self {
            current_tab: BrowserTab::Presets,

            presets_gallery: FractalConfigGallery::new(presets),
            presets_needs_generation: true,

            batch_gallery: FractalConfigGallery::default(),
            batch_needs_generation: false,

            files_gallery: FractalConfigGallery::default(),
            files_current_path: None,
            files_error_message: None,
            files_open_requested: false,
        }
    }

    /// Get the current tab
    pub fn current_tab(&self) -> BrowserTab {
        self.current_tab
    }

    /// Switch to a specific tab
    pub fn switch_to_tab(&mut self, tab: BrowserTab) {
        self.current_tab = tab;
    }

    // ========== Batch tab methods ==========

    /// Load batch results (from random generation)
    pub fn load_batch(&mut self, configs: Vec<FractalConfig>) {
        if configs.is_empty() {
            return;
        }
        log::info!("Loaded {} config(s) into batch", configs.len());
        self.batch_gallery = FractalConfigGallery::new(configs);
        self.batch_needs_generation = true;
        // Auto-switch to batch tab
        self.current_tab = BrowserTab::Batch;
    }

    /// Check if batch has any results
    pub fn has_batch_results(&self) -> bool {
        self.batch_gallery.len() > 0
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

    /// Load configs directly (e.g., from Apophysis import)
    pub fn load_configs(&mut self, configs: Vec<FractalConfig>, source_name: &str) {
        self.files_error_message = None;
        self.files_current_path = None;

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

    // ========== Thumbnail generation (desktop) ==========

    /// Check if any gallery is generating thumbnails (desktop only)
    #[cfg(not(target_arch = "wasm32"))]
    pub fn is_generating(&self) -> bool {
        self.presets_gallery.is_generating()
            || self.batch_gallery.is_generating()
            || self.files_gallery.is_generating()
    }

    /// Get the next config that needs thumbnail generation (desktop only)
    /// Returns (gallery_id, config) where gallery_id identifies which gallery
    #[cfg(not(target_arch = "wasm32"))]
    pub fn next_pending_config(&self) -> Option<(BrowserTab, &FractalConfig)> {
        // Prioritize current tab
        match self.current_tab {
            BrowserTab::Presets => {
                if let Some(config) = self.presets_gallery.next_pending_config() {
                    return Some((BrowserTab::Presets, config));
                }
            }
            BrowserTab::Batch => {
                if let Some(config) = self.batch_gallery.next_pending_config() {
                    return Some((BrowserTab::Batch, config));
                }
            }
            BrowserTab::Files => {
                if let Some(config) = self.files_gallery.next_pending_config() {
                    return Some((BrowserTab::Files, config));
                }
            }
        }

        // Then check others
        if let Some(config) = self.presets_gallery.next_pending_config() {
            return Some((BrowserTab::Presets, config));
        }
        if let Some(config) = self.batch_gallery.next_pending_config() {
            return Some((BrowserTab::Batch, config));
        }
        if let Some(config) = self.files_gallery.next_pending_config() {
            return Some((BrowserTab::Files, config));
        }

        None
    }

    /// Generate one thumbnail for the specified gallery (desktop only)
    #[cfg(not(target_arch = "wasm32"))]
    pub fn generate_one_thumbnail<F>(
        &mut self,
        tab: BrowserTab,
        ctx: &egui::Context,
        render_fn: F,
    ) -> bool
    where
        F: FnOnce(&FractalConfig) -> image::RgbaImage,
    {
        match tab {
            BrowserTab::Presets => self.presets_gallery.generate_one_thumbnail(ctx, render_fn),
            BrowserTab::Batch => self.batch_gallery.generate_one_thumbnail(ctx, render_fn),
            BrowserTab::Files => self.files_gallery.generate_one_thumbnail(ctx, render_fn),
        }
    }

    /// WASM: Start async thumbnail generation for all galleries
    #[cfg(target_arch = "wasm32")]
    pub fn start_async_thumbnails(
        &mut self,
        device: &egui_wgpu::wgpu::Device,
        queue: &egui_wgpu::wgpu::Queue,
    ) {
        if self.presets_needs_generation {
            self.presets_gallery
                .start_async_thumbnail_generation(device, queue);
            self.presets_needs_generation = false;
        }
        if self.batch_needs_generation {
            self.batch_gallery
                .start_async_thumbnail_generation(device, queue);
            self.batch_needs_generation = false;
        }
        // Files gallery starts generation when files are loaded
        self.files_gallery
            .start_async_thumbnail_generation(device, queue);
    }

    /// Get generation progress for the current tab
    pub fn generation_progress(&self) -> (usize, usize) {
        match self.current_tab {
            BrowserTab::Presets => self.presets_gallery.generation_progress(),
            BrowserTab::Batch => self.batch_gallery.generation_progress(),
            BrowserTab::Files => self.files_gallery.generation_progress(),
        }
    }

    // ========== Rendering ==========

    /// Render the panel
    pub fn render(&mut self, ui: &mut egui::Ui) -> GalleryResponse {
        // Tab bar
        ui.horizontal(|ui| {
            if ui
                .selectable_label(self.current_tab == BrowserTab::Presets, t!("browser.tab_presets"))
                .clicked()
            {
                self.current_tab = BrowserTab::Presets;
            }

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
        });

        ui.separator();

        // Render current tab content
        match self.current_tab {
            BrowserTab::Presets => self.render_presets_tab(ui),
            BrowserTab::Batch => self.render_batch_tab(ui),
            BrowserTab::Files => self.render_files_tab(ui),
        }
    }

    fn render_presets_tab(&mut self, ui: &mut egui::Ui) -> GalleryResponse {
        self.presets_gallery.render(ui)
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
