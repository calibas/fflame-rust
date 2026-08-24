//! fflame-render — the Endless Gallery's renderer.
//!
//! One call: a `FractalConfig` JSON string (exactly what the script
//! module's `config_json` field carries, or any `.fflame` file) plus
//! dimensions and an iteration budget, out come raw RGBA pixels for
//! `canvas.putImageData`. Internally this is the main crate's unified
//! headless render path, unchanged.
//!
//! Device lifecycle: **one device and one renderer, reused across
//! tiles.** This was per-render once, destroying the device each time,
//! and that is what a real gallery scroll exhausted — reported as
//! `requestDevice` itself failing with "Not enough memory left".
//!
//! The reason is specific to WebGPU: wgpu implements `Drop` for
//! `WebBuffer` and `WebTexture` as a **no-op**, so a finished renderer's
//! GPU memory survives until the JS garbage collector collects the
//! wrappers. `device.destroy()` is the only synchronous reclamation, and
//! it can only sweep a whole device — so a device per tile made
//! reclamation a race against the scroll, and the scroll won.
//!
//! Reuse removes the garbage rather than sweeping it faster.
//! `load_config` is a full reset point (transforms, variation params,
//! palette size, accumulation), so a new config needs no new renderer;
//! only a size change does, and that branch destroys the old one
//! explicitly. The shader cache surviving is a real bonus: consecutive
//! seeds of one generator usually share a variation set, so the compile
//! amortizes across the hallway.
//!
//! The same rule extends below the renderer: the per-render readback
//! staging buffer is persistent in `FlameRenderer` (grow-only,
//! re-mapped each read), and the effect chain in `render_with` is only
//! built when an effect is enabled and is destroyed after the pixel
//! read. Steady-state, a render creates **zero** GPU buffers — the
//! client measured 2,137 `createBuffer` calls and zero `destroy()`
//! across 1,000 renders before that, growing without bound because the
//! JS GC feels no GPU-memory pressure.
//!
//! Renders are serial. A second call while one is in flight gets an
//! error rather than sharing the renderer — see `render_impl`. `release()`
//! frees everything for a page that is done.

use egui_wgpu::wgpu;
use fractal_flame_wgpu::config::FractalConfig;
use fractal_flame_wgpu::renderer::compute_kernel::FlameRenderer;
use fractal_flame_wgpu::renderer::render::{render_with, NoProgress, RenderJob};
use std::cell::RefCell;

/// The device, queue and renderer, kept alive across tiles.
///
/// Single-threaded by construction: wasm has one thread, and the render
/// entry point is `async` but not re-entrant — a second call while one is
/// in flight would find the `RefCell` already borrowed and error rather
/// than corrupt anything. The gallery renders in series.
struct Live {
    device: wgpu::Device,
    queue: wgpu::Queue,
    max_dim: u32,
    /// `None` until the first render; carries the size it was built for,
    /// because a dimension change needs `resize`, not just `load_config`.
    renderer: Option<(FlameRenderer, u32, u32)>,
}

/// Wrapper whose `Drop` deliberately **leaks** the device.
///
/// Dropping a wgpu `Device` reaches into other thread-locals. At thread
/// exit those may already have been destroyed, and the access panics —
/// inside a destructor, which Rust turns into an abort
/// (`STATUS_STACK_BUFFER_OVERRUN`). The native smoke test hit exactly
/// that the moment the device outlived a single call.
///
/// Leaking is the right answer rather than a dodge: this only runs when
/// the thread is ending, which on wasm means the page is being torn down
/// and the whole linear memory plus every GPU resource goes with it.
/// Callers wanting deterministic cleanup have `release()`, which runs
/// while the thread is alive and can drop safely.
struct LiveCell(RefCell<Option<Live>>);

impl Drop for LiveCell {
    fn drop(&mut self) {
        if let Ok(mut slot) = self.0.try_borrow_mut() {
            std::mem::forget(slot.take());
        }
    }
}

