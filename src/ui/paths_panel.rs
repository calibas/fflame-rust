//! **The Paths panel** (`docs/projects/word-editing.md` §6): the plan on
//! screen as a tree of its words -- "paths" in the UI, the transforms
//! that lead into each part of the picture -- grouped by their
//! last-applied maps, each with its share of the view. A path can be
//! removed (a pattern in `FractalConfig::word_removals`, undoable), drawn
//! alone while its solo button is held, opened to see the paths inside it
//! (the plan splits it: `FlameRenderer::request_split`), and restored
//! from the removed list. The trim slider lives here too.
//!
//! Also here: the Focused Rendering switch (cylinder targeting, in the
//! code) and its status line, which the View panel shows as well.
//!
//! It replaces the Path Editor, whose filters were never saved and
//! matched only 4-bit symbols without arms.

use crate::config::{ConfigManager, ConfigPath, FractalConfig};
use crate::scene::word_tree::{parse_pattern, pattern_text, Branch, Tree};
use rust_i18n::t;

/// How many of a path's children are listed before the rest are summed
/// on one line.
const CHILDREN_SHOWN: usize = 40;

/// The Focused Rendering switch -- Off, Auto, Always -- and a line saying
/// what it is doing for this view. Shared by the View and Paths panels.
pub(super) fn focused_rendering(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    config: &FractalConfig,
    deep_zoom: &super::DeepZoom,
) {
    #[derive(PartialEq, Clone, Copy)]
    enum Focus {
        Off,
        Auto,
        Always,
    }
    let now = match (config.cylinder_targeting, config.cylinder_always) {
        (false, _) => Focus::Off,
        (true, false) => Focus::Auto,
        (true, true) => Focus::Always,
    };
    ui.horizontal(|ui| {
        ui.label(t!("view.focused_rendering")).on_hover_text(t!("view.tooltip_focused_rendering"));
        for (mode, label, tip) in [
            (Focus::Off, "view.focus_off", "view.tooltip_focus_off"),
            (Focus::Auto, "view.focus_auto", "view.tooltip_focus_auto"),
            (Focus::Always, "view.focus_always", "view.tooltip_focus_always"),
        ] {
            if ui.selectable_label(now == mode, t!(label).as_ref()).on_hover_text(t!(tip)).clicked() && now != mode {
                let _ = config_manager.update_batch(
                    vec![
                        (ConfigPath::CylinderTargeting, (mode != Focus::Off).into()),
                        (ConfigPath::CylinderAlways, (mode == Focus::Always).into()),
                    ],
                    t!("view.focused_rendering").to_string(),
                );
            }
        }
    });
    if !config.cylinder_targeting {
        return;
    }
    use crate::renderer::TargetingState as TS;
    if let Some(secs) = deep_zoom.planning {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new());
            ui.label(t!("view.targeting_generating", secs = format!("{secs:.1}")));
        });
        // The plan below is the one still drawing.
        if matches!(deep_zoom.targeting, TS::Active { .. }) {
            ui.label(t!("view.targeting_previous_plan"));
        }
    }
    let line = match &deep_zoom.targeting {
        TS::Off => t!("view.targeting_off").to_string(),
        TS::NotPlanar => t!("view.targeting_not_planar").to_string(),
        TS::NotWorthIt { speedup } => t!("view.targeting_not_worth_it", speedup = format!("{speedup:.2}")).to_string(),
        TS::Active { words, depth, speedup, lost, .. } => {
            let mut line = if *speedup >= 1.0 {
                t!(
                    "view.targeting_active",
                    words = words.to_string(),
                    depth = depth.to_string(),
                    speedup = format!("{speedup:.0}")
                )
                .to_string()
            } else {
                // Kept where it does not pay: Always, or an edit.
                t!("view.targeting_active_slower", words = words.to_string(), speedup = format!("{speedup:.2}")).to_string()
            };
            // Never folded into the sentence above: a render that is
            // missing part of its attractor says so in its own clause,
            // or it does not really say it.
            if *lost > 0.0 {
                line.push(' ');
                line.push_str(&t!("view.targeting_lost", percent = format!("{:.3}", lost * 100.0)).to_string());
            }
            line
        }
        TS::Declined(why) => {
            use crate::scene::cylinder::NoCylinders as NC;
            let reason = match why {
                NC::Unbounded { index, why } => {
                    t!("view.no_cyl_unbounded", index = index.to_string(), why = why.clone()).to_string()
                }
                NC::NoInvariantBall => t!("view.no_cyl_no_ball").to_string(),
                NC::ColourNotAffine => t!("view.no_cyl_colour").to_string(),
                NC::Xaos => t!("view.no_cyl_xaos").to_string(),
                NC::NotContractive(i) => t!("view.no_cyl_expanding", index = i.to_string()).to_string(),
                NC::Empty => t!("view.no_cyl_empty").to_string(),
                NC::ViewIsEmpty => t!("view.no_cyl_off_attractor").to_string(),
                NC::TimedOut { nodes } => t!("view.no_cyl_timed_out", nodes = nodes.to_string()).to_string(),
                NC::TooManyWords(n) => t!("view.no_cyl_too_many", count = n.to_string()).to_string(),
            };
            t!("view.targeting_declined", reason = reason).to_string()
        }
    };
    // "Not running" beside "Generating" says the same thing twice, and
    // less.
    if !(deep_zoom.planning.is_some() && matches!(deep_zoom.targeting, TS::Off)) {
        ui.label(line);
    }
}

