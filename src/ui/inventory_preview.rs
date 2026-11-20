//! The Catalog's previews (docs/archive/projects/catalog.md, *Phase 6*): a picture
//! of what an entry is, made by building the config that shows it and
//! rendering it with the gallery's machinery (`renderer::render_preview`),
//! lazily -- what the info pane and the grid ask for this frame, one a
//! frame -- and kept in the shared `ThumbnailCache`.
//!
//! A config per kind, through what Use does (a scratch `ConfigManager`
//! and `inventory::use_entry`), so a preview is what choosing the entry
//! would show:
//!
//! - a variation: its image of the plane (decision 2) -- four transforms
//!   that fill a square evenly, coloured by quadrant, and the variation
//!   alone as the final transform;
//! - an effect: a reference fractal before and after;
//! - a formula, field or solid: its default view; a colouring: the
//!   default formula drawn with it;
//! - a simulation model or colouring: its first layer, run to a capped
//!   step count;
//! - a generator: its run at seed 1; a modifier: the reference fractal,
//!   modified at seed 1.
//!
//! The key is the configs' hash with the entry's version, so an update or
//! an edit makes a new preview.
//!
//! One set of previews serves the Catalog and every picker ([`with`]): its
//! frame begins in the UI's frame, before any panel draws
//! ([`begin_frame`]), and the app makes one a frame of what was asked.

use crate::config::{ConfigManager, FractalConfig};
use crate::inventory::{Entry, EntryKey, Family, Kind};
use std::collections::{HashMap, HashSet};

/// The registry names registered only to draw a preview (a file of the
/// user's or an example, not a plug-in): the pickers do not offer them.
/// The record an entry previews by: its file's, compiled
/// (`inventory::plugin_of`), for a Rhai file of a kind a list uses.
fn plugin_for(e: &Entry) -> Option<crate::storage::library::Plugin> {
    super::inventory::plugin_of(e)
}

/// What a preview renders, and the key its pictures are kept under.
#[derive(Clone)]
pub struct Preview {
    /// One config; an effect's two (before, after).
    pub configs: Vec<FractalConfig>,
    pub key: String,
    /// The key is the picture's own key in the cache: a preset's, as the
    /// presets' gallery cached it (`ThumbnailCache::config_hash`), so the
    /// pictures made before are found.
    pub raw: bool,
}

impl Preview {
    /// Picture `i`'s key in the thumbnail cache.
    fn image(&self, i: usize) -> String {
        if self.raw { self.key.clone() } else { image_key(&self.key, i) }
    }
}

/// How far a simulation's preview runs at most. A preview runs its
/// preset's measured steps (the picture is the run at that count), but
/// on desktop a preview renders on the UI's thread: two presets measure
/// 200,000 and 2,000,000 steps, and every other first preset 24,000 or
/// fewer.
const SIM_STEPS: u32 = 25_000;
/// A simulation preview's grid side, in cells (the picture is the
/// preview's size, the grid resolved down to it).
const SIM_GRID: u32 = 512;
/// A variation's preview's zoom: the frame about 4.4 units wide, so the
/// square it starts from fills half of it and most variations land within
/// it (the far-flung ones show their core).
const VARIATION_ZOOM: f32 = 0.9;
/// A colouring variation's zoom, closer: what it draws is often a unit
/// square of its own (the GLSL generators), whose pattern the wider frame
/// averages away.
const COLOUR_ZOOM: f32 = 1.8;

/// The families whose entries have previews (the grid's toggle).
pub fn has_previews(family: Family) -> bool {
    matches!(family, Family::Fractals | Family::Variations | Family::Effects | Family::Escape | Family::Simulation | Family::Scripts)
}

/// The reference fractal an effect is shown on and a modifier modifies:
/// the first preset, a flame.
fn reference() -> FractalConfig {
    crate::scene::presets::global_preset_library()
        .presets()
        .iter()
        .find(|p| p.render_mode == crate::scene::transforms::RenderMode::TwoD && !p.flame.transforms.is_empty())
        .cloned()
        .unwrap_or_default()
}