thread_local! {
    static LIVE: LiveCell = const { LiveCell(RefCell::new(None)) };
}

pub struct RenderedTile {
    pub pixels: Vec<u8>,
    pub width: u32,
    pub height: u32,
    pub iterations: u64,
    pub ms: f64,
}

/// Returns the device, its queue, and the adapter's maximum 2D texture
/// dimension — the ceiling a render must respect.
async fn create_device() -> Result<(wgpu::Device, wgpu::Queue, u32), String> {
    let backends = if cfg!(target_arch = "wasm32") {
        wgpu::Backends::BROWSER_WEBGPU
    } else {
        // Native builds exist for the smoke test only.
        wgpu::Backends::PRIMARY
    };
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });

    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .map_err(|e| format!("no GPU adapter (WebGPU unavailable?): {e:?}"))?;

    // Same limits the app's WASM export requests: downlevel defaults,
    // with the adapter's real storage-buffer capacity (the compute bind
    // group uses 10 storage buffers per stage). Browsers fill the
    // compute limits downlevel_webgl2_defaults zeroes out; native wgpu
    // enforces the zeros literally, so the (test-only) native build
    // starts from the real defaults instead.
    let adapter_limits = adapter.limits();
    let mut limits = if cfg!(target_arch = "wasm32") {
        wgpu::Limits::downlevel_webgl2_defaults()
    } else {
        wgpu::Limits::default()
    };
    limits.max_storage_buffer_binding_size = adapter_limits.max_storage_buffer_binding_size;
    limits.max_storage_buffers_per_shader_stage =
        adapter_limits.max_storage_buffers_per_shader_stage;

    let mut required_features = wgpu::Features::CLEAR_TEXTURE;
    if adapter.features().contains(wgpu::Features::FLOAT32_FILTERABLE) {
        required_features |= wgpu::Features::FLOAT32_FILTERABLE;
    }

    let max_dim = limits.max_texture_dimension_2d;
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("Gallery Renderer Device"),
            required_features,
            required_limits: limits,
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        })
        .await
        .map_err(|e| format!("device request failed: {e:?}"))?;

    // wgpu's default handler PANICS on a validation error. In wasm a
    // panic poisons the module: every later call fails and the page has
    // to reload. This module promises `Result`, so route uncaptured
    // errors to the log and let the render return its own error
    // instead. (Dimensions are checked up front, but a config can be
    // hostile in ways no up-front check enumerates.)
    device.on_uncaptured_error(std::sync::Arc::new(|e| {
        log::error!("wgpu error (render will fail): {e}");
    }));

    Ok((device, queue, max_dim))
}

/// Probe for a usable adapter without rendering — lets a page fail
/// early with a clear message instead of on the first tile.
pub async fn probe_impl() -> Result<(), String> {
    let (device, _queue, _max_dim) = create_device().await?;
    device.destroy();
    Ok(())
}

/// Upper bound on a single render's chaos-game budget.
///
/// A config carries its own `max_iterations`, and a config is a
/// shareable artifact — one asking for 5e11 ground for 90 seconds and
/// was still going when killed, which in a browser is a frozen tab or a
/// GPU reset. 8e9 is far above any sensible tile (the gallery uses 3e7)
/// while keeping a hostile file to seconds rather than forever.
const MAX_RENDER_ITERATIONS: u64 = 8_000_000_000;

/// Iterations-per-thread bounds. ipt is trajectory depth — one dispatch
/// runs the whole trajectory, so an unbounded value turns a single
/// dispatch into a browser-watchdog-length stall the same way an
/// unbounded iteration budget would. Mirrors the CLI's documented range.
const MIN_ITERATIONS_PER_THREAD: u32 = 64;
const MAX_ITERATIONS_PER_THREAD: u32 = 4096;

