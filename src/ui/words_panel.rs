//! **The Words panel** (`docs/projects/word-editing.md` §6): the plan on
//! screen as a tree of its words, grouped by their last-applied maps,
//! each branch with its share of the view. A branch can be removed (a
//! pattern in `FractalConfig::word_removals`, undoable), drawn alone
//! while its solo button is held, and restored from the removed list.
//! The trim slider lives here too.
//!
//! It replaces the Path Editor, whose filters were never saved and
//! matched only 4-bit symbols without arms.

use crate::config::{ConfigManager, ConfigPath};
use crate::scene::word_tree::{parse_pattern, pattern_text, Branch, Tree};
use rust_i18n::t;

/// How many of a branch's children are listed before the rest are
/// summed on one line.
const CHILDREN_SHOWN: usize = 40;

/// Render the Words panel. `solo` is set to the pattern whose solo
/// button is held this frame.
pub fn render_words_content(
    ui: &mut egui::Ui,
    config_manager: &mut ConfigManager,
    renderer: Option<&crate::renderer::compute_kernel::FlameRenderer>,
    deep_zoom: &super::DeepZoom,
    solo: &mut Option<Vec<u32>>,
) {
    let config = config_manager.active_config().clone();

    let mut targeting = config.cylinder_targeting;
    if ui
        .checkbox(&mut targeting, t!("view.cylinder_targeting").as_ref())
        .on_hover_text(t!("words.tooltip_targeting"))
        .changed()
    {
        let _ = config_manager.update_param(ConfigPath::CylinderTargeting, targeting.into());
    }
    if !config.cylinder_targeting {
        ui.label(egui::RichText::new(t!("words.needs_targeting")).weak());
        removed_list(ui, config_manager, &config.word_removals, &armed_transforms(&config.flame));
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

    let armed = armed_transforms(&config.flame);
    ui.separator();
    if let Some(secs) = deep_zoom.planning {
        ui.horizontal(|ui| {
            ui.add(egui::Spinner::new());
            ui.label(t!("view.targeting_generating", secs = format!("{secs:.1}")));
        });
    }
    let tree = renderer.and_then(|r| r.word_tree());
    match &tree {
        Some(tree) => {
            ui.label(t!(
                "words.summary",
                words = tree.words.to_string(),
                drawn = tree.drawn.to_string()
            ));
            let mut remove: Option<Vec<u32>> = None;
            egui::ScrollArea::vertical()
                .id_salt("words_tree")
                .max_height(ui.available_height() * 0.7)
                .auto_shrink([false, true])
                .show(ui, |ui| {
                    for b in &tree.branches {
                        branch_row(ui, b, tree, &armed, &mut remove, solo);
                    }
                });
            if let Some(p) = remove {
                // A removal a new one ends with is contained in it: the
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
        None => {
            if deep_zoom.planning.is_none() {
                ui.label(egui::RichText::new(t!("words.no_plan")).weak());
            }
        }
    }
    removed_list(ui, config_manager, &config.word_removals, &armed);
}

/// One branch: its map, its share, its word count, solo and remove; its
/// children beneath when opened.
fn branch_row(
    ui: &mut egui::Ui,
    b: &Branch,
    tree: &Tree,
    armed: &[bool],
    remove: &mut Option<Vec<u32>>,
    solo: &mut Option<Vec<u32>>,
) {
    let header = |ui: &mut egui::Ui, remove: &mut Option<Vec<u32>>, solo: &mut Option<Vec<u32>>| {
        let mut name = egui::RichText::new(map_label(b.pattern[0], armed)).strong();
        if b.trimmed {
            name = name.weak().strikethrough();
        }
        ui.label(name).on_hover_text(t!("words.tooltip_branch", path = path_label(&b.pattern, armed)));
        let share = if tree.share > 0.0 && b.share > 0.0 {
            format!("{:.2}%", 100.0 * b.share / tree.share)
        } else {
            t!("words.unmeasured", percent = format!("{:.1}", 100.0 * b.prob / tree.prob.max(1e-300))).to_string()
        };
        ui.label(share);
        ui.label(egui::RichText::new(t!("words.word_count", count = b.words.to_string())).weak());
        if b.trimmed {
            ui.label(egui::RichText::new(t!("words.trimmed")).weak().italics());
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.small_button("🗑").on_hover_text(t!("words.tooltip_remove")).clicked() {
                *remove = Some(b.pattern.clone());
            }
            let held = ui.small_button(t!("words.solo").as_ref()).on_hover_text(t!("words.tooltip_solo"));
            if held.is_pointer_button_down_on() {
                *solo = Some(b.pattern.clone());
            }
        });
    };
    if b.children.is_empty() {
        ui.horizontal(|ui| {
            ui.add_space(ui.spacing().indent);
            header(ui, remove, solo);
        });
        return;
    }
    let id = ui.make_persistent_id(("words_branch", &b.pattern));
    egui::collapsing_header::CollapsingState::load_with_default_open(ui.ctx(), id, false)
        .show_header(ui, |ui| header(ui, remove, solo))
        .body(|ui| {
            for c in b.children.iter().take(CHILDREN_SHOWN) {
                branch_row(ui, c, tree, armed, remove, solo);
            }
            if b.children.len() > CHILDREN_SHOWN {
                let rest = &b.children[CHILDREN_SHOWN..];
                let share: f64 = rest.iter().map(|c| c.share).sum();
                ui.label(egui::RichText::new(t!(
                    "words.more",
                    count = rest.len().to_string(),
                    percent = format!("{:.2}", 100.0 * share / tree.share.max(1e-300))
                ))
                .weak());
            }
        });
}

/// The removed patterns, each with a restore button.
fn removed_list(ui: &mut egui::Ui, config_manager: &mut ConfigManager, removals: &[String], armed: &[bool]) {
    if removals.is_empty() {
        return;
    }
    ui.separator();
    ui.label(egui::RichText::new(t!("words.removed")).strong());
    let mut restore: Option<usize> = None;
    for (i, text) in removals.iter().enumerate() {
        ui.horizontal(|ui| {
            if ui.small_button("↺").on_hover_text(t!("words.tooltip_restore")).clicked() {
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
