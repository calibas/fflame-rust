//! What the menu bar's progress bar shows (`ui/render_progress.rs`),
//! read from counters the frame loop already keeps.

use super::App;
use crate::scene::transforms::RenderMode;
use crate::ui::{ExportStatus, RenderProgress};

impl App {
    /// The escape view's path tracer, once it is the one working --
    /// a terrain's, or a solid's once its walk has settled (or at once,
    /// in the Path Traced tier): its samples, target, and the fraction
    /// with a part-done pass counted.
    fn escape_path_progress(&self) -> Option<(u32, u32, f32)> {
        use crate::config::escape::RenderTier;
        let escape = &self.config_manager.active_config().escape;
        let target = escape.path.samples.max(1);
        #[cfg(feature = "terrain")]
        if escape.terrain_active() {
            if escape.terrain.tier == RenderTier::Lit {
                return None;
            }
            let (samples, _) = self.escape_terrain.as_ref()?.path_progress();
            return (samples > 0).then(|| (samples.min(target), target, samples as f32 / target as f32));
        }
        if !crate::escape::ifs::formula_is_solid(&escape.formula) || escape.solid_tier == RenderTier::Lit || self.escape_dirty {
            return None;
        }
        let r = self.escape_renderer.as_ref()?;
        let samples = r.solid_path_samples().min(target);
        Some((samples, target, r.solid_path_progress().min(target as f32) / target as f32))
    }

    /// The one rendering task the bar reports, in priority order: an
    /// export, then playback, then the mode's own render.
    ///
    /// An export comes first because it pauses the viewport's render
    /// for as long as it runs. Playback comes next because it renders
    /// without end in every mode -- an animated flame overwrites its
    /// picture each frame, and a timeline owns the simulation's steps.
    pub(super) fn render_progress(&self, export: Option<&ExportStatus>) -> RenderProgress {
        if let Some(export) = export.filter(|e| e.active) {
            return RenderProgress::Export {
                fraction: export.fraction,
                headline: export.headline(),
                detail: export.detail.clone(),
            };
        }
        if self.animation_controller.is_playing() {
            return RenderProgress::Animation;
        }
        let config = self.config_manager.active_config();
        match config.render_mode {
            RenderMode::Escape => {
                if let Some((samples, target, fraction)) = self.escape_path_progress() {
                    return RenderProgress::PathTrace { samples, target, fraction };
                }
                // The same conditions that keep the escape frames
                // coming: an unsettled render, a texture being made, and
                // the interaction window, whose quarter-resolution
                // preview settles before the full render is asked for.
                let refining = self.escape_dirty
                    || self.escape_texture.busy()
                    || self
                        .escape_last_edit
                        .is_some_and(|t| t.elapsed() < super::ESCAPE_INTERACTION_WINDOW);
                RenderProgress::Escape {
                    settled: !refining,
                    fraction: crate::escape::renderer::render_progress()
                        .filter(|_| refining)
                        .map(|(done, want)| done as f32 / want.max(1) as f32),
                }
            }
            RenderMode::Simulation => {
                // A terrain's path tracer, once it is the one working: the
                // run at rest (a running one shows the lit tier).
                #[cfg(all(feature = "engine-sim", feature = "terrain"))]
                if config.sim.terrain_active()
                    && config.sim.terrain.tier != crate::config::escape::RenderTier::Lit
                    && !self.sim_running
                {
                    if let Some(t) = self.sim_terrain.as_ref() {
                        let target = config.sim.terrain.path.samples.max(1);
                        let (samples, _) = t.path_progress();
                        if samples > 0 {
                            let samples = samples.min(target);
                            return RenderProgress::PathTrace { samples, target, fraction: samples as f32 / target as f32 };
                        }
                    }
                }
                #[cfg(feature = "engine-sim")]
                {
                    RenderProgress::Sim {
                        step: self.sim_renderer.as_ref().map_or(0, |s| s.step_index()),
                        cap: config.sim.steps,
                        // Catching up to a scrubbed step count runs too.
                        running: self.sim_running || self.sim_timeline_target.is_some(),
                    }
                }
                #[cfg(not(feature = "engine-sim"))]
                {
                    RenderProgress::Idle
                }
            }
            RenderMode::TwoD | RenderMode::ThreeD => match self.flame_renderer.as_ref() {
                Some(renderer) => RenderProgress::Flame {
                    done: renderer.total_iterations(),
                    target: config.max_iterations,
                    paused: self.paused,
                },
                None => RenderProgress::Idle,
            },
        }
    }
}
