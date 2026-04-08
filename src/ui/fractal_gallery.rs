//! Reusable gallery widget for browsing and selecting FractalConfigs
//!
//! Used by:
//! - Preset Library panel (built-in presets)
//! - User Library panel (saved fractals)
//! - Backup Library panel (auto-saved backups)
//!
//! On desktop: Thumbnails are rendered synchronously using pollster
//! On WASM: Thumbnails are rendered asynchronously using wasm_bindgen_futures

use std::collections::VecDeque;

use egui::{self, Color32, CursorIcon, Sense, Vec2};
use rust_i18n::t;

use crate::config::FractalConfig;
use crate::storage::{GalleryItem, TextureCache, ThumbnailCache};

#[cfg(target_arch = "wasm32")]
use std::cell::RefCell;
#[cfg(target_arch = "wasm32")]
use std::collections::HashMap;
#[cfg(target_arch = "wasm32")]
use std::rc::Rc;

/// View mode for the gallery
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum GalleryViewMode {
    #[default]
    Grid,
    List,
}

/// Response from gallery rendering
#[derive(Default)]
pub struct GalleryResponse {
    /// Config was selected (clicked)
    pub selected: Option<FractalConfig>,
    /// Request to close the panel
    pub close_requested: bool,
    /// API flame ID (when loading from Online tab)
    pub api_flame_id: Option<String>,
    /// Whether the loaded flame is public (None = unknown)
    pub api_flame_is_public: Option<bool>,
    /// API notification message (message, is_error) — e.g. delete result
    pub api_notification: Option<(String, bool)>,
    /// Session expired (401 detected) — triggers sign-out flow
    pub session_expired: bool,
}

/// Shared state for async thumbnail results (WASM only)
#[cfg(target_arch = "wasm32")]
type AsyncThumbnailResults = Rc<RefCell<HashMap<String, image::RgbaImage>>>;

/// Reusable gallery widget for browsing FractalConfigs
pub struct FractalConfigGallery {
    /// Items to display (config + precomputed hash)
    items: Vec<GalleryItem>,

    /// Current view mode
    view_mode: GalleryViewMode,

    /// Search filter text
    search_query: String,

    /// Thumbnail display size (grid view)
    thumbnail_size: f32,

    /// GPU texture cache (hash -> TextureHandle)
    texture_cache: TextureCache,

    /// Disk cache reference
    disk_cache: ThumbnailCache,

    /// Queue of items pending thumbnail generation (desktop only)
    pending_generation: VecDeque<usize>,

    /// Total items that needed generation (for progress calculation)
    total_to_generate: usize,

    /// WASM: Shared state for receiving async thumbnail results
    #[cfg(target_arch = "wasm32")]
    async_results: AsyncThumbnailResults,

    /// WASM: Set of hashes currently being rendered (to avoid duplicate spawns)
    #[cfg(target_arch = "wasm32")]
    in_flight: std::collections::HashSet<String>,
}

impl FractalConfigGallery {
    /// Create a new gallery with the given configs
    pub fn new(configs: Vec<FractalConfig>) -> Self {
        let items: Vec<GalleryItem> = configs.into_iter().map(GalleryItem::new).collect();

        Self {
            items,
            view_mode: GalleryViewMode::Grid,
            search_query: String::new(),
            thumbnail_size: 128.0,
            texture_cache: TextureCache::new(),
            disk_cache: ThumbnailCache::new(),
            pending_generation: VecDeque::new(),
            total_to_generate: 0,
            #[cfg(target_arch = "wasm32")]
            async_results: Rc::new(RefCell::new(HashMap::new())),
            #[cfg(target_arch = "wasm32")]
            in_flight: std::collections::HashSet::new(),
        }
    }

    /// Add configs to the gallery (e.g., when loading a new library file)
    pub fn add_configs(&mut self, configs: Vec<FractalConfig>) {
        for config in configs {
            self.items.push(GalleryItem::new(config));
        }
    }

    /// Get all configs
    pub fn configs(&self) -> Vec<&FractalConfig> {
        self.items.iter().map(|item| &item.config).collect()
    }

    /// Get number of items
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if gallery is empty
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Check if thumbnail generation is in progress (desktop only)
    #[cfg(not(target_arch = "wasm32"))]
    pub fn is_generating(&self) -> bool {
        !self.pending_generation.is_empty()
    }

