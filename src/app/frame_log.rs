//! **The frame log** (docs/projects/quality-governor.md, phase 1): a
//! row a frame of what the frame cost and what it rendered, for
//! measuring the governor's rungs in the running app.
//!
//! Off unless `FFLAME_FRAME_LOG` names a file: then a CSV is written
//! there, one row a frame, and beside it `<file>.json`, the device
//! settings the frame times depend on. The GPU's times
//! (`gpu::timer::FrameTimer`) come back a few frames late, so a row waits
//! for its frame's and is written without them once `GPU_WAIT` frames
//! have passed; a device that cannot time passes writes every row
//! without them. `scripts/frame_metrics.py summary` reads it.
//!
//! For a scripted run (`scripts/governor_playback.py`), two more:
//! `FFLAME_FRAME_LOG_OPEN` opens an `.anim` (and plays it) or an
//! `.fflame` on the first frame, and `FFLAME_FRAME_LOG_SECONDS` quits that
//! many seconds after it.

use std::collections::VecDeque;
use std::io::Write;

use crate::gpu::timer::{FrameTiming, FrameTimer};

/// Frames a row waits for its GPU times.
const GPU_WAIT: u64 = 16;

/// What the frame rendered, read off the app at the frame's end.
#[derive(Clone, Debug, Default)]
pub(crate) struct FrameState {
    /// The time since the last frame started, ms.
    pub interval_ms: f64,
    /// `render`'s own time, ms.
    pub cpu_ms: f64,
    /// The UI's share of it, ms.
    pub ui_cpu_ms: f64,
    /// Its waits for the surface: acquiring the frame's image (what a
    /// GPU still busy with earlier frames shows up as) and presenting it.
    pub acquire_ms: f64,
    pub present_ms: f64,
    pub mode: &'static str,
    pub view: (u32, u32),
    pub playing: bool,
    /// Rapid edits (the overwrite window, which playback uses for its
    /// own frames too, so not while it plays), or a mode-D preview.
    pub interacting: bool,
    /// The escape image shown is part-drawn: a chunk of a picture that
    /// is not settled yet.
    pub escape_partial: bool,
    /// The escape renderer's supersampling in use, 0 without one.
    pub escape_ss: u32,
    /// The governor's batch scale.
    pub iter_scale: f64,
    /// The simulation's grid and its step, zeros without one.
    pub sim_grid: (u32, u32),
    pub sim_step: u32,
}

impl super::App {
    /// The scripted run's part of a frame, after the frame's row: on the
    /// first frame the settings' sidecar and the file opened (an
    /// animation played from its start); then the quit.
    pub(super) fn frame_log_drive(&mut self) {
        let Some(log) = self.frame_log.as_mut() else { return };
        if log.frame == 1 {
            let s = self.config_manager.system_settings();
            let sidecar = serde_json::json!({
                "open": log.open,
                "window": [self.gpu.size.width, self.gpu.size.height],
                "vsync_enabled": s.vsync_enabled,
                "target_fps": s.target_fps,
                "frame_governor": s.frame_governor,
                "manual_workgroups": s.manual_workgroups,
                "iterations_per_thread": s.iterations_per_thread,
                "burn_in": s.burn_in,
                "gpu_times": log.timer.is_some(),
            });
            let _ = std::fs::write(format!("{}.json", log.path), serde_json::to_string_pretty(&sidecar).unwrap_or_default());
            let Some(path) = log.open.clone() else { return };
            log.opened_at = Some(web_time::Instant::now());
            // Opened as the web build opens a picked file's text.
            match std::fs::read_to_string(&path) {
                Ok(text) if path.ends_with(".anim") => {
                    self.open_animation_text(&text);
                    if self.animation_controller.animation.is_some() {
                        self.animation_controller.play();
                    }
                }
                Ok(text) => self.open_config_text(&text),
                Err(e) => log::error!("FFLAME_FRAME_LOG_OPEN: {path}: {e}"),
            }
            return;
        }
        let start = log.opened_at.unwrap_or(log.start);
        if log.seconds.is_some_and(|s| start.elapsed().as_secs_f64() >= s) {
            self.quit_requested = true;
        }
    }

    /// The frame log's reading of the app at the frame's end.
    pub(super) fn frame_state(&self, interval_ms: f64, cpu_ms: f64, ui_cpu_ms: f64) -> FrameState {
        use crate::scene::transforms::RenderMode;
        let config = self.config_manager.active_config();
        let mode = match config.render_mode {
            RenderMode::TwoD => "2d",
            RenderMode::ThreeD => "3d",
            RenderMode::Escape if config.escape.terrain_active() => "escape-terrain",
            RenderMode::Escape => "escape",
            RenderMode::Simulation if config.sim.terrain_active() => "sim-terrain",
            RenderMode::Simulation => "sim",
        };
        let preview = self.escape_last_edit.is_some_and(|t| t.elapsed() < super::ESCAPE_INTERACTION_WINDOW);
        #[cfg(feature = "engine-sim")]
        let (sim_grid, sim_step) = self.sim_renderer.as_ref().map_or(((0, 0), 0), |s| (s.grid_size(), s.step_index()));
        #[cfg(not(feature = "engine-sim"))]
        let (sim_grid, sim_step) = ((0, 0), 0);
        FrameState {
            interval_ms,
            cpu_ms,
            ui_cpu_ms,
            acquire_ms: 0.0,
            present_ms: 0.0,
            mode,
            view: self.fractal_viewport_size,
            playing: self.animation_controller.is_playing(),
            interacting: (self.use_overwrite_next_frame && !self.animation_controller.is_playing()) || preview,
            escape_partial: self.escape_renderer.is_some() && self.escape_dirty,
            escape_ss: self.escape_renderer.as_ref().map_or(0, |e| e.effective_supersample()),
            iter_scale: self.iter_scale,
            sim_grid,
            sim_step,
        }
    }
}

