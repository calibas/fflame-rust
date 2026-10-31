//! **An active simulation keeps its grid** (docs/projects/quality-governor.md,
//! D7). A bound grid's cells follow the viewport, and a change of cells
//! reseeds the run, so a resize or full screen used to restart a live
//! simulation. While a run is active (the transport running, or the
//! timeline driving its steps) its grid is held instead: the viewport
//! letterboxes to the grid's shape once its size is no longer the one the
//! run began at (`ui::SimLock`, `view_controls::letterbox_shape`), and the
//! renderer is resized with the grid fixed at the run's cells, so the field
//! goes on at its step, shown larger or smaller. A change of the run's scale
//! (an edit, or the quality governor's pick) keeps the lock: the held grid
//! is the run's at the new scale, the same shape, and the field is carried
//! into it (`held_grid`). Pausing or stopping lets go: the viewport fills
//! its panel again and the grid follows it at the config's scale, re-run
//! from the seed if its shape changed, as a resize always did.
//!
//! Not with the letterbox setting on (the viewport has `image_size`'s
//! shape already and a resize never changes the grid's), and not for a
//! fixed grid, which never follows the window.

use crate::ui::SimLock;

/// The lock this frame, from last frame's: `eligible` is the run's grid
/// scale while a bound grid's run is active (None otherwise), `grid` the
/// run's grid when it matches the viewport now, `viewport` the viewport's
/// size as it reports it unletterboxed. Held while the run is, with the
/// scale it was taken at.
pub(super) fn next_lock(
    held: Option<(SimLock, f32)>,
    eligible: Option<f32>,
    grid: Option<(u32, u32)>,
    viewport: (u32, u32),
) -> Option<(SimLock, f32)> {
    let scale = eligible?;
    match held {
        Some(held) => Some(held),
        None => grid.map(|grid| (SimLock { grid, viewport }, scale)),
    }
}

/// The grid a lock holds at the run's scale `now`: its grid, taken at
/// `at`, scaled the same in both axes.
pub(super) fn held_grid(lock: SimLock, at: f32, now: f32) -> (u32, u32) {
    let r = if at > 0.0 && now > 0.0 { now / at } else { 1.0 };
    let side = |n: u32| ((n as f32 * r).round() as u32).max(1);
    (side(lock.grid.0), side(lock.grid.1))
}

impl super::App {
    /// Take, hold or let go of the active simulation's grid, before the
    /// UI draws the viewport (`next_lock`).
    pub(super) fn update_sim_lock(&mut self) {
        let config = self.config_manager.active_config();
        let active = self.sim_running || self.timeline_owns_sim();
        let eligible = match config.sim.grid {
            crate::config::sim::SimGrid::Viewport { scale }
                if config.render_mode == crate::scene::transforms::RenderMode::Simulation
                    && !self.config_manager.system_settings().viewport_letterbox
                    && active =>
            {
                // The governor's pick, when it holds one (governor.rs).
                Some(self.governor.sim.scale.unwrap_or(scale))
            }
            _ => None,
        };
        // The run's grid, while it is the viewport's: one renderer resize
        // behind a window being dragged, it would be the old shape; and at
        // the run's scale, which the governor may have just picked.
        let rendered = self.flame_renderer.as_ref().map(|r| (r.width, r.height));
        let at_scale = eligible.zip(self.sim_grid_at).is_some_and(|(want, at)| (want - at).abs() < 1e-6);
        let grid = self
            .sim_renderer
            .as_ref()
            .filter(|s| Some(s.output_size()) == rendered && at_scale)
            .map(|s| s.grid_size());
        self.sim_lock = next_lock(self.sim_lock, eligible, grid, self.fractal_viewport_size);
        self.egui_layer.set_sim_lock(self.sim_lock.map(|(lock, _)| lock));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: (u32, u32) = (1079, 1056);

    #[test]
    fn a_run_takes_its_grid_and_holds_it_through_a_resize() {
        let lock = next_lock(None, Some(1.0), Some((1079, 1056)), VIEW).expect("taken");
        assert_eq!(lock.0, SimLock { grid: (1079, 1056), viewport: VIEW });
        // The window grows; the run's grid and its starting size are kept.
        let held = next_lock(Some(lock), Some(1.0), Some((1079, 1056)), (1920, 1080)).expect("held");
        assert_eq!(held, lock);
    }

    #[test]
    fn stopping_or_leaving_lets_go() {
        let lock = next_lock(None, Some(0.5), Some((540, 528)), VIEW);
        assert_eq!(next_lock(lock, None, Some((540, 528)), VIEW), None);
    }

    #[test]
    fn a_change_of_scale_keeps_the_lock_and_scales_its_grid() {
        let lock = next_lock(None, Some(1.0), Some((1079, 1056)), VIEW);
        // Letterboxed to the run's shape in a wider window, the scale is
        // edited (or the governor picks): the lock holds, and its grid is
        // the run's at the new scale, the same shape.
        let (held, at) = next_lock(lock, Some(0.5), Some((1079, 1056)), (1920, 1080)).expect("held");
        assert_eq!(held, lock.unwrap().0);
        assert_eq!(held_grid(held, at, 0.5), (540, 528));
        assert_eq!(held_grid(held, at, 1.0), (1079, 1056));
    }

    #[test]
    fn nothing_is_taken_until_the_grid_is_the_viewports() {
        assert_eq!(next_lock(None, Some(1.0), None, VIEW), None);
    }
}