/// A variation's image of the plane: four transforms, each a quarter of
/// the square [-1, 1]^2 with its own colour, so the chaos game fills the
/// square evenly; the variation alone as the final transform.
fn plane(name: &str, three_d: bool, ctx: &egui::Context) -> Option<FractalConfig> {
    use crate::scene::transforms::{RenderMode, Transform};
    use crate::variations::{definition::Feature, VariationPhase};
    let (pre_post, colours) = {
        let reg = crate::variations::global_registry();
        let v = reg.get(name)?;
        (matches!(v.phase, VariationPhase::Pre | VariationPhase::Post), v.has_feature(Feature::WritesColor) || v.has_feature(Feature::WritesRgb))
    };
    let mut config = FractalConfig::default();
    config.render_mode = if three_d { RenderMode::ThreeD } else { RenderMode::TwoD };
    config.flame.transforms.clear();
    for (i, (e, f)) in [(-0.5, -0.5), (0.5, -0.5), (-0.5, 0.5), (0.5, 0.5)].into_iter().enumerate() {
        let mut t = Transform::new();
        t.a = 0.5;
        t.b = 0.0;
        t.c = 0.0;
        t.d = 0.5;
        t.e = e;
        t.f = f;
        t.weight = 1.0;
        t.color = i as f32 / 3.0;
        t.color_speed = 0.0;
        if !colours {
            t.set_variation("linear", 1.0);
            // The final applies to the transforms that attach it.
            t.final_attachments = vec![0];
        }
        config.flame.transforms.push(t);
    }
    if !colours {
        let mut fin = Transform::new();
        // A pre or post variation moves the point around the normal ones:
        // alone, nothing would carry it through.
        if pre_post {
            fin.set_variation("linear", 1.0);
        }
        config.flame.final_transforms = vec![fin];
    }
    config.zoom = VARIATION_ZOOM;
    config.palette = plane_palette();
    if colours {
        // A variation's own colours, dense on its square: their hue kept
        // where they are bright, not clipped to white.
        config.highlight_mode = crate::scene::tonemap::HighlightMode::MaxNorm;
        config.zoom = COLOUR_ZOOM;
    }
    let _ = ctx;
    let mut cm = ConfigManager::new(config);
    if colours {
        // A final's colour writes are discarded (colour is settled before
        // the final chain), so a variation that colours -- direct colour,
        // the GLSL generators -- runs in the quarters themselves: the
        // square, filled and coloured by it.
        for i in 0..4 {
            super::transforms::add_variation(&mut cm, crate::config::TransformRef::Normal(i), name).ok()?;
        }
    } else {
        super::transforms::add_variation(&mut cm, crate::config::TransformRef::Final(0), name).ok()?;
    }
    Some(cm.active_config().clone())
}

/// The previews' palette (where the entry does not choose its own): Fire
/// without its black end, which on the black background hid whatever was
/// drawn at that end -- dark red, orange, yellow, cream, evenly spaced.
fn plane_palette() -> crate::scene::palette::Palette {
    let mut fire = crate::scene::palette::Palette::fire();
    fire.stops.retain(|s| s.color.iter().any(|c| *c > 0.05));
    let n = fire.stops.len().max(2) - 1;
    for (i, s) in fire.stops.iter_mut().enumerate() {
        s.position = i as f32 / n as f32;
    }
    fire.name = "Catalog preview".to_string();
    fire.built_in = false;
    fire
}

