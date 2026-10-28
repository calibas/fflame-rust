//! Fractal Browser panel - unified browser for presets, batch results, files, and online flames
//!
//! A tabbed panel that provides views:
//! - Presets: Built-in presets from assets/presets.fflame, a gallery per
//!   category ([`PresetCategory`])
//! - Batch: Random batch generation results (persists until next batch)
//! - Files: Loaded .fflame files
//! - Online: Flames fetched from the API (feature-gated behind `api`)

use egui;
use rust_i18n::t;

use super::fractal_gallery::{FractalConfigGallery, GalleryResponse};
use crate::config::FractalConfig;
use crate::scene::presets::global_preset_library;

use std::sync::{Arc, Mutex};

/// Which tab is currently active in the browser
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BrowserTab {
    #[default]
    Presets,
    Batch,
    Files,
    Online,
}

/// **The Presets tab's categories**, each its own gallery and tab. A
/// gallery makes every thumbnail it is missing before it shows any, and
/// makes them only once it is shown, so a category's are made when it is
/// first opened -- not every preset's at once, path-traced solids among
/// them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PresetCategory {
    #[default]
    Flame2d,
    Flame3d,
    Escape2d,
    Escape3d,
    Simulation,
}

impl PresetCategory {
    pub const ALL: [PresetCategory; 5] =
        [Self::Flame2d, Self::Flame3d, Self::Escape2d, Self::Escape3d, Self::Simulation];

    /// The category a config falls in: by the camera its picture is seen
    /// through (`camera::gesture::view_kind`), an escape solid or terrain
    /// being a 3D escape picture. A Rhai solid is told from its source,
    /// which is not compiled yet when the presets are sorted, so the
    /// solids' registry does not know it.
    pub fn of(config: &FractalConfig) -> Self {
        use crate::camera::gesture::{view_kind, ViewKind};
        match view_kind(config) {
            ViewKind::Flame2d => Self::Flame2d,
            ViewKind::Flame3d => Self::Flame3d,
            ViewKind::EscapePlane if !runs_a_rhai_solid(config) => Self::Escape2d,
            ViewKind::EscapePlane | ViewKind::Solid | ViewKind::EscapeTerrain => Self::Escape3d,
            ViewKind::Sim2d | ViewKind::SimTerrain => Self::Simulation,
        }
    }

    fn label_key(self) -> &'static str {
        match self {
            Self::Flame2d => "browser.presets_flame_2d",
            Self::Flame3d => "browser.presets_flame_3d",
            Self::Escape2d => "browser.presets_escape_2d",
            Self::Escape3d => "browser.presets_escape_3d",
            Self::Simulation => "browser.presets_simulation",
        }
    }

    fn index(self) -> usize {
        self as usize
    }
}

/// Whether the config's formula is its own Rhai source, and that source a
/// solid. The source stays in the config when another formula is picked
/// (undo finds it), so the formula's name has to be a custom one too.
fn runs_a_rhai_solid(config: &FractalConfig) -> bool {
    #[cfg(feature = "engine-escape")]
    {
        use crate::config::escape::CustomDialect;
        use crate::escape::lang::{rhai, EntryKind};
        let escape = &config.escape;
        escape.formula.starts_with("custom-")
            && escape
                .custom_formula
                .as_ref()
                .is_some_and(|src| src.dialect == CustomDialect::Rhai && rhai::kind_of(&src.source) == Some(EntryKind::Solid))
    }
    #[cfg(not(feature = "engine-escape"))]
    {
        let _ = config;
        false
    }
}

/// Which gallery a thumbnail is made for (desktop).
#[cfg(not(target_arch = "wasm32"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GalleryId {
    Preset(PresetCategory),
    Batch,
    Files,
}

/// The Online tab's render-mode filter: the value sent as
/// `render_mode`, and the label key.
///
/// No Simulation entry: the server's `render_mode` enum does not know
/// that value yet (see [`ApiRenderMode::Simulation`]), so filtering by
/// it would fail the whole search rather than return nothing.
///
/// [`ApiRenderMode::Simulation`]: crate::api::types::ApiRenderMode::Simulation
const ONLINE_RENDER_MODE_FILTERS: &[(Option<crate::api::types::ApiRenderMode>, &str)] = &[
    (None, "browser.search_all"),
    (Some(crate::api::types::ApiRenderMode::TwoD), "browser.search_2d"),
    (Some(crate::api::types::ApiRenderMode::ThreeD), "browser.search_3d"),
    (Some(crate::api::types::ApiRenderMode::Escape), "mode.escape"),
];

