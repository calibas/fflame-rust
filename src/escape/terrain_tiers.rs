//! A terrain's tiers (docs/projects/heightfield-3d.md, sections 4 and 8),
//! over a [`TerrainRenderer`]: the lit tier's antialiasing grid and the
//! path tracer, by tier, for the viewport and for an export. What an
//! escape terrain and a simulation's share -- each hands in its own view
//! and settings.

use super::path_core::{PathSettings, PATH_SHOW_SAMPLES};
use super::terrain::{TerrainRenderer, TerrainView};
use super::EscapeRenderer;
use crate::config::escape::RenderTier;
use std::sync::{Arc, Mutex};
use wgpu::{Device, Queue, TextureView};

/// The time a viewport frame gives the path tracer, in ms.
const PATH_FRAME_MS: f32 = 12.0;

/// What a terrain is drawn as, this frame.
pub struct TierInputs<'a> {
    /// The view, its rays offset by a jitter within their pixels.
    pub view: &'a dyn Fn([f32; 2]) -> TerrainView,
    pub settings: PathSettings,
    pub tier: RenderTier,
    /// The path tracer's target: the viewport's, and an export's count.
    pub samples: u32,
    /// The lit tier's antialiasing grid, a side.
    pub supersample: u32,
}

/// The tiers' state between frames.
#[derive(Default)]
pub struct TerrainTiers {
    /// What the viewport's accumulation is of, and how far it has got.
    key: Option<String>,
    lit_samples: u32,
    /// Whether the output is the path tracer's, not the lit tier's.
    showing_path: bool,
    /// A path-traced sample's time, in ms, measured to the GPU's
    /// completion of a batch, and its average.
    path_done: Arc<Mutex<Option<f32>>>,
    path_ms: Option<f32>,
}

impl TerrainTiers {
    /// Start over at the next frame, whatever the key says (a new size).
    pub fn invalidate(&mut self) {
        self.key = None;
    }

    /// One frame of the viewport. The lit tier: a sample of the
    /// antialiasing grid (`supersample²` jittered renders, the export's
    /// own) folded into its accumulation. Path traced: as many samples as
    /// fit the frame, added to the path tracer's sum, up to `samples`.
    /// Auto draws the lit tier while anything moves and path traces while
    /// nothing does, showing it from [`PATH_SHOW_SAMPLES`]. Any change to
    /// the view, the ground or the settings restarts both. True while
    /// there is more to do.
    pub fn viewport(&mut self, terrain: &mut TerrainRenderer, device: &Device, queue: &Queue, i: &TierInputs) -> bool {
        if terrain.tile_version() == 0 {
            return false;
        }
        let key = format!("{:?}|{}|{:?}", (i.view)([0.0, 0.0]), terrain.tile_version(), i.settings);
        if self.key.as_deref() != Some(key.as_str()) {
            self.key = Some(key);
            self.lit_samples = 0;
            terrain.reset_accumulation();
            terrain.reset_path();
            self.showing_path = false;
        }
        if i.tier != RenderTier::PathTraced {
            let grid = EscapeRenderer::sample_grid(i.supersample.max(1));
            if let Some(&jitter) = grid.get(self.lit_samples as usize) {
                terrain.render(device, queue, &(i.view)(jitter));
                terrain.accumulate(device, queue);
                self.lit_samples += 1;
                self.showing_path = false;
                return true;
            }
            if i.tier == RenderTier::Lit {
                return false;
            }
        }
        let target = i.samples.max(1);
        let have = terrain.path_samples();
        if have >= target {
            return false;
        }
        // As many samples as fit the frame at the measured cost.
        let per_frame = self.path_ms().map_or(1, |ms| (PATH_FRAME_MS / ms.max(0.05)).floor().clamp(1.0, 64.0) as u32);
        let n = per_frame.min(target - have);
        self.trace(terrain, device, queue, i, n);
        self.showing_path = i.tier == RenderTier::PathTraced || terrain.path_samples() >= PATH_SHOW_SAMPLES;
        terrain.path_samples() < target
    }

    /// The export's picture: path traced at `samples` unless the tier is
    /// Lit, which draws the antialiasing grid. In dispatches of a few
    /// samples each, `wait` called between them (a blocking poll on the
    /// desktop). Submits its own work.
    pub fn still(&mut self, terrain: &mut TerrainRenderer, device: &Device, queue: &Queue, i: &TierInputs, mut wait: impl FnMut()) {
        if i.tier == RenderTier::Lit {
            terrain.reset_accumulation();
            for jitter in EscapeRenderer::sample_grid(i.supersample.max(1)) {
                terrain.render(device, queue, &(i.view)(jitter));
                terrain.accumulate(device, queue);
            }
            self.showing_path = false;
            return;
        }
        terrain.reset_path();
        let target = i.samples.max(1);
        // A batch the watchdog never notices: about a quarter second at
        // the measured cost, from a cautious start.
        while terrain.path_samples() < target {
            let per = self.path_ms().map_or(2, |ms| (250.0 / ms.max(0.05)).floor().clamp(1.0, 64.0) as u32);
            let n = per.min(target - terrain.path_samples());
            self.trace(terrain, device, queue, i, n);
            wait();
        }
        self.showing_path = true;
    }

    /// Add `samples` path-traced samples of the view, timing them. Submits
    /// its own work.
    fn trace(&mut self, terrain: &mut TerrainRenderer, device: &Device, queue: &Queue, i: &TierInputs, samples: u32) {
        let t0 = web_time::Instant::now();
        terrain.render_path(device, queue, &(i.view)([0.0, 0.0]), &i.settings, samples, samples);
        let slot = Arc::clone(&self.path_done);
        queue.on_submitted_work_done(move || {
            if let Ok(mut g) = slot.lock() {
                *g = Some(t0.elapsed().as_secs_f32() * 1000.0 / samples.max(1) as f32);
            }
        });
    }

    /// A path-traced sample's average time, in ms, once measured.
    pub fn path_ms(&mut self) -> Option<f32> {
        if let Some(ms) = self.path_done.lock().ok().and_then(|mut g| g.take()) {
            self.path_ms = Some(match self.path_ms {
                Some(avg) => avg + (ms - avg) * 0.3,
                None => ms,
            });
        }
        self.path_ms
    }

    /// What the tail reads: the path tracer's resolve when it is shown,
    /// else the lit tier's accumulation once there is one, else the last
    /// render.
    pub fn output_view<'a>(&self, terrain: &'a TerrainRenderer) -> &'a TextureView {
        if self.showing_path {
            return terrain.output_view();
        }
        terrain.accumulated_view().unwrap_or(terrain.output_view())
    }

    /// Path-traced samples so far, and whether the picture is theirs.
    pub fn path_progress(&self, terrain: &TerrainRenderer) -> (u32, bool) {
        (terrain.path_samples(), self.showing_path)
    }
}
