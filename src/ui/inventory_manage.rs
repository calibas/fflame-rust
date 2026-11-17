//! Your own resources in the Catalog's info pane (docs/projects/catalog.md,
//! *Phase 4*): a Rhai file of yours is edited, renamed, duplicated,
//! deleted, exported, made a plug-in and published from here; a shipped
//! file or a built-in with a twin is copied into your files to edit (Edit
//! as Rhai). The family's top offers New and Import. The operations are
//! `storage::manage`'s; publishing is the script cloud's requests, a
//! conflict shown with the file it names.

use super::inventory::{InventoryContext, InventoryResponse};
use crate::app::script_cloud::{Local, ScriptCloudRequest as Req};
use crate::api::types::ApiVisibility;
use crate::inventory::{self, Entry, EntryKey, Family, Kind};
use crate::storage::library::Store;
use crate::storage::manage;

/// What the info pane is in the middle of: a rename being typed, a delete
/// awaiting its confirmation, the last operation's word.
#[derive(Default)]
pub struct Manage {
    renaming: Option<(EntryKey, String)>,
    confirm_delete: Option<(Store, String, String)>,
    /// A palette of yours awaiting its delete's confirmation, by name.
    confirm_delete_palette: Option<String>,
    /// A palette being published, by name.
    publishing: Option<(String, crate::api::job::Job<crate::api::types::PaletteItem>)>,
    status: Option<(String, bool)>,
}

/// What a management action asks of the panel.
pub enum Act {
    /// Select this entry (a file just made or renamed).
    Select(EntryKey),
}

/// The Catalog's key for a stored file of the user's.
pub fn file_key(store: Store, file: &str) -> EntryKey {
    EntryKey::File { store, file: file.to_string(), shipped: false }
}

/// A script's stem, for the Scripts panel.
fn stem(file: &str) -> String {
    file.strip_suffix(".rhai").unwrap_or(file).to_string()
}

/// Open a file in the Rhai Editor.
pub fn edit(store: Store, file: &str, shipped: bool, out: &mut InventoryResponse) {
    out.open_file = Some((store, file.to_string(), shipped));
}

