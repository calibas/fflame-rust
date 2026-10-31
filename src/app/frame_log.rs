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
//! many seconds after it. `FFLAME_FRAME_LOG_RENDER_SCALE` renders at that
//! scale for the run, in place of the saved setting, which it does not
//! touch: a measurement needs a known ceiling. `FFLAME_FRAME_LOG_TARGET_FPS`
//! does the same for the Target Frame Rate (`display`, or frames a second),
//! and `FFLAME_FRAME_LOG_VSYNC` (`on` or `off`) for VSync.
//! `FFLAME_FRAME_LOG_NO_GPU_TIMER` starts the app without its frame timer,
//! as a browser without `timestamp-query` runs: the quality governor's
//! fallback (`governor::fallback_rung`).

use std::collections::VecDeque;
use std::io::Write;

use crate::gpu::timer::FrameTiming;

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
    /// The viewport's size in points, and the size its picture renders
    /// at (the render scale's; app/render_scale.rs).
    pub view: (u32, u32),
    pub render: (u32, u32),
    pub playing: bool,
    /// Rapid edits (the overwrite window, which playback uses for its
    /// own frames too, so not while it plays), or a mode-D preview.
    pub interacting: bool,
    /// The escape image shown is part-drawn: a chunk of a picture that
    /// is not settled yet, with no finished one held up in its place.
    pub escape_partial: bool,
    /// An escape picture settled this frame.
    pub escape_done: bool,
    /// The quality governor (app/governor.rs): the governed rung, an
    /// escape terrain's or escape-time's (its
    /// place on the ladder, 0 the best), the last picture it observed (GPU
    /// ms, frames), and the fixed part of the cost it has learned.
    pub gov_rung: usize,
    pub gov_picture: Option<(f64, u64)>,
    pub gov_fixed: f64,
    /// The frame budget the governor holds to, ms, and the display's
    /// refresh as the platform reports it, Hz.
    pub gov_budget_ms: f64,
    pub refresh_hz: f64,
    /// The escape renderer's supersampling in use, 0 without one.
    pub escape_ss: u32,
    /// The governor's batch scale, and the pacing cap it was held to, in
    /// the same units (governor.rs, `BatchCap`), when it was.
    pub iter_scale: f64,
    pub flame_cap: Option<f64>,
    /// The simulation's grid and its step, zeros without one.
    pub sim_grid: (u32, u32),
    pub sim_step: u32,
    /// The last timed batch's ms a step, raw, and the grid scale the
    /// governor holds (governor.rs), if it holds one.
    pub sim_step_ms: f64,
    pub sim_scale: Option<f32>,
}

/// A scripted run without the frame timer (`FFLAME_FRAME_LOG_NO_GPU_TIMER`,
/// with the frame log on).
pub(super) fn without_gpu_timer() -> bool {
    std::env::var("FFLAME_FRAME_LOG").is_ok_and(|p| !p.is_empty())
        && std::env::var("FFLAME_FRAME_LOG_NO_GPU_TIMER").is_ok_and(|v| !v.is_empty() && v != "0")
}

impl super::App {
    /// The render scale chosen in the Rendering menu, or a scripted run's
    /// (`FFLAME_FRAME_LOG_RENDER_SCALE`).
    pub(super) fn user_render_scale(&self) -> f32 {
        self.frame_log
            .as_ref()
            .and_then(|l| l.render_scale)
            .unwrap_or(self.config_manager.system_settings().render_scale)
    }

    /// The Target Frame Rate the governor holds: the setting's, or the
    /// scripted run's (`FFLAME_FRAME_LOG_TARGET_FPS`).
    pub(super) fn target_rate(&self) -> crate::storage::settings::TargetRate {
        self.frame_log
            .as_ref()
            .and_then(|l| l.target_rate)
            .unwrap_or(self.config_manager.system_settings().target_rate)
    }