/// Render the Paths panel. `solo` is set to the path whose solo button is
/// held this frame, `split` to a path opened to see inside that the plan
/// has not split yet.
pub fn render_paths_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    renderer: Option<&crate::renderer::compute_kernel::FlameRenderer>,
    deep_zoom: &super::DeepZoom,
    solo: &mut Option<Vec<u32>>,
    split: &mut Option<Vec<u32>>,
) {
    let config = config_manager.active_config().clone();
    let armed = armed_transforms(&config.flame);

    focused_rendering(ui, config_manager, &config, deep_zoom);
    if !config.cylinder_targeting {
        ui.label(egui::RichText::new(t!("paths.needs_focus")).weak());
        removed_list(ui, config_manager, &config.word_removals, &armed);
        return;
    }

    // Trim (§4): logarithmic, because the settings that matter are small
    // -- 0.01 already drops a sliver at 0.5% of the view.
    let mut trim = config.cylinder_trim;
    if ui
        .add(
            egui::Slider::new(&mut trim, 0.0..=1.0)
                .logarithmic(true)
                .smallest_positive(1e-4)
                .text(t!("view.cylinder_trim").as_ref()),
        )
        .on_hover_text(t!("view.tooltip_cylinder_trim"))
        .changed()
    {
        let _ = config_manager.update_param(ConfigPath::CylinderTrim, trim.into());
    }
    if config.cylinder_trim > 0.0 {
        let mut levels = config.cylinder_trim_levels;
        ui.horizontal(|ui| {
            if ui
                .add(egui::DragValue::new(&mut levels).range(1..=8))
                .on_hover_text(t!("view.tooltip_cylinder_trim_levels"))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::CylinderTrimLevels, levels.into());
            }
            ui.label(t!("view.cylinder_trim_levels"));
        });
    }

    ui.separator();
    if let Some(tree) = renderer.and_then(|r| r.word_tree()) {
        ui.label(t!("paths.summary", paths = tree.words.to_string(), drawn = tree.drawn.to_string()));
        let mut remove: Option<Vec<u32>> = None;
        let planning = deep_zoom.planning.is_some();
        egui::ScrollArea::vertical()
            .id_salt("paths_tree")
            .max_height(ui.available_height() * 0.7)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                for b in &tree.branches {
                    branch_row(ui, b, &tree, &armed, planning, &mut remove, solo, split);
                }
            });
        if let Some(p) = remove {
            // A removal the new one ends with is contained in it: the
            // new one replaces it.
            let mut list: Vec<String> = config
                .word_removals
                .iter()
                .filter(|t| parse_pattern(t).is_none_or(|q| !q.ends_with(&p)))
                .cloned()
                .collect();
            list.push(pattern_text(&p));
            let _ = config_manager.update_param(ConfigPath::WordRemovals, list.into());
        }
    }
    removed_list(ui, config_manager, &config.word_removals, &armed);
}

