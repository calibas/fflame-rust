//! Application state and event loop

mod input;
mod config;
mod ui_handlers;
mod gpu_updates;
mod animation_update;
mod effect_fetch;
mod variation_fetch;
mod texture_sync;
mod aa_governor;
mod motion_blur;
mod render_progress;
pub mod script_cloud;
mod fly_camera;
pub mod export;
pub mod render_mode;

pub use render_mode::{RenderModeFSM};

#[cfg(not(target_arch = "wasm32"))]
pub use export::export_headless;

// Consumed by `wasm_api`, which is gated on the `web-app` feature.
#[cfg(all(target_arch = "wasm32", feature = "web-app"))]
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
    let on_change = move |_event: web_sys::Event| {
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
        let on_load = move |_event: web_sys::Event| {
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
        };

        // `once_into_js`, NOT `wrap(..).forget()`. The old comment here
        // said the closure "will be cleaned up when reader is done";
        // `forget` never cleans anything up -- it leaks the box and its
        // function-table entry for the life of the page, once per file
        // loaded. A FileReader onload fires exactly once, so a
        // once-closure is both correct and self-freeing: JS owns it
        // until it runs, then the Rust side is dropped.
        reader.set_onload(Some(
            Closure::once_into_js(move |e: web_sys::Event| on_load(e)).unchecked_ref(),
        ));

        let _ = reader.read_as_array_buffer(&file);

        // Clean up input element
        if let Some(parent) = input_clone.parent_node() {
            let _ = parent.remove_child(&input_clone);
        }
    };

    // Same again: one picker, one change event, one closure that frees
    // itself afterwards instead of accumulating per load.
    input.set_onchange(Some(
        Closure::once_into_js(move |e: web_sys::Event| on_change(e)).unchecked_ref(),
    ));

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
    let on_change = move |_event: web_sys::Event| {
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
        let on_load = move |_event: web_sys::Event| {
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
        };

        // `once_into_js`, NOT `wrap(..).forget()`. The old comment here
        // said the closure "will be cleaned up when reader is done";
        // `forget` never cleans anything up -- it leaks the box and its
        // function-table entry for the life of the page, once per file
        // loaded. A FileReader onload fires exactly once, so a
        // once-closure is both correct and self-freeing: JS owns it
        // until it runs, then the Rust side is dropped.
        reader.set_onload(Some(
            Closure::once_into_js(move |e: web_sys::Event| on_load(e)).unchecked_ref(),
        ));

        let _ = reader.read_as_array_buffer(&file);

        // Clean up input element
        if let Some(parent) = input_clone.parent_node() {
            let _ = parent.remove_child(&input_clone);
        }
    };

    // Same again: one picker, one change event, one closure that frees
    // itself afterwards instead of accumulating per load.
    input.set_onchange(Some(
        Closure::once_into_js(move |e: web_sys::Event| on_change(e)).unchecked_ref(),
    ));

    // Trigger file picker
    input.click();
}

use winit::{event::*, event_loop::{EventLoop, ControlFlow, ActiveEventLoop}, window::Window};
use std::sync::{Arc, Mutex};

/// Local error type for `App::render` — replaces the wgpu 0.27
/// `SurfaceError` variants we used to match on. wgpu 0.29's
/// `Surface::get_current_texture` returns the `CurrentSurfaceTexture`
/// enum instead of a `Result`, with finer-grained non-success states
/// (Suboptimal counts as usable; Validation/Timeout are new).
/// These are NOT all the same severity, and collapsing them was a real
/// bug: `Occluded` and `Timeout` used to share an `Other` variant with
/// `Validation`, so all three tore down and rebuilt the entire GPU
/// device. An occluded surface only means the window is not visible —
/// routine while a window is first being composited, or minimised — and
/// the correct response is to skip the frame.
#[derive(Debug)]
pub enum RenderError {
    /// Surface dropped/invalidated — needs full recreate.
    Lost,
    /// Surface configuration outdated — needs reconfigure.
    Outdated,
    /// The window is not visible, so there is nothing to present to.
    /// Routine, and not a device problem: skip the frame.
    Occluded,
    /// Acquisition timed out. Transient — skip the frame and try again.
    Timeout,
    /// The surface reported a validation error. Genuine trouble.
    Validation,
}

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
        }
    }
}

impl ApiContentState {

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
use crate::ui::{ExportStatus, ExportKind, UiReporter};
use crate::renderer::FlameRenderer;
use crate::scene::transforms::Flame;
use crate::scene::palette::{global_palette_library, PaletteLibrary};
use crate::scene::presets::global_preset_library;
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

/// How long after the last edit a mode-D render stays a preview.
///
/// Long enough that a drag never sees a full render between two of
/// its own events, short enough that the full picture is there before
/// the eye asks for it. The coalescing window in the config manager is
/// 500 ms; this is deliberately shorter, because that one is about
/// UNDO granularity and this one is about what is on screen.
/// The escape engine's image for the tail: the terrain's while the
/// config draws one, the 2D picture's otherwise. Takes the fields, not
/// the app, so a caller holding the flame renderer can still ask.
fn escape_view<'a>(
    escape: &'a Option<crate::escape::EscapeRenderer>,
    #[cfg(feature = "terrain")] terrain: &'a Option<crate::escape::footprint::EscapeTerrain>,
    config: &crate::config::FractalConfig,
) -> Option<&'a egui_wgpu::wgpu::TextureView> {
    #[cfg(feature = "terrain")]
    if config.escape.terrain_active() {
        return terrain.as_ref().map(|t| t.output_view());
    }
    let _ = config;
    // A motion-blurred frame, finished, while the next builds.
    escape.as_ref().map(|e| e.presented_view().unwrap_or_else(|| e.output_view()))
}

/// The simulation's image for the tail: its terrain's while the config
/// draws one, the grid's picture otherwise.
#[cfg(feature = "engine-sim")]
fn sim_view<'a>(
    sim: &'a Option<crate::sim::SimRenderer>,
    #[cfg(feature = "terrain")] terrain: &'a Option<crate::sim::terrain::SimTerrain>,
    config: &crate::config::FractalConfig,
) -> Option<&'a egui_wgpu::wgpu::TextureView> {
    #[cfg(feature = "terrain")]
    if config.sim.terrain_active() {
        if let Some(t) = terrain.as_ref() {
            return Some(t.output_view());
        }
    }
    let _ = config;
    // A motion-blurred frame, finished, while the next builds.
    sim.as_ref().map(|s| s.presented_view().unwrap_or_else(|| s.output_view()))
}

const ESCAPE_INTERACTION_WINDOW: std::time::Duration = std::time::Duration::from_millis(250);

pub struct App {
    // Window reference (needed for fullscreen toggle)
    pub(super) window: Arc<Window>,

    // Core state management
    pub(super) config_manager: ConfigManager,  // Single source of truth for ALL config (fractal + system)

    // GPU and rendering resources
    pub(super) gpu: GpuContext,
    pub(super) egui_layer: EguiLayer,
    pub(super) flame_renderer: Option<FlameRenderer>,
    /// Escape-time generator, created lazily the first frame the config
    /// is in `RenderMode::Escape`. Lives beside (not inside) the flame
    /// renderer: it consumes the flame renderer's palette texture and
    /// tonemap tail but owns its own pipelines and output.
    pub(super) escape_renderer: Option<crate::escape::EscapeRenderer>,
    /// Mode D's packed flame, analysed again only when it changes
    /// (`escape::ifs::PackCache`).
    pub(super) ifs_pack_cache: crate::escape::ifs::PackCache,
    /// The 3D terrain view of the escape picture
    /// (docs/archive/terrain/heightfield-3d.md): its footprint's renderer and
    /// the terrain renderer. Held instead of `escape_renderer` while
    /// the config draws a terrain, and freed when it does not.
    #[cfg(feature = "terrain")]
    pub(super) escape_terrain: Option<crate::escape::footprint::EscapeTerrain>,
    /// Set by `process_gpu_updates` when an edit requires re-running
    /// the escape pass (escape params, palette, structural loads).
    /// Starts true so the first escape frame always renders.
    pub(super) escape_dirty: bool,
    /// The viewport frozen while an export runs in the background (a
    /// PNG on a device of its own, a high-res PNG, a video): its last
    /// picture stays on screen, nothing renders, and the engines that
    /// rebuild themselves -- the escape renderer, the terrains -- give
    /// their GPU memory back, since the export competes for the same
    /// card (`freeze_viewport`).
    pub(super) viewport_frozen: bool,
    /// Keeps `escape_renderer` holding the texture the config names.
    pub(super) escape_texture: texture_sync::TextureSync,
    /// The frame governor's antialiasing axis for escape playback
    /// (aa_governor.rs), and when the last settled frame landed.
    pub(super) escape_aa: aa_governor::AaGovernor,
    pub(super) escape_aa_settled_at: Option<web_time::Instant>,
    /// The escape frame being built from its motion-blur moments
    /// (motion_blur.rs), while one is.
    pub(super) escape_motion: Option<motion_blur::Moments>,
    /// The simulation frame being walked through its moments' step
    /// counts (motion_blur.rs), while one is.
    pub(super) sim_motion: Option<motion_blur::Moments>,
    /// When the user last EDITED something the escape pass renders.
    /// For the interaction window: within it a mode-D render is a
    /// quarter-resolution preview, after it a full one. See
    /// `ESCAPE_INTERACTION_WINDOW`.
    pub(super) escape_last_edit: Option<web_time::Instant>,

    /// The simulation's grid and step state. Lazily created on first
    /// use so a flame session never allocates it.
    #[cfg(feature = "engine-sim")]
    pub(super) sim_renderer: Option<crate::sim::SimRenderer>,
    /// The simulation's 3D terrain (heightfield plan T4), while the config
    /// draws one.
    #[cfg(all(feature = "terrain", feature = "engine-sim"))]
    pub(super) sim_terrain: Option<crate::sim::terrain::SimTerrain>,
    /// Whether the Run button is engaged. Deliberately NOT in the
    /// config: it is a view state like the playhead, not part of the
    /// picture, and saving it would make a file that starts moving as
    /// soon as it opens.
    pub(super) sim_running: bool,
    /// A single Step was requested this frame.
    pub(super) sim_step_once: bool,
    /// Restart from the seed before the next frame (a reseed-class
    /// edit, a device loss, or a grid the field cannot be carried into).
    pub(super) sim_reseed: bool,
    /// The step count the TIMELINE has committed the grid to.
    ///
    /// `Some` means the timeline owns the step count: the driver walks
    /// the field toward this target within a per-frame budget, and
    /// Run / Pause / Step are inert because a picture that depended on
    /// both would depend on how long the user looked at it. `None` --
    /// the ordinary state -- means `sim.steps` is the Max Steps CAP
    /// and the transport drives the grid, exactly as before.
    ///
    /// Cleared when the grid arrives, so playback re-commits each
    /// frame while a scrub commits once and the run then holds where
    /// the timeline left it.
    ///
    /// Deliberately not the config's `sim.steps`: that field means
    /// "cap" to the transport and "target" to the timeline, and the
    /// only honest way to tell them apart is to record which one is
    /// driving (`docs/archive/projects/video-loop-and-sim-timeline.md` D4).
    #[cfg(feature = "engine-sim")]
    pub(super) sim_timeline_target: Option<u32>,
    /// Wall time accumulated while an animation frame is still coming
    /// into being -- an escape render that has not settled, or a
    /// simulation grid still stepping toward this frame's target.
    /// Playback of both is SETTLE-THEN-JUMP: the controller is sampled
    /// once per completed frame and advanced by everything that
    /// elapsed meanwhile, rather than every display frame, so every
    /// displayed frame is the one the export would render (see
    /// `paced_playback_tick`).
    pub(super) paced_anim_pending: f64,
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

    // Animation system
    pub(super) animation_controller: AnimationController,

    // Performance tracking
    pub(super) metrics: PerformanceMetrics,

    // Rendering internals (frame timing and batching)
    pub(super) last_frame_time: Option<web_time::Instant>,
    // Adaptive iteration governor (see the dispatch site): scales the
    // per-frame iterations_per_thread down when frames blow the target
    // budget and back up when there is headroom. 1.0 = full batch.
    pub(super) iter_scale: f64,
    /// The dispatch's length past the full batch, from the GPU's timing
    /// (`fill_step`); 1 is the batch.
    pub(super) batch_fill: f64,
    /// Bumped whenever the fill starts again: a timing tagged with an
    /// older one measured something else.
    pub(super) fill_tag: u64,
    /// The renderer's iteration count last frame: a smaller one is a
    /// restart.
    pub(super) fill_last_total: u64,
    pub(super) last_iter_frame: Option<web_time::Instant>,
    /// The previous two frames' budget ratios, for the governor's
    /// median-of-3 outlier filter. Neutral (1.0) at rest.
    pub(super) governor_ratio_prev: [f64; 2],
    /// Whether shedding width still helps (`Knee`), and the dispatch
    /// geometry it was judged for: a change of geometry starts over.
    pub(super) governor_knee: crate::app::Knee,
    pub(super) governor_batch: Option<crate::app::Batch>,
    // Display refresh in millihertz, and a counter that re-queries it every
    // 120 iterating frames. Under vsync this — not `target_fps` — is the
    // governor's budget; see `frame_budget`.
    pub(super) display_refresh_mhz: Option<u32>,
    pub(super) display_refresh_age: u32,
    pub(super) accumulation_batch_size: u32,  // Process every N frames (1 = normal, 4 = batched)
    pub(super) frames_since_accumulation: u32,
    // The samples the batch's frames plotted so far, each frame's as
    // `compute_pass` counted it: frames differ (the first after a restart
    // pays the burn-in, the governor changes the width), so a batch is
    // their sum, not its last frame times its length.
    pub(super) samples_in_batch: u64,
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
    // Tracks whether the fractal was actively rendering on the previous
    // AboutToWait. When it transitions from true to false (e.g.,
    // max_iterations just reached), we need one more redraw to flush
    // the final tonemap output to the screen — PHASE 1 paints from the
    // *previous* frame's fractal_texture, so without an extra cycle
    // the final compute pass never makes it visible.
    pub(super) was_rendering_prev_frame: bool,
    /// What the menu bar's progress bar was last handed.
    pub(super) progress_drawn: crate::ui::RenderProgress,
    // Set true whenever the egui layout reports a fractal-viewport
    // size different from what the renderer is configured for. Cleared
    // when the renderer resize actually fires. Keeps the event loop
    // awake while a resize is in flight, so the post-resize render
    // doesn't stall on `ControlFlow::Wait` before completing.
    pub(super) viewport_resize_pending: bool,

    // Fractal viewport size (updated from UI each frame)
    pub(super) fractal_viewport_size: (u32, u32),
    // Debounce viewport resize (WASM only - prevents rapid resize loops)
    #[cfg(target_arch = "wasm32")]
    pub(super) last_viewport_resize_time: Option<web_time::Instant>,

    // PNG export settings (UI state only, not in config; the size is the
    // picture's, `FractalConfig::image_size`)
    /// Transparent PNG export uses premultiplied alpha (vs the default
    /// straight-alpha flatten-over-black reconstruction). Set in the Export panel.
    pub(super) png_export_premultiplied: bool,
    /// 2× supersampled export (render at double resolution, box-filter
    /// + firefly clamp down) — see export::supersample.
    pub(super) png_export_supersample: bool,

    // Unified export status — shared with every background export thread (PNG
    // direct / high-res / video). Drives the menu bar's progress bar and the
    // terminal toast. Replaces the former per-path progress structs + the
    // window-title hack.
    pub(super) export_status: Arc<Mutex<ExportStatus>>,

    // Rendering mode state machine (Normal, Animating, Overwrite)
    pub(super) render_mode: RenderModeFSM,

    // Histogram computation (computed periodically, not every frame)
    pub(super) histogram_frame_counter: u32,
    /// Whether the Histogram section was on screen last frame: it opening
    /// reads the histogram at once.
    pub(super) histogram_was_shown: bool,
    // WASM: async histogram result slot + in-flight guard
    #[cfg(target_arch = "wasm32")]
    pub(super) histogram_result_slot: Option<std::sync::Arc<std::sync::Mutex<Option<crate::renderer::DensityHistogram>>>>,
    #[cfg(target_arch = "wasm32")]
    pub(super) histogram_in_flight: bool,

    // Post-processing effect chain
    pub(super) effect_chain: crate::renderer::effect_chain::EffectChainRunner,

    // Fullscreen state (two-stage: window fullscreen, then hide UI)
    pub(super) window_fullscreen: bool,  // Window is in fullscreen mode
    pub(super) ui_hidden: bool,          // UI panels are hidden (only in fullscreen)

    // Free-fly camera mode (3D mode only). Toggled via the View panel
    // button or the F2 hotkey. When on, mouse drag in the viewport
    // rotates the camera (instead of panning) and WASD/QE/Shift
    // translate the camera position. The set of currently-held movement
    // keys is integrated per-frame via `update_fly_camera`.
    pub(super) fly_mode: bool,
    pub(super) fly_keys_held: std::collections::HashSet<winit::keyboard::KeyCode>,
    /// Time of the last fly-mode movement integration step. Used to
    /// compute delta-time so movement speed is frame-rate-independent.
    pub(super) fly_last_update: Option<web_time::Instant>,

