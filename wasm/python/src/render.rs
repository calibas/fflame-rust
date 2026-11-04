//! Rendering: stills, sequences a Python program drives frame by frame,
//! and animations the animation engine drives -- the video export's frame
//! loop (`animation::export::export_animation`) without its encoder,
//! which is the Python side's (frames to PNG files, or piped to FFmpeg).
//!
//! COPIED: `Gpu::open`/`Gpu::frame` from that export loop, and the
//! renderer kept per size (`renderer_for`) from wasm/render's
//! `render_with_live`; listed for review in docs/projects/pyfflame.md,
//! "Copied code".
//!
//! Every render is an export's: iterations per thread default to the
//! CLI's, and the sticky shader superset is off unless asked for, so a
//! frame is the specialized shader's picture, as the CLI draws it.

use std::collections::HashMap;
use std::future::Future;
use std::task::{Context, Poll, Waker};

use fractal_flame_wgpu::animation::export::{apply_animation_values, export_signals, moment_configs, tracks_missing_signals};
use fractal_flame_wgpu::animation::{Animation, AnimationController};
use fractal_flame_wgpu::config::defaults::DEFAULT_EXPORT_ITERATIONS_PER_THREAD;
use fractal_flame_wgpu::config::FractalConfig;
use fractal_flame_wgpu::png_metadata::{encode_png_with_metadata, PngMetadata};
use fractal_flame_wgpu::renderer::{render_with, FlameRenderer, NoProgress, RenderEngines, RenderJob, RenderOutput};
use fractal_flame_wgpu::signal::SignalManager;
use serde_json::{json, Value};

use crate::backend;

/// Runs a future to completion. Everything the engine awaits resolves
/// once the backend's queued maps and callbacks run, so a pending poll
/// runs them and tries again.
pub fn block_on<F: Future>(f: F) -> F::Output {
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

/// What a render call may set beyond the config and the size.
#[derive(Default)]
pub struct Options {
    pub iterations: Option<u64>,
    pub iterations_per_thread: Option<u32>,
    pub transparent: bool,
    pub premultiplied: bool,
    /// The sticky shader superset: fewer recompiles across a batch of
    /// flames, at the price of the specialized shader's exact picture.
    pub sticky: bool,
}

impl Options {
    pub fn from_json(v: &Value) -> Self {
        Options {
            iterations: v["iterations"].as_u64().filter(|&n| n > 0),
            iterations_per_thread: v["iterations_per_thread"].as_u64().map(|n| n as u32),
            transparent: v["transparent"].as_bool().unwrap_or(false),
            premultiplied: v["premultiplied"].as_bool().unwrap_or(false),
            sticky: v["sticky"].as_bool().unwrap_or(false),
        }
    }

    fn job<'a>(&self, config: &'a FractalConfig, width: u32, height: u32) -> RenderJob<'a> {
        let mut job = RenderJob::new(config, width, height)
            .with_iterations_per_thread(self.iterations_per_thread.unwrap_or(DEFAULT_EXPORT_ITERATIONS_PER_THREAD))
            .with_transparent(self.transparent)
            .with_premultiplied(self.premultiplied);
        if let Some(n) = self.iterations {
            job = job.with_iterations(n);
        }
        job
    }
}

/// A rendered frame, with what its PNG's metadata records.
pub struct Frame {
    pub out: RenderOutput,
    pub iterations_per_thread: u32,
}

impl Frame {
    pub fn info(&self) -> Value {
        json!({
            "width": self.out.width, "height": self.out.height,
            "iterations": self.out.total_iterations, "ms": self.out.render_time_ms,
            "iterations_per_thread": self.iterations_per_thread,
        })
    }

}

/// The PNG the CLI writes for a frame: the same metadata (the config,
/// the iterations, the build), so the file reloads in the app.
pub fn png(rgba: Vec<u8>, width: u32, height: u32, iterations: u64, ms: f64, iterations_per_thread: u32, config: &FractalConfig) -> Result<Vec<u8>, String> {
    if rgba.len() != width as usize * height as usize * 4 {
        return Err(format!("{} bytes are not {width}x{height} RGBA", rgba.len()));
    }
    let meta = PngMetadata::from_app_state(width, height, iterations, ms, iterations_per_thread, config.speed_factor, config);
    encode_png_with_metadata(width, height, rgba, &meta)
}

/// A renderer kept for its size and engine family (an escape or
/// simulation config needs only the tail).
struct Target {
    renderer: FlameRenderer,
    width: u32,
    height: u32,
    tail: bool,
}

fn renderer_for<'a>(
    slot: &'a mut Option<Target>,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    config: &FractalConfig,
    width: u32,
    height: u32,
) -> &'a mut FlameRenderer {
    let tail = config.render_mode.is_non_flame();
    let fits = matches!(slot, Some(t) if t.width == width && t.height == height && t.tail == tail);
    if !fits {
        if let Some(old) = slot.take() {
            old.renderer.destroy();
        }
        let renderer = FlameRenderer::for_job(device, queue, wgpu::TextureFormat::Rgba8Unorm, width, height, config);
        *slot = Some(Target { renderer, width, height, tail });
    }
    &mut slot.as_mut().expect("made above").renderer
}

/// An animation, opened: the controller with its tracks bound, its
/// signals, and the renderer and engines its frames share -- a
/// simulation's field runs on from frame to frame rather than
/// restarting, as in a video.
struct Session {
    config: FractalConfig,
    controller: AnimationController,
    signals: Option<SignalManager>,
    fps: u32,
    frames: u32,
    width: u32,
    height: u32,
    options: Options,
    target: Option<Target>,
    engines: RenderEngines,
}

