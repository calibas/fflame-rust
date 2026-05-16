//! Application state and event loop

mod input;
mod config;
mod ui_handlers;
mod gpu_updates;
mod animation_update;
mod variation_fetch;
pub mod export;
pub mod render_mode;

pub use render_mode::{RenderModeFSM};

#[cfg(not(target_arch = "wasm32"))]
pub use export::export_headless;

#[cfg(target_arch = "wasm32")]
pub use export::export_headless_wasm;

/// Trigger a browser download of binary data (WASM only)
#[cfg(target_arch = "wasm32")]
pub fn trigger_browser_download(data: &[u8], filename: &str, mime_type: &str) -> Result<(), String> {
    use wasm_bindgen::JsCast;
    use web_sys::{Blob, BlobPropertyBag, Url, HtmlAnchorElement};

    // Create Uint8Array from data
    let array = js_sys::Uint8Array::from(data);
    let blob_parts = js_sys::Array::new();
    blob_parts.push(&array);

    // Create Blob with correct MIME type
    let options = BlobPropertyBag::new();
    options.set_type(mime_type);

    let blob = Blob::new_with_u8_array_sequence_and_options(&blob_parts, &options)
        .map_err(|e| format!("Failed to create blob: {:?}", e))?;

    // Create object URL
    let url = Url::create_object_url_with_blob(&blob)
        .map_err(|e| format!("Failed to create object URL: {:?}", e))?;

    // Create and click anchor element to trigger download
    let window = web_sys::window().ok_or("No window")?;
    let document = window.document().ok_or("No document")?;
    let a = document.create_element("a")
        .map_err(|e| format!("Failed to create anchor: {:?}", e))?
        .dyn_into::<HtmlAnchorElement>()
        .map_err(|_| "Failed to cast to anchor")?;

    a.set_href(&url);
    a.set_download(filename);
    a.click();

    // Clean up object URL
    let _ = Url::revoke_object_url(&url);

    Ok(())
}

/// Trigger a native browser file picker and read file contents (WASM only)
/// Uses <input type="file"> directly instead of rfd to avoid extra dialogs
#[cfg(target_arch = "wasm32")]
pub fn trigger_browser_file_picker(accept: &str, ctx: egui::Context, result_id: &'static str) {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{HtmlInputElement, FileReader};

    let window = match web_sys::window() {
        Some(w) => w,
        None => { log::error!("No window"); return; }
    };
    let document = match window.document() {
        Some(d) => d,
        None => { log::error!("No document"); return; }
    };

    // Create hidden file input
    let input: HtmlInputElement = match document.create_element("input") {
        Ok(el) => match el.dyn_into::<HtmlInputElement>() {
            Ok(input) => input,
            Err(_) => { log::error!("Failed to cast to input"); return; }
        },
        Err(_) => { log::error!("Failed to create input"); return; }
    };

    input.set_type("file");
    input.set_accept(accept);
    input.style().set_property("display", "none").ok();

    // Append to body temporarily
    if let Some(body) = document.body() {
        let _ = body.append_child(&input);
    }

    // Set up change handler
    let input_clone = input.clone();
    let ctx_clone = ctx.clone();
    let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
        let files = match input_clone.files() {
            Some(f) => f,
            None => return,
        };

        if files.length() == 0 {
            return;
        }

        let file = match files.get(0) {
            Some(f) => f,
            None => return,
        };

        let reader = match FileReader::new() {
            Ok(r) => r,
            Err(_) => { log::error!("Failed to create FileReader"); return; }
        };

        let reader_clone = reader.clone();
        let ctx_for_load = ctx_clone.clone();
        let onload = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            let result = match reader_clone.result() {
                Ok(r) => r,
                Err(_) => return,
            };

            let array_buffer = match result.dyn_into::<js_sys::ArrayBuffer>() {
                Ok(ab) => ab,
                Err(_) => return,
            };

            let uint8_array = js_sys::Uint8Array::new(&array_buffer);
            let mut contents = vec![0u8; uint8_array.length() as usize];
            uint8_array.copy_to(&mut contents);

            let text = String::from_utf8_lossy(&contents).to_string();

            // Store in egui temp storage for pickup
            ctx_for_load.data_mut(|data| {
                data.insert_temp(egui::Id::new(result_id), text);
            });
            ctx_for_load.request_repaint();
        }) as Box<dyn FnMut(_)>);

        reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget(); // Leak closure - it will be cleaned up when reader is done

        let _ = reader.read_as_array_buffer(&file);

        // Clean up input element
        if let Some(parent) = input_clone.parent_node() {
            let _ = parent.remove_child(&input_clone);
        }
    }) as Box<dyn FnMut(_)>);

    input.set_onchange(Some(closure.as_ref().unchecked_ref()));
    closure.forget(); // Leak closure - it will be called when file is selected

    // Trigger file picker
    input.click();
}

/// Trigger a native browser file picker for binary files (WASM only)
/// Stores raw bytes instead of converting to String (for audio, images, etc.)
#[cfg(target_arch = "wasm32")]
pub fn trigger_browser_file_picker_binary(accept: &str, ctx: egui::Context, result_id: &'static str) {
    use wasm_bindgen::prelude::*;
    use wasm_bindgen::JsCast;
    use web_sys::{HtmlInputElement, FileReader};

    let window = match web_sys::window() {
        Some(w) => w,
        None => { log::error!("No window"); return; }
    };
    let document = match window.document() {
        Some(d) => d,
        None => { log::error!("No document"); return; }
    };

    // Create hidden file input
    let input: HtmlInputElement = match document.create_element("input") {
        Ok(el) => match el.dyn_into::<HtmlInputElement>() {
            Ok(input) => input,
            Err(_) => { log::error!("Failed to cast to input"); return; }
        },
        Err(_) => { log::error!("Failed to create input"); return; }
    };

    input.set_type("file");
    input.set_accept(accept);
    input.style().set_property("display", "none").ok();

    // Append to body temporarily
    if let Some(body) = document.body() {
        let _ = body.append_child(&input);
    }

    // Set up change handler
    let input_clone = input.clone();
    let ctx_clone = ctx.clone();
    let closure = Closure::wrap(Box::new(move |_event: web_sys::Event| {
        let files = match input_clone.files() {
            Some(f) => f,
            None => return,
        };

        if files.length() == 0 {
            return;
        }

        let file = match files.get(0) {
            Some(f) => f,
            None => return,
        };

        let reader = match FileReader::new() {
            Ok(r) => r,
            Err(_) => { log::error!("Failed to create FileReader"); return; }
        };

        let reader_clone = reader.clone();
        let ctx_for_load = ctx_clone.clone();
        let onload = Closure::wrap(Box::new(move |_event: web_sys::Event| {
            let result = match reader_clone.result() {
                Ok(r) => r,
                Err(_) => return,
            };

            let array_buffer = match result.dyn_into::<js_sys::ArrayBuffer>() {
                Ok(ab) => ab,
                Err(_) => return,
            };

            let uint8_array = js_sys::Uint8Array::new(&array_buffer);
            let mut contents = vec![0u8; uint8_array.length() as usize];
            uint8_array.copy_to(&mut contents);

            // Store raw bytes in egui temp storage for pickup
            ctx_for_load.data_mut(|data| {
                data.insert_temp(egui::Id::new(result_id), contents);
            });
            ctx_for_load.request_repaint();
        }) as Box<dyn FnMut(_)>);

        reader.set_onload(Some(onload.as_ref().unchecked_ref()));
        onload.forget(); // Leak closure - it will be cleaned up when reader is done

        let _ = reader.read_as_array_buffer(&file);

        // Clean up input element
        if let Some(parent) = input_clone.parent_node() {
            let _ = parent.remove_child(&input_clone);
        }
    }) as Box<dyn FnMut(_)>);

    input.set_onchange(Some(closure.as_ref().unchecked_ref()));
    closure.forget(); // Leak closure - it will be called when file is selected

    // Trigger file picker
    input.click();
}

use winit::{event::*, event_loop::{EventLoop, ControlFlow, ActiveEventLoop}, window::Window};
use egui_wgpu::wgpu::SurfaceError;
use std::sync::{Arc, Mutex};

/// Tracks which flame/animation is currently loaded from the API.
/// Session-only state (not persisted). Cleared on new flame, load file, etc.
#[derive(Clone)]
pub struct ApiContentState {
    pub flame_id: Option<String>,
    pub flame_is_public: Option<bool>,
    pub flame_user_id: Option<String>,
    pub animation_id: Option<String>,
    pub animation_count: u32,
    /// List of animations linked to this flame (from FlameResponse.animations)
    pub flame_animations: Vec<crate::api::types::AnimationSummary>,
    /// Palette ID if loaded from the API
    pub palette_id: Option<String>,
    /// Palette owner's user_id (for ownership checks)
    pub palette_user_id: Option<String>,
}

impl Default for ApiContentState {
    fn default() -> Self {
        Self {
            flame_id: None,
            flame_is_public: None,
            flame_user_id: None,
            animation_id: None,
            animation_count: 0,
            flame_animations: Vec::new(),
            palette_id: None,
            palette_user_id: None,
        }
    }
}

impl ApiContentState {
    /// Clear all API content state (new flame, load from disk, etc.)
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Clear only animation state (new animation, etc.)
    pub fn clear_animation(&mut self) {
        self.animation_id = None;
    }

    /// Check if the current user owns the flame.
    /// Compares flame's user_id against the provided current user ID.
    pub fn flame_owned_by(&self, current_user_id: Option<&str>) -> bool {
        match (&self.flame_user_id, current_user_id) {
            (Some(flame_uid), Some(current_uid)) => flame_uid == current_uid,
            // If either is unknown, assume owned (e.g., just saved, no user_id returned)
            _ => true,
        }
    }
}

use crate::gpu::device::GpuContext;
use crate::ui::EguiLayer;
use crate::ui::animation_panel::ExportProgress;
use crate::ui::PngExportProgress;
use crate::renderer::FlameRenderer;
use crate::scene::transforms::Flame;
use crate::scene::palette::{global_palette_library, PaletteLibrary};
use crate::scene::presets::{global_preset_library, PresetLibrary};
use crate::util::PerformanceMetrics;
use crate::config::ConfigManager;
use crate::animation::AnimationController;