pub async fn render_impl(
    config_json: &str,
    width: u32,
    height: u32,
    target_iterations: Option<u64>,
    iterations_per_thread: Option<u32>,
) -> Result<RenderedTile, String> {
    if width == 0 || height == 0 {
        return Err("width and height must be nonzero".into());
    }
    let mut config = FractalConfig::from_json(config_json)
        .map_err(|e| format!("config did not parse: {e}"))?;

    // Create the device once and keep it. Creating one per tile is what
    // exhausted the GPU: wgpu's WebGPU backend implements `Drop` for
    // buffers and textures as a NO-OP, so a finished renderer's memory
    // lives until the JS garbage collector runs, and `device.destroy()`
    // — the only synchronous reclamation — can just sweep a whole
    // device. A scroll of tiles created devices faster than the browser
    // reclaimed them, and eventually `requestDevice` itself failed with
    // "Not enough memory left".
    //
    // Reuse removes the garbage instead of sweeping it faster, and keeps
    // the shader cache warm: consecutive seeds of one generator usually
    // share a variation set, so the per-tile compile amortizes.
    if LIVE.with(|l| l.0.borrow().is_none()) {
        let (device, queue, max_dim) = create_device().await?;
        LIVE.with(|l| {
            *l.0.borrow_mut() = Some(Live { device, queue, max_dim, renderer: None });
        });
    }

    config.max_iterations = config.max_iterations.min(MAX_RENDER_ITERATIONS);
    let iters = target_iterations.map(|i| i.min(MAX_RENDER_ITERATIONS));

    // Take the whole `Live` out for the duration, put it back on every
    // path. That is also the re-entrancy guard — a second concurrent
    // call finds the slot empty and gets a clear error instead of two
    // renders sharing one renderer. The borrow is released before the
    // `await`, because holding a `RefCell` borrow across a yield point
    // is how a single-threaded async runtime deadlocks itself.
    let mut live = LIVE
        .with(|l| l.0.borrow_mut().take())
        .ok_or_else(|| {
            "a render is already in flight — this module renders one tile at              a time; await the previous call"
                .to_string()
        })?;

    let ipt = iterations_per_thread
        .map(|v| v.clamp(MIN_ITERATIONS_PER_THREAD, MAX_ITERATIONS_PER_THREAD));

    let outcome = render_with_live(&mut live, &config, width, height, iters, ipt).await;

    LIVE.with(|l| *l.0.borrow_mut() = Some(live));
    outcome
}

async fn render_with_live(
    live: &mut Live,
    config: &FractalConfig,
    width: u32,
    height: u32,
    target_iterations: Option<u64>,
    iterations_per_thread: Option<u32>,
) -> Result<RenderedTile, String> {
    // Build the renderer on first use, or resize it if the tile geometry
    // changed. `load_config` is a full reset for everything else —
    // transforms, variation params, palette size, accumulation — so the
    // renderer does not need to have been built for THIS config, only
    // for this size.
    if width > live.max_dim || height > live.max_dim {
        return Err(format!(
            "{width}x{height} exceeds this device's maximum texture dimension of {}",
            live.max_dim
        ));
    }

    let needs_new = match &live.renderer {
        None => true,
        Some((_, w, h)) => *w != width || *h != height,
    };
    if needs_new {
        if let Some((old, _, _)) = live.renderer.take() {
            // Explicit, because dropping frees nothing on WebGPU.
            old.destroy();
        }
        let renderer = FlameRenderer::with_palette_size(
            &live.device,
            &live.queue,
            wgpu::TextureFormat::Rgba8Unorm,
            width,
            height,
            &config.flame,
            config.palette_size,
        );
        live.renderer = Some((renderer, width, height));
    }

    let (renderer, _, _) = live.renderer.as_mut().expect("built above");

    let mut job = RenderJob::new(config, width, height);
    if let Some(iters) = target_iterations {
        job = job.with_iterations(iters);
    }
    if let Some(ipt) = iterations_per_thread {
        job = job.with_iterations_per_thread(ipt);
    }

    let out = render_with(renderer, &live.device, &live.queue, job, &mut NoProgress)
        .await
        .map_err(|e| e.to_string())?;

    Ok(RenderedTile {
        pixels: out.rgba_data,
        width: out.width,
        height: out.height,
        iterations: out.total_iterations,
        ms: out.render_time_ms,
    })
}

