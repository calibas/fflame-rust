//! SPIKE (branch python-wasm-spike): the engine as a WASI module for
//! Python. wasmtime runs it; its GPU calls go to the host through a wgpu
//! custom backend (`backend`), and the host makes them with wgpu-py.
//!
//! The exports are a C ABI over linear memory: the host allocates with
//! `fflame_alloc`, writes a config, calls `fflame_render`, and reads the
//! output (`fflame_output_*`: pixels on success, the error's text on
//! failure) and a JSON note about the render (`fflame_info_*`).

mod backend;
mod host;

use std::cell::RefCell;
use std::future::Future;
use std::task::{Context, Poll, Waker};

use fractal_flame_wgpu::config::FractalConfig;
use fractal_flame_wgpu::renderer::compute_kernel::FlameRenderer;
use fractal_flame_wgpu::renderer::render::{render_with, NoProgress, RenderJob};

struct Live {
    device: wgpu::Device,
    queue: wgpu::Queue,
    renderer: Option<(FlameRenderer, u32, u32)>,
}

thread_local! {
    static LIVE: RefCell<Option<Live>> = const { RefCell::new(None) };
    static OUTPUT: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
    static INFO: RefCell<Vec<u8>> = const { RefCell::new(Vec::new()) };
}

/// Runs a future to completion. Everything the engine awaits resolves
/// once the backend's queued maps and callbacks run, so a pending poll
/// runs them and tries again.
fn block_on<F: Future>(f: F) -> F::Output {
    let mut f = std::pin::pin!(f);
    let mut cx = Context::from_waker(Waker::noop());
    for _ in 0..1000 {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }
        backend::flush();
    }
    panic!("a future never completed");
}

#[no_mangle]
pub extern "C" fn fflame_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len.max(1));
    let p = v.as_mut_ptr();
    std::mem::forget(v);
    p
}

/// # Safety
/// `ptr` and `len` from one `fflame_alloc`.
#[no_mangle]
pub unsafe extern "C" fn fflame_free(ptr: *mut u8, len: usize) {
    drop(Vec::from_raw_parts(ptr, 0, len.max(1)));
}

#[no_mangle]
pub extern "C" fn fflame_output_ptr() -> *const u8 {
    OUTPUT.with(|o| o.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn fflame_output_len() -> usize {
    OUTPUT.with(|o| o.borrow().len())
}

#[no_mangle]
pub extern "C" fn fflame_info_ptr() -> *const u8 {
    INFO.with(|o| o.borrow().as_ptr())
}

#[no_mangle]
pub extern "C" fn fflame_info_len() -> usize {
    INFO.with(|o| o.borrow().len())
}

/// Renders a config (JSON, `len` bytes at `ptr`) at `width` x `height`;
/// `iterations` 0 keeps the config's own. Returns 0 on success.
///
/// # Safety
/// `ptr`/`len` a UTF-8 string in this module's memory.
#[no_mangle]
pub unsafe extern "C" fn fflame_render(ptr: *const u8, len: usize, width: u32, height: u32, iterations: u64) -> i32 {
    let json = std::str::from_utf8(std::slice::from_raw_parts(ptr, len)).unwrap_or("");
    match render(json, width, height, iterations) {
        Ok((pixels, info)) => {
            OUTPUT.with(|o| *o.borrow_mut() = pixels);
            INFO.with(|o| *o.borrow_mut() = info.into_bytes());
            0
        }
        Err(e) => {
            OUTPUT.with(|o| *o.borrow_mut() = e.into_bytes());
            INFO.with(|o| o.borrow_mut().clear());
            1
        }
    }
}

fn render(json: &str, width: u32, height: u32, iterations: u64) -> Result<(Vec<u8>, String), String> {
    if width == 0 || height == 0 {
        return Err("width and height must be nonzero".into());
    }
    let config = FractalConfig::from_json(json).map_err(|e| format!("config did not parse: {e}"))?;
    if LIVE.with(|l| l.borrow().is_none()) {
        let (device, queue) = backend::device()?;
        LIVE.with(|l| *l.borrow_mut() = Some(Live { device, queue, renderer: None }));
    }
    let mut live = LIVE.with(|l| l.borrow_mut().take()).ok_or("a render is already running")?;
    let out = render_live(&mut live, &config, width, height, iterations);
    LIVE.with(|l| *l.borrow_mut() = Some(live));
    out
}

fn render_live(
    live: &mut Live,
    config: &FractalConfig,
    width: u32,
    height: u32,
    iterations: u64,
) -> Result<(Vec<u8>, String), String> {
    let needs_new = !matches!(&live.renderer, Some((_, w, h)) if *w == width && *h == height);
    if needs_new {
        if let Some((old, _, _)) = live.renderer.take() {
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
    if iterations > 0 {
        job = job.with_iterations(iterations);
    }
    let out = block_on(render_with(renderer, &live.device, &live.queue, job, &mut NoProgress)).map_err(|e| e.to_string())?;
    let info = serde_json::json!({
        "width": out.width, "height": out.height,
        "iterations": out.total_iterations, "ms": out.render_time_ms,
    })
    .to_string();
    Ok((out.rgba_data, info))
}