    /// VSync: the setting's, or the scripted run's (`FFLAME_FRAME_LOG_VSYNC`).
    pub(super) fn vsync_enabled(&self) -> bool {
        self.frame_log
            .as_ref()
            .and_then(|l| l.vsync)
            .unwrap_or(self.config_manager.system_settings().vsync_enabled)
    }

    /// The scripted run's part of a frame, after the frame's row: on the
    /// first frame the settings' sidecar and the file opened (an
    /// animation played from its start); then the quit.
    pub(super) fn frame_log_drive(&mut self) {
        let (target_rate, render_scale, vsync) = (self.target_rate(), self.user_render_scale(), self.vsync_enabled());
        let Some(log) = self.frame_log.as_mut() else { return };
        if log.frame == 1 {
            let s = self.config_manager.system_settings();
            let sidecar = serde_json::json!({
                "open": log.open,
                "window": [self.gpu.size.width, self.gpu.size.height],
                "vsync_enabled": vsync,
                "target_fps": s.target_fps,
                "target_rate": format!("{target_rate:?}"),
                "render_scale": render_scale,
                "frame_governor": s.frame_governor,
                "manual_workgroups": s.manual_workgroups,
                "iterations_per_thread": s.iterations_per_thread,
                "burn_in": s.burn_in,
                "gpu_times": log.timed,
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
        let (sim_grid, sim_step, sim_step_ms) =
            self.sim_renderer.as_ref().map_or(((0, 0), 0, 0.0), |s| (s.grid_size(), s.step_index(), s.last_step().1));
        #[cfg(not(feature = "engine-sim"))]
        let (sim_grid, sim_step, sim_step_ms) = ((0, 0), 0, 0.0);
        FrameState {
            interval_ms,
            cpu_ms,
            ui_cpu_ms,
            acquire_ms: 0.0,
            present_ms: 0.0,
            mode,
            view: self.fractal_viewport_size,
            render: self.flame_renderer.as_ref().map_or((0, 0), |r| (r.width, r.height)),
            playing: self.animation_controller.is_playing(),
            interacting: (self.use_overwrite_next_frame && !self.animation_controller.is_playing()) || preview,
            escape_partial: self.escape_dirty
                && self.escape_renderer.as_ref().is_some_and(|e| e.presented_view().is_none()),
            escape_done: self.governor.pictures.iter().any(|p| p.last == self.frame_index),
            // Whichever is governed: an escape terrain's, else escape-time's.
            gov_rung: if self.governor.terrain_on { self.governor.terrain.rung_index() } else { self.governor.escape.rung_index() },
            gov_picture: self.governor.last_picture,
            gov_fixed: if self.governor.terrain_on { self.governor.terrain.fixed() } else { self.governor.escape.fixed() },
            gov_budget_ms: self.governor.capped.unwrap_or(self.governor_budget_ms()),
            refresh_hz: self.display_refresh_mhz.map_or(0.0, |m| m as f64 / 1000.0),
            escape_ss: self.escape_renderer.as_ref().map_or(0, |e| e.effective_supersample()),
            iter_scale: self.iter_scale,
            flame_cap: self.governor.flame_cap,
            sim_grid,
            sim_step,
            sim_step_ms,
            sim_scale: self.governor.sim.scale,
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
    /// Whether the app's frame timer runs: rows wait for their GPU times
    /// only then.
    timed: bool,
    pending: VecDeque<Row>,
    /// This frame's flame dispatch, until the frame's row is made.
    flame: Option<(u32, u32, u64)>,
    /// The scripted run: the file to open, and the seconds to run after it.
    open: Option<String>,
    seconds: Option<f64>,
    opened_at: Option<web_time::Instant>,
    /// The run's render scale and target frame rate, in place of the
    /// settings'.
    render_scale: Option<f32>,
    target_rate: Option<crate::storage::settings::TargetRate>,
    vsync: Option<bool>,
}

impl FrameLog {
    /// The log `FFLAME_FRAME_LOG` asks for, if it does and the file opens.
    pub(super) fn from_env(timed: bool) -> Option<Self> {
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
             render_w,render_h,playing,interacting,escape_ss,escape_partial,escape_done,gov_rung,gov_picture_ms,gov_picture_frames,gov_fixed,gov_budget_ms,refresh_hz,\
             iter_scale,flame_workgroups,flame_ipt,flame_samples,sim_grid_w,sim_grid_h,sim_step,sim_step_ms,sim_scale,flame_cap"
        );
        log::info!(
            "Frame log: {path}{}",
            if timed { "" } else { " (no GPU times: the device cannot time passes)" }
        );
        Some(Self {
            path,
            out,
            start: web_time::Instant::now(),
            frame: 0,
            timed,
            pending: VecDeque::new(),
            flame: None,
            open: std::env::var("FFLAME_FRAME_LOG_OPEN").ok().filter(|p| !p.is_empty()),
            seconds: std::env::var("FFLAME_FRAME_LOG_SECONDS").ok().and_then(|s| s.parse().ok()),
            opened_at: None,
            render_scale: std::env::var("FFLAME_FRAME_LOG_RENDER_SCALE")
                .ok()
                .and_then(|s| s.parse::<f32>().ok())
                .filter(|s| s.is_finite() && *s > 0.0),
            target_rate: std::env::var("FFLAME_FRAME_LOG_TARGET_FPS").ok().and_then(|s| {
                use crate::storage::settings::TargetRate;
                if s.eq_ignore_ascii_case("display") {
                    return Some(TargetRate::Display);
                }
                s.parse::<f32>().ok().filter(|f| f.is_finite() && *f > 0.0).map(TargetRate::Fixed)
            }),
            vsync: std::env::var("FFLAME_FRAME_LOG_VSYNC").ok().and_then(|s| match s.to_ascii_lowercase().as_str() {
                "on" | "1" | "true" => Some(true),
                "off" | "0" | "false" => Some(false),
                _ => None,
            }),
        })
    }

    /// The flame dispatched this frame.
    pub(super) fn flame(&mut self, workgroups: u32, ipt: u32, samples: u64) {
        self.flame = Some((workgroups, ipt, samples));
    }

    /// The frame (the app's `frame`) is submitted and presented: its row,
    /// and every row whose GPU times are in (`timings`, the frames the
    /// frame timer has heard back from) or will not come.
    pub(super) fn finish(&mut self, state: FrameState, frame: u64, timings: &[FrameTiming]) {
        self.pending.push_back(Row {
            frame,
            t_ms: self.start.elapsed().as_secs_f64() * 1000.0,
            state,
            flame: self.flame.take(),
            gpu: None,
        });
        for timing in timings {
            if let Some(row) = self.pending.iter_mut().find(|r| r.frame == timing.frame) {
                row.gpu = Some(*timing);
            }
        }
        while let Some(row) = self.pending.front() {
            if self.timed && row.gpu.is_none() && frame - row.frame < GPU_WAIT {
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
            "{},{:.3},{:.3},{:.3},{:.3},{:.3},{:.3},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{},{:.3},{:.3},{:.2},{:.4},{},{},{},{},{},{},{:.3},{},{}",
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
            s.render.0,
            s.render.1,
            u8::from(s.playing),
            u8::from(s.interacting),
            s.escape_ss,
            u8::from(s.escape_partial),
            u8::from(s.escape_done),
            s.gov_rung,
            opt(s.gov_picture.map(|p| p.0)),
            s.gov_picture.map_or(String::new(), |p| p.1.to_string()),
            s.gov_fixed,
            s.gov_budget_ms,
            s.refresh_hz,
            s.iter_scale,
            wg,
            ipt,
            samples,
            s.sim_grid.0,
            s.sim_grid.1,
            s.sim_step,
            s.sim_step_ms,
            s.sim_scale.map_or(String::new(), |v| format!("{v:.4}")),
            s.flame_cap.map_or(String::new(), |v| format!("{v:.4}")),
        );
    }
}