impl Session {
    fn frame_config(&mut self, frame: u32) -> (FractalConfig, Vec<FractalConfig>) {
        let time = frame as f64 / self.fps as f64;
        let values = match &self.signals {
            Some(s) => self.controller.evaluate_at_time_with_signals(time, Some(s)),
            None => self.controller.evaluate_at_time(time),
        };
        let mut config = self.config.clone();
        apply_animation_values(&mut config, &values);
        let moments = moment_configs(&self.config, &self.controller, time, self.signals.as_ref());
        (config, moments)
    }
}

/// The GPU and everything kept on it between calls.
pub struct Gpu {
    device: wgpu::Device,
    queue: wgpu::Queue,
    still: Option<Target>,
    /// A Python-driven sequence's engines (`render` with `sequence`).
    sequence: RenderEngines,
    sessions: HashMap<u32, Session>,
    next_session: u32,
}

impl Gpu {
    pub fn new() -> Result<Self, String> {
        let (device, queue) = backend::device()?;
        Ok(Gpu { device, queue, still: None, sequence: RenderEngines::default(), sessions: HashMap::new(), next_session: 1 })
    }

    /// A config at a size. With `sequence`, the frame continues the last
    /// sequence frame's engine state (a simulation runs on to this
    /// frame's step count) instead of starting afresh.
    pub fn render(&mut self, config: FractalConfig, width: u32, height: u32, options: &Options, sequence: bool) -> Result<Frame, String> {
        check_size(width, height)?;
        let renderer = renderer_for(&mut self.still, &self.device, &self.queue, &config, width, height);
        renderer.set_sticky_enabled(options.sticky);
        let mut job = options.job(&config, width, height);
        if sequence {
            job = job.with_engines(&mut self.sequence);
        }
        let ipt = job.iterations_per_thread;
        let out = block_on(render_with(renderer, &self.device, &self.queue, job, &mut NoProgress)).map_err(|e| e.to_string())?;
        Ok(Frame { out, iterations_per_thread: ipt })
    }

    /// Opens an animation (`.anim` JSON) over its base config -- `config`
    /// if given, else the one it embeds, as the CLI's export-animation.
    pub fn open(&mut self, animation: &str, config: Option<&str>, width: u32, height: u32, fps: u32, options: Options) -> Result<(u32, Value), String> {
        check_size(width, height)?;
        let mut animation = Animation::from_json(animation).map_err(|e| format!("animation did not parse: {e}"))?;
        let config = match config {
            Some(json) => FractalConfig::from_json(json).map_err(|e| format!("config did not parse: {e}"))?,
            None => animation.base_config.clone().ok_or("the animation embeds no base config; pass one")?,
        };
        let fps = fps.max(1);
        let mut warnings: Vec<String> = animation
            .remove_broken_tracks()
            .into_iter()
            .map(|t| format!("the track on `{}` lost its target and is left out", t.target))
            .collect();
        animation.bind_to_config(&config);
        let signals = export_signals(&animation, &HashMap::new());
        for (target, signal) in tracks_missing_signals(&animation, &signals) {
            warnings.push(format!("the track on `{target}` reads signal `{signal}`, which is not available here"));
        }
        let manager = (!signals.is_empty()).then(|| {
            let mut m = SignalManager::new();
            for s in signals.values() {
                m.insert(s.clone());
            }
            m
        });
        let frames = (animation.duration * fps as f64).ceil() as u32;
        let duration = animation.duration;
        let mut controller = AnimationController::new();
        controller.load(animation);
        controller.set_step(1.0 / fps as f64);
        let id = self.next_session;
        self.next_session += 1;
        self.sessions.insert(id, Session {
            config, controller, signals: manager, fps, frames, width, height, options,
            target: None, engines: RenderEngines::default(),
        });
        Ok((id, json!({ "id": id, "frames": frames, "fps": fps, "duration": duration, "warnings": warnings })))
    }

    fn session(&mut self, id: u32) -> Result<&mut Session, String> {
        self.sessions.get_mut(&id).ok_or_else(|| format!("no open animation {id}"))
    }

    /// An animation's frame, rendered.
    pub fn frame(&mut self, id: u32, frame: u32) -> Result<Frame, String> {
        let (device, queue) = (&self.device, &self.queue);
        let s = self.sessions.get_mut(&id).ok_or_else(|| format!("no open animation {id}"))?;
        if frame >= s.frames {
            return Err(format!("frame {frame} is past the animation's {} frames", s.frames));
        }
        let (config, moments) = s.frame_config(frame);
        let renderer = renderer_for(&mut s.target, device, queue, &config, s.width, s.height);
        renderer.set_sticky_enabled(s.options.sticky);
        let job = s.options.job(&config, s.width, s.height).with_engines(&mut s.engines).with_moments(&moments);
        let ipt = job.iterations_per_thread;
        let out = block_on(render_with(renderer, device, queue, job, &mut NoProgress)).map_err(|e| e.to_string())?;
        Ok(Frame { out, iterations_per_thread: ipt })
    }

    /// An animation's frame as a config, not rendered.
    pub fn frame_config(&mut self, id: u32, frame: u32) -> Result<FractalConfig, String> {
        Ok(self.session(id)?.frame_config(frame).0)
    }

    pub fn close(&mut self, id: u32) {
        if let Some(s) = self.sessions.remove(&id) {
            if let Some(t) = s.target {
                t.renderer.destroy();
            }
        }
    }
}

fn check_size(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err("width and height must be nonzero".into());
    }
    Ok(())
}