impl Manage {
    fn ok(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), false));
    }

    fn fail(&mut self, msg: impl Into<String>) {
        self.status = Some((msg.into(), true));
    }

    /// The last operation's word, if any.
    pub fn render_status(&self, ui: &mut egui::Ui) {
        if let Some((text, is_error)) = &self.status {
            if *is_error {
                ui.colored_label(ui.visuals().error_fg_color, text);
            } else {
                ui.weak(text);
            }
        }
    }

    /// The actions for an entry the Catalog lists: a file of yours, a
    /// shipped file, a built-in with a twin. Drawn below the summary.
    pub fn render_actions(&mut self, ui: &mut egui::Ui, e: &Entry, cx: &mut InventoryContext, out: &mut InventoryResponse) -> Option<Act> {
        // A palette's publish, landed (whatever is selected now).
        if let Some((name, job)) = &self.publishing {
            if let Some(result) = job.take() {
                let name = name.clone();
                self.publishing = None;
                match result {
                    Ok(_) => self.ok(t!("manage.palette_published", name = name)),
                    Err(err) => self.fail(t!("manage.palette_publish_failed", error = err.to_string())),
                }
            }
        }
        let mut act = None;
        match &e.key {
            EntryKey::Palette(hash) if Some(e.group.order as usize) == cx.palette_library.custom_pack_index() => {
                act = self.render_own_palette(ui, e, hash, cx);
            }
            EntryKey::File { store, file, shipped: false } => act = self.render_own(ui, e, *store, file, cx, out),
            EntryKey::File { store, file, shipped: true } => {
                ui.horizontal_wrapped(|ui| {
                    if ui.button(t!("manage.edit_copy")).on_hover_text(t!("manage.edit_copy_tip")).clicked() {
                        let text = crate::storage::library::scan().into_iter().find(|f| f.store == *store && f.file == *file).map(|f| f.source);
                        act = text.and_then(|t| self.copy_in(&stem_of(file), &t, out));
                    }
                });
            }
            EntryKey::Registered(kind, name) => {
                if let Some(src) = inventory::twin_source(*kind, name) {
                    ui.horizontal_wrapped(|ui| {
                        if ui.button(t!("manage.edit_copy")).on_hover_text(t!("manage.edit_copy_tip")).clicked() {
                            act = self.copy_in(name, src, out);
                        }
                    });
                }
            }
            _ => {}
        }
        self.render_status(ui);
        act
    }

    /// A palette of yours (the Custom pack): Rename, Delete, and Publish
    /// to the online library, private or public (a palette item of its
    /// own there, listed under Online, Mine).
    fn render_own_palette(&mut self, ui: &mut egui::Ui, e: &Entry, hash: &str, cx: &mut InventoryContext) -> Option<Act> {
        let mut act = None;
        let Some(palette) = inventory::palette(cx.palette_library, hash) else { return None };
        let renaming = self.renaming.as_ref().is_some_and(|(k, _)| *k == e.key);
        ui.horizontal_wrapped(|ui| {
            if renaming {
                let mut done = None;
                if let Some((_, name)) = self.renaming.as_mut() {
                    let r = ui.add(egui::TextEdit::singleline(name).desired_width(160.0));
                    super::vkb_sync(ui, &r, name);
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        done = Some(true);
                    }
                }
                if ui.button(t!("manage.rename")).clicked() {
                    done = Some(true);
                }
                if ui.button(t!("manage.cancel")).clicked() {
                    done = Some(false);
                }
                if let Some(go) = done {
                    let name = self.renaming.take().map(|(_, n)| n.trim().to_string()).unwrap_or_default();
                    if go && !name.is_empty() && name != palette.name {
                        if cx.palette_library.has_custom_palette_named(&name) {
                            self.fail(t!("manage.name_taken", name = name));
                        } else {
                            let mut renamed = palette.clone();
                            renamed.name = name.clone();
                            let result = cx.palette_library.save_to_custom_library(renamed).and_then(|_| cx.palette_library.delete_from_custom_library(&palette.name).map(|_| ()));
                            match result {
                                Ok(()) => self.ok(t!("manage.renamed", name = name)),
                                Err(err) => self.fail(err),
                            }
                        }
                    }
                }
                return;
            }
            if ui.button(t!("manage.rename_dots")).clicked() {
                self.renaming = Some((e.key.clone(), palette.name.clone()));
            }
            if ui.button(t!("manage.delete_dots")).clicked() {
                self.confirm_delete_palette = Some(palette.name.clone());
            }
        });
        if cx.account.signed_in {
            ui.horizontal_wrapped(|ui| {
                let busy = self.publishing.is_some();
                for (visibility, key) in [(ApiVisibility::Private, "formula_editor.online_publish_private"), (ApiVisibility::Public, "formula_editor.online_publish_public")] {
                    if ui.add_enabled(!busy, egui::Button::new(t!(key))).on_hover_text(t!("manage.palette_publish_tip")).clicked() {
                        let api = cx.account.api();
                        let p = palette.clone();
                        let wake = ui.ctx().clone();
                        let job = crate::api::job::spawn(async move { api.create_palette(&p, visibility).await }, move || wake.request_repaint()).authed(true);
                        self.publishing = Some((palette.name.clone(), job));
                    }
                }
                if busy {
                    ui.spinner();
                }
            });
        }
        if let Some(name) = self.confirm_delete_palette.clone() {
            let mut open = true;
            let mut decided = None;
            egui::Window::new(t!("manage.delete_palette_title")).collapsible(false).resizable(false).open(&mut open).show(ui.ctx(), |ui| {
                ui.label(t!("manage.delete_palette_confirm", name = name.clone()));
                ui.horizontal(|ui| {
                    if ui.button(t!("catalog.delete_yes")).clicked() {
                        decided = Some(true);
                    }
                    if ui.button(t!("catalog.delete_no")).clicked() {
                        decided = Some(false);
                    }
                });
            });
            if !open || decided.is_some() {
                self.confirm_delete_palette = None;
            }
            if decided == Some(true) {
                match cx.palette_library.delete_from_custom_library(&name) {
                    Ok(_) => {
                        self.ok(t!("manage.deleted", name = name));
                        act = None;
                    }
                    Err(err) => self.fail(err),
                }
            }
        }
        act
    }

    /// A copy of `text` among your files, opened in its editor.
    fn copy_in(&mut self, name: &str, text: &str, out: &mut InventoryResponse) -> Option<Act> {
        match manage::create(name, text) {
            Ok((store, file)) => {
                self.ok(t!("manage.copied", name = file.clone()));
                edit(store, &file, false, out);
                Some(Act::Select(file_key(store, &file)))
            }
            Err(e) => {
                self.fail(e);
                None
            }
        }
    }

    fn render_own(&mut self, ui: &mut egui::Ui, e: &Entry, store: Store, file: &str, cx: &mut InventoryContext, out: &mut InventoryResponse) -> Option<Act> {
        let mut act = None;
        let renaming = self.renaming.as_ref().is_some_and(|(k, _)| *k == e.key);
        if renaming {
            ui.horizontal_wrapped(|ui| {
                let mut done = None;
                if let Some((_, name)) = self.renaming.as_mut() {
                    let r = ui.add(egui::TextEdit::singleline(name).desired_width(160.0));
                    super::vkb_sync(ui, &r, name);
                    if r.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                        done = Some(true);
                    }
                }
                if ui.button(t!("manage.rename")).clicked() {
                    done = Some(true);
                }
                if ui.button(t!("manage.cancel")).clicked() {
                    done = Some(false);
                }
                if let Some(go) = done {
                    let name = self.renaming.take().map(|(_, n)| n).unwrap_or_default();
                    if go {
                        match manage::rename(store, file, &name) {
                            Ok(to) => {
                                self.ok(t!("manage.renamed", name = to.clone()));
                                act = Some(Act::Select(file_key(store, &to)));
                            }
                            Err(err) => self.fail(err),
                        }
                    }
                }
            });
        } else {
            ui.horizontal_wrapped(|ui| {
                if ui.button(t!("manage.edit")).on_hover_text(t!("manage.edit_tip")).clicked() {
                    edit(store, file, false, out);
                }
                if ui.button(t!("manage.rename_dots")).clicked() {
                    self.renaming = Some((e.key.clone(), stem_of(file)));
                }
                if ui.button(t!("manage.duplicate")).clicked() {
                    match manage::duplicate(store, file) {
                        Ok(to) => {
                            self.ok(t!("manage.duplicated", name = to.clone()));
                            act = Some(Act::Select(file_key(store, &to)));
                        }
                        Err(err) => self.fail(err),
                    }
                }
                if ui.button(t!("manage.delete_dots")).clicked() {
                    self.confirm_delete = Some((store, file.to_string(), e.title.clone()));
                }
                if ui.button(t!("manage.export")).on_hover_text(t!("manage.export_tip")).clicked() {
                    self.export(store, file);
                }
            });
        }
        let text = manage::load(store, file).unwrap_or_default();
        if manage::can_be_plugin(store, file, &text) {
            let mut on = e.plugin;
            if ui.checkbox(&mut on, t!("formula_editor.plugin")).on_hover_text(t!("formula_editor.plugin_tip")).changed() {
                match manage::set_plugin(file, on) {
                    Ok(title) if on => self.ok(t!("formula_editor.plugin_on", title = title)),
                    Ok(_) => self.ok(t!("formula_editor.plugin_off", file = file)),
                    Err(err) => self.fail(t!("formula_editor.plugin_failed", file = file, error = err)),
                }
            }
        }
        if e.kind != Kind::Unreadable {
            self.render_online(ui, store, file, &text, cx, out);
        }
        self.render_confirm_delete(ui.ctx());
        act
    }

    /// Publish, update, unpublish and visibility for a file of yours, and
    /// a conflict's choice when the server's copy moved on.
    fn render_online(&mut self, ui: &mut egui::Ui, store: Store, file: &str, text: &str, cx: &mut InventoryContext, out: &mut InventoryResponse) {
        ui.separator();
        ui.label(egui::RichText::new(t!("manage.online")).strong());
        let cloud = cx.script_cloud;
        if !cx.account.signed_in {
            ui.weak(t!("formula_editor.online_sign_in"));
            return;
        }
        let local = match store {
            Store::Formulas => Local::Entry(file.to_string()),
            Store::Scripts => Local::Script(stem(file)),
        };
        if let Some((l, c)) = &cloud.conflict {
            if *l == local {
                ui.weak(t!("formula_editor.online_conflict", file = file, version = c.current_version));
                ui.horizontal_wrapped(|ui| {
                    if ui.add_enabled(!cloud.busy, egui::Button::new(t!("formula_editor.online_overwrite"))).clicked() {
                        out.script_cloud_request = Some(Req::Update { local: local.clone(), cloud_id: c.id.clone(), source: text.to_string(), version: c.current_version });
                    }
                    if ui.add_enabled(!cloud.busy, egui::Button::new(t!("formula_editor.online_load_theirs"))).clicked() {
                        out.script_cloud_request = Some(Req::Refetch { local: local.clone(), id: c.id.clone() });
                    }
                });
            }
        }
        if let Some(why) = manage::publish_refusal(store, file, text) {
            ui.weak(why);
            return;
        }
        let link = manage::link_of(store, file).unwrap_or_default();
        ui.horizontal_wrapped(|ui| match (link.cloud_id.clone(), link.version) {
            (Some(cloud_id), Some(version)) => {
                if ui
                    .add_enabled(!cloud.busy, egui::Button::new(t!("formula_editor.online_update")))
                    .on_hover_text(t!("formula_editor.online_update_tip", next = version + 1))
                    .clicked()
                {
                    out.script_cloud_request = Some(Req::Update { local: local.clone(), cloud_id: cloud_id.clone(), source: text.to_string(), version });
                }
                if ui.add_enabled(!cloud.busy, egui::Button::new(t!("manage.make_public"))).clicked() {
                    out.script_cloud_request = Some(Req::SetVisibility { local: local.clone(), cloud_id: cloud_id.clone(), visibility: ApiVisibility::Public });
                }
                if ui.add_enabled(!cloud.busy, egui::Button::new(t!("manage.make_private"))).clicked() {
                    out.script_cloud_request = Some(Req::SetVisibility { local: local.clone(), cloud_id: cloud_id.clone(), visibility: ApiVisibility::Private });
                }
                if ui.add_enabled(!cloud.busy, egui::Button::new(t!("formula_editor.online_unpublish"))).on_hover_text(t!("formula_editor.online_unpublish_tip")).clicked() {
                    out.script_cloud_request = Some(Req::Unpublish { local: local.clone(), cloud_id });
                }
                ui.weak(format!("v{version}"));
            }
            _ => {
                for (visibility, key, tip) in [
                    (ApiVisibility::Private, "formula_editor.online_publish_private", "formula_editor.online_publish_private_tip"),
                    (ApiVisibility::Public, "formula_editor.online_publish_public", "formula_editor.online_publish_public_tip"),
                ] {
                    if ui.add_enabled(!cloud.busy, egui::Button::new(t!(key))).on_hover_text(t!(tip)).clicked() {
                        match manage::prepare_publish(store, file) {
                            Ok(source) => out.script_cloud_request = Some(Req::Publish { local: local.clone(), source, visibility }),
                            Err(err) => self.fail(err),
                        }
                    }
                }
            }
        });
        if cloud.busy {
            ui.spinner();
        }
        if let Some(status) = &cloud.status {
            if cloud.status_is_error {
                ui.colored_label(ui.visuals().error_fg_color, status);
            } else {
                ui.weak(status);
            }
        }
    }

    fn render_confirm_delete(&mut self, ctx: &egui::Context) {
        let Some((store, file, title)) = self.confirm_delete.clone() else { return };
        let mut open = true;
        let mut decided = None;
        egui::Window::new(t!("manage.delete_title")).collapsible(false).resizable(false).open(&mut open).show(ctx, |ui| {
            ui.label(t!("manage.delete_confirm", name = title.clone(), file = file.clone()));
            ui.horizontal(|ui| {
                if ui.button(t!("catalog.delete_yes")).clicked() {
                    decided = Some(true);
                }
                if ui.button(t!("catalog.delete_no")).clicked() {
                    decided = Some(false);
                }
            });
        });
        if !open || decided.is_some() {
            self.confirm_delete = None;
        }
        if decided == Some(true) {
            match manage::delete(store, &file) {
                Ok(()) => self.ok(t!("manage.deleted", name = file)),
                Err(err) => self.fail(err),
            }
        }
    }

    /// Save a copy of a file outside the app.
    fn export(&mut self, store: Store, file: &str) {
        let Some(text) = manage::load(store, file) else { return };
        #[cfg(not(target_arch = "wasm32"))]
        {
            let picked = rfd::FileDialog::new().set_file_name(file).save_file();
            if let Some(path) = picked {
                match std::fs::write(&path, &text) {
                    Ok(()) => self.ok(t!("formula_editor.exported", file = path.display().to_string())),
                    Err(err) => self.fail(err.to_string()),
                }
            }
        }
        #[cfg(target_arch = "wasm32")]
        match crate::app::trigger_browser_download(text.as_bytes(), file, "text/plain") {
            Ok(()) => self.ok(t!("formula_editor.exported", file = file)),
            Err(err) => self.fail(err),
        }
    }

    // ------------------------------------------------------------- the top

    /// New and Import, in a family that holds Rhai files.
    pub fn render_top(&mut self, ui: &mut egui::Ui, family: Family, out: &mut InventoryResponse) -> Option<Act> {
        let kinds: Vec<Kind> = inventory::NEW_KINDS.iter().copied().filter(|k| k.family() == family).collect();
        if kinds.is_empty() {
            return None;
        }
        let mut act = None;
        ui.horizontal_wrapped(|ui| {
            ui.menu_button(t!("manage.new"), |ui| {
                for kind in &kinds {
                    if ui.button(t!(kind.label_key())).clicked() {
                        if let Some(text) = inventory::template(*kind) {
                            let name = format!("new-{}", t!(kind.label_key()).to_lowercase().replace(' ', "-"));
                            act = self.copy_in(&name, &text, out);
                        }
                        ui.close();
                    }
                }
            });
            if ui.button(t!("manage.import")).on_hover_text(t!("manage.import_tip")).clicked() {
                act = self.import(ui.ctx());
            }
        });
        #[cfg(target_arch = "wasm32")]
        if let Some(text) = ui.ctx().data_mut(|d| d.remove_temp::<String>(egui::Id::new(PENDING_IMPORT))) {
            // The picker gives the text alone: a Rhai entry or script
            // declares itself.
            let name = if text.contains("fn ") || text.contains("script(") { "imported.rhai" } else { "imported.ufm" };
            act = self.imported(name, &text);
        }
        self.render_status(ui);
        act
    }

    fn imported(&mut self, name: &str, text: &str) -> Option<Act> {
        match manage::import(name, text) {
            Ok((store, file)) => {
                self.ok(t!("manage.imported", name = file.clone()));
                Some(Act::Select(file_key(store, &file)))
            }
            Err(err) => {
                self.fail(err);
                None
            }
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    fn import(&mut self, _ctx: &egui::Context) -> Option<Act> {
        let path = rfd::FileDialog::new().add_filter("Rhai, Ultra Fractal, Fractint", crate::storage::formula_store::EXTENSIONS).pick_file()?;
        match std::fs::read(&path) {
            Ok(bytes) => {
                // Old formula files are often Latin-1, not UTF-8.
                let text = String::from_utf8(bytes.clone()).unwrap_or_else(|_| bytes.iter().map(|&b| b as char).collect());
                let name = path.file_name().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
                self.imported(&name, &text)
            }
            Err(err) => {
                self.fail(err.to_string());
                None
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    fn import(&mut self, ctx: &egui::Context) -> Option<Act> {
        crate::app::trigger_browser_file_picker(".rhai,.ufm,.ucl,.frm", ctx.clone(), PENDING_IMPORT);
        None
    }
}

/// Where the browser's picker leaves an imported file's text.
#[cfg(target_arch = "wasm32")]
const PENDING_IMPORT: &str = "catalog_pending_import";

/// A file's name without its extension.
fn stem_of(file: &str) -> String {
    file.rsplit_once('.').map_or(file, |(s, _)| s).to_string()
}
