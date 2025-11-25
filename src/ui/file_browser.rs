//! File Browser panel - browse and load .fflame files
//!
//! Allows users to open .fflame files (single or multi-config) and
//! displays thumbnails for each configuration within the file.

use egui;

use super::fractal_gallery::{FractalConfigGallery, GalleryResponse};
use crate::config::FractalConfig;

/// State for the File Browser panel
pub struct FileBrowserPanel {
    /// Gallery widget for displaying configs
    gallery: FractalConfigGallery,
    /// Currently loaded file path (if any)
    current_file: Option<std::path::PathBuf>,
    /// Error message from last operation
    error_message: Option<String>,
    /// Whether a file open dialog was requested
    open_file_requested: bool,
}

impl FileBrowserPanel {
    /// Create a new empty file browser panel
    pub fn new() -> Self {
        Self {
            gallery: FractalConfigGallery::default(),
            current_file: None,
            error_message: None,
            open_file_requested: false,
        }
    }

    /// Load configs from a file path
    pub fn load_file(&mut self, path: std::path::PathBuf) {
        self.error_message = None;

        match FractalConfig::load_multi_from_file(&path) {
            Ok(configs) => {
                if configs.is_empty() {
                    self.error_message = Some("File contains no configurations".to_string());
                } else {
                    log::info!("Loaded {} config(s) from {:?}", configs.len(), path);
                    self.gallery = FractalConfigGallery::new(configs);
                    self.current_file = Some(path);
                }
            }
            Err(e) => {
                log::error!("Failed to load file {:?}: {}", path, e);
                self.error_message = Some(format!("Failed to load file: {}", e));
            }
        }
    }

    /// Load configs from JSON string (e.g., from clipboard or dialog)
    pub fn load_json(&mut self, json: &str, source_name: &str) {
        self.error_message = None;

        match FractalConfig::from_json_multi(json) {
            Ok(configs) => {
                if configs.is_empty() {
                    self.error_message = Some("JSON contains no configurations".to_string());
                } else {
                    log::info!("Loaded {} config(s) from {}", configs.len(), source_name);
                    self.gallery = FractalConfigGallery::new(configs);
                    self.current_file = None; // No file path for JSON import
                }
            }
            Err(e) => {
                log::error!("Failed to parse JSON from {}: {}", source_name, e);
                self.error_message = Some(format!("Failed to parse JSON: {}", e));
            }
        }
    }

    /// Load configs directly (e.g., from Apophysis import)
    pub fn load_configs(&mut self, configs: Vec<FractalConfig>, source_name: &str) {
        self.error_message = None;
        self.current_file = None;

        if configs.is_empty() {
            self.error_message = Some("No configurations provided".to_string());
        } else {
            log::info!("Loaded {} config(s) from {}", configs.len(), source_name);
            self.gallery = FractalConfigGallery::new(configs);
        }
    }

    /// Check if thumbnail generation is in progress
    pub fn is_generating(&self) -> bool {
        self.gallery.is_generating()
    }

    /// Generate one thumbnail (call once per frame during generation)
    pub fn generate_one_thumbnail<F>(&mut self, ctx: &egui::Context, render_fn: F) -> bool
    where
        F: FnOnce(&FractalConfig) -> image::RgbaImage,
    {
        self.gallery.generate_one_thumbnail(ctx, render_fn)
    }

    /// Check if file open dialog was requested
    pub fn take_open_file_request(&mut self) -> bool {
        std::mem::take(&mut self.open_file_requested)
    }

    /// Render the panel
    pub fn render(&mut self, ui: &mut egui::Ui) -> GalleryResponse {
        // Toolbar: Open file button + current file info
        ui.horizontal(|ui| {
            if ui.button("📂 Open File...").clicked() {
                self.open_file_requested = true;
            }

            ui.separator();

            if let Some(path) = &self.current_file {
                let filename = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown");
                ui.label(format!("📄 {}", filename));
            } else if self.gallery.len() > 0 {
                ui.label("(Loaded from JSON)");
            } else {
                ui.label("No file loaded");
            }
        });

        // Show error message if any
        if let Some(error) = &self.error_message {
            ui.separator();
            ui.colored_label(egui::Color32::RED, format!("⚠ {}", error));
        }

        ui.separator();

        // Show gallery if we have configs
        if self.gallery.len() > 0 {
            self.gallery.render(ui)
        } else {
            // Empty state
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(50.0);
                    ui.label("No file loaded");
                    ui.add_space(10.0);
                    ui.label("Click 'Open File...' to browse .fflame files");
                    ui.add_space(10.0);
                    ui.label("Files can contain one or multiple fractal configurations");
                });
            });
            GalleryResponse::default()
        }
    }

    /// Get generation progress
    pub fn generation_progress(&self) -> (usize, usize) {
        self.gallery.generation_progress()
    }
}

impl Default for FileBrowserPanel {
    fn default() -> Self {
        Self::new()
    }
}
