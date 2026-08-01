//! Variations panel — lists every variation in the global registry,
//! grouped by category, with a "Clear Variation Cache" action.
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
use rust_i18n::t;

use crate::variations::{global_registry, VariationCategory};

/// Variations this flame uses that did not ship with the app.
///
/// Pure and separate from the panel so the rule can be tested: "is this
/// flame running third-party code" is the question §1 of the
/// shared-resources plan asks, and the answer must not depend on
/// scrolling a 646-row list looking for "API v#".
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
        .filter(|name| registry.get(name).is_some_and(|v| !v.is_core))
        .collect()
}

/// The catalog section: what exists server-side versus what is here.
///
/// Silent when there is no catalog. An app that renders fractals
/// perfectly well offline should not grow an error panel because a
/// metadata endpoint was unreachable — and a user who has never signed
/// in has no reason to see one at all.
///
/// Returns the names the user asked to re-download.
fn render_catalog_summary(
    ui: &mut egui::Ui,
    catalog: Option<&crate::storage::variation_catalog::CachedCatalog>,
    registry: &crate::variations::VariationRegistry,
) -> Vec<String> {
    use crate::storage::variation_catalog::summarize;

    let mut update_requested = Vec::new();

    let Some(catalog) = catalog else { return update_requested };
    if catalog.items.is_empty() {
        return update_requested;
    }

    let crate::storage::variation_catalog::CatalogSummary {
        available,
        updatable,
        builtin_only_elsewhere,
    } = summarize(&catalog.items, |name| registry.get(name).map(|v| (v.is_core, v.version)));

    egui::CollapsingHeader::new(t!(
        "variations_panel.catalog_header",
        total = catalog.items.len(),
        available = available.len()
    ))
    .default_open(!updatable.is_empty())
    .show(ui, |ui| {
        if !updatable.is_empty() {
            ui.label(
                egui::RichText::new(t!(
                    "variations_panel.catalog_updates",
                    count = updatable.len()
                ))
                .strong(),
            );
            for (item, have, avail) in &updatable {
                ui.horizontal(|ui| {
                    ui.weak(format!("{} — have v{have}, v{avail} available", item.display_name));
                    // Re-fetching by name overwrites the cached copy:
                    // `register_from_api` replaces a non-core entry, so
                    // the same path that installs also updates.
                    if ui.small_button(t!("variations_panel.update_btn")).clicked() {
                        update_requested.push(item.name.clone());
                    }
                });
            }
            if updatable.len() > 1
                && ui
                    .button(t!("variations_panel.update_all_btn", count = updatable.len()))
                    .clicked()
            {
                update_requested.extend(updatable.iter().map(|(i, _, _)| i.name.clone()));
            }
            ui.add_space(4.0);
        }

        if available.is_empty() {
            ui.weak(t!("variations_panel.catalog_all_present"));
        } else {
            // Fetched on demand when a flame references one, so this is
            // a listing rather than a set of buttons — a download-all
            // control would pull shader code the user has no use for.
            ui.weak(t!("variations_panel.catalog_on_demand"));
            for item in available.iter().take(40) {
                ui.horizontal(|ui| {
                    ui.weak(&item.display_name);
                    if !item.has_shader_3d {
                        ui.weak("· 2D only");
                    }
                });
                if let Some(d) = item.description_plain.as_ref().filter(|d| !d.is_empty()) {
                    ui.indent(format!("cat_desc_{}", item.name), |ui| {
                        ui.weak(d);
                    });
                }
            }
            if available.len() > 40 {
                ui.weak(format!("… and {} more", available.len() - 40));
            }
        }

        if builtin_only_elsewhere > 0 {
            ui.add_space(4.0);
            ui.weak(t!(
                "variations_panel.catalog_builtin_only",
                count = builtin_only_elsewhere
            ))
            .on_hover_text(t!("variations_panel.catalog_builtin_only_hint"));
        }
    });

    update_requested
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
) -> VariationsPanelResponse {
    let mut response = VariationsPanelResponse::default();

    let registry = global_registry();
    let total = registry.all().iter().count();
    let api_count = registry.all().iter().filter(|v| !v.is_core).count();

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

    // What the server has that this client does not — the half of the
    // picture the registry cannot show. Absent when offline, which is a
    // normal state: the listing below is still the truth about what is
    // installed, just not about what exists.
    response.update_requested = render_catalog_summary(ui, catalog, &registry);

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

        for (cat, label_key) in categories {
            let in_cat: Vec<_> = registry.by_category(cat);
            if in_cat.is_empty() {
                continue;
            }
            egui::CollapsingHeader::new(format!("{} ({})", t!(label_key), in_cat.len()))
                .default_open(false)
                .show(ui, |ui| {
                    for v in in_cat {
                        ui.horizontal(|ui| {
                            ui.label(&v.display_name);
                            ui.weak(format!("({})", v.name));
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
                                    let source = if v.is_core {
                                        t!("variations_panel.source_built_in").to_string()
                                    } else {
                                        format!("{} v{}", t!("variations_panel.source_api"), v.version)
                                    };
                                    ui.weak(source);
                                },
                            );
                        });
                        if let Some(item) = by_name.get(v.name.as_str()) {
                            use crate::storage::variation_catalog::{merge_state, CatalogState};
                            if let CatalogState::UpdateAvailable { have, available } =
                                merge_state(item, Some((v.is_core, v.version)))
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
                                        p.display_name, p.default_value
                                    ));
                                }
                            });
                        }
                    }
                });
        }
    }

    ui.add_space(8.0);
    ui.separator();
    ui.add_space(4.0);

    ui.horizontal(|ui| {
        let label = t!("variations_panel.clear_cache_btn", count = api_count);
        if ui.add_enabled(api_count > 0, egui::Button::new(label.as_ref())).clicked() {
            response.clear_cache_requested = true;
        }
    });

    response
}

#[derive(Default)]
pub struct VariationsPanelResponse {
    pub clear_cache_requested: bool,
    /// Downloaded variations the user asked to re-fetch at the catalog's
    /// version. Re-uses the install path, which overwrites.
    pub update_requested: Vec<String>,
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
        registry.register_from_api(&api_download("borrowed_thing"));

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
