//! The menu bar's progress bar: one bar for every rendering task.
//!
//! The app decides each frame WHAT is running ([`RenderProgress`]) from
//! counters it already holds: the flame's sample count, the escape
//! renderer's settled flag, the simulation's step index, the animation
//! controller and the export status. So the bar costs no readback.
//!
//! It costs no frames either. It is drawn inside a UI pass that runs
//! anyway, and it never asks for a repaint: every state in which it
//! moves on its own (an escape render refining, an animation playing, a
//! simulation running past its cap) is one in which the app is already
//! redrawing. An export redraws at ten frames a second for it, not at
//! the display rate (the event loop's `AboutToWait`). Its text is built
//! only while the pointer is on it.

use rust_i18n::t;

/// What the bar shows. Built by the app (`App::render_progress`) and
/// handed to the UI once a frame.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum RenderProgress {
    /// Nothing to report: no renderer yet.
    #[default]
    Idle,
    /// A flame accumulating toward its iteration target.
    Flame { done: u64, target: u64, paused: bool },
    /// An escape render. `fraction` is the share of the iteration budget
    /// the perturbed path has submitted, when it reports one.
    Escape { settled: bool, fraction: Option<f32> },
    /// A simulation at `step` of `cap` (a cap of 0 means no Max Steps).
    Sim { step: u32, cap: u32, running: bool },
    /// Animation playback: frames keep coming, with no end to measure.
    Animation,
    /// A PNG or video export, which takes the bar over.
    Export { fraction: f32, headline: String, detail: String },
}

/// How the bar is filled.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Fill {
    /// Filled to a fraction. `active` is false for a finished or paused
    /// task, which is drawn in a quieter colour.
    Part { fraction: f32, active: bool },
    /// Running with no measure of how far: a segment sweeps the track.
    Sweep,
}

impl RenderProgress {
    pub fn fill(&self) -> Fill {
        let part = |fraction: f32, active: bool| Fill::Part {
            fraction: fraction.clamp(0.0, 1.0),
            active,
        };
        match *self {
            RenderProgress::Idle => part(0.0, false),
            RenderProgress::Flame { done, target, paused } => {
                if target == 0 || done >= target {
                    part(1.0, false)
                } else {
                    part(done as f32 / target as f32, !paused)
                }
            }
            RenderProgress::Escape { settled: true, .. } => part(1.0, false),
            RenderProgress::Escape { settled: false, .. } => Fill::Sweep,
            RenderProgress::Sim { step, cap, running } => {
                if cap > 0 && step < cap {
                    part(step as f32 / cap as f32, running)
                } else if running {
                    // Past Max Steps, or no cap: running, with no end.
                    Fill::Sweep
                } else if cap > 0 {
                    part(1.0, false)
                } else {
                    part(0.0, false)
                }
            }
            RenderProgress::Animation => Fill::Sweep,
            RenderProgress::Export { fraction, .. } => part(fraction, true),
        }
    }

    /// Whether something is running: the bar moves, or fills in the
    /// accent colour. A finished, paused or idle task is not.
    pub fn is_active(&self) -> bool {
        match self.fill() {
            Fill::Part { active, .. } => active,
            Fill::Sweep => true,
        }
    }

    /// The hover text. Built only while hovered.
    fn describe(&self) -> String {
        let percent = |f: f32| format!("{:.0}", (f * 100.0).clamp(0.0, 100.0));
        match self {
            RenderProgress::Idle => t!("progress.idle").to_string(),
            RenderProgress::Flame { done, target, paused } => {
                let fmt = crate::ui::formatting::format_iterations;
                if *target == 0 || done >= target {
                    t!("progress.flame_done", target = fmt(*target)).to_string()
                } else {
                    let (p, d, tg) = (percent(*done as f32 / *target as f32), fmt(*done), fmt(*target));
                    if *paused {
                        t!("progress.flame_paused", percent = p, done = d, target = tg).to_string()
                    } else {
                        t!("progress.flame", percent = p, done = d, target = tg).to_string()
                    }
                }
            }
            RenderProgress::Escape { settled: true, .. } => t!("progress.escape_settled").to_string(),
            RenderProgress::Escape { settled: false, fraction: Some(f) } => {
                t!("progress.escape_rendering_percent", percent = percent(*f)).to_string()
            }
            RenderProgress::Escape { settled: false, fraction: None } => {
                t!("progress.escape_rendering").to_string()
            }
            RenderProgress::Sim { step, cap, running } => {
                if *cap > 0 && step < cap && *running {
                    t!("progress.sim", step = step, cap = cap).to_string()
                } else if *cap > 0 && step < cap {
                    t!("progress.sim_paused", step = step, cap = cap).to_string()
                } else if *running && *cap > 0 {
                    t!("progress.sim_past_cap", step = step).to_string()
                } else if *running {
                    t!("progress.sim_uncapped", step = step).to_string()
                } else if *cap > 0 {
                    t!("progress.sim_done", step = step).to_string()
                } else {
                    t!("progress.sim_stopped", step = step).to_string()
                }
            }
            RenderProgress::Animation => t!("progress.animation").to_string(),
            RenderProgress::Export { fraction, headline, detail } => {
                let mut s = format!("{headline} \u{b7} {}%", percent(*fraction));
                if !detail.is_empty() {
                    s.push('\n');
                    s.push_str(detail);
                }
                s
            }
        }
    }
}

