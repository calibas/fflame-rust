//! Reusable gallery widget for browsing and selecting FractalConfigs
//!
//! Used by:
//! - Preset Library panel (built-in presets)
//! - User Library panel (saved fractals)
//! - Backup Library panel (auto-saved backups)
//!
//! Previews (`renderer::render_preview_async`) are made one at a time,
//! and only while the gallery is drawn: the cards show at once, a
//! placeholder where a preview is missing, and the missing ones fill in
//! -- those on screen first, then the rest of the gallery. Closing the
//! panel or switching its tab stops them. On desktop the app renders one
//! a frame, blocking (`generate_one_preview`); on WASM one is spawned at
//! a time (`start_next_preview`).

use egui::{self, Color32, CursorIcon, Sense, Vec2};

use crate::config::FractalConfig;
use crate::storage::{shared_thumbnail_cache, GalleryItem, TextureCache};

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
    /// API flame ID (when loading from Online tab)
    pub api_flame_id: Option<String>,
    /// Whether the loaded flame is public (None = unknown)
    pub api_flame_is_public: Option<bool>,
    /// User ID of the flame's owner (for ownership check)
    pub api_flame_user_id: Option<String>,
    /// Number of animations linked to this flame
    pub api_flame_animation_count: u32,
    /// List of animations linked to this flame
    pub api_flame_animations: Vec<crate::api::types::AnimationSummary>,
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

    /// GPU texture cache (hash -> TextureHandle). The disk cache is
    /// `shared_thumbnail_cache`, one for every gallery.
    texture_cache: TextureCache,

    /// Drawn since the app last asked (`take_shown`): a gallery makes
    /// previews only while it is on screen.
    shown: bool,

    /// The items whose card was on screen without a preview at the last
    /// draw, in the order they were drawn: made first.
    visible_missing: Vec<usize>,

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
            shown: false,
            visible_missing: Vec::new(),
            #[cfg(target_arch = "wasm32")]
            async_results: Rc::new(RefCell::new(HashMap::new())),
            #[cfg(target_arch = "wasm32")]
            in_flight: std::collections::HashSet::new(),
        }
    }

    /// Add configs to the gallery (e.g., when loading a new library file)
    #[cfg(target_arch = "wasm32")]
    pub fn add_configs(&mut self, configs: Vec<FractalConfig>) {
        for config in configs {
            self.items.push(GalleryItem::new(config));
        }
    }

    /// Get all configs
    #[cfg(target_arch = "wasm32")]
    pub fn configs(&self) -> Vec<&FractalConfig> {
        self.items.iter().map(|item| &item.config).collect()
    }

    /// Get number of items
    pub fn len(&self) -> usize {
        self.items.len()
    }

    /// Check if gallery is empty
    #[cfg(target_arch = "wasm32")]
    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    /// Whether it was drawn since the last call, and forget that it was:
    /// the app asks once a frame, after the UI, so a gallery not drawn
    /// this frame -- its panel closed, another tab shown -- makes nothing.
    pub fn take_shown(&mut self) -> bool {
        std::mem::take(&mut self.shown)
    }

    /// Whether `index` has no preview yet: none uploaded, none on disk
    /// (a disk hit loads when its card is drawn), none being made.
    fn is_missing(&self, index: usize) -> bool {
        let hash = &self.items[index].hash;
        if self.texture_cache.contains(hash) {
            return false;
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            !shared_thumbnail_cache().exists(hash)
        }
        #[cfg(target_arch = "wasm32")]
        {
            !self.in_flight.contains(hash)
        }
    }

    /// The next preview to make: a card on screen first, then the rest
    /// of the gallery in its order, so a scroll finds them made.
    fn next_missing(&self) -> Option<usize> {
        self.next_of(|i| self.is_missing(i))
    }

    /// [`next_missing`](Self::next_missing)'s order, over any `missing`.
    fn next_of(&self, missing: impl Fn(usize) -> bool) -> Option<usize> {
        self.visible_missing
            .iter()
            .copied()
            .find(|&i| missing(i))
            .or_else(|| (0..self.items.len()).find(|&i| missing(i)))
    }

    /// How many previews are still to make.
    fn missing_count(&self) -> usize {
        (0..self.items.len()).filter(|&i| self.is_missing(i)).count()
    }

    /// Desktop: make the next missing preview with `render_fn` (one call
    /// a frame, blocking), save it to the disk cache and upload it.
    /// Returns whether one was made.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn generate_one_preview<F>(&mut self, ctx: &egui::Context, render_fn: F) -> bool
    where
        F: FnOnce(&FractalConfig) -> image::RgbaImage,
    {
        let Some(index) = self.next_missing() else {
            return false;
        };
        let hash = self.items[index].hash.clone();
        let image = render_fn(&self.items[index].config);
        shared_thumbnail_cache().save(&hash, &image).ok();
        self.upload_texture(ctx, &hash, image);
        true
    }

    /// WASM: spawn the next missing preview, one at a time; its result
    /// is taken in `render()` (`poll_async_thumbnails`).
    #[cfg(target_arch = "wasm32")]
    pub fn start_next_preview(
        &mut self,
        device: &egui_wgpu::wgpu::Device,
        queue: &egui_wgpu::wgpu::Queue,
        ctx: &egui::Context,
    ) {
        use wasm_bindgen_futures::spawn_local;

        if !self.in_flight.is_empty() {
            return;
        }
        let Some(index) = self.next_missing() else { return };
        let hash = self.items[index].hash.clone();
        self.in_flight.insert(hash.clone());

        let config = self.items[index].config.clone();
        let results = Rc::clone(&self.async_results);
        // wgpu's Device and Queue are internally Arc-wrapped, so these
        // clones are cheap.
        let device = device.clone();
        let queue = queue.clone();
        let ctx = ctx.clone();
        spawn_local(async move {
            let image = crate::renderer::render_preview_async(&device, &queue, &config).await;
            results.borrow_mut().insert(hash, image);
            // Drawn again, the gallery takes it and starts the next.
            ctx.request_repaint();
        });
    }

    /// Render the gallery UI
    /// Returns GalleryResponse with selected config if any
    pub fn render(&mut self, ui: &mut egui::Ui) -> GalleryResponse {
        // WASM: Poll for completed async thumbnails and upload them
        #[cfg(target_arch = "wasm32")]
        self.poll_async_thumbnails(ui.ctx());

        self.shown = true;
        self.visible_missing.clear();

        // Render toolbar
        self.render_toolbar(ui);

        let left = self.missing_count();
        if left > 0 {
            ui.weak(t!("fractal_gallery.previews_left", count = left));
        }

        ui.separator();

        // Render gallery based on view mode
        match self.view_mode {
            GalleryViewMode::Grid => self.render_grid(ui),
            GalleryViewMode::List => self.render_list(ui),
        }
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

    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        // Wrapped so a narrow panel folds this onto several lines
        // instead of imposing a minimum width. Widget widths are sized
        // against the PANEL width, read before the row starts — sizing
        // them to "whatever is left on the current line" (the first
        // attempt) made every widget fit-by-construction, so nothing
        // ever wrapped and the last widget rendered crammed or clipped
        // at the row's edge.
        // Explicit narrow/wide layouts, decided from the measured panel
        // width — NOT emergent wrapping. Two attempts at
        // horizontal_wrapped both failed mechanically: bare label +
        // widget siblings wrap independently (the slider was placed
        // after its label without its own width ever joining the wrap
        // decision), and nesting the pair in a child `horizontal` is
        // worse — a child ui is placed at the cursor and only sized
        // AFTER its contents run, so the wrapped parent cannot wrap it
        // at all. Measuring once and choosing a layout is deterministic
        // and immune to both.
        let panel_width = ui.available_width();
        let narrow = panel_width < 480.0;

        let search_width = (panel_width - 90.0).clamp(60.0, 150.0);
        let rail_width = (panel_width - 110.0).clamp(50.0, 100.0);

        let view_toggle = |ui: &mut egui::Ui, view_mode: &mut GalleryViewMode| {
            ui.selectable_value(view_mode, GalleryViewMode::Grid, t!("fractal_gallery.view_grid"));
            ui.selectable_value(view_mode, GalleryViewMode::List, t!("fractal_gallery.view_list"));
        };
        let search_row = |ui: &mut egui::Ui, search_query: &mut String| {
            ui.label(t!("fractal_gallery.search"));
            let r = ui.add(
                egui::TextEdit::singleline(search_query)
                    .desired_width(search_width)
                    .hint_text(t!("fractal_gallery.search_hint")),
            );
            super::vkb_sync(ui, &r, search_query);
            if !search_query.is_empty() && ui.button(t!("fractal_gallery.clear_search")).clicked() {
                search_query.clear();
            }
        };
        let size_row = |ui: &mut egui::Ui, thumbnail_size: &mut f32| {
            ui.label(t!("fractal_gallery.size"));
            ui.spacing_mut().slider_width = rail_width;
            ui.add(super::VkbSlider::new(thumbnail_size, 64.0..=256.0).show_value(false));
        };

        if narrow {
            ui.horizontal(|ui| view_toggle(ui, &mut self.view_mode));
            ui.horizontal(|ui| search_row(ui, &mut self.search_query));
            if self.view_mode == GalleryViewMode::Grid {
                ui.horizontal(|ui| size_row(ui, &mut self.thumbnail_size));
            }
        } else {
            let show_size = self.view_mode == GalleryViewMode::Grid;
            ui.horizontal(|ui| {
                view_toggle(ui, &mut self.view_mode);
                ui.separator();
                search_row(ui, &mut self.search_query);
                if show_size {
                    ui.separator();
                    size_row(ui, &mut self.thumbnail_size);
                }
            });
        }
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

        // Cap the card to the panel: on a panel narrower than the
        // chosen thumbnail size, shrink the cards to fit rather than
        // clipping their right edge (a wrapped row cannot wrap its
        // FIRST item, so an oversized card clips silently). Read once,
        // before the row starts, so every card this frame agrees.
        let card_cap = (ui.available_width() - 12.0).max(64.0);

        ui.horizontal_wrapped(|ui| {
            for (original_index, hash, name, config) in &filtered {
                let card_response = self.render_grid_card(ui, *original_index, hash, name, card_cap);
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
        index: usize,
        hash: &str,
        name: &str,
        card_cap: f32,
    ) -> egui::Response {
        let side = self.thumbnail_size.min(card_cap);
        let card_size = Vec2::new(side, side + 24.0);

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

            // Thumbnail area (side, not thumbnail_size: the card may
            // have been capped to fit a narrow panel)
            let img_rect = egui::Rect::from_min_size(
                rect.min,
                Vec2::new(side, side),
            );

            // Try to get texture, or load from disk cache (a file that
            // has gone is forgotten by the load, and queued next frame)
            if !self.texture_cache.contains(hash) {
                let cached = shared_thumbnail_cache().load(hash);
                if let Some(image) = cached {
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
                // Placeholder, until its preview is made: on screen, it
                // is made first.
                self.visible_missing.push(index);
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
                Vec2::new(side, 20.0),
            );

            // Truncate name if too long (by characters: names are any script)
            let max_chars = (side / 8.0) as usize;
            let display_name = super::ellipsize(name, max_chars);

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
                ui.label(list_summary(config));
            })
            .body(|ui| {
                // Expanded details
                for line in list_details(config) {
                    ui.label(line);
                }

                // Small thumbnail if available; open without one, it is
                // made first.
                if let Some(texture) = self.texture_cache.get(hash) {
                    ui.image((texture.id(), Vec2::new(64.0, 64.0)));
                } else {
                    self.visible_missing.push(*original_index);
                }
            });

            ui.separator();
        }

        response
    }

    fn format_variations(config: &FractalConfig) -> String {
        let registry = crate::variations::global_registry();
        // What the user calls each: a carried Rhai source's title where
        // this session has not registered it.
        let label = |name: &String| match config.flame.custom_variations.iter().find(|s| &s.entry == name) {
            Some(s) if registry.get(name).is_none() => {
                crate::lang::rhai::source_labels(&s.source).map(|l| l.title.resolve()).unwrap_or_else(|| name.clone())
            }
            _ => registry.label(name),
        };
        let mut variations: Vec<String> = config
            .flame
            .transforms
            .iter()
            .flat_map(|t| t.variations.keys().map(label))
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
    }
}

