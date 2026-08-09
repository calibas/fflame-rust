pub mod animation_panel;
pub mod signal_panel;
mod config_dialog;
mod effects_panel;
mod export_panel;
pub mod export_status;
mod font_loader;
mod formatting;
pub mod fractal_browser;
pub mod fractal_gallery;
mod help;
pub mod login_dialog;
pub mod save_online_dialog;
pub mod histogram;
mod compact_menu;
mod menu_bar;
mod menu_context;
mod palette_editor;
mod palette_library;
mod panel_viewer;
mod path_editor;
mod performance;
mod random_generator;
mod scripts_panel;
pub mod response;
mod settings;
mod solid_panel;
mod subflames;
mod target_selector;
mod tone_mapping;
pub mod track_editor;
mod transforms;
mod transform_colors;
mod variations;
mod triangle_editor;
mod undo_history;
mod variation_params;
mod view;
pub mod workspace;
mod palette_generate;
mod script_params;
mod xaos_editor;

pub use export_status::{ExportKind, ExportStatus, UiReporter};
pub use font_loader::ensure_font_for_locale;
pub use menu_context::{MenuActions, MenuState};
pub use palette_editor::PaletteEditor;
pub use response::UiResponse;
pub use response::ApiSaveAction;
pub use response::ApiAnimationSaveAction;
pub use workspace::Workspace;

/// Publish a TextEdit's content for the virtual keyboard overlay (WASM compact mode).
/// Call after any `ui.text_edit_singleline()` or `ui.add(TextEdit::singleline())`.
#[allow(unused_variables)]
pub fn vkb_sync(ui: &egui_dock::egui::Ui, response: &egui_dock::egui::Response, text: &str) {
    vkb_sync_opts(ui, response, text, "text");
}

/// Like `vkb_sync`, but with a field type ("text", "integer", "decimal", "email", "password").
#[allow(unused_variables)]
pub fn vkb_sync_opts(ui: &egui_dock::egui::Ui, response: &egui_dock::egui::Response, text: &str, field_type: &str) {
    vkb_sync_full(ui, response, text, field_type, None, None);
}

/// Full VKB sync with min/max range hints for numeric fields.
#[allow(unused_variables)]
pub fn vkb_sync_full(
    ui: &egui_dock::egui::Ui,
    response: &egui_dock::egui::Response,
    text: &str,
    field_type: &str,
    min: Option<f64>,
    max: Option<f64>,
) {
    #[cfg(target_arch = "wasm32")]
    if response.has_focus() {
        ui.ctx().data_mut(|d| {
            d.insert_temp(egui_dock::egui::Id::new("vkb_editing_text"), text.to_owned());
            d.insert_temp(egui_dock::egui::Id::new("vkb_field_type"), field_type.to_owned());
            if let Some(min) = min {
                d.insert_temp(egui_dock::egui::Id::new("vkb_min"), min);
            } else {
                d.remove_temp::<f64>(egui_dock::egui::Id::new("vkb_min"));
            }
            if let Some(max) = max {
                d.insert_temp(egui_dock::egui::Id::new("vkb_max"), max);
            } else {
                d.remove_temp::<f64>(egui_dock::egui::Id::new("vkb_max"));
            }
        });
    }
}

/// Builder for a DragValue with automatic VKB sync.
/// Display form for an integral value that arrived as `f64`.
///
/// Sign-aware because `as i64` **saturates**: every unsigned value above
/// 2^63 - 1 rendered as 9223372036854775807, one number standing in for
/// the whole upper half of the range. Seeds now use [`VkbU64`] (exact,
/// no float on the path), but `u64` fields like `max_iterations` still
/// come through here, and a display that silently collapses is worse
/// than one that rounds.
///
/// Values above 2^53 remain approximate — that is `f64`, and unavoidable
/// here. Use [`VkbU64`] when every integer has to be exact.
fn fmt_integral(v: f64) -> String {
    if v >= 0.0 {
        format!("{}", v as u64)
    } else {
        format!("{}", v as i64)
    }
}

/// Exact text entry for a `u64` — the whole range, no float anywhere.
///
/// `VkbDragValue` cannot carry a `u64`. egui's `DragValue` works in
/// `f64`, whose 53-bit mantissa stops representing consecutive integers
/// at 9,007,199,254,740,992, and the integer display path additionally
/// went through `as i64`, which saturates — so every value above
/// 2^63 - 1 rendered as that one number.
///
/// That is fine for a pixel count and wrong for a seed. A seed is a
/// shareable artifact addressing a ring of 2^64 (`wasm/README.md`,
/// "Seeds — a ring of 2⁶⁴", which states outright that nothing special
/// happens at 2^53), and only 0.049% of that ring survives an `f64`
/// round-trip. The Random Generator's dice button draws over the full
/// range, so it produced an unrepresentable seed 2047 times out of 2048.
///
/// Deliberately a text field rather than a drag: consecutive seeds are
/// scrambled far apart by design (SplitMix64), so scrubbing walks
/// unrelated flames rather than a gradient. Stepping belongs to the
/// caller's Reroll button, which does exact `u64` arithmetic.
///
/// Invalid input is *held, not discarded* — the caller's value only
/// changes when the text parses, so a half-typed number cannot silently
/// reset the seed to 0.
pub struct VkbU64<'a> {
    value: &'a mut u64,
    id_salt: &'a str,
    desired_width: Option<f32>,
}

impl<'a> VkbU64<'a> {
    pub fn new(value: &'a mut u64, id_salt: &'a str) -> Self {
        Self { value, id_salt, desired_width: None }
    }

    pub fn desired_width(mut self, w: f32) -> Self {
        self.desired_width = Some(w);
        self
    }
}

impl egui_dock::egui::Widget for VkbU64<'_> {
    fn ui(self, ui: &mut egui_dock::egui::Ui) -> egui_dock::egui::Response {
        use egui_dock::egui;

        // The in-progress text lives in egui memory, keyed per field, so
        // typing can pass through states that do not parse ("", "12a")
        // without touching the caller's value. Re-seeded from the value
        // whenever the two have diverged and the field is not focused —
        // that is what makes an external change (Reroll, dice, loading a
        // config) show up.
        let id = ui.make_persistent_id(("vkb_u64", self.id_salt));
        let mut text = ui
            .memory(|m| m.data.get_temp::<String>(id))
            .unwrap_or_else(|| self.value.to_string());

        let focused = ui.memory(|m| m.has_focus(id.with("edit")));
        if !focused && text.parse::<u64>() != Ok(*self.value) {
            text = self.value.to_string();
        }

        let mut edit = egui::TextEdit::singleline(&mut text).id(id.with("edit"));
        if let Some(w) = self.desired_width {
            edit = edit.desired_width(w);
        }
        let response = ui.add(edit);

        if response.changed() {
            // Tolerate separators a person might paste in; reject
            // anything else by leaving the value alone.
            let cleaned: String = text.chars().filter(|c| !matches!(c, ',' | '_' | ' ')).collect();
            if let Ok(v) = cleaned.parse::<u64>() {
                *self.value = v;
            }
        }

        ui.memory_mut(|m| m.data.insert_temp(id, text.clone()));
        vkb_sync_opts(ui, &response, &text, "integer");
        response
    }
}

/// Usage: `ui.add(VkbDragValue::new(&mut val).speed(0.01).range(0..=100))`
/// Entry point only — `new()` returns the generic builder below.
pub struct VkbDragValue<'a> {
    _marker: std::marker::PhantomData<&'a ()>,
}

/// Adapter that lets the Vkb widgets own the edit round-trip.
///
/// egui works in `f64` and writes the value itself, which left the wrapper
/// no chance to apply a precision policy. Callers now hand us their number,
/// we edit a local `f64`, and we decide what lands back in it — see
/// [`crate::config::precision`].
pub struct VkbNum<'a, Num: egui_dock::egui::emath::Numeric> {
    target: &'a mut Num,
    scratch: f64,
}

impl<'a, Num: egui_dock::egui::emath::Numeric> VkbNum<'a, Num> {
    pub fn new(target: &'a mut Num) -> Self {
        let scratch = target.to_f64();
        Self { target, scratch }
    }
    /// Write `v` back to the caller's number.
    fn commit(&mut self, v: f64) {
        // `Numeric::from_f64` is `num as Self` for integers — truncation, not
        // rounding. Stepping 5 up by a hair lands on 5.05 and truncates back
        // to 5 (the Up arrow appearing dead), while stepping down lands on
        // 4.95 and truncates to 4, so Down "worked" by accident.
        let v = if Num::INTEGRAL { v.round() } else { v };
        self.scratch = v;
        *self.target = Num::from_f64(v);
    }
}

impl<'a> VkbDragValue<'a> {
    pub fn new<Num: egui_dock::egui::emath::Numeric>(value: &'a mut Num) -> VkbDragValueOwned<'a, Num> {
        VkbDragValueOwned {
            num: VkbNum::new(value),
            min: None,
            max: None,
            speed: None,
            prefix: None,
            suffix: None,
            min_decimals: None,
            fixed_decimals: None,
        }
    }
}

/// The actual builder (generic over the caller's numeric type).
pub struct VkbDragValueOwned<'a, Num: egui_dock::egui::emath::Numeric> {
    num: VkbNum<'a, Num>,
    min: Option<f64>,
    max: Option<f64>,
    speed: Option<f64>,
    prefix: Option<String>,
    suffix: Option<String>,
    min_decimals: Option<usize>,
    fixed_decimals: Option<usize>,
}

impl<'a, Num: egui_dock::egui::emath::Numeric> VkbDragValueOwned<'a, Num> {
    pub fn speed(mut self, speed: impl Into<f64>) -> Self { self.speed = Some(speed.into()); self }
    pub fn range<R: egui_dock::egui::emath::Numeric>(mut self, range: std::ops::RangeInclusive<R>) -> Self {
        self.min = Some(range.start().to_f64());
        self.max = Some(range.end().to_f64());
        self
    }
    pub fn prefix(mut self, prefix: impl ToString) -> Self { self.prefix = Some(prefix.to_string()); self }
    pub fn suffix(mut self, suffix: impl ToString) -> Self { self.suffix = Some(suffix.to_string()); self }
    pub fn min_decimals(mut self, min_decimals: usize) -> Self { self.min_decimals = Some(min_decimals); self }
    pub fn fixed_decimals(mut self, fixed_decimals: usize) -> Self { self.fixed_decimals = Some(fixed_decimals); self }
}

impl<Num: egui_dock::egui::emath::Numeric> egui_dock::egui::Widget for VkbDragValueOwned<'_, Num> {
    fn ui(mut self, ui: &mut egui_dock::egui::Ui) -> egui_dock::egui::Response {
        use crate::config::precision;
        let is_integer = Num::INTEGRAL;
        let before = self.num.scratch;
        let value_str = if is_integer {
            fmt_integral(before)
        } else {
            precision::fmt_f32(before as f32)
        };

        let mut edited = before;
        let mut dv = egui_dock::egui::DragValue::new(&mut edited);
        if let Some(sp) = self.speed { dv = dv.speed(sp); }
        if let (Some(lo), Some(hi)) = (self.min, self.max) { dv = dv.range(lo..=hi); }
        if let Some(p) = &self.prefix { dv = dv.prefix(p.clone()); }
        if let Some(sx) = &self.suffix { dv = dv.suffix(sx.clone()); }
        if let Some(d) = self.min_decimals { dv = dv.min_decimals(d); }
        if let Some(d) = self.fixed_decimals { dv = dv.fixed_decimals(d); }
        if !is_integer && self.fixed_decimals.is_none() {
            // Show the value's own shortest f32 form rather than a
            // decimal count egui guesses from the range.
            dv = dv
                .custom_formatter(|v, _| precision::fmt_f32(v as f32))
                .custom_parser(|s| s.trim().parse::<f64>().ok());
        }
        let response = dv.ui(ui);

        if edited != before {
            // A DRAG may land anywhere, so quantize it; typed text is taken
            // exactly as entered (that is how you reach a value like
            // 0.93248 that no drag would hit).
            let snapped = if !is_integer && response.dragged() {
                match self.speed.filter(|s| *s > 0.0) {
                    // Derive the quantum from the drag speed the caller chose.
                    Some(sp) => precision::snap_to_step(edited, 0.0, precision::nice_step(sp * 200.0)) as f64,
                    // No speed given and no range to scale from: keep it tidy
                    // by significant figures instead of leaving raw drag noise.
                    None => precision::snap_to_significant(edited, 4) as f64,
                }
            } else {
                edited
            };
            self.num.commit(snapped);
        }

        let field_type = if is_integer { "integer" } else { "decimal" };
        vkb_sync_full(ui, &response, &value_str, field_type, self.min, self.max);
        response
    }
}