/// The config that shows `e`, through what Use does; `None` for a kind
/// with no picture of its own.
fn configs(e: &Entry, ctx: &egui::Context, palettes: &[crate::scene::palette::Palette]) -> Option<Vec<FractalConfig>> {
    let gpu = matches!(e.kind, Kind::Variation | Kind::Effect | Kind::Formula | Kind::Coloring | Kind::Field | Kind::FieldColoring | Kind::Solid | Kind::SolidColoring | Kind::SimModel | Kind::SimColoring);
    let plugin = if gpu { plugin_for(e) } else { None };
    let usable = e.installed && (matches!(e.key, EntryKey::Registered(..)) || plugin.is_some());
    match e.kind {
        Kind::Variation if usable => {
            let name = plugin.as_ref().map(|p| p.key.clone()).or_else(|| super::inventory::applied_name(e))?;
            let three_d = crate::variations::global_registry().get(&name).is_some_and(|v| v.category == crate::variations::VariationCategory::Only3D);
            Some(vec![plane(&name, three_d, ctx)?])
        }
        Kind::Effect if usable => {
            let before = reference();
            let mut cm = ConfigManager::new(before.clone());
            super::inventory::use_entry_with(e, plugin, ctx, &mut cm);
            let after = cm.active_config().clone();
            (after.color_effects.len() != before.color_effects.len() || after.density_effects.len() != before.density_effects.len()).then(|| vec![before, after])
        }
        Kind::Formula | Kind::Coloring | Kind::Field | Kind::FieldColoring | Kind::Solid | Kind::SolidColoring | Kind::SimModel | Kind::SimColoring if usable => {
            let mut cm = ConfigManager::new(FractalConfig::default());
            // A colouring draws something: the default formula, field or
            // solid first (a built-in's pick does this itself; a file's
            // plug-in path takes what the config has).
            match e.kind {
                Kind::Coloring => {
                    if let Some(f) = crate::escape::FORMULAS.first() {
                        super::escape_panel::choose_formula(&mut cm, f);
                    }
                    // The colouring's own defaults: the formula's preset
                    // tuned its colouring's parameters (`scale` 0.03 for
                    // the Mandelbrot's), which another colouring with a
                    // parameter of that name would read.
                    let mut config = cm.active_config().clone();
                    config.escape.coloring_params.clear();
                    cm = ConfigManager::new(config);
                }
                Kind::FieldColoring => {
                    if let Some(f) = crate::escape::fields::FIELDS.first() {
                        super::escape_panel::choose_field(&mut cm, f);
                    }
                }
                // A colouring on the model and preset that name it (a
                // preset names the colouring it is to be seen through):
                // Gray-Scott's spots were the wrong picture for most.
                Kind::SimColoring => {
                    let name = plugin.as_ref().map(|p| p.key.clone()).or_else(|| super::inventory::applied_name(e)).unwrap_or_default();
                    if let Some((m, pre)) = super::sim_panel::preset_for_coloring(&name) {
                        super::sim_panel::choose_first_layer_model(&mut cm, m);
                        super::sim_panel::apply_first_layer_preset(&mut cm, m, pre);
                    }
                }
                Kind::SolidColoring => {
                    // A solid it draws -- not the flame attractor, which a
                    // default config has no flame for.
                    let name = super::inventory::applied_name(e).unwrap_or_default();
                    let coloring = crate::escape::ifs::IFS_COLORINGS.iter().find(|c| c.name == name);
                    let solid = crate::escape::ifs::IFS_DEFS
                        .iter()
                        .filter(|d| !d.name.starts_with("ifs_flame"))
                        .find(|d| coloring.is_none_or(|c| crate::escape::ifs::coloring_applies(d, c)));
                    if let Some(d) = solid {
                        super::escape_panel::choose_solid(&mut cm, d);
                    }
                }
                _ => {}
            }
            super::inventory::use_entry_with(e, plugin, ctx, &mut cm);
            // A model at its first preset: its parameters, time step,
            // boundary, initial field, steps and colouring -- one default
            // field and colouring drew nothing for most models.
            if e.kind == Kind::SimModel {
                let model = crate::sim::model_or_default(&cm.active_config().sim.model);
                if let Some(pre) = model.presets.first() {
                    super::sim_panel::apply_first_layer_preset(&mut cm, model, pre);
                }
            }
            let mut config = cm.active_config().clone();
            if config.render_mode == crate::scene::transforms::RenderMode::Simulation {
                config.sim.steps = config.sim.steps.min(SIM_STEPS);
                // The grid the presets were measured at the scale of: on
                // the preview's own 256 cells SmoothLife's gliders die.
                config.sim.grid = crate::config::sim::SimGrid::Fixed { width: SIM_GRID, height: SIM_GRID };
            }
            // The default palette's black end on the black background hid
            // whatever an entry draws at index 0 (an orbit trap near the
            // origin, a formula's first bands).
            config.palette = plane_palette();
            Some(vec![config])
        }
        Kind::Generator | Kind::Modifier => {
            let EntryKey::File { store: crate::storage::library::Store::Scripts, file, shipped } = &e.key else { return None };
            let text = match shipped {
                true => crate::script::library::EMBEDDED.iter().find(|(n, _)| n == file).map(|(_, s)| s.to_string()),
                false => crate::storage::manage::load(crate::storage::library::Store::Scripts, file),
            }?;
            let untrusted = !shipped && e.link.as_ref().is_some_and(|l| l.from_others);
            let base = if e.kind == Kind::Modifier { reference() } else { FractalConfig::default() };
            let mut host = crate::script::ScriptHost::with_palettes(palettes.to_vec());
            if untrusted {
                host = host.with_untrusted_entry();
            }
            let outcome = host.run(&text, &base, 1, HashMap::new()).ok()?;
            Some(vec![outcome.config])
        }
        _ => None,
    }
}

