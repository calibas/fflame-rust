// Load I18n macro to allow use of `t!` macro anywhere
#[macro_use]
extern crate rust_i18n;

// Initialize rust-i18n with the locales directory
i18n!("locales", fallback = "en");

mod app;
pub mod gpu;
mod ui;
mod util;
pub mod scene;
pub mod renderer;
pub mod config;
pub mod profiler;
pub mod version;
pub mod variations;
pub mod png_metadata;
pub mod flame_xml;
pub mod i18n;
pub mod animation;
pub mod signal;
pub mod storage;
pub mod export;
pub mod resources;
pub mod effects;
pub mod audio;
pub mod api;
// mod shader_builder; // Legacy - replaced by shader_builder_v2
mod shader_builder_v2;
mod shader_cache;
/// Golden-file dumps of the generated WGSL (see `tests/shader_dumps/`).
#[cfg(test)]
mod shader_dumps;

#[cfg(target_arch = "wasm32")]
pub mod wasm_api;
#[cfg(target_arch = "wasm32")]
mod web_clipboard;
#[cfg(target_arch = "wasm32")]
mod web_text_agent;

// Prelude for convenient imports
pub mod prelude {
    pub use crate::scene::presets::PresetLibrary;
    pub use crate::scene::palette::{PaletteLibrary, ColorMode};
    pub use crate::scene::tonemap::{ToneCurve, ToneMapMode};
    pub use crate::renderer::compute_kernel::FlameRenderer;
    pub use crate::config::FractalConfig;
}

use app::App;
use winit::dpi::PhysicalSize;

/// Force a specific export engine from the CLI (`export --engine`); see app::export.
#[cfg(not(target_arch = "wasm32"))]
pub use app::export::ExportEngine;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen(start)]
pub async fn wasm_main() {
    // Set up panic hook for better error messages in browser console
    console_error_panic_hook::set_once();

    // Initialize logging
    console_log::init_with_level(log::Level::Info).expect("Failed to initialize logger");

    run().await.expect("Failed to run app");
}

#[cfg(not(target_arch = "wasm32"))]
pub fn desktop_main() {
    env_logger::init();
    pollster::block_on(run()).expect("Failed to run app");
}

#[cfg(not(target_arch = "wasm32"))]
pub fn export_mode(input: &str, output: &str, width: Option<u32>, height: Option<u32>, category: Option<String>, iterations_per_thread: Option<u32>, dump_shader: bool, transparent: bool, premultiplied: bool, engine: crate::app::export::ExportEngine, supersample: bool) {
    env_logger::init();
    if dump_shader {
        shader_builder_v2::enable_shader_dump();
    }
    // Enable inlined constants for CLI export - compiles flame data as shader constants
    // for maximum performance (eliminates buffer reads, enables dead code elimination)
    shader_builder_v2::enable_inlined_constants();
    pollster::block_on(export_async(input, output, width, height, category, iterations_per_thread, transparent, premultiplied, engine, supersample)).expect("Export failed");
}

#[cfg(not(target_arch = "wasm32"))]
pub fn export_animation_mode(
    config_path: &str,
    animation_path: &str,
    output_path: &str,
    width: u32,
    height: u32,
    fps: u32,
    iterations_per_thread: u32,
    video_settings: animation::export::VideoEncodingSettings,
    audio: Option<animation::export::AudioExportConfig>,
) {
    env_logger::init();
    // Enable inlined constants for animation export - maximum shader performance
    shader_builder_v2::enable_inlined_constants();
    pollster::block_on(export_animation_async(config_path, animation_path, output_path, width, height, fps, iterations_per_thread, video_settings, audio)).expect("Animation export failed");
}