/// Builder for a Slider with automatic VKB sync.
/// Usage: `ui.add(VkbSlider::new(&mut val, 0.0..=1.0).text("label").logarithmic(true))`
pub struct VkbSlider<'a> {
    _marker: std::marker::PhantomData<&'a ()>,
}

impl<'a> VkbSlider<'a> {
    pub fn new<Num: egui_dock::egui::emath::Numeric>(
        value: &'a mut Num,
        range: std::ops::RangeInclusive<Num>,
    ) -> VkbSliderOwned<'a, Num> {
        let min = range.start().to_f64();
        let max = range.end().to_f64();
        VkbSliderOwned {
            num: VkbNum::new(value),
            min,
            max,
            text: None,
            logarithmic: false,
            suffix: None,
            show_value: true,
            step: None,
            clamping: None,
            drag_value_speed: None,
        }
    }
}

/// The actual slider builder (generic over the caller's numeric type).
pub struct VkbSliderOwned<'a, Num: egui_dock::egui::emath::Numeric> {
    num: VkbNum<'a, Num>,
    min: f64,
    max: f64,
    text: Option<egui_dock::egui::WidgetText>,
    logarithmic: bool,
    suffix: Option<String>,
    show_value: bool,
    step: Option<f64>,
    clamping: Option<egui_dock::egui::SliderClamping>,
    drag_value_speed: Option<f64>,
}

impl<'a, Num: egui_dock::egui::emath::Numeric> VkbSliderOwned<'a, Num> {
    pub fn text(mut self, text: impl Into<egui_dock::egui::WidgetText>) -> Self { self.text = Some(text.into()); self }
    pub fn logarithmic(mut self, logarithmic: bool) -> Self { self.logarithmic = logarithmic; self }
    pub fn suffix(mut self, suffix: impl ToString) -> Self { self.suffix = Some(suffix.to_string()); self }
    pub fn show_value(mut self, show_value: bool) -> Self { self.show_value = show_value; self }
    pub fn step_by(mut self, step: f64) -> Self { self.step = Some(step); self }
    pub fn clamping(mut self, clamping: egui_dock::egui::SliderClamping) -> Self { self.clamping = Some(clamping); self }
    pub fn drag_value_speed(mut self, speed: impl Into<f64>) -> Self { self.drag_value_speed = Some(speed.into()); self }
}

impl<Num: egui_dock::egui::emath::Numeric> egui_dock::egui::Widget for VkbSliderOwned<'_, Num> {
    fn ui(mut self, ui: &mut egui_dock::egui::Ui) -> egui_dock::egui::Response {
        use crate::config::precision;
        let is_integer = Num::INTEGRAL;
        let before = self.num.scratch;
        let value_str = if is_integer {
            fmt_integral(before)
        } else {
            precision::fmt_f32(before as f32)
        };

        let mut edited = before;
        let mut sl = egui_dock::egui::Slider::new(&mut edited, self.min..=self.max)
            .logarithmic(self.logarithmic)
            .show_value(self.show_value);
        if let Some(t) = self.text.clone() { sl = sl.text(t); }
        if let Some(sx) = &self.suffix { sl = sl.suffix(sx.clone()); }
        // Deliberately NOT `sl.step_by(self.step)`: egui rounds the value it
        // is HANDED, so a stored value off the step grid gets rewritten just
        // by drawing the slider — no interaction needed. The step is a
        // drag-feel preference, so it's applied below, on drag only.
        if let Some(c) = self.clamping { sl = sl.clamping(c); }
        if let Some(sp) = self.drag_value_speed { sl = sl.drag_value_speed(sp); }
        if !is_integer {
            // egui otherwise derives the displayed decimals from the range
            // (2..5 depending on span) and rounds the stored value to them,
            // which is what put a floor of ~0.01 on fine-grained params.
            sl = sl
                .custom_formatter(|v, _| precision::fmt_f32(v as f32))
                .custom_parser(|s| s.trim().parse::<f64>().ok());
        } else {
            // We hand egui an f64, so it can't see that the caller's number
            // is integral — `Slider::new` applies this itself, but only when
            // `Num::INTEGRAL`. Without it an integer param renders with two
            // decimals (egui derives them from the drag speed) and keyboard
            // arrows step by the slider's pixel gradient (~0.05 over a 1..12
            // range) instead of by 1.
            sl = sl.integer();
        }
        let response = sl.ui(ui);

        if edited != before {
            // Dragging the handle snaps to a round decimal — a slider is a
            // couple of hundred pixels wide, so arbitrary values are neither
            // reachable on purpose nor worth storing. Typing is exact.
            let snapped = if !is_integer && response.dragged() {
                if let Some(step) = self.step {
                    // Caller asked for a specific drag granularity.
                    precision::snap_to_step(edited, self.min, step) as f64
                } else if self.logarithmic {
                    // A linear step is wrong on a log scale — see
                    // precision::snap_to_significant.
                    precision::snap_to_significant(edited, 4) as f64
                } else {
                    let step = precision::nice_step(self.max - self.min);
                    precision::snap_to_step(edited, self.min, step) as f64
                }
            } else {
                edited
            };
            self.num.commit(snapped);
        }

        let field_type = if is_integer { "integer" } else { "decimal" };
        vkb_sync_full(ui, &response, &value_str, field_type, Some(self.min), Some(self.max));
        response
    }
}

#[cfg(test)]
mod u64_field_tests {
    use super::fmt_integral;

    /// The reported bug: a seed is a `u64`, the drag widget is `f64`.
    ///
    /// egui's `DragValue` works in `f64` throughout, and `f64` stops
    /// representing consecutive integers at 2^53. Only 0.049% of the
    /// 2^64 seed ring survives the round trip, and the Random
    /// Generator's dice button draws over the whole ring — so it
    /// produced an unrepresentable seed 2047 times in 2048.
    #[test]
    fn f64_cannot_carry_a_large_seed_which_is_why_vkbu64_exists() {
        let exact = (1u64 << 53) - 1;
        assert_eq!(exact as f64 as u64, exact, "2^53-1 is the last exact one");

        for seed in [(1u64 << 53) + 1, 12_345_678_901_234_567, u64::MAX - 1] {
            assert_ne!(
                seed as f64 as u64,
                seed,
                "{seed} must NOT survive f64 — if it does, this test is wrong, not the widget"
            );
        }
    }

    /// `as i64` saturates, so the whole upper half of `u64` displayed as
    /// one number. Values there are still approximate (that is `f64`),
    /// but they must at least be distinguishable and in the right range.
    #[test]
    fn the_integer_display_no_longer_collapses_above_i64_max() {
        let big = (1u64 << 63) as f64;
        let bigger = ((1u64 << 63) + (1u64 << 40)) as f64;

        assert_eq!(big as i64, i64::MAX, "the old path saturated here");
        assert_eq!(bigger as i64, i64::MAX, "...and here, to the same value");

        assert_ne!(
            fmt_integral(big),
            fmt_integral(bigger),
            "two different seeds must not render as the same number"
        );
        assert_eq!(fmt_integral(big), "9223372036854775808");
    }

    #[test]
    fn negative_values_still_render_signed() {
        assert_eq!(fmt_integral(-7.0), "-7");
        assert_eq!(fmt_integral(0.0), "0");
        assert_eq!(fmt_integral(42.0), "42");
    }

    /// What `VkbU64` accepts. Parsing is the whole widget: it holds the
    /// caller's value unchanged unless the text parses, so a half-typed
    /// number cannot reset a seed to 0.
    #[test]
    fn seed_text_parses_across_the_whole_ring_and_rejects_junk() {
        let clean = |t: &str| -> Option<u64> {
            t.chars().filter(|c| !matches!(c, ',' | '_' | ' ')).collect::<String>().parse().ok()
        };

        assert_eq!(clean("0"), Some(0));
        assert_eq!(clean("18446744073709551615"), Some(u64::MAX));
        assert_eq!(clean("18_446_744_073_709_551_615"), Some(u64::MAX));
        assert_eq!(clean("9,007,199,254,740,993"), Some(9_007_199_254_740_993));

        assert_eq!(clean(""), None);
        assert_eq!(clean("12a"), None);
        assert_eq!(clean("-1"), None, "the ring's -1 is entered as its u64 position");
        assert_eq!(clean("18446744073709551616"), None, "2^64 does not fit");
    }

    /// Drive the real widget through real egui frames — no window.
    ///
    /// The tests above check arithmetic; this checks the egui
    /// integration, which is where the widget could still be wrong: it
    /// keeps in-progress text in `Memory`, re-seeds that text from the
    /// caller's value when the two diverge and the field is not focused,
    /// and must not write back on a frame where nothing was typed. A bug
    /// in any of those corrupts the seed on display — the failure being
    /// fixed.
    ///
    /// Note this alone does NOT distinguish the new widget from the old:
    /// `VkbDragValue` only writes back when its value changes, so with no
    /// input events it also leaves a large seed intact. The old widget
    /// corrupted on INTERACTION — see the typing test below, which is the
    /// one with teeth.
    #[test]
    fn a_large_seed_survives_being_displayed() {
        use egui_dock::egui;

        for original in [u64::MAX, (1u64 << 63) + 12_345, (1u64 << 53) + 1] {
            let ctx = egui::Context::default();
            let mut seed = original;

            // Several frames: the first populates Memory, later ones take
            // the "text already present" path where a careless re-seed or
            // an unconditional write-back would show up.
            for _ in 0..3 {
                let _ = ctx.run(Default::default(), |ctx| {
                    egui::CentralPanel::default().show(ctx, |ui| {
                        ui.add(super::VkbU64::new(&mut seed, "test_seed"));
                    });
                });
            }

            assert_eq!(
                seed, original,
                "displaying {original} changed it to {seed} — the old f64 widget's bug"
            );
        }
    }

    /// Type a seed that `f64` cannot hold, and read back what landed.
    ///
    /// This is the test with teeth. The old widget survived mere display
    /// (it only writes back when its value changes), so the failure only
    /// appears once someone edits — which is exactly how it presented:
    /// the seed looked right until you touched it.
    ///
    /// Focus, then send the digits as text events, then commit with
    /// Enter, the way a person does it.
    #[test]
    fn typing_a_seed_past_2_pow_53_lands_exactly() {
        use egui_dock::egui;

        const TYPED: u64 = 9_007_199_254_740_993; // 2^53 + 1
        assert_ne!(TYPED as f64 as u64, TYPED, "premise: f64 cannot hold it");

        let ctx = egui::Context::default();
        let mut seed: u64 = 1;

        // Frame 1: draw, and take focus.
        let _ = ctx.run(Default::default(), |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                let r = ui.add(super::VkbU64::new(&mut seed, "typed_seed"));
                r.request_focus();
            });
        });

        // Frame 2: select-all then type the digits, then Enter.
        let mut input = egui::RawInput::default();
        input.events.push(egui::Event::Key {
            key: egui::Key::A,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::COMMAND,
        });
        input.events.push(egui::Event::Text(TYPED.to_string()));
        input.events.push(egui::Event::Key {
            key: egui::Key::Enter,
            physical_key: None,
            pressed: true,
            repeat: false,
            modifiers: egui::Modifiers::NONE,
        });
        let _ = ctx.run(input, |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                ui.add(super::VkbU64::new(&mut seed, "typed_seed"));
            });
        });

        assert_eq!(
            seed, TYPED,
            "typed {TYPED} but got {seed} — an f64 round-trip in the widget"
        );
    }

    /// An external change (Reroll, the dice button, loading a config)
    /// must reach the field, not be masked by the cached text.
    #[test]
    fn an_externally_changed_seed_reaches_the_field() {
        use egui_dock::egui;

        let ctx = egui::Context::default();
        let mut seed: u64 = 1;

        for step in 0..3 {
            if step == 1 {
                // What Reroll does: exact u64 arithmetic, outside the widget.
                seed = u64::MAX - 3;
            }
            let _ = ctx.run(Default::default(), |ctx| {
                egui::CentralPanel::default().show(ctx, |ui| {
                    ui.add(super::VkbU64::new(&mut seed, "reroll_seed"));
                });
            });
        }

        assert_eq!(seed, u64::MAX - 3, "the external write must survive");
    }
}