/// One row of the Online tab: the name, the kind of fractal, and for a
/// flame its transform count and variations.
///
/// Escape and simulation configs legitimately have no transforms and no
/// variations -- the formula or the model IS the fractal -- so for them
/// those columns would read "0T | " and look like a broken row rather
/// than a different kind of fractal.
fn online_row_label(flame: &crate::api::types::FlameListItem) -> String {
    use crate::api::types::ApiRenderMode as M;
    let render_mode = t!(match flame.render_mode {
        M::TwoD => "fractal_gallery.render_mode_2d",
        M::ThreeD => "fractal_gallery.render_mode_3d",
        M::Escape => "fractal_gallery.render_mode_escape",
        M::Simulation => "fractal_gallery.render_mode_simulation",
    });
    match flame.render_mode {
        M::Escape | M::Simulation => format!("{} — {}", flame.name, render_mode),
        M::TwoD | M::ThreeD => {
            let variations_summary = if flame.variation_names.len() <= 3 {
                flame.variation_names.join(", ")
            } else {
                format!(
                    "{}, +{}",
                    flame.variation_names[..2].join(", "),
                    flame.variation_names.len() - 2
                )
            };
            format!(
                "{} — {} | {}T | {}",
                flame.name, render_mode, flame.transform_count, variations_summary
            )
        }
    }
}

/// State for the Fractal Browser panel
pub struct FractalBrowserPanel {
    /// Currently active tab
    current_tab: BrowserTab,

    // --- Presets tab ---
    /// A gallery a category, in [`PresetCategory::ALL`] order.
    preset_galleries: Vec<FractalConfigGallery>,
    /// The category shown.
    preset_category: PresetCategory,
    /// WASM: the categories whose thumbnails are not started yet. Each is
    /// started when it is first shown.
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    presets_unstarted: [bool; 5],

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

    // --- Online tab ---
    /// Cached flame list items from the API
    online_flames: Vec<crate::api::types::FlameListItem>,
    /// Whether the flame list has been fetched
    online_fetched: bool,
    /// Whether a flame list fetch is in progress
    online_loading: bool,
    /// Error from last fetch
    online_error: Option<String>,
    /// Shared slot for receiving async flame list results
    online_list_result: Arc<Mutex<Option<Result<Vec<crate::api::types::FlameListItem>, String>>>>,
    /// Whether we're loading a specific flame's full config
    online_loading_flame: bool,
    /// Shared slot for receiving async flame config result (config, flame_id, is_public)
    online_flame_result: Arc<Mutex<Option<Result<OnlineFlameResult, String>>>>,
    /// Whether a delete is in progress
    online_deleting: bool,
    /// Shared slot for receiving async delete result (flame_name on success)
    online_delete_result: Arc<Mutex<Option<Result<String, String>>>>,
    /// A delete the user asked for and has not yet confirmed: (id, name).
    /// The X sits beside the row's load button and the delete cannot be
    /// undone, so it goes through a confirmation as the palette
    /// library's does.
    online_pending_delete: Option<(String, String)>,
    /// Search filter: name text
    online_search_name: String,
    /// Search filter: render mode (`None` = All). One of
    /// [`ONLINE_RENDER_MODE_FILTERS`].
    online_search_render_mode: Option<crate::api::types::ApiRenderMode>,

    /// Cached auth credentials for trigger methods (base_url, token) — set each frame from render()
    auth_credentials: Option<(String, String)>,
}