    // Surface recovery: set on surface error, handled at top of next RedrawRequested
    pub(super) needs_surface_recreate: bool,
    /// `ConfigManager::load_generation` as of the last frame, so a
    /// newly LOADED fractal can be noticed exactly once. Loads are
    /// the only thing that bumps it (preset, file import, the online
    /// browser) -- animation playback and undo snapshots go through
    /// `load_config_silent` / `load_config_with_explicit_before` and
    /// deliberately do not.
    last_load_generation: u64,
    /// The render mode as of the last frame, so a change -- from the
    /// Mode menu, a panel, a script or an undo -- brings the workspace
    /// with it.
    last_render_mode: crate::scene::transforms::RenderMode,

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
    /// Online-library state for the Scripts panel, and the slot its
    /// background requests land in.
    /// Effects fetched on demand: results in flight, and the names
    /// already tried this session so a miss is not retried every frame.
    pub(super) effect_fetch_results:
        std::sync::Arc<std::sync::Mutex<Vec<(String, Result<crate::api::types::EffectDownload, String>)>>>,
    pub(super) effect_fetch_attempted: std::collections::HashSet<String>,
    /// Last-fetched effect catalog, and the once-per-session refresh.
    pub(super) effect_catalog: Option<crate::storage::effect_catalog::CachedEffectCatalog>,
    pub(super) effect_catalog_result: std::sync::Arc<
        std::sync::Mutex<Option<Result<Vec<crate::api::types::EffectListItem>, String>>>,
    >,
    pub(super) effect_catalog_started: bool,
    pub(super) script_cloud: script_cloud::ScriptCloudState,
    pub(in crate::app) script_cloud_results: script_cloud::ScriptCloudSlot,
    /// Parallel to config history: for each FullConfig snapshot, stores
    /// (before_api_state, after_api_state). Keyed by the history index of
    /// the snapshot. Delta-level undo/redo doesn't touch api_state.
    pub(super) api_state_history: std::collections::HashMap<usize, (ApiContentState, ApiContentState)>,
    /// Which snapshot we're currently "in" — updates on undo/redo across snapshots.
    /// Used to keep the current snapshot's "after" up to date with any api_state changes.
    pub(super) current_api_snapshot_idx: Option<usize>,
    // API save in-flight state
    /// The name the in-flight online save was dispatched with.
    ///
    /// Held across the async round trip so the result reports what was
    /// actually sent, and so the local flame adopts it on success —
    /// the two names are one name. Reading the live config instead
    /// (what the notification used to do) reports whatever the flame
    /// was called BEFORE the save.
    pub(super) api_pending_name: Option<String>,
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

    /// Subflames panel "Load from file" target: index of the subflame
    /// to replace once the picked `.fflame` finishes loading. Captured
    /// at button-click time so the async WASM file picker can pair the
    /// returned JSON with the right slot. Desktop uses a synchronous
    /// dialog, so the index never has to survive across frames — the
    /// field is unused there.
    #[allow(dead_code)]
    pub(super) pending_subflame_load_index: Option<usize>,

    // Variation fetching: when loading a flame that references unknown
    // variations, fetch them from the API in parallel and pause rendering
    // until done (or timeout).
    /// Completed fetches, tagged with the batch that asked for them.
    ///
    /// The tag is load-bearing. A timed-out batch's threads keep running
    /// and push here whenever they finish — possibly *after* the next
    /// batch has started. Untagged, the next drain counted those as its
    /// own and decremented its pending count, finalizing it early and
    /// registering variations nobody asked for. Clearing the vector on
    /// trigger does not fix that: the late push happens after the clear.
    pub(super) variation_fetch_results: std::sync::Arc<std::sync::Mutex<
        Vec<(u64, String, Result<crate::api::types::VariationDownload, String>)>
    >>,
    pub(super) variation_fetch_in_progress: bool,
    pub(super) variation_fetch_started: Option<web_time::Instant>,
    /// Number of in-flight fetches; when this drops to 0, processing is complete
    pub(super) variation_fetch_pending_count: usize,
    /// Monotonic batch id. Results not carrying the current value are
    /// stragglers from an abandoned batch and are discarded.
    pub(super) variation_fetch_epoch: u64,

    /// The server's variation catalog, for the panel's merged listing.
    ///
    /// Loaded from cache at startup so the panel works offline, then
    /// refreshed once in the background. `None` means neither a cache
    /// nor a successful fetch — the panel then shows only what is
    /// installed, which is still the truth, just less of it.
    pub(super) variation_catalog: Option<crate::storage::variation_catalog::CachedCatalog>,
    /// In-flight catalog fetch. Same shared-slot shape as the variation
    /// fetches; no epoch needed because there is only ever one and a
    /// stale result is merely old data, not a miscount.
    pub(super) catalog_fetch_result: std::sync::Arc<std::sync::Mutex<
        Option<Result<Vec<crate::api::types::VariationListItem>, String>>
    >>,
    pub(super) catalog_fetch_started: bool,
    /// What the current batch is still waiting on, for the timeout message.
    pub(super) variation_fetch_names: Vec<String>,

    // API connectivity tracking
    pub(super) health_check_result: std::sync::Arc<std::sync::Mutex<Option<crate::api::HealthCheckOutcome>>>,
    pub(super) health_check_in_progress: bool,
    pub(super) api_connectivity: crate::api::ApiConnectivity,
    /// Decides when a failed health check means the server is gone —
    /// see `api::health`. `api_connectivity` above is its last verdict,
    /// copied out for the UI.
    pub(super) health_policy: crate::api::health::HealthPolicy,
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
        let mut egui_layer = EguiLayer::new(&window, &gpu.device, gpu.config.format);

        // Cached downloads, then local plugins. Shared with the headless
        // entry points so there is one order rather than two.
        let plugin_report = crate::storage::load_installed_resources();
        // A refused plugin must be REPORTED. The user installed a file
        // and it is not there; a log line is not a report, and this is
        // the one moment they can connect the absence to a cause.
        if !plugin_report.refused.is_empty() {
            let list = plugin_report
                .refused
                .iter()
                .map(|(n, why)| format!("{n} ({why})"))
                .collect::<Vec<_>>()
                .join("; ");
            egui_layer.show_api_notification(
                &format!("Some plugins were not loaded — {list}"),
                true,
            );
        }

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

        let mut flame_renderer = FlameRenderer::with_palette_size(
            &gpu.device,
            &gpu.queue,
            gpu.config.format,
            gpu.size.width,
            gpu.size.height,
            &flame,
            initial_config.palette_size,
        );
        // The app plans deep-zoom cylinders off the UI thread.
        flame_renderer.set_background_planning(true);

        // ConfigManager loads SystemSettings automatically
        let config_manager = ConfigManager::new(initial_config.clone());
        // Open in the language the user last chose (recorded after each
        // frame's UI, below). A locale whose script needs a font this
        // platform cannot load stays in English, as the globe menu does.
        let language = config_manager.system_settings().language.clone();
        if language != crate::i18n::current_locale()
            && crate::i18n::supported_locales().iter().any(|l| l.code == language)
            && crate::ui::ensure_font_for_locale(&egui_layer.ctx, &language)
        {
            crate::i18n::set_locale(&language);
        }
        // Read before the move below: a boot must not look like a
        // load, or it would override the layout the user left.
        let initial_load_generation = config_manager.load_generation();

        // Get initial size before moving gpu
        let initial_viewport_size = (gpu.size.width, gpu.size.height);

        // The privacy switch is persisted, but every export reads it
        // from a global at encode time — so mirror it before anything
        // can export, or the first export of a session ignores it.
        crate::png_metadata::set_strip_metadata(
            config_manager.system_settings().png_export_strip_metadata,
        );
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
            escape_renderer: None,
            ifs_pack_cache: Default::default(),
            #[cfg(feature = "terrain")]
            escape_terrain: None,
            escape_dirty: true,
            viewport_frozen: false,
            escape_texture: Default::default(),
            escape_aa: Default::default(),
            escape_aa_settled_at: None,
            escape_motion: None,
            sim_motion: None,
            escape_last_edit: None,
            #[cfg(feature = "engine-sim")]
            sim_renderer: None,
            #[cfg(all(feature = "terrain", feature = "engine-sim"))]
            sim_terrain: None,
            // Runs on entry: a simulation that sits still looks broken,
            // and the first thing anyone does is press Run anyway.
            sim_running: true,
            sim_step_once: false,
            sim_reseed: true,
            #[cfg(feature = "engine-sim")]
            sim_timeline_target: None,
            paced_anim_pending: 0.0,
            flame,
            workspace: crate::ui::Workspace::new(),
            view_changed_by_keyboard: false,
            paused: false,
            view_subflame_in_isolation: false,
            modifiers: winit::keyboard::ModifiersState::default(),
            quit_requested: false,
            palette_library,
            animation_controller: AnimationController::new(),
            metrics: PerformanceMetrics::new(),
            last_frame_time: None,
            iter_scale: 1.0,
            batch_fill: 1.0,
            fill_tag: 0,
            fill_last_total: 0,
            last_iter_frame: None,
            governor_ratio_prev: [1.0, 1.0],
            governor_knee: crate::app::Knee::Shedding { over: 0 },
            governor_batch: None,
            display_refresh_mhz: None,
            display_refresh_age: 0,
            accumulation_batch_size: 4, // EXPERIMENT: Test batching
            frames_since_accumulation: 0,
            samples_in_batch: 0,
            use_overwrite_next_frame: false,
            last_param_change_time: None,
            rendering_complete: false,
            clear_paths_next_frame: true,  // Clear paths on first frame
            ui_needs_repaint: false,
            last_input_time: None,
            was_rendering_prev_frame: false,
            progress_drawn: crate::ui::RenderProgress::Idle,
            viewport_resize_pending: false,
            fractal_viewport_size: initial_viewport_size, // Initialize to window size
            #[cfg(target_arch = "wasm32")]
            last_viewport_resize_time: None,
            png_export_premultiplied: false,
            png_export_supersample: false,
            export_status: Arc::new(Mutex::new(ExportStatus::default())),
            render_mode: RenderModeFSM::new(),
            histogram_frame_counter: 0,
            histogram_was_shown: false,
            #[cfg(target_arch = "wasm32")]
            histogram_result_slot: None,
            #[cfg(target_arch = "wasm32")]
            histogram_in_flight: false,
            effect_chain,
            needs_surface_recreate: false,
            // Whatever the config manager starts at, so a boot does
            // not fight the layout the user left the app in.
            last_load_generation: initial_load_generation,
            last_render_mode: initial_config.render_mode,
            window_fullscreen: false,
            ui_hidden: false,
            fly_mode: false,
            fly_keys_held: std::collections::HashSet::new(),
            fly_last_update: None,
            audio_manager: crate::audio::AudioManager::new(),
            audio_player: crate::audio::AudioPlayer::new(),
            audio_capture: crate::audio::AudioCapture::new(),
            signal_manager: crate::signal::SignalManager::new(),
            api_state: ApiContentState::default(),
            current_user_id: None,
            effect_fetch_results: Default::default(),
            effect_fetch_attempted: Default::default(),
            effect_catalog: crate::storage::effect_catalog::load(),
            effect_catalog_result: Default::default(),
            effect_catalog_started: false,
            script_cloud: Default::default(),
            script_cloud_results: Default::default(),
            api_state_history: std::collections::HashMap::new(),
            current_api_snapshot_idx: None,
            api_pending_name: None,
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
            pending_subflame_load_index: None,

