//! The Texture panel (`docs/projects/sim-textures.md`): the textures an
//! escape-time config can use -- the shipped presets, then the user's
//! own, saved from Simulation mode with "Save as texture" -- each with a
//! preview, picked into the config with one undo step.
//!
//! A preview is the texture itself, generated (or read from the cache)
//! and shrunk. Desktop generates one a frame, blocking, as the fractal
//! browser does its thumbnails; the web spawns them.

use std::collections::{HashMap, HashSet};

use crate::config::escape::{EscapeTexture, ShadingTexture, TextureFit};
use crate::config::{ConfigChange, ConfigManager, ConfigPath, ConfigValue, FractalConfig};
use crate::textures::{TextureLibrary, TextureOrigin};

/// Preview side, in points.
const THUMB: u32 = 96;

pub struct TexturesPanel {
    library: TextureLibrary,
    /// The cache key of each entry, in library order: what a preview is
    /// filed under, and what tells a renamed copy from a different recipe.
    keys: Vec<String>,
    seen_generation: u64,
    previews: HashMap<String, egui::TextureHandle>,
    failed: HashSet<String>,
    /// A user texture being renamed: (its name, the edit buffer).
    rename: Option<(String, String)>,
    #[cfg(target_arch = "wasm32")]
    in_flight: HashSet<String>,
    #[cfg(target_arch = "wasm32")]
    async_results: std::rc::Rc<std::cell::RefCell<Vec<(String, Result<image::RgbaImage, String>)>>>,
}

impl Default for TexturesPanel {
    fn default() -> Self {
        Self::new()
    }
}

impl TexturesPanel {
    pub fn new() -> Self {
        let library = TextureLibrary::load();
        let keys = library.entries.iter().map(|e| crate::textures::cache::key(&e.config)).collect();
        Self {
            library,
            keys,
            seen_generation: crate::textures::library_generation(),
            previews: HashMap::new(),
            failed: HashSet::new(),
            rename: None,
            #[cfg(target_arch = "wasm32")]
            in_flight: HashSet::new(),
            #[cfg(target_arch = "wasm32")]
            async_results: Default::default(),
        }
    }

    /// Reload the list when a texture was saved, renamed or deleted
    /// anywhere since it was last read.
    fn refresh(&mut self) {
        let generation = crate::textures::library_generation();
        if generation != self.seen_generation {
            self.library = TextureLibrary::load();
            self.keys = self.library.entries.iter().map(|e| crate::textures::cache::key(&e.config)).collect();
            self.seen_generation = generation;
        }
    }

    /// The next texture still without a preview.
    pub fn next_pending(&self) -> Option<(String, FractalConfig)> {
        self.library
            .entries
            .iter()
            .zip(&self.keys)
            .find(|(_, k)| !self.previews.contains_key(*k) && !self.failed.contains(*k))
            .map(|(e, k)| (k.clone(), e.config.clone()))
    }

    /// Take a generated texture: shrink it to a preview.
    pub fn deliver(&mut self, ctx: &egui::Context, key: String, image: Result<image::RgbaImage, String>) {
        match image {
            Ok(image) => {
                let small = image::imageops::resize(&image, THUMB, THUMB, image::imageops::FilterType::Triangle);
                let color = egui::ColorImage::from_rgba_unmultiplied([THUMB as usize, THUMB as usize], small.as_raw());
                let handle = ctx.load_texture(format!("texture-{key}"), color, egui::TextureOptions::LINEAR);
                self.previews.insert(key, handle);
            }
            Err(e) => {
                log::warn!("texture preview failed: {e}");
                self.failed.insert(key);
            }
        }
    }

    /// Desktop: generate one missing preview, blocking (a cache hit is a
    /// PNG read; a miss renders the recipe, about half a second for the
    /// shipped presets). Call once a frame while [`Self::next_pending`].
    #[cfg(not(target_arch = "wasm32"))]
    pub fn generate_one(&mut self, ctx: &egui::Context, device: &egui_wgpu::wgpu::Device, queue: &egui_wgpu::wgpu::Queue) {
        if let Some((key, recipe)) = self.next_pending() {
            let image = pollster::block_on(crate::textures::obtain(device, queue, &recipe)).map_err(|e| e.to_string());
            self.deliver(ctx, key, image);
        }
    }