/// Data loaded from a URL deep-link (?flame=uuid or ?animation=uuid)
pub(super) enum UrlLoadedData {
    /// A flame loaded from the API, with full metadata
    Flame {
        config: crate::config::FractalConfig,
        flame_id: String,
        is_public: Option<bool>,
        user_id: String,
        animation_count: u32,
        animations: Vec<crate::api::types::AnimationSummary>,
    },
    /// An animation loaded from the API, with optional flame metadata
    Animation {
        animation: crate::animation::Animation,
        animation_id: String,
        flame_id: Option<String>,
        /// Full flame metadata (fetched separately if flame_id is present)
        flame_meta: Option<crate::api::FlameLoadResult>,
    },
}

pub struct App {
    // Window reference (needed for fullscreen toggle)
    pub(super) window: Arc<Window>,

    // Core state management
    pub(super) config_manager: ConfigManager,  // Single source of truth for ALL config (fractal + system)

    // GPU and rendering resources
    pub(super) gpu: GpuContext,
    pub(super) egui_layer: EguiLayer,
    pub(super) flame_renderer: Option<FlameRenderer>,
    pub(super) flame: Flame,  // Working copy for renderer (synced from config_manager)

    // UI state (not saved in config)
    pub(super) workspace: crate::ui::Workspace,
    pub(super) view_changed_by_keyboard: bool,
    pub(super) paused: bool,
    pub(super) modifiers: winit::keyboard::ModifiersState,
    pub(super) quit_requested: bool,  // Graceful quit requested (check unsaved changes, etc.)
    /// Renderer-side view toggle: when true and a subflame is being
    /// edited, the viewport renders the active subflame in isolation
    /// (its IFS standalone). When false (default), the viewport
    /// renders the parent flame with the live-edited subflame
    /// inlined via `subflame_wf`. This is purely a render-source
    /// decision now — App.flame and ConfigManager state are
    /// unchanged regardless of the toggle.
    pub(super) view_subflame_in_isolation: bool,

    // Libraries (not saved in config)
    pub(super) palette_library: PaletteLibrary,
    pub(super) preset_library: &'static PresetLibrary,

    // Animation system
    pub(super) animation_controller: AnimationController,

    // Performance tracking
    pub(super) metrics: PerformanceMetrics,

    // Rendering internals (frame timing and batching)
    pub(super) last_frame_time: Option<web_time::Instant>,
    pub(super) accumulation_batch_size: u32,  // Process every N frames (1 = normal, 4 = batched)
    pub(super) frames_since_accumulation: u32,
    // Overwrite mode timing (100ms window after parameter changes)
    // Note: This is intentionally separate from RenderModeFSM. The FSM manages high-level
    // state transitions (Normal/Animating/Overwrite), while this simple timer handles the
    // brief overwrite window. Moving the timer into FSM would add complexity for minimal benefit.
    pub(super) use_overwrite_next_frame: bool,
    pub(super) last_param_change_time: Option<web_time::Instant>,
    pub(super) rendering_complete: bool,  // True when rendering has finished (max_iterations reached)
    pub(super) clear_paths_next_frame: bool,  // Clear path buffer on next compute pass (full reset)
    pub(super) ui_needs_repaint: bool,  // Track if UI is requesting repaints (for frame rate boost)
    pub(super) last_input_time: Option<web_time::Instant>,  // Time of last user input (for idle detection)

    // Fractal viewport size (updated from UI each frame)
    pub(super) fractal_viewport_size: (u32, u32),
    // Debounce viewport resize (WASM only - prevents rapid resize loops)
    #[cfg(target_arch = "wasm32")]
    pub(super) last_viewport_resize_time: Option<web_time::Instant>,

    // PNG export settings (UI state only, not in config)
    pub(super) export_width: u32,
    pub(super) export_height: u32,
    pub(super) use_custom_export_size: bool,

    // Animation export progress (shared with background export thread)
    pub(super) animation_export_progress: Arc<Mutex<ExportProgress>>,

    // PNG export progress (shared with background export thread)
    pub(super) png_export_progress: Arc<Mutex<PngExportProgress>>,

    // Rendering mode state machine (Normal, Animating, Overwrite)
    pub(super) render_mode: RenderModeFSM,

    // Histogram computation (computed periodically, not every frame)
    pub(super) histogram_frame_counter: u32,
    // WASM: async histogram result slot + in-flight guard
    #[cfg(target_arch = "wasm32")]
    pub(super) histogram_result_slot: Option<std::sync::Arc<std::sync::Mutex<Option<crate::renderer::DensityHistogram>>>>,
    #[cfg(target_arch = "wasm32")]
    pub(super) histogram_in_flight: bool,

    // Post-processing effect chain
    pub(super) effect_chain: crate::renderer::effect_chain::EffectChainRunner,

    // Track export state to detect when export finishes (for surface recovery)
    pub(super) was_video_exporting: bool,

    // Fullscreen state (two-stage: window fullscreen, then hide UI)
    pub(super) window_fullscreen: bool,  // Window is in fullscreen mode
    pub(super) ui_hidden: bool,          // UI panels are hidden (only in fullscreen)

    // Surface recovery: set on surface error, handled at top of next RedrawRequested
    pub(super) needs_surface_recreate: bool,

    // Audio system
    pub(super) audio_manager: crate::audio::AudioManager,
    pub(super) audio_player: crate::audio::AudioPlayer,
    pub(super) audio_capture: crate::audio::AudioCapture,
    pub(super) signal_manager: crate::signal::SignalManager,

    // API content state — tracks which flame/animation is loaded from the API
    pub(super) api_state: ApiContentState,
    /// Current authenticated user's ID (from /api/users/me). None when signed out.
    /// Used for ownership checks when deciding whether to allow Update vs Save as Copy.
    pub(super) current_user_id: Option<String>,
    /// Parallel to config history: for each FullConfig snapshot, stores
    /// (before_api_state, after_api_state). Keyed by the history index of
    /// the snapshot. Delta-level undo/redo doesn't touch api_state.
    pub(super) api_state_history: std::collections::HashMap<usize, (ApiContentState, ApiContentState)>,
    /// Which snapshot we're currently "in" — updates on undo/redo across snapshots.
    /// Used to keep the current snapshot's "after" up to date with any api_state changes.
    pub(super) current_api_snapshot_idx: Option<usize>,
    // API save in-flight state
    pub(super) api_pending_visibility: Option<bool>,
    /// True if the in-flight save is a SaveNew (creates a new flame ID).
    /// Used to clear flame_animations / animation_id on completion.
    pub(super) api_pending_is_new: bool,
    pub(super) api_save_result: std::sync::Arc<std::sync::Mutex<Option<Result<String, String>>>>,
    pub(super) api_save_in_progress: bool,
    pub(super) api_animation_save_result: std::sync::Arc<std::sync::Mutex<Option<Result<String, String>>>>,
    pub(super) api_animation_save_in_progress: bool,
    /// If Some, save animation after flame save completes (legacy deferred save).
    pub(super) pending_animation_save: Option<Option<crate::api::types::ApiVisibility>>,

    // URL deep-link loading (WASM: ?flame=uuid or ?animation=uuid)
    pub(super) url_load_result: std::sync::Arc<std::sync::Mutex<Option<Result<UrlLoadedData, String>>>>,
    pub(super) url_load_in_progress: bool,
    pub(super) url_load_started: Option<web_time::Instant>,

    // Variation fetching: when loading a flame that references unknown
    // variations, fetch them from the API in parallel and pause rendering
    // until done (or timeout).
    pub(super) variation_fetch_results: std::sync::Arc<std::sync::Mutex<
        Vec<(String, Result<crate::api::types::VariationDownload, String>)>
    >>,
    pub(super) variation_fetch_in_progress: bool,
    pub(super) variation_fetch_started: Option<web_time::Instant>,
    /// Number of in-flight fetches; when this drops to 0, processing is complete
    pub(super) variation_fetch_pending_count: usize,

