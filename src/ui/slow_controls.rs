//! Controls for settings whose next frame is slow.
//!
//! An egui slider takes its value from where the pointer is, every frame.
//! When a change makes the next frame slow -- Beam Width, an IFS walk's
//! depth, a variation's iteration count -- that frame comes late, the
//! pointer has moved on meanwhile, and the value jumps: the user sees
//! nothing, keeps dragging, and the setting runs to its maximum before
//! the picture has caught up once. Two remedies, for the settings marked
//! slow (`escape::SLOW_PARAMS`, `sim::SLOW_PARAMS`,
//! `variations::SLOW_PARAMS`, and a few config controls):
//! - [`on_release`]: the control moves and shows its value while dragged,
//!   and the value reaches the config when the drag ends (at once for a
//!   click, an arrow key or a typed number);
//! - [`whole_numbers`]: a small whole-number range as a dropdown, one
//!   deliberate value at a time.

/// Run `add` -- a slider, a drag value -- on a copy of `value` that a
/// drag holds in egui's memory, and write it to `value` only when the
/// drag ends or the control changes without one. Returns the control's
/// response and whether `value` changed.
///
/// `id` must be stable for the control across frames. A held value more
/// than a frame old -- the control stopped being drawn mid-drag -- is
/// dropped, never committed.
pub fn on_release<T>(
    ui: &mut egui::Ui,
    id: egui::Id,
    value: &mut T,
    add: impl FnOnce(&mut egui::Ui, &mut T) -> egui::Response,
) -> (egui::Response, bool)
where
    T: Clone + PartialEq + Send + Sync + 'static,
{
    let pass = ui.ctx().cumulative_pass_nr();
    let held: Option<(T, u64)> = ui.data(|d| d.get_temp(id));
    let held = held.filter(|(_, at)| *at + 1 >= pass).map(|(v, _)| v);
    let mut shown = held.clone().unwrap_or_else(|| value.clone());
    let response = add(ui, &mut shown);
    // Held from the press, not only once egui calls it a drag: a slider
    // moves to the pointer on the frame the button goes down.
    if response.dragged() || response.is_pointer_button_down_on() {
        ui.data_mut(|d| d.insert_temp(id, (shown, pass)));
        return (response, false);
    }
    ui.data_mut(|d| d.remove::<(T, u64)>(id));
    let commit = held.is_some() || response.changed();
    if commit && shown != *value {
        *value = shown;
        return (response, true);
    }
    (response, false)
}

/// A whole-number setting from `min` to `max` as a dropdown, labelled
/// `label` and keyed by `id_salt`. Returns whether `value` changed. A
/// stored value outside the range shows clamped and is left alone until
/// a pick.
pub fn whole_numbers(
    ui: &mut egui::Ui,
    id_salt: impl std::hash::Hash,
    label: &str,
    tooltip: Option<&str>,
    value: &mut f32,
    min: i32,
    max: i32,
) -> bool {
    let current = if value.is_finite() { (value.round() as i64).clamp(min as i64, max as i64) as i32 } else { min };
    let mut picked = current;
    ui.horizontal(|ui| {
        ui.label(label);
        let response = egui::ComboBox::from_id_salt(id_salt)
            .selected_text(current.to_string())
            .show_ui(ui, |ui| {
                for n in min..=max {
                    ui.selectable_value(&mut picked, n, n.to_string());
                }
            })
            .response;
        if let Some(t) = tooltip {
            response.on_hover_text(t);
        }
    });
    if picked != current {
        *value = picked as f32;
        return true;
    }
    false
}

/// Whether a slow setting's range is small enough for [`whole_numbers`]:
/// whole-number ends, at most sixteen values.
pub fn fits_a_dropdown(min: f32, max: f32) -> bool {
    min.fract() == 0.0 && max.fract() == 0.0 && max >= min && max - min <= 15.0
}

#[cfg(test)]
mod tests {
    use super::*;

    /// One frame of a 400x200 screen holding a slider over 0..100,
    /// driven by `events`: the slider's rect and whether it committed.
    fn frame(ctx: &egui::Context, events: Vec<egui::Event>, value: &mut f32) -> (egui::Rect, bool) {
        let input = egui::RawInput {
            events,
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(400.0, 200.0))),
            ..Default::default()
        };
        let mut out = (egui::Rect::NOTHING, false);
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let (r, changed) = on_release(ui, egui::Id::new("slow"), value, |ui, v| ui.add(egui::Slider::new(v, 0.0..=100.0)));
                out = (r.rect, changed);
            });
        });
        out
    }

    fn button(pos: egui::Pos2, pressed: bool) -> egui::Event {
        egui::Event::PointerButton { pos, button: egui::PointerButton::Primary, pressed, modifiers: Default::default() }
    }

    /// A drag reaches the value once, when it ends -- not on the press,
    /// not on any frame in between.
    #[test]
    fn a_drag_commits_once_when_it_ends() {
        let ctx = egui::Context::default();
        let mut value = 10.0f32;
        let (rect, _) = frame(&ctx, vec![], &mut value);
        let y = rect.center().y;
        let (start, end) = (egui::pos2(rect.left() + 20.0, y), egui::pos2(rect.left() + rect.width() * 0.6, y));
        let mut commits = 0;
        for events in [
            vec![egui::Event::PointerMoved(start), button(start, true)],
            vec![egui::Event::PointerMoved(egui::pos2((start.x + end.x) / 2.0, y))],
            vec![egui::Event::PointerMoved(end)],
        ] {
            commits += frame(&ctx, events, &mut value).1 as u32;
            assert_eq!(value, 10.0, "the value moved before the drag ended");
        }
        commits += frame(&ctx, vec![button(end, false)], &mut value).1 as u32;
        assert_eq!(commits, 1, "one commit for the drag");
        assert!(value > 20.0, "the release committed where the drag ended: {value}");
    }
}