    /// Web: spawn every missing preview, and take the finished ones.
    #[cfg(target_arch = "wasm32")]
    pub fn start_async(&mut self, ctx: &egui::Context, device: &egui_wgpu::wgpu::Device, queue: &egui_wgpu::wgpu::Queue) {
        let done: Vec<_> = self.async_results.borrow_mut().drain(..).collect();
        for (key, image) in done {
            self.in_flight.remove(&key);
            self.deliver(ctx, key, image);
        }
        for (entry, key) in self.library.entries.iter().zip(&self.keys) {
            if self.previews.contains_key(key) || self.failed.contains(key) || self.in_flight.contains(key) {
                continue;
            }
            self.in_flight.insert(key.clone());
            let (key, recipe, results) = (key.clone(), entry.config.clone(), std::rc::Rc::clone(&self.async_results));
            let (device, queue) = (device.clone(), queue.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let image = crate::textures::obtain(&device, &queue, &recipe).await.map_err(|e| e.to_string());
                results.borrow_mut().push((key, image));
            });
        }
    }

    pub fn render(&mut self, ui: &mut egui::Ui, config_manager: &mut ConfigManager) {
        self.refresh();
        let current = config_manager.config().escape.texture.clone();
        // Simulation mode shows the library to browse and manage what it
        // saved; only an escape-time fractal takes a texture.
        let can_pick = config_manager.config().render_mode == crate::scene::transforms::RenderMode::Escape;
        if !can_pick {
            ui.weak(t!("textures_panel.browse_only"));
        }

        ui.horizontal(|ui| {
            ui.label(t!("textures_panel.current"));
            match &current {
                Some(t) => {
                    ui.strong(&t.name);
                    if ui.button(t!("textures_panel.clear")).on_hover_text(t!("textures_panel.clear_tip")).clicked() {
                        set_texture(config_manager, None, "history.action.texture_clear");
                    }
                }
                None => {
                    ui.weak(t!("textures_panel.none"));
                }
            }
        });
        if can_pick {
            overlay_controls(ui, config_manager, current.is_some());
            bump_controls(ui, config_manager);
            // The third use is a colouring, picked in the Escape panel.
            let trapping = crate::escape::get_coloring(&config_manager.config().escape.coloring)
                .has_feature(crate::escape::ColoringFeature::TextureInLoop);
            ui.weak(if trapping { t!("textures_panel.trap_in_use") } else { t!("textures_panel.trap_hint") });
        }
        ui.separator();

        let mut pick = None;
        let mut delete = None;
        let mut rename_done = None;
        egui::ScrollArea::vertical().auto_shrink([false, false]).show(ui, |ui| {
            let mut last_origin = None;
            for (entry, key) in self.library.entries.iter().zip(&self.keys) {
                if last_origin != Some(entry.origin) {
                    ui.add_space(4.0);
                    ui.strong(match entry.origin {
                        TextureOrigin::Preset => t!("textures_panel.presets"),
                        TextureOrigin::User => t!("textures_panel.saved"),
                    });
                    last_origin = Some(entry.origin);
                }
                ui.horizontal(|ui| {
                    let size = egui::vec2(THUMB as f32, THUMB as f32) * 0.6;
                    let picked = current.as_ref().is_some_and(|t| t.name == entry.name);
                    let clicked = match self.previews.get(key) {
                        Some(tex) => ui
                            .add(egui::ImageButton::new(egui::load::SizedTexture::new(tex.id(), size)).selected(picked))
                            .on_hover_text(t!("textures_panel.pick_tip"))
                            .clicked(),
                        None => {
                            let label = if self.failed.contains(key) {
                                t!("textures_panel.failed")
                            } else {
                                t!("textures_panel.generating")
                            };
                            ui.add_sized(size, egui::Label::new(label));
                            false
                        }
                    };
                    if clicked {
                        pick = Some(entry.clone());
                    }
                    ui.vertical(|ui| {
                        let editing = self.rename.as_ref().is_some_and(|(n, _)| *n == entry.name);
                        if editing {
                            let (_, buffer) = self.rename.as_mut().unwrap();
                            let field = ui.text_edit_singleline(buffer);
                            if field.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                                rename_done = Some((entry.name.clone(), buffer.clone()));
                            }
                            let mut cancel = false;
                            ui.horizontal(|ui| {
                                if ui.button(t!("textures_panel.rename_ok")).clicked() {
                                    rename_done = Some((entry.name.clone(), buffer.clone()));
                                }
                                cancel = ui.button(t!("textures_panel.cancel")).clicked();
                            });
                            if cancel {
                                self.rename = None;
                            }
                        } else {
                            let label = if picked { egui::RichText::new(&entry.name).strong() } else { egui::RichText::new(&entry.name) };
                            ui.label(label);
                            let (w, h) = crate::textures::texture_size(&entry.config);
                            ui.weak(format!("{w}×{h}, {} steps", entry.config.sim.steps));
                            if entry.origin == TextureOrigin::User {
                                ui.horizontal(|ui| {
                                    if ui.small_button(t!("textures_panel.rename")).clicked() {
                                        self.rename = Some((entry.name.clone(), entry.name.clone()));
                                    }
                                    if ui.small_button(t!("textures_panel.delete")).on_hover_text(t!("textures_panel.delete_tip")).clicked() {
                                        delete = Some(entry.name.clone());
                                    }
                                });
                            }
                        }
                    });
                });
            }
            if !self.library.entries.iter().any(|e| e.origin == TextureOrigin::User) {
                ui.add_space(6.0);
                ui.weak(t!("textures_panel.empty_hint"));
            }
        });

        if let Some(entry) = pick.filter(|_| can_pick) {
            let texture = EscapeTexture { name: entry.name, config: Box::new(entry.config) };
            set_texture(config_manager, Some(texture), "history.action.texture_pick");
        }
        if let Some(name) = delete {
            if let Err(e) = crate::textures::delete_user(&name) {
                log::error!("could not delete texture {name}: {e}");
            }
        }
        if let Some((old, new)) = rename_done {
            if let Err(e) = crate::textures::rename_user(&old, &new) {
                log::error!("could not rename texture {old}: {e}");
            }
            self.rename = None;
        }
    }
}