/// The preview of `e`, or `None` for an entry with none.
pub fn preview(e: &Entry, ctx: &egui::Context, palettes: &[crate::scene::palette::Palette]) -> Option<Preview> {
    // A preset is its own picture.
    if let EntryKey::Preset(name) = &e.key {
        let config = crate::inventory::preset(name)?;
        let key = crate::storage::thumbnail_cache::ThumbnailCache::config_hash(&config);
        return Some(Preview { configs: vec![config], key, raw: true });
    }
    let configs = configs(e, ctx, palettes)?;
    let mut seed = String::from("catalog-preview-1");
    for c in &configs {
        seed.push_str(&crate::storage::thumbnail_cache::ThumbnailCache::config_hash(c));
    }
    seed.push_str(e.version.as_deref().unwrap_or(""));
    Some(Preview { configs, key: hash16(&seed), raw: false })
}

fn hash16(s: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.bytes() {
        h ^= b as u64;
        h = h.wrapping_mul(0x100000001b3);
    }
    format!("{h:016x}")
}

/// One picture's key in the cache: the preview's, and which of its
/// configs.
fn image_key(key: &str, i: usize) -> String {
    hash16(&format!("{key}/{i}"))
}

/// Where a preview stands.
pub enum Shown {
    /// The entry has no picture of its own.
    None,
    /// Being made.
    Pending,
    /// Its pictures (an effect's two).
    Ready(Vec<egui::TextureHandle>),
}

thread_local! {
    /// The previews, the Catalog's and the pickers' (the UI is one thread).
    static SHARED: std::cell::RefCell<Previews> = std::cell::RefCell::new(Previews::default());
}

/// The shared previews, for one call (not re-entered: building a preview
/// draws no picker).
pub fn with<R>(f: impl FnOnce(&mut Previews) -> R) -> R {
    SHARED.with(|p| f(&mut p.borrow_mut()))
}

/// A new frame for the shared previews: called once a frame before any
/// panel draws.
pub fn begin_frame(ctx: &egui::Context) {
    with(|p| p.begin_frame(ctx));
}

/// The preview of `e` from the shared previews, asking for it when it is
/// not made.
pub fn want(e: &Entry, ctx: &egui::Context, palettes: &[crate::scene::palette::Palette]) -> Shown {
    with(|p| p.want(e, ctx, palettes))
}

/// The previews asked for, made and shown.
#[derive(Default)]
pub struct Previews {
    /// Each entry's preview, built once (an entry without one: `None`).
    built: HashMap<(EntryKey, Option<String>), Option<Preview>>,
    textures: HashMap<String, Vec<egui::TextureHandle>>,
    /// Asked for this frame and not made: rendered first-asked first.
    wanted: Vec<Preview>,
    failed: HashSet<String>,
    /// How many previews were asked for this frame (made or not).
    asked: usize,
    #[cfg(target_arch = "wasm32")]
    in_flight: Option<String>,
    #[cfg(target_arch = "wasm32")]
    landed: std::rc::Rc<std::cell::RefCell<Vec<(String, Vec<image::RgbaImage>)>>>,
}

fn upload(ctx: &egui::Context, key: &str, images: &[image::RgbaImage]) -> Vec<egui::TextureHandle> {
    images
        .iter()
        .enumerate()
        .map(|(i, img)| {
            let color = egui::ColorImage::from_rgba_unmultiplied([img.width() as usize, img.height() as usize], img.as_raw());
            ctx.load_texture(format!("catalog_preview_{key}_{i}"), color, egui::TextureOptions::LINEAR)
        })
        .collect()
}

impl Previews {
    /// A new frame: what was asked for last frame is asked again by what
    /// is still on screen.
    pub fn begin_frame(&mut self, ctx: &egui::Context) {
        self.wanted.clear();
        self.asked = 0;
        #[cfg(target_arch = "wasm32")]
        {
            let landed: Vec<_> = self.landed.borrow_mut().drain(..).collect();
            for (key, images) in landed {
                if self.in_flight.as_deref() == Some(key.as_str()) {
                    self.in_flight = None;
                }
                let textures = upload(ctx, &key, &images);
                self.textures.insert(key, textures);
            }
        }
        let _ = ctx;
    }

