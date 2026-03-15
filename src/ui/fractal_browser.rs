//! Fractal Browser panel - unified browser for presets, batch results, files, and online flames
//!
//! A tabbed panel that provides views:
//! - Presets: Built-in presets from assets/presets.fflame
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
    online_flame_result: Arc<Mutex<Option<Result<(FractalConfig, String, Option<bool>), String>>>>,
    /// Whether a delete is in progress
    online_deleting: bool,
    /// Shared slot for receiving async delete result (flame_name on success)
    online_delete_result: Arc<Mutex<Option<Result<String, String>>>>,
    /// Search filter: name text
    online_search_name: String,
    /// Search filter: render mode (0=All, 1=2D, 2=3D)
    online_search_render_mode: u8,

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

            online_flames: Vec::new(),
            online_fetched: false,
            online_loading: false,
            online_error: None,
            online_list_result: Arc::new(Mutex::new(None)),
            online_loading_flame: false,
            online_flame_result: Arc::new(Mutex::new(None)),
            online_deleting: false,
            online_delete_result: Arc::new(Mutex::new(None)),
            online_search_name: String::new(),
            online_search_render_mode: 0,

            auth_credentials: None,
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

    /// Clear online tab data (used when signing out).
    /// Keeps `online_fetched = true` so the auto-fetch doesn't re-trigger
    /// and hammer the API with 401s. User must sign in + click Refresh.
    pub fn clear_online_data(&mut self) {
        self.online_flames.clear();
        self.online_fetched = true; // Don't auto-retry after sign-out
        self.online_loading = false;
        self.online_error = None;
        self.online_search_name.clear();
        self.online_search_render_mode = 0;
    }

    /// Reset online tab for a fresh session (used after sign-in).
    /// Sets `online_fetched = false` so the next visit auto-fetches.
    pub fn reset_online_for_new_session(&mut self) {
        self.online_fetched = false;
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
            BrowserTab::Online => {
                // Online tab doesn't use FractalConfigGallery for thumbnails
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
            BrowserTab::Online => true, // Online tab doesn't generate thumbnails locally
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
            BrowserTab::Online => (0, 0),
        }
    }

    // ========== Rendering ==========

    /// Render the panel
    /// `auth` is `Some((base_url, token))` when signed in, `None` otherwise.
    pub fn render(&mut self, ui: &mut egui::Ui, online_mode: bool, auth: Option<(&str, &str)>) -> GalleryResponse {
        // Cache auth credentials for use by trigger methods
        self.auth_credentials = auth.map(|(b, t)| (b.to_string(), t.to_string()));

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
                        Ok((config, flame_id, is_public)) => {
                            response.selected = Some(config);
                            response.api_flame_id = Some(flame_id);
                            response.api_flame_is_public = is_public;
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
            if text_response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                search_triggered = true;
            }

            ui.separator();

            if ui.selectable_label(self.online_search_render_mode == 0, t!("browser.search_all").as_ref()).clicked() {
                self.online_search_render_mode = 0;
            }
            if ui.selectable_label(self.online_search_render_mode == 1, t!("browser.search_2d").as_ref()).clicked() {
                self.online_search_render_mode = 1;
            }
            if ui.selectable_label(self.online_search_render_mode == 2, t!("browser.search_3d").as_ref()).clicked() {
                self.online_search_render_mode = 2;
            }

            ui.separator();

            let search_enabled = !self.online_loading && !self.online_loading_flame && !self.online_deleting;
            if ui.add_enabled(search_enabled, egui::Button::new(t!("browser.search_button"))).clicked() {
                search_triggered = true;
            }

            let has_filters = !self.online_search_name.is_empty() || self.online_search_render_mode != 0;
            if has_filters {
                if ui.add_enabled(search_enabled, egui::Button::new(t!("browser.search_clear"))).clicked() {
                    self.online_search_name.clear();
                    self.online_search_render_mode = 0;
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

            egui::ScrollArea::vertical().show(ui, |ui| {
                for flame in &flames {
                    let render_mode = match flame.render_mode {
                        crate::api::types::ApiRenderMode::TwoD => "2D",
                        crate::api::types::ApiRenderMode::ThreeD => "3D",
                    };

                    let variations_summary = if flame.variation_names.len() <= 3 {
                        flame.variation_names.join(", ")
                    } else {
                        format!(
                            "{}, +{}",
                            flame.variation_names[..2].join(", "),
                            flame.variation_names.len() - 2
                        )
                    };

                    let label = format!(
                        "{} — {} | {}T | {}",
                        flame.name, render_mode, flame.transform_count, variations_summary
                    );

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
            });

            // Trigger delete outside the borrow
            if let Some((id, name)) = delete_flame {
                self.trigger_delete_flame(&id, &name);
            }
        }

        response
    }

    /// Trigger async fetch of the flame list from the API.
    /// Uses search_flames() if any filters are active, otherwise list_my_flames().
    fn trigger_fetch_flames(&mut self) {
        self.online_loading = true;
        self.online_error = None;

        let has_filters = !self.online_search_name.is_empty() || self.online_search_render_mode != 0;

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
            let render_mode = match self.online_search_render_mode {
                1 => Some(crate::api::types::ApiRenderMode::TwoD),
                2 => Some(crate::api::types::ApiRenderMode::ThreeD),
                _ => None,
            };
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
async fn fetch_online_flame(base_url: &str, token: &str, flame_id: &str) -> Result<(crate::config::FractalConfig, String, Option<bool>), String> {
    let mut api = crate::api::ApiState::new(base_url);
    api.set_token(token);
    let (config, is_public) = api.load_flame_with_visibility(flame_id)
        .await
        .map_err(|e| e.to_string())?;
    Ok((config, flame_id.to_string(), is_public))
}

/// Delete a flame from the API (cross-platform async helper)
async fn delete_online_flame(base_url: &str, token: &str, flame_id: &str) -> Result<(), String> {
    let mut api = crate::api::ApiState::new(base_url);
    api.set_token(token);
    api.delete_flame(flame_id)
        .await
        .map_err(|e| e.to_string())
}

/// Check if an error string indicates an authentication/authorization failure (401/403)
fn is_auth_error(error: &str) -> bool {
    error.contains("Authentication required") || error.contains("Access denied")
}