    // API connectivity tracking
    pub(super) health_check_result: std::sync::Arc<std::sync::Mutex<Option<crate::api::HealthCheckOutcome>>>,
    pub(super) health_check_in_progress: bool,
    pub(super) api_connectivity: crate::api::ApiConnectivity,
    pub(super) last_health_check: Option<web_time::Instant>,
}
impl App {
    pub async fn run(
        event_loop: EventLoop<()>,
        window: Arc<Window>,
        #[cfg(target_arch = "wasm32")] url_flame_id: Option<String>,
        #[cfg(target_arch = "wasm32")] url_animation_id: Option<String>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let gpu = GpuContext::new(window.clone()).await.expect("GPU init failed");
        let egui_layer = EguiLayer::new(&window, &gpu.device, gpu.config.format);

        // Load any API variations cached from previous sessions into the global registry
        crate::variations::load_cached_api_variations();

        // Use global preset library singleton
        let preset_library = global_preset_library();
        // Clone from global palette library singleton (App needs mutable copy)
        let palette_library = {
            let guard = global_palette_library().read().unwrap();
            (*guard).clone()
        };

        // Use entire FractalConfig from first preset (not just the flame!)
        let initial_config = preset_library.get(0)
            .cloned()
            .unwrap_or_default();

        // Note: Device-specific settings (iterations_per_thread, vsync_enabled, target_fps)
        // are loaded by ConfigManager from SystemSettings

        let flame = initial_config.flame.clone();

        let flame_renderer = FlameRenderer::with_palette_size(
            &gpu.device,
            &gpu.queue,
            gpu.config.format,
            gpu.size.width,
            gpu.size.height,
            &flame,
            initial_config.palette_size,
        );

        // ConfigManager loads SystemSettings automatically
        let config_manager = ConfigManager::new(initial_config.clone());

        // Get initial size before moving gpu
        let initial_viewport_size = (gpu.size.width, gpu.size.height);

        // Get export dimensions before moving config_manager
        let export_width = config_manager.system_settings().default_export_width;
        let export_height = config_manager.system_settings().default_export_height;

        // Create effect chain for post-processing effects
        let effect_chain = crate::renderer::effect_chain::EffectChainRunner::new(
            &gpu.device,
            gpu.size.width,
            gpu.size.height,
        );

        let mut app = Self {
            window: window.clone(),
            config_manager,
            gpu,
            egui_layer,
            flame_renderer: Some(flame_renderer),
            flame,
            workspace: crate::ui::Workspace::new(),
            view_changed_by_keyboard: false,
            paused: false,
            view_subflame_in_isolation: false,
            modifiers: winit::keyboard::ModifiersState::default(),
            quit_requested: false,
            palette_library,
            preset_library,
            animation_controller: AnimationController::new(),
            metrics: PerformanceMetrics::new(),
            last_frame_time: None,
            accumulation_batch_size: 4, // EXPERIMENT: Test batching
            frames_since_accumulation: 0,
            use_overwrite_next_frame: false,
            last_param_change_time: None,
            rendering_complete: false,
            clear_paths_next_frame: true,  // Clear paths on first frame
            ui_needs_repaint: false,
            last_input_time: None,
            fractal_viewport_size: initial_viewport_size, // Initialize to window size
            #[cfg(target_arch = "wasm32")]
            last_viewport_resize_time: None,
            export_width,
            export_height,
            use_custom_export_size: false,  // Default to viewport size
            animation_export_progress: Arc::new(Mutex::new(ExportProgress::default())),
            png_export_progress: Arc::new(Mutex::new(PngExportProgress::default())),
            render_mode: RenderModeFSM::new(),
            histogram_frame_counter: 0,
            #[cfg(target_arch = "wasm32")]
            histogram_result_slot: None,
            #[cfg(target_arch = "wasm32")]
            histogram_in_flight: false,
            effect_chain,
            was_video_exporting: false,
            needs_surface_recreate: false,
            window_fullscreen: false,
            ui_hidden: false,
            audio_manager: crate::audio::AudioManager::new(),
            audio_player: crate::audio::AudioPlayer::new(),
            audio_capture: crate::audio::AudioCapture::new(),
            signal_manager: crate::signal::SignalManager::new(),
            api_state: ApiContentState::default(),
            current_user_id: None,
            api_state_history: std::collections::HashMap::new(),
            current_api_snapshot_idx: None,
            api_pending_visibility: None,
            api_pending_is_new: false,
            api_save_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
            api_save_in_progress: false,
            api_animation_save_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
            api_animation_save_in_progress: false,
            pending_animation_save: None,
            url_load_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
            url_load_in_progress: false,
            url_load_started: None,

            variation_fetch_results: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            variation_fetch_in_progress: false,
            variation_fetch_started: None,
            variation_fetch_pending_count: 0,
            health_check_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
            health_check_in_progress: false,
            api_connectivity: crate::api::ApiConnectivity::Unknown,
            last_health_check: None,
        };

        // Register live audio capture as a signal producer so live_* signals
        // appear in the animation track signal dropdown
        app.signal_manager.add_producer(app.audio_capture.create_producer());

        // Initialize GPU state with initial config (ensures shaders are compiled with correct variations)
        app.import_config(initial_config);

        // Detect compact (mobile) mode from logical window size
        {
            let compact_mode = match app.config_manager.system_settings().compact_mode {
                Some(saved) => saved, // User has a saved preference
                None => {
                    // Auto-detect: logical width < 600 means compact
                    // On WASM, use browser's CSS width directly (reliable logical size).
                    // winit's scale_factor() may not reflect devicePixelRatio on all browsers.
                    #[cfg(target_arch = "wasm32")]
                    let logical_width = {
                        let w = web_sys::window().unwrap();
                        w.inner_width().unwrap().as_f64().unwrap() as f32
                    };
                    #[cfg(not(target_arch = "wasm32"))]
                    let logical_width = {
                        let scale_factor = window.scale_factor() as f32;
                        app.gpu.size.width as f32 / scale_factor
                    };
                    let is_compact = logical_width < 600.0;
                    log::info!(
                        "Compact mode auto-detected: {} (logical width {:.0}px)",
                        is_compact, logical_width
                    );
                    // Save auto-detected value so it persists
                    app.config_manager.system_settings_mut().compact_mode = Some(is_compact);
                    let _ = app.config_manager.system_settings().save();
                    is_compact
                }
            };
            if compact_mode {
                app.egui_layer.set_compact_mode(true);
                app.workspace.apply_layout(crate::ui::workspace::WorkspaceLayout::Compact);
            }
        }

        // Open Help panel on startup if setting is enabled (centered, minimum 350px wide)
        if !app.workspace.is_compact() && app.config_manager.system_settings().show_help_on_startup {
            use crate::ui::workspace::PanelType;
            let screen_size = egui::vec2(app.gpu.size.width as f32, app.gpu.size.height as f32);
            let help_size = egui::vec2(400.0, 450.0); // Fixed size for Help panel
            app.workspace.open_floating_panel_centered(PanelType::Help, help_size, screen_size);
        }

        // Trigger initial API health check if a token exists (desktop: Bearer token, WASM: cookie)
        app.maybe_trigger_health_check();

        // Desktop: attempt auto-login from saved encrypted credentials
        #[cfg(not(target_arch = "wasm32"))]
        {
            crate::ui::login_dialog::try_auto_login(
                app.egui_layer.login_dialog_state_mut(),
                &app.config_manager,
            );
        }

        // WASM: trigger deep-link load from URL params (?flame=uuid or ?animation=uuid)
        // Pause rendering so the default flame doesn't render before the URL-loaded one arrives.
        #[cfg(target_arch = "wasm32")]
        if url_flame_id.is_some() || url_animation_id.is_some() {
            app.paused = true;
            app.trigger_url_load(url_flame_id, url_animation_id);
        }

        #[allow(deprecated)]
        event_loop.run(move |event, elwt| {
            match event {
                Event::WindowEvent { event, window_id } if window_id == window.id() => {
                    // Let egui handle events first
                    let consumed = app.egui_layer.handle_event(&event, &window);

                    // Request redraw for events that need visual updates
                    // This wakes from ControlFlow::Wait when user interacts
                    match &event {
                        WindowEvent::CursorMoved { .. } |
                        WindowEvent::MouseInput { .. } |
                        WindowEvent::MouseWheel { .. } |
                        WindowEvent::KeyboardInput { .. } => {
                            // Track last input time for UI idle detection (tooltips, animations)
                            app.last_input_time = Some(web_time::Instant::now());
                            window.request_redraw();
                        }
                        WindowEvent::Touch(ref touch) => {
                            app.last_input_time = Some(web_time::Instant::now());
                            window.request_redraw();
                        }
                        WindowEvent::Resized(_) |
                        WindowEvent::ScaleFactorChanged { .. } => {
                            app.last_input_time = Some(web_time::Instant::now());
                            window.request_redraw();
                        }
                        _ => {}
                    }

                    match event {
                        WindowEvent::CloseRequested => {
                            app.shutdown(elwt);
                        },
                        WindowEvent::Resized(size) => {
                            if size.width > 0 && size.height > 0 {
                                log::debug!("Window resized to {}x{}", size.width, size.height);
                                app.gpu.resize(size);
                                // NOTE: Don't resize renderer here - it will be resized by fractal viewport resize
                                // The fractal panel is smaller than the window (due to UI panels)
                                // Resizing renderer to window size causes aspect ratio mismatch
                            } else {
                                // Window minimized — mark size as zero so render() skips
                                app.gpu.size = size;
                            }
                        },
                        WindowEvent::ScaleFactorChanged { .. } => {
                            // Handle DPI/zoom changes - resize surface to match new scale
                            // Note: Renderer resize will happen via fractal viewport resize in render()
                            let new_size = window.inner_size();
                            if new_size.width > 0 && new_size.height > 0 {
                                app.gpu.resize(new_size);
                                window.request_redraw();
                            }
                        },
                        WindowEvent::Focused(true) => {
                            // Reconfigure surface on focus regain to fix UI offset
                            // (Windows DWM composition changes can desync surface position)
                            let size = window.inner_size();
                            if size.width > 0 && size.height > 0 {
                                app.gpu.resize(size);
                            }
                            window.request_redraw();
                        }
                        WindowEvent::KeyboardInput { event: key_event, .. } if !consumed => {
                            // Handle keyboard input only if egui didn't consume it
                            app.handle_keyboard(&key_event);
                        }
                        WindowEvent::ModifiersChanged(new_modifiers) => {
                            app.modifiers = new_modifiers.state();
                        }
                        WindowEvent::RedrawRequested => {
                            // Handle deferred GPU reinitialization (from previous frame's error)
                            // Desktop only — WASM WebGPU doesn't have device loss from sleep/wake
                            #[cfg(not(target_arch = "wasm32"))]
                            if app.needs_surface_recreate {
                                log::info!("Attempting full GPU reinitialization...");

                                // Drop GPU-dependent resources BEFORE reinit
                                // (they hold Arc refs to the dead device)
                                app.flame_renderer = None;

                                match app.gpu.reinit(window.clone()) {
                                    Ok(()) => {
                                        // Recreate all GPU-dependent resources with new device
                                        app.egui_layer.reinit_gpu_resources(
                                            &window, &app.gpu.device, &app.gpu.queue, app.gpu.config.format,
                                        );
                                        let config = app.config_manager.active_config();
                                        app.flame_renderer = Some(FlameRenderer::with_palette_size(
                                            &app.gpu.device,
                                            &app.gpu.queue,
                                            app.gpu.config.format,
                                            app.gpu.size.width,
                                            app.gpu.size.height,
                                            &config.flame,
                                            config.palette_size,
                                        ));
                                        app.effect_chain = crate::renderer::effect_chain::EffectChainRunner::new(
                                            &app.gpu.device,
                                            app.gpu.size.width,
                                            app.gpu.size.height,
                                        );
                                        // Force full GPU state re-sync: the new FlameRenderer
                                        // has default buffers, so we must re-upload everything.
                                        app.config_manager.request_full_resync();
                                        // Force viewport resize on next frame to fix aspect ratio
                                        app.fractal_viewport_size = (0, 0);
                                        app.needs_surface_recreate = false;
                                        log::info!("GPU reinitialized successfully");
                                    }
                                    Err(e) => {
                                        log::error!("GPU reinitialization failed: {:?}", e);
                                        // Stay in needs_surface_recreate state; AboutToWait
                                        // won't spin — it just waits for user events
                                        return;
                                    }
                                }
                            }

                            app.update();
                            match app.render(&window) {
                                Ok(_) => {},
                                Err(SurfaceError::Lost | SurfaceError::Outdated) => {
                                    log::warn!("Surface lost/outdated, scheduling recovery...");
                                    #[cfg(not(target_arch = "wasm32"))]
                                    { app.needs_surface_recreate = true; }
                                    #[cfg(target_arch = "wasm32")]
                                    { app.gpu.resize(app.gpu.size); }
                                }
                                Err(SurfaceError::OutOfMemory) => elwt.exit(),
                                Err(e) => {
                                    log::error!("Surface error: {:?}, scheduling recovery...", e);
                                    #[cfg(not(target_arch = "wasm32"))]
                                    { app.needs_surface_recreate = true; }
                                    #[cfg(target_arch = "wasm32")]
                                    { app.gpu.resize(app.gpu.size); }
                                }
                            }

                            // Handle graceful quit (triggered by File → Quit menu)
                            if app.quit_requested {
                                app.shutdown(elwt);
                            }
                        }
                        _ => {}
                    }
                }
                Event::Resumed => {
                    // Refresh window state when app resumes (wake from sleep, etc.)
                    let size = window.inner_size();
                    if size.width > 0 && size.height > 0 {
                        if app.needs_surface_recreate {
                            log::info!("Resumed event with pending surface recreate");
                        } else {
                            log::info!("Resumed event - resizing to {}x{}", size.width, size.height);
                            app.gpu.resize(size);
                        }
                        window.request_redraw();
                    }
                }
                Event::AboutToWait => {
                    use std::time::Duration;
                    use web_time::Instant;

                    // Check if actively rendering (not paused and under max_iterations)
                    let config = app.config_manager.active_config();
                    let max_iterations = Some(config.max_iterations);
                    let is_rendering = !app.paused && app.flame_renderer.as_ref().map_or(false, |r| {
                        max_iterations.map_or(true, |max| r.total_iterations() < max)
                    });

                    // Check if animation is playing (needs continuous redraws)
                    let animation_playing = app.animation_controller.is_playing();

                    // Check if audio is playing or capturing (needs UI updates for progress/signals)
                    let audio_playing = app.audio_player.state() == crate::audio::PlaybackState::Playing;
                    let audio_capturing = app.audio_capture.is_capturing();

                    // Check if video/PNG export is in progress (needs UI updates for progress bar)
                    let is_exporting = app.animation_export_progress.lock()
                        .map(|p| p.is_exporting)
                        .unwrap_or(false)
                        || app.png_export_progress.lock()
                        .map(|p| p.is_exporting)
                        .unwrap_or(false);

                    // Update present mode based on system settings
                    app.gpu.set_present_mode(app.config_manager.system_settings().vsync_enabled);

                    // Time-based UI idle detection (600ms after last input)
                    const UI_IDLE_TIMEOUT: Duration = Duration::from_millis(700);
                    let ui_active = app.last_input_time
                        .map(|t| t.elapsed() < UI_IDLE_TIMEOUT)
                        .unwrap_or(false);

                    // If surface needs recreation, request one redraw to trigger it
                    // but don't continuously spin — wait for events after that
                    if app.needs_surface_recreate {
                        window.request_redraw();
                        elwt.set_control_flow(ControlFlow::Wait);
                        return;
                    }

                    // EVENT-DRIVEN RENDERING:
                    // Only render when something actually changes
                    // During export, audio playback, or live capture, keep redrawing to update UI
                    if is_rendering || animation_playing || audio_playing || audio_capturing || ui_active || is_exporting {
                        // Actively rendering fractals OR UI is active (for tooltips, hover effects)
                        if app.config_manager.system_settings().vsync_enabled {
                            // VSync enabled: render continuously, let VSync cap frame rate
                            window.request_redraw();
                        } else {
                            // VSync disabled: manually limit to target FPS
                            let target_frame_time = Duration::from_secs_f32(1.0 / app.config_manager.system_settings().target_fps);
                            let now = Instant::now();
                            if let Some(last_frame) = app.last_frame_time {
                                let elapsed = now.duration_since(last_frame);
                                if elapsed >= target_frame_time {
                                    window.request_redraw();
                                } else {
                                    let wait_until = last_frame + target_frame_time;
                                    elwt.set_control_flow(ControlFlow::WaitUntil(wait_until));
                                }
                            } else {
                                window.request_redraw();
                            }
                        }
                    } else {
                        // Truly idle: sleep until event wakes us
                        elwt.set_control_flow(ControlFlow::Wait);
                    }
                }
                _ => {}
            }
        })?;

        Ok(())
    }

