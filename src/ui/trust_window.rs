//! The question `curated::trust` asks: a fractal or an animation from the
//! server carries code nobody reviewed -- load it? (docs/projects/
//! curated-content.md, *Trust*; the app holds the load, `app::trust`.)

use crate::curated::trust::{self, Answer};

/// Draw the open question, if any, and record the answer.
pub fn show(ctx: &egui::Context) {
    let Some(q) = trust::question() else { return };
    let mut answer = None;
    egui::Window::new(t!("trust.title"))
        .id(egui::Id::new("trust_question"))
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_max_width(420.0);
            ui.label(t!("trust.intro", name = q.what.clone()));
            ui.add_space(4.0);
            for c in &q.code {
                let kind = c.kind.as_deref().map(kind_label).unwrap_or_default();
                ui.label(format!("  • {}  {}", c.title, if kind.is_empty() { String::new() } else { format!("({kind})") }));
            }
            ui.add_space(4.0);
            ui.weak(t!("trust.explain"));
            ui.add_space(6.0);
            ui.horizontal_wrapped(|ui| {
                if ui.button(t!("trust.load")).on_hover_text(t!("trust.load_tip")).clicked() {
                    answer = Some(Answer::Load);
                }
                if ui.button(t!("trust.always")).on_hover_text(t!("trust.always_tip")).clicked() {
                    answer = Some(Answer::Always);
                }
                if ui.button(t!("trust.no")).clicked() {
                    answer = Some(Answer::No);
                }
            });
        });
    if let Some(a) = answer {
        trust::answer(q.id, a);
    }
}

/// A server kind's label (`formula` -> the Catalog's "Formula").
fn kind_label(wire: &str) -> String {
    match wire {
        "track" => t!("inventory.kind.track").to_string(),
        w => crate::storage::library::Kind::from_wire(w)
            .and_then(crate::inventory::Kind::of_file)
            .map(|k| t!(k.label_key()).to_string())
            .unwrap_or_else(|| w.to_string()),
    }
}