/// What the texture does in the picture: Kalles Fraktaler's overlay.
fn overlay_controls(ui: &mut egui::Ui, config_manager: &mut ConfigManager, has_texture: bool) {
    let ov = config_manager.config().escape.texture_overlay;
    let mut enabled = ov.enabled;
    if ui
        .checkbox(&mut enabled, t!("textures_panel.overlay"))
        .on_hover_text(t!("textures_panel.overlay_tip"))
        .changed()
    {
        let _ = config_manager.update_param(ConfigPath::EscapeTextureOverlay, ConfigValue::Bool(enabled));
    }
    if !ov.enabled {
        return;
    }
    if !has_texture {
        ui.weak(t!("textures_panel.overlay_needs_texture"));
    }
    egui::Grid::new("texture_overlay").num_columns(2).show(ui, |ui| {
        ui.label(t!("textures_panel.merge"));
        let mut v = ov.merge;
        if ui
            .add(egui::Slider::new(&mut v, 0.0..=1.0))
            .on_hover_text(t!("textures_panel.merge_tip"))
            .changed()
        {
            let _ = config_manager.update_param(ConfigPath::EscapeTextureOverlayMerge, v.into());
        }
        ui.end_row();

        ui.label(t!("textures_panel.power"));
        let mut v = ov.power;
        let (lo, hi) = crate::config::escape::OVERLAY_POWER_RANGE;
        if ui
            .add(egui::Slider::new(&mut v, lo..=hi).step_by(1.0).fixed_decimals(0))
            .on_hover_text(t!("textures_panel.power_tip"))
            .changed()
        {
            let _ = config_manager.update_param(ConfigPath::EscapeTextureOverlayPower, v.into());
        }
        ui.end_row();

        ui.label(t!("textures_panel.ratio"));
        let mut v = ov.ratio;
        let (lo, hi) = crate::config::escape::OVERLAY_RATIO_RANGE;
        if ui
            .add(egui::Slider::new(&mut v, lo..=hi).suffix("%"))
            .on_hover_text(t!("textures_panel.ratio_tip"))
            .changed()
        {
            let _ = config_manager.update_param(ConfigPath::EscapeTextureOverlayRatio, v.into());
        }
        ui.end_row();

        ui.label(t!("textures_panel.fit"));
        ui.horizontal(|ui| {
            for (fit, label, tip) in [
                (TextureFit::Stretch, t!("textures_panel.fit_stretch"), t!("textures_panel.fit_stretch_tip")),
                (TextureFit::Tile, t!("textures_panel.fit_tile"), t!("textures_panel.fit_tile_tip")),
            ] {
                if ui.selectable_label(ov.fit == fit, label).on_hover_text(tip).clicked() && ov.fit != fit {
                    let _ = config_manager.update_param(
                        ConfigPath::EscapeTextureOverlayFit,
                        ConfigValue::String(fit.as_str().to_string()),
                    );
                }
            }
        });
        ui.end_row();

        if ov.fit == TextureFit::Tile {
            ui.label(t!("textures_panel.tile_scale"));
            let mut v = ov.tile_scale;
            let (lo, hi) = crate::config::escape::OVERLAY_TILE_RANGE;
            if ui
                .add(egui::Slider::new(&mut v, lo..=hi).logarithmic(true))
                .on_hover_text(t!("textures_panel.tile_scale_tip"))
                .changed()
            {
                let _ = config_manager.update_param(ConfigPath::EscapeTextureOverlayTile, v.into());
            }
            ui.end_row();
        }
    });
}

