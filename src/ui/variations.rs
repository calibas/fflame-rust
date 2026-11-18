//! Variations panel — lists every variation in the global registry,
//! grouped by category. What the online library offers, updates and
//! Clear Cache are the Catalog's (docs/archive/projects/catalog.md, *Phase 3*).
//!
//! Two provenance signals, answering different questions:
//!
//! * **Per row** — built-in vs `API v#`. Registry provenance, useful
//!   while browsing.
//! * **At the top** — the variations *this flame* uses that were
//!   downloaded. That is the question §1 of the shared-resources plan
//!   actually poses ("is this flame about to run third-party code"),
//!   and it was previously answerable only by scrolling 646 rows.
//!
//! Rows also mark 2D-only variations, which are dropped entirely from
//! 3D flames — invisible in the render otherwise.

use egui;

use crate::variations::{global_registry, VariationCategory};

/// Variations this flame uses that were downloaded.
///
/// Pure and separate from the panel so the rule can be tested: "is this
/// flame running third-party code" is the question §1 of the
/// shared-resources plan asks, and the answer must not depend on
/// scrolling a 646-row list looking for "API v#". Downloads only, as the
/// banner says: a Rhai variation of the user's (`Provenance::Local`) is
/// theirs, and its key would read as noise (rhai-plugins.md, P3).
///
/// A name the registry does not know is NOT reported here — it is
/// missing rather than untrusted, and the fetch path handles it.
pub fn downloaded_variations_in_use(
    flame: &crate::scene::transforms::Flame,
    registry: &crate::variations::VariationRegistry,
) -> Vec<String> {
    flame
        .active_variation_names_ordered(registry)
        .into_iter()
        .filter(|name| registry.get(name).is_some_and(|v| v.provenance.is_cached_download()))
        .collect()
}

