//! What the menu bar's progress bar shows (`ui/render_progress.rs`),
//! read from counters the frame loop already keeps.

use super::App;
use crate::scene::transforms::RenderMode;
use crate::ui::{ExportStatus, RenderProgress};

impl App {
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
