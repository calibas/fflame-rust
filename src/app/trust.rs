//! Holding a load from the server until its code is trusted
//! (docs/projects/curated-content.md, *Trust*; `curated::trust`).
//!
//! A fractal or an animation loaded from the server -- the Catalog, *Open
//! by ID or link*, a deep link, the Animation panel's list -- passes
//! through [`App::admit`]. The code it carries that is not trusted yet is
//! looked up on the server by hash: curated code is trusted for the
//! session. If anything is left, the load is held and the UI asks; the
//! answer loads it (and trusts the code, for the session or always) or
//! drops it. Code from this device -- a file opened, a preset shipped --
//! does not pass through here, and neither does the signed-in user's own
//! (its owner is the user).

use super::{ApiContentState, App, UrlLoadedData};
use crate::api::job::{self, Job};
use crate::api::types::{ResolveItem, Resolution};
use crate::config::FractalConfig;
use crate::curated::trust::{self, Answer, Code, Question};

/// A load from the server, as the app would apply it.
pub(crate) enum ApiLoad {
    /// A fractal the Catalog loaded (the preset path, one undo step).
    Preset { config: FractalConfig, api_metadata: Option<ApiContentState> },
    /// A deep link's, the Animation panel's or *Open by ID*'s load.
    Url(UrlLoadedData),
}

impl ApiLoad {
    fn code(&self) -> Vec<Code> {
        match self {
            ApiLoad::Preset { config, .. } | ApiLoad::Url(UrlLoadedData::Flame { config, .. }) => trust::in_config(config),
            ApiLoad::Url(UrlLoadedData::Animation { animation, .. }) => trust::in_animation(animation),
        }
    }

    /// Whose it is, when the server said (a fractal's owner; an
    /// animation's fractal's, which is the animation's too).
    fn owner_id(&self) -> Option<&str> {
        match self {
            ApiLoad::Preset { api_metadata, .. } => api_metadata.as_ref()?.flame_user_id.as_deref(),
            ApiLoad::Url(UrlLoadedData::Flame { user_id, .. }) => Some(user_id.as_str()),
            ApiLoad::Url(UrlLoadedData::Animation { flame_meta, .. }) => flame_meta.as_ref().map(|m| m.user_id.as_str()),
        }
    }

    fn name(&self) -> String {
        match self {
            ApiLoad::Preset { config, .. } | ApiLoad::Url(UrlLoadedData::Flame { config, .. }) => config.flame.name.clone(),
            ApiLoad::Url(UrlLoadedData::Animation { animation, .. }) => animation.name.clone(),
        }
    }
}

/// A load held for its code.
pub(crate) struct HeldLoad {
    load: ApiLoad,
    /// The code not trusted yet.
    unknown: Vec<Code>,
    /// The server's answer about it, on its way.
    lookup: Option<Job<Vec<Option<Resolution>>>>,
    /// The question asked of the user, once asked.
    question: Option<u64>,
}

static NEXT_QUESTION: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

impl App {
    /// Load `load` once its code is trusted (at once when it carries none
    /// that is not).
    pub(super) fn admit(&mut self, load: ApiLoad) {
        // The user's own: their code, nothing to ask.
        let own = matches!((load.owner_id(), self.current_user_id.as_deref()), (Some(owner), Some(me)) if owner == me);
        if own {
            return self.apply_api_load(load);
        }
        let unknown = trust::untrusted(&load.code());
        if unknown.is_empty() {
            return self.apply_api_load(load);
        }
        let items: Vec<ResolveItem> = unknown
            .iter()
            .filter_map(|c| c.kind.clone().map(|kind| ResolveItem { kind, name: None, hash: Some(c.hash.clone()) }))
            .collect();
        let api = crate::api::ApiState::new(crate::api::API_BASE_URL);
        let ctx = self.egui_layer.ctx.clone();
        let lookup = job::spawn(async move { api.resolve_curated_many(&items).await }, move || ctx.request_repaint());
        // A newer load replaces one still held (its question goes unanswered).
        self.held_load = Some(HeldLoad { load, unknown, lookup: Some(lookup), question: None });
    }