/// Render the Variations panel.
///
/// `flame` is the current flame, used only to answer "does this run any
/// third-party code" at the top — the registry listing below it is
/// flame-independent.
pub fn render_variations_panel(
    ui: &mut egui::Ui,
    flame: &crate::scene::transforms::Flame,
    catalog: Option<&crate::storage::variation_catalog::CachedCatalog>,
) {

    let registry = global_registry();
    let total = registry.all().iter().count();

    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(t!("variations_panel.total", count = total)).strong());
    });

    // Which variations THIS flame uses that did not ship with the app.
    //
    // The per-row tags below are registry provenance — useful when
    // browsing, useless for the question that actually matters: is the
    // flame I just opened about to run code somebody else wrote? That
    // answer was previously only reachable by scrolling 646 rows looking
    // for "API v#".
    let downloaded_in_use = downloaded_variations_in_use(flame, &registry);

    if !downloaded_in_use.is_empty() {
        ui.add_space(4.0);
        egui::Frame::group(ui.style())
            .fill(ui.visuals().warn_fg_color.linear_multiply(0.06))
            .show(ui, |ui| {
                ui.label(
                    egui::RichText::new(t!(
                        "variations_panel.flame_uses_downloaded",
                        count = downloaded_in_use.len()
                    ))
                    .strong(),
                );
                ui.weak(downloaded_in_use.join(", "));
            })
            .response
            .on_hover_text(t!("variations_panel.flame_uses_downloaded_hint"));
    }

    ui.add_space(4.0);

    // What the server has that this client does not, its updates and
    // Clear Cache: the Catalog's.
    super::inventory::moved_here(ui, &t!("variations_panel.moved_to_catalog"), crate::inventory::Family::Variations, super::inventory::Place::Online);

    ui.add_space(4.0);

    // Prose and update state, keyed by name. Built-in descriptions live
    // in Rust doc comments, invisible at runtime, so the catalog is the
    // only route by which any description reaches a row — including for
    // variations that shipped with the app.
    let by_name: std::collections::HashMap<&str, &crate::api::types::VariationListItem> = catalog
        .map(|c| c.items.iter().map(|i| (i.name.as_str(), i)).collect())
        .unwrap_or_default();

    {
        let categories = [
            (VariationCategory::Basic2D, "variations_panel.category_basic_2d"),
            (VariationCategory::Advanced2D, "variations_panel.category_advanced_2d"),
            (VariationCategory::Depth3D, "variations_panel.category_depth_3d"),
            (VariationCategory::Rotation3D, "variations_panel.category_rotation_3d"),
            (VariationCategory::Full3D, "variations_panel.category_full_3d"),
            (VariationCategory::Only3D, "variations_panel.category_only_3d"),
            (VariationCategory::Plugin, "variations_panel.category_plugin"),
        ];

        // One version of each of the user's Rhai variations.
        let superseded = registry.superseded(&flame.custom_variations);
        for (cat, label_key) in categories {
            let in_cat: Vec<_> = registry.by_category(cat).into_iter().filter(|v| !superseded.contains(&v.name)).collect();
            if in_cat.is_empty() {
                continue;
            }
            egui::CollapsingHeader::new(format!("{} ({})", t!(label_key), in_cat.len()))
                .default_open(false)
                .show(ui, |ui| {
                    for v in in_cat {
                        ui.horizontal(|ui| {
                            ui.label(&v.display_name);
                            // A built-in's name beside its title; a Rhai
                            // variation's is a key, nothing to read.
                            if !crate::scripted::is_key(&v.name) {
                                ui.weak(format!("({})", v.name));
                            }
                            // A variation with no 3D body is DROPPED from a
                            // 3D flame — it keeps its weight and contributes
                            // nothing, which is invisible in the render.
                            // Marking it here is the cheap half of the fix;
                            // the build also logs when it actually happens.
                            if v.wgsl_source_3d.is_none() {
                                ui.weak("· 2D only")
                                    .on_hover_text(t!("variations_panel.two_d_only_hint"));
                            }
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.weak(v.provenance.label());
                                },
                            );
                        });
                        if let Some(item) = by_name.get(v.name.as_str()) {
                            use crate::storage::variation_catalog::{merge_state, CatalogState};
                            if let CatalogState::UpdateAvailable { have, available } =
                                merge_state(*item, Some(&v.provenance))
                            {
                                ui.indent(format!("upd_{}", v.name), |ui| {
                                    ui.label(
                                        egui::RichText::new(t!(
                                            "variations_panel.row_update",
                                            have = have,
                                            available = available
                                        ))
                                        .color(ui.visuals().warn_fg_color),
                                    );
                                });
                            }
                            ui.indent(format!("meta_{}", v.name), |ui| {
                                if let Some(d) =
                                    item.description_plain.as_ref().filter(|d| !d.is_empty())
                                {
                                    ui.weak(d);
                                }
                                if !item.authors.is_empty() {
                                    ui.weak(t!(
                                        "variations_panel.row_authors",
                                        authors = item.authors.join(", ")
                                    ));
                                }
                            });
                        }
                        if !v.parameters.is_empty() {
                            ui.indent(format!("params_{}", v.name), |ui| {
                                for p in &v.parameters {
                                    ui.weak(format!(
                                        "  {} = {}",
                                        p.label, p.slot_default()
                                    ));
                                }
                            });
                        }
                    }
                });
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::scene::transforms::{Flame, Transform};

    fn api_download(name: &str) -> crate::api::types::VariationDownload {
        crate::api::types::VariationDownload {
            id: name.into(), name: name.into(), display_name: name.into(),
            description: None, category: "advanced_2d".into(), version: 3,
            phase: crate::api::types::ApiVariationPhase::Normal,
            needs_rng: false, needs_transform: false, writes_color: false,
            parameters: Vec::new(),
            shader_2d: Some("fn variation_x(p: vec2<f32>) -> vec2<f32> { return p; }".into()),
            shader_3d: None, init_param_count: 0, shader_init: None,
            features: Vec::new(), state_count: 0, shader_state_init: None,
            aliases: Vec::new(), plot_emits: 0, authors: Vec::new(),
            description_plain: None,
        }
    }

    /// A flame using a downloaded variation must say so; one using only
    /// built-ins must not cry wolf.
    #[test]
    fn a_flame_reports_the_third_party_code_it_runs() {
        let mut registry = crate::variations::VariationRegistry::new();
        registry.register_from_api(&api_download("borrowed_thing"), crate::provenance::Provenance::Api { version: 3 });

        // Built-ins only — nothing to warn about.
        let mut flame = Flame::default();
        let mut t = Transform::new();
        t.set_variation("linear", 1.0);
        flame.transforms.push(t);
        assert!(downloaded_variations_in_use(&flame, &registry).is_empty());

        // Add a downloaded one.
        let mut t2 = Transform::new();
        t2.set_variation("borrowed_thing", 1.0);
        flame.transforms.push(t2);
        assert_eq!(
            downloaded_variations_in_use(&flame, &registry),
            vec!["borrowed_thing".to_string()]
        );

        // One of the user's own Rhai variations is not a download.
        registry.register_from_api(&api_download("mine_in_rhai"), crate::provenance::Provenance::Local);
        let mut t3 = Transform::new();
        t3.set_variation("mine_in_rhai", 1.0);
        flame.transforms.push(t3);
        assert_eq!(downloaded_variations_in_use(&flame, &registry), vec!["borrowed_thing".to_string()]);
    }

    /// An unknown name is missing, not untrusted — the fetch path deals
    /// with it, and reporting it as third-party code would be a lie.
    #[test]
    fn an_unknown_variation_is_not_reported_as_downloaded() {
        let registry = crate::variations::VariationRegistry::new();
        let mut flame = Flame::default();
        let mut t = Transform::new();
        t.set_variation("never_heard_of_it", 1.0);
        flame.transforms.push(t);
        assert!(downloaded_variations_in_use(&flame, &registry).is_empty());
    }
}