/// Release the device and everything on it.
///
/// Optional — the module works without it — but a page that is done
/// rendering (navigating away, closing the gallery) can free the GPU
/// memory immediately rather than waiting for the JS garbage collector
/// to notice the wrappers.
pub fn release_impl() {
    LIVE.with(|l| {
        if let Some(live) = l.0.borrow_mut().take() {
            if let Some((renderer, _, _)) = live.renderer {
                renderer.destroy();
            }
            live.device.destroy();
        }
    });
}

#[cfg(target_arch = "wasm32")]
mod wasm {
    use wasm_bindgen::prelude::*;

    #[wasm_bindgen(start)]
    pub fn start() {
        console_error_panic_hook::set_once();
    }

    /// A finished tile. `pixels` is RGBA8, `width * height * 4` bytes —
    /// feed it to `new ImageData(new Uint8ClampedArray(...), w, h)`.
    #[wasm_bindgen]
    pub struct RenderResult {
        inner: crate::RenderedTile,
    }

    #[wasm_bindgen]
    impl RenderResult {
        #[wasm_bindgen(getter)]
        pub fn pixels(&self) -> js_sys::Uint8Array {
            js_sys::Uint8Array::from(&self.inner.pixels[..])
        }
        #[wasm_bindgen(getter)]
        pub fn width(&self) -> u32 {
            self.inner.width
        }
        #[wasm_bindgen(getter)]
        pub fn height(&self) -> u32 {
            self.inner.height
        }
        #[wasm_bindgen(getter)]
        pub fn iterations(&self) -> f64 {
            self.inner.iterations as f64
        }
        #[wasm_bindgen(getter)]
        pub fn ms(&self) -> f64 {
            self.inner.ms
        }
    }

    /// Fail early if WebGPU is unavailable, with a clear message.
    #[wasm_bindgen]
    pub async fn probe() -> Result<(), JsValue> {
        crate::probe_impl().await.map_err(|e| JsValue::from_str(&e))
    }

    /// Render a config. `iterations` caps the chaos-game budget
    /// (defaults to the config's own `max_iterations`).
    /// `iterations_per_thread` is trajectory depth per dispatch
    /// (default 256, clamped to 64–4096). Depth-vs-breadth trade-off,
    /// not a quality dial: deep orbits reach late-emerging structure
    /// with less restart overhead, but orbits stuck in a sink or
    /// off-frame waste more budget before a restart resamples. It
    /// changes rendered pixels either way.
    #[wasm_bindgen]
    pub async fn render(
        config_json: &str,
        width: u32,
        height: u32,
        iterations: Option<f64>,
        iterations_per_thread: Option<u32>,
    ) -> Result<RenderResult, JsValue> {
        let tile = crate::render_impl(
            config_json,
            width,
            height,
            iterations.map(|i| i as u64),
            iterations_per_thread,
        )
        .await
        .map_err(|e| JsValue::from_str(&e))?;
        Ok(RenderResult { inner: tile })
    }

    /// Release the GPU device and everything on it.
    ///
    /// Optional. The module holds one device across tiles, which is what
    /// keeps GPU memory flat during a long scroll; call this when the
    /// page is done rendering (navigating away, closing the gallery) to
    /// free it immediately instead of waiting for the JS garbage
    /// collector to notice. The next `render` transparently builds a new
    /// device, so calling it early costs a device request, not a bug.
    #[wasm_bindgen]
    pub fn release() {
        crate::release_impl();
    }
}