impl Default for FractalBrowserPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl FractalBrowserPanel {
    /// Create a new fractal browser panel
    pub fn new() -> Self {
        // The presets from the global library, sorted into their
        // categories in the library's order.
        let preset_library = global_preset_library();
        let mut by_category: Vec<Vec<FractalConfig>> = vec![Vec::new(); PresetCategory::ALL.len()];
        for preset in preset_library.presets() {
            by_category[PresetCategory::of(preset).index()].push(preset.clone());
        }
        // Open on the first category that has any.
        let preset_category = PresetCategory::ALL.into_iter().find(|c| !by_category[c.index()].is_empty()).unwrap_or_default();

        Self {
            current_tab: BrowserTab::Presets,

            preset_galleries: by_category.into_iter().map(FractalConfigGallery::new).collect(),
            preset_category,
            presets_unstarted: [true; 5],

            batch_gallery: FractalConfigGallery::default(),
            batch_needs_generation: false,

            files_gallery: FractalConfigGallery::default(),
            files_current_path: None,
            files_error_message: None,
            files_open_requested: false,

            online_flames: Vec::new(),
            online_fetched: false,
            online_loading: false,
            online_error: None,
            online_list_result: Arc::new(Mutex::new(None)),
            online_loading_flame: false,
            online_flame_result: Arc::new(Mutex::new(None)),
            online_deleting: false,
            online_delete_result: Arc::new(Mutex::new(None)),
            online_pending_delete: None,
            online_search_name: String::new(),
            online_search_render_mode: None,

            auth_credentials: None,
        }
    }

    /// Get the current tab
    #[cfg(test)]
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

    /// Clear online tab data (used when signing out).
    /// Keeps `online_fetched = true` so the auto-fetch doesn't re-trigger
    /// and hammer the API with 401s. User must sign in + click Refresh.
    pub fn clear_online_data(&mut self) {
        self.online_flames.clear();
        self.online_fetched = true; // Don't auto-retry after sign-out
        self.online_loading = false;
        self.online_error = None;
        self.online_search_name.clear();
        self.online_search_render_mode = None;
    }

    // ========== Thumbnail generation (desktop) ==========

    /// The gallery `id` names.
    #[cfg(not(target_arch = "wasm32"))]
    fn gallery(&self, id: GalleryId) -> &FractalConfigGallery {
        match id {
            GalleryId::Preset(c) => &self.preset_galleries[c.index()],
            GalleryId::Batch => &self.batch_gallery,
            GalleryId::Files => &self.files_gallery,
        }
    }