struct Row {
    frame: u64,
    t_ms: f64,
    state: FrameState,
    /// The flame's dispatch: workgroups, iterations a thread, samples.
    flame: Option<(u32, u32, u64)>,
    gpu: Option<FrameTiming>,
}

pub(crate) struct FrameLog {
    path: String,
    out: std::io::BufWriter<std::fs::File>,
    start: web_time::Instant,
    frame: u64,
    timer: Option<FrameTimer>,
    pending: VecDeque<Row>,
    /// This frame's flame dispatch, until the frame's row is made.
    flame: Option<(u32, u32, u64)>,
    /// The scripted run: the file to open, and the seconds to run after it.
    open: Option<String>,
    seconds: Option<f64>,
    opened_at: Option<web_time::Instant>,
}

impl FrameLog {
    /// The log `FFLAME_FRAME_LOG` asks for, if it does and the file opens.
    pub(super) fn from_env(device: &wgpu::Device, queue: &wgpu::Queue) -> Option<Self> {
        let path = std::env::var("FFLAME_FRAME_LOG").ok().filter(|p| !p.is_empty())?;
        let file = match std::fs::File::create(&path) {
            Ok(f) => f,
            Err(e) => {
                log::error!("FFLAME_FRAME_LOG: cannot write {path}: {e}");
                return None;
            }
        };
        let mut out = std::io::BufWriter::new(file);
        let _ = writeln!(
            out,
            "frame,t_ms,interval_ms,cpu_ms,ui_cpu_ms,acquire_ms,present_ms,gpu_ui_ms,gpu_engine_ms,mode,view_w,view_h,\
             playing,interacting,escape_ss,escape_partial,iter_scale,flame_workgroups,flame_ipt,flame_samples,sim_grid_w,sim_grid_h,sim_step"
        );
        let timer = FrameTimer::new(device, queue);
        log::info!(
            "Frame log: {path}{}",
            if timer.is_some() { "" } else { " (no GPU times: the device cannot time passes)" }
        );
        Some(Self {
            path,
            out,
            start: web_time::Instant::now(),
            frame: 0,
            timer,
            pending: VecDeque::new(),
            flame: None,
            open: std::env::var("FFLAME_FRAME_LOG_OPEN").ok().filter(|p| !p.is_empty()),
            seconds: std::env::var("FFLAME_FRAME_LOG_SECONDS").ok().and_then(|s| s.parse().ok()),
            opened_at: None,
        })
    }

    /// The frame's first encoder is made.
    pub(super) fn begin(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(t) = self.timer.as_mut() {
            t.begin(encoder, self.frame);
        }
    }

    /// The UI's paint is recorded, before its encoder is submitted.
    pub(super) fn ui_done(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(t) = self.timer.as_mut() {
            t.ui_done(encoder);
        }
    }

    /// The flame dispatched this frame.
    pub(super) fn flame(&mut self, workgroups: u32, ipt: u32, samples: u64) {
        self.flame = Some((workgroups, ipt, samples));
    }

    /// The engine's last encoder is recorded, before it is submitted.
    pub(super) fn end(&mut self, encoder: &mut wgpu::CommandEncoder) {
        if let Some(t) = self.timer.as_mut() {
            t.end(encoder);
        }
    }

    /// The frame is submitted and presented: its row, and every row whose
    /// GPU times are in (or will not come).
    pub(super) fn finish(&mut self, state: FrameState) {
        self.pending.push_back(Row {
            frame: self.frame,
            t_ms: self.start.elapsed().as_secs_f64() * 1000.0,
            state,
            flame: self.flame.take(),
            gpu: None,
        });
        if let Some(t) = self.timer.as_mut() {
            for timing in t.collect() {
                if let Some(row) = self.pending.iter_mut().find(|r| r.frame == timing.frame) {
                    row.gpu = Some(timing);
                }
            }
        }
        let timed = self.timer.is_some();
        while let Some(row) = self.pending.front() {
            if timed && row.gpu.is_none() && self.frame - row.frame < GPU_WAIT {
                break;
            }
            let row = self.pending.pop_front().unwrap();
            self.write(&row);
        }
        let _ = self.out.flush();
        self.frame += 1;
    }

    fn write(&mut self, r: &Row) {
        let s = &r.state;
        let opt = |v: Option<f64>| v.map_or(String::new(), |v| format!("{v:.3}"));
        let (wg, ipt, samples) = r.flame.map_or((String::new(), String::new(), String::new()), |(w, i, n)| {
            (w.to_string(), i.to_string(), n.to_string())
        });
        let _ = writeln!(
            self.out,
            "{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{},{},{},{},{},{},{},{:.4},{},{},{},{},{},{}",
            r.frame,
            r.t_ms,
            s.interval_ms,
            s.cpu_ms,
            s.ui_cpu_ms,
            s.acquire_ms,
            s.present_ms,
            opt(r.gpu.map(|g| g.ui_ms)),
            opt(r.gpu.map(|g| g.engine_ms)),
            s.mode,
            s.view.0,
            s.view.1,
            u8::from(s.playing),
            u8::from(s.interacting),
            s.escape_ss,
            u8::from(s.escape_partial),
            s.iter_scale,
            wg,
            ipt,
            samples,
            s.sim_grid.0,
            s.sim_grid.1,
            s.sim_step,
        );
    }
}