    /// WASM: Start async thumbnail generation for items that need it
    ///
    /// This spawns async tasks to render thumbnails in the background.
    /// Results are polled in `render()` via `poll_async_thumbnails()`.
    #[cfg(target_arch = "wasm32")]
    pub fn start_async_thumbnail_generation(
        &mut self,
        device: &egui_wgpu::wgpu::Device,
        queue: &egui_wgpu::wgpu::Queue,
    ) {
        use wasm_bindgen_futures::spawn_local;

        for item in &self.items {
            let hash = item.hash.clone();

            // Skip if already cached or in-flight
            if self.texture_cache.contains(&hash) {
                continue;
            }
            if self.in_flight.contains(&hash) {
                continue;
            }

            // Mark as in-flight
            self.in_flight.insert(hash.clone());

            // Clone what we need for the async task
            let config = item.config.clone();
            let results = Rc::clone(&self.async_results);

            // We need to clone device/queue handles for the async task
            // Note: wgpu Device and Queue are internally Arc-wrapped, so this is cheap
            let device = device.clone();
            let queue = queue.clone();

            spawn_local(async move {
                let image = crate::renderer::render_thumbnail_async(&device, &queue, &config).await;

                // Store result for pickup on next frame
                results.borrow_mut().insert(hash, image);
            });
        }
    }

    /// Get generation progress (completed, total)
    pub fn generation_progress(&self) -> (usize, usize) {
        let completed = self.total_to_generate - self.pending_generation.len();
        (completed, self.total_to_generate)
    }

    /// Render the gallery UI
    /// Returns GalleryResponse with selected config if any
    pub fn render(&mut self, ui: &mut egui::Ui) -> GalleryResponse {
        // WASM: Poll for completed async thumbnails and upload them
        #[cfg(target_arch = "wasm32")]
        self.poll_async_thumbnails(ui.ctx());

        // Desktop: Queue missing thumbnails for synchronous generation
        #[cfg(not(target_arch = "wasm32"))]
        self.queue_missing_thumbnails();

        // Desktop only: If generating, show progress modal and wait
        #[cfg(not(target_arch = "wasm32"))]
        if !self.pending_generation.is_empty() {
            self.show_generation_progress(ui);
            // Note: Actual generation happens in generate_one_thumbnail()
            // which must be called by the parent with access to the renderer
            return GalleryResponse::default();
        }

        // Render toolbar
        self.render_toolbar(ui);

        ui.separator();

        // Render gallery based on view mode
        match self.view_mode {
            GalleryViewMode::Grid => self.render_grid(ui),
            GalleryViewMode::List => self.render_list(ui),
        }
    }

    /// Generate one thumbnail (call once per frame during generation)
    /// Returns true if generation is complete
    pub fn generate_one_thumbnail<F>(&mut self, ctx: &egui::Context, render_fn: F) -> bool
    where
        F: FnOnce(&FractalConfig) -> image::RgbaImage,
    {
        let Some(index) = self.pending_generation.pop_front() else {
            return true; // Nothing to generate
        };

        // Clone what we need to avoid borrow issues
        let hash = self.items[index].hash.clone();
        let config = self.items[index].config.clone();

        // Check disk cache first
        if let Some(image) = self.disk_cache.load(&hash) {
            self.upload_texture(ctx, &hash, image);
            return self.pending_generation.is_empty();
        }

        // Render thumbnail
        let image = render_fn(&config);

        // Save to disk cache
        self.disk_cache.save(&hash, &image).ok();

        // Upload to GPU texture
        self.upload_texture(ctx, &hash, image);

        self.pending_generation.is_empty()
    }

    /// Get the next config that needs thumbnail generation (if any)
    pub fn next_pending_config(&self) -> Option<&FractalConfig> {
        self.pending_generation
            .front()
            .map(|&index| &self.items[index].config)
    }

    /// Get the name of the config currently being generated
    pub fn current_generating_name(&self) -> Option<&str> {
        self.pending_generation
            .front()
            .map(|&index| self.items[index].config.flame.name.as_str())
    }

    /// Desktop: Queue missing thumbnails for synchronous generation
    #[cfg(not(target_arch = "wasm32"))]
    fn queue_missing_thumbnails(&mut self) {
        // Only queue if not already generating
        if !self.pending_generation.is_empty() {
            return;
        }

        for (index, item) in self.items.iter().enumerate() {
            // Skip if already in texture cache or disk cache
            if self.texture_cache.contains(&item.hash) {
                continue;
            }
            if self.disk_cache.exists(&item.hash) {
                // Will load from disk on first access
                continue;
            }
            self.pending_generation.push_back(index);
        }

        self.total_to_generate = self.pending_generation.len();
    }