#[cfg(not(target_arch = "wasm32"))]
async fn export_async(input: &str, output: &str, width: Option<u32>, height: Option<u32>, category: Option<String>, iterations_per_thread: Option<u32>, transparent: bool, premultiplied: bool, engine: crate::app::export::ExportEngine, supersample: bool) -> Result<(), Box<dyn std::error::Error>> {
    use std::path::Path;

    println!("Fractal Flame Batch Export");
    println!("===========================");
    println!("Input: {}", input);
    println!("Output: {}", output);
    println!();

    // Find all .fflame and .flame files
    let input_path = Path::new(input);
    let flame_files = if input_path.is_dir() {
        scene::assets::load_configs_from_dir(input_path)
    } else if input_path.extension().and_then(|s| s.to_str()) == Some("flame") {
        // Flame XML format (Apophysis / JWildfire / Chaotica) — a .flame
        // can contain multiple <flame> elements.
        let xml = std::fs::read_to_string(input_path)?;
        flame_xml::parse_flame_xml(&xml)?
    } else {
        vec![config::FractalConfig::load_from_file(input_path)?]
    };

    if flame_files.is_empty() {
        eprintln!("No .fflame files found");
        return Ok(());
    }

    println!("Found {} config(s)\n", flame_files.len());

    // Create output directory if needed
    let output_path = Path::new(output);
    if flame_files.len() > 1 {
        std::fs::create_dir_all(output_path)?;
    }

    // Export each config
    for (i, config) in flame_files.iter().enumerate() {
        let flame_name = &config.flame.name;
        println!("[{}/{}] Exporting {}...", i + 1, flame_files.len(), flame_name);

        // Determine output file path
        let output_file = if flame_files.len() == 1 && output_path.extension().is_some() {
            output_path.to_path_buf()
        } else {
            output_path.join(format!("{}.png", flame_name.to_lowercase().replace(" ", "_")))
        };

        // Use config dimensions or provided dimensions
        let (w, h) = (
            width.unwrap_or(1920),
            height.unwrap_or(1080),
        );

        // Use CLI override or default (device-specific setting not in FractalConfig)
        let ipt = iterations_per_thread.unwrap_or(crate::config::defaults::DEFAULT_ITERATIONS_PER_THREAD);

        // Call the existing PNG export logic from app
        // We'll need to add a headless export helper
        let success = app::export_headless(config, &output_file, w, h, category.clone(), ipt, transparent, premultiplied, engine, supersample).await?;

        if success {
            println!("  ✓ Saved to {}", output_file.display());
        }
    }

    println!("\nExport complete!");
    Ok(())
}

#[cfg(not(target_arch = "wasm32"))]
async fn export_animation_async(
    config_path: &str,
    animation_path: &str,
    output_path: &str,
    width: u32,
    height: u32,
    fps: u32,
    iterations_per_thread: u32,
    video_settings: animation::export::VideoEncodingSettings,
    audio: Option<animation::export::AudioExportConfig>,
) -> Result<(), Box<dyn std::error::Error>> {
    use std::path::Path;
    use animation::export::{AnimationExportConfig, export_animation, is_ffmpeg_available};
    use export::ConsoleReporter;
    use animation::Animation;

    println!("Fractal Flame Animation Export");
    println!("===============================");
    println!("Config: {}", config_path);
    println!("Animation: {}", animation_path);
    println!("Output: {}", output_path);
    println!("Resolution: {}x{} @ {} FPS", width, height, fps);
    println!("Iterations per thread: {}", iterations_per_thread);
    println!("Codec: {} (CRF {})", video_settings.codec.display_name(), video_settings.quality);
    if let Some(ref audio_config) = audio {
        println!("Audio: {}", audio_config.file.display());
        if audio_config.offset != 0.0 {
            println!("  Offset: {:.1}s", audio_config.offset);
        }
        if audio_config.fade_in > 0.0 {
            println!("  Fade in: {:.1}s", audio_config.fade_in);
        }
        if audio_config.fade_out > 0.0 {
            println!("  Fade out: {:.1}s", audio_config.fade_out);
        }
    }
    println!();

    // Check ffmpeg availability
    if !is_ffmpeg_available() {
        eprintln!("Error: FFmpeg not found.");
        eprintln!("Install FFmpeg and ensure it's in your PATH to export animations.");
        return Err("FFmpeg not found".into());
    }

    // Load config file
    let config = config::FractalConfig::load_from_file(Path::new(config_path))?;
    println!("Loaded config: {}", config.flame.name);

    // Load animation file
    let animation_contents = std::fs::read_to_string(animation_path)?;
    let anim = Animation::from_json(&animation_contents)?;
    println!("Loaded animation: {} ({:.1}s duration)", anim.name, anim.duration);

    let total_frames = (anim.duration * fps as f64).ceil() as u32;
    println!("Total frames: {}", total_frames);
    println!();

    // Create export config
    let export_config = AnimationExportConfig {
        config,
        animation: anim,
        output_path: Path::new(output_path).to_path_buf(),
        width,
        height,
        fps,
        iterations_per_thread,
        video_settings,
        audio,
        signals: std::collections::HashMap::new(), // CLI: no signal data (TODO: load from signal files)
    };

    // Run export (pipes directly to FFmpeg)
    let mut reporter = ConsoleReporter;
    let result = export_animation(export_config, &mut reporter).await?;
    println!();

    println!();
    println!("Animation export complete!");
    println!("  Total frames: {}", result.total_frames);
    println!("  Total time: {:.1}s", result.total_time_ms / 1000.0);
    println!("  Average per frame: {:.1}s", result.avg_frame_time_ms / 1000.0);
    println!("  Output: {}", result.output_path.display());

    Ok(())
}