    /// How many previews were asked for this frame, made or not.
    #[cfg(test)]
    pub fn asked(&self) -> usize {
        self.asked
    }

    /// The preview of `e` as it stands, asking for it when it is not made.
    pub fn want(&mut self, e: &Entry, ctx: &egui::Context, palettes: &[crate::scene::palette::Palette]) -> Shown {
        self.asked += 1;
        let id = (e.key.clone(), e.version.clone());
        // A file not compiled yet: compiled before the next frame (never
        // inside a draw, `inventory::compile_requested`); not "no preview".
        if !self.built.contains_key(&id) && super::inventory::compile_pending(e) {
            ctx.request_repaint();
            return Shown::Pending;
        }
        if !self.built.contains_key(&id) {
            let p = preview(e, ctx, palettes);
            self.built.insert(id.clone(), p);
        }
        let Some(p) = self.built.get(&id).cloned().flatten() else { return Shown::None };
        if let Some(t) = self.textures.get(&p.key) {
            return Shown::Ready(t.clone());
        }
        if self.failed.contains(&p.key) {
            return Shown::None;
        }
        // Made before (this run or another): from the cache.
        let mut cache = crate::storage::thumbnail_cache::shared_thumbnail_cache();
        let cached: Option<Vec<image::RgbaImage>> = (0..p.configs.len()).map(|i| cache.load(&p.image(i))).collect();
        drop(cache);
        if let Some(images) = cached {
            let t = upload(ctx, &p.key, &images);
            self.textures.insert(p.key.clone(), t.clone());
            return Shown::Ready(t);
        }
        if !self.wanted.iter().any(|w| w.key == p.key) {
            self.wanted.push(p);
        }
        ctx.request_repaint();
        Shown::Pending
    }

    /// Desktop: make the first preview asked for (one a frame, after the
    /// UI). Gives whether one was made.
    #[cfg(not(target_arch = "wasm32"))]
    pub fn generate_one<F>(&mut self, ctx: &egui::Context, render: F) -> bool
    where
        F: Fn(&FractalConfig) -> image::RgbaImage,
    {
        let Some(p) = self.wanted.first().cloned() else { return false };
        self.wanted.remove(0);
        let images: Vec<image::RgbaImage> = p.configs.iter().map(&render).collect();
        let mut cache = crate::storage::thumbnail_cache::shared_thumbnail_cache();
        for (i, img) in images.iter().enumerate() {
            if let Err(e) = cache.save(&p.image(i), img) {
                log::info!("catalog preview not cached: {e}");
            }
        }
        drop(cache);
        let t = upload(ctx, &p.key, &images);
        self.textures.insert(p.key, t);
        true
    }

    /// The web: start the first preview asked for, one at a time.
    #[cfg(target_arch = "wasm32")]
    pub fn start_next(&mut self, device: &egui_wgpu::wgpu::Device, queue: &egui_wgpu::wgpu::Queue, ctx: &egui::Context) {
        if self.in_flight.is_some() {
            return;
        }
        let Some(p) = self.wanted.first().cloned() else { return };
        self.in_flight = Some(p.key.clone());
        let (device, queue, ctx, landed) = (device.clone(), queue.clone(), ctx.clone(), self.landed.clone());
        wasm_bindgen_futures::spawn_local(async move {
            let mut images = Vec::new();
            for c in &p.configs {
                images.push(crate::renderer::render_preview_async(&device, &queue, c).await);
            }
            landed.borrow_mut().push((p.key, images));
            ctx.request_repaint();
        });
    }
}