    /// Check if any gallery is generating thumbnails (desktop only). A
    /// gallery queues its missing thumbnails only once shown, so a preset
    /// category never opened is not generating.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn is_generating(&self) -> bool {
        self.preset_galleries.iter().any(|g| g.is_generating())
            || self.batch_gallery.is_generating()
            || self.files_gallery.is_generating()
    }

    /// Get the next config that needs thumbnail generation (desktop only):
    /// the shown gallery's first, then any other's that was shown and is
    /// not finished.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn next_pending_config(&self) -> Option<(GalleryId, &FractalConfig)> {
        let shown = match self.current_tab {
            BrowserTab::Presets => Some(GalleryId::Preset(self.preset_category)),
            BrowserTab::Batch => Some(GalleryId::Batch),
            BrowserTab::Files => Some(GalleryId::Files),
            // The Online tab doesn't use FractalConfigGallery for thumbnails.
            BrowserTab::Online => None,
        };
        let rest = PresetCategory::ALL.into_iter().map(GalleryId::Preset).chain([GalleryId::Batch, GalleryId::Files]);
        shown
            .into_iter()
            .chain(rest)
            .find_map(|id| self.gallery(id).next_pending_config().map(|config| (id, config)))
    }

    /// Generate one thumbnail for the specified gallery (desktop only)
    #[cfg(not(target_arch = "wasm32"))]
    pub fn generate_one_thumbnail<F>(
        &mut self,
        id: GalleryId,
        ctx: &egui::Context,
        render_fn: F,
    ) -> bool
    where
        F: FnOnce(&FractalConfig) -> image::RgbaImage,
    {
        match id {
            GalleryId::Preset(c) => self.preset_galleries[c.index()].generate_one_thumbnail(ctx, render_fn),
            GalleryId::Batch => self.batch_gallery.generate_one_thumbnail(ctx, render_fn),
            GalleryId::Files => self.files_gallery.generate_one_thumbnail(ctx, render_fn),
        }
    }

    /// WASM: Start async thumbnail generation for all galleries
    #[cfg(target_arch = "wasm32")]
    pub fn start_async_thumbnails(
        &mut self,
        device: &egui_wgpu::wgpu::Device,
        queue: &egui_wgpu::wgpu::Queue,
    ) {
        // A preset category's, the first time it is shown.
        let shown = self.preset_category.index();
        if self.current_tab == BrowserTab::Presets && self.presets_unstarted[shown] {
            self.preset_galleries[shown].start_async_thumbnail_generation(device, queue);
            self.presets_unstarted[shown] = false;
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

    // ========== Rendering ==========

    /// Render the panel
    /// `auth` is `Some((base_url, token))` when signed in, `None` otherwise.
    pub fn render(&mut self, ui: &mut egui::Ui, online_mode: bool, auth: Option<(&str, &str)>) -> GalleryResponse {
        // Cache auth credentials for use by trigger methods
        self.auth_credentials = auth.map(|(b, t)| (b.to_string(), t.to_string()));

        // Tab bar. Wrapped: four tabs with counts exceed a phone-width
        // panel, and an unwrappable row sets the panel's minimum width.
        ui.horizontal_wrapped(|ui| {
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

            // Online tab (only shown when online mode is enabled)
            if online_mode {
                let online_label = if self.online_flames.is_empty() {
                    t!("browser.tab_online").to_string()
                } else {
                    format!("{} ({})", t!("browser.tab_online"), self.online_flames.len())
                };
                if ui
                    .selectable_label(self.current_tab == BrowserTab::Online, online_label)
                    .clicked()
                {
                    self.current_tab = BrowserTab::Online;
                }
            }
        });

        ui.separator();

        // Render current tab content
        match self.current_tab {
            BrowserTab::Presets => self.render_presets_tab(ui),
            BrowserTab::Batch => self.render_batch_tab(ui),
            BrowserTab::Files => self.render_files_tab(ui),
            BrowserTab::Online => self.render_online_tab(ui),
        }
    }

    fn render_presets_tab(&mut self, ui: &mut egui::Ui) -> GalleryResponse {
        // A tab a category that has presets, with its count. Wrapped, as
        // the browser's own tabs are, for a phone-width panel.
        ui.horizontal_wrapped(|ui| {
            for category in PresetCategory::ALL {
                let count = self.preset_galleries[category.index()].len();
                if count == 0 {
                    continue;
                }
                let label = format!("{} ({count})", t!(category.label_key()));
                if ui.selectable_label(self.preset_category == category, label).clicked() {
                    self.preset_category = category;
                }
            }
        });
        ui.separator();
        self.preset_galleries[self.preset_category.index()].render(ui)
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

    /// Render the Online tab — fetches flame list from API, loads full config on click
    fn render_online_tab(&mut self, ui: &mut egui::Ui) -> GalleryResponse {
        let mut response = GalleryResponse::default();

        // Poll for completed async flame list fetch
        if self.online_loading {
            if let Ok(mut slot) = self.online_list_result.lock() {
                if let Some(result) = slot.take() {
                    self.online_loading = false;
                    self.online_fetched = true; // Mark as fetched even on error (don't auto-retry)
                    match result {
                        Ok(flames) => {
                            self.online_flames = flames;
                            self.online_error = None;
                        }
                        Err(e) => {
                            if is_auth_error(&e) {
                                response.session_expired = true;
                            }
                            self.online_error = Some(e);
                        }
                    }
                }
            }
        }

        // Poll for completed async single-flame load
        if self.online_loading_flame {
            if let Ok(mut slot) = self.online_flame_result.lock() {
                if let Some(result) = slot.take() {
                    self.online_loading_flame = false;
                    match result {
                        Ok(flame_result) => {
                            response.selected = Some(flame_result.config);
                            response.api_flame_id = Some(flame_result.flame_id);
                            response.api_flame_is_public = flame_result.is_public;
                            response.api_flame_user_id = Some(flame_result.user_id);
                            response.api_flame_animation_count = flame_result.animation_count;
                            response.api_flame_animations = flame_result.animations;
                        }
                        Err(e) => {
                            if is_auth_error(&e) {
                                response.session_expired = true;
                            }
                            self.online_error = Some(e);
                        }
                    }
                }
            }
        }

        // Poll for completed async delete
        if self.online_deleting {
            if let Ok(mut slot) = self.online_delete_result.lock() {
                if let Some(result) = slot.take() {
                    self.online_deleting = false;
                    match result {
                        Ok(name) => {
                            response.api_notification = Some((
                                rust_i18n::t!("api.delete_success", name = name).to_string(),
                                false,
                            ));
                            // Refresh the list
                            self.online_fetched = false;
                        }
                        Err(e) => {
                            if is_auth_error(&e) {
                                response.session_expired = true;
                            }
                            response.api_notification = Some((
                                rust_i18n::t!("api.delete_error", error = e).to_string(),
                                true,
                            ));
                        }
                    }
                }
            }
        }

        // Auto-fetch on first visit (only if signed in — avoids 401 loop after sign-out)
        if !self.online_fetched && !self.online_loading && self.auth_credentials.is_some() {
            self.trigger_fetch_flames();
        }

        // Toolbar
        ui.horizontal(|ui| {
            let refresh_enabled = !self.online_loading && !self.online_loading_flame && !self.online_deleting;
            if ui
                .add_enabled(refresh_enabled, egui::Button::new(t!("browser.online_refresh")))
                .clicked()
            {
                self.trigger_fetch_flames();
            }

            if self.online_loading {
                ui.spinner();
                ui.label(t!("browser.online_loading_list"));
            } else if self.online_loading_flame {
                ui.spinner();
                ui.label(t!("browser.online_loading_flame"));
            } else if self.online_deleting {
                ui.spinner();
                ui.label(t!("browser.online_deleting"));
            }
        });

        // Search / Filter bar
        let mut search_triggered = false;
        ui.horizontal(|ui| {
            let text_response = ui.add(
                egui::TextEdit::singleline(&mut self.online_search_name)
                    .hint_text(t!("browser.search_placeholder"))
                    .desired_width(150.0),
            );
            super::vkb_sync(ui, &text_response, &self.online_search_name);
            if text_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                search_triggered = true;
            }

            ui.separator();

            for &(mode, key) in ONLINE_RENDER_MODE_FILTERS {
                if ui.selectable_label(self.online_search_render_mode == mode, t!(key).as_ref()).clicked() {
                    self.online_search_render_mode = mode;
                }
            }

            ui.separator();

            let search_enabled = !self.online_loading && !self.online_loading_flame && !self.online_deleting;
            if ui.add_enabled(search_enabled, egui::Button::new(t!("browser.search_button"))).clicked() {
                search_triggered = true;
            }

            let has_filters = !self.online_search_name.is_empty() || self.online_search_render_mode.is_some();
            if has_filters {
                if ui.add_enabled(search_enabled, egui::Button::new(t!("browser.search_clear"))).clicked() {
                    self.online_search_name.clear();
                    self.online_search_render_mode = None;
                    search_triggered = true;
                }
            }
        });

        if search_triggered {
            self.trigger_fetch_flames();
        }

        // Error message
        if let Some(ref error) = self.online_error {
            ui.colored_label(egui::Color32::RED, format!("{}", error));
        }

        ui.separator();

        // Flame list
        if self.online_flames.is_empty() && self.online_fetched {
            ui.centered_and_justified(|ui| {
                ui.vertical_centered(|ui| {
                    ui.add_space(50.0);
                    ui.label(t!("browser.online_empty"));
                    ui.add_space(10.0);
                    ui.label(t!("browser.online_empty_hint"));
                });
            });
        } else if !self.online_flames.is_empty() {
            // Clone the list to avoid borrow issues during iteration
            let flames: Vec<_> = self.online_flames.clone();
            let is_busy = self.online_loading_flame || self.online_deleting;
            let mut delete_flame: Option<(String, String)> = None;

            for flame in &flames {
                let label = online_row_label(flame);

                ui.horizontal(|ui| {
                    // Load button (main flame row)
                    let button = ui.add_enabled(
                        !is_busy,
                        egui::Button::new(&label).wrap_mode(egui::TextWrapMode::Truncate),
                    );
                    if button.clicked() {
                        self.trigger_load_flame(&flame.id);
                    }

                    // Delete button (small, red)
                    let del_btn = ui.add_enabled(
                        !is_busy,
                        egui::Button::new(
                            egui::RichText::new("X")
                                .color(egui::Color32::from_rgb(220, 80, 80))
                                .small(),
                        ),
                    );
                    if del_btn.clicked() {
                        delete_flame = Some((flame.id.clone(), flame.name.clone()));
                    }
                    del_btn.on_hover_text(t!("browser.online_delete"));
                });
            }

            // Ask first; the delete runs from the confirmation below.
            if delete_flame.is_some() {
                self.online_pending_delete = delete_flame;
            }
        }

        // Delete confirmation dialog
        if let Some((id, name)) = self.online_pending_delete.clone() {
            let mut confirmed = false;
            let mut cancelled = false;
            egui::Window::new(t!("browser.online_delete_title"))
                .collapsible(false)
                .resizable(false)
                .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
                .show(ui.ctx(), |ui| {
                    ui.label(t!("browser.online_delete_message", name = name));

                    ui.separator();

                    ui.horizontal(|ui| {
                        if ui.button(t!("browser.online_delete_confirm")).clicked() {
                            confirmed = true;
                        }
                        if ui.button(t!("browser.online_delete_cancel")).clicked() {
                            cancelled = true;
                        }
                    });
                });
            if confirmed {
                self.online_pending_delete = None;
                self.trigger_delete_flame(&id, &name);
            } else if cancelled {
                self.online_pending_delete = None;
            }
        }

        response
    }

    /// Trigger async fetch of the flame list from the API.
    /// Uses search_flames() if any filters are active, otherwise list_my_flames().
    fn trigger_fetch_flames(&mut self) {
        self.online_loading = true;
        self.online_error = None;

        let has_filters = !self.online_search_name.is_empty() || self.online_search_render_mode.is_some();

        // Get credentials (WASM: from localStorage, Desktop: from cached auth)
        #[cfg(target_arch = "wasm32")]
        let credentials = get_wasm_credentials();
        #[cfg(not(target_arch = "wasm32"))]
        let credentials = self.auth_credentials.clone().map(|(b, t)| Ok((b, t))).unwrap_or(Err("Not signed in".to_string()));

        let (base_url, token) = match credentials {
            Ok(creds) => creds,
            Err(e) => {
                self.online_loading = false;
                self.online_error = Some(e);
                return;
            }
        };

        let result_slot = self.online_list_result.clone();

        if has_filters {
            let name = if self.online_search_name.is_empty() {
                None
            } else {
                Some(self.online_search_name.clone())
            };
            let render_mode = self.online_search_render_mode;
            #[cfg(target_arch = "wasm32")]
            wasm_bindgen_futures::spawn_local(async move {
                let result = search_online_flames(&base_url, &token, name, render_mode).await;
                if let Ok(mut slot) = result_slot.lock() { *slot = Some(result); }
            });
            #[cfg(not(target_arch = "wasm32"))]
            std::thread::spawn(move || {
                let result = pollster::block_on(search_online_flames(&base_url, &token, name, render_mode));
                if let Ok(mut slot) = result_slot.lock() { *slot = Some(result); }
            });
        } else {
            #[cfg(target_arch = "wasm32")]
            wasm_bindgen_futures::spawn_local(async move {
                let result = fetch_online_flames(&base_url, &token).await;
                if let Ok(mut slot) = result_slot.lock() { *slot = Some(result); }
            });
            #[cfg(not(target_arch = "wasm32"))]
            std::thread::spawn(move || {
                let result = pollster::block_on(fetch_online_flames(&base_url, &token));
                if let Ok(mut slot) = result_slot.lock() { *slot = Some(result); }
            });
        }
    }

    /// Trigger async load of a single flame's full config
    fn trigger_load_flame(&mut self, flame_id: &str) {
        self.online_loading_flame = true;
        self.online_error = None;

        // Get credentials (WASM: from localStorage, Desktop: from cached auth)
        #[cfg(target_arch = "wasm32")]
        let credentials = get_wasm_credentials();
        #[cfg(not(target_arch = "wasm32"))]
        let credentials = self.auth_credentials.clone().map(|(b, t)| Ok((b, t))).unwrap_or(Err("Not signed in".to_string()));

        let (base_url, token) = match credentials {
            Ok(creds) => creds,
            Err(e) => {
                self.online_loading_flame = false;
                self.online_error = Some(e);
                return;
            }
        };

        let result_slot = self.online_flame_result.clone();
        let id = flame_id.to_string();

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            let result = fetch_online_flame(&base_url, &token, &id).await;
            if let Ok(mut slot) = result_slot.lock() { *slot = Some(result); }
        });
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let result = pollster::block_on(fetch_online_flame(&base_url, &token, &id));
            if let Ok(mut slot) = result_slot.lock() { *slot = Some(result); }
        });
    }

    /// Request a refresh of the online flame list (e.g., after save/delete)
    pub fn request_online_refresh(&mut self) {
        self.online_fetched = false;
    }

    /// Trigger async delete of a flame
    fn trigger_delete_flame(&mut self, flame_id: &str, flame_name: &str) {
        self.online_deleting = true;
        self.online_error = None;

        // Get credentials (WASM: from localStorage, Desktop: from cached auth)
        #[cfg(target_arch = "wasm32")]
        let credentials = get_wasm_credentials();
        #[cfg(not(target_arch = "wasm32"))]
        let credentials = self.auth_credentials.clone().map(|(b, t)| Ok((b, t))).unwrap_or(Err("Not signed in".to_string()));

        let (base_url, token) = match credentials {
            Ok(creds) => creds,
            Err(e) => {
                self.online_deleting = false;
                self.online_error = Some(e);
                return;
            }
        };

        let result_slot = self.online_delete_result.clone();
        let id = flame_id.to_string();
        let name = flame_name.to_string();

        #[cfg(target_arch = "wasm32")]
        wasm_bindgen_futures::spawn_local(async move {
            let result = delete_online_flame(&base_url, &token, &id).await.map(|_| name);
            if let Ok(mut slot) = result_slot.lock() { *slot = Some(result); }
        });
        #[cfg(not(target_arch = "wasm32"))]
        std::thread::spawn(move || {
            let result = pollster::block_on(delete_online_flame(&base_url, &token, &id)).map(|_| name);
            if let Ok(mut slot) = result_slot.lock() { *slot = Some(result); }
        });
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

/// Get API credentials for WASM. Auth is handled via cookies.
/// Returns (base_url, token) where token is empty.
#[cfg(target_arch = "wasm32")]
fn get_wasm_credentials() -> Result<(String, String), String> {
    Ok((crate::api::API_BASE_URL.to_string(), String::new()))
}

/// Fetch the list of flames from the API (cross-platform async helper)
async fn fetch_online_flames(base_url: &str, token: &str) -> Result<Vec<crate::api::types::FlameListItem>, String> {
    let mut api = crate::api::ApiState::new(base_url);
    api.set_token(token);
    api.list_my_flames(1, 100)
        .await
        .map_err(|e| e.to_string())
}

/// Search flames from the API with filters (cross-platform async helper)
async fn search_online_flames(
    base_url: &str,
    token: &str,
    name: Option<String>,
    render_mode: Option<crate::api::types::ApiRenderMode>,
) -> Result<Vec<crate::api::types::FlameListItem>, String> {
    let mut api = crate::api::ApiState::new(base_url);
    api.set_token(token);

    let params = crate::api::types::SearchFlamesParams {
        name,
        render_mode,
        per_page: Some(100),
        ..Default::default()
    };
    api.search_flames(&params)
        .await
        .map_err(|e| e.to_string())
}

/// Fetch a single flame's full config from the API (cross-platform async helper)
/// Returns (FractalConfig, flame_id, is_public) on success.
/// Result of fetching a flame from the API
struct OnlineFlameResult {
    config: crate::config::FractalConfig,
    flame_id: String,
    is_public: Option<bool>,
    user_id: String,
    animation_count: u32,
    animations: Vec<crate::api::types::AnimationSummary>,
}

async fn fetch_online_flame(base_url: &str, token: &str, flame_id: &str) -> Result<OnlineFlameResult, String> {
    let mut api = crate::api::ApiState::new(base_url);
    api.set_token(token);
    let result = api.load_flame_with_visibility(flame_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok(OnlineFlameResult {
        config: result.config,
        flame_id: flame_id.to_string(),
        is_public: result.is_public,
        user_id: result.user_id,
        animation_count: result.animation_count,
        animations: result.animations,
    })
}

/// Delete a flame from the API (cross-platform async helper)
async fn delete_online_flame(base_url: &str, token: &str, flame_id: &str) -> Result<(), String> {
    let mut api = crate::api::ApiState::new(base_url);
    api.set_token(token);
    api.delete_flame(flame_id)
        .await
        .map_err(|e| e.to_string())
}

/// Check if an error string means the session is gone (401), which
/// signs the user out.
///
/// Only the 401: a 403 ("Access denied") is the server refusing THIS
/// request — someone else's private flame, a delete of a flame the user
/// does not own — while the session is fine. Treating it as an expired
/// session signed the user out for asking.
fn is_auth_error(error: &str) -> bool {
    error.contains(&crate::resources::FetchError::Unauthorized.to_string())
}

#[cfg(test)]
mod auth_tests {
    use super::is_auth_error;
    use crate::resources::FetchError;

    #[test]
    fn only_a_401_signs_the_user_out() {
        assert!(is_auth_error(&FetchError::Unauthorized.to_string()));
        // As the async helpers carry it, wrapped in context.
        assert!(is_auth_error(&format!("Failed to load flame: {}", FetchError::Unauthorized)));
        assert!(!is_auth_error(&FetchError::Forbidden.to_string()), "a 403 is not an expired session");
        assert!(!is_auth_error(&FetchError::NotFound("x".into()).to_string()));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::types::{ApiRenderMode, FlameListItem};

    fn item(render_mode: ApiRenderMode, transform_count: i32, variations: &[&str]) -> FlameListItem {
        FlameListItem {
            id: "id".to_string(),
            name: "Reef".to_string(),
            render_mode,
            transform_count,
            variation_names: variations.iter().map(|s| s.to_string()).collect(),
            has_3d: false,
            created_at: String::new(),
            updated_at: String::new(),
        }
    }

    /// A simulation has no transforms and no variations, legitimately;
    /// its row used to read "Reef — Simulation | 0T | " like a broken
    /// flame, because only Escape got the short form.
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
        use super::PresetCategory as C;
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
        use super::PresetCategory as C;
        let mut kept = preset("Halayka: sin Z + C sin Z");
        assert!(kept.escape.custom_formula.is_some());
        kept.escape.formula = "mandelbrot".to_string();
        assert_eq!(C::of(&kept), C::Escape2d);
        let mut terrain = preset("Douady Rabbit");
        terrain.escape.formula = "mandelbrot".to_string();
        terrain.escape.terrain.enabled = true;
        assert_eq!(C::of(&terrain), C::Escape3d);
    }

    /// The browser holds every preset, each in its category's gallery, and
    /// opens on the first category.
    #[test]
    fn the_browser_sorts_the_presets_into_their_galleries() {
        use super::PresetCategory as C;
        let presets = crate::resources::load_presets_with_fallback();
        let panel = FractalBrowserPanel::new();
        assert_eq!(panel.preset_category, C::Flame2d);
        for category in C::ALL {
            let want = presets.iter().filter(|p| C::of(p) == category).count();
            assert_eq!(panel.preset_galleries[category.index()].len(), want, "{category:?}");
        }
    }

    #[test]
    fn a_simulation_row_has_no_transform_columns() {
        let label = online_row_label(&item(ApiRenderMode::Simulation, 0, &[]));
        assert!(label.starts_with("Reef — "), "{label}");
        assert!(!label.contains("0T"), "{label}");
        assert!(!label.contains('|'), "{label}");

        let label = online_row_label(&item(ApiRenderMode::Escape, 0, &[]));
        assert!(!label.contains('|'), "{label}");
    }

    #[test]
    fn a_flame_row_keeps_its_transform_count_and_variations() {
        let label = online_row_label(&item(ApiRenderMode::TwoD, 3, &["linear", "swirl"]));
        assert!(label.contains("| 3T | linear, swirl"), "{label}");
        let label = online_row_label(&item(ApiRenderMode::ThreeD, 2, &["a", "b", "c", "d"]));
        assert!(label.contains("| 2T | a, b, +2"), "{label}");
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

    /// Escape is searchable; Simulation is not offered, because the
    /// server's render_mode enum rejects it.
    #[test]
    fn the_online_filter_offers_escape_but_not_simulation() {
        let modes: Vec<_> = ONLINE_RENDER_MODE_FILTERS.iter().map(|(m, _)| *m).collect();
        assert_eq!(
            modes,
            vec![
                None,
                Some(ApiRenderMode::TwoD),
                Some(ApiRenderMode::ThreeD),
                Some(ApiRenderMode::Escape),
            ]
        );
    }
}