/// Parse URL query parameters for deep-linking (?flame=uuid or ?animation=uuid).
/// Returns (flame_id, animation_id).
#[cfg(target_arch = "wasm32")]
fn parse_url_load_params() -> (Option<String>, Option<String>) {
    let search = match web_sys::window()
        .and_then(|w| w.location().search().ok())
    {
        Some(s) if !s.is_empty() => s,
        _ => return (None, None),
    };

    // Strip leading '?'
    let query = search.trim_start_matches('?');
    let mut flame_id = None;
    let mut animation_id = None;

    for pair in query.split('&') {
        if let Some((key, value)) = pair.split_once('=') {
            match key {
                "flame" if !value.is_empty() => flame_id = Some(value.to_string()),
                "animation" if !value.is_empty() => animation_id = Some(value.to_string()),
                _ => {}
            }
        }
    }

    (flame_id, animation_id)
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = winit::event_loop::EventLoop::new()?;

    #[cfg(target_arch = "wasm32")]
    {
        use winit::platform::web::WindowAttributesExtWebSys;

        let window = {
            let web_window = web_sys::window().unwrap();
            let document = web_window.document().unwrap();
            let canvas = document
                .get_element_by_id("canvas")
                .unwrap()
                .dyn_into::<web_sys::HtmlCanvasElement>()
                .unwrap();

            // Prevent browser from intercepting touch gestures (pinch-zoom, scroll)
            // so they reach winit as raw touch events for multi-touch handling
            canvas.style().set_property("touch-action", "none").unwrap();

            // Release implicit pointer capture on touch so multi-touch events
            // reach the canvas (winit 0.30 doesn't do this; fixed in 0.31+)
            let canvas_for_touch = canvas.clone();
            let touch_fix = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::PointerEvent)>::new(
                move |event: web_sys::PointerEvent| {
                    if event.pointer_type() == "touch" {
                        let _ = canvas_for_touch.release_pointer_capture(event.pointer_id());
                    }
                },
            );
            canvas
                .add_event_listener_with_callback(
                    "pointerdown",
                    touch_fix.as_ref().unchecked_ref(),
                )
                .unwrap();
            touch_fix.forget(); // Leak closure so it lives for the app lifetime

            // When pointer capture is released (above), pointerup/pointercancel events
            // won't fire on the canvas if the finger lifts outside it. This leaves egui's
            // interaction state stuck (thinks drag is still in progress, blocks all UI).
            // Fix: listen on document and re-dispatch to the canvas so winit/egui see the End.
            let canvas_for_up = canvas.clone();
            let touch_up_fix = wasm_bindgen::closure::Closure::<dyn FnMut(web_sys::PointerEvent)>::new(
                move |event: web_sys::PointerEvent| {
                    if event.pointer_type() == "touch" {
                        // Only re-dispatch if the event didn't originate on the canvas
                        if event.target().as_ref() != Some(canvas_for_up.as_ref()) {
                            // web-sys 0.3.98 deprecated chained setters on
                            // PointerEventInit; the new API uses `set_*`
                            // taking `&Self`. clientX/clientY become f64
                            // under web_sys_unstable_apis, so cast to i32
                            // unconditionally — no-op when already i32.
                            let init = web_sys::PointerEventInit::new();
                            init.set_pointer_id(event.pointer_id());
                            init.set_pointer_type(&event.pointer_type());
                            init.set_client_x(event.client_x() as i32);
                            init.set_client_y(event.client_y() as i32);
                            let new_event = web_sys::PointerEvent::new_with_event_init_dict(
                                event.type_().as_str(),
                                &init,
                            ).unwrap();
                            let _ = canvas_for_up.dispatch_event(&new_event);
                        }
                    }
                },
            );
            document
                .add_event_listener_with_callback("pointerup", touch_up_fix.as_ref().unchecked_ref())
                .unwrap();
            document
                .add_event_listener_with_callback("pointercancel", touch_up_fix.as_ref().unchecked_ref())
                .unwrap();
            touch_up_fix.forget();

            let attributes = winit::window::Window::default_attributes()
                .with_title("Fractal Art Editor")
                .with_canvas(Some(canvas));

            #[allow(deprecated)]
            let window = event_loop.create_window(attributes)?;

            // Set canvas size to match display size with device pixel ratio.
            // Cap effective DPR at 1.5 — at full DPR (2.0 on retina iPad, 3.0
            // on iPhone Pro), the per-pixel GPU buffers exceed iOS Safari's
            // ~512 MB-on-iPad / ~256 MB-on-iPhone per-tab memory limit and
            // the tab silently crashes. Cost: slight softening on retina
            // displays. See docs/projects/wasm-dpr-cap.md if we want to
            // revisit (e.g., a user preference toggle for high-DPR mode).
            //
            // Memory budget math at full DPR on iPad retina (2048×1536, DPR
            // 2.0 → 4096×3072 physical, 12.6M pixels):
            //   histogram_buffer = 12.6M × 16 bytes  = 192 MB
            //   iteration_count  = 12.6M × 4 bytes   =  48 MB
            //   accum textures   = 12.6M × 8 × 2     = 192 MB
            //   total > 432 MB just for these → tab gets killed.
            // At DPR 1.5 the same iPad lands ~7M pixels, ~270 MB total.
            let raw_dpr = web_window.device_pixel_ratio();
            let dpr = raw_dpr.min(1.5);
            let width = web_window.inner_width().unwrap().as_f64().unwrap();
            let height = web_window.inner_height().unwrap().as_f64().unwrap();

            // Physical pixels = CSS pixels × (capped) device pixel ratio
            let physical_width = (width * dpr) as u32;
            let physical_height = (height * dpr) as u32;

            let _ = window.request_inner_size(PhysicalSize::new(physical_width, physical_height));

            // Override winit's inline canvas style only when the DPR cap
            // actually kicked in (raw_dpr > 1.5). On Windows with DPR=1.0
            // the override is a no-op visually but tripped a reflow loop
            // in Firefox; gating on the cap keeps it macOS-retina-only
            // where it's needed. See resize listener comment below for the
            // full explanation.
            if raw_dpr > 1.5 {
                let canvas_for_style = document
                    .get_element_by_id("canvas")
                    .unwrap()
                    .dyn_into::<web_sys::HtmlCanvasElement>()
                    .unwrap();
                let _ = canvas_for_style.style().set_property("width", "100%");
                let _ = canvas_for_style.style().set_property("height", "100%");
            }

            // DEBUG: Log all size-related information
            let actual_inner_size = window.inner_size();
            let canvas_element = document.get_element_by_id("canvas").unwrap();
            let canvas = canvas_element.dyn_into::<web_sys::HtmlCanvasElement>().unwrap();

            log::info!("=== WASM Canvas Size Debug ===");
            log::info!("  Browser window (CSS): {}x{}", width as u32, height as u32);
            log::info!("  Device Pixel Ratio: {}", dpr);
            log::info!("  Calculated physical: {}x{}", physical_width, physical_height);
            log::info!("  Window inner_size(): {}x{}", actual_inner_size.width, actual_inner_size.height);
            log::info!("  Canvas element width: {}", canvas.width());
            log::info!("  Canvas element height: {}", canvas.height());
            log::info!("  Canvas clientWidth: {}", canvas.client_width());
            log::info!("  Canvas clientHeight: {}", canvas.client_height());
            log::info!("  Canvas offsetWidth: {}", canvas.offset_width());
            log::info!("  Canvas offsetHeight: {}", canvas.offset_height());
            log::info!("===============================");

            window
        };

        // Wrap the window in Arc up here so we can clone it into the browser
        // resize listener installed below. The Arc gets passed through to
        // App::run unchanged.
        let window = std::sync::Arc::new(window);

        // Browser resize listener.
        //
        // winit 0.30's web backend observes the canvas with `ResizeObserver`
        // configured for `DevicePixelContentBox`. For HTMLCanvasElement, that
        // box is keyed to the `canvas.width × canvas.height` *attributes*, not
        // the CSS layout box. So when the browser window resizes and only the
        // canvas's CSS dimensions change (via `width: 100%` etc.), winit's
        // observer never fires and no `WindowEvent::Resized` is dispatched —
        // the app stays at startup size forever.
        //
        // Fix: bypass winit's observer entirely. Listen to `window.resize`
        // ourselves, compute the desired physical size (`CSS × min(DPR, 1.5)`
        // — DPR cap for iOS Safari memory budget), and call
        // `winit::Window::request_inner_size`. That sets canvas.width and
        // canvas.height to the new values, which DOES fire winit's observer
        // (the attributes changed), which dispatches `WindowEvent::Resized`
        // with the new size, which flows through `app.gpu.resize` normally.
        // Single source of truth: this listener.
        {
            use std::cell::Cell;
            use std::rc::Rc;
            use wasm_bindgen::JsCast;
            use wasm_bindgen::closure::Closure;

            let window_for_resize = std::sync::Arc::clone(&window);

            // Debounce: collapse rapid drag-resize events into a single
            // request_inner_size call after the user has stopped moving.
            // Without this, each browser 'resize' event triggers a wgpu
            // surface reconfigure and the pile-up freezes the renderer.
            // 100ms matches the original JS handler's behavior.
            let pending_timeout: Rc<Cell<Option<i32>>> = Rc::new(Cell::new(None));

            // The closure that fires after the debounce delay.
            let pending_timeout_for_fire = pending_timeout.clone();
            let fire_closure = Closure::<dyn FnMut()>::new(move || {
                pending_timeout_for_fire.set(None);
                let web_window = match web_sys::window() {
                    Some(w) => w,
                    None => return,
                };

                // Prefer visualViewport over window.innerWidth/Height.
                // On iOS Safari, window.innerWidth can balloon to the
                // layout viewport (10000+ during pinch-zoom and address-bar
                // transitions); visualViewport reports what's actually
                // visible. Falls back to innerWidth/Height on browsers
                // that don't expose visualViewport.
                let (css_width, css_height) = {
                    let vv = web_window.visual_viewport();
                    if let Some(vv) = vv {
                        (vv.width(), vv.height())
                    } else {
                        let w = web_window
                            .inner_width()
                            .ok()
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);
                        let h = web_window
                            .inner_height()
                            .ok()
                            .and_then(|v| v.as_f64())
                            .unwrap_or(0.0);
                        (w, h)
                    }
                };
                if css_width <= 0.0 || css_height <= 0.0 {
                    return;
                }
                let raw_dpr = web_window.device_pixel_ratio();
                let dpr = raw_dpr.min(1.5);

                // Hard cap on physical dimensions. Defense against iOS
                // Safari reporting absurd visualViewport values during
                // unusual UI states (rare but observed). At 4096×4096
                // the histogram is ~256 MB, the upper bound we'll
                // tolerate even on a misbehaving browser.
                const MAX_PHYSICAL_DIM: u32 = 4096;
                let uncapped_width = (css_width * dpr) as u32;
                let uncapped_height = (css_height * dpr) as u32;
                let physical_width = uncapped_width.min(MAX_PHYSICAL_DIM);
                let physical_height = uncapped_height.min(MAX_PHYSICAL_DIM);
                log::info!(
                    "WASM resize: css={}x{} dpr_raw={} dpr_used={} requested={}x{} (uncapped would be {}x{})",
                    css_width, css_height, raw_dpr, dpr,
                    physical_width, physical_height,
                    uncapped_width, uncapped_height,
                );
                let _ = window_for_resize.request_inner_size(
                    PhysicalSize::new(physical_width, physical_height),
                );

                // winit sets `canvas.style.width = ${physical / raw_dpr}px`
                // inline when request_inner_size runs. When our DPR cap is
                // below the OS DPR (e.g., 1.5 < 2.0 on macOS retina) the
                // inline style becomes ${CSS × 0.75}px and overrides our
                // CSS `width: 100%` rule, shrinking the visible canvas.
                // Restore CSS sizing in that case — drawing buffer stays
                // at the capped resolution, browser upscales at composite.
                //
                // Skip when no cap was applied (raw_dpr <= 1.5). On
                // Windows DPR=1.0, setting style.width "100%" on a canvas
                // that didn't need it has been observed to trip a reflow
                // loop in Firefox that froze the renderer.
                if raw_dpr > 1.5 {
                    if let Some(doc) = web_window.document() {
                        if let Some(canvas_el) = doc.get_element_by_id("canvas") {
                            if let Ok(html_canvas) =
                                canvas_el.dyn_into::<web_sys::HtmlCanvasElement>()
                            {
                                let _ = html_canvas
                                    .style()
                                    .set_property("width", "100%");
                                let _ = html_canvas
                                    .style()
                                    .set_property("height", "100%");
                            }
                        }
                    }
                }
            });

            // The resize handler: schedule/reschedule the fire closure.
            let pending_timeout_for_schedule = pending_timeout.clone();
            let fire_closure_ref = fire_closure.as_ref().clone();
            let resize_closure = Closure::<dyn FnMut()>::new(move || {
                let web_window = match web_sys::window() {
                    Some(w) => w,
                    None => return,
                };
                // Cancel any pending fire scheduled by a previous event.
                if let Some(id) = pending_timeout_for_schedule.take() {
                    web_window.clear_timeout_with_handle(id);
                }
                // Schedule the fire 100ms from now — coalesces drag storms.
                if let Ok(id) = web_window.set_timeout_with_callback_and_timeout_and_arguments_0(
                    fire_closure_ref.unchecked_ref(),
                    100,
                ) {
                    pending_timeout_for_schedule.set(Some(id));
                }
            });

            let web_window = web_sys::window().expect("no global window");
            web_window
                .add_event_listener_with_callback(
                    "resize",
                    resize_closure.as_ref().unchecked_ref(),
                )
                .expect("Failed to register resize listener");

            // Keep both closures alive for the lifetime of the page.
            fire_closure.forget();
            resize_closure.forget();
        }

        // Parse URL query params for deep-linking (?flame=uuid or ?animation=uuid)
        let (url_flame_id, url_animation_id) = parse_url_load_params();
        if url_flame_id.is_some() || url_animation_id.is_some() {
            log::info!("URL params: flame={:?}, animation={:?}", url_flame_id, url_animation_id);
        }

        App::run(event_loop, window, url_flame_id, url_animation_id).await
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        let attributes = winit::window::Window::default_attributes()
            .with_title("Fractal Art Editor")
            .with_inner_size(PhysicalSize::new(1920, 1080))
            .with_min_inner_size(PhysicalSize::new(300, 200));

        #[allow(deprecated)]
        let window = event_loop.create_window(attributes)?;

        App::run(event_loop, std::sync::Arc::new(window)).await
    }
}