/// An entry's preview in the info pane: its pictures (an effect's before
/// and after, captioned), a spinner while it is made.
pub fn show(ui: &mut egui::Ui, shown: &Shown) {
    match shown {
        Shown::None => {}
        Shown::Pending => {
            ui.horizontal(|ui| {
                ui.spinner();
                ui.weak(t!("catalog.preview_making"));
            });
        }
        Shown::Ready(textures) => {
            // Exactly the width there is, with the row's own spacing: a
            // guessed gap 2 points short made an effect's pair 2 points too
            // wide, the pane grew to hold it, the pictures grew with the
            // pane -- every frame, up to 256 each, shifting the list under
            // the pointer. Never wider than the room, so never a minimum.
            let n = textures.len().max(1) as f32;
            let spacing = ui.spacing().item_spacing.x;
            let side = ((ui.available_width() - spacing * (n - 1.0)) / n).floor().clamp(1.0, 256.0);
            ui.horizontal(|ui| {
                for (i, t) in textures.iter().enumerate() {
                    ui.vertical(|ui| {
                        ui.add(egui::Image::new(t).fit_to_exact_size(egui::vec2(side, side)));
                        if textures.len() == 2 {
                            ui.weak(if i == 0 { t!("catalog.preview_before") } else { t!("catalog.preview_after") });
                        }
                    });
                }
            });
        }
    }
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;

    /// An effect's two pictures fit the width they are given, frame after
    /// frame (reported: the Catalog's info pane grew while an effect was
    /// pointed at, shifting the list).
    #[test]
    fn two_pictures_fit_their_pane_and_it_keeps_its_width() {
        let ctx = egui::Context::default();
        let texture = ctx.load_texture("t", egui::ColorImage::new([4, 4], vec![egui::Color32::WHITE; 16]), egui::TextureOptions::LINEAR);
        let shown = Shown::Ready(vec![texture.clone(), texture]);
        let mut widths = Vec::new();
        for _ in 0..6 {
            let input = egui::RawInput { screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(1000.0, 700.0))), ..Default::default() };
            let _ = ctx.run_ui(input, |ui| {
                let r = egui::Panel::right("pane_test").resizable(true).default_size(280.0).min_size(200.0).show_inside(ui, |ui| show(ui, &shown));
                widths.push(r.response.rect.width());
            });
        }
        assert!(widths.windows(2).all(|w| w[0] == w[1]), "the pane keeps its width: {widths:?}");
        assert!(widths[0] <= 281.0, "its default: {widths:?}");
    }

    fn entry(kind: Kind, name: &str) -> Entry {
        crate::inventory::scan(&crate::scene::palette::global_palette_library().read().unwrap().clone(), crate::inventory::Online::default())
            .into_iter()
            .find(|e| e.kind == kind && e.key == EntryKey::Registered(kind, name.to_string()))
            .unwrap_or_else(|| panic!("no {kind:?} {name}"))
    }

    /// A variation's preview is its image of the plane: four even
    /// quarters and the variation alone as the final transform; an
    /// effect's is the same fractal before and after; a formula's its
    /// default view; the key follows the config.
    #[test]
    fn previews_are_what_using_the_entry_shows() {
        let ctx = egui::Context::default();
        let p = preview(&entry(Kind::Variation, "swirl"), &ctx, &[]).expect("a variation's preview");
        let flame = &p.configs[0].flame;
        assert_eq!(flame.transforms.len(), 4);
        assert_eq!(flame.final_transforms[0].variations.get("swirl"), Some(&1.0));
        assert_eq!(flame.final_transforms[0].variations.len(), 1, "the variation alone");
        assert!(flame.transforms.iter().all(|t| t.final_attachments == [0]), "every quarter goes through it");
        assert!(p.configs[0].palette.stops.iter().all(|s| s.color.iter().any(|c| *c > 0.05)), "no black on the black background");

        let other = preview(&entry(Kind::Variation, "julia"), &ctx, &[]).unwrap();
        assert_ne!(p.key, other.key);
        assert_eq!(p.key, preview(&entry(Kind::Variation, "swirl"), &ctx, &[]).unwrap().key, "the same entry, the same key");

        let effect = crate::effects::global_effect_registry().all().next().map(|i| (i.name.clone(), i.category)).unwrap();
        let p = preview(&entry(Kind::Effect, &effect.0), &ctx, &[]).expect("an effect's preview");
        assert_eq!(p.configs.len(), 2, "before and after");

        let p = preview(&entry(Kind::Formula, "mandelbrot"), &ctx, &[]).expect("a formula's preview");
        assert_eq!(p.configs[0].render_mode, crate::scene::transforms::RenderMode::Escape);
        assert_eq!(p.configs[0].escape.formula, "mandelbrot");

        // A model at its first preset; a colouring on the preset that
        // names it.
        for m in crate::sim::MODELS {
            let Some(pre) = m.presets.first() else { continue };
            let p = preview(&entry(Kind::SimModel, m.name), &ctx, &[]).expect("a model's preview");
            let c = &p.configs[0];
            assert_eq!(c.sim.steps, pre.steps.min(SIM_STEPS), "{}: its preset's steps, capped", m.name);
            assert_eq!(c.sim.boundary, pre.boundary, "{}: its preset's boundary", m.name);
            if let Some(col) = pre.coloring {
                assert_eq!(c.sim.coloring, col, "{}: its preset's colouring", m.name);
            }
        }
        let (m, _) = super::super::sim_panel::preset_for_coloring("age").expect("a preset names age");
        let p = preview(&entry(Kind::SimColoring, "age"), &ctx, &[]).expect("a colouring's preview");
        assert_eq!((p.configs[0].sim.model.as_str(), p.configs[0].sim.coloring.as_str()), (m.name, "age"));
    }
}