#[cfg(test)]
mod slider_tests {
    /// A step is a drag-feel preference, not a constraint on what a config
    /// may hold — so merely DRAWING a slider must leave the stored value
    /// alone. Regression: `egui::Slider::step_by` rounds the value it is
    /// handed, so a `.flame` import carrying `perspective 0.0273` was
    /// silently rewritten to 0.03 on any frame the View panel was open,
    /// with no interaction at all.
    #[test]
    fn drawing_a_stepped_slider_leaves_off_grid_values_alone() {
        let mut value = 0.0273_f32;
        egui_dock::egui::__run_test_ui(|ui| {
            ui.add(super::VkbSlider::new(&mut value, 0.0..=10.0).step_by(0.01));
        });
        assert_eq!(value, 0.0273);
    }

    /// Integer write-back must round, not truncate. `Numeric::from_f64` is
    /// `num as Self` for integers, so a step that lands a hair short of the
    /// next whole number fell back to the current one — which is why the Up
    /// arrow on Iterations/Map Count appeared dead while Down worked.
    #[test]
    fn integer_write_back_rounds_rather_than_truncating() {
        let mut value = 5i32;
        super::VkbNum::new(&mut value).commit(5.999_999_9);
        assert_eq!(value, 6);

        let mut down = 5i32;
        super::VkbNum::new(&mut down).commit(4.000_000_1);
        assert_eq!(down, 4);
    }

    /// The same guarantee without a caller-supplied step.
    #[test]
    fn drawing_an_unstepped_slider_leaves_values_alone() {
        let mut value = 0.006_f32;
        egui_dock::egui::__run_test_ui(|ui| {
            ui.add(super::VkbSlider::new(&mut value, 0.0..=0.2));
        });
        assert_eq!(value, 0.006);
    }
}

/// Information about a clicked pixel in PathMap mode
/// Includes pixel coordinates, fractal space coordinates, path data, and a 5x5 color preview
#[derive(Clone, Debug)]
pub struct PathClickInfo {
    /// View space pixel coordinates (where user clicked)
    pub click_pixel: (u32, u32),
    /// Actual pixel with valid path data (may differ if click was empty)
    pub found_pixel: (u32, u32),
    /// Fractal space coordinates of the found pixel
    pub fractal_coords: (f32, f32),
    /// Distance from click to found pixel (0 if exact match)
    pub search_distance: f32,
    /// Path data at the found pixel
    pub path_entry: crate::renderer::PathEntry,
    /// 5x5 color preview centered on found pixel (RGBA, row-major)
    /// May be smaller if near edges
    pub color_preview: Vec<[u8; 4]>,
    /// Dimensions of the color preview (width, height) - usually 5x5
    pub preview_size: (u32, u32),
}

/// API notification for toast-style feedback overlay
pub struct ApiNotification {
    pub message: String,
    pub is_error: bool,
    pub created_at: web_time::Instant,
}

impl ApiNotification {
    const DURATION_SECS: f32 = 4.0;
    const FADE_START_SECS: f32 = 3.0;
}

/// State for cloud palette browsing in Palette Library (the caller's
/// bookmarked entries from `/api/users/me/palettes`).
pub struct CloudPaletteState {
    pub palettes: Vec<crate::api::types::LibraryPaletteEntry>,
    pub fetched: bool,
    pub loading: bool,
    pub error: Option<String>,
    pub list_result: std::sync::Arc<std::sync::Mutex<Option<Result<Vec<crate::api::types::LibraryPaletteEntry>, String>>>>,
    pub deleting: bool,
    /// On success, carries the content hash of the removed entry.
    pub delete_result: std::sync::Arc<std::sync::Mutex<Option<Result<String, String>>>>,
    /// Notification from palette operations (message, is_error)
    pub notification: Option<(String, bool)>,
}

impl Default for CloudPaletteState {
    fn default() -> Self {
        Self {
            palettes: Vec::new(),
            fetched: false,
            loading: false,
            error: None,
            list_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
            deleting: false,
            delete_result: std::sync::Arc::new(std::sync::Mutex::new(None)),
            notification: None,
        }
    }
}

use egui_wgpu::wgpu::*;
use egui_wgpu::{Renderer as EguiRenderer, RendererOptions};
use egui_winit::State as EguiWinitState;
use winit::{event::WindowEvent, window::Window};

pub struct EguiLayer {
    state: EguiWinitState,
    pub ctx: egui_dock::egui::Context,
    renderer: EguiRenderer,
    config_json_buffer: String,
    palette_editor: PaletteEditor,

    /// Texture IDs the egui renderer holds a FULL image for.
    ///
    /// egui-wgpu panics ("tried to update a texture that has not been
    /// allocated yet") if a partial update arrives for a texture it never
    /// received in full. That happens when the atlas is rebuilt out from
    /// under the renderer — notably on the web, where the browser's
    /// scale factor changes during startup and egui rasterizes its font
    /// atlas per pixels_per_point.
    allocated_textures: std::collections::HashSet<egui_dock::egui::TextureId>,

    // Fractal texture ID (registered from renderer's texture)
    fractal_texture_id: Option<egui_dock::egui::TextureId>,
    // Track registered texture dimensions to detect resize
    fractal_texture_width: u32,
    fractal_texture_height: u32,

    // Selected preset config (from FractalBrowser or other sources)
    selected_preset_config: Option<crate::config::FractalConfig>,

    // Subflames panel: pending "Load from file" request. Carries the
    // target subflame index. Set by the Subflames panel, taken by
    // App's UI handler each frame and turned into a file-dialog or
    // browser-picker invocation.
    load_subflame_into: Option<usize>,

    // Animation export settings
    animation_export_settings: animation_panel::AnimationExportSettings,

    // Export Animation panel state (Phase 5)
    export_panel_state: animation_panel::ExportPanelState,

    // Track editor state
    track_editor_state: track_editor::TrackEditorState,

    // PathMap mode: clicked pixel info (includes path, coordinates, color preview)
    clicked_pixel: Option<(u32, u32)>,
    path_click_info: Option<PathClickInfo>,
    close_path_overlay: bool,

    // Path editor state
    path_editor_state: path_editor::PathEditorState,

    // Random generator panel state
    random_generator_panel: Option<random_generator::RandomGeneratorPanel>,
    scripts_panel: Option<scripts_panel::ScriptsPanel>,
    generated_flame: Option<crate::scene::randomize::RandomFlame>,
    generated_batch: Option<Vec<crate::config::FractalConfig>>,
    /// Config produced by the Scripts panel, applied as one undo step.
    script_generated: Option<crate::config::FractalConfig>,
    script_animation: Option<crate::animation::Animation>,

    // Fractal browser panel state
    fractal_browser_panel: Option<fractal_browser::FractalBrowserPanel>,

    // API: flame metadata loaded from Online tab (passed through to UiResponse)
    loaded_api_flame_id: Option<String>,
    loaded_api_flame_is_public: Option<bool>,
    loaded_api_flame_user_id: Option<String>,
    loaded_api_flame_animation_count: u32,
    loaded_api_flame_animations: Vec<crate::api::types::AnimationSummary>,

    // API: notification toast
    api_notification: Option<ApiNotification>,

    // API: save dialog state (docked panel)
    save_online_dialog_state: save_online_dialog::SaveOnlineDialogState,

    /// API notification from browser panel (e.g. delete result)
    api_browser_notification: Option<(String, bool)>,

    // Login dialog state
    login_dialog_state: login_dialog::LoginDialogState,

    // Cloud palette state (for Palette Library panel)
    cloud_palette_state: CloudPaletteState,

    // API connectivity state (set by App each frame before render_ui)
    pub(crate) api_connectivity: crate::api::ApiConnectivity,

    // Histogram for density visualization (levels now in ConfigManager)
    density_histogram: crate::renderer::DensityHistogram,

    // Xaos editor state
    xaos_editor_state: xaos_editor::XaosEditorState,

    // Signal panel state
    pub(crate) signal_panel_state: signal_panel::SignalPanelState,

    // Touch gesture tracking (for multi-touch on web)
    touch_tracker: panel_viewer::TouchTracker,

    // Compact mode state
    /// Whether compact (mobile) layout is active
    compact_mode: bool,
    /// Last time any input was received (for menu button fade)
    last_input_time: web_time::Instant,

    /// Tab bar height of the FractalViewport leaf node (from previous frame).
    /// Used to inflate the fractal texture so it covers the tab bar seamlessly.
    viewport_tab_bar_height: f32,

    // WASM clipboard bridge
    #[cfg(target_arch = "wasm32")]
    web_clipboard: crate::web_clipboard::WebClipboard,
    // WASM text input agent (virtual keyboard)
    #[cfg(target_arch = "wasm32")]
    web_text_agent: crate::web_text_agent::WebTextAgent,
    #[cfg(target_arch = "wasm32")]
    vkb_defocus_pending: bool,
}

impl EguiLayer {
    /// Whether this browser reports a touchscreen. Drives the virtual
    /// keyboard gate: touch devices need the overlay whether or not the
    /// layout is compact. `maxTouchPoints` is 0 on mouse-only desktops,
    /// including ones where the OS merely supports touch drivers.
    #[cfg(target_arch = "wasm32")]
    fn device_has_touch() -> bool {
        web_sys::window()
            .map(|w| w.navigator().max_touch_points() > 0)
            .unwrap_or(false)
    }

    /// Reinitialize GPU-dependent resources after surface recreation.
    /// Preserves all UI state (panels, editors, settings, etc).
    pub fn reinit_gpu_resources(&mut self, window: &Window, device: &Device, queue: &Queue, format: TextureFormat) {
        let viewport_id = self.ctx.viewport_id();
        self.state = EguiWinitState::new(self.ctx.clone(), viewport_id, window, None, None, None);
        self.renderer = EguiRenderer::new(
            device,
            format,
            RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: false,
                predictable_texture_filtering: false,
            },
        );
        // Clear stale texture registrations from old surface
        self.allocated_textures.clear();
        self.fractal_texture_id = None;
        self.fractal_texture_width = 0;
        self.fractal_texture_height = 0;