    fn update(&mut self) {
        // Update performance metrics
        self.metrics.update();
    }
    fn render(&mut self, window: &Window) -> Result<(), SurfaceError> {
        use web_time::Instant;

        // Sync fullscreen state with browser (WASM only)
        // Detects when browser exits fullscreen via its own Esc handler
        self.sync_fullscreen_state();

        // Skip rendering if window is minimized (size is 0)
        // This prevents surface errors and wasted GPU work
        if self.gpu.size.width == 0 || self.gpu.size.height == 0 {
            return Ok(());
        }

        let render_start = Instant::now();

        // Calculate delta time BEFORE updating last_frame_time (for animation)
        let delta_time = self.last_frame_time
            .map(|last| render_start.duration_since(last).as_secs_f64())
            .unwrap_or(1.0 / 60.0);

        self.last_frame_time = Some(render_start);

        // ============================================================================
        // NEW FRAME ORDER (Fixed race conditions):
        // 1. Render UI (reads current state, shows previous frame's fractal)
        // 2. Process all UI responses and config updates
        // 3. Get FINAL config after all updates
        // 4. Compute/accumulate/tonemap (generates new fractal with updated config)
        // 5. Submit and present
        // ============================================================================

        // Check if video export is in progress
        let is_video_exporting = self.animation_export_progress.lock()
            .map(|p| p.is_exporting)
            .unwrap_or(false);

        // During video export: completely skip rendering to avoid surface corruption
        // The export uses its own GPU device which interferes with surface acquisition
        if is_video_exporting {
            // Update window title with export progress
            if let Ok(progress) = self.animation_export_progress.lock() {
                let percent = if progress.total_frames > 0 {
                    (progress.current_frame * 100) / progress.total_frames
                } else {
                    0
                };
                let title = format!(
                    "⏳ Exporting {}/{} ({}%) - Fractal Flame",
                    progress.current_frame,
                    progress.total_frames,
                    percent
                );
                window.set_title(&title);
            }
            // Sleep to avoid busy-waiting
            std::thread::sleep(std::time::Duration::from_millis(100));
            self.was_video_exporting = true;
            return Ok(());
        }

        // Detect when video export has just finished
        if self.was_video_exporting {
            log::info!("Video export finished, reconfiguring surface...");
            // Give driver time to release export device
            std::thread::sleep(std::time::Duration::from_millis(200));
            // Reconfigure surface to ensure clean state
            self.gpu.resize(self.gpu.size);
            // Restore window title
            window.set_title("Fractal Flame");
            self.was_video_exporting = false;
        }

        // Normal rendering: acquire surface texture
        let frame = match self.gpu.surface.get_current_texture() {
            Ok(f) => f,
            Err(e) => return Err(e),
        };
        let surface_view = frame.texture.create_view(&egui_wgpu::wgpu::TextureViewDescriptor::default());
        let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
            label: Some("Render Encoder"),
        });

        // ============================================================================
        // PHASE 1: Render UI First
        // ============================================================================
        // UI displays PREVIOUS frame's fractal while we prepare CURRENT frame
        let t_ui_start = Instant::now();
        let can_undo = self.can_undo();
        let can_redo = self.can_redo();

        // Register the appropriate texture with egui for display:
        // - If effects ran on previous frame, use the effect output texture
        // - Otherwise, use the renderer's fractal texture
        if let Some(ref renderer) = self.flame_renderer {
            // Check if we have enabled color effects
            let config = self.config_manager.active_config();
            let has_enabled_effects = config.color_effects.iter().any(|e| e.enabled);
            let has_enabled_density_effects = config.density_effects.iter().any(|e| e.enabled);

            // Ensure effect chain dimensions match renderer (defensive sync)
            if has_enabled_effects || has_enabled_density_effects {
                self.effect_chain.resize(&self.gpu.device, renderer.width, renderer.height);
            }

            // Use effect output if available and effects are enabled
            let texture_view = if has_enabled_effects {
                self.effect_chain.get_color_output()
            } else {
                None
            };

            let texture_view = texture_view.unwrap_or_else(|| renderer.get_fractal_texture_view());

            self.egui_layer.register_fractal_texture(
                &self.gpu.device,
                texture_view,
                self.fractal_viewport_size.0,
                self.fractal_viewport_size.1,
            );
        }

        // Get a snapshot of export progress for UI display
        // Use ok() + unwrap_or_default() to handle poisoned mutexes gracefully
        let export_progress = self.animation_export_progress.lock()
            .ok()
            .map(|p| p.clone())
            .unwrap_or_default();
        let png_export_progress = self.png_export_progress.lock()
            .ok()
            .map(|p| p.clone())
            .unwrap_or_default();

        // Get signal names for track editor dropdown
        let signal_names: Vec<String> = self.signal_manager.signal_names();

        // Pass current connectivity state to UI layer
        self.egui_layer.api_connectivity = self.api_connectivity;

        let ui_response = self.egui_layer.render_ui(
            &self.gpu.device,
            &self.gpu.queue,
            &mut encoder,
            &surface_view,
            window,
            self.gpu.size,
            &self.metrics,
            &mut self.config_manager,
            self.flame_renderer.as_mut(),
            &mut self.flame,
            &mut self.palette_library,
            &self.preset_library,
            &mut self.animation_controller,
            &mut self.paused,
            &mut self.view_subflame_in_isolation,
            &mut self.quit_requested,
            can_undo,
            can_redo,
            &mut self.workspace,
            &mut self.export_width,
            &mut self.export_height,
            &mut self.use_custom_export_size,
            &export_progress,
            &png_export_progress,
            self.ui_hidden,
            &mut self.audio_manager,
            &mut self.audio_player,
            &mut self.audio_capture,
            &mut self.signal_manager,
            &signal_names,
            &self.api_state,
            self.current_user_id.as_deref(),
        );

        self.metrics.record_ui_time(t_ui_start.elapsed().as_secs_f64() * 1000.0);

        // Track UI repaint requests for frame rate optimization
        self.ui_needs_repaint = ui_response.needs_repaint;

        // Handle viewport resize immediately (before rendering)
        // WASM: Debounce resizes to prevent rapid resize loops that can freeze the browser
        if let Some(viewport_size) = ui_response.fractal_viewport_size {
            #[cfg(target_arch = "wasm32")]
            let should_resize = {
                let now = web_time::Instant::now();
                // 100ms debounce - safe even at 10 FPS while still feeling responsive
                let debounce_ok = self.last_viewport_resize_time
                    .map(|t| now.duration_since(t).as_millis() > 100)
                    .unwrap_or(true);
                viewport_size != self.fractal_viewport_size && debounce_ok
            };

            #[cfg(not(target_arch = "wasm32"))]
            let should_resize = viewport_size != self.fractal_viewport_size;

            if should_resize {
                log::info!("Fractal viewport resize: {:?} → {:?}", self.fractal_viewport_size, viewport_size);
                self.fractal_viewport_size = viewport_size;

                #[cfg(target_arch = "wasm32")]
                {
                    self.last_viewport_resize_time = Some(web_time::Instant::now());
                }

                // Resize renderer to match new viewport dimensions
                if let Some(ref mut renderer) = self.flame_renderer {
                    // Get config for resize
                    let resize_config = self.config_manager.active_config();
                    let mut resize_encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                        label: Some("Viewport Resize Encoder"),
                    });

                    // Pick the flame the renderer should be configured
                    // against — same routing as `process_gpu_updates`.
                    // When the user has ticked "View subflame in
                    // isolation" while editing a subflame, resize must
                    // build buffers + bind groups for that subflame,
                    // not the main flame. Otherwise resize hands the
                    // main flame to renderer.resize() and
                    // update_path_features(), and both run shader-cache
                    // checks that recompile the shader for the main —
                    // wiping the in-isolation shader and producing the
                    // single-pixel chaos-game-stuck rendering.
                    use crate::config::manager::EditingTarget;
                    let resize_source: &crate::scene::transforms::Flame = match self
                        .config_manager
                        .editing_target()
                    {
                        EditingTarget::Subflame { index }
                            if self.view_subflame_in_isolation
                                && index < self.flame.subflames.len() =>
                        {
                            &self.flame.subflames[index]
                        }
                        _ => &self.flame,
                    };

                    renderer.resize(&self.gpu.device, &mut resize_encoder, &self.gpu.queue, viewport_size.0, viewport_size.1,
                        resize_source, self.config_manager.system_settings().iterations_per_thread, resize_config.zoom, resize_config.pan_x, resize_config.pan_y, resize_config.rotation,
                        resize_config.camera_rotation_x, resize_config.camera_rotation_y, resize_config.camera_z, resize_config.speed_factor);
                    self.gpu.queue.submit(std::iter::once(resize_encoder.finish()));

                    // Resize effect chain textures
                    self.effect_chain.resize(&self.gpu.device, viewport_size.0, viewport_size.1);

                    // Restore palette and color mode after buffer recreation
                    renderer.update_palette(&self.gpu.device, &self.gpu.queue, &resize_config.palette, resize_config.palette_rotation, resize_config.palette_squeeze, resize_config.palette_squeeze_mode, resize_config.palette_squeeze_falloff, resize_config.palette_log_strength, resize_config.palette_reverse);
                    renderer.set_color_mode(&self.gpu.queue, resize_config.color_mode, self.config_manager.system_settings().iterations_per_thread, self.config_manager.system_settings().burn_in,
                        resize_config.zoom, resize_config.pan_x, resize_config.pan_y, resize_config.rotation,
                        resize_config.camera_rotation_x, resize_config.camera_rotation_y, resize_config.camera_z, resize_config.speed_factor);
                    renderer.set_path_map_style(resize_config.path_map_style);

                    // Update path buffer allocation based on color_mode and filters (after resize recreates buffers).
                    // Same routing as above — must use resize_source so the internal shader-cache check
                    // doesn't recompile back to the main flame's spec.
                    renderer.update_path_features(&self.gpu.device, &self.gpu.queue, resize_source);

                    // Restore tonemap parameters after buffer recreation (not in live preview mode)
                    renderer.update_tonemap(&self.gpu.queue, resize_config.tonemap_mode, resize_config.use_curve, resize_config.exposure, resize_config.gamma,
                        resize_config.gamma_threshold, resize_config.brightness, resize_config.vibrancy, resize_config.saturation, resize_config.hue_shift,
                        resize_config.alpha_blend_low, resize_config.alpha_blend_high,
                        viewport_size.0, viewport_size.1, renderer.total_iterations(), resize_config.max_iterations, resize_config.zoom, self.config_manager.system_settings().iterations_per_thread, 1, false,
                        resize_config.levels_low, resize_config.levels_high, resize_config.levels_gamma);
                    renderer.update_curve_lut(&self.gpu.queue, &resize_config.tonemap_curve);

                    // Re-register texture with egui after resize (new texture view created)
                    self.egui_layer.register_fractal_texture(
                        &self.gpu.device,
                        renderer.get_fractal_texture_view(),
                        viewport_size.0,
                        viewport_size.1,
                    );
                }
            }
        }

        // Submit UI rendering (must happen before we start processing responses)
        let t_submit = Instant::now();

        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        self.metrics.record_submit_time(t_submit.elapsed().as_secs_f64() * 1000.0);

        // ============================================================================
        // PHASE 2: Process ALL UI Responses and Config Updates
        // ============================================================================

        // Handle UI responses via extracted handlers
        self.handle_ui_responses(&ui_response);

        // Poll API health check and trigger periodic checks
        self.poll_health_check();

                // Handle PNG export
        if ui_response.png_export_with_background || ui_response.png_export_transparent {
            let transparent = ui_response.png_export_transparent;

            #[cfg(not(target_arch = "wasm32"))]
            {
                // Desktop: use blocking task for both capture and save
                // Build metadata before borrowing renderer
                let export_config = self.export_config();
                let render_time_ms = self.metrics.render_time_ms;

                // Check if we need custom-size export
                if self.use_custom_export_size {
                    // Custom-size export: create temporary renderer at export dimensions
                    self.export_custom_size(transparent, export_config, render_time_ms);
                } else if let Some(ref mut renderer) = self.flame_renderer {
                    // Viewport-size export: use current renderer
                    let total_iterations = renderer.total_iterations();
                    let has_color_effects = export_config.color_effects.iter().any(|e| e.enabled);

                    // For transparent export, we need to re-run tonemap with transparent_mode=1
                    // and then re-run color effects if enabled
                    if transparent {
                        let iterations_per_thread = self.config_manager.system_settings().iterations_per_thread;
                        renderer.set_transparent_mode(&self.gpu.queue, true, &export_config, iterations_per_thread);

                        // Run tonemap pass with transparent mode
                        let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                            label: Some("Transparent Export Tonemap"),
                        });
                        renderer.tonemap_pass(&self.gpu.queue, &mut encoder);

                        // Re-run color effects if enabled (they need to process the new tonemapped output)
                        if has_color_effects {
                            self.effect_chain.reset_slots();
                            self.effect_chain.run_color_effects(
                                &self.gpu.device,
                                &self.gpu.queue,
                                &mut encoder,
                                renderer.get_fractal_texture_view(),
                                &export_config.color_effects,
                            );
                        }

                        self.gpu.queue.submit(std::iter::once(encoder.finish()));
                    }

                    // Read pixels from effect chain output if color effects are enabled,
                    // otherwise read from renderer's fractal texture
                    let pixels_result: Result<(u32, u32, Vec<u8>), String> = if has_color_effects {
                        pollster::block_on(
                            self.effect_chain.read_color_output_pixels(&self.gpu.device, &self.gpu.queue)
                        )
                    } else {
                        pollster::block_on(
                            renderer.read_fractal_pixels(&self.gpu.device, &self.gpu.queue, transparent, export_config.background_color)
                        ).map_err(|e| e.to_string())
                    };

                    match pixels_result {
                        Ok((width, height, rgba_data)) => {
                            // Build metadata with captured values
                            let metadata = crate::png_metadata::PngMetadata::from_app_state(
                                width,
                                height,
                                total_iterations,
                                render_time_ms,
                                self.config_manager.system_settings().iterations_per_thread,
                                export_config.speed_factor,
                                &export_config,
                            );

                            // Encode PNG with metadata
                            match crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data, Some(metadata)) {
                                Ok(png_data) => {
                                    // Open file dialog
                                    if let Some(path) = rfd::FileDialog::new()
                                        .add_filter("PNG Image", &["png"])
                                        .set_file_name("fractal.png")
                                        .save_file()
                                    {
                                        if let Err(e) = std::fs::write(&path, png_data) {
                                            eprintln!("Failed to save PNG: {}", e);
                                        } else {
                                            println!("PNG saved to: {}", path.display());
                                        }
                                    }
                                }
                                Err(e) => eprintln!("Failed to encode PNG: {}", e),
                            }
                        }
                        Err(e) => eprintln!("Failed to capture pixels: {}", e),
                    }

                    // Reset transparent mode back to normal for display
                    if transparent {
                        let iterations_per_thread = self.config_manager.system_settings().iterations_per_thread;
                        renderer.set_transparent_mode(&self.gpu.queue, false, &export_config, iterations_per_thread);

                        // Run tonemap pass to restore normal display
                        let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                            label: Some("Restore Normal Tonemap"),
                        });
                        renderer.tonemap_pass(&self.gpu.queue, &mut encoder);

                        // Re-run color effects with normal tonemap output
                        if has_color_effects {
                            self.effect_chain.reset_slots();
                            self.effect_chain.run_color_effects(
                                &self.gpu.device,
                                &self.gpu.queue,
                                &mut encoder,
                                renderer.get_fractal_texture_view(),
                                &export_config.color_effects,
                            );
                        }

                        self.gpu.queue.submit(std::iter::once(encoder.finish()));
                    }
                }
            }

            #[cfg(target_arch = "wasm32")]
            {
                // WASM: Use async task for pixel reading and file save
                use wasm_bindgen_futures::spawn_local;
                use crate::renderer::compute_kernel::FlameRenderer;

                let export_config = self.export_config();
                let iterations_per_thread = self.config_manager.system_settings().iterations_per_thread;
                let background_color = export_config.background_color;

                if self.use_custom_export_size {
                    // Custom size export: create temporary renderer and render
                    let export_width = self.export_width;
                    let export_height = self.export_height;
                    let max_iterations = export_config.max_iterations;

                    log::info!("WASM: Exporting at custom size {}×{}", export_width, export_height);

                    // Create temporary renderer at export dimensions
                    let surface_format = egui_wgpu::wgpu::TextureFormat::Rgba8Unorm;
                    let mut temp_renderer = FlameRenderer::new(
                        &self.gpu.device,
                        &self.gpu.queue,
                        surface_format,
                        export_width,
                        export_height,
                        &export_config.flame,
                    );

                    // Load config into temp renderer
                    let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                        label: Some("WASM Custom Export Encoder"),
                    });

                    temp_renderer.load_config(&self.gpu.device, &mut encoder, &self.gpu.queue, &export_config, &export_config.palette, iterations_per_thread, 20); // burn_in - use default for WASM export
                    self.gpu.queue.submit(std::iter::once(encoder.finish()));

                    // Render frames until we reach max_iterations
                    let render_start = web_time::Instant::now();
                    let mut total_rendered = 0u64;

                    const NUM_WORKGROUPS: u32 = 128;
                    const THREADS_PER_WORKGROUP: u64 = 64;
                    const BATCH_SIZE: u32 = 4;

                    let mut batch_frame_count = 0;

                    while total_rendered < max_iterations {
                        let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                            label: Some("WASM Export Render Frame"),
                        });

                        let clear_histogram = batch_frame_count == 0;
                        // Clear paths only on very first batch of the entire export
                        let clear_paths = total_rendered == 0 && clear_histogram;

                        temp_renderer.compute_pass(
                            &mut encoder,
                            &self.gpu.queue,
                            NUM_WORKGROUPS,
                            iterations_per_thread,
                            20, // burn_in - use default for WASM export
                            export_config.zoom,
                            export_config.pan_x,
                            export_config.pan_y,
                            export_config.rotation,
                            export_config.camera_rotation_x,
                            export_config.camera_rotation_y,
                            export_config.camera_z,
                            export_config.speed_factor,
                            clear_histogram,
                            clear_paths,
                        );

                        let samples_this_frame = NUM_WORKGROUPS as u64 * THREADS_PER_WORKGROUP * iterations_per_thread as u64;
                        total_rendered += samples_this_frame;
                        batch_frame_count += 1;

                        if batch_frame_count >= BATCH_SIZE {
                            let total_samples_in_batch = samples_this_frame * BATCH_SIZE as u64;
                            temp_renderer.accumulate_pass(&mut encoder, &self.gpu.queue, &self.gpu.device, total_samples_in_batch);
                            batch_frame_count = 0;
                        }

                        self.gpu.queue.submit(std::iter::once(encoder.finish()));

                        if total_rendered >= max_iterations {
                            if batch_frame_count > 0 {
                                let mut final_encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                                    label: Some("WASM Export Final Accumulation"),
                                });
                                let total_samples_in_batch = samples_this_frame * batch_frame_count as u64;
                                temp_renderer.accumulate_pass(&mut final_encoder, &self.gpu.queue, &self.gpu.device, total_samples_in_batch);
                                self.gpu.queue.submit(std::iter::once(final_encoder.finish()));
                            }
                            break;
                        }
                    }

                    let render_time_ms = render_start.elapsed().as_secs_f64() * 1000.0;

                    // Set transparent mode if requested
                    if transparent {
                        temp_renderer.set_transparent_mode(&self.gpu.queue, true, &export_config, iterations_per_thread);
                    }

                    // Final tonemap pass
                    let mut final_encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                        label: Some("WASM Export Final Tonemap"),
                    });
                    temp_renderer.tonemap_pass(&self.gpu.queue, &mut final_encoder);
                    self.gpu.queue.submit(std::iter::once(final_encoder.finish()));

                    // Run color effects if enabled
                    let has_color_effects = export_config.color_effects.iter().any(|e| e.enabled);
                    let mut effect_chain: Option<crate::renderer::effect_chain::EffectChainRunner> = None;
                    let color_effects_ran = if has_color_effects {
                        log::info!("WASM custom export: Running {} color effect(s)",
                            export_config.color_effects.iter().filter(|e| e.enabled).count());

                        let mut chain = crate::renderer::effect_chain::EffectChainRunner::new(
                            &self.gpu.device, export_width, export_height);

                        let mut effect_encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                            label: Some("WASM Export Color Effects"),
                        });

                        chain.reset_slots();
                        let ran = chain.run_color_effects(
                            &self.gpu.device,
                            &self.gpu.queue,
                            &mut effect_encoder,
                            temp_renderer.get_fractal_texture_view(),
                            &export_config.color_effects,
                        );

                        self.gpu.queue.submit(std::iter::once(effect_encoder.finish()));
                        effect_chain = Some(chain);
                        ran
                    } else {
                        false
                    };

                    // Move renderer to heap for async task
                    let temp_renderer = Box::new(temp_renderer);
                    let speed_factor = export_config.speed_factor;

                    // Clone Arc handles for the async task
                    let device = self.gpu.device.clone();
                    let queue = self.gpu.queue.clone();

                    // Read from effect chain if effects ran, otherwise from renderer
                    if color_effects_ran {
                        if let Some(chain) = effect_chain {
                            let chain = Box::new(chain);
                            spawn_local(async move {
                                match chain.read_color_output_pixels(&device, &queue).await {
                                    Ok((width, height, rgba_data)) => {
                                        let metadata = crate::png_metadata::PngMetadata::from_app_state(
                                            width,
                                            height,
                                            total_rendered,
                                            render_time_ms,
                                            iterations_per_thread,
                                            speed_factor,
                                            &export_config,
                                        );

                                        match crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data, Some(metadata)) {
                                            Ok(png_data) => {
                                                match trigger_browser_download(&png_data, "fractal.png", "image/png") {
                                                    Ok(()) => log::info!("PNG download started: {}×{} in {:.2}s", width, height, render_time_ms / 1000.0),
                                                    Err(e) => log::error!("Failed to trigger download: {}", e),
                                                }
                                            }
                                            Err(e) => log::error!("Failed to encode PNG: {}", e),
                                        }
                                    }
                                    Err(e) => log::error!("Failed to capture effect pixels: {}", e),
                                }
                            });
                        }
                    } else {
                        spawn_local(async move {
                            match temp_renderer.read_fractal_pixels(&device, &queue, transparent, background_color).await {
                                Ok((width, height, rgba_data)) => {
                                    let metadata = crate::png_metadata::PngMetadata::from_app_state(
                                        width,
                                        height,
                                        total_rendered,
                                        render_time_ms,
                                        iterations_per_thread,
                                        speed_factor,
                                        &export_config,
                                    );

                                    match crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data, Some(metadata)) {
                                        Ok(png_data) => {
                                            match trigger_browser_download(&png_data, "fractal.png", "image/png") {
                                                Ok(()) => log::info!("PNG download started: {}×{} in {:.2}s", width, height, render_time_ms / 1000.0),
                                                Err(e) => log::error!("Failed to trigger download: {}", e),
                                            }
                                        }
                                        Err(e) => log::error!("Failed to encode PNG: {}", e),
                                    }
                                }
                                Err(e) => log::error!("Failed to capture pixels: {}", e),
                            }
                        });
                    }
                } else if let Some(ref mut renderer) = self.flame_renderer {
                    // Viewport size export: use current renderer
                    //
                    // IMPORTANT: We must read pixels BEFORE spawning the async task, because
                    // the next frame will overwrite fractal_texture with a new render.
                    // The async task is only used for the file dialog and write.

                    let total_iterations = renderer.total_iterations();
                    let render_time_ms = self.metrics.render_time_ms;
                    let speed_factor = export_config.speed_factor;
                    let has_color_effects = export_config.color_effects.iter().any(|e| e.enabled);

                    // For transparent export, set transparent mode and run tonemap before reading
                    // Also re-run color effects if enabled
                    if transparent {
                        renderer.set_transparent_mode(&self.gpu.queue, true, &export_config, iterations_per_thread);

                        let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                            label: Some("Transparent Export Tonemap"),
                        });
                        renderer.tonemap_pass(&self.gpu.queue, &mut encoder);

                        // Re-run color effects if enabled
                        if has_color_effects {
                            self.effect_chain.reset_slots();
                            self.effect_chain.run_color_effects(
                                &self.gpu.device,
                                &self.gpu.queue,
                                &mut encoder,
                                renderer.get_fractal_texture_view(),
                                &export_config.color_effects,
                            );
                        }

                        self.gpu.queue.submit(std::iter::once(encoder.finish()));
                    }

                    // Read pixels NOW, before the next frame overwrites the texture
                    // Use effect chain output if color effects are enabled
                    let width = renderer.width;
                    let height = renderer.height;

                    let (staging_buffer, padded_bytes_per_row) = if has_color_effects && self.effect_chain.has_color_output() {
                        self.effect_chain.create_color_staging_buffer(&self.gpu.device)
                    } else {
                        renderer.create_pixel_staging_buffer(&self.gpu.device)
                    };

                    // Copy texture to staging buffer
                    let mut copy_encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
                        label: Some("Viewport Export Copy"),
                    });

                    if has_color_effects && self.effect_chain.has_color_output() {
                        self.effect_chain.copy_color_to_buffer(&mut copy_encoder, &staging_buffer, padded_bytes_per_row);
                    } else {
                        renderer.copy_fractal_to_buffer(&mut copy_encoder, &staging_buffer, padded_bytes_per_row);
                    }
                    self.gpu.queue.submit(std::iter::once(copy_encoder.finish()));

                    // Clone Arc handle for the async task
                    let device = self.gpu.device.clone();

                    spawn_local(async move {
                        // Map the staging buffer (this is the async part)
                        let buffer_slice = staging_buffer.slice(..);
                        let (tx, rx) = futures::channel::oneshot::channel();
                        buffer_slice.map_async(egui_wgpu::wgpu::MapMode::Read, move |result| {
                            let _ = tx.send(result);
                        });
                        let _ = device.poll(egui_wgpu::wgpu::PollType::Wait { submission_index: None, timeout: None });

                        match rx.await {
                            Ok(Ok(())) => {
                                let data = buffer_slice.get_mapped_range();

                                // Extract RGBA data from staging buffer (handle row padding)
                                let bytes_per_pixel = 4u32;
                                let unpadded_bytes_per_row = (width * bytes_per_pixel) as usize;
                                let mut rgba_data = Vec::with_capacity((width * height * 4) as usize);

                                for y in 0..height {
                                    let row_start = (y * padded_bytes_per_row) as usize;
                                    // Only copy the actual pixel data, not the padding
                                    let row_data = &data[row_start..row_start + unpadded_bytes_per_row];
                                    // For opaque export, the shader already blended with background (transparent_mode=0)
                                    // For transparent export, we ran tonemap with transparent_mode=1
                                    rgba_data.extend_from_slice(row_data);
                                }
                                drop(data);
                                staging_buffer.unmap();

                                // Build metadata and encode PNG
                                let metadata = crate::png_metadata::PngMetadata::from_app_state(
                                    width,
                                    height,
                                    total_iterations,
                                    render_time_ms,
                                    iterations_per_thread,
                                    speed_factor,
                                    &export_config,
                                );

                                match crate::renderer::compute_kernel::encode_png_from_rgba(width, height, rgba_data, Some(metadata)) {
                                    Ok(png_data) => {
                                        // Trigger direct browser download
                                        match trigger_browser_download(&png_data, "fractal.png", "image/png") {
                                            Ok(()) => log::info!("PNG download started!"),
                                            Err(e) => log::error!("Failed to trigger download: {}", e),
                                        }
                                    }
                                    Err(e) => log::error!("Failed to encode PNG: {}", e),
                                }
                            }
                            Ok(Err(e)) => log::error!("Buffer map error: {:?}", e),
                            Err(_) => log::error!("Failed to receive buffer map result"),
                        }
                    });
                }
            }
        }

        // Handle animation export request
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(ref export_settings) = ui_response.animation_export_requested {
            // Check if already exporting (handle poisoned mutex gracefully)
            let already_exporting = self.animation_export_progress.lock()
                .map(|p| p.is_exporting)
                .unwrap_or(false);
            if already_exporting {
                log::warn!("Animation export already in progress");
            } else if let Some(ref animation) = self.animation_controller.animation {
                use crate::animation::export::{AnimationExportConfig, UiProgressCallback, export_animation_fast, VideoEncodingSettings};

                // Clone config and override max_iterations from export settings
                let mut config = self.config_manager.active_config().clone();
                config.max_iterations = export_settings.max_iterations;

                let export_config = AnimationExportConfig {
                    config,
                    animation: animation.clone(),
                    output_path: export_settings.output_path.clone(),
                    width: export_settings.width,
                    height: export_settings.height,
                    fps: export_settings.fps,
                    iterations_per_thread: export_settings.iterations_per_thread,
                    video_settings: VideoEncodingSettings {
                        codec: export_settings.video_codec,
                        hardware_accel: export_settings.hardware_accel,
                        quality: export_settings.video_quality,
                        preset: export_settings.preset,
                        tune: export_settings.tune,
                    },
                    audio: export_settings.audio_file.as_ref().map(|path| {
                        crate::animation::export::AudioExportConfig {
                            file: path.clone(),
                            offset: export_settings.audio_offset,
                            fade_in: export_settings.audio_fade_in,
                            fade_out: export_settings.audio_fade_out,
                            bitrate_kbps: export_settings.audio_bitrate,
                        }
                    }),
                    signals: self.signal_manager.clone_signals(),
                };

                println!("Starting animation export (background thread)...");
                println!("  Output: {}", export_config.output_path.display());
                println!("  Resolution: {}x{} @ {} FPS", export_config.width, export_config.height, export_config.fps);
                println!("  Total frames: {}", export_config.total_frames());
                println!("  Codec: {} (CRF {})", export_config.video_settings.codec.display_name(), export_config.video_settings.quality);

                // Set initial export progress (handle poisoned mutex gracefully)
                if let Ok(mut p) = self.animation_export_progress.lock() {
                    p.is_exporting = true;
                    p.current_frame = 0;
                    p.total_frames = export_config.total_frames();
                    p.seconds_per_frame = 0.0;
                    p.status = "Starting export...".to_string();
                }

                // Clone progress Arc for the background thread
                let progress_arc = Arc::clone(&self.animation_export_progress);

                // Spawn background thread for export
                std::thread::spawn(move || {
                    let mut progress = UiProgressCallback::new(Arc::clone(&progress_arc));

                    match pollster::block_on(export_animation_fast(export_config, &mut progress)) {
                        Ok(result) => {
                            println!("\nAnimation export complete!");
                            println!("  {} frames in {:.1}s", result.total_frames, result.total_time_ms / 1000.0);
                            println!("  Output: {}", result.output_path.display());

                            // Mark export complete
                            if let Ok(mut p) = progress_arc.lock() {
                                p.is_exporting = false;
                                p.status = format!("Complete: {}", result.output_path.display());
                            }
                        }
                        Err(e) => {
                            eprintln!("Animation export failed: {}", e);
                            if let Ok(mut p) = progress_arc.lock() {
                                p.is_exporting = false;
                                p.status = format!("Failed: {}", e);
                            }
                        }
                    }
                });
            } else {
                eprintln!("No animation loaded for export");
            }
        }

        // ============================================================================
        // ANIMATION UPDATE (before GPU updates so animation changes are included)
        // ============================================================================
        let is_controller_playing = self.update_animation(delta_time);


        // ============================================================================
        // GPU UPDATES (includes both UI and animation changes)
        // ============================================================================
        // Process pending config actions and update GPU buffers
        let view_changed_by_keyboard = self.view_changed_by_keyboard;
        self.process_gpu_updates(view_changed_by_keyboard);

        // Clear keyboard flag for next frame
        self.view_changed_by_keyboard = false;


        // ============================================================================
        // PHASE 3: Get FINAL Config and Render Fractal
        // ============================================================================
        // Single config read after all updates are complete
        let final_config = self.config_manager.active_config();

        // Create new encoder for rendering phase
        let mut render_encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
            label: Some("Fractal Render Encoder"),
        });

        // Determine overwrite mode (smooth transitions during parameter changes)
        // Must be computed before mutable borrow of flame_renderer
        let use_overwrite = self.should_use_overwrite();

        // Run flame compute shader with progressive refinement
        if let Some(ref mut renderer) = self.flame_renderer {
            renderer.set_overwrite_mode(use_overwrite);

            // Check if we should continue iterating
            // During animation playback, always iterate (ignore max_iterations limit)
            // Skip GPU work during video/PNG export to avoid GPU contention (separate device in background thread)
            let is_video_exporting = self.animation_export_progress.lock()
                .map(|p| p.is_exporting)
                .unwrap_or(false);
            let is_png_exporting = self.png_export_progress.lock()
                .map(|p| p.is_exporting)
                .unwrap_or(false);
            let max_iterations = Some(final_config.max_iterations);
            let should_iterate = !self.paused && !is_video_exporting && !is_png_exporting && (
                is_controller_playing ||
                max_iterations.map_or(true, |max| renderer.total_iterations() < max)
            );

            // Mark rendering as complete the frame after max_iterations is reached
            if !should_iterate && !self.rendering_complete {
                self.rendering_complete = true;
                log::debug!("Rendering complete: max_iterations reached");
            }

            if should_iterate {
                const NUM_WORKGROUPS: u32 = 128;

                self.frames_since_accumulation += 1;

                // Determine if we should accumulate this frame
                // During overwrite mode, accumulate every frame for smooth transitions
                // During normal accumulation, batch to reduce GPU overhead
                let batch_size = if use_overwrite { 1 } else { self.accumulation_batch_size };
                let should_accumulate = self.frames_since_accumulation >= batch_size;

                let t_compute = Instant::now();
                // 1. Compute new samples with fresh random seed
                // Clear histogram only when starting a new batch (frame 1 of batch)
                let clear_histogram = self.frames_since_accumulation == 1;
                // Clear paths only on full reset (not every batch)
                let clear_paths = self.clear_paths_next_frame;
                if clear_paths {
                    self.clear_paths_next_frame = false;  // Reset flag after use
                }

                let samples_this_frame = renderer.compute_pass(&mut render_encoder, &self.gpu.queue, NUM_WORKGROUPS,
                    self.config_manager.system_settings().iterations_per_thread, self.config_manager.system_settings().burn_in,
                    final_config.zoom, final_config.pan_x, final_config.pan_y, final_config.rotation,
                    final_config.camera_rotation_x, final_config.camera_rotation_y, final_config.camera_z, final_config.speed_factor, clear_histogram, clear_paths);

                self.metrics.record_compute_time(t_compute.elapsed().as_secs_f64() * 1000.0);

                let t_accumulate = Instant::now();
                // 2. Accumulate samples - but only every N frames if batching enabled
                if should_accumulate {
                    // samples_this_frame is only THIS frame's samples, but histogram contains
                    // accumulated samples from all frames in the batch
                    // Pass total samples for proper blend_factor calculation
                    let total_samples_in_batch = samples_this_frame * batch_size as u64;

                    renderer.accumulate_pass(&mut render_encoder, &self.gpu.queue, &self.gpu.device, total_samples_in_batch);

                    self.frames_since_accumulation = 0;
                    self.metrics.record_accumulate_time(t_accumulate.elapsed().as_secs_f64() * 1000.0);
                } else {
                    self.metrics.record_accumulate_time(0.0);
                }
            } else {
                self.metrics.record_compute_time(0.0);
                self.metrics.record_accumulate_time(0.0);
            }
            
            let t_tonemap = Instant::now();
            // 3. Update accumulation parameters from config
            renderer.set_blend_factor(final_config.blend_factor);
            renderer.set_use_dynamic_blend(final_config.use_dynamic_blend);
            renderer.set_target_iterations_per_pixel(final_config.target_iterations_per_pixel);

            // 4. Update tonemap parameters and render to fractal texture
            renderer.update_density_scale(&self.gpu.queue, final_config.density_scale);
            renderer.update_background_color(&self.gpu.queue, final_config.background_color);
            renderer.set_path_map_style(final_config.path_map_style);
            // Calculate batch_size for tonemap (same logic as accumulation)
            let batch_size_for_tonemap = if use_overwrite { 1 } else { self.accumulation_batch_size };

            // Update FSM brightness state and get boost decision
            // The FSM tracks when we're in Animating/Overwrite mode and for how long after
            self.render_mode.update_brightness_state(should_iterate);
            let is_live_preview = self.render_mode.needs_brightness_boost();

            renderer.update_tonemap(&self.gpu.queue, final_config.tonemap_mode, final_config.use_curve,
                final_config.exposure, final_config.gamma, final_config.gamma_threshold, final_config.brightness,
                final_config.vibrancy, final_config.saturation, final_config.hue_shift,
                final_config.alpha_blend_low, final_config.alpha_blend_high,
                renderer.width, renderer.height, renderer.total_iterations(), final_config.max_iterations, final_config.zoom,
                self.config_manager.system_settings().iterations_per_thread, batch_size_for_tonemap, is_live_preview,
                final_config.levels_low, final_config.levels_high, final_config.levels_gamma);

            // Reset effect slot counter for this frame (allows multiple effects with unique params)
            self.effect_chain.reset_slots();

            // Run density effects (before tonemap, on HDR accumulation data)
            let density_effects_ran = self.effect_chain.run_density_effects(
                &self.gpu.device,
                &self.gpu.queue,
                &mut render_encoder,
                renderer.get_accumulation_view(),
                &final_config.density_effects,
            );

            // Render to internal fractal texture with tone mapping
            // If density effects ran, use their output; otherwise use accumulation directly
            if density_effects_ran {
                if let Some(density_output) = self.effect_chain.get_density_output() {
                    renderer.tonemap_pass_with_input(&self.gpu.device, &self.gpu.queue, &mut render_encoder, density_output);
                } else {
                    renderer.tonemap_pass(&self.gpu.queue, &mut render_encoder);
                }
            } else {
                renderer.tonemap_pass(&self.gpu.queue, &mut render_encoder);
            }

            self.metrics.record_tonemap_time(t_tonemap.elapsed().as_secs_f64() * 1000.0);

            // Run color effects (after tonemap)
            let color_effect_count = final_config.color_effects.iter().filter(|e| e.enabled).count();
            let effects_ran = self.effect_chain.run_color_effects(
                &self.gpu.device,
                &self.gpu.queue,
                &mut render_encoder,
                renderer.get_fractal_texture_view(),
                &final_config.color_effects,
            );

            // If effects ran, re-register the effect output texture with egui
            if effects_ran {
                if let Some(output_view) = self.effect_chain.get_color_output() {
                    self.egui_layer.register_fractal_texture(
                        &self.gpu.device,
                        output_view,
                        renderer.width,
                        renderer.height,
                    );
                } else {
                    log::warn!("Effects ran but no output texture available!");
                }
            } else if color_effect_count > 0 {
                // Effects are enabled in config but didn't run - log why
                log::warn!("Config has {} enabled color effects but run_color_effects returned false", color_effect_count);
            }
        }

        // Submit rendering commands
        let t_submit = Instant::now();

        self.gpu.queue.submit(std::iter::once(render_encoder.finish()));
        self.metrics.record_submit_time(t_submit.elapsed().as_secs_f64() * 1000.0);

        // Update density histogram for Levels controls (every ~30 frames)
        // Skip during animation playback to avoid frame drops from GPU readback
        if !self.animation_controller.is_playing() {
            // On WASM, check if an async histogram computation has completed
            #[cfg(target_arch = "wasm32")]
            if let Some(ref slot) = self.histogram_result_slot {
                if let Ok(mut guard) = slot.try_lock() {
                    if let Some(histogram) = guard.take() {
                        self.egui_layer.update_histogram(histogram);
                        self.histogram_in_flight = false;
                    }
                }
            }

            self.histogram_frame_counter += 1;
            if self.histogram_frame_counter >= 30 {
                self.histogram_frame_counter = 0;

                if let Some(ref renderer) = self.flame_renderer {
                    let texture = renderer.accumulation_texture();
                    let total_iterations = renderer.samples_accumulated();

                    // Desktop: blocking readback via pollster
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        match pollster::block_on(crate::renderer::compute_histogram_async(
                            &self.gpu.device,
                            &self.gpu.queue,
                            texture,
                            renderer.width,
                            renderer.height,
                            total_iterations,
                        )) {
                            Ok(histogram) => {
                                self.egui_layer.update_histogram(histogram);
                            }
                            Err(e) => {
                                log::warn!("Failed to compute histogram: {}", e);
                            }
                        }
                    }

                    // WASM: submit GPU copy synchronously, then spawn_local for async readback
                    #[cfg(target_arch = "wasm32")]
                    if !self.histogram_in_flight {
                        let readback = crate::renderer::histogram::submit_histogram_readback(
                            &self.gpu.device,
                            &self.gpu.queue,
                            texture,
                            renderer.width,
                            renderer.height,
                            total_iterations,
                        );

                        let slot = self.histogram_result_slot.get_or_insert_with(|| {
                            std::sync::Arc::new(std::sync::Mutex::new(None))
                        }).clone();
                        self.histogram_in_flight = true;

                        wasm_bindgen_futures::spawn_local(async move {
                            match readback.compute().await {
                                Ok(histogram) => {
                                    if let Ok(mut guard) = slot.lock() {
                                        *guard = Some(histogram);
                                    }
                                }
                                Err(e) => {
                                    log::warn!("Failed to compute histogram: {}", e);
                                }
                            }
                        });
                    }
                }
            }
        }

        // Generate thumbnails for fractal browser (unified panel)
        // Desktop: Blocking generation, one per frame
        // WASM: Async generation via spawn_local
        #[cfg(not(target_arch = "wasm32"))]
        if self.egui_layer.fractal_browser_needs_thumbnails() {
            self.egui_layer.generate_fractal_browser_thumbnail(
                &self.gpu.device,
                &self.gpu.queue,
                &self.palette_library,
            );
            // Request immediate repaint to continue generation next frame
            window.request_redraw();
        }

        #[cfg(target_arch = "wasm32")]
        self.egui_layer.start_fractal_browser_thumbnails(
            &self.gpu.device,
            &self.gpu.queue,
        );

        // Handle PathMap mode: query path at clicked pixel or close overlay
        #[cfg(not(target_arch = "wasm32"))]
        {
            // Check if close was requested
            if self.egui_layer.take_close_path_overlay() {
                self.egui_layer.set_path_click_info(None);
            }

            // Handle new path query
            if let Some((click_x, click_y)) = self.egui_layer.take_clicked_pixel() {
                if let Some(ref renderer) = self.flame_renderer {
                    let config = self.config_manager.active_config();
                    let width = renderer.width;
                    let height = renderer.height;

                    // Clamp to valid pixel coordinates
                    let pixel_x = click_x.min(width - 1);
                    let pixel_y = click_y.min(height - 1);

                    // Read path entry for this specific pixel
                    let path_entry = match pollster::block_on(renderer.read_path_buffer(&self.gpu.device, &self.gpu.queue)) {
                        Ok(path_buffer) => path_buffer[pixel_y as usize][pixel_x as usize],
                        Err(e) => {
                            log::error!("Failed to read path buffer: {}", e);
                            crate::renderer::PathEntry::default()
                        }
                    };

                    // Calculate fractal space coordinates
                    let fractal_coords = Self::pixel_to_fractal(pixel_x, pixel_y, width, height, config);

                    // Read 9x9 color preview from fractal texture
                    let color_preview = match pollster::block_on(
                        renderer.read_pixel_region(&self.gpu.device, &self.gpu.queue, pixel_x, pixel_y, 9, 9)
                    ) {
                        Ok(pixels) => pixels,
                        Err(_) => vec![[0, 0, 0, 255]; 81], // Fallback to black
                    };

                    let click_info = crate::ui::PathClickInfo {
                        click_pixel: (click_x, click_y),
                        found_pixel: (pixel_x, pixel_y),
                        fractal_coords,
                        search_distance: 0.0, // No search, exact pixel
                        path_entry,
                        color_preview,
                        preview_size: (9, 9),
                    };

                    if path_entry.iteration_count > 0 {
                        log::info!("Path at ({}, {}): {:?}", pixel_x, pixel_y, path_entry.to_vec());
                    } else {
                        log::debug!("No path data at ({}, {})", pixel_x, pixel_y);
                    }
                    self.egui_layer.set_path_click_info(Some(click_info));
                }
            }
        }

        let t5 = Instant::now();

        frame.present();

        self.metrics.record_present_time(t5.elapsed().as_secs_f64() * 1000.0);

        self.metrics.record_render_time(render_start.elapsed().as_secs_f64() * 1000.0);

        // Save current low-density state for next frame's brightness decision
        // (matches ping-pong buffer timing - tonemap reads previous frame's buffer)
        self.render_mode.end_frame();

        Ok(())
    }

    /// Graceful shutdown - performs cleanup and exits
    /// Called from: File → Quit, window close button (X), Alt+F4
    fn shutdown(&mut self, event_loop: &ActiveEventLoop) {
        // TODO: Check for unsaved changes
        // TODO: Show confirmation dialog if needed
        // TODO: Perform cleanup tasks

        log::info!("Graceful shutdown initiated");
        event_loop.exit();
    }

    /// Convert pixel coordinates to fractal space coordinates
    /// Takes into account zoom, pan, and rotation
    fn pixel_to_fractal(
        pixel_x: u32,
        pixel_y: u32,
        width: u32,
        height: u32,
        config: &crate::config::FractalConfig,
    ) -> (f32, f32) {
        // Convert pixel to normalized device coordinates (-1 to 1)
        let ndc_x = (pixel_x as f32 / width as f32) * 2.0 - 1.0;
        let ndc_y = (pixel_y as f32 / height as f32) * 2.0 - 1.0;

        // Account for aspect ratio
        let aspect = width as f32 / height as f32;
        let scaled_x = ndc_x * aspect;
        let scaled_y = ndc_y;

        // Apply inverse rotation
        let cos_r = (-config.rotation).cos();
        let sin_r = (-config.rotation).sin();
        let rotated_x = scaled_x * cos_r - scaled_y * sin_r;
        let rotated_y = scaled_x * sin_r + scaled_y * cos_r;

        // Apply inverse zoom and add pan
        let fractal_x = rotated_x / config.zoom + config.pan_x;
        let fractal_y = rotated_y / config.zoom + config.pan_y;

        (fractal_x, fractal_y)
    }
}