/// One path: its transform, its share, how many paths it holds, solo and
/// remove; the paths inside it beneath when opened.
#[allow(clippy::too_many_arguments)]
fn branch_row(
    ui: &mut egui::Ui,
    b: &Branch,
    tree: &Tree,
    armed: &[bool],
    planning: bool,
    remove: &mut Option<Vec<u32>>,
    solo: &mut Option<Vec<u32>>,
    split: &mut Option<Vec<u32>>,
) {
    let header = |ui: &mut egui::Ui, remove: &mut Option<Vec<u32>>, solo: &mut Option<Vec<u32>>| {
        let mut name = egui::RichText::new(map_label(b.pattern[0], armed)).strong();
        if b.trimmed {
            name = name.weak().strikethrough();
        }
        ui.label(name).on_hover_text(t!("paths.tooltip_path", path = path_label(&b.pattern, armed)));
        let share = if tree.share > 0.0 && b.share > 0.0 {
            format!("{:.2}%", 100.0 * b.share / tree.share)
        } else {
            t!("paths.unmeasured", percent = format!("{:.1}", 100.0 * b.prob / tree.prob.max(1e-300))).to_string()
        };
        ui.label(share);
        if b.words > 1 {
            ui.label(egui::RichText::new(t!("paths.path_count", count = b.words.to_string())).weak());
        }
        if b.trimmed {
            ui.label(egui::RichText::new(t!("paths.trimmed")).weak().italics());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("🗑").on_hover_text(t!("paths.tooltip_remove")).clicked() {
                *remove = Some(b.pattern.clone());
            }
            let held = ui.small_button(t!("paths.solo").as_ref()).on_hover_text(t!("paths.tooltip_solo"));
            if held.is_pointer_button_down_on() {
                *solo = Some(b.pattern.clone());
            }
        });
    };
    // A path the tree stops at for its depth, not because the plan does,
    // has nothing further to show.
    if b.children.is_empty() && !b.leaf {
        ui.horizontal(|ui| {
            ui.add_space(ui.spacing().indent);
            header(ui, remove, solo);
        });
        return;
    }
    let id = ui.make_persistent_id(("paths_branch", &b.pattern));
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
        .show_header(ui, |ui| header(ui, remove, solo))
        .body(|ui| {
            if b.children.is_empty() {
                // One path of the plan, whole: opening it asks the plan to
                // split it. A path that cannot be split -- a blur's, whose
                // region is the whole fractal -- stays whole.
                *split = Some(b.pattern.clone());
                let note = if planning { t!("paths.splitting") } else { t!("paths.cannot_split") };
                ui.label(egui::RichText::new(note).weak().italics());
                return;
            }
            for c in b.children.iter().take(CHILDREN_SHOWN) {
                branch_row(ui, c, tree, armed, planning, remove, solo, split);
            }
            if b.children.len() > CHILDREN_SHOWN {
                let rest = &b.children[CHILDREN_SHOWN..];
                let share: f64 = rest.iter().map(|c| c.share).sum();
                ui.label(
                    egui::RichText::new(t!(
                        "paths.more",
                        count = rest.len().to_string(),
                        percent = format!("{:.2}", 100.0 * share / tree.share.max(1e-300))
                    ))
                    .weak(),
                );
            }
        });
}

/// The removed paths, each with a restore button.
fn removed_list(ui: &mut egui::Ui, config_manager: &mut ConfigManager, removals: &[String], armed: &[bool]) {
    if removals.is_empty() {
        return;
    }
    ui.separator();
    ui.label(egui::RichText::new(t!("paths.removed")).strong());
    let mut restore: Option<usize> = None;
    for (i, text) in removals.iter().enumerate() {
        ui.horizontal(|ui| {
            if ui.small_button("↺").on_hover_text(t!("paths.tooltip_restore")).clicked() {
                restore = Some(i);
            }
            match parse_pattern(text) {
                Some(p) => ui.label(path_label(&p, armed)),
                None => ui.label(egui::RichText::new(text).weak()),
            };
        });
    }
    if let Some(i) = restore {
        let mut list = removals.to_vec();
        list.remove(i);
        let _ = config_manager.update_param(ConfigPath::WordRemovals, list.into());
    }
}

/// Which transforms draw among several images, so a map names its arm.
fn armed_transforms(flame: &crate::scene::transforms::Flame) -> Vec<bool> {
    flame
        .transforms
        .iter()
        .map(|t| t.variations.iter().any(|(n, w)| *w != 0.0 && crate::variations::bound::arms_for(n).is_some()))
        .collect()
}

/// A map as the panel names it: the transform as the Transforms panel
/// numbers it, and the arm, from 1, where the transform has arms.
fn map_label(sym: u32, armed: &[bool]) -> String {
    let t = (sym & 0xff) as usize;
    if armed.get(t).copied().unwrap_or(false) {
        format!("T{}·{}", t + 1, (sym >> 8) + 1)
    } else {
        format!("T{}", t + 1)
    }
}

/// A pattern's maps in the order they are applied, the map nearest the
/// view last.
fn path_label(pattern: &[u32], armed: &[bool]) -> String {
    pattern.iter().map(|&s| map_label(s, armed)).collect::<Vec<_>>().join(" → ")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_are_named_as_the_transforms_panel_numbers_them() {
        let armed = [false, true];
        assert_eq!(map_label(0, &armed), "T1");
        assert_eq!(map_label(1 | 2 << 8, &armed), "T2·3");
        assert_eq!(path_label(&[1 | 1 << 8, 0], &armed), "T2·2 → T1");
    }
}