/// Draw the bar at the cursor in a `size` slot: a track at most 6 px
/// tall, centred in it.
pub fn show(ui: &mut egui::Ui, progress: &RenderProgress, size: egui::Vec2) -> egui::Response {
    let (rect, response) = ui.allocate_exact_size(size, egui::Sense::hover());
    let width = size.x;
    if ui.is_rect_visible(rect) {
        let height = size.y.min(6.0);
        let track = egui::Rect::from_center_size(rect.center(), egui::vec2(width, height));
        let radius = height / 2.0;
        let visuals = ui.visuals();
        let painter = ui.painter();
        painter.rect(
            track,
            radius,
            visuals.extreme_bg_color,
            visuals.widgets.noninteractive.bg_stroke,
            egui::StrokeKind::Inside,
        );
        let accent = visuals.selection.bg_fill;
        match progress.fill() {
            Fill::Part { fraction, active } => {
                if fraction > 0.0 {
                    let mut bar = track;
                    bar.set_width((width * fraction).max(height));
                    let colour = if active {
                        accent
                    } else {
                        visuals.widgets.inactive.fg_stroke.color.gamma_multiply(0.6)
                    };
                    painter.rect_filled(bar, radius, colour);
                }
            }
            Fill::Sweep => {
                // A third of the track, crossing it once a second and a
                // half. Time comes from the frame being drawn anyway.
                const PERIOD: f64 = 1.5;
                let phase = (ui.input(|i| i.time) % PERIOD / PERIOD) as f32;
                let seg = width / 3.0;
                let left = track.left() - seg + phase * (width + seg);
                let bar = egui::Rect::from_min_max(
                    egui::pos2(left.max(track.left()), track.top()),
                    egui::pos2((left + seg).min(track.right()), track.bottom()),
                );
                if bar.width() > 0.0 {
                    painter.rect_filled(bar, radius, accent);
                }
            }
        }
    }
    response.on_hover_ui(|ui| {
        ui.label(progress.describe());
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn part(p: &RenderProgress) -> (f32, bool) {
        match p.fill() {
            Fill::Part { fraction, active } => (fraction, active),
            Fill::Sweep => panic!("{p:?} swept"),
        }
    }

    #[test]
    fn a_flame_fills_toward_its_iteration_target() {
        let at = |done, paused| RenderProgress::Flame { done, target: 1000, paused };
        assert_eq!(part(&at(250, false)), (0.25, true));
        // Paused: the same fill, drawn quietly.
        assert_eq!(part(&at(250, true)), (0.25, false));
        // Done, and past it (the last dispatch overshoots).
        assert_eq!(part(&at(1000, false)), (1.0, false));
        assert_eq!(part(&at(1300, false)), (1.0, false));
    }

    #[test]
    fn an_escape_render_sweeps_until_it_settles() {
        let busy = RenderProgress::Escape { settled: false, fraction: Some(0.4) };
        assert_eq!(busy.fill(), Fill::Sweep);
        let done = RenderProgress::Escape { settled: true, fraction: None };
        assert_eq!(part(&done), (1.0, false));
    }

    #[test]
    fn a_simulation_fills_to_max_steps_then_sweeps_past_it() {
        let sim = |step, cap, running| RenderProgress::Sim { step, cap, running };
        assert_eq!(part(&sim(50, 200, true)), (0.25, true));
        assert_eq!(part(&sim(50, 200, false)), (0.25, false));
        // Reached the cap and paused there.
        assert_eq!(part(&sim(200, 200, false)), (1.0, false));
        // Run pressed again: it carries on past the cap.
        assert_eq!(sim(260, 200, true).fill(), Fill::Sweep);
        // No cap at all.
        assert_eq!(sim(10, 0, true).fill(), Fill::Sweep);
        assert_eq!(part(&sim(10, 0, false)), (0.0, false));
    }

    #[test]
    fn playback_sweeps_and_an_export_fills() {
        assert_eq!(RenderProgress::Animation.fill(), Fill::Sweep);
        let export = RenderProgress::Export {
            fraction: 0.5,
            headline: String::new(),
            detail: String::new(),
        };
        assert_eq!(part(&export), (0.5, true));
    }

    #[test]
    fn only_running_tasks_are_active() {
        assert!(!RenderProgress::Idle.is_active());
        assert!(!RenderProgress::Flame { done: 5, target: 5, paused: false }.is_active());
        assert!(!RenderProgress::Flame { done: 1, target: 5, paused: true }.is_active());
        assert!(RenderProgress::Flame { done: 1, target: 5, paused: false }.is_active());
        assert!(!RenderProgress::Escape { settled: true, fraction: None }.is_active());
        assert!(RenderProgress::Escape { settled: false, fraction: None }.is_active());
        assert!(RenderProgress::Animation.is_active());
    }
}