    /// WASM: Poll for completed async thumbnail results and upload them
    #[cfg(target_arch = "wasm32")]
    fn poll_async_thumbnails(&mut self, ctx: &egui::Context) {
        // Check for completed async renders
        let completed: Vec<(String, image::RgbaImage)> = {
            let mut results = self.async_results.borrow_mut();
            results.drain().collect()
        };

        // Upload completed thumbnails
        for (hash, image) in completed {
            self.in_flight.remove(&hash);
            self.upload_texture(ctx, &hash, image);
        }
    }

    fn show_generation_progress(&self, ui: &mut egui::Ui) {
        let (completed, total) = self.generation_progress();
        let progress = if total > 0 {
            completed as f32 / total as f32
        } else {
            0.0
        };

        let name = self.current_generating_name().unwrap_or("...");

        egui::Window::new(t!("fractal_gallery.generating_title"))
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.add(
                    egui::ProgressBar::new(progress)
                        .text(format!("{}/{}", completed, total))
                        .animate(true),
                );
                ui.label(t!("fractal_gallery.rendering", name = name));
            });
    }

    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            // View mode toggle
            ui.selectable_value(&mut self.view_mode, GalleryViewMode::Grid, t!("fractal_gallery.view_grid"));
            ui.selectable_value(&mut self.view_mode, GalleryViewMode::List, t!("fractal_gallery.view_list"));

            ui.separator();

            // Search box
            ui.label(t!("fractal_gallery.search"));
            let r = ui.add(
                egui::TextEdit::singleline(&mut self.search_query)
                    .desired_width(150.0)
                    .hint_text(t!("fractal_gallery.search_hint")),
            );
            super::vkb_sync(ui, &r, &self.search_query);

            if !self.search_query.is_empty() && ui.button(t!("fractal_gallery.clear_search")).clicked() {
                self.search_query.clear();
            }

            ui.separator();

            // Thumbnail size slider (grid view only)
            if self.view_mode == GalleryViewMode::Grid {
                ui.label(t!("fractal_gallery.size"));
                ui.add(super::VkbSlider::new(&mut self.thumbnail_size, 64.0..=256.0).show_value(false));
            }
        });
    }

    fn render_grid(&mut self, ui: &mut egui::Ui) -> GalleryResponse {
        let mut response = GalleryResponse::default();

        // Collect filtered items with their data (clone to avoid borrow issues)
        let filtered: Vec<(usize, String, String, FractalConfig)> = {
            let query = self.search_query.to_lowercase();
            self.items
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    if query.is_empty() {
                        return true;
                    }
                    item.config.flame.name.to_lowercase().contains(&query)
                })
                .map(|(idx, item)| {
                    (idx, item.hash.clone(), item.config.flame.name.clone(), item.config.clone())
                })
                .collect()
        };

        ui.horizontal_wrapped(|ui| {
            for (original_index, hash, name, config) in &filtered {
                let card_response = self.render_grid_card(ui, hash, name);
                if card_response.clicked() {
                    response.selected = Some(config.clone());
                }
            }
        });

        response
    }

    fn render_grid_card(
        &mut self,
        ui: &mut egui::Ui,
        hash: &str,
        name: &str,
    ) -> egui::Response {
        let card_size = Vec2::new(self.thumbnail_size, self.thumbnail_size + 24.0);

        // Allocate space for the card
        let (rect, response) = ui.allocate_exact_size(card_size, Sense::click());

        if ui.is_rect_visible(rect) {
            let visuals = ui.visuals();

            // Hover highlight
            if response.hovered() {
                ui.painter().rect_filled(
                    rect.expand(4.0),
                    4.0,
                    visuals.widgets.hovered.bg_fill,
                );
                ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
            }

            // Thumbnail area
            let img_rect = egui::Rect::from_min_size(
                rect.min,
                Vec2::new(self.thumbnail_size, self.thumbnail_size),
            );

            // Try to get texture, or load from disk cache
            if !self.texture_cache.contains(hash) {
                if let Some(image) = self.disk_cache.load(hash) {
                    self.upload_texture(ui.ctx(), hash, image);
                }
            }

            if let Some(texture) = self.texture_cache.get(hash) {
                // Draw thumbnail
                let uv = egui::Rect::from_min_max(egui::pos2(0.0, 0.0), egui::pos2(1.0, 1.0));
                ui.painter().image(
                    texture.id(),
                    img_rect,
                    uv,
                    Color32::WHITE,
                );
            } else {
                // Placeholder
                ui.painter().rect_filled(img_rect, 4.0, Color32::DARK_GRAY);
                // Spinner in center
                let spinner_pos = img_rect.center();
                ui.painter().circle_stroke(
                    spinner_pos,
                    16.0,
                    egui::Stroke::new(2.0, Color32::GRAY),
                );
            }

            // Name below thumbnail
            let name_rect = egui::Rect::from_min_size(
                img_rect.left_bottom() + Vec2::new(0.0, 4.0),
                Vec2::new(self.thumbnail_size, 20.0),
            );

            // Truncate name if too long
            let max_chars = (self.thumbnail_size / 8.0) as usize;
            let display_name = if name.len() > max_chars {
                format!("{}...", &name[..max_chars.saturating_sub(3)])
            } else {
                name.to_string()
            };

            ui.painter().text(
                name_rect.center_top() + Vec2::new(0.0, 2.0),
                egui::Align2::CENTER_TOP,
                display_name,
                egui::FontId::default(),
                visuals.text_color(),
            );
        }

        response
    }

    fn render_list(&mut self, ui: &mut egui::Ui) -> GalleryResponse {
        let mut response = GalleryResponse::default();

        // Collect filtered items with their data (clone to avoid borrow issues)
        let filtered: Vec<(usize, String, FractalConfig)> = {
            let query = self.search_query.to_lowercase();
            self.items
                .iter()
                .enumerate()
                .filter(|(_, item)| {
                    if query.is_empty() {
                        return true;
                    }
                    item.config.flame.name.to_lowercase().contains(&query)
                })
                .map(|(idx, item)| (idx, item.hash.clone(), item.config.clone()))
                .collect()
        };

        for (original_index, hash, config) in &filtered {
            let header_id = ui.make_persistent_id(format!("gallery_list_{}", original_index));

            egui::collapsing_header::CollapsingState::load_with_default_open(
                ui.ctx(),
                header_id,
                false,
            )
            .show_header(ui, |ui| {
                ui.horizontal(|ui| {
                    // Load button
                    if ui.button(">").clicked() {
                        response.selected = Some(config.clone());
                    }

                    ui.strong(&config.flame.name);
                });

                // Summary line
                let render_mode = match config.flame.render_mode {
                    crate::scene::transforms::RenderMode::TwoD => t!("fractal_gallery.render_mode_2d"),
                    crate::scene::transforms::RenderMode::ThreeD => t!("fractal_gallery.render_mode_3d"),
                };
                ui.label(format!(
                    "{}, {}, {}",
                    t!("fractal_gallery.transforms_count", count = config.flame.transforms.len()),
                    render_mode,
                    Self::format_variations(config),
                ));
            })
            .body(|ui| {
                // Expanded details
                ui.label(t!("fractal_gallery.zoom", value = format!("{:.2}", config.zoom)));
                ui.label(t!("fractal_gallery.max_iterations", value = config.max_iterations));
                ui.label(t!("fractal_gallery.color_mode", value = format!("{:?}", config.color_mode)));

                // Small thumbnail if available
                if let Some(texture) = self.texture_cache.get(hash) {
                    ui.image((texture.id(), Vec2::new(64.0, 64.0)));
                }
            });

            ui.separator();
        }

        response
    }

    fn format_variations(config: &FractalConfig) -> String {
        let mut variations: Vec<String> = config
            .flame
            .transforms
            .iter()
            .flat_map(|t| t.variations.keys().cloned())
            .collect();
        variations.sort();
        variations.dedup();

        if variations.len() <= 3 {
            variations.join(", ")
        } else {
            t!("fractal_gallery.variations_more",
                first = variations[..2].join(", "),
                count = variations.len() - 2
            ).to_string()
        }
    }

    fn upload_texture(&mut self, ctx: &egui::Context, hash: &str, image: image::RgbaImage) {
        let size = [image.width() as usize, image.height() as usize];
        let pixels = image.into_flat_samples();
        let color_image = egui::ColorImage::from_rgba_unmultiplied(size, &pixels.samples);

        let texture = ctx.load_texture(
            format!("thumbnail_{}", hash),
            color_image,
            egui::TextureOptions::LINEAR,
        );

        self.texture_cache.insert(hash.to_string(), texture);
        self.disk_cache.mark_cached(hash);
    }
}

impl Default for FractalConfigGallery {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}