impl Default for FractalConfigGallery {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

/// The list view's one-line summary of a config.
///
/// What describes a config depends on the engine: a flame by its
/// transforms and variations, an escape-time fractal by its formula and
/// coloring, a simulation by its model and colouring. The flame columns
/// on the other two read "1 transforms, ..., linear" -- the default
/// flame they carry and never render.
fn list_summary(config: &FractalConfig) -> String {
    use crate::scene::transforms::RenderMode;
    let render_mode = t!(match config.render_mode {
        RenderMode::TwoD => "fractal_gallery.render_mode_2d",
        RenderMode::ThreeD => "fractal_gallery.render_mode_3d",
        RenderMode::Escape => "fractal_gallery.render_mode_escape",
        RenderMode::Simulation => "fractal_gallery.render_mode_simulation",
    });
    match config.render_mode {
        RenderMode::TwoD | RenderMode::ThreeD => format!(
            "{}, {}, {}",
            t!("fractal_gallery.transforms_count", count = config.flame.transforms.len()),
            render_mode,
            FractalConfigGallery::format_variations(config),
        ),
        RenderMode::Escape => format!(
            "{}, {}, {}",
            render_mode,
            super::escape_panel::formula_display_name(&config.escape.formula),
            super::escape_panel::coloring_display_name(&config.escape),
        ),
        RenderMode::Simulation => format!(
            "{}, {}, {}",
            render_mode,
            crate::sim::model_or_default(&config.sim.model).display_name,
            crate::sim::coloring_or_default(&config.sim.coloring).display_name,
        ),
    }
}

/// The list view's expanded details, by engine as [`list_summary`] is.
fn list_details(config: &FractalConfig) -> Vec<String> {
    use crate::scene::transforms::RenderMode;
    match config.render_mode {
        RenderMode::TwoD | RenderMode::ThreeD => vec![
            t!("fractal_gallery.zoom", value = format!("{:.2}", config.zoom)).to_string(),
            t!("fractal_gallery.max_iterations", value = config.max_iterations).to_string(),
            t!("fractal_gallery.color_mode", value = format!("{:?}", config.color_mode)).to_string(),
        ],
        // The escape zoom is a log2 exponent that deep zooms take past
        // anything a plain number can show, so it is shown as one.
        RenderMode::Escape => vec![
            t!("fractal_gallery.zoom", value = format!("10^{:.2}", config.escape.zoom_log10())).to_string(),
            t!("fractal_gallery.max_iterations", value = config.escape.max_iter).to_string(),
        ],
        RenderMode::Simulation => vec![
            t!("fractal_gallery.steps", value = config.sim.steps).to_string(),
        ],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::transforms::RenderMode;

    /// The next preview made is a card on screen without one, in the
    /// order they were drawn, then the rest of the gallery in its order;
    /// and a gallery not drawn since the app last asked makes none.
    #[test]
    fn the_next_preview_is_one_on_screen_then_the_gallerys_order() {
        let mut g = FractalConfigGallery::new(vec![FractalConfig::default(); 5]);
        let missing = |set: &'static [usize]| move |i: usize| set.contains(&i);
        g.visible_missing = vec![3, 1];
        assert_eq!(g.next_of(missing(&[0, 1, 3, 4])), Some(3), "on screen first");
        assert_eq!(g.next_of(missing(&[0, 1, 4])), Some(1), "then the next on screen");
        assert_eq!(g.next_of(missing(&[0, 4])), Some(0), "then the gallery's order");
        assert_eq!(g.next_of(missing(&[])), None);

        assert!(!g.take_shown(), "never drawn");
        g.shown = true;
        assert!(g.take_shown());
        assert!(!g.take_shown(), "drawn once, asked once");
    }

    #[test]
    fn an_escape_row_describes_its_formula_not_the_flame_it_carries() {
        let mut config = FractalConfig::default();
        config.render_mode = RenderMode::Escape;
        config.escape.zoom_log2 = 10.5;
        config.escape.max_iter = 777;
        let summary = list_summary(&config);
        assert!(!summary.contains("transforms"), "{summary}");
        assert!(
            summary.contains(super::super::escape_panel::formula_display_name(&config.escape.formula)),
            "{summary}"
        );
        let details = list_details(&config).join("\n");
        // Base 10, as the View panel shows it: 2^10.5 is 10^3.16.
        assert!(details.contains("10^3.16"), "{details}");
        assert!(details.contains("777"), "{details}");
        assert!(!details.contains("Color Mode"), "{details}");
    }

    #[test]
    fn a_simulation_row_describes_its_model_and_steps() {
        let mut config = FractalConfig::default();
        config.render_mode = RenderMode::Simulation;
        config.sim.steps = 1234;
        let summary = list_summary(&config);
        assert!(!summary.contains("transforms"), "{summary}");
        assert!(
            summary.contains(crate::sim::model_or_default(&config.sim.model).display_name),
            "{summary}"
        );
        let details = list_details(&config).join("\n");
        assert!(details.contains("1234"), "{details}");
        assert!(!details.contains("Max Iterations"), "{details}");
    }

    #[test]
    fn a_flame_row_is_unchanged() {
        let config = FractalConfig::default();
        assert!(list_summary(&config).contains("transforms"));
        assert_eq!(list_details(&config).len(), 3);
    }
}