            variation_fetch_results: std::sync::Arc::new(std::sync::Mutex::new(Vec::new())),
            variation_fetch_in_progress: false,
            variation_fetch_started: None,
            variation_fetch_epoch: 0,
            variation_catalog: crate::storage::variation_catalog::load(),
            catalog_fetch_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
            catalog_fetch_started: false,
            variation_fetch_names: Vec::new(),
            variation_fetch_pending_count: 0,
            health_check_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
            health_check_in_progress: false,
            api_connectivity: crate::api::ApiConnectivity::Unknown,
            health_policy: crate::api::health::HealthPolicy::default(),
            last_health_check: None,
        };

        // Register live audio capture as a signal producer so live_* signals
        // appear in the animation track signal dropdown
        app.signal_manager.add_producer(app.audio_capture.create_producer());

        // Initialize GPU state with initial config (ensures shaders are compiled with correct variations)
        app.import_config(initial_config, true);

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

        // The handler is bound first so the two platforms can start it
        // differently. On the web that is not a nicety:
        //
        // winit's web `run()` boxes a handler that BORROWS its own stack
        // frame -- `self`, `target` and `event_handler` are locals of
        // `run` -- transmutes it to 'static, and then throws a JS
        // exception so the function never returns and the frame is
        // never cleaned up ("SAFETY: Don't use `move` to make sure we
        // leak", event_loop/mod.rs:41). `spawn()` is the same handler
        // built with `move`, owning its captures on the heap, and it
        // returns normally instead of throwing.
        //
        // The wasm load crash traps inside exactly that handler closure
        // -- identified from the shipped module's own string literals,
        // "handler woken up without user event" plus the `unreachable!`
        // message, which are winit's and appear in no other function.
        // Using the entry point that does not depend on an abandoned
        // stack frame staying readable removes the hazard rather than
        // reasoning about whether it fires. See
        // docs/archive/projects/wasm-load-crash.md.
        let handler = move |event: Event<()>, elwt: &ActiveEventLoop| {
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
                        WindowEvent::Touch(ref _touch) => {
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
                                // On WASM, the size here has already been capped
                                // by the `window.resize` listener in lib.rs (which
                                // calls request_inner_size with `CSS × min(DPR,
                                // 1.5)`). winit then fires this event with the
                                // capped value.
                                log::info!("Window resized to {}x{}", size.width, size.height);
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
                        WindowEvent::Focused(false) => {
                            // A key held as the window lost focus never
                            // sends its key-up here: let go of them all,
                            // or the fly camera flies on.
                            app.release_fly_keys();
                        }
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
                        WindowEvent::KeyboardInput { event: key_event, .. } => {
                            // A release egui took (a text field had the
                            // focus) still lets go of a fly key: held
                            // otherwise, the camera flew on.
                            if !key_event.state.is_pressed() {
                                if let winit::keyboard::PhysicalKey::Code(code) = key_event.physical_key {
                                    app.fly_keys_held.remove(&code);
                                }
                            }
                        }
                        WindowEvent::ModifiersChanged(new_modifiers) => {
                            app.modifiers = new_modifiers.state();
                        }
                        WindowEvent::RedrawRequested => {
                            // Handle deferred GPU reinitialization (from previous frame's error)
                            // Desktop only — WASM WebGPU doesn't have device loss from sleep/wake
                            // A lost DEVICE reaches us through wgpu's
                            // device-lost callback, not as a surface
                            // error, so it has to be picked up here --
                            // otherwise the app runs on against a dead
                            // device with every call failing. The
                            // rebuild below is the one surface loss
                            // already used.
                            #[cfg(not(target_arch = "wasm32"))]
                            if crate::gpu::device::take_device_lost() {
                                log::warn!("Device lost: scheduling a full GPU rebuild");
                                app.needs_surface_recreate = true;
                            }
                            #[cfg(not(target_arch = "wasm32"))]
                            if app.needs_surface_recreate {
                                log::info!("Attempting full GPU reinitialization...");

                                // Drop GPU-dependent resources BEFORE reinit
                                // (they hold Arc refs to the dead device)
                                app.flame_renderer = None;
                                // The escape renderer too: it was forgotten
                                // here once, survived a reinit holding
                                // buffers from the DEAD device, and the next
                                // escape frame's write_buffer tripped
                                // wgpu-core's dead-buffer assert -- whose
                                // unwind then hit the swapchain-semaphore
                                // Drop assert, and the double panic aborted
                                // the process as 0xc0000409 (from a user
                                // crash.log backtrace). Plain drop, not
                                // destroy(): the device is dead and this
                                // path is desktop-only, where Drop frees.
                                // The frame loop lazily recreates it against
                                // the new device (get_or_insert_with), and
                                // its orbit worker thread exits cleanly when
                                // its channel drops; the reference reloads
                                // from the disk orbit store.
                                app.escape_renderer = None;
                                #[cfg(feature = "terrain")]
                                {
                                    app.escape_terrain = None;
                                }
                                // The field lives in GPU textures that
                                // just went away; the run cannot be
                                // recovered, only restarted.
                                #[cfg(feature = "engine-sim")]
                                {
                                    app.sim_renderer = None;
                                    #[cfg(feature = "terrain")]
                                    {
                                        app.sim_terrain = None;
                                    }
                                }
                                app.sim_reseed = true;

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
                                        if let Some(r) = app.flame_renderer.as_mut() {
                                            r.set_background_planning(true);
                                        }
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
                                        // The recreated escape renderer must
                                        // render before it has anything to
                                        // show.
                                        app.escape_dirty = true;
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
                                Err(RenderError::Lost | RenderError::Outdated) => {
                                    log::warn!("Surface lost/outdated, scheduling recovery...");
                                    #[cfg(not(target_arch = "wasm32"))]
                                    { app.needs_surface_recreate = true; }
                                    #[cfg(target_arch = "wasm32")]
                                    { app.gpu.resize(app.gpu.size); }
                                }
                                Err(e @ (RenderError::Occluded | RenderError::Timeout)) => {
                                    // Not a device fault. The window is hidden
                                    // or the swapchain was busy; there is
                                    // nothing to fix and nothing to present
                                    // to. Rebuilding the GPU here cost two
                                    // full reinitialisations on every macOS
                                    // launch, purely because the window had
                                    // not finished appearing yet.
                                    log::debug!("Surface not presentable ({e:?}); skipping frame");
                                }
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

                    // Check if any export is in progress, or has finished
                    // with a toast the next frame has not shown yet.
                    let (is_exporting, toast_pending) = app.export_status.lock()
                        .map(|s| (s.active, s.toast.is_some()))
                        .unwrap_or((false, false));

                    // Check if actively rendering (not paused and under
                    // max_iterations). An export pauses the viewport's
                    // flame (`should_iterate`), so it is not rendering then.
                    let config = app.config_manager.active_config();
                    let max_iterations = Some(config.max_iterations);
                    let is_rendering = !app.paused
                        && !is_exporting
                        && config.render_mode != crate::scene::transforms::RenderMode::Escape
                        && config.render_mode != crate::scene::transforms::RenderMode::Simulation
                        && app.flame_renderer.as_ref().map_or(false, |r| {
                            max_iterations.map_or(true, |max| r.total_iterations() < max)
                        });

                    // Check if animation is playing (needs continuous redraws)
                    let animation_playing = app.animation_controller.is_playing();

                    // Check if audio is playing or capturing (needs UI updates for progress/signals)
                    let audio_playing = app.audio_player.state() == crate::audio::PlaybackState::Playing;
                    let audio_capturing = app.audio_capture.is_capturing();

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

                    // Detect the rendering→stopped transition. When the
                    // last frame finished compute that brought
                    // total_iterations to max_iterations, the new
                    // tonemap output is in fractal_texture but hasn't
                    // been displayed yet — PHASE 1 paints from the
                    // *previous* frame's fractal_texture, so without an
                    // extra redraw cycle the user is stuck looking at
                    // the second-to-last frame (e.g., "983M / 1B"
                    // counter freezes). Request one more redraw on the
                    // transition so PHASE 1 picks up the final state.
                    let just_finished_rendering = app.was_rendering_prev_frame && !is_rendering;
                    app.was_rendering_prev_frame = is_rendering;

                    // EVENT-DRIVEN RENDERING:
                    // Only render when something actually changes
                    // Fly mode with any movement key held also drives
                    // continuous redraws — the per-frame integrator in
                    // `update_fly_camera` only advances when render()
                    // runs, so without this the camera would freeze
                    // mid-flight as soon as event-driven rendering
                    // settled.
                    let fly_active = app.fly_mode && !app.fly_keys_held.is_empty();

                    // A running simulation needs continuous redraws for
                    // the same reason fly mode does: it only advances
                    // when render() runs. Deliberately NOT folded into
                    // `is_rendering`, which drives the flame's
                    // "rendering complete" UI state and its
                    // max_iterations comparison -- neither means
                    // anything here.
                    // ...or catching up to a timeline target, which
                    // is the same need: the grid only advances when
                    // render() runs, and a scrub that lands mid-jump
                    // must not stall because the window went to sleep.
                    #[cfg(feature = "engine-sim")]
                    let sim_catching_up = app.sim_timeline_target.is_some();
                    #[cfg(not(feature = "engine-sim"))]
                    let sim_catching_up = false;
                    let sim_active = (app.sim_running || sim_catching_up)
                        && app.config_manager.active_config().render_mode
                            == crate::scene::transforms::RenderMode::Simulation;

                    // The menu bar's progress bar is built before the frame's
                    // render runs, so what it shows can be a frame behind:
                    // an escape render that has just settled, a simulation
                    // that has just paused. One more frame brings it up to
                    // date, and then the loop can sleep. An export is left
                    // to its own cadence, below.
                    let progress_stale =
                        !is_exporting && app.render_progress(None) != app.progress_drawn;

                    // During audio playback or live capture, keep redrawing to update UI
                    if is_rendering || animation_playing || audio_playing || audio_capturing || ui_active || app.viewport_resize_pending || just_finished_rendering || fly_active || sim_active || progress_stale {
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
                    } else if is_exporting || toast_pending {
                        // An export runs on its own thread, usually on its
                        // own device. The window redraws only to move the
                        // progress bar, and ten frames a second is plenty
                        // for that: at the display rate the redraws compete
                        // with the export for the GPU.
                        const EXPORT_PROGRESS_INTERVAL: Duration = Duration::from_millis(100);
                        match app.last_frame_time {
                            Some(last) if last.elapsed() < EXPORT_PROGRESS_INTERVAL => {
                                elwt.set_control_flow(ControlFlow::WaitUntil(last + EXPORT_PROGRESS_INTERVAL));
                            }
                            _ => window.request_redraw(),
                        }
                    } else {
                        // Truly idle: sleep until event wakes us
                        elwt.set_control_flow(ControlFlow::Wait);
                    }
                }
                _ => {}
            }
        };

        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::EventLoopExtWebSys;
            event_loop.spawn(handler);
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            #[allow(deprecated)]
            event_loop.run(handler)?;
        }

        Ok(())
    }

    /// Report linear-memory growth per load, in the browser.
    ///
    /// A reported crash -- `RuntimeError: memory access out of bounds`
    /// in the frame callback, after loading a couple of files, on
    /// EITHER fractal type -- came with the reasonable suspicion that
    /// something is not being freed between loads. This says whether
    /// that is true, rather than leaving it a guess: wasm linear
    /// memory only ever GROWS (there is no shrink), so a load-over-load
    /// climb that never levels off is a leak, and a plateau means the
    /// memory is being reused and the fault is elsewhere.
    ///
    /// The undo depth rides along because it is the one thing that
    /// grows by design on every load -- a full config clone per entry,
    /// 500 deep -- so it separates "expected growth" from "leak".
    ///
    /// Browser only: on the desktop this is neither interesting nor
    /// available (`memory_size` is a wasm intrinsic).
    fn log_wasm_memory_after_load(&self, load_gen: u64) {
        #[cfg(target_arch = "wasm32")]
        {
            // One page is 64 KiB.
            let pages = core::arch::wasm32::memory_size(0);
            let mib = (pages as f64) * 65536.0 / (1024.0 * 1024.0);
            // The shadow stack grows DOWN from its top (64 MiB, set by
            // -zstack-size), so the address of a local IS the remaining
            // headroom. Read at the same point in the frame loop every
            // time, so it is comparable: if it falls load over load the
            // call depth is growing and the eventual trap is a stack
            // overflow; if it holds steady the stack is not the story.
            let probe = 0u8;
            let sp = core::ptr::addr_of!(probe) as usize;
            log::info!(
                "load #{load_gen}: wasm memory {mib:.1} MiB ({pages} pages), \
                 stack headroom {:.2} MiB, undo depth {}",
                sp as f64 / (1024.0 * 1024.0),
                self.config_manager.history_len()
            );
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = load_gen;
        }
    }

    /// Put the workspace in front of the fractal that was just
    /// loaded.
    ///
    /// Both directions, because the Escape layout deliberately
    /// carries NONE of the flame-only editors (there is a test that
    /// holds it to that): a flame loaded while it is up is not
    /// editable at all, which is the same trap in reverse. Compact
    /// mode is single-panel, so it opens the panel rather than
    /// rearranging a dock it does not have -- mirroring what an
    /// animation opened from a URL already does.
    ///
    /// Only ever called when `load_generation` moved, so an explicit
    /// layout choice survives everything except loading a fractal of
    /// the other kind.
    /// Give back the GPU state of whichever engine the new mode does
    /// not use (ui-render-modes plan, phase 5).
    ///
    /// Both engines are created lazily and were previously dropped
    /// only on device loss or before a synchronous export, so leaving
    /// a mode left everything allocated. The escape renderer alone
    /// holds gigabytes at a high antialiasing factor -- the same
    /// reason a high-res export already frees it.
    ///
    /// The two cases are not symmetric, and the comment is here so
    /// nobody has to rediscover it: the escape renderer rebuilds
    /// itself from the config, so returning costs only the re-render,
    /// but the simulation's grid IS its state, so returning restarts
    /// it from the seed.
    fn release_inactive_engines(&mut self, mode: crate::scene::transforms::RenderMode) {
        if !crate::ui::render_mode::keeps_escape_engine(mode) {
            if let Some(esc) = self.escape_renderer.take() {
                let mb = esc.resident_bytes() as f64 / (1024.0 * 1024.0);
                esc.destroy();
                log::info!("Left escape mode: freed {mb:.0} MB of escape renderer state");
            }
            #[cfg(feature = "terrain")]
            if let Some(t) = self.escape_terrain.take() {
                t.destroy();
                log::info!("Left escape mode: freed the terrain");
            }
            // Rebuilt lazily; the flag makes the first frame back render.
            self.escape_dirty = true;
        }
        #[cfg(feature = "engine-sim")]
        if !crate::ui::render_mode::keeps_sim_engine(mode) {
            #[cfg(feature = "terrain")]
            if let Some(t) = self.sim_terrain.take() {
                t.destroy();
            }
            if let Some(sim) = self.sim_renderer.take() {
                sim.destroy();
                log::info!("Left simulation mode: freed the simulation grid");
                // The grid was the state; coming back starts from the
                // seed rather than from an uninitialised field.
                self.sim_reseed = true;
            }
        }
    }

    /// The mode's editor to the front (one-workspace plan): every mode
    /// works in the one workspace, so a mode change swaps and closes
    /// nothing -- it brings Transforms, Escape or Simulation forward.
    fn show_mode_panel(&mut self) {
        let mode = self.config_manager.active_config().render_mode;
        let ctx = self.egui_layer.ctx.clone();
        self.workspace.show_main_panel(crate::ui::render_mode::main_panel_for(mode), &ctx);
    }

    fn update(&mut self) {
        // Update performance metrics
        self.metrics.update();
    }
    fn render(&mut self, window: &Window) -> Result<(), RenderError> {
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

        // Fly-mode camera integration. No-op when fly_mode is off or
        // no movement keys are held. Runs before the UI / GPU update
        // step so any camera-position changes flow through normally.
        self.update_fly_camera();

        // ============================================================================
        // NEW FRAME ORDER (Fixed race conditions):
        // 1. Render UI (reads current state, shows previous frame's fractal)
        // 2. Process all UI responses and config updates
        // 3. Get FINAL config after all updates
        // 4. Compute/accumulate/tonemap (generates new fractal with updated config)
        // 5. Submit and present
        // ============================================================================

        // Video export runs on a background thread with its own GPU device.
        // The main loop keeps drawing (egui + last fractal frame), at ten
        // frames a second, so the menu bar's progress bar stays live;
        // `should_iterate` (below) pauses the main fractal compute during any
        // export to avoid GPU contention. (Previously the whole loop was frozen
        // here and progress was written to the window title.)

        // Normal rendering: acquire surface texture. wgpu 0.29's
        // `get_current_texture` returns a `CurrentSurfaceTexture`
        // enum (not a Result). Suboptimal is still usable — we just
        // accept it and let the next reconfigure address it. The
        // other non-success states bubble up as `RenderError` for
        // the event loop to handle.
        use egui_wgpu::wgpu::CurrentSurfaceTexture;
        let frame = match self.gpu.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(f) => f,
            CurrentSurfaceTexture::Suboptimal(f) => f,
            CurrentSurfaceTexture::Outdated => return Err(RenderError::Outdated),
            CurrentSurfaceTexture::Lost => return Err(RenderError::Lost),
            CurrentSurfaceTexture::Occluded => return Err(RenderError::Occluded),
            CurrentSurfaceTexture::Timeout => return Err(RenderError::Timeout),
            CurrentSurfaceTexture::Validation => return Err(RenderError::Validation),
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

        // Snapshot the unified export status for the progress bar, and drain
        // any terminal toast queued by a finished export thread into the
        // notification system. ok() guards a poisoned mutex.
        let export_status = {
            let mut status = self.export_status.lock()
                .ok()
                .map(|s| s.clone())
                .unwrap_or_default();
            if let Some((message, is_error)) = status.toast.take() {
                self.egui_layer.show_api_notification(&message, is_error);
                // Clear the drained toast in the shared state too.
                if let Ok(mut s) = self.export_status.lock() {
                    s.toast = None;
                }
            }
            status
        };

        // The menu bar's progress bar, and what it was handed, so the loop
        // can tell when it has fallen behind (`progress_stale`).
        let progress = self.render_progress(Some(&export_status));
        self.egui_layer.update_render_progress(progress.clone());
        self.progress_drawn = progress;

        // Get signal names for track editor dropdown
        let signal_names: Vec<String> = self.signal_manager.signal_names();

        // Pass current connectivity state to UI layer
        self.egui_layer.api_connectivity = self.api_connectivity;

        // Read before the call: `config_manager` goes in mutably, so the
        // sign-in check cannot be an argument expression.
        let signed_in = self.config_manager.system_settings().is_signed_in();
        // Being HELD: the timeline is playing and asking for a step
        // count BELOW the grid, which would restart the run on every
        // frame. The panel says so, because a deliberately-held
        // picture and a stuck one look identical otherwise. Computed
        // BEFORE the call, which borrows both of these mutably.
        // The panel greys the transport on OWNERSHIP, which is wider
        // than a committed target (see `timeline_owns_sim`): a held
        // leg has no target and must still be greyed, or Run could be
        // pressed into the hold. The target itself feeds the readout.
        let sim_timeline_driven = self.timeline_owns_sim();
        let sim_timeline_holding = {
            #[cfg(feature = "engine-sim")]
            {
                let cfg = self.config_manager.active_config();
                cfg.render_mode == crate::scene::transforms::RenderMode::Simulation
                    && self.animation_controller.is_playing()
                    && self
                        .sim_renderer
                        .as_ref()
                        .is_some_and(|s| cfg.sim.steps < s.step_index())
            }
            #[cfg(not(feature = "engine-sim"))]
            {
                false
            }
        };

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
            &mut self.animation_controller,
            &mut self.paused,
            &mut self.view_subflame_in_isolation,
            &mut self.quit_requested,
            can_undo,
            can_redo,
            &mut self.workspace,
            &mut self.png_export_premultiplied,
            &mut self.png_export_supersample,
            &export_status,
            self.ui_hidden,
            &mut self.audio_manager,
            &mut self.audio_player,
            &mut self.audio_capture,
            &mut self.signal_manager,
            &signal_names,
            &self.api_state,
            self.current_user_id.as_deref(),
            self.fly_mode,
            self.variation_catalog.as_ref(),
            &self.script_cloud,
            self.effect_catalog.as_ref(),
            signed_in,
            self.sim_running,
            {
                #[cfg(feature = "engine-sim")]
                {
                    self.sim_renderer.as_ref().map_or(0, |s| s.step_index())
                }
                #[cfg(not(feature = "engine-sim"))]
                {
                    0
                }
            },
            {
                #[cfg(feature = "engine-sim")]
                {
                    self.sim_renderer.as_ref().map_or((0, 0), |s| s.grid_size())
                }
                #[cfg(not(feature = "engine-sim"))]
                {
                    (0, 0)
                }
            },
            {
                #[cfg(feature = "engine-sim")]
                {
                    self.sim_timeline_target
                }
                #[cfg(not(feature = "engine-sim"))]
                {
                    None
                }
            },
            sim_timeline_holding,
            sim_timeline_driven,
            {
                #[cfg(feature = "terrain")]
                {
                    self.escape_terrain.as_ref().and_then(|t| t.measured_range())
                }
                #[cfg(not(feature = "terrain"))]
                {
                    None
                }
            },
        );

        // The globe menus (desktop and compact) set the locale and nothing
        // else; record it, so the next start opens in it.
        let locale = crate::i18n::current_locale();
        if locale != self.config_manager.system_settings().language {
            if let Err(e) = self
                .config_manager
                .update_system_setting(crate::config::ConfigPath::SystemLanguage, crate::config::ConfigValue::String(locale))
            {
                log::warn!("Could not save the language: {e:?}");
            }
        }

        // Simulation transport, back from the panel. `sim_running` is
        // the panel's resulting state; the other two are one-shot
        // requests the frame loop consumes.
        if let Some(running) = ui_response.sim_running {
            self.sim_running = running;
        }
        if ui_response.sim_step_once {
            self.sim_step_once = true;
        }
        if ui_response.sim_reseed {
            self.sim_reseed = true;
        }
        #[cfg(all(feature = "engine-sim", not(target_arch = "wasm32")))]
        if let Some(name) = ui_response.sim_saved_texture.as_deref() {
            self.cache_saved_texture_from_run(name);
        }

        let load_gen = self.config_manager.load_generation();
        if self.last_load_generation != load_gen {
            self.last_load_generation = load_gen;
            self.log_wasm_memory_after_load(load_gen);
        }
        // The mode's editor comes forward WHENEVER the mode changes: the
        // menu bar's switch, a panel's choice, a load, an undo. And once
        // after a layout is built, which does not know the mode, so a
        // reset or a boot in Escape opens on the Escape tab. Here rather
        // than in the UI because the workspace is borrowed by the UI for
        // the whole frame -- changing its tree mid-draw would move the
        // tabs the caller is still walking.
        let mode_now = self.config_manager.active_config().render_mode;
        let new_layout = self.workspace.take_needs_main_panel();
        if self.last_render_mode != mode_now {
            self.last_render_mode = mode_now;
            self.show_mode_panel();
            self.release_inactive_engines(mode_now);
        } else if new_layout {
            self.show_mode_panel();
        }

        // Consume fly-mode responses produced by the UI this frame: a
        // look turns the camera the viewport shows, whichever 3D camera
        // it is (`camera::fly`), and nothing in a 2D view.
        if let Some((dx, dy)) = ui_response.fly_mouse_drag {
            if self.fly_mode {
                self.apply_fly_mouse_look(dx, dy);
            }
        }
        if ui_response.fly_mode_toggle_requested {
            self.toggle_fly_mode();
        }

        self.metrics.record_ui_time(t_ui_start.elapsed().as_secs_f64() * 1000.0);

        // Track UI repaint requests for frame rate optimization
        self.ui_needs_repaint = ui_response.needs_repaint;

        // Handle viewport resize immediately (before rendering)
        // WASM: Debounce resizes to prevent rapid resize loops that can freeze the browser
        if let Some(viewport_size) = ui_response.fractal_viewport_size {
            // Defensive ceiling. The egui/egui-winit screen_rect can inflate
            // far beyond the canvas's physical size if pixels_per_point gets
            // pushed below 1 (iOS Safari does this during pinch-zoom: DPR
            // drops to ~0.1, screen_rect = inner_size / 0.1 = 10× inflation,
            // dock panels report 10000+ available pixels, GPU buffer
            // allocation crashes the tab). Disabled at the source via the
            // viewport meta tag, but kept here as defense-in-depth.
            const MAX_VIEWPORT_DIM: u32 = 4096;
            if viewport_size.0 > MAX_VIEWPORT_DIM || viewport_size.1 > MAX_VIEWPORT_DIM {
                log::warn!(
                    "Suspicious fractal viewport size {}x{} (exceeds {}); refusing to resize",
                    viewport_size.0, viewport_size.1, MAX_VIEWPORT_DIM
                );
                return Ok(());
            }

            // Track resize-pending state so AboutToWait knows to keep
            // the event loop awake until the renderer has caught up.
            // Without this, a rapid resize on a flame that's already
            // hit max_iterations can leave the UI sleeping mid-resize
            // (browser stops firing winit events for a beat, ui_active
            // times out, and the post-resize compute frame never runs).
            self.viewport_resize_pending = viewport_size != self.fractal_viewport_size;

            #[cfg(target_arch = "wasm32")]
            let should_resize = {
                let now = web_time::Instant::now();
                // 100ms debounce - safe even at 10 FPS while still feeling responsive
                let debounce_ok = self.last_viewport_resize_time
                    .map(|t| now.duration_since(t).as_millis() > 100)
                    .unwrap_or(true);
                self.viewport_resize_pending && debounce_ok
            };

            #[cfg(not(target_arch = "wasm32"))]
            let should_resize = self.viewport_resize_pending;

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
                        resize_source, self.config_manager.system_settings().iterations_per_thread, resize_config.zoom, resize_config.pan_x as f32, resize_config.pan_y as f32, resize_config.rotation,
                        resize_config.camera_rotation_x, resize_config.camera_rotation_y, resize_config.camera_bank, resize_config.camera_x, resize_config.camera_y, resize_config.camera_z, resize_config.speed_factor);
                    self.gpu.queue.submit(std::iter::once(resize_encoder.finish()));

                    // Resize effect chain textures
                    self.effect_chain.resize(&self.gpu.device, viewport_size.0, viewport_size.1);

                    // Restore palette and color mode after buffer recreation
                    renderer.update_palette(&self.gpu.device, &self.gpu.queue, &resize_config.palette, resize_config.palette_rotation, resize_config.palette_squeeze, resize_config.palette_squeeze_mode, resize_config.palette_squeeze_falloff, resize_config.palette_log_strength, resize_config.palette_reverse);
                    renderer.set_color_mode(&self.gpu.queue, resize_config.color_mode, self.config_manager.system_settings().iterations_per_thread, self.config_manager.system_settings().burn_in,
                        resize_config.zoom, resize_config.pan_x as f32, resize_config.pan_y as f32, resize_config.rotation,
                        resize_config.camera_rotation_x, resize_config.camera_rotation_y, resize_config.camera_bank, resize_config.camera_x, resize_config.camera_y, resize_config.camera_z, resize_config.speed_factor);
                    renderer.set_path_map_style(resize_config.path_map_style);

                    // Update path buffer allocation based on color_mode and filters (after resize recreates buffers).
                    // Same routing as above — must use resize_source so the internal shader-cache check
                    // doesn't recompile back to the main flame's spec.
                    renderer.update_path_features(&self.gpu.device, &self.gpu.queue, resize_source);

                    // Restore tonemap parameters after buffer recreation (not in live preview mode)
                    renderer.update_tonemap(&self.gpu.queue, resize_config.tonemap_mode, resize_config.highlight_mode, resize_config.use_curve, resize_config.exposure, resize_config.gamma,
                        resize_config.gamma_threshold, resize_config.brightness, resize_config.vibrancy, resize_config.white_level, resize_config.saturation, resize_config.hue_shift,
                        resize_config.alpha_blend_low, resize_config.alpha_blend_high,
                        viewport_size.0, viewport_size.1, renderer.total_iterations(), resize_config.max_iterations, resize_config.zoom, self.config_manager.system_settings().iterations_per_thread, 1, false,
                        // Same Levels gate as every other path. This
                        // one used to test Escape alone, so a resize in
                        // Simulation let the flag through.
                        resize_config.effective_levels_enabled(),
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
        // The paint just submitted was the last user of an output texture
        // a resize above replaced.
        if let Some(renderer) = self.flame_renderer.as_mut() {
            renderer.destroy_replaced_output();
        }

        // ============================================================================
        // PHASE 2: Process ALL UI Responses and Config Updates
        // ============================================================================

        // Handle UI responses via extracted handlers
        self.handle_ui_responses(&ui_response);

        // Poll API health check and trigger periodic checks
        self.poll_health_check();

        // Handle PNG export: rendered at the picture's size -- the View
        // panel's Output Size, `FractalConfig::image_size` -- fitted to the
        // export's pixel budget as the Export panel shows it.
        if ui_response.png_export_with_background || ui_response.png_export_transparent {
            let transparent = ui_response.png_export_transparent;
            let export_config = self.export_config();
            let max_dim = self.gpu.device.limits().max_texture_dimension_2d;
            let (out_width, out_height) =
                crate::ui::fit_export_size(&export_config, self.png_export_supersample, max_dim);

            #[cfg(not(target_arch = "wasm32"))]
            {
                let render_time_ms = self.metrics.render_time_ms;
                self.export_custom_size(transparent, self.png_export_premultiplied, export_config, render_time_ms, out_width, out_height);
            }

            #[cfg(target_arch = "wasm32")]
            {
                // `render()` on the app's device, as the desktop's export
                // runs it: the pipeline every other export shares -- the
                // shade pass, DoF, density effects, the exact density
                // fraction, every engine, the burn-in setting. On the app's
                // device, not a second one: beside the live device a large
                // export runs out of memory. `render()` frees its renderer's
                // iteration buffers before the colour effects allocate, and
                // pins escape chunks.
                use wasm_bindgen_futures::spawn_local;
                let iterations_per_thread = self.config_manager.system_settings().iterations_per_thread;
                let burn_in = self.config_manager.system_settings().burn_in;
                let premultiplied = self.png_export_premultiplied;
                let device = self.gpu.device.clone();
                let queue = self.gpu.queue.clone();
                log::info!("WASM: Exporting at {}×{}", out_width, out_height);
                spawn_local(async move {
                    use crate::renderer::render::{render, NoProgress, RenderJob};
                    let job = RenderJob::new(&export_config, out_width, out_height)
                        .with_iterations_per_thread(iterations_per_thread)
                        .with_burn_in(burn_in)
                        .with_transparent(transparent)
                        .with_premultiplied(premultiplied);
                    match render(&device, &queue, job, &mut NoProgress).await {
                        Ok(out) => {
                            let metadata = crate::png_metadata::PngMetadata::from_app_state(
                                out.width,
                                out.height,
                                out.total_iterations,
                                out.render_time_ms,
                                iterations_per_thread,
                                export_config.speed_factor,
                                &export_config,
                            );
                            match crate::renderer::compute_kernel::encode_png_from_rgba(out.width, out.height, out.rgba_data, Some(metadata)) {
                                Ok(png_data) => match trigger_browser_download(&png_data, "fractal.png", "image/png") {
                                    Ok(()) => log::info!("PNG download started: {}×{} in {:.2}s", out.width, out.height, out.render_time_ms / 1000.0),
                                    Err(e) => log::error!("Failed to trigger download: {}", e),
                                },
                                Err(e) => log::error!("Failed to encode PNG: {}", e),
                            }
                        }
                        Err(e) => log::error!("PNG export failed: {}", e),
                    }
                });
            }
        }

        // Export Current Image: the viewport's picture as it stands.
        if ui_response.png_export_current {
            self.export_current_image();
        }

        // Handle animation export request
        #[cfg(not(target_arch = "wasm32"))]
        if let Some(ref export_settings) = ui_response.animation_export_requested {
            // Check if already exporting (handle poisoned mutex gracefully)
            let already_exporting = self.export_status.lock()
                .map(|s| s.active)
                .unwrap_or(false);
            if !already_exporting {
                // The generator signals the export copies below must be
                // rendered over the duration it renders.
                self.sync_generator_signals();
            }
            if already_exporting {
                log::warn!("Animation export already in progress");
            } else if let Some(ref animation) = self.animation_controller.animation {
                use crate::animation::export::{AnimationExportConfig, export_animation, VideoEncodingSettings};

                // Clone config and override max_iterations from export settings
                let mut config = self.config_manager.active_config().clone();
                config.max_iterations = export_settings.max_iterations;
                // The frames are the picture's size (the View panel's
                // Output Size), or the size this export was given; the
                // exporter rounds an odd side down.
                let (width, height) = export_settings.size_override.unwrap_or(config.image_size);

                let export_config = AnimationExportConfig {
                    config,
                    animation: animation.clone(),
                    output_path: export_settings.output_path.clone(),
                    width,
                    height,
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
                    signals: {
                        let mut signals = self.signal_manager.clone_signals();
                        // The offset moves the muxed audio; the
                        // audio-analysis signals move with it, or the
                        // tracks react to sound the viewer hears
                        // `offset` seconds away. Generators and
                        // `.signal` files are not audio and stay put.
                        let offset = export_settings.audio_offset;
                        if export_settings.audio_file.is_some() && offset != 0.0 {
                            for name in self.audio_manager.available_signals() {
                                if let Some(signal) = signals.get_mut(name) {
                                    *signal = signal.shifted(offset);
                                }
                            }
                        }
                        signals
                    },
                };

                println!("Starting animation export (background thread)...");
                println!("  Output: {}", export_config.output_path.display());
                println!("  Resolution: {}x{} @ {} FPS", export_config.width, export_config.height, export_config.fps);
                println!("  Total frames: {}", export_config.total_frames());
                println!("  Codec: {} (CRF {})", export_config.video_settings.codec.display_name(), export_config.video_settings.quality);

                // Initialize the unified export status (handle poisoned mutex gracefully)
                let total_frames = export_config.total_frames();
                let res_label = format!(
                    "Exporting video · {}×{} · {} frames",
                    export_config.width, export_config.height, total_frames
                );
                if let Ok(mut s) = self.export_status.lock() {
                    s.begin(ExportKind::Video, res_label);
                }

                // Clone the status Arc for the background thread
                let status_arc = Arc::clone(&self.export_status);

                // Spawn background thread for export
                std::thread::spawn(move || {
                    let mut reporter = UiReporter::new(Arc::clone(&status_arc));

                    match pollster::block_on(export_animation(export_config, &mut reporter)) {
                        Ok(result) => {
                            println!("\nAnimation export complete!");
                            println!("  {} frames in {:.1}s", result.total_frames, result.total_time_ms / 1000.0);
                            println!("  Output: {}", result.output_path.display());

                            if let Ok(mut s) = status_arc.lock() {
                                s.finish_ok(format!(
                                    "Video saved · {}",
                                    result.output_path.file_name()
                                        .map(|n| n.to_string_lossy().into_owned())
                                        .unwrap_or_else(|| result.output_path.display().to_string())
                                ));
                            }
                        }
                        Err(e) => {
                            eprintln!("Animation export failed: {}", e);
                            if let Ok(mut s) = status_arc.lock() {
                                s.finish_err(format!("Video export failed: {e}"));
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
        // An export in the background freezes the viewport: nothing below
        // renders, and the picture it last drew stays on screen.
        let frozen = self.freeze_viewport();

        // Single config read after all updates are complete
        let final_config = self.config_manager.active_config();

        // Create new encoder for rendering phase
        let mut render_encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor {
            label: Some("Fractal Render Encoder"),
        });

        // Determine overwrite mode (smooth transitions during parameter changes)
        // Must be computed before mutable borrow of flame_renderer
        let use_overwrite = self.should_use_overwrite();
        // Read before the renderer is borrowed: the simulation driver
        // below needs it and the method takes `&self`.
        #[cfg(feature = "engine-sim")]
        let timeline_owns_sim = self.timeline_owns_sim();

        // Run flame compute shader with progressive refinement
        if let (false, Some(renderer)) = (frozen, self.flame_renderer.as_mut()) {
            renderer.set_overwrite_mode(use_overwrite);

            // Escape-time mode: the generator is one compute dispatch,
            // not a progressive chaos game — render it only when an
            // edit marked it dirty (or the viewport size changed), and
            // let the shared tonemap/effects tail below run every frame
            // exactly as it does for flames. `should_iterate` is forced
            // false so the whole chaos-game block (governor included)
            // idles without re-indenting it.
            let is_escape = final_config.render_mode
                == crate::scene::transforms::RenderMode::Escape;
            if !is_escape {
                self.egui_layer.update_escape_aa(None);
            }
            // The 3D terrain view of the escape picture
            // (docs/archive/terrain/heightfield-3d.md): a footprint -- the 2D
            // picture at the view -- rendered when its picture changes,
            // and the terrain drawn from it every frame something moves,
            // accumulating the antialiasing grid while nothing does.
            #[cfg(feature = "terrain")]
            let terrain_on = is_escape && final_config.escape.terrain_active();
            #[cfg(not(feature = "terrain"))]
            let terrain_on = false;
            #[cfg(feature = "terrain")]
            if terrain_on {
                self.egui_layer.update_escape_aa(None);
                // One engine at a time: the footprint is a full escape
                // render of its own.
                if let Some(esc) = self.escape_renderer.take() {
                    esc.destroy();
                }
                let (w, h) = (renderer.width, renderer.height);
                let device = &self.gpu.device;
                let queue = &self.gpu.queue;
                let terrain = self.escape_terrain.get_or_insert_with(|| {
                    #[allow(unused_mut)]
                    let mut t = crate::escape::footprint::EscapeTerrain::new(device, w, h);
                    // Reference orbits on the worker thread, as the
                    // 2D view's: a deep footprint refines over frames
                    // rather than stalling one.
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        t.footprint_renderer().progressive = true;
                    }
                    t
                });
                terrain.resize(device, w, h);
                // A 2D IFS formula draws the flame; nothing in the
                // escape config says when it changed.
                if let Some(def) = crate::escape::ifs::get_ifs(&final_config.escape.formula) {
                    let registry = crate::variations::global_registry();
                    let packed = self.ifs_pack_cache.pack_for(def, &final_config, &registry);
                    let fp = terrain.footprint_renderer();
                    fp.set_ifs(packed);
                    fp.ensure_coarse(device, queue, &final_config.escape, &final_config.flame);
                }
                // A texture's image arrives on its own schedule; the
                // config's recipe, which the footprint is keyed on,
                // did not change when it did.
                if self.escape_texture.update(terrain.footprint_renderer(), device, queue, &final_config.escape) {
                    terrain.invalidate_footprint();
                }
                if let Some(name) = self.escape_texture.take_notice() {
                    self.egui_layer.show_api_notification(&rust_i18n::t!("textures_panel.viewport_waiting", name = name), false);
                }
                let mut busy = self.escape_texture.busy();
                // The sections the view wants (plan section 12): as many
                // render steps a frame as fit about 8 ms, at the measured
                // section time, so a pan or a dolly fills in as it goes.
                // An export in the background has the GPU meanwhile: the
                // viewport keeps its picture, and resumes after.
                let exporting = self.export_status.lock().map(|s| s.active).unwrap_or(false);
                let steps = terrain.section_ms().map_or(1, |ms| (8.0 / ms.max(0.5)).floor().clamp(1.0, 4.0) as u32);
                if !exporting && terrain.update(
                    device,
                    queue,
                    &final_config,
                    w,
                    h,
                    renderer.escape_palette_view(final_config.escape.palette_map.stepped),
                    renderer.palette_generation(),
                    steps,
                ) {
                    busy = true;
                }
                if !exporting && terrain.render_viewport(device, queue, &final_config) {
                    busy = true;
                }
                // Not settled while paused; the export's cadence redraws.
                self.escape_dirty = busy || exporting;
                if busy {
                    self.window.request_redraw();
                }
            }
            #[cfg(feature = "terrain")]
            if is_escape && !terrain_on {
                if let Some(t) = self.escape_terrain.take() {
                    t.destroy();
                }
            }
            if is_escape && !terrain_on {
                // A frame part-way through its motion-blur moments went
                // with the renderer that held its accumulation.
                if self.escape_renderer.is_none() {
                    self.escape_motion = None;
                }
                let escape = self.escape_renderer.get_or_insert_with(|| {
                    let mut esc = crate::escape::EscapeRenderer::new(
                        &self.gpu.device,
                        renderer.width,
                        renderer.height,
                    );
                    // Interactive: reference orbits compute on the
                    // worker thread and deep frames refine
                    // progressively instead of stalling the UI.
                    #[cfg(not(target_arch = "wasm32"))]
                    {
                        esc.progressive = true;
                    }
                    esc
                });
                // The frame governor's antialiasing axis (aa_governor.rs):
                // while an animation plays, with the governor on, the
                // viewport's factor is governed to keep up with the
                // display. Otherwise, and the moment playback stops, it is
                // the config's.
                let aa_ceiling = crate::escape::EscapeRenderer::affordable_supersample(
                    &self.gpu.device,
                    renderer.width,
                    renderer.height,
                    final_config.escape.supersample,
                );
                let aa_governed = self.animation_controller.is_playing()
                    && self.config_manager.system_settings().frame_governor;
                let aa_factor = if aa_governed {
                    refresh_display_rate(&mut self.display_refresh_age, &mut self.display_refresh_mhz, window);
                    self.escape_aa.factor(aa_ceiling)
                } else {
                    self.escape_aa.release();
                    self.escape_aa_settled_at = None;
                    final_config.escape.supersample
                };
                if escape.resize(&self.gpu.device, renderer.width, renderer.height, aa_factor) {
                    self.escape_dirty = true;
                }
                crate::escape::set_viewport_render_lines(
                    renderer.height.saturating_mul(escape.effective_supersample()),
                );
                // The viewport's corner says what it is drawn at, and why
                // that is below the setting when it is.
                self.egui_layer.update_escape_aa(Some(aa_governor::readout(
                    escape.effective_supersample(),
                    final_config.escape.supersample,
                    aa_governed,
                    aa_ceiling,
                )));
                // Motion blur (motion_blur.rs): a new frame while it plays
                // starts its moments in the accumulation. Stopped or
                // paused, the sharp frame at the playhead is drawn again.
                // A path-traced solid renders display pixels of its own,
                // and is not blurred here (docs/projects/motion-blur.md,
                // phase 5).
                let solid_traced = crate::escape::ifs::formula_is_solid(&final_config.escape.formula)
                    && final_config.escape.solid_tier != crate::config::escape::RenderTier::Lit;
                if motion_blur::blurring(&self.animation_controller) && !solid_traced {
                    if self.escape_dirty && self.escape_motion.is_none() {
                        let moments = crate::animation::export::moment_configs(
                            final_config,
                            &self.animation_controller,
                            self.animation_controller.current_time,
                            Some(&self.signal_manager),
                        );
                        escape.begin_accumulation_of(&self.gpu.device, &self.gpu.queue, moments.len() as u32);
                        self.escape_motion = Some(motion_blur::Moments { moments, next: 0, rewind: false });
                    }
                } else if self.escape_motion.take().is_some() | escape.drop_presented() {
                    self.escape_dirty = true;
                }
                // What the escape pass renders: the moment, or the frame.
                let target = self.escape_motion.as_ref().map_or(final_config, |m| m.current());
                // Mode D renders the FLAME as a distance field, so a
                // flame edit has to reach the escape image — and
                // nothing else marks it dirty, because every other
                // escape formula is a function of the escape config
                // alone. Asked each frame, analysed again only when the
                // flame changed (`PackCache`); `set_ifs` reports whether
                // the packing changed, which is the cue.
                if let Some(def) = crate::escape::ifs::get_ifs(&final_config.escape.formula) {
                    let registry = crate::variations::global_registry();
                    let packed = self.ifs_pack_cache.pack_for(def, target, &registry);
                    if escape.set_ifs(packed) {
                        self.escape_dirty = true;
                    }
                    // The MEASURE colouring reads a coarse pass over
                    // the ball; without one it renders black. Keyed on
                    // the flame, so a pan or a zoom does not rebuild
                    // it.
                    escape.ensure_coarse(
                        &self.gpu.device,
                        &self.gpu.queue,
                        &target.escape,
                        &target.flame,
                    );
                    if escape.set_solid_lighting(
                        &final_config.solid_shading,
                        (
                            final_config.fog_strength,
                            final_config.fog_start,
                            final_config.background_color,
                        ),
                    ) {
                        self.escape_dirty = true;
                    }
                    if escape.set_solid_sky(crate::escape::path_core::sky_seen(&final_config.escape.path, &final_config)) {
                        self.escape_dirty = true;
                    }
                    if escape.set_solid_material(crate::escape::path_core::lit_material(&final_config)) {
                        self.escape_dirty = true;
                    }
                    if escape.set_solid_studio(crate::escape::path_core::studio_level(&final_config.escape.path, &final_config)) {
                        self.escape_dirty = true;
                    }
                    if escape.set_environment(
                        &self.gpu.device,
                        &self.gpu.queue,
                        crate::escape::path_core::environment_settings(&final_config.escape.path, &final_config),
                    ) {
                        self.escape_dirty = true;
                    }
                }
                // The interaction window. A mode-D walk at 1080p is
                // hundreds of milliseconds; a drag that re-walks at
                // every mouse event is a slideshow. Inside the window
                // the render is a quarter-resolution preview -- same
                // lighting, same look, blockier -- and when the edits
                // stop, one full render lands. The window is measured
                // from the LAST edit, so a drag stays in preview for
                // as long as it lasts and the full render arrives a
                // quarter-second after it ends.
                let preview = self
                    .escape_last_edit
                    .is_some_and(|t| t.elapsed() < ESCAPE_INTERACTION_WINDOW);
                if escape.set_preview(preview) {
                    // The last preview settled; nothing else would
                    // trigger the full render.
                    self.escape_dirty = true;
                }
                if preview {
                    // Keep the loop turning until the window lapses,
                    // or the full render never gets asked for.
                    self.window.request_redraw();
                }
                // The texture's image: looked at when the frame
                // re-renders anyway, or while the web generates one.
                if (self.escape_dirty || self.escape_texture.busy())
                    && self.escape_texture.update(escape, &self.gpu.device, &self.gpu.queue, &final_config.escape)
                {
                    self.escape_dirty = true;
                }
                if let Some(name) = self.escape_texture.take_notice() {
                    self.egui_layer.show_api_notification(&rust_i18n::t!("textures_panel.viewport_waiting", name = name), false);
                }
                if self.escape_texture.busy() {
                    self.window.request_redraw();
                }
                // A solid's tier (heightfield plan T3c): the walk alone
                // (Lit), the walk while anything moves and the path tracer
                // once it settles (Auto), or the path tracer alone.
                let solid_tier = if crate::escape::ifs::formula_is_solid(&final_config.escape.formula) {
                    final_config.escape.solid_tier
                } else {
                    crate::config::escape::RenderTier::Lit
                };
                if self.escape_dirty && solid_tier == crate::config::escape::RenderTier::PathTraced {
                    // No walk to wait for: the path tracer starts over.
                    escape.reset_solid_path();
                    self.escape_dirty = false;
                }
                if self.escape_dirty {
                    let settled = escape.render(
                        &self.gpu.device,
                        &self.gpu.queue,
                        &mut render_encoder,
                        &target.escape,
                        renderer.escape_palette_view(final_config.escape.palette_map.stepped),
                        renderer.palette_generation(),
                    );
                    // A motion-blur moment folds in as it settles, and the
                    // frame is done after its last; the finished frame is
                    // what the viewport shows until the next is.
                    let done = match self.escape_motion.as_mut() {
                        Some(motion) if settled => {
                            escape.accumulate_sample(&self.gpu.device, &self.gpu.queue, &mut render_encoder);
                            motion.next += 1;
                            motion.next == motion.moments.len()
                        }
                        Some(_) => false,
                        None => settled,
                    };
                    if done && self.escape_motion.take().is_some() {
                        escape.present_accumulation(&self.gpu.device, &mut render_encoder);
                    }
                    // Progressive deep zoom: an unsettled frame keeps
                    // the dirty flag so the next frame re-renders with
                    // the longer orbit prefix.
                    self.escape_dirty = !done;
                    if !done {
                        self.window.request_redraw();
                    }
                    // A settled playback frame: how long since the last one,
                    // against the governor's budget.
                    if done && self.escape_aa.active() {
                        let now = Instant::now();
                        if let Some(prev) = self.escape_aa_settled_at.replace(now) {
                            let settings = self.config_manager.system_settings();
                            let budget = crate::app::frame_budget(
                                settings.vsync_enabled,
                                settings.target_fps,
                                self.display_refresh_mhz,
                            );
                            let ratio = now.duration_since(prev).as_secs_f64() / budget;
                            if self.escape_aa.observe(ratio, aa_ceiling) {
                                self.window.request_redraw();
                            }
                        }
                    }
                }
                // The path tracer's call: short while the user is at it, so
                // the UI keeps about twenty frames a second; the export's
                // once nothing has been touched for two seconds and nothing
                // plays, which fills the card (`IDLE_FRAME_MS`).
                let idle = !self.animation_controller.is_playing()
                    && self.last_input_time.is_none_or(|t| t.elapsed() > std::time::Duration::from_secs(2));
                let frame_ms = if idle { crate::escape::renderer::IDLE_FRAME_MS } else { crate::escape::renderer::VIEWPORT_FRAME_MS };
                if solid_tier != crate::config::escape::RenderTier::Lit
                    && !self.escape_dirty
                    && escape.trace_solid_viewport(
                        &self.gpu.device,
                        &self.gpu.queue,
                        &final_config,
                        renderer.escape_palette_view(final_config.escape.palette_map.stepped),
                        frame_ms,
                    )
                {
                    self.window.request_redraw();
                }
            }

            // Simulation mode: a stateful grid stepped K times per
            // frame. Unlike escape there is no "settled" notion — the
            // picture is whatever step it has reached — so the frame
            // advances only while Run is engaged or a Step was asked
            // for, and ALWAYS recolours so a parameter or palette edit
            // is visible without advancing the simulation. Stepping to
            // show an edit would make the picture depend on how long
            // the user looked at it.
            #[cfg(feature = "engine-sim")]
            let is_sim = final_config.render_mode
                == crate::scene::transforms::RenderMode::Simulation;
            #[cfg(not(feature = "engine-sim"))]
            let is_sim = false;
            #[cfg(feature = "engine-sim")]
            if is_sim {
                let (w, h) = (renderer.width, renderer.height);
                // A frame part-way through its moments went with the
                // renderer that held their average.
                if self.sim_renderer.is_none() {
                    self.sim_motion = None;
                }
                let sim = self.sim_renderer.get_or_insert_with(|| {
                    crate::sim::SimRenderer::new(&self.gpu.device, &final_config.sim, w, h)
                });
                // A resize may change the grid (bound) or only the
                // resolve ratio (fixed); the renderer decides which and
                // reports whether the field survived.
                sim.resize(&self.gpu.device, &final_config.sim, w, h);
                // The flame's transforms are the layers' maps when the
                // config asks (simulation-layers plan, section 4).
                if final_config.sim.use_transforms {
                    sim.set_layer_transforms(&self.gpu.device, &self.gpu.queue, &final_config.flame);
                }
                if self.sim_reseed {
                    sim.request_seed();
                    self.sim_reseed = false;
                }
                // Budgeted walks of the grid, so a jump it cannot make in
                // one display frame is walked over the next few instead of
                // freezing the UI.
                const SIM_CATCHUP_BUDGET_MS: f64 = 8.0;
                // Motion blur (motion_blur.rs): a frame's moments, where
                // they differ from the frame -- a run the timeline does not
                // step, with no colouring track, is the same at every
                // moment. A terrain is phase 5's.
                let blur_moments = if motion_blur::blurring(&self.animation_controller) && !final_config.sim.terrain_active() {
                    let moments = crate::animation::export::moment_configs(
                        final_config,
                        &self.animation_controller,
                        self.animation_controller.current_time,
                        Some(&self.signal_manager),
                    );
                    if moments.iter().any(|m| m.sim != final_config.sim) { moments } else { Vec::new() }
                } else {
                    Vec::new()
                };
                if blur_moments.is_empty() {
                    self.sim_motion = None;
                    sim.drop_presented();
                }
                // A step count the timeline committed starts a blurred
                // frame's walk through its moments' step counts instead.
                if !blur_moments.is_empty() && self.sim_motion.is_none() {
                    if let Some(target) = self.sim_timeline_target.take() {
                        let index = if sim.will_reseed(&final_config.sim) { 0 } else { sim.step_index() };
                        sim.begin_moments(&self.gpu.device, &self.gpu.queue, blur_moments.len() as u32);
                        self.sim_motion = Some(motion_blur::Moments { moments: blur_moments.clone(), next: 0, rewind: target < index });
                    }
                }
                let walking = self.sim_motion.is_some();
                if let Some(motion) = self.sim_motion.as_mut() {
                    // Each moment's step count in turn, coloured as that
                    // moment colours it, folded into the frame. One behind
                    // the field is held where the field is, as playback
                    // holds a backward step count (an export runs back).
                    let m = motion.current();
                    let index = if sim.will_reseed(&m.sim) { 0 } else { sim.step_index() };
                    let target = if motion.rewind && motion.next == 0 { m.sim.steps } else { m.sim.steps.max(index) };
                    let budget = sim.steps_in(SIM_CATCHUP_BUDGET_MS);
                    let reached = sim.advance_to(
                        &self.gpu.device,
                        &self.gpu.queue,
                        &m.sim,
                        renderer.palette_view(),
                        target,
                        Some(budget),
                    );
                    if reached {
                        sim.add_moment(&self.gpu.device, &self.gpu.queue);
                        motion.next += 1;
                    }
                    if motion.next == motion.moments.len() {
                        sim.present_moments(&self.gpu.device, &self.gpu.queue);
                        self.sim_motion = None;
                    } else {
                        self.window.request_redraw();
                    }
                    self.sim_step_once = false;
                } else if let Some(target) = self.sim_timeline_target {
                    // THE TIMELINE OWNS THE STEP COUNT. The picture at
                    // a given time is the state at that step count, so
                    // the transport is inert here: a picture that
                    // depended on both the playhead and how long Run
                    // had been held would depend on how long the user
                    // looked at it.
                    //
                    // Budgeted, so a jump the grid cannot make in one
                    // display frame is walked over the next few
                    // instead of freezing the UI -- and a scrub can
                    // ask for a two-thousand-step jump on every slider
                    // event. `steps_in` sizes it from the measured
                    // cost of a step.
                    let budget = sim.steps_in(SIM_CATCHUP_BUDGET_MS);
                    let reached = sim.advance_to(
                        &self.gpu.device,
                        &self.gpu.queue,
                        &final_config.sim,
                        renderer.palette_view(),
                        target,
                        Some(budget),
                    );
                    if reached {
                        // Arrived: hand the grid back. Playback
                        // re-commits next frame; a scrub does not, so
                        // the run holds where the timeline left it and
                        // Run / Pause / Step work again from there.
                        self.sim_timeline_target = None;
                    } else {
                        // Still catching up -- keep the frames coming.
                        self.window.request_redraw();
                    }
                    self.sim_step_once = false;
                } else if timeline_owns_sim {
                    // Playing, but HOLDING: the track asked for a step
                    // count below the grid under continuous motion, so
                    // nothing was committed. The picture stays put --
                    // recoloured, so parameter tracks still show, but
                    // not stepped. Run is disengaged on playback start
                    // and the transport is greyed, so nothing else
                    // moves it either.
                    sim.render_frame(
                        &self.gpu.device,
                        &self.gpu.queue,
                        &final_config.sim,
                        renderer.palette_view(),
                        0,
                    );
                    self.sim_step_once = false;
                } else {
                    let steps = if self.sim_running {
                        final_config.sim.steps_per_frame
                    } else if self.sim_step_once {
                        1
                    } else {
                        0
                    };
                    self.sim_step_once = false;
                    // Was the run short of Max Steps before this frame?
                    // `render_frame` clamps the batch so it cannot pass
                    // the cap, so crossing it is exactly "was below, is
                    // now at" -- and a frame that reseeds counts as below,
                    // because the index it is about to measure from is 0.
                    let cap = final_config.sim.steps;
                    let was_below = cap > 0 && (sim.will_reseed(&final_config.sim) || sim.step_index() < cap);
                    sim.render_frame(
                        &self.gpu.device,
                        &self.gpu.queue,
                        &final_config.sim,
                        renderer.palette_view(),
                        steps,
                    );
                    // Reaching Max Steps pauses; it does not end the run.
                    // Pressing Run again carries on past the cap, because
                    // `steps_remaining` stops holding it back once the
                    // index is there. A reseed arms the pause again.
                    if self.sim_running
                        && crate::sim::should_pause_at_limit(cap, was_below, sim.step_index())
                    {
                        self.sim_running = false;
                        // THE PANEL FOR THIS FRAME WAS BUILT BEFORE THE
                        // STEP RAN, so it still says Pause and shows the
                        // previous step count. Without one more frame the
                        // window sleeps on that, and the last thing drawn
                        // is a stale panel over a finished picture.
                        self.window.request_redraw();
                    }
                    if self.sim_running {
                        self.window.request_redraw();
                    }
                }
                // A grid the timeline did not step this frame -- no step
                // track, or a held backward leg: the field as it now is,
                // recoloured at each moment and averaged, where the moments
                // differ in more than their step count.
                let recoloured = blur_moments.iter().any(|m| {
                    let mut c = m.sim.clone();
                    c.steps = final_config.sim.steps;
                    c != final_config.sim
                });
                if !walking && recoloured {
                    sim.begin_moments(&self.gpu.device, &self.gpu.queue, blur_moments.len() as u32);
                    for m in &blur_moments {
                        sim.color(&self.gpu.device, &self.gpu.queue, &m.sim, renderer.palette_view());
                        sim.add_moment(&self.gpu.device, &self.gpu.queue);
                    }
                    sim.present_moments(&self.gpu.device, &self.gpu.queue);
                }
                // The 3D terrain (heightfield plan T4): its ground made
                // from the field as this frame left it -- every step of a
                // run, so a run shows the lit tier -- then a frame of its
                // tiers, which path trace once the run rests.
                #[cfg(feature = "terrain")]
                if final_config.sim.terrain_active() {
                    let device = &self.gpu.device;
                    let queue = &self.gpu.queue;
                    // Fly mode's floor: the ground under the eye (P6).
                    let fly_ground = self.fly_mode && {
                        let s = self.config_manager.system_settings();
                        s.fly_collisions || s.fly_proximity
                    };
                    let terrain = self
                        .sim_terrain
                        .get_or_insert_with(|| crate::sim::terrain::SimTerrain::new(device, w, h));
                    terrain.resize(device, w, h);
                    terrain.update(device, queue, sim, &final_config, renderer.palette_view());
                    // An export in the background has the GPU meanwhile;
                    // its own cadence redraws the window.
                    let exporting = self.export_status.lock().map(|s| s.active).unwrap_or(false);
                    if !exporting && fly_ground {
                        terrain.probe_ground(device, queue, &final_config);
                    }
                    if !exporting && terrain.render_viewport(device, queue, &final_config) {
                        self.window.request_redraw();
                    }
                } else if let Some(t) = self.sim_terrain.take() {
                    t.destroy();
                }
            }

            // Everything below that asks "is this a chaos game?" must
            // treat both non-flame modes alike: idle the iteration
            // governor, skip the flame-only post-processing, and take
            // the generator's own image into the tonemap.
            let is_non_flame = is_escape || is_sim;

            // Check if we should continue iterating
            // During animation playback, always iterate (ignore max_iterations limit)
            // Skip GPU work during any export to avoid GPU contention (the export
            // runs on a background thread, often with its own device).
            let is_exporting = self.export_status.lock()
                .map(|s| s.active)
                .unwrap_or(false);
            // Keep the cylinder enumeration current with the view --
            // EVERY frame, not only while iterating: a plan made on the
            // background thread has to be picked up even if the render
            // it restarts had already finished.
            //
            // A true return means targeting started, stopped, or changed
            // arm, which changes the SHADER -- and the only path that
            // rebuilds it consistently with the sticky superset and the
            // variation-params packing is a full load.
            if !is_non_flame && !is_exporting {
                if renderer.sync_cylinders(&self.gpu.device, &self.gpu.queue, &final_config) {
                    renderer.load_config(
                        &self.gpu.device,
                        &mut render_encoder,
                        &self.gpu.queue,
                        &final_config,
                        &final_config.palette,
                        self.config_manager.system_settings().iterations_per_thread,
                        self.config_manager.system_settings().burn_in,
                    );
                }
                // A new plan arrived: what was accumulated was drawn
                // under the previous one (or none) and carries a
                // different weight, so the picture starts again.
                if renderer.take_plan_arrived() {
                    renderer.reset(
                        &mut render_encoder,
                        &self.gpu.queue,
                        self.config_manager.system_settings().iterations_per_thread,
                        final_config.zoom,
                        final_config.pan_x as f32,
                        final_config.pan_y as f32,
                        final_config.rotation,
                        final_config.camera_rotation_x,
                        final_config.camera_rotation_y,
                        final_config.camera_bank,
                        final_config.camera_x,
                        final_config.camera_y,
                        final_config.camera_z,
                        final_config.speed_factor,
                    );
                    self.frames_since_accumulation = 0;
                    self.rendering_complete = false;
                    // PathMap's per-pixel path ids are 1-based into the
                    // plan's words: under another plan they name other
                    // words, so they go with the picture.
                    self.clear_paths_next_frame = true;
                }
                // Keep frames coming while a plan is being made, so it
                // is picked up the moment it lands -- and, on the web,
                // because each frame is what runs the next slice of it.
                if renderer.plans_running() {
                    self.window.request_redraw();
                }
            }

            let max_iterations = Some(final_config.max_iterations);
            let should_iterate = !is_non_flame && !self.paused && !is_exporting && (
                is_controller_playing ||
                // Overwrite mode bypasses the max_iterations gate. With
                // it gated, a cheap flame that hits max during a long
                // drag would freeze: total_iterations stays at max,
                // should_iterate stays false, no compute runs, the
                // accumulator can't pick up new param state, and the
                // user is stuck looking at the pre-stop render. Always
                // iterate while overwriting; the compute_pass also
                // resets total_iterations each frame in overwrite mode,
                // so this gate only matters as a fallback.
                use_overwrite ||
                // Fresh accumulator (e.g., right after a resize cleared
                // the buffers) must be allowed to populate even when
                // max_iterations is very small. Without this, a flame
                // with max_iterations < one frame's sample count
                // (~32K) would stay black after every resize: the
                // accumulator gets cleared, `total_iterations` is also
                // reset, but the very first compute_pass overshoots
                // max_iterations and the next frame's
                // `should_iterate` gates compute off before the buffer
                // ever rebuilt enough for tonemap to produce output.
                renderer.samples_in_buffer() == 0 ||
                max_iterations.map_or(true, |max| renderer.total_iterations() < max)
            );

            // Mark rendering as complete the frame after max_iterations is reached
            if !should_iterate && !self.rendering_complete {
                self.rendering_complete = true;
                log::debug!("Rendering complete: max_iterations reached");
            }

            // Shadow-map auto-fit: when the first real bounds measurement
            // lands (early in a run), re-freeze the shadow fit and
            // restart accumulation so the maps cover the actual flame
            // instead of a zoom guess. Only fires while the run is young.
            if should_iterate
                && renderer.maybe_refit_shadow(
                    final_config.zoom,
                    final_config.pan_x as f32,
                    final_config.pan_y as f32,
                    final_config.camera_rotation_x,
                    final_config.camera_rotation_y,
                    final_config.camera_bank,
                    [final_config.camera_x, final_config.camera_y, final_config.camera_z],
                )
            {
                renderer.reset(
                    &mut render_encoder,
                    &self.gpu.queue,
                    self.config_manager.system_settings().iterations_per_thread,
                    final_config.zoom,
                    final_config.pan_x as f32,
                    final_config.pan_y as f32,
                    final_config.rotation,
                    final_config.camera_rotation_x,
                    final_config.camera_rotation_y,
                    final_config.camera_bank,
                    final_config.camera_x,
                    final_config.camera_y,
                    final_config.camera_z,
                    final_config.speed_factor,
                );
                self.frames_since_accumulation = 0;
            }

            if should_iterate {
                const NUM_WORKGROUPS: u32 = 128;

                // Adaptive iteration governor. The dispatch below is
                // otherwise a FIXED batch (128 workgroups x
                // iterations_per_thread) regardless of how heavy the flame
                // is — the frame pacer only ever waits when frames are
                // FAST, so a heavy flame (multi-emit spray/kaleidoscope,
                // fat variation stacks) turns directly into dropped FPS.
                // Measure the gap between successive iterating frames and
                // scale the batch to fit the target budget; the same total
                // GPU work happens across more, smaller batches, so
                // convergence speed is unchanged while the UI stays
                // responsive. (The compute-time metric can't drive this:
                // it measures command ENCODING, not GPU execution — the
                // frame delta is the only honest GPU-load signal here.)
                //
                // The governor SHORTENS the dispatch first, then sheds
                // workgroups (`Batch`): orbits persist across dispatches
                // (docs/archive/projects/persistent-orbits.md), so a shorter
                // dispatch costs no trajectory depth, and a full-width one
                // keeps the GPU occupied. Where shedding width stops
                // shortening the frame, the width is grown back while that
                // is free (`Knee`).
                // Under vsync the compositor blocks until the next refresh, so
                // this delta cannot go below the refresh interval however
                // little work we submit — landing exactly ON the target is the
                // healthy steady state, not a warning sign. `adjust_iter_scale`
                // grows the batch there for that reason; requiring the frame to
                // beat the target made the increase branch unreachable on every
                // vsync-locked platform, and the batch ratcheted to its
                // 1-workgroup floor on the first hitch and stayed. macOS and
                // WASM are both pinned to Fifo (WASM has no other option);
                // Windows with vsync off escaped it because Immediate lets the
                // delta track real GPU time.
                let now = web_time::Instant::now();
                let batch = {
                    let settings = self.config_manager.system_settings();
                    crate::app::Batch {
                        max_workgroups: NUM_WORKGROUPS,
                        iterations_per_thread: settings.iterations_per_thread,
                        burn_in: settings.burn_in,
                    }
                };
                if self.governor_batch != Some(batch) {
                    self.governor_batch = Some(batch);
                    self.governor_knee = crate::app::Knee::Shedding { over: 0 };
                }
                // **A tight plan being made** (tracker P10): the dispatch is
                // capped so the planner's round trips, queued behind it,
                // wait for little, and the governor HOLDS -- the planner's
                // GPU work lengthens these frames, and read as the render's
                // cost it would shed a batch that fits.
                let planning = renderer.planning_elapsed().is_some();
                // The governor can be switched off (Rendering menu): every
                // frame then dispatches the chosen workgroups, and the
                // governor neither measures nor adapts.
                let (governed, manual_workgroups) = {
                    let settings = self.config_manager.system_settings();
                    (settings.frame_governor, settings.manual_workgroups)
                };
                if let Some(prev) = self.last_iter_frame.filter(|_| !planning && governed) {
                    // Re-ask the platform every so often rather than every
                    // frame: the answer only changes when the window moves to
                    // a different display, and `current_monitor` is a syscall.
                    refresh_display_rate(&mut self.display_refresh_age, &mut self.display_refresh_mhz, window);

                    let settings = self.config_manager.system_settings();
                    let target = crate::app::frame_budget(
                        settings.vsync_enabled,
                        settings.target_fps,
                        self.display_refresh_mhz,
                    );
                    let ratio = prev.elapsed().as_secs_f64() / target;
                    // Median-of-3 filters the trigger. Frame deltas on a
                    // real desktop are NOISY — measured here: 5–12 ms
                    // against an 8.33 ms budget, with 12.7% of idle-scene
                    // frames poking past the old 1.3 shed threshold — and
                    // the old rule answered every single outlier with a
                    // ÷4 slash followed by a ten-frame ×1.15 regrow. That
                    // asymmetry is the reported 50–500 Miter/s
                    // fluctuation. The median makes an isolated hitch
                    // invisible while a REAL overload (2+ consecutive
                    // slow frames) still moves it in one extra frame.
                    let median = {
                        let [a, b] = self.governor_ratio_prev;
                        let c = ratio;
                        a.max(b).min(a.min(b).max(c)) // median of {a, b, c}
                    };
                    self.governor_ratio_prev = [self.governor_ratio_prev[1], ratio];
                    if self.batch_fill > 1.0 && (median > 1.3 || ratio >= 4.0) {
                        // Late with the dispatch lengthened past the batch
                        // (`fill_step`): give back the length first.
                        self.batch_fill = (self.batch_fill / ratio.clamp(1.15, 4.0)).max(1.0);
                    } else {
                        let (knee, scale) =
                            crate::app::governor_step(self.governor_knee, self.iter_scale, median, ratio, &batch);
                        self.governor_knee = knee;
                        self.iter_scale = scale;
                    }
                }
                self.last_iter_frame = Some(now);
                // **Filling the frame** (`fill_step`): at the full batch,
                // a dispatch the GPU says is short is made longer. Only
                // while an accumulation runs undisturbed: a restart, a drag
                // (overwrite mode) or playback may change what a dispatch
                // costs, so the fill starts again from the batch and waits
                // for a measurement of the new state (`fill_tag`).
                renderer.enable_dispatch_timing(&self.gpu.device, &self.gpu.queue);
                let filling = governed && !planning && !use_overwrite && !is_controller_playing;
                let total = renderer.total_iterations();
                if !filling || total < self.fill_last_total {
                    self.batch_fill = 1.0;
                    self.fill_tag += 1;
                    renderer.set_timing_tag(self.fill_tag);
                }
                self.fill_last_total = total;
                if let Some(timing) = renderer.take_dispatch_timing().filter(|t| t.tag == self.fill_tag) {
                    let at_full_batch = matches!(self.governor_knee, crate::app::Knee::Shedding { .. }) && self.iter_scale >= 1.0;
                    self.batch_fill = if at_full_batch {
                        let settings = self.config_manager.system_settings();
                        let budget_ms = crate::app::frame_budget(settings.vsync_enabled, settings.target_fps, self.display_refresh_mhz) * 1000.0;
                        let full = batch.max_workgroups as f64 * 64.0 * batch.iterations_per_thread as f64;
                        crate::app::fill_step(self.batch_fill, timing.ms, timing.iterations as f64, full, budget_ms)
                    } else {
                        1.0
                    };
                }
                // Floor of ONE workgroup: at extreme depth x emit x solid
                // stacks, even 4 workgroups (256 threads x 10k iterations
                // x 17 plot sources) blow the frame budget, and the only
                // remaining shed axis would be trajectory depth — which is
                // exactly what the governor exists to protect. One
                // workgroup still makes forward progress every frame.
                let (effective_workgroups, effective_ipt) = if !governed {
                    batch.fixed(manual_workgroups, planning)
                } else if planning {
                    batch.while_planning(self.governor_knee, self.iter_scale)
                } else {
                    batch.shape(self.governor_knee, self.iter_scale)
                };
                let effective_ipt = if self.batch_fill > 1.0
                    && (effective_workgroups, effective_ipt) == (batch.max_workgroups, batch.iterations_per_thread)
                {
                    ((effective_ipt as f64 * self.batch_fill).round() as u32).min(crate::app::MAX_FILLED_IPT)
                } else {
                    effective_ipt
                };

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

                // Motion blur (motion_blur.rs): the frame's batch shared
                // among its moments -- each moment's flame and view uploaded
                // in turn and drawn into the one histogram, folded once
                // below -- so a blurred frame costs what a sharp one does.
                // Not under Focused Rendering, whose plans are one view's,
                // nor for a subflame viewed in isolation, which the moments
                // (the main flame's) are not.
                let isolated = self.view_subflame_in_isolation
                    && matches!(self.config_manager.editing_target(), crate::config::manager::EditingTarget::Subflame { .. });
                let moments = if motion_blur::blurring(&self.animation_controller) && !final_config.cylinder_targeting && !isolated {
                    crate::animation::export::moment_configs(
                        final_config,
                        &self.animation_controller,
                        self.animation_controller.current_time,
                        Some(&self.signal_manager),
                    )
                } else {
                    Vec::new()
                };
                let samples_this_frame = if moments.is_empty() {
                    renderer.compute_pass(&mut render_encoder, &self.gpu.queue, &self.gpu.device, effective_workgroups,
                        effective_ipt, self.config_manager.system_settings().burn_in,
                        final_config.zoom, final_config.pan_x as f32, final_config.pan_y as f32, final_config.rotation,
                        final_config.camera_rotation_x, final_config.camera_rotation_y, final_config.camera_bank, final_config.camera_x, final_config.camera_y, final_config.camera_z, final_config.speed_factor, clear_histogram, clear_paths)
                } else {
                    // Each moment its own submission, after what this frame
                    // recorded before it: an upload lands at its submission,
                    // so the moment's dispatch sees its own flame.
                    let recorded = std::mem::replace(
                        &mut render_encoder,
                        self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor { label: Some("Render Encoder") }),
                    );
                    self.gpu.queue.submit(std::iter::once(recorded.finish()));
                    let (setting_ipt, burn_in) = {
                        let settings = self.config_manager.system_settings();
                        (settings.iterations_per_thread, settings.burn_in)
                    };
                    let moment_ipt = effective_ipt.div_ceil(moments.len() as u32).max(1);
                    let mut plotted = 0;
                    for (k, m) in moments.iter().enumerate() {
                        renderer.set_moment(&self.gpu.device, &self.gpu.queue, m, setting_ipt, burn_in);
                        renderer.set_continuing_frame(k > 0);
                        let mut encoder = self.gpu.device.create_command_encoder(&egui_wgpu::wgpu::CommandEncoderDescriptor { label: Some("Motion Blur Moment") });
                        plotted += renderer.compute_pass(&mut encoder, &self.gpu.queue, &self.gpu.device, effective_workgroups,
                            moment_ipt, burn_in,
                            m.zoom, m.pan_x as f32, m.pan_y as f32, m.rotation,
                            m.camera_rotation_x, m.camera_rotation_y, m.camera_bank, m.camera_x, m.camera_y, m.camera_z, m.speed_factor,
                            clear_histogram && k == 0, clear_paths && k == 0);
                        self.gpu.queue.submit(std::iter::once(encoder.finish()));
                    }
                    renderer.set_continuing_frame(false);
                    // The frame's own flame back on the GPU, where the next
                    // frame's edits -- and a stop -- expect it.
                    renderer.set_moment(&self.gpu.device, &self.gpu.queue, final_config, setting_ipt, burn_in);
                    plotted
                };

                self.metrics.record_compute_time(t_compute.elapsed().as_secs_f64() * 1000.0);
                // The histogram holds every frame of the batch: count them
                // all, frame by frame.
                if clear_histogram {
                    self.samples_in_batch = 0;
                }
                self.samples_in_batch += samples_this_frame;

                let t_accumulate = Instant::now();
                // 2. Accumulate samples - but only every N frames if batching enabled
                if should_accumulate {
                    renderer.accumulate_pass(&mut render_encoder, &self.gpu.queue, &self.gpu.device, self.samples_in_batch);

                    self.frames_since_accumulation = 0;
                    self.metrics.record_accumulate_time(t_accumulate.elapsed().as_secs_f64() * 1000.0);
                } else {
                    self.metrics.record_accumulate_time(0.0);
                }
            } else {
                self.metrics.record_compute_time(0.0);
                self.metrics.record_accumulate_time(0.0);
                // Not iterating: drop the governor's timing anchor and
                // neutralize its history. Without this, the first frame
                // after an idle stretch (max_iterations reached, pause,
                // export) measured the whole idle gap as one "frame",
                // hit the catastrophic clause, and resumed at a quarter
                // batch for no reason.
                self.last_iter_frame = None;
                self.governor_ratio_prev = [1.0, 1.0];
                self.governor_knee = crate::app::Knee::Shedding { over: 0 };
            }
            
            let t_tonemap = Instant::now();
            // 3. Update accumulation parameters from config
            renderer.set_blend_factor(final_config.blend_factor);
            renderer.set_use_dynamic_blend(final_config.use_dynamic_blend);

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

            renderer.update_tonemap(&self.gpu.queue, final_config.tonemap_mode, final_config.highlight_mode, final_config.use_curve,
                final_config.exposure, final_config.gamma, final_config.gamma_threshold, final_config.brightness,
                final_config.vibrancy, final_config.white_level, final_config.saturation, final_config.hue_shift,
                final_config.alpha_blend_low, final_config.alpha_blend_high,
                renderer.width, renderer.height, renderer.total_iterations(), final_config.max_iterations, final_config.zoom,
                self.config_manager.system_settings().iterations_per_thread, batch_size_for_tonemap, is_live_preview,
                // Escape mode: Levels normalizes by the chaos game's
                // measured sample density — stale flame data here, and
                // the escape image's density is a constant 1/px, so the
                // remap has no statistic to act on. Hard-off (the
                // panel says so too).
                final_config.effective_levels_enabled(),
                final_config.levels_low, final_config.levels_high, final_config.levels_gamma);

            // Reset effect slot counter for this frame (allows multiple effects with unique params)
            self.effect_chain.reset_slots();

            // Solid brightness renormalization: measure the accepted
            // density every few frames while occlusion culls (async, EMA-
            // smoothed) so hard solids tone-map at full brightness.
            if !is_non_flame {
                renderer.update_density_stats(&self.gpu.device, &self.gpu.queue, &mut render_encoder);
            }

            // Solid-rendering shade pass (lighting/SSAO on the depth buffer)
            // — runs before density effects; both consume HDR pre-tonemap data.
            let shade_ran = !is_non_flame && renderer.run_shade_pass(
                &self.gpu.device,
                &self.gpu.queue,
                &mut render_encoder,
                final_config.zoom,
                final_config.rotation,
                final_config.pan_x as f32,
                final_config.pan_y as f32,
                final_config.camera_rotation_x,
                final_config.camera_rotation_y,
                final_config.camera_bank,
                final_config.camera_x,
                final_config.camera_y,
                final_config.camera_z,
            );
            // Post-process DoF (solid mode) sits between shade and
            // density effects/tonemap.
            let dof_ran = !is_non_flame && renderer.run_dof_pass(
                &self.gpu.device,
                &self.gpu.queue,
                &mut render_encoder,
                shade_ran,
                final_config.zoom,
            );
            #[cfg(feature = "engine-sim")]
            let sim_view = if is_sim {
                Some(
                    sim_view(
                        &self.sim_renderer,
                        #[cfg(feature = "terrain")]
                        &self.sim_terrain,
                        final_config,
                    )
                    .expect("sim branch above created it"),
                )
            } else {
                None
            };
            #[cfg(not(feature = "engine-sim"))]
            let sim_view: Option<&wgpu::TextureView> = None;
            let pre_tonemap_view = if is_escape {
                escape_view(
                    &self.escape_renderer,
                    #[cfg(feature = "terrain")]
                    &self.escape_terrain,
                    final_config,
                )
                .expect("escape branch above created it")
            } else if let Some(v) = sim_view {
                v
            } else if dof_ran {
                renderer.dof_output_view()
            } else if shade_ran {
                renderer.shade_output_view()
            } else {
                renderer.get_accumulation_view()
            };

            // Run density effects (before tonemap, on HDR accumulation data).
            // A flame's alpha is a hit count, measured against its mean; an
            // escape or simulation image's is not, and is read as it is.
            let density_mean = if is_non_flame { 1.0 } else { renderer.mean_density() };
            let density_effects_ran = self.effect_chain.run_density_effects(
                &self.gpu.device,
                &self.gpu.queue,
                &mut render_encoder,
                pre_tonemap_view,
                &final_config.density_effects,
                density_mean,
            );

            // Render to internal fractal texture with tone mapping.
            // Input priority: density-effect output > shade output > accumulator.
            if density_effects_ran {
                if let Some(density_output) = self.effect_chain.get_density_output() {
                    renderer.tonemap_pass_with_input(&self.gpu.device, &self.gpu.queue, &mut render_encoder, density_output);
                } else if is_non_flame || dof_ran || shade_ran {
                    renderer.tonemap_pass_with_input(&self.gpu.device, &self.gpu.queue, &mut render_encoder, pre_tonemap_view);
                } else {
                    renderer.tonemap_pass(&self.gpu.queue, &mut render_encoder);
                }
            } else if is_non_flame || dof_ran || shade_ran {
                renderer.tonemap_pass_with_input(&self.gpu.device, &self.gpu.queue, &mut render_encoder, pre_tonemap_view);
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


        // Tell the UI what deep zoom decided this frame. Free: both
        // halves are already-measured renderer state, and the panel
        // needs them to report a DECISION rather than a request.
        if let Some(ref renderer) = self.flame_renderer {
            self.egui_layer.update_deep_zoom(crate::ui::DeepZoom {
                coverage: renderer.frame_coverage_fraction(),
                targeting: renderer.targeting_state().clone(),
                planning: renderer.planning_elapsed().map(|d| d.as_secs_f32()),
            });
        }

        // Update density histogram for Levels controls (every ~30 frames)
        // while the Histogram section shows it: the readback copies the
        // whole accumulator, and on desktop the frame waits for it.
        // Skip during animation playback to avoid frame drops from GPU readback
        let histogram_shown = self.egui_layer.histogram_shown();
        let histogram_opened = histogram_shown && !self.histogram_was_shown;
        self.histogram_was_shown = histogram_shown;
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

            // Opened: read now, not up to 30 frames on.
            if histogram_opened {
                self.histogram_frame_counter = 29;
            }
            if histogram_shown {
                self.histogram_frame_counter += 1;
            }
            if histogram_shown && self.histogram_frame_counter >= 30 {
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
                        // This readback BLOCKS for tens of milliseconds,
                        // and the governor measures frame deltas — so
                        // every 30th frame read as 4–8× over budget and
                        // tripped the catastrophic shed (traced: a slash
                        // to quarter batch every 30 frames like
                        // clockwork, with a 15-frame regrow tail). The
                        // stall is self-inflicted CPU waiting, not batch
                        // cost; dropping the timing anchor makes the
                        // governor skip the one delta that contains it.
                        self.last_iter_frame = None;
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

        // Texture previews (the Texture panel): the same pattern -- one
        // blocking generation a frame on desktop, spawned on the web.
        #[cfg(not(target_arch = "wasm32"))]
        if self.egui_layer.textures_need_previews() {
            self.egui_layer.generate_texture_preview(&self.gpu.device, &self.gpu.queue);
            window.request_redraw();
        }
        #[cfg(target_arch = "wasm32")]
        self.egui_layer.start_texture_previews(&self.gpu.device, &self.gpu.queue);

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

                    // The path this pixel was last drawn through.
                    let path = match pollster::block_on(renderer.read_path_at(&self.gpu.device, &self.gpu.queue, pixel_x, pixel_y)) {
                        Ok(path) => path,
                        Err(e) => {
                            log::error!("Failed to read the path at a pixel: {e}");
                            None
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
                        found_pixel: (pixel_x, pixel_y),
                        fractal_coords,
                        search_distance: 0.0, // No search, exact pixel
                        path,
                        color_preview,
                        preview_size: (9, 9),
                    };

                    self.egui_layer.set_path_click_info(Some(click_info));
                }
            }
        }

        let t5 = Instant::now();

        frame.present();

        // Non-blocking maintain. wgpu delivers buffer-map callbacks
        // only while the device is polled, and the escape renderer
        // reads its GPU timestamps through one. Poll, never Wait: this
        // must not stall the frame.
        #[cfg(not(target_arch = "wasm32"))]
        {
            let _ = self.gpu.device.poll(egui_wgpu::wgpu::PollType::Poll);
        }

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
        // f64 out, because this is an absolute position in the
        // fractal plane and that is the one thing a deep zoom
        // destroys -- the same reason `pan_x` is f64.
    ) -> (f64, f64) {
        // Convert pixel to normalized device coordinates (-1 to 1)
        let ndc_x = (pixel_x as f32 / width as f32) * 2.0 - 1.0;
        let ndc_y = (pixel_y as f32 / height as f32) * 2.0 - 1.0;

        // Account for aspect ratio
        let aspect = width as f32 / height as f32;
        let scaled_x = ndc_x * aspect;
        let scaled_y = ndc_y;

        // Screen space → pan frame (rotation-aware: both modes compose
        // pan → rotate → zoom)
        let (rotated_x, rotated_y) = config.screen_delta_to_pan_frame(scaled_x as f64, scaled_y as f64);

        // Apply inverse zoom and add pan
        let fractal_x = rotated_x / config.zoom as f64 + config.pan_x;
        let fractal_y = rotated_y / config.zoom as f64 + config.pan_y;

        (fractal_x, fractal_y)
    }
}

/// Adaptive batch governor (pure math; see the dispatch site — the scale
/// is applied to the WORKGROUP count so trajectory depth is untouched).
///
/// `ratio` is last frame's duration over the target budget. Over budget
/// by more than 30% -> shrink the batch proportionally (bounded, so one
/// catastrophic frame doesn't collapse the batch). Comfortably under
/// budget -> recover gently toward full batches. The dead zone between
/// avoids oscillation around the target.
/// The frame time the governor is trying to hit, in seconds.
///
/// With vsync ON the compositor only ever hands back whole refresh
/// intervals, so the display's rate IS the budget — and `target_fps` is
/// documented as vsync-off-only anyway. Asking for more than the panel
/// can give (144 on a 60Hz screen) is unreachable by construction, and
/// budgeting against it would shed work forever chasing a frame time
/// that will never arrive.
///
/// `refresh_mhz` is winit's millihertz — 60000 for 60Hz. `None` (WASM,
/// or a platform that will not say) falls back to 60Hz rather than to
/// `target_fps`, because under vsync the target is the thing we already
/// know to be wrong.
/// Re-ask the platform for the display's refresh rate every 120 calls
/// rather than every frame: it only changes when the window moves to a
/// different display, and `current_monitor` is a syscall.
fn refresh_display_rate(age: &mut u32, mhz: &mut Option<u32>, window: &Window) {
    if *age == 0 {
        *mhz = window.current_monitor().and_then(|m| m.refresh_rate_millihertz());
    }
    *age = (*age + 1) % 120;
}

pub(crate) fn frame_budget(vsync: bool, target_fps: f32, refresh_mhz: Option<u32>) -> f64 {
    if vsync {
        match refresh_mhz {
            Some(mhz) if mhz > 0 => 1000.0 / mhz as f64,
            _ => 1.0 / 60.0,
        }
    } else {
        1.0 / target_fps.max(1.0) as f64
    }
}

/// Additive-increase / multiplicative-decrease on the batch size, driven
/// by how the last frame compared to its budget (see the dispatch site).
///
/// Growth triggers at MEETING the budget, not beating it. A frame that
/// lands exactly on budget — `ratio == 1.0` — is the healthy steady state
/// under vsync, because the compositor hands back the refresh interval
/// however little work was submitted. The old threshold of 0.9 asked the
/// frame to come in 10% under a floor it could not go below, so the
/// increase branch was dead code on every vsync-locked platform and the
/// governor could only ever shed.
/// One governor rule for interactive rendering AND animation playback,
/// with three regimes. `median_ratio` is the median of the last three
/// frames' budget ratios (the caller maintains the window); `raw_ratio`
/// is this frame's alone.
///
/// * **Catastrophic** (`raw >= 4`): shed proportionally, immediately.
///   Scheduling jitter never quadruples a frame; only a genuinely
///   monstrous batch does (the extreme depth × emit × solid flames the
///   1-workgroup floor exists for). Waiting for a median here means
///   multi-second freezes.
/// * **Sustained overload** (median > 1.3): shed ÷1.15 per frame. The
///   median makes an ISOLATED slow frame invisible — measured on an
///   idle default scene, 12.7% of frames were outliers past 1.3, and
///   the old per-frame ÷4 slash + ten-frame ×1.15 regrow turned that
///   noise into a 10× throughput oscillation (field-reported as
///   "erratic 50–500 Miter/s"). Two consecutive slow frames move the
///   median, so real load still sheds with one frame of extra latency.
/// * **Headroom** (median < 1.05): grow ×1.15 per frame. Growing at
///   exactly-on-budget stays deliberate: under vsync the compositor
///   pins ratio at 1.0 however light the work, and requiring
///   under-budget frames there ratchets the batch to the floor (the
///   original macOS/WASM collapse).
///
/// Slew-limiting both directions also serves playback, where each batch
/// IS the displayed frame and a batch jump is visible grain flicker —
/// the ≤1.15 per-frame change is at the edge of perception. Playback
/// hitches big enough to trip the catastrophic clause were already
/// dropped frames; a one-off grain correction alongside is acceptable,
/// and it caps the slow-recovery tail the old playback-only rule had.
///
/// `floor` is the smallest scale: the batch's (`Batch::min_scale`).
pub(crate) fn adjust_iter_scale(scale: f64, median_ratio: f64, raw_ratio: f64, floor: f64) -> f64 {
    let mut s = scale;
    if raw_ratio >= 4.0 {
        s /= raw_ratio.min(4.0);
    } else if median_ratio > 1.3 {
        s /= 1.15;
    } else if median_ratio < 1.05 {
        s *= 1.15;
    }
    s.clamp(floor, 1.0)
}

/// The share of the frame budget the trajectory dispatch may fill on the
/// GPU when the governor lengthens it past the full batch (`fill_step`):
/// the rest is the tone map, the effects, the UI and the compositor's.
const FILL_TARGET: f64 = 0.5;
/// How much the fill may grow a measurement: about a quarter of a second
/// from the full batch to four times it.
const FILL_GROWTH: f64 = 1.25;
/// The longest the fill makes a dispatch, as a multiple of the batch and
/// in iterations a thread -- belt and braces against a mismeasured cost.
const MAX_FILL: f64 = 64.0;
pub(crate) const MAX_FILLED_IPT: u32 = 16_384;

/// **Filling the frame.** The governor only ever sheds from the full
/// batch (128 workgroups of `iterations_per_thread`), and under vsync it
/// cannot see how much of a frame that leaves idle: the frame lasts the
/// refresh interval however little the GPU did. Measured on a GTX 1660
/// SUPER at 754x786, Julian Disc's full batch at 1,000 a thread took
/// about 2 ms of a 16.7 ms frame -- the app drew 490 million iterations a
/// second against the CLI's 4.2 billion, and 4 billion took 8.1 s against
/// 0.94 s. So when the governor sits at the full batch and the GPU's own
/// clock (`gpu::timer`) says the dispatch is short, the dispatch is made
/// LONGER -- `fill` times the iterations a thread, at full width.
/// Persistent orbits make that free for the picture: a dispatch's length
/// only slices the work.
///
/// `gpu_ms` over `iterations` is a measured dispatch; `full_batch` the
/// full batch's thread-iterations; `budget_ms` the frame's. The fill
/// moves toward filling `FILL_TARGET` of the budget, growing at most
/// `FILL_GROWTH` a measurement and falling at once, never below 1 (the
/// batch) or above `MAX_FILL`.
pub(crate) fn fill_step(fill: f64, gpu_ms: f64, iterations: f64, full_batch: f64, budget_ms: f64) -> f64 {
    let per_iteration = gpu_ms / iterations.max(1.0);
    if !(per_iteration > 0.0) || !(full_batch > 0.0) {
        return fill;
    }
    let wanted = FILL_TARGET * budget_ms / (per_iteration * full_batch);
    wanted.min(fill * FILL_GROWTH).clamp(1.0, MAX_FILL)
}

/// Frames over budget at the floor before the governor asks whether
/// width was the constraint at all (`Knee`).
const KNEE_AFTER: u32 = 8;
/// Frames held at each probed width before its median is read: one of
/// lag, since a frame's delta is read at the next frame's start, and
/// three for the median.
const KNEE_SETTLE: u32 = 4;
/// A wider dispatch whose median frame costs at most this much more
/// than the narrower one's costs nothing that matters: it is kept.
const KNEE_FREE: f64 = 1.25;

/// **The dispatch's geometry, and how the governor's scale maps onto
/// it.** The scale SHORTENS the dispatch first, down to `ipt_floor`, and
/// only then sheds workgroups.
///
/// Orbits persist across dispatches (`docs/archive/projects/persistent-orbits.md`),
/// so `iterations_per_thread` only slices the work: a shorter dispatch
/// costs no trajectory depth, and full width keeps the GPU occupied. And
/// one thread's iterations alone can outlast the frame -- a targeted
/// sample replays its word, up to 96 maps at depth: julian-disc at zoom
/// 1,598 with 1,000 iterations per thread took about 58 ms at ANY width,
/// and a governor able only to shed width went to one workgroup and
/// stayed, at 1.1 million samples a second against the CLI's 76 million
/// (the same view at full width and 256 per thread: 80-87 million, 45
/// fps). Until orbits persisted it shortened only under targeting, where
/// a forced sample's position comes from its word, not from how long the
/// free orbit has run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Batch {
    pub max_workgroups: u32,
    pub iterations_per_thread: u32,
    pub burn_in: u32,
}

impl Batch {
    /// **The governor's smallest scale**: one workgroup at the shortest
    /// dispatch, the batch the heaviest flames (deep emit and solid
    /// stacks, a workgroup tens of milliseconds) need. A fixed share of
    /// the full batch is not enough once the dispatch shortens first:
    /// 1/256 of 128 workgroups of 1,000 is 8 workgroups of 64.
    pub fn min_scale(&self) -> f64 {
        self.ipt_floor() as f64 / (self.max_workgroups.max(1) as f64 * self.iterations_per_thread.max(1) as f64)
    }

    /// The shortest dispatch a batch is cut to: 64 iterations, or three
    /// burn-ins, never more than the setting.
    fn ipt_floor(&self) -> u32 {
        self.iterations_per_thread.min((3 * self.burn_in).max(64)).max(1)
    }

    /// Workgroups and iterations per thread for this frame.
    pub fn shape(&self, knee: Knee, scale: f64) -> (u32, u32) {
        let ipt = self.iterations_per_thread.max(1);
        let width = match knee {
            Knee::Probing { width, .. } | Knee::Holding { width, .. } => Some(width.clamp(1, self.max_workgroups)),
            Knee::Shedding { .. } => None,
        };
        let floor = self.ipt_floor();
        if let Some(w) = width {
            return (w, floor);
        }
        // The work of the full batch at this scale: full width while the
        // dispatch can shorten, then narrower at its floor.
        let short = ((ipt as f64 * scale).round() as u32).clamp(floor, ipt);
        let work = scale * self.max_workgroups as f64 * ipt as f64;
        let w = ((work / short as f64).round() as u32).clamp(1, self.max_workgroups);
        (w, short)
    }

    /// **The dispatch while a tight plan is made** (tracker P10): the
    /// governor's shape, no longer than the shortened floor. The planner
    /// submits to the render's queue, so each of its round trips waits
    /// behind the dispatch queued ahead of it: a 100,479-word julian-disc
    /// plan at 1e6 took 8.2 s behind 128x256 and 4.0 s behind 128x64 (4.5 s
    /// with no render at all). Whatever is drawn meanwhile is thrown away
    /// when the plan lands, so the length it gives up costs nothing kept.
    pub fn while_planning(&self, knee: Knee, scale: f64) -> (u32, u32) {
        let (w, ipt) = self.shape(knee, scale);
        (w, ipt.min(self.ipt_floor()))
    }

    /// **The dispatch with the governor off**: `workgroups` of the full
    /// length, every frame. While a tight plan is made it is still capped
    /// to the shortest dispatch -- that cap is the planner's, not the
    /// governor's (tracker P10: the plan's round trips queue behind it).
    pub fn fixed(&self, workgroups: u32, planning: bool) -> (u32, u32) {
        let ipt = self.iterations_per_thread.max(1);
        (workgroups.max(1), if planning { ipt.min(self.ipt_floor()) } else { ipt })
    }

    /// The width of the floor's dispatch.
    fn floor_width(&self) -> u32 {
        self.shape(Knee::Shedding { over: 0 }, self.min_scale()).0
    }

    /// The scale whose dispatch is `width` wide at the floor's length:
    /// where shedding resumes when a hold ends.
    fn scale_of(&self, width: u32) -> f64 {
        let per = self.ipt_floor() as f64 / (self.max_workgroups as f64 * self.iterations_per_thread.max(1) as f64);
        (width as f64 * per).clamp(self.min_scale(), 1.0)
    }
}

/// **Whether shedding width still helps.** The governor assumed a
/// frame's time grows with its width. On a GPU that runs every workgroup
/// of the batch at once, it does not: the frame lasts as long as one
/// thread's iterations, and shedding width throws throughput away
/// without shortening the frame. So when the governor sits at its floor
/// and frames are still over budget, it PROBES: it doubles the width
/// while the frame costs at most `KNEE_FREE` times more, and holds the
/// widest width that was free -- full width where the frame never grew.
/// Gradually, not at once: where width does cost (heavy solid and emit
/// stacks, the floor's reason), each step at most doubles the frame, and
/// the first costly step ends the probe.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Knee {
    /// Scaling by the budget (`adjust_iter_scale`); `over` counts frames
    /// over budget at the floor.
    Shedding { over: u32 },
    /// Trying `width`; `free` is the median at the last width found free,
    /// and `frames` how long `width` has been held.
    Probing { width: u32, free: f64, frames: u32 },
    /// Holding `width`, whose median was `at`.
    Holding { width: u32, at: f64 },
}

/// One frame of the governor: the knee's state and the scale, from this
/// frame's median and raw budget ratios.
pub(crate) fn governor_step(knee: Knee, scale: f64, median: f64, raw: f64, batch: &Batch) -> (Knee, f64) {
    match knee {
        Knee::Shedding { over } => {
            let s = adjust_iter_scale(scale, median, raw, batch.min_scale());
            let floor_width = batch.floor_width();
            if s <= batch.min_scale() && median > 1.3 && floor_width < batch.max_workgroups {
                let over = over + 1;
                if over >= KNEE_AFTER {
                    let width = (floor_width * 2).min(batch.max_workgroups);
                    return (Knee::Probing { width, free: median, frames: 0 }, s);
                }
                return (Knee::Shedding { over }, s);
            }
            (Knee::Shedding { over: 0 }, s)
        }
        Knee::Probing { width, free, frames } => {
            // Under budget: the flame got lighter. Shed and grow from here.
            if median < 1.05 {
                return (Knee::Shedding { over: 0 }, batch.scale_of(width));
            }
            let narrower = (width / 2).max(batch.floor_width());
            // One frame this far past the last free width: widening is not
            // free, and waiting for the median would hold it.
            if raw >= 4.0 * free {
                return (Knee::Holding { width: narrower, at: free }, scale);
            }
            let frames = frames + 1;
            if frames < KNEE_SETTLE {
                return (Knee::Probing { width, free, frames }, scale);
            }
            if median <= free * KNEE_FREE {
                if width >= batch.max_workgroups {
                    return (Knee::Holding { width, at: median }, scale);
                }
                return (Knee::Probing { width: (width * 2).min(batch.max_workgroups), free: median, frames: 0 }, scale);
            }
            (Knee::Holding { width: narrower, at: free }, scale)
        }
        Knee::Holding { width, at } => {
            if median < 1.05 {
                // Under budget: the flame got lighter.
                return (Knee::Shedding { over: 0 }, batch.scale_of(width));
            }
            if median > 2.0 * at {
                // Heavier: back to the floor, and probe again from there.
                return (Knee::Shedding { over: 0 }, batch.min_scale());
            }
            (Knee::Holding { width, at }, scale)
        }
    }
}

#[cfg(test)]
mod governor_tests {
    use super::{adjust_iter_scale, fill_step, frame_budget, governor_step, Batch, Knee, MAX_FILL};

    /// The slew tests' floor: the scale alone, without a batch.
    const MIN_ITER_SCALE: f64 = 1.0 / 256.0;

    /// The governor with the knee and the dispatch's shape, driven by a
    /// model of what a dispatch costs: `frame(workgroups, ipt)` in
    /// seconds, against a 60 Hz budget. Returns each frame's shape and
    /// time.
    fn drive(batch: Batch, frames: usize, frame: &dyn Fn(u32, u32) -> f64) -> Vec<(u32, u32, f64)> {
        const BUDGET: f64 = 1.0 / 60.0;
        let (mut scale, mut knee, mut prev) = (1.0f64, Knee::Shedding { over: 0 }, [1.0f64, 1.0]);
        let mut out = Vec::new();
        for _ in 0..frames {
            let (w, ipt) = batch.shape(knee, scale);
            let t = frame(w, ipt);
            out.push((w, ipt, t));
            let ratio = t / BUDGET;
            let [a, b] = prev;
            let median = a.max(b).min(a.min(b).max(ratio));
            prev = [prev[1], ratio];
            (knee, scale) = governor_step(knee, scale, median, ratio, &batch);
        }
        out
    }

    /// **While a tight plan is made the dispatch is short** (tracker P10),
    /// and never heavier than the governor's own shape.
    #[test]
    fn a_dispatch_while_planning_is_short() {
        let batch = Batch { max_workgroups: 128, iterations_per_thread: 256, burn_in: 20 };
        let knees = [
            Knee::Shedding { over: 0 },
            Knee::Probing { width: 16, free: 1.0, frames: 0 },
            Knee::Holding { width: 64, at: 1.0 },
        ];
        for knee in knees {
            for scale in [1.0, 0.5, 0.1, batch.min_scale()] {
                let (w, ipt) = batch.shape(knee, scale);
                let (pw, pipt) = batch.while_planning(knee, scale);
                assert_eq!(pw, w, "{knee:?} at {scale}: the width changed");
                assert!(pipt <= 64 && pipt <= ipt, "{knee:?} at {scale}: {pipt} iterations a thread, against {ipt}");
            }
        }
        // The full batch, as the app runs it on a light flame.
        assert_eq!(batch.while_planning(Knee::Shedding { over: 0 }, 1.0), (128, 64));
        // A setting shorter than the floor stays as it is.
        let batch = Batch { max_workgroups: 128, iterations_per_thread: 32, burn_in: 20 };
        assert_eq!(batch.while_planning(Knee::Shedding { over: 0 }, 1.0).1, 32);
    }

    /// A GPU that runs every workgroup of the batch at once: the frame
    /// lasts as long as one thread's iterations. 80 us an iteration is a
    /// targeted sample's replay at depth: 1,000 iterations, 84 ms.
    fn latency_bound(_w: u32, ipt: u32) -> f64 {
        0.004 + ipt as f64 * 80e-6
    }

    /// **A render shortens its dispatch and keeps its width** (reported,
    /// targeted: julian-disc at zoom 1,598, 1,000 iterations per thread,
    /// one workgroup at 17 fps, 1.1 million samples a second). Every
    /// render, since orbits persist.
    #[test]
    fn a_render_shortens_its_dispatch_and_keeps_its_width() {
        let batch = Batch { max_workgroups: 128, iterations_per_thread: 1000, burn_in: 20 };
        let run = drive(batch, 300, &latency_bound);
        let (w, ipt, t) = *run.last().unwrap();
        assert_eq!(w, 128, "the width was shed: {run:?}");
        assert!(ipt >= 100 && t <= 1.3 / 60.0, "{ipt} iterations per thread, {:.1} ms a frame", t * 1e3);
        // Plotted samples a second, against one workgroup at the full length.
        let now = w as f64 * 64.0 * (ipt - 20) as f64 / t;
        let before = 64.0 * 980.0 / latency_bound(1, 1000);
        assert!(now > 50.0 * before, "{now:.3e} samples a second against {before:.3e}");
    }

    /// A latency-bound frame too long even at the floor's length: 400 us an
    /// iteration, 30 ms at 64 iterations a thread, at any width.
    fn too_slow(_w: u32, ipt: u32) -> f64 {
        0.004 + ipt as f64 * 400e-6
    }

    /// **Shedding that does not shorten the frame is undone.** Latency-
    /// bound past the shortest dispatch: shortening goes to its floor,
    /// shedding width then buys nothing, and the width goes back to full
    /// once the floor proves no faster.
    #[test]
    fn shedding_that_does_not_help_is_undone() {
        let batch = Batch { max_workgroups: 128, iterations_per_thread: 1000, burn_in: 20 };
        let run = drive(batch, 400, &too_slow);
        let (w, ipt, _) = *run.last().unwrap();
        assert_eq!((w, ipt), (128, 64), "the batch stayed shed: {:?}", &run[run.len() - 5..]);
    }

    /// **Where width costs, the floor holds.** Each workgroup 20 ms: the
    /// heavy solid and emit stacks the one-workgroup floor exists for. The
    /// probe tries two workgroups, finds them costly, and goes back --
    /// never a frame much past twice the floor's.
    #[test]
    fn where_width_costs_the_floor_holds() {
        let batch = Batch { max_workgroups: 128, iterations_per_thread: 1000, burn_in: 20 };
        let heavy = |w: u32, _ipt: u32| 0.004 + w as f64 * 0.020;
        let run = drive(batch, 400, &heavy);
        let settled = &run[100..];
        assert!(settled.iter().all(|(w, _, _)| *w == 1), "left the floor: {:?}", &run[run.len() - 5..]);
        let worst = run[50..].iter().map(|(_, _, t)| *t).fold(0.0, f64::max);
        assert!(worst <= 2.0 * heavy(1, 1000) + 1e-9, "a probe frame took {:.0} ms", worst * 1e3);
    }

    /// **The knee is where widening starts to cost**: a GPU that runs 16
    /// workgroups at once, then another wave, each wave over budget even at
    /// the floor's length.
    #[test]
    fn the_knee_is_where_widening_starts_to_cost() {
        let batch = Batch { max_workgroups: 128, iterations_per_thread: 1000, burn_in: 20 };
        let waves = |w: u32, ipt: u32| 0.004 + ipt as f64 * 400e-6 * ((w + 15) / 16) as f64;
        let run = drive(batch, 400, &waves);
        assert_eq!(run.last().unwrap().0, 16, "{:?}", &run[run.len() - 5..]);
    }

    /// **A lighter flame leaves the hold**, and the batch grows back to
    /// full.
    #[test]
    fn a_lighter_flame_leaves_the_hold() {
        let batch = Batch { max_workgroups: 128, iterations_per_thread: 1000, burn_in: 20 };
        let (mut scale, mut knee, mut prev) = (1.0f64, Knee::Shedding { over: 0 }, [1.0f64, 1.0]);
        let mut shapes = Vec::new();
        for i in 0..600 {
            let (w, ipt) = batch.shape(knee, scale);
            let t = if i < 300 { too_slow(w, ipt) } else { 0.004 + w as f64 * 20e-6 };
            let ratio = t * 60.0;
            let [a, b] = prev;
            let median = a.max(b).min(a.min(b).max(ratio));
            prev = [prev[1], ratio];
            (knee, scale) = governor_step(knee, scale, median, ratio, &batch);
            shapes.push((w, knee));
        }
        assert!(matches!(shapes[299].1, Knee::Holding { width: 128, .. }), "not holding before: {:?}", shapes[299]);
        assert!(matches!(knee, Knee::Shedding { .. }) && (scale - 1.0).abs() < 1e-9, "{knee:?} at scale {scale}");
    }

    /// **With the governor off** every frame is the chosen workgroups at
    /// the full length -- but for the planner's cap while a plan is made.
    #[test]
    fn with_the_governor_off_the_dispatch_is_fixed() {
        let batch = Batch { max_workgroups: 128, iterations_per_thread: 1000, burn_in: 20 };
        assert_eq!(batch.fixed(512, false), (512, 1000));
        assert_eq!(batch.fixed(512, true), (512, 64));
        assert_eq!(batch.fixed(0, false), (1, 1000));
    }

    /// The floor's dispatch, and a hold's return to shedding, keep the
    /// shape consistent: the full batch shortens first, and the floor is
    /// one workgroup of the shortest dispatch.
    #[test]
    fn the_floor_is_the_smallest_batch() {
        let batch = Batch { max_workgroups: 128, iterations_per_thread: 1000, burn_in: 20 };
        assert_eq!(batch.shape(Knee::Shedding { over: 0 }, 1.0), (128, 1000));
        assert_eq!(batch.shape(Knee::Shedding { over: 0 }, 0.25), (128, 250));
        assert_eq!(batch.shape(Knee::Shedding { over: 0 }, batch.min_scale()), (1, 64));
        assert_eq!(batch.scale_of(1), batch.min_scale());
        // A setting under the floor is left alone.
        let short = Batch { iterations_per_thread: 32, ..batch };
        assert_eq!(short.shape(Knee::Shedding { over: 0 }, short.min_scale()), (1, 32));
    }

    /// Drive the governor the way the app does: a rolling median-of-3
    /// window over the per-frame ratios.
    struct Gov {
        scale: f64,
        prev: [f64; 2],
    }
    impl Gov {
        fn new() -> Self {
            Self { scale: 1.0, prev: [1.0, 1.0] }
        }
        fn step(&mut self, ratio: f64) -> f64 {
            let [a, b] = self.prev;
            let median = a.max(b).min(a.min(b).max(ratio));
            self.prev = [self.prev[1], ratio];
            self.scale = adjust_iter_scale(self.scale, median, ratio, MIN_ITER_SCALE);
            self.scale
        }
    }

    /// `target_fps` is documented as vsync-off-only. Honour that: under
    /// vsync the budget is the panel's, and a target it cannot reach must
    /// not turn into a permanent shed.
    #[test]
    fn vsync_budget_comes_from_the_display_not_target_fps() {
        let b = frame_budget(true, 144.0, Some(60_000));
        assert!((b - 1.0 / 60.0).abs() < 1e-9, "{b}");
        assert!((frame_budget(true, 60.0, Some(120_000)) - 1.0 / 120.0).abs() < 1e-9);
        assert!((frame_budget(true, 144.0, None) - 1.0 / 60.0).abs() < 1e-9);
        assert!((frame_budget(true, 144.0, Some(0)) - 1.0 / 60.0).abs() < 1e-9);
        assert!((frame_budget(false, 144.0, Some(60_000)) - 1.0 / 144.0).abs() < 1e-9);
        assert!(frame_budget(false, 0.0, None).is_finite());
    }

    /// THE reported regression: measured on an idle default scene,
    /// 12.7% of frames were isolated outliers past the shed threshold
    /// (desktop frame timing is just that noisy), and the old rule
    /// answered each with a ÷4 slash + ten-frame regrow — a permanent
    /// 10× throughput oscillation, field-reported as "erratic 50–500
    /// Miter/s where it used to be a solid 500". Isolated outliers must
    /// now be INVISIBLE: a light flame under realistic noise holds full
    /// batch.
    #[test]
    fn isolated_frame_spikes_do_not_move_the_batch()  {
        let mut g = Gov::new();
        let mut min_scale: f64 = 1.0;
        for i in 0..600 {
            // A comfortably light flame (ratio ~0.75) with a 2.4×
            // outlier every 8th frame — the measured shape of the idle
            // trace (one-frame hitches from the compositor, panel
            // repaints, timer jitter).
            let ratio = if i % 8 == 7 { 2.4 } else { 0.75 };
            min_scale = min_scale.min(g.step(ratio));
        }
        assert!(
            (min_scale - 1.0).abs() < 1e-9,
            "isolated spikes dented the batch to {min_scale} — the 50–500 oscillation is back"
        );
    }

    /// Sustained overload is not noise and must still shed — with one
    /// frame of added latency (the median needs two slow frames), then
    /// at the slewed ÷1.15 rate.
    #[test]
    fn sustained_overload_sheds_within_frames() {
        let mut g = Gov::new();
        // Steady 2× over budget.
        let mut scales = Vec::new();
        for _ in 0..60 {
            scales.push(g.step(2.0));
        }
        assert!(scales[0] > 0.99, "first slow frame alone must not shed yet");
        assert!(scales[2] < 1.0, "by the third consecutive slow frame the median has moved");
        assert!(*scales.last().unwrap() < 0.51, "sustained 2× must shed to ~half batch");
    }

    /// A catastrophic frame (4×+ budget) is unambiguous — scheduling
    /// jitter never quadruples a frame, only a monstrous batch does —
    /// and waiting two frames for the median means multi-second freezes
    /// on the flames the 1-workgroup floor exists for. Immediate
    /// proportional shed, no filter.
    #[test]
    fn a_catastrophic_frame_sheds_immediately() {
        let mut g = Gov::new();
        let s = g.step(30.0);
        assert!(s <= 0.25 + 1e-9, "4×+ frame must shed proportionally at once, got {s}");
    }

    /// The macOS/WASM collapse, carried forward: under vsync the
    /// compositor pins the delta at whole refresh intervals, so the
    /// healthy steady state is ratio EXACTLY 1.0 — which must grow, or
    /// an occasional hitch ratchets the batch to the floor with nothing
    /// to earn it back.
    #[test]
    fn vsync_quantized_frames_do_not_ratchet_the_batch_away() {
        const REFRESH: f64 = 1.0 / 60.0;
        let target = REFRESH;
        let present = |s: f64, fixed: f64, per_batch: f64| {
            ((fixed + per_batch * s) / REFRESH).ceil().max(1.0) * REFRESH
        };

        // A flame that comfortably fits, with a hitch every 20 frames.
        let mut g = Gov::new();
        for i in 0..400 {
            let mut delta = present(g.scale, 0.004, 0.012);
            if i % 20 == 0 {
                delta += REFRESH;
            }
            g.step(delta / target);
        }
        assert!(g.scale > 0.99, "vsync-locked frames collapsed the batch to {}", g.scale);

        // ...and one that genuinely does not fit must still shed.
        let mut h = Gov::new();
        for _ in 0..400 {
            let d = present(h.scale, 0.004, 0.400);
            h.step(d / target);
        }
        assert!(h.scale < 0.25, "overloaded flame failed to shed work: {}", h.scale);
    }

    /// The unreachable-target case end to end: a 144fps target on a 60Hz
    /// panel must not read as permanently over budget.
    #[test]
    fn unreachable_target_fps_does_not_starve_the_batch() {
        const REFRESH: f64 = 1.0 / 60.0;
        let budget = frame_budget(true, 144.0, Some(60_000));
        let mut g = Gov::new();
        for _ in 0..200 {
            g.step(REFRESH / budget);
        }
        assert!(g.scale > 0.99, "unreachable target starved the batch to {}", g.scale);
    }

    /// Playback's anti-flicker property, now provided by the unified
    /// rule: outside the catastrophic clause, consecutive-frame batch
    /// changes stay within the ~15% a viewer cannot see as grain
    /// flicker — including through the mid-range equilibrium where the
    /// old unfiltered AIMD saw-toothed at ÷2–÷4 per cycle.
    #[test]
    fn batch_changes_stay_below_flicker_threshold_at_equilibrium() {
        const REFRESH: f64 = 1.0 / 60.0;
        let target = REFRESH;
        let present = |s: f64, fixed: f64, per_batch: f64| {
            ((fixed + per_batch * s) / REFRESH).ceil().max(1.0) * REFRESH
        };
        // Costs put the equilibrium mid-range — the sawtooth regime.
        let mut g = Gov::new();
        let mut worst_jump: f64 = 1.0;
        let mut prev_scale = g.scale;
        for i in 0..400 {
            let d = present(g.scale, 0.004, 0.022);
            g.step(d / target);
            if i > 50 {
                worst_jump = worst_jump.max((g.scale / prev_scale).max(prev_scale / g.scale));
            }
            prev_scale = g.scale;
        }
        assert!(
            worst_jump <= 1.16,
            "batch jumped {worst_jump:.2}× between frames — visible grain flicker in playback"
        );
    }
    /// The fill grows toward half the frame on the GPU, a quarter more a
    /// measurement at most, and sits there: Julian Disc's full batch at
    /// 2 ms of a 16.7 ms frame settles near 4.2 times the batch.
    #[test]
    fn the_fill_grows_toward_half_the_frame() {
        let (full, budget) = (128.0 * 64.0 * 1000.0, 1000.0 / 60.0);
        let per = 2.0 / full;
        let mut fill = 1.0;
        let mut steps = 0;
        loop {
            let next = fill_step(fill, per * full * fill, full * fill, full, budget);
            assert!(next <= fill * 1.25 + 1e-9, "grew {fill} to {next}");
            if (next - fill).abs() < 1e-9 {
                break;
            }
            fill = next;
            steps += 1;
            assert!(steps < 20, "did not settle: {fill}");
        }
        let gpu_ms = per * full * fill;
        assert!((gpu_ms - budget / 2.0).abs() < 0.01, "settled at {gpu_ms} ms of {budget}");
    }

    /// A dispatch costlier than half the frame falls at once, never below
    /// the batch; a cheap one stops at the cap; a measurement of nothing
    /// changes nothing.
    #[test]
    fn the_fill_falls_at_once_and_is_bounded() {
        let (full, budget) = (128.0 * 64.0 * 256.0, 1000.0 / 60.0);
        // Measured at fill 8: 40 ms. Half the frame is 8.3 ms: a fifth of it.
        let fallen = fill_step(8.0, 40.0, full * 8.0, full, budget);
        assert!((fallen - 8.0 * (budget / 2.0) / 40.0).abs() < 1e-9, "{fallen}");
        assert_eq!(fill_step(1.0, 40.0, full, full, budget), 1.0);
        let mut fill = 1.0;
        for _ in 0..100 {
            fill = fill_step(fill, 0.001 * fill, full * fill, full, budget);
        }
        assert_eq!(fill, MAX_FILL);
        assert_eq!(fill_step(3.0, 0.0, full, full, budget), 3.0);
        assert_eq!(fill_step(3.0, f64::NAN, full, full, budget), 3.0);
    }
}
