//! Preset Library panel - browse and select built-in presets
//!
//! Displays presets using the FractalConfigGallery widget with
//! thumbnail previews and search functionality.

use egui;

use super::fractal_gallery::{FractalConfigGallery, GalleryResponse};
use crate::config::FractalConfig;
use crate::scene::presets::PresetLibrary;

/// State for the Preset Library panel
pub struct PresetLibraryPanel {
    /// Gallery widget
    gallery: FractalConfigGallery,
    /// Whether we need to generate thumbnails
    needs_generation: bool,
}

impl PresetLibraryPanel {
    /// Create a new preset library panel
    pub fn new(preset_library: &PresetLibrary) -> Self {
        let configs: Vec<FractalConfig> = preset_library
            .presets()
            .iter()
            .cloned()
            .collect();

        Self {
            gallery: FractalConfigGallery::new(configs),
            needs_generation: true,
        }
    }

    /// Check if thumbnail generation is in progress
    pub fn is_generating(&self) -> bool {
        self.gallery.is_generating()
    }

    /// Get the next config that needs thumbnail generation
    pub fn next_pending_config(&self) -> Option<&FractalConfig> {
        self.gallery.next_pending_config()
    }

    /// Generate one thumbnail (call once per frame during generation)
    pub fn generate_one_thumbnail<F>(&mut self, ctx: &egui::Context, render_fn: F) -> bool
    where
        F: FnOnce(&FractalConfig) -> image::RgbaImage,
    {
        self.gallery.generate_one_thumbnail(ctx, render_fn)
    }

    /// Render the panel
    pub fn render(&mut self, ui: &mut egui::Ui) -> GalleryResponse {
        self.gallery.render(ui)
    }

    /// Get generation progress
    pub fn generation_progress(&self) -> (usize, usize) {
        self.gallery.generation_progress()
    }
}

impl Default for PresetLibraryPanel {
    fn default() -> Self {
        Self {
            gallery: FractalConfigGallery::default(),
            needs_generation: true,
        }
    }
}