/// The relief's bump: the texture as the relief's surface texture. The
/// same setting as the Escape panel's surface texture, offered here too.
fn bump_controls(ui: &mut egui::Ui, config_manager: &mut ConfigManager) {
    let sh = config_manager.config().escape.shading.clone();
    let mut on = sh.texture_kind == ShadingTexture::Simulation;
    if ui
        .checkbox(&mut on, t!("textures_panel.bump"))
        .on_hover_text(t!("textures_panel.bump_tip"))
        .changed()
    {
        let kind = if on { ShadingTexture::Simulation } else { ShadingTexture::None };
        let _ = config_manager.update_param(
            ConfigPath::EscapeShadingTextureKind,
            ConfigValue::String(kind.as_str().to_string()),
        );
    }
    if !on {
        return;
    }
    if !sh.enabled {
        ui.weak(t!("textures_panel.bump_needs_relief"));
    }
    egui::Grid::new("texture_bump").num_columns(2).show(ui, |ui| {
        ui.label(t!("escape_panel.texture_strength"));
        let mut v = sh.texture_strength;
        if ui
            .add(egui::Slider::new(&mut v, 0.0..=4.0))
            .on_hover_text(t!("escape_panel.tooltip_texture_strength"))
            .changed()
        {
            let _ = config_manager.update_param(ConfigPath::EscapeShadingTextureStrength, v.into());
        }
        ui.end_row();
        ui.label(t!("escape_panel.texture_scale"));
        let mut v = sh.texture_scale;
        if ui
            .add(egui::Slider::new(&mut v, 0.25..=64.0).logarithmic(true))
            .on_hover_text(t!("escape_panel.tooltip_texture_scale"))
            .changed()
        {
            let _ = config_manager.update_param(ConfigPath::EscapeShadingTextureScale, v.into());
        }
        ui.end_row();
    });
}

/// Put a texture in the config (or take it out), as one undo step.
fn set_texture(config_manager: &mut ConfigManager, texture: Option<EscapeTexture>, description: &str) {
    let before = config_manager.config().clone();
    let mut after = before.clone();
    after.escape.texture = texture;
    let change = ConfigChange::full_config_snapshot(before, after, description.to_string());
    if let Err(e) = config_manager.apply_structural_change(change) {
        log::error!("could not set the texture: {e:?}");
    }
}