    /// Once a frame: the server's answer, then the user's.
    pub(super) fn handle_held_load(&mut self) {
        let Some(held) = self.held_load.as_mut() else { return };
        if let Some(lookup) = &held.lookup {
            let Some(result) = lookup.take() else { return };
            held.lookup = None;
            match result {
                // Curated: what the server answered for, hash for hash.
                Ok(answers) => trust::trust_for_session(answers.iter().flatten().map(|r| r.content_hash.as_str())),
                // Offline, or a server without the curated routes: nothing
                // is known curated, so everything left is asked about.
                Err(e) => log::info!("curated lookup failed ({e}): asking about all of it"),
            }
            held.unknown = trust::untrusted(&held.unknown);
            if held.unknown.is_empty() {
                if let Some(held) = self.held_load.take() {
                    self.apply_api_load(held.load);
                }
                return;
            }
            let id = NEXT_QUESTION.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            trust::ask(Question { id, what: held.load.name(), owner: None, code: held.unknown.clone() });
            held.question = Some(id);
            self.egui_layer.ctx.request_repaint();
            return;
        }
        let Some(id) = held.question else { return };
        let Some(answer) = trust::take_answer(id) else { return };
        let Some(held) = self.held_load.take() else { return };
        match answer {
            Answer::Load | Answer::Always => self.apply_api_load(held.load),
            Answer::No => {
                let name = held.load.name();
                self.egui_layer.show_api_notification(&t!("trust.not_loaded", name = name), false);
            }
        }
    }

    /// Apply a load from the server: as the Catalog, a deep link or the
    /// Animation panel always did.
    pub(super) fn apply_api_load(&mut self, load: ApiLoad) {
        match load {
            ApiLoad::Preset { config, api_metadata } => {
                let name = config.flame.name.clone();
                self.egui_layer.show_api_notification(&t!("api.loading_flame", name = name.clone()), false);
                if let Err(e) = self.load_config_with_undo(config, "history.action.load_preset".to_string(), api_metadata) {
                    log::error!("Failed to load preset: {}", e);
                } else {
                    self.egui_layer.show_api_notification(&t!("api.load_flame_success", name = name), false);
                }
            }
            ApiLoad::Url(UrlLoadedData::Flame { config, flame_id, is_public, user_id, animation_count, animations }) => {
                let name = config.flame.name.clone();
                log::info!("URL load: flame '{}' ({})", name, flame_id);
                let api_metadata = Some(ApiContentState {
                    flame_id: Some(flame_id.clone()),
                    flame_is_public: is_public,
                    flame_user_id: Some(user_id),
                    animation_id: None,
                    animation_count,
                    flame_animations: animations,
                });
                if let Err(e) = self.load_config_with_undo(config, format!("Load flame from URL: {}", name), api_metadata) {
                    log::error!("Failed to load flame from URL: {}", e);
                    self.egui_layer.show_api_notification(&t!("api.url_load_error", error = e), true);
                    return;
                }
                self.egui_layer.show_api_notification(&t!("api.url_load_flame_success", name = name), false);
            }
            ApiLoad::Url(UrlLoadedData::Animation { animation, animation_id, flame_id, flame_meta }) => {
                let anim_name = animation.name.clone();
                log::info!("URL load: animation '{}' ({})", anim_name, animation_id);

                // Switch to the animation-focused workspace so the user lands on a layout
                // with the Animation panel visible. In compact mode, just open the panel
                // since compact is single-panel.
                let compact = self.config_manager.system_settings().compact_mode.unwrap_or(false);
                if compact {
                    let ctx = &self.egui_layer.ctx;
                    self.workspace.open_compact_panel(crate::ui::workspace::PanelType::Animation, ctx);
                } else if !matches!(self.workspace.current_layout, crate::ui::workspace::WorkspaceLayout::Animation) {
                    self.workspace.apply_layout(crate::ui::workspace::WorkspaceLayout::Animation);
                }

                // Load the flame config from the animation's base_config (populated from embedded flame)
                if let Some(config) = animation.base_config.clone() {
                    let flame_name = config.flame.name.clone();
                    let api_metadata = flame_meta.as_ref().map(|meta| ApiContentState {
                        flame_id: flame_id.clone(),
                        flame_is_public: meta.is_public,
                        flame_user_id: Some(meta.user_id.clone()),
                        animation_id: Some(animation_id.clone()),
                        animation_count: meta.animation_count,
                        flame_animations: meta.animations.clone(),
                    });
                    if let Err(e) = self.load_config_with_undo(config, format!("Load flame for animation: {}", flame_name), api_metadata) {
                        log::error!("Failed to load flame for animation: {}", e);
                    }
                }

                // Load the animation
                let generators = animation.generators.clone();
                let duration = animation.duration;
                self.animation_controller.load(animation);
                // Bind AFTER controller.load (as the file-load path does).
                if let Some(anim) = self.animation_controller.animation.as_mut() {
                    anim.bind_to_config(self.config_manager.active_config());
                }
                self.egui_layer.signal_panel_state.restore_generators(generators, &mut self.signal_manager, duration);

                self.egui_layer.show_api_notification(&t!("api.url_load_animation_success", name = anim_name), false);
            }
        }
    }
}
