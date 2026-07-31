//! fflame-render — the Endless Gallery's renderer.
//!
//! One call: a `FractalConfig` JSON string (exactly what the script
//! module's `config_json` field carries, or any `.fflame` file) plus
//! dimensions and an iteration budget, out come raw RGBA pixels for
//! `canvas.putImageData`. Internally this is the main crate's unified
//! headless render path, unchanged.
//!
//! Device lifecycle: a device is created per render and destroyed
//! afterwards — the pattern the app's own WASM export uses, because on
//! WebGPU dropping Rust buffer handles only defers reclamation to the
//! JS garbage collector; a long scroll of tiles would otherwise
//! accumulate GPU memory until renders start failing black.
//! `device.destroy()` frees synchronously. The cost (adapter + device
//! request, one shader compile per tile) is milliseconds against a
//! multi-hundred-ms render. The full version's optimization door:
//! a persistent FlameRenderer + shader cache, with explicit buffer
//! destruction — consecutive seeds of one generator usually share a
//! variation set, so the compile would amortize across the hallway.

use egui_wgpu::wgpu;
use fractal_flame_wgpu::config::FractalConfig;
use fractal_flame_wgpu::renderer::render::{render as unified_render, NoProgress, RenderJob};

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

pub async fn render_impl(
    config_json: &str,
    width: u32,
    height: u32,
    target_iterations: Option<u64>,
) -> Result<RenderedTile, String> {
    if width == 0 || height == 0 {
        return Err("width and height must be nonzero".into());
    }
    let mut config = FractalConfig::from_json(config_json)
        .map_err(|e| format!("config did not parse: {e}"))?;

    let (device, queue, max_dim) = create_device().await?;

    // Check dimensions against what the adapter actually allows, BEFORE
    // any texture is created. Unchecked, an oversized request reached
    // wgpu's validation and panicked (`Dimension X value 8193 exceeds
    // the limit of 8192`) instead of returning this error.
    if width > max_dim || height > max_dim {
        device.destroy();
        return Err(format!(
            "{width}x{height} exceeds this device's maximum texture dimension of {max_dim}"
        ));
    }

    // Clamp the chaos-game budget from BOTH sources: the caller's
    // argument and the config's own `max_iterations`, either of which
    // can be hostile.
    config.max_iterations = config.max_iterations.min(MAX_RENDER_ITERATIONS);
    let mut job = RenderJob::new(&config, width, height);
    if let Some(iters) = target_iterations {
        job = job.with_iterations(iters.min(MAX_RENDER_ITERATIONS));
    }
    let result = unified_render(&device, &queue, job, &mut NoProgress).await;

    // Free the device's memory synchronously — see the module docs.
    // The pixels are already on the CPU; runs on both paths.
    device.destroy();

    let out = result.map_err(|e| e.to_string())?;
    Ok(RenderedTile {
        pixels: out.rgba_data,
        width: out.width,
        height: out.height,
        iterations: out.total_iterations,
        ms: out.render_time_ms,
    })
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
    #[wasm_bindgen]
    pub async fn render(
        config_json: &str,
        width: u32,
        height: u32,
        iterations: Option<f64>,
    ) -> Result<RenderResult, JsValue> {
        let tile = crate::render_impl(
            config_json,
            width,
            height,
            iterations.map(|i| i as u64),
        )
        .await
        .map_err(|e| JsValue::from_str(&e))?;
        Ok(RenderResult { inner: tile })
    }
}