        // Pre-seed the font atlas in the new renderer. After reinit, egui's
        // Context still thinks the font atlas exists and may send partial
        // updates (pos: Some). The new Renderer has no textures, so a partial
        // update would panic. Pre-seeding the full atlas here prevents that.
        let font_image = self.ctx.fonts(|f| f.image());
        self.renderer.update_texture(
            device,
            queue,
            egui::TextureId::Managed(0),
            &egui::epaint::ImageDelta {
                image: egui::ImageData::Color(std::sync::Arc::new(font_image)),
                pos: None,
                options: egui::TextureOptions::LINEAR,
            },
        );
        self.allocated_textures.insert(egui::TextureId::Managed(0));
    }

    pub fn new(window: &Window, device: &Device, format: TextureFormat) -> Self {
        let ctx = egui_dock::egui::Context::default();

        // egui 0.34 requires Context::run() to be called at least once before
        // fonts can be accessed. Run a dummy frame to initialize the font system.
        let _ = ctx.run_ui(egui_dock::egui::RawInput::default(), |_ctx| {});

        // Initialize fonts with Noto Sans (better Unicode coverage than Ubuntu-Light)
        font_loader::initialize_default_fonts(&ctx);

        // Configure style to disable window shadows
        ctx.set_visuals(egui_dock::egui::Visuals {
            window_shadow: egui_dock::egui::epaint::Shadow::NONE,
            ..egui_dock::egui::Visuals::dark()
        });

        let viewport_id = ctx.viewport_id();
        let state = EguiWinitState::new(ctx.clone(), viewport_id, window, None, None, None);
        let renderer = EguiRenderer::new(
            device,
            format,
            RendererOptions {
                msaa_samples: 1,
                depth_stencil_format: None,
                dithering: false,
                predictable_texture_filtering: false,
            },
        );
        Self {
            state,
            ctx,
            renderer,
            config_json_buffer: String::new(),
            palette_editor: PaletteEditor::new(),
            fractal_texture_id: None,
            fractal_texture_width: 0,
            fractal_texture_height: 0,
            selected_preset_config: None,
            load_subflame_into: None,
            animation_export_settings: animation_panel::AnimationExportSettings::default(),
            export_panel_state: animation_panel::ExportPanelState::default(),
            track_editor_state: track_editor::TrackEditorState::default(),
            clicked_pixel: None,
            path_click_info: None,
            close_path_overlay: false,
            path_editor_state: path_editor::PathEditorState::new(),
            random_generator_panel: None,
            allocated_textures: std::collections::HashSet::new(),
            scripts_panel: None,
            generated_flame: None,
            generated_batch: None,
            script_generated: None,
            script_animation: None,
            fractal_browser_panel: None,
            loaded_api_flame_id: None,
            loaded_api_flame_is_public: None,
            loaded_api_flame_user_id: None,
            loaded_api_flame_animation_count: 0,
            loaded_api_flame_animations: Vec::new(),
            api_notification: None,
            save_online_dialog_state: save_online_dialog::SaveOnlineDialogState::default(),
            api_browser_notification: None,
            login_dialog_state: login_dialog::LoginDialogState::default(),
            cloud_palette_state: CloudPaletteState::default(),
            api_connectivity: crate::api::ApiConnectivity::Unknown,
            density_histogram: crate::renderer::DensityHistogram::default(),
            xaos_editor_state: xaos_editor::XaosEditorState::default(),
            signal_panel_state: signal_panel::SignalPanelState::new(),
            touch_tracker: panel_viewer::TouchTracker::default(),
            compact_mode: false,
            last_input_time: web_time::Instant::now(),
            viewport_tab_bar_height: 0.0,
            #[cfg(target_arch = "wasm32")]
            web_clipboard: crate::web_clipboard::WebClipboard::install(),
            #[cfg(target_arch = "wasm32")]
            web_text_agent: crate::web_text_agent::WebTextAgent::install(),
            #[cfg(target_arch = "wasm32")]
            vkb_defocus_pending: false,
        }
    }

    /// Enable or disable compact (mobile) mode
    pub fn set_compact_mode(&mut self, enabled: bool) {
        self.compact_mode = enabled;
    }

    /// Mutable access to login dialog state (for auto-login on startup)
    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn login_dialog_state_mut(&mut self) -> &mut login_dialog::LoginDialogState {
        &mut self.login_dialog_state
    }

    pub fn handle_event(&mut self, event: &WindowEvent, window: &Window) -> bool {
        let response = self.state.on_window_event(window, event);

        // For mouse events, we need to be more aggressive about detecting UI interaction
        // The issue: is_using_pointer() can return false even when over panels
        // Better approach: Check if pointer is over ANY layer (not just interacting with widgets)
        match event {
            WindowEvent::MouseInput { .. } | WindowEvent::CursorMoved { .. } | WindowEvent::MouseWheel { .. } => {
                // Check multiple egui states to detect UI interaction
                let is_using = self.ctx.egui_is_using_pointer();
                let wants_pointer = self.ctx.egui_wants_pointer_input();
                let is_pointer_over_area = self.ctx.is_pointer_over_egui();

                // Consume if egui wants the pointer OR pointer is over any UI area
                let consumed = response.consumed && (is_using || wants_pointer || is_pointer_over_area);

                // DEBUG: Log pointer state for cursor moves (only when dragging might be active)
                // if matches!(event, WindowEvent::CursorMoved { .. }) && consumed {
                //     log::debug!("CursorMoved over UI: consumed={}, is_using={}, wants_pointer={}, is_over_area={}",
                //         response.consumed, is_using, wants_pointer, is_pointer_over_area);
                // }

                consumed
            }
            _ => response.consumed
        }
    }

    pub fn update_palette_editor(&mut self, palette: crate::scene::palette::Palette) {
        self.palette_editor.current_palette = palette;
    }

    /// Get the clicked pixel coordinates (for PathMap mode)
    /// Returns Some((x, y)) if user clicked on the fractal viewport
    pub fn take_clicked_pixel(&mut self) -> Option<(u32, u32)> {
        self.clicked_pixel.take()
    }

    /// Update the cached path click info for display
    pub fn set_path_click_info(&mut self, info: Option<PathClickInfo>) {
        self.path_click_info = info;
    }

    /// Get reference to current path click info
    pub fn path_click_info(&self) -> Option<&PathClickInfo> {
        self.path_click_info.as_ref()
    }

    /// Check if the path overlay should be closed and reset the flag
    pub fn take_close_path_overlay(&mut self) -> bool {
        let close = self.close_path_overlay;
        self.close_path_overlay = false;
        close
    }

    /// Show an API notification toast
    pub fn show_api_notification(&mut self, message: &str, is_error: bool) {
        self.api_notification = Some(ApiNotification {
            message: message.to_string(),
            is_error,
            created_at: web_time::Instant::now(),
        });
    }

    /// Request the Online tab to refresh its flame list
    pub fn request_online_refresh(&mut self) {
        if let Some(ref mut panel) = self.fractal_browser_panel {
            panel.request_online_refresh();
        }
    }

    /// Clear cloud-related state (called on sign-out or session expiry)
    pub fn clear_cloud_state(&mut self) {
        self.cloud_palette_state = CloudPaletteState::default();
        if let Some(ref mut panel) = self.fractal_browser_panel {
            panel.clear_online_data();
        }
    }

    /// Render API notification toast overlay (bottom-center)
    fn render_api_notification(&mut self, ctx: &egui::Context) {
        let notification = match &self.api_notification {
            Some(n) => n,
            None => return,
        };

        let elapsed = notification.created_at.elapsed().as_secs_f32();
        if elapsed >= ApiNotification::DURATION_SECS {
            self.api_notification = None;
            return;
        }

        // Calculate opacity (fade out in last second)
        let alpha = if elapsed >= ApiNotification::FADE_START_SECS {
            let fade_progress = (elapsed - ApiNotification::FADE_START_SECS)
                / (ApiNotification::DURATION_SECS - ApiNotification::FADE_START_SECS);
            1.0 - fade_progress
        } else {
            1.0
        };

        let bg_color = if notification.is_error {
            egui::Color32::from_rgba_unmultiplied(180, 40, 40, (alpha * 230.0) as u8)
        } else {
            egui::Color32::from_rgba_unmultiplied(40, 140, 40, (alpha * 230.0) as u8)
        };

        let text_color = egui::Color32::from_rgba_unmultiplied(255, 255, 255, (alpha * 255.0) as u8);

        egui::Area::new(egui::Id::new("api_notification"))
            .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -60.0))
            .order(egui::Order::Foreground)
            .interactable(false)
            .show(ctx, |ui| {
                egui::Frame::new()
                    .fill(bg_color)
                    .corner_radius(6.0)
                    .inner_margin(egui::Margin::symmetric(16, 10))
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(&notification.message).color(text_color).size(14.0));
                    });
            });

        // Request repaint for animation
        ctx.request_repaint();
    }

    /// Register the renderer's fractal texture with egui for display
    /// Call this when the texture size changes or on first frame
    pub fn register_fractal_texture(&mut self, device: &Device, texture_view: &TextureView, width: u32, height: u32) {
        // Check if we need to re-register (size changed or first time)
        let needs_reregister = self.fractal_texture_id.is_none()
            || self.fractal_texture_width != width
            || self.fractal_texture_height != height;

        if needs_reregister {
            log::info!("Re-registering fractal texture: {}x{} → {}x{}",
                self.fractal_texture_width, self.fractal_texture_height, width, height);

            // Unregister old texture if it exists
            if let Some(old_id) = self.fractal_texture_id.take() {
                self.renderer.free_texture(&old_id);
            }

            self.fractal_texture_width = width;
            self.fractal_texture_height = height;
        }

        // ALWAYS update the texture view, even if size didn't change
        // This is critical for minimize/restore - the texture view can become stale
        // even if the size is the same
        if let Some(old_id) = self.fractal_texture_id.take() {
            if !needs_reregister {
                self.renderer.free_texture(&old_id);
            }
        }

        let texture_id = self.renderer.register_native_texture(
            device,
            texture_view,
            FilterMode::Linear,
        );
        self.fractal_texture_id = Some(texture_id);
    }

    /// Get the egui TextureId for the fractal texture (for displaying in UI)
    pub fn fractal_texture_id(&self) -> Option<egui_dock::egui::TextureId> {
        self.fractal_texture_id
    }

    pub fn render_ui(
        &mut self,
        device: &egui_wgpu::wgpu::Device,
        queue: &egui_wgpu::wgpu::Queue,
        encoder: &mut egui_wgpu::wgpu::CommandEncoder,
        target_view: &egui_wgpu::wgpu::TextureView,
        window: &Window,
        window_size: winit::dpi::PhysicalSize<u32>,
        metrics: &crate::util::PerformanceMetrics,
        config_manager: &mut crate::config::ConfigManager,
        flame_renderer: Option<&mut crate::renderer::compute_kernel::FlameRenderer>,
        flame: &mut crate::scene::transforms::Flame,
        palette_library: &mut crate::scene::palette::PaletteLibrary,
        preset_library: &crate::scene::presets::PresetLibrary,
        animation_controller: &mut crate::animation::AnimationController,
        paused: &mut bool,
        view_subflame_in_isolation: &mut bool,
        quit_requested: &mut bool,
        can_undo: bool,
        can_redo: bool,
        workspace: &mut workspace::Workspace,
        export_width: &mut u32,
        export_height: &mut u32,
        use_custom_export_size: &mut bool,
        png_export_premultiplied: &mut bool,
        png_export_supersample: &mut bool,
        export_status: &export_status::ExportStatus,
        fullscreen_mode: bool,
        audio_manager: &mut crate::audio::AudioManager,
        audio_player: &mut crate::audio::AudioPlayer,
        audio_capture: &mut crate::audio::AudioCapture,
        signal_manager: &mut crate::signal::SignalManager,
        signal_names: &[String],
        api_state: &crate::app::ApiContentState,
        current_user_id: Option<&str>,
        fly_mode_active: bool,
        variation_catalog: Option<&crate::storage::variation_catalog::CachedCatalog>,
        script_cloud: &crate::app::script_cloud::ScriptCloudState,
        effect_catalog: Option<&crate::storage::effect_catalog::CachedEffectCatalog>,
        signed_in: bool,
    ) -> UiResponse {
        // Sync compact mode from workspace (handles layout switches from menus)
        let is_compact = workspace.is_compact();
        if is_compact != self.compact_mode {
            self.set_compact_mode(is_compact);
            // Persist the change to system settings
            config_manager.system_settings_mut().compact_mode = Some(is_compact);
            let _ = config_manager.system_settings().save();
        }

        // Wider, always-visible scrollbars in compact mode (touch-friendly).
        // Applied every frame since egui_dock may reset styles.
        self.ctx.global_style_mut(|style| {
            if self.compact_mode {
                style.spacing.scroll = egui::style::ScrollStyle {
                    bar_width: 7.0,
                    floating_width: 7.0,
                    floating_allocated_width:7.0,
                    bar_inner_margin: 2.0,
                    bar_outer_margin: 2.0,
                    // foreground_color: true,
                    ..egui::style::ScrollStyle::floating()
                };
                // Finger-sized hit targets. egui's desktop defaults
                // (18px interact height, 4x1 button padding) are mouse
                // sizes; the accepted floor for touch is ~40px+. This
                // grows the CLICKABLE area — checkboxes, slider
                // handles, drag-value boxes, combo rows — without
                // scaling fonts, so layouts stay recognizable.
                style.spacing.interact_size = egui::vec2(40.0, 28.0);
                style.spacing.button_padding = egui::vec2(8.0, 5.0);
                style.spacing.item_spacing = egui::vec2(8.0, 6.0);
                style.spacing.slider_rail_height = 10.0;
            } else {
                style.spacing.scroll = egui::style::ScrollStyle::floating();
                let d = egui::Spacing::default();
                style.spacing.interact_size = d.interact_size;
                style.spacing.button_padding = d.button_padding;
                style.spacing.item_spacing = d.item_spacing;
                style.spacing.slider_rail_height = d.slider_rail_height;
            }
            // Label drag-to-select disabled globally — opt in per-widget with
            // `.selectable(true)` on any specific Label that should support
            // copy/paste. egui 0.33 → 0.34 introduced a regression where a
            // single click on a Label can leave LabelSelectionState's
            // `is_dragging` flag stuck (subsequent hovers select text as if
            // the mouse button were held). The widget-level code in
            // text_selection/label_text_selection.rs is identical between
            // 0.33 and 0.34.2; the actual regression is upstream of it in
            // the new interaction-stack flag computation (context.rs ~1424).
            // No known upstream fix yet. Mirrors how egui's own tooltip
            // containers handle this — see tooltip.rs:158 in egui 0.34.2.
            // Runs every frame because egui_dock can reset styles.
            style.interaction.selectable_labels = false;
        });

        #[cfg(not(target_arch = "wasm32"))]
        let raw_input = self.state.take_egui_input(window);

        #[cfg(target_arch = "wasm32")]
        let mut raw_input = self.state.take_egui_input(window);

        // Track input activity for compact menu fade
        if self.compact_mode && !raw_input.events.is_empty() {
            self.last_input_time = web_time::Instant::now();
        }

        // Clear stale VKB editing text from previous frame (WASM only)
        #[cfg(target_arch = "wasm32")]
        self.ctx.data_mut(|d| {
            d.remove_temp::<String>(egui_dock::egui::Id::new("vkb_editing_text"));
        });

        // Inject clipboard paste events from the browser (WASM only)
        #[cfg(target_arch = "wasm32")]
        if let Some(text) = self.web_clipboard.take_paste() {
            raw_input.events.push(egui_dock::egui::Event::Paste(text));
        }

        // Virtual keyboard: if user submitted a value, inject it (defocus happens post-frame)
        #[cfg(target_arch = "wasm32")]
        if let Some(text) = self.web_text_agent.take_submitted() {
            // Select all + replace with submitted text (TextEdit still has focus)
            raw_input.events.push(egui_dock::egui::Event::Key {
                key: egui_dock::egui::Key::A,
                physical_key: None,
                pressed: true,
                repeat: false,
                modifiers: egui_dock::egui::Modifiers::COMMAND,
            });
            raw_input.events.push(egui_dock::egui::Event::Text(text));
            self.vkb_defocus_pending = true;
        }

        // Virtual keyboard: if user cancelled, just defocus the field (no value change)
        #[cfg(target_arch = "wasm32")]
        if self.web_text_agent.take_cancelled() {
            self.vkb_defocus_pending = true;
        }

        // Desktop: poll auto-login result (runs even when Login panel is not visible)
        #[cfg(not(target_arch = "wasm32"))]
        {
            if self.login_dialog_state.loading {
                if let Some(success) = login_dialog::poll_auto_login_result(
                    &mut self.login_dialog_state,
                    config_manager,
                ) {
                    self.api_notification = Some(ApiNotification {
                        message: format!("{} ({})", t!("auth.signed_in"), success.email),
                        is_error: false,
                        created_at: web_time::Instant::now(),
                    });
                }
            }
        }

        // Note: Config change tracking now handled by ConfigManager.get_pending_actions()
        // Only non-config actions tracked here (file I/O, palette library, transforms, etc.)

        let mut add_transform = false;
        let mut delete_transform = None;
        let mut clone_transform = None;
        let mut add_linked_transform = false;
        let mut delete_linked_transform = None;
        let mut clone_linked_transform = None;
        let mut add_final_transform = false;
        let mut delete_final_transform = None;
        let mut clone_final_transform = None;
        let mut attachment_edit: Option<crate::ui::response::AttachmentEdit> = None;

        // Config import/export
        let mut config_export_json = None;
        let mut config_import_json = None;
        let mut config_save_file = false;
        let mut config_load_file = false;
        let mut flame_xml_import_file = false;
        let mut flame_xml_export_file = false;
        let mut new_flame_requested = false;
        let mut random_flame_requested = false;

        // Palette library management
        let mut palette_export_json = None;
        let mut palette_save_file = None;
        let mut palette_save_to_library = None;
        let mut palette_delete_from_library = None;
        let mut palette_import_json = None;
        let mut palette_load_file = false;

        // Undo/redo
        let mut undo_requested = false;
        let mut redo_requested = false;

        // Export
        let mut png_export_with_background = false;
        let mut png_export_transparent = false;
        // let mut png_export_requested = false;

        // Panel open requests
        let mut open_palette_editor = false;
        let mut open_palette_library = false;
        let mut open_config_dialog = false;
        let mut open_triangle_editor = false;
        let mut open_preset_library = false;
        let mut open_random_generator = false;

        // Fractal viewport size tracking
        let mut fractal_viewport_size = None;

        // Free-fly camera: drag delta accumulator + toggle flag
        // populated by the viewport panel; consumed by App on response.
        let mut fly_mouse_drag: Option<(f32, f32)> = None;
        let mut fly_mode_toggle_requested = false;

        // File browser
        let mut file_browser_open_requested = false;

        // Sign out requested (from account panel or 401 detection)
        let mut sign_out_requested = false;

        // Animation export
        let mut animation_export_requested: Option<animation_panel::AnimationExportSettings> = None;
        let mut animation_seek_changed = false;
        let mut animation_seek_drag_stopped = false;

        // Animation API save action
        let mut api_animation_save_action = response::ApiAnimationSaveAction::None;
        let mut open_save_online_dialog = false;
        let mut load_api_animation_id: Option<String> = None;
        let mut clear_variation_cache_requested = false;
        let mut variation_update_requested: Vec<String> = Vec::new();
        let mut script_cloud_request: Option<crate::app::script_cloud::ScriptCloudRequest> = None;

        // Path filters
        let mut path_filters_changed: Option<Vec<crate::gpu::buffers::GpuPathFilter>> = None;

        // Audio file loading
        let mut load_audio_file = false;

        // Signal file load/save
        let mut load_signal_file = false;
        let mut save_signal_file: Option<String> = None;

        // Menu actions and state
        let mut menu_actions = MenuActions::default();
        let has_animation_tracks = animation_controller.animation
            .as_ref()
            .map_or(false, |a| !a.tracks.is_empty());

        // Keep the Save Online dialog state in sync with ApiContentState so that
        // successful saves immediately update the displayed IDs and button states
        // without requiring the dialog to be closed and reopened.
        self.save_online_dialog_state.api_flame_id = api_state.flame_id.clone();
        self.save_online_dialog_state.api_animation_id = api_state.animation_id.clone();
        self.save_online_dialog_state.animation_count = api_state.animation_count;
        self.save_online_dialog_state.has_animation_tracks = has_animation_tracks;
        self.save_online_dialog_state.flame_owned = api_state.flame_owned_by(current_user_id);
        self.save_online_dialog_state.animation_owned = api_state.flame_owned_by(current_user_id);
        let menu_state = MenuState {
            can_undo,
            can_redo,
            is_paused: *paused,
            render_mode_2d: config_manager.config().render_mode == crate::scene::transforms::RenderMode::TwoD,
            online_mode: config_manager.system_settings().online_mode,
            has_api_flame_id: api_state.flame_id.is_some(),
            api_flame_id: api_state.flame_id.clone(),
            api_flame_is_public: api_state.flame_is_public,
            has_animation_tracks,
            animation_playing: animation_controller.is_playing(),
            api_animation_id: api_state.animation_id.clone(),
            animation_count: api_state.animation_count,
            flame_owned: api_state.flame_owned_by(current_user_id),
            animation_owned: api_state.flame_owned_by(current_user_id),
            flame_name: config_manager.config().flame.name.clone(),
            auth_email: read_auth_email(config_manager),
            api_connectivity: self.api_connectivity,
            fly_mode_active,
        };

        // Log ConfigManager state at start of UI render
        // log::debug!("render_ui start: ConfigManager has exposure={:.3}, gamma={:.3}",
        //     config_manager.config().exposure, config_manager.config().gamma);

        // Get fractal texture ID before the closure (avoid borrow conflict)
        let fractal_texture_id = self.fractal_texture_id();
        // Capture animation time before closure to avoid borrow conflict with animation_controller
        let anim_current_time = animation_controller.current_time;

        // egui 0.34 deprecates `Context::run` + `CentralPanel::show` + `Panel::show` +
        // `DockArea::show(ctx, ...)` in favor of an eframe::App-based flow that hands
        // out a `&mut Ui` directly. We drive winit + wgpu ourselves, so the legacy
        // ctx-based path is still the supported way to feed egui from outside eframe.
        // Suppress for the duration of this closure rather than restructuring the
        // whole UI tree.
        #[allow(deprecated)]
        let full_output = self.ctx.run(raw_input, |ctx| {
            // Debug: Print font info once after fonts are available
            static FONT_DEBUG_DONE: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);
            if !FONT_DEBUG_DONE.swap(true, std::sync::atomic::Ordering::Relaxed) {
                font_loader::debug_font_info(ctx);
            }

            // Fullscreen mode: render only the fractal, no UI panels
            if fullscreen_mode {
                // Render fractal texture fullscreen
                egui::CentralPanel::default()
                    .frame(egui::Frame::NONE)
                    .show(ctx, |ui| {
                        if let Some(texture_id) = fractal_texture_id {
                            let available = ui.available_size();
                            ui.add(egui::Image::new(egui::load::SizedTexture::new(
                                texture_id,
                                available,
                            )));
                            // Track viewport size for renderer resize
                            fractal_viewport_size = Some((available.x as u32, available.y as u32));
                        }
                    });

                // Show exit hint at bottom
                egui::Area::new(egui::Id::new("fullscreen_hint"))
                    .anchor(egui::Align2::CENTER_BOTTOM, egui::vec2(0.0, -20.0))
                    .interactable(false)
                    .show(ctx, |ui| {
                        ui.add(egui::Label::new(
                            egui::RichText::new("Press F or Esc to exit fullscreen")
                                .color(egui::Color32::from_rgba_unmultiplied(255, 255, 255, 180))
                                .size(14.0)
                        ).selectable(false));
                    });

                return;
            }

            // Compute auth state before struct init (avoids borrow conflict)
            let is_signed_in = config_manager.system_settings().is_signed_in();

            // Normal mode: render menu bar (desktop) or floating button (compact)
            if self.compact_mode {
                compact_menu::render_compact_menu(
                    ctx,
                    workspace,
                    &mut menu_actions,
                    &menu_state,
                    &mut self.save_online_dialog_state,
                );
            } else {
                menu_bar::render_menu_bar(
                    ctx,
                    workspace,
                    &mut menu_actions,
                    &menu_state,
                    &mut self.save_online_dialog_state,
                    #[cfg(not(target_arch = "wasm32"))]
                    window,
                );
            }

            // Menu-bar Fly Mode button shares the viewport's toggle path.
            if menu_actions.fly_mode_toggle {
                fly_mode_toggle_requested = true;
            }

            // All windows are now dockable panels (see Windows menu)
            // Fullscreen docking system with Fractal Viewport as a panel
            // Fractal renders as a panel in the dock, can be arranged with other panels

            // Render fullscreen DockArea - manages all panels including FractalViewport
            // egui automatically handles input routing for panels
            egui_dock::DockArea::new(&mut workspace.dock_state)
                .id(egui::Id::new("main_dock_area"))
                .show(ctx, &mut panel_viewer::PanelViewer {
                    context: panel_viewer::PanelContext {
                        variation_catalog,
                        // Core state
                        config_manager,
                        flame,

                        // Libraries
                        preset_library,
                        palette_library,

                        // Renderer
                        flame_renderer: flame_renderer.as_ref().map(|v| &**v),

                        // Animation controller
                        animation_controller,

                        // Action flags
                        add_transform: &mut add_transform,
                        delete_transform: &mut delete_transform,
                        clone_transform: &mut clone_transform,
                        add_linked_transform: &mut add_linked_transform,
                        delete_linked_transform: &mut delete_linked_transform,
                        clone_linked_transform: &mut clone_linked_transform,
                        add_final_transform: &mut add_final_transform,
                        delete_final_transform: &mut delete_final_transform,
                        clone_final_transform: &mut clone_final_transform,
                        attachment_edit: &mut attachment_edit,
                        undo_requested: &mut undo_requested,
                        redo_requested: &mut redo_requested,
                        open_palette_editor: &mut open_palette_editor,
                        open_palette_library: &mut open_palette_library,
                        open_triangle_editor: &mut open_triangle_editor,
                        open_preset_library: &mut open_preset_library,
                        open_random_generator: &mut open_random_generator,

                        // UI state
                        paused,
                        view_subflame_in_isolation,
                        png_export_with_background: &mut png_export_with_background,
                        png_export_transparent: &mut png_export_transparent,
                        export_width,
                        export_height,
                        use_custom_export_size,
                        png_export_premultiplied,
                        png_export_supersample,
                        max_export_dimension: device.limits().max_texture_dimension_2d,
                        palette_editor: &mut self.palette_editor,
                        palette_export_json: &mut palette_export_json,
                        palette_save_file: &mut palette_save_file,
                        palette_save_to_library: &mut palette_save_to_library,
                        palette_delete_from_library: &mut palette_delete_from_library,
                        palette_import_json: &mut palette_import_json,
                        palette_load_file: &mut palette_load_file,

                        // Performance metrics
                        metrics,
                        window_size,
                        window,

                        // Fractal texture for display
                        fractal_texture_id,
                        fractal_viewport_size: &mut fractal_viewport_size,
                        fly_mouse_drag: &mut fly_mouse_drag,
                        fly_mode_toggle_requested: &mut fly_mode_toggle_requested,
                        fly_mode_active,
                        viewport_tab_bar_height: self.viewport_tab_bar_height,

                        // Config dialog state
                        config_json_buffer: &mut self.config_json_buffer,
                        config_export_json: &mut config_export_json,
                        config_import_json: &mut config_import_json,
                        config_save_file: &mut config_save_file,
                        config_load_file: &mut config_load_file,
                        flame_xml_import_file: &mut flame_xml_import_file,
                        flame_xml_export_file: &mut flame_xml_export_file,
                        open_config_dialog: &mut open_config_dialog,

                        // Selected preset config (from FractalBrowser or other sources)
                        selected_preset_config: &mut self.selected_preset_config,

                        // Subflames panel: "Load from file" target index
                        load_subflame_into: &mut self.load_subflame_into,

                        // API flame ID loaded from Online tab
                        loaded_api_flame_id: &mut self.loaded_api_flame_id,
                        loaded_api_flame_is_public: &mut self.loaded_api_flame_is_public,
                        loaded_api_flame_user_id: &mut self.loaded_api_flame_user_id,
                        loaded_api_flame_animation_count: &mut self.loaded_api_flame_animation_count,
                        loaded_api_flame_animations: &mut self.loaded_api_flame_animations,

                        // API notification from browser panel
                        api_notification: &mut self.api_browser_notification,

                        // Login dialog state
                        login_dialog_state: &mut self.login_dialog_state,

                        // Save Online dialog state
                        save_online_dialog_state: &mut self.save_online_dialog_state,

                        // Sign out requested flag
                        sign_out_requested: &mut sign_out_requested,

                        // Cloud palette state
                        cloud_palette_state: &mut self.cloud_palette_state,

                        // File browser open request (shared by FractalBrowser)
                        file_browser_open_requested: &mut file_browser_open_requested,

                        // Animation export settings
                        animation_export_settings: &mut self.animation_export_settings,
                        animation_export_requested: &mut animation_export_requested,

                        // Whether ANY export is running (disables export buttons;
                        // live progress is shown by the global overlay, not panels)
                        export_active: export_status.active,

                        // Export Animation panel state (Phase 5)
                        export_panel_state: &mut self.export_panel_state,

                        // Track editor state
                        track_editor_state: &mut self.track_editor_state,

                        // Animation seek changed flag
                        animation_seek_changed: &mut animation_seek_changed,

                        // Animation scrubber drag stopped - for reset accumulation
                        animation_seek_drag_stopped: &mut animation_seek_drag_stopped,

                        // PathMap mode: clicked pixel and path info
                        hovered_pixel: &mut self.clicked_pixel,
                        path_click_info: &self.path_click_info,
                        close_path_overlay: &mut self.close_path_overlay,

                        // Path editor state
                        path_editor_state: &mut self.path_editor_state,
                        path_filters_changed: &mut path_filters_changed,

                        // Random generator panel state
                        random_generator_panel: &mut self.random_generator_panel,
                        scripts_panel: &mut self.scripts_panel,
                        script_generated: &mut self.script_generated,
                        script_animation: &mut self.script_animation,
                        generated_flame: &mut self.generated_flame,
                        generated_batch: &mut self.generated_batch,

                        // Fractal browser panel state
                        fractal_browser_panel: &mut self.fractal_browser_panel,

                        // Histogram for density visualization (levels now in ConfigManager)
                        density_histogram: &self.density_histogram,

                        // Xaos editor state
                        xaos_editor_state: &mut self.xaos_editor_state,

                        // Signal panel state
                        audio_manager,
                        audio_player,
                        audio_capture,
                        signal_panel_state: &mut self.signal_panel_state,
                        signal_manager,
                        current_time: anim_current_time,
                        load_audio_file: &mut load_audio_file,
                        load_signal_file: &mut load_signal_file,
                        save_signal_file: &mut save_signal_file,

                        // API animation state
                        api_flame_id: &api_state.flame_id,
                        api_animation_id: &api_state.animation_id,
                        flame_animations: &api_state.flame_animations,
                        is_signed_in,
                        api_animation_save_action: &mut api_animation_save_action,
                        open_save_online_dialog: &mut open_save_online_dialog,
                        load_api_animation_id: &mut load_api_animation_id,
                        clear_variation_cache_requested: &mut clear_variation_cache_requested,
                        variation_update_requested: &mut variation_update_requested,
                        script_cloud,
                        effect_catalog,
                        script_cloud_request: &mut script_cloud_request,
                        signed_in,
                        compact_mode: self.compact_mode,
                    },
                    touch_tracker: &mut self.touch_tracker,
                });

            // Hide the FractalViewport's tab bar seamlessly.
            // The GPU renders the fractal texture taller (body + tab bar height). The body
            // shows the bottom portion via UV offset. Here we draw the top portion over the
            // tab bar and block input so the hidden tab buttons can't be clicked.
            if let Some(tab_path) = workspace.dock_state.find_tab(&workspace::PanelType::FractalViewport) {
                let node_index = tab_path.node;
                if let Some(leaf) = workspace.dock_state.main_surface()[node_index].get_leaf() {
                    let tab_bar_h = leaf.viewport.min.y - leaf.rect.min.y;
                    // Store for next frame so render_fractal_viewport can inflate texture size
                    self.viewport_tab_bar_height = tab_bar_h;

                    if tab_bar_h > 0.0 {
                        let tab_bar_rect = egui::Rect::from_min_max(
                            leaf.rect.min,
                            egui::pos2(leaf.rect.max.x, leaf.viewport.min.y),
                        );
                        let bg = config_manager.active_config().background_color;
                        let color = egui::Color32::from_rgb(
                            (bg[0] * 255.0) as u8,
                            (bg[1] * 255.0) as u8,
                            (bg[2] * 255.0) as u8,
                        );
                        let texture_id = fractal_texture_id;
                        let full_height = leaf.rect.height();
                        // Draw fractal texture over the tab bar and block clicks on
                        // hidden tab buttons. Order::Background sits above the dock's
                        // own Background content (tab bar buttons get hidden) but
                        // below floating windows on Order::Middle — keeps panels like
                        // Help drawn on top of the cover instead of behind it.
                        // Full inflated panel rect/size — the cover sits at the
                        // top of the leaf, so for pan/zoom math the relevant
                        // size is the body+cover combined (i.e. the leaf's full
                        // rect). Using these here makes drag and scroll behave
                        // identically whether the mouse is in the body or the
                        // cover strip — no scale discontinuity at the seam.
                        let leaf_rect = leaf.rect;
                        let leaf_size = leaf_rect.size();
                        egui::Area::new(egui::Id::new("viewport_tab_cover"))
                            .fixed_pos(tab_bar_rect.min)
                            .order(egui::Order::Background)
                            .interactable(true)
                            .show(ctx, |ui| {
                                let (rect, response) = ui.allocate_exact_size(
                                    tab_bar_rect.size(),
                                    egui::Sense::click_and_drag(),
                                );
                                if let Some(tid) = texture_id {
                                    // Draw the top slice of the inflated texture (matches the
                                    // UV offset used in render_fractal_viewport for the body)
                                    let uv_bottom = tab_bar_h / full_height;
                                    let uv = egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, uv_bottom),
                                    );
                                    ui.painter().image(tid, rect, uv, egui::Color32::WHITE);
                                } else {
                                    ui.painter().rect_filled(rect, 0.0, color);
                                }

                                // Forward pan/zoom input so the cover strip is
                                // not a "dead zone" above the body. The
                                // FractalViewport body inside the dock handles
                                // input the same way; here we use the leaf's
                                // full rect/size so scaling matches between the
                                // two regions.
                                if response.dragged_by(egui::PointerButton::Primary) {
                                    // Pan, or look (pitch/yaw) when fly mode
                                    // is active or Alt is held — same
                                    // fly_mouse_drag channel the viewport
                                    // body uses.
                                    let alt = ui.input(|i| i.modifiers.alt);
                                    if fly_mode_active || alt {
                                        let d = response.drag_delta();
                                        let prev = fly_mouse_drag.unwrap_or((0.0, 0.0));
                                        fly_mouse_drag = Some((prev.0 + d.x, prev.1 + d.y));
                                    } else {
                                        panel_viewer::pan_fractal_view(
                                            config_manager,
                                            response.drag_delta(),
                                            leaf_size,
                                        );
                                    }
                                }
                                if response.hovered() {
                                    let scroll_delta = ui.input(|i| i.smooth_scroll_delta.y);
                                    if scroll_delta.abs() > 0.1 {
                                        panel_viewer::zoom_fractal_view(
                                            config_manager,
                                            scroll_delta,
                                            response.hover_pos(),
                                            leaf_rect,
                                            leaf_size,
                                            // Fly mode: zoom to center, not cursor
                                            !fly_mode_active,
                                        );
                                    }
                                }
                            });
                    }
                }

                // Eject any tabs that were docked into the viewport's leaf via drag-and-drop.
                // egui_dock has no per-leaf docking restriction, so we undo it after the fact.
                if let Some(leaf) = workspace.dock_state.main_surface()[node_index].get_leaf() {
                    let extra_tabs: Vec<egui_dock::TabIndex> = leaf.tabs()
                        .iter()
                        .enumerate()
                        .filter(|(_, tab)| !matches!(tab, workspace::PanelType::FractalViewport))
                        .map(|(i, _)| egui_dock::TabIndex(i))
                        .collect();
                    if !extra_tabs.is_empty() {
                        // Find a non-viewport leaf to receive ejected tabs
                        let target = workspace.dock_state.main_surface()
                            .iter()
                            .enumerate()
                            .find(|(i, node)| egui_dock::NodeIndex(*i) != node_index && node.is_leaf())
                            .map(|(i, _)| egui_dock::NodeIndex(i));

                        let mut removed = Vec::new();
                        for tab_index in extra_tabs.into_iter().rev() {
                            if let Some(tab) = workspace.dock_state.main_surface_mut().remove_tab(
                                (node_index, tab_index)
                            ) {
                                removed.push(tab);
                            }
                        }
                        if let Some(target_idx) = target {
                            for tab in removed {
                                let count = workspace.dock_state.main_surface()[target_idx]
                                    .get_leaf()
                                    .map(|l| l.tabs().len())
                                    .unwrap_or(0);
                                workspace.dock_state.main_surface_mut()[target_idx]
                                    .insert_tab(egui_dock::TabIndex(count), tab);
                            }
                        }
                    }
                }
            }

            // Show palette editor dialogs (fixed mode warning, overwrite/delete confirmations)
            palette_editor::render_palette_dialogs(
                ctx,
                &mut self.palette_editor,
                config_manager,
                &mut palette_save_to_library,
                &mut palette_delete_from_library,
            );

            // Show Export Animation panel (Phase 5)
            if let Some(export_settings) = animation_panel::render_export_panel(
                ctx,
                animation_controller,
                &mut self.animation_export_settings,
                export_status.active,
                &mut self.export_panel_state,
                #[cfg(not(target_arch = "wasm32"))]
                window,
            ) {
                animation_export_requested = Some(export_settings);
            }

            // Show Track Editor panel (unified Add/Edit track panel)
            track_editor::render_track_editor_panel(
                ctx,
                animation_controller,
                &mut self.track_editor_state,
                config_manager.active_config(),
                animation_controller.current_time,
                signal_names,
            );

            // Note: quit_requested is now handled in app.rs event loop for graceful shutdown
        });

        // API notification toast + export progress overlay (rendered outside
        // ctx.run to avoid borrow conflicts with self). The overlay sits just
        // above the toast so a terminal "Saved …" toast can show beneath the
        // final bar without overlapping.
        {
            let ctx = self.ctx.clone();
            self.render_api_notification(&ctx);
            export_status::render_export_overlay(&ctx, export_status);
        }

        // Open Save Online dialog from animation panel (if requested)
        if open_save_online_dialog {
            self.save_online_dialog_state.open(
                &config_manager.config().flame.name,
                api_state.flame_id.clone(),
                api_state.animation_id.clone(),
                api_state.flame_is_public,
                has_animation_tracks,
                api_state.animation_count,
                api_state.flame_owned_by(current_user_id),
                api_state.flame_owned_by(current_user_id),
            );
            if self.compact_mode {
                workspace.open_compact_panel(workspace::PanelType::SaveOnlineDialog, &self.ctx);
            } else {
                workspace.open_floating_panel(workspace::PanelType::SaveOnlineDialog, &self.ctx);
            }
        }

        // Poll save dialog actions from the docked panel
        let api_save_dialog_action = self.save_online_dialog_state.take_flame_action();
        let api_animation_dialog_action = self.save_online_dialog_state.take_animation_action();

        // Close save dialog panel if requested (Cancel or after Save)
        if self.save_online_dialog_state.close_requested {
            self.save_online_dialog_state.close_requested = false;
            if let Some(tab_path) = workspace.dock_state.find_tab(&workspace::PanelType::SaveOnlineDialog) {
                workspace.dock_state.remove_tab(tab_path);
            }
        }

        // Process browser notifications (e.g. delete results) through the toast system
        if let Some((message, is_error)) = self.api_browser_notification.take() {
            self.show_api_notification(&message, is_error);
        }

        // Log egui repaint requests for performance investigation
        let needs_repaint = self.ctx.has_requested_repaint();

        // Write copied text to browser clipboard (WASM only)
        #[cfg(target_arch = "wasm32")]
        for cmd in &full_output.platform_output.commands {
            if let egui_dock::egui::OutputCommand::CopyText(text) = cmd {
                crate::web_clipboard::WebClipboard::copy_text(text);
            }
        }

        // Virtual keyboard: post-frame handling. Gated on the device
        // being touch-capable, NOT on compact mode — a tablet wide
        // enough to skip compact (>=600 logical px), or a phone whose
        // user tapped "Desktop view" once (the choice persists), still
        // has no hardware keyboard. Before this, both had no way to
        // type into any field at all. On a mouse-driven desktop
        // browser, max_touch_points is 0 and nothing changes.
        #[cfg(target_arch = "wasm32")]
        if self.compact_mode || Self::device_has_touch() {
            // If we just submitted, defocus now (after egui processed the text events)
            if self.vkb_defocus_pending {
                self.vkb_defocus_pending = false;
                self.ctx.memory_mut(|mem| {
                    if let Some(id) = mem.focused() {
                        mem.surrender_focus(id);
                    }
                });
            } else {
                // Open overlay when egui wants keyboard input
                let wants_keyboard = self.ctx.wants_keyboard_input();
                if wants_keyboard {
                    let (editing_text, field_type, min, max) = self.ctx.data_mut(|d| {
                        let text = d.get_temp::<String>(egui_dock::egui::Id::new("vkb_editing_text"));
                        let ftype = d.get_temp::<String>(egui_dock::egui::Id::new("vkb_field_type"))
                            .unwrap_or_else(|| "text".to_owned());
                        let min = d.get_temp::<f64>(egui_dock::egui::Id::new("vkb_min"));
                        let max = d.get_temp::<f64>(egui_dock::egui::Id::new("vkb_max"));
                        (text, ftype, min, max)
                    });
                    // Only open when a vkb_sync call published the
                    // field's current text. The old fallback guessed
                    // "decimal, empty" for un-synced widgets — and the
                    // submit path injects Ctrl+A + the typed text, so
                    // tapping any un-synced field offered a numeric
                    // keypad prefilled empty and WIPED the field on
                    // submit (the script editor lost whole scripts to
                    // this). No keyboard is strictly better than a
                    // destructive one; the fix for an un-synced field
                    // is a vkb_sync call at its site.
                    if let Some(text) = editing_text {
                        self.web_text_agent.open(&field_type, &text, min, max, false);
                    }
                }
            }
        }

        self.state
            .handle_platform_output(window, full_output.platform_output);

        let tris = self
            .ctx
            .tessellate(full_output.shapes, full_output.pixels_per_point);

        let screen_descriptor = egui_wgpu::ScreenDescriptor {
            size_in_pixels: [window_size.width, window_size.height],
            pixels_per_point: full_output.pixels_per_point,
        };

        for (id, image_delta) in &full_output.textures_delta.set {
            // A partial update (pos: Some) is a patch into an image the
            // renderer is assumed to already hold. If it doesn't, egui-wgpu
            // panics — so repair the gap instead of crashing.
            if image_delta.pos.is_some() && !self.allocated_textures.contains(id) {
                if *id == egui::TextureId::Managed(0) {
                    // The font atlas: we can rebuild it in full from the
                    // context, so seed that and let the patch apply on top.
                    let font_image = self.ctx.fonts(|f| f.image());
                    self.renderer.update_texture(
                        device,
                        queue,
                        *id,
                        &egui::epaint::ImageDelta {
                            image: egui::ImageData::Color(std::sync::Arc::new(font_image)),
                            pos: None,
                            options: image_delta.options,
                        },
                    );
                    self.allocated_textures.insert(*id);
                } else {
                    // Any other managed texture (user images) — we have no
                    // way to reconstruct the base, so skip the patch. It
                    // will be re-sent in full when its owner next changes
                    // it; a stale texture beats a panic.
                    log::warn!(
                        "skipping partial update for unallocated texture {:?}",
                        id
                    );
                    continue;
                }
            }

            self.renderer
                .update_texture(device, queue, *id, image_delta);
            if image_delta.pos.is_none() {
                self.allocated_textures.insert(*id);
            }
        }

        self.renderer
            .update_buffers(device, queue, encoder, &tris, &screen_descriptor);

        {
            let rpass = encoder.begin_render_pass(&egui_wgpu::wgpu::RenderPassDescriptor {
                label: Some("egui render pass"),
                color_attachments: &[Some(egui_wgpu::wgpu::RenderPassColorAttachment {
                    view: target_view,
                    resolve_target: None,
                    ops: egui_wgpu::wgpu::Operations {
                        load: egui_wgpu::wgpu::LoadOp::Load, // Load existing content (flame rendering)
                        store: egui_wgpu::wgpu::StoreOp::Store,
                    },
                    depth_slice: None,
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
                multiview_mask: None,
            });

            // forget_lifetime() converts RenderPass<'a> to RenderPass<'static>
            // This is the official wgpu API for satisfying egui-wgpu's 'static requirement
            let mut rpass = rpass.forget_lifetime();

            self.renderer
                .render(&mut rpass, &tris, &screen_descriptor);
        } // Render pass is dropped here

        for id in &full_output.textures_delta.free {
            self.renderer.free_texture(id);
            self.allocated_textures.remove(id);
        }

        // Handle View menu actions BEFORE syncing flame (so changes take effect this frame)
        use crate::config::ConfigPath;
        if menu_actions.view.reset_view {
            let _ = config_manager.update_param(ConfigPath::Zoom, 1.0.into());
            let _ = config_manager.update_param(ConfigPath::Pan, (0.0, 0.0).into());
            let _ = config_manager.update_param(ConfigPath::Rotation, 0.0.into());
        }

        if menu_actions.view.zoom_in {
            let current_zoom = config_manager.active_config().zoom;
            let _ = config_manager.update_param(ConfigPath::Zoom, (current_zoom * 1.2).into());
        }

        if menu_actions.view.zoom_out {
            let current_zoom = config_manager.active_config().zoom;
            let _ = config_manager.update_param(ConfigPath::Zoom, (current_zoom / 1.2).into());
        }

        if menu_actions.view.set_mode_2d {
            let _ = config_manager.update_param(
                ConfigPath::RenderMode,
                crate::scene::transforms::RenderMode::TwoD.into()
            );
        }

        if menu_actions.view.set_mode_3d {
            let _ = config_manager.update_param(
                ConfigPath::RenderMode,
                crate::scene::transforms::RenderMode::ThreeD.into()
            );
        }

        // Sync flame from ConfigManager AFTER UI updates
        *flame = config_manager.active_config().flame.clone();

        // Extract menu actions into individual flags for backward compatibility
        config_load_file |= menu_actions.file.load_config;
        config_save_file |= menu_actions.file.save_config;
        flame_xml_import_file |= menu_actions.file.import_flame_xml;
        flame_xml_export_file |= menu_actions.file.export_flame_xml;
        new_flame_requested |= menu_actions.file.new_flame;
        random_flame_requested |= menu_actions.file.random_flame;
        open_preset_library |= menu_actions.file.open_preset_library;
        if menu_actions.file.export_png {
            png_export_with_background = true;
        }
        if menu_actions.file.export_png_transparent {
            png_export_transparent = true;
        }
        *quit_requested |= menu_actions.file.quit;

        let api_save_action = if let Some(action) = api_save_dialog_action {
            action
        } else {
            response::ApiSaveAction::None
        };

        // Merge animation action from dialog (overrides panel action if set)
        if let Some(action) = api_animation_dialog_action {
            api_animation_save_action = action;
        }

        // Handle sign out action (from menu bar or account panel)
        if menu_actions.file.sign_out || sign_out_requested {
            // Clear auth from SystemSettings
            {
                let settings = config_manager.system_settings_mut();
                settings.auth_email = None;
                // On desktop, clear token and persist; on WASM, cookies handle auth
                #[cfg(not(target_arch = "wasm32"))]
                {
                    settings.auth_token = None;
                    settings.saved_credentials = None;
                    let _ = settings.save();
                }
            }

            // On WASM, call logout endpoint to clear server cookie (fire-and-forget)
            #[cfg(target_arch = "wasm32")]
            {
                let base_url = crate::api::API_BASE_URL.to_string();
                wasm_bindgen_futures::spawn_local(async move {
                    let _ = crate::api::client::api_post_logout(&base_url).await;
                });
            }

            // Clear cloud palette state
            self.cloud_palette_state = CloudPaletteState::default();

            // Clear online tab in fractal browser
            if let Some(ref mut panel) = self.fractal_browser_panel {
                panel.clear_online_data();
            }

            // Show notification (different message for session expiry vs user sign-out)
            let is_session_expired = sign_out_requested && !menu_actions.file.sign_out;
            let message = if is_session_expired {
                t!("auth.session_expired").to_string()
            } else {
                t!("auth.signed_out_success").to_string()
            };
            self.api_notification = Some(ApiNotification {
                message,
                is_error: is_session_expired,
                created_at: web_time::Instant::now(),
            });
        }

        // Combine undo/redo from both menu and panels (OR to not override panel buttons)
        undo_requested |= menu_actions.edit.undo;
        redo_requested |= menu_actions.edit.redo;

        // Handle animation transport actions (compact menu). Same
        // semantics as the Space shortcut: pause keeps position, play
        // needs tracks, stop rewinds (the FSM handles the exit +
        // seek-to-start on its next update).
        if menu_actions.animation.play_pause {
            let has_tracks = animation_controller
                .animation
                .as_ref()
                .is_some_and(|a| !a.tracks.is_empty());
            if animation_controller.is_playing() {
                animation_controller.pause();
            } else if has_tracks {
                animation_controller.play();
            }
        }
        if menu_actions.animation.stop {
            animation_controller.stop();
        }

        // Handle Rendering menu actions
        if menu_actions.rendering.pause_toggle {
            *paused = !*paused;
        }

        if menu_actions.rendering.reset_accumulation {
            let _ = config_manager.request_reset();
        }

        if let Some(ipt) = menu_actions.rendering.set_iterations_per_thread {
            let _ = config_manager.update_system_setting(
                crate::config::ConfigPath::SystemIterationsPerThread,
                ipt.into()
            );
        }

        if menu_actions.rendering.reset_to_defaults {
            reset_rendering_to_defaults(config_manager);
            *paused = false; // Resume rendering after reset
        }

        // Take selected preset config (reset to None after returning)
        let selected_preset_config = self.selected_preset_config.take();

        // Take pending subflame load-from-file request
        let load_subflame_into = self.load_subflame_into.take();

        // Take API flame metadata (reset after returning)
        let loaded_api_flame_id = self.loaded_api_flame_id.take();
        let loaded_api_flame_is_public = self.loaded_api_flame_is_public.take();
        let loaded_api_flame_user_id = self.loaded_api_flame_user_id.take();
        let loaded_api_flame_animation_count = self.loaded_api_flame_animation_count;
        self.loaded_api_flame_animation_count = 0;
        let loaded_api_flame_animations = std::mem::take(&mut self.loaded_api_flame_animations);

        // Take generated flame from random generator panel
        let generated_flame = self.generated_flame.take();
        let script_generated = self.script_generated.take();
        let script_animation = self.script_animation.take();
        let generated_batch = self.generated_batch.take();

        UiResponse {
            config_export_requested: config_export_json,
            config_import_requested: config_import_json,
            config_save_file_requested: config_save_file,
            config_load_file_requested: config_load_file,
            flame_xml_import_file_requested: flame_xml_import_file,
            flame_xml_export_file_requested: flame_xml_export_file,
            new_flame_requested,
            random_flame_requested,
            palette_export_json,
            palette_save_file,
            palette_save_to_library,
            palette_delete_from_library,
            palette_import_json,
            palette_load_file,
            undo_requested,
            redo_requested,
            png_export_with_background,
            png_export_transparent,
            add_transform,
            delete_transform,
            clone_transform,
            add_linked_transform,
            delete_linked_transform,
            clone_linked_transform,
            add_final_transform,
            delete_final_transform,
            clone_final_transform,
            attachment_edit,
            open_palette_editor,
            open_palette_library,
            open_config_dialog,
            open_triangle_editor,
            open_preset_library,
            open_random_generator,
            fractal_viewport_size,
            fly_mouse_drag,
            fly_mode_toggle_requested,
            needs_repaint,
            selected_preset_config,
            file_browser_open_requested,
            animation_export_requested,
            load_subflame_into,
            animation_seek_changed,
            animation_seek_drag_stopped,
            path_filters_changed,
            generated_flame,
            script_generated,
            script_animation,
            generated_batch,
            load_audio_file,
            load_signal_file,
            save_signal_file,
            api_save_action,
            api_animation_save_action,
            loaded_api_flame_id,
            loaded_api_flame_is_public,
            loaded_api_flame_user_id,
            loaded_api_flame_animation_count,
            loaded_api_flame_animations,
            load_api_animation_id,
            clear_variation_cache_requested,
            variation_update_requested,
            script_cloud_request,
        }
    }

    /// Check if fractal browser panel needs thumbnail generation
    #[cfg(not(target_arch = "wasm32"))]
    pub fn fractal_browser_needs_thumbnails(&self) -> bool {
        if let Some(ref panel) = self.fractal_browser_panel {
            panel.is_generating()
        } else {
            false
        }
    }

    /// Generate one thumbnail for the fractal browser (call once per frame)
    /// Returns true if generation is complete
    #[cfg(not(target_arch = "wasm32"))]
    pub fn generate_fractal_browser_thumbnail(
        &mut self,
        device: &egui_wgpu::wgpu::Device,
        queue: &egui_wgpu::wgpu::Queue,
        palette_library: &crate::scene::palette::PaletteLibrary,
    ) -> bool {
        if let Some(ref mut panel) = self.fractal_browser_panel {
            // Get which tab/config needs generation
            if let Some((tab, _config)) = panel.next_pending_config() {
                panel.generate_one_thumbnail(tab, &self.ctx, |config| {
                    crate::renderer::render_thumbnail(device, queue, config, palette_library)
                })
            } else {
                true // Nothing to generate
            }
        } else {
            true // No panel, nothing to generate
        }
    }

    /// WASM: Start async thumbnail generation for fractal browser
    #[cfg(target_arch = "wasm32")]
    pub fn start_fractal_browser_thumbnails(
        &mut self,
        device: &egui_wgpu::wgpu::Device,
        queue: &egui_wgpu::wgpu::Queue,
    ) {
        if let Some(ref mut panel) = self.fractal_browser_panel {
            panel.start_async_thumbnails(device, queue);
        }
    }

    /// Load batch results into the unified Fractal Browser panel
    pub fn load_batch_into_fractal_browser(&mut self, configs: Vec<crate::config::FractalConfig>) {
        // Initialize panel if not already created
        if self.fractal_browser_panel.is_none() {
            self.fractal_browser_panel = Some(fractal_browser::FractalBrowserPanel::new());
        }

        if let Some(ref mut panel) = self.fractal_browser_panel {
            panel.load_batch(configs);
        }
    }

    /// Load file contents into the unified Fractal Browser panel
    pub fn load_file_into_fractal_browser(&mut self, path: std::path::PathBuf) {
        // Initialize panel if not already created
        if self.fractal_browser_panel.is_none() {
            self.fractal_browser_panel = Some(fractal_browser::FractalBrowserPanel::new());
        }

        if let Some(ref mut panel) = self.fractal_browser_panel {
            panel.load_file(path);
        }
    }

    /// Load JSON configs into the unified Fractal Browser panel
    pub fn load_json_into_fractal_browser(&mut self, json: &str, source_name: &str) {
        // Initialize panel if not already created
        if self.fractal_browser_panel.is_none() {
            self.fractal_browser_panel = Some(fractal_browser::FractalBrowserPanel::new());
        }

        if let Some(ref mut panel) = self.fractal_browser_panel {
            panel.load_json(json, source_name);
        }
    }

    /// Switch Fractal Browser to a specific tab
    pub fn switch_fractal_browser_tab(&mut self, tab: fractal_browser::BrowserTab) {
        // Initialize panel if not already created
        if self.fractal_browser_panel.is_none() {
            self.fractal_browser_panel = Some(fractal_browser::FractalBrowserPanel::new());
        }

        if let Some(ref mut panel) = self.fractal_browser_panel {
            panel.switch_to_tab(tab);
        }
    }

    /// Update the density histogram from computed data
    pub fn update_histogram(&mut self, histogram: crate::renderer::DensityHistogram) {
        self.density_histogram = histogram;
        // Note: Auto-levels is a one-shot button in render_levels_controls_managed
    }

    /// Get a reference to the density histogram
    pub fn density_histogram(&self) -> &crate::renderer::DensityHistogram {
        &self.density_histogram
    }
}

/// Read auth email from SystemSettings (cross-platform).
fn read_auth_email(config_manager: &crate::config::ConfigManager) -> Option<String> {
    config_manager.system_settings().auth_email.clone()
}

/// Reset all rendering settings to their defaults
fn reset_rendering_to_defaults(config_manager: &mut crate::config::ConfigManager) {
    use crate::config::{ConfigPath, defaults};

    // Reset fractal config rendering settings
    let _ = config_manager.update_batch(
        vec![
            (ConfigPath::MaxIterations, defaults::DEFAULT_MAX_ITERATIONS.into()),
            (ConfigPath::BlendFactor, defaults::DEFAULT_BLEND_FACTOR.into()),
            (ConfigPath::UseDynamicBlend, defaults::DEFAULT_USE_DYNAMIC_BLEND.into()),
            (ConfigPath::DeterministicRng, false.into()),
        ],
        "history.action.reset_rendering".to_string()
    );

    // Reset system settings (these don't go through undo history)
    let _ = config_manager.update_system_setting(
        ConfigPath::SystemIterationsPerThread,
        defaults::DEFAULT_ITERATIONS_PER_THREAD.into()
    );
    let _ = config_manager.update_system_setting(
        ConfigPath::SystemBurnIn,
        20u32.into() // Default burn-in
    );
    let _ = config_manager.update_system_setting(
        ConfigPath::SystemVsyncEnabled,
        true.into()
    );
    let _ = config_manager.update_system_setting(
        ConfigPath::SystemTargetFps,
        60.0f32.into()
    );
}

/// Reset all color/tone mapping settings to their defaults (except palette and background)
/// Returns the UpdateType so the caller can propagate GPU updates
fn reset_colors_to_defaults(config_manager: &mut crate::config::ConfigManager) -> crate::config::UpdateType {
    use crate::config::{ConfigPath, defaults, UpdateType};
    use crate::scene::tonemap::{ToneMapMode, ToneCurve};
    use crate::scene::palette::ColorMode;

    config_manager.update_batch(
        vec![
            // Color mode
            (ConfigPath::ColorMode, ColorMode::Palette.into()),
            // Tone mapping mode
            (ConfigPath::TonemapMode, ToneMapMode::Logarithmic.into()),
            // Tone mapping settings
            (ConfigPath::Exposure, defaults::DEFAULT_EXPOSURE.into()),
            (ConfigPath::Gamma, 2.2f32.into()),
            (ConfigPath::GammaThreshold, defaults::DEFAULT_GAMMA_THRESHOLD.into()),
            (ConfigPath::Brightness, defaults::DEFAULT_BRIGHTNESS.into()),
            (ConfigPath::Vibrancy, 1.0f32.into()),
            (ConfigPath::Saturation, defaults::DEFAULT_SATURATION.into()),
            (ConfigPath::HueShift, defaults::DEFAULT_HUE_SHIFT.into()),
            (ConfigPath::AlphaBlendLow, defaults::DEFAULT_ALPHA_BLEND_LOW.into()),
            (ConfigPath::AlphaBlendHigh, defaults::DEFAULT_ALPHA_BLEND_HIGH.into()),
            (ConfigPath::DensityScale, defaults::DEFAULT_DENSITY_SCALE.into()),
            // Tone curve
            (ConfigPath::UseCurve, true.into()),
            (ConfigPath::TonemapCurve, ToneCurve::default().into()),
            // Levels
            (ConfigPath::LevelsLow, 0.0f32.into()),
            (ConfigPath::LevelsHigh, 1000.0f32.into()),
            (ConfigPath::LevelsGamma, 1.0f32.into()),
            // Palette settings (not the palette itself or background)
            (ConfigPath::PaletteRotation, defaults::DEFAULT_PALETTE_ROTATION.into()),
            (ConfigPath::PaletteSqueeze, defaults::DEFAULT_PALETTE_SQUEEZE.into()),
            (ConfigPath::PaletteSize, defaults::DEFAULT_PALETTE_SIZE.into()),
        ],
        "history.action.reset_colors".to_string()
    ).unwrap_or(UpdateType::None)
}