#[cfg(all(test, not(target_arch = "wasm32"), feature = "terrain"))]
mod inspection {
    use super::*;

    /// Renders previews for a look, through the preview path itself, into
    /// `output/previews/`: one PNG an entry and a contact sheet.
    /// `PREVIEWS="variation:swirl;effect:vignette;formula:mandelbrot"`, or
    /// a sample of every kind when unset.
    #[test]
    #[ignore = "inspection; needs a GPU"]
    fn render_previews_for_inspection() {
        let (device, queue) = crate::escape::terrain::gpu_tests::device().expect("gpu");
        device.on_uncaptured_error(std::sync::Arc::new(|e| println!("wgpu error: {e}")));
        let ctx = egui::Context::default();
        let library = crate::scene::palette::global_palette_library().read().unwrap().clone();
        let entries = crate::inventory::scan(&library, crate::inventory::Online::default());
        let wanted = std::env::var("PREVIEWS").unwrap_or_else(|_| {
            "variation:linear;variation:swirl;variation:julia;variation:spherical;variation:bubble;variation:waves;variation:heart;variation:polar;\
             effect:vignette;formula:mandelbrot;formula:burning_ship;coloring:smooth;sim_model:gray_scott;generator:basic_random"
                .to_string()
        });
        let kind_of = |w: &str| match w {
            "variation" => Some(Kind::Variation),
            "effect" => Some(Kind::Effect),
            "formula" => Some(Kind::Formula),
            "coloring" => Some(Kind::Coloring),
            "field" => Some(Kind::Field),
            "solid" => Some(Kind::Solid),
            "sim_model" => Some(Kind::SimModel),
            "sim_coloring" => Some(Kind::SimColoring),
            "field_coloring" => Some(Kind::FieldColoring),
            "solid_coloring" => Some(Kind::SolidColoring),
            "generator" => Some(Kind::Generator),
            "modifier" => Some(Kind::Modifier),
            _ => None,
        };
        let dir = std::path::Path::new("output/previews");
        std::fs::create_dir_all(dir).unwrap();
        let mut sheet: Vec<image::RgbaImage> = Vec::new();
        for item in wanted.split(';').filter(|s| !s.is_empty()) {
            let (k, name) = item.split_once(':').expect("kind:name");
            let kind = kind_of(k).unwrap_or_else(|| panic!("kind {k}"));
            let e = entries
                .iter()
                .find(|e| e.kind == kind && (e.key == EntryKey::Registered(kind, name.into()) || matches!(&e.key, EntryKey::File { file, .. } if file.trim_end_matches(".rhai") == name)))
                .unwrap_or_else(|| panic!("no {item}"));
            if super::super::inventory::compile_pending(e) {
                super::super::inventory::compile_requested();
            }
            let Some(p) = preview(e, &ctx, &[]) else {
                println!("{item}: no preview");
                continue;
            };
            for (i, c) in p.configs.iter().enumerate() {
                let t = std::time::Instant::now();
                let img = crate::renderer::render_preview(&device, &queue, c);
                println!("{item}[{i}] in {} ms", t.elapsed().as_millis());
                img.save(dir.join(format!("{k}-{name}-{i}.png"))).unwrap();
                std::fs::write(dir.join(format!("{k}-{name}-{i}.fflame")), c.to_json().unwrap_or_default()).unwrap();
                sheet.push(img);
            }
        }
        let cols = 6u32;
        let rows = (sheet.len() as u32).div_ceil(cols);
        let mut out = image::RgbaImage::from_pixel(cols * 260, rows * 260, image::Rgba([40, 40, 40, 255]));
        for (i, img) in sheet.iter().enumerate() {
            let (x, y) = ((i as u32 % cols) * 260 + 2, (i as u32 / cols) * 260 + 2);
            image::imageops::overlay(&mut out, img, x as i64, y as i64);
        }
        out.save(dir.join("sheet.png")).unwrap();
    }
}
