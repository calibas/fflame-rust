//! Keeping the viewport's escape renderer holding the texture its config
//! names (`EscapeConfig::texture`, docs/archive/projects/sim-textures.md).
//!
//! The config carries the texture's recipe, never its pixels; the image
//! comes from the cache or is generated. Desktop does that blocking -- a
//! texture picked from the panel is almost always a cache hit, since its
//! preview generated it -- and the web spawns it and polls.
//!
//! Only what may generate on its own (`textures::may_generate`: cached,
//! cheap, or asked for) does. A file whose texture is minutes of
//! simulation and not cached is drawn without it, and says so once
//! (`take_notice`), until the Textures panel's Generate asks for it.
//!
//! Checked only on frames that re-render anyway, plus every frame while
//! a web generation is in flight: the cache key hashes the recipe's
//! JSON, which is not a per-frame cost to pay for nothing.

use crate::config::escape::EscapeConfig;
use crate::escape::EscapeRenderer;
use egui_wgpu::wgpu::{Device, Queue};

#[cfg(target_arch = "wasm32")]
type Slot = std::rc::Rc<std::cell::RefCell<Option<Result<image::RgbaImage, String>>>>;

#[derive(Default)]
pub(crate) struct TextureSync {
    /// A key that could not be generated: not retried on every edit.
    failed: Option<String>,
    /// A texture the picture is drawn without until the user asks for it
    /// (its cache key), and whether that has been said yet.
    waiting: Option<String>,
    notice: Option<String>,
    /// The web: the generation in flight, by cache key.
    #[cfg(target_arch = "wasm32")]
    pending: Option<(String, Slot)>,
}

impl TextureSync {
    /// Whether a generation is in flight, or a waiting texture has just
    /// been asked for (the frame loop keeps turning, and looks again).
    pub(crate) fn busy(&self) -> bool {
        let asked = self.waiting.as_deref().is_some_and(crate::textures::is_approved);
        #[cfg(target_arch = "wasm32")]
        {
            asked || self.pending.is_some()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            asked
        }
    }

    /// The name of a texture the picture has just started being drawn
    /// without, once: for the app to say so.
    pub(crate) fn take_notice(&mut self) -> Option<String> {
        self.notice.take()
    }

    fn drop_pending(&mut self) {
        #[cfg(target_arch = "wasm32")]
        {
            self.pending = None;
        }
    }

    /// Bring the renderer's texture in line with the config. Returns
    /// whether it changed, which is a re-render.
    pub(crate) fn update(&mut self, escape: &mut EscapeRenderer, device: &Device, queue: &Queue, config: &EscapeConfig) -> bool {
        // Every early return drops a generation in flight: nothing will
        // read it, and `busy` would otherwise keep the loop turning.
        let Some(texture) = config.texture.as_ref() else {
            self.drop_pending();
            self.waiting = None;
            return escape.clear_texture();
        };
        // Held while nothing draws with it: switching a use back on is
        // then immediate. Nothing reads it meanwhile.
        if !config.uses_texture() {
            self.drop_pending();
            return false;
        }
        let key = crate::textures::cache::key(&texture.config);
        if escape.texture_key() == Some(key.as_str()) || self.failed.as_deref() == Some(key.as_str()) {
            self.drop_pending();
            self.waiting = None;
            return false;
        }
        // Too long to make without asking: drawn without it, said once.
        if !crate::textures::may_generate(&texture.config, &key) {
            self.drop_pending();
            if self.waiting.as_deref() != Some(key.as_str()) {
                log::info!("texture '{}' is not generated; drawing without it until it is asked for", texture.name);
                self.waiting = Some(key);
                self.notice = Some(texture.name.clone());
            }
            return escape.clear_texture();
        }
        self.waiting = None;

        #[cfg(not(target_arch = "wasm32"))]
        {
            match pollster::block_on(crate::textures::obtain(device, queue, &texture.config)) {
                Ok(image) => escape.set_texture(device, queue, &key, &image),
                Err(e) => {
                    log::warn!("texture '{}' could not be generated: {e}", texture.name);
                    self.failed = Some(key);
                    false
                }
            }
        }

        #[cfg(target_arch = "wasm32")]
        {
            if let Some((pending_key, slot)) = self.pending.take() {
                if pending_key == key {
                    let done = slot.borrow_mut().take();
                    match done {
                        Some(Ok(image)) => return escape.set_texture(device, queue, &key, &image),
                        Some(Err(e)) => {
                            log::warn!("texture '{}' could not be generated: {e}", texture.name);
                            self.failed = Some(key);
                            return false;
                        }
                        None => {
                            self.pending = Some((pending_key, slot));
                            return false;
                        }
                    }
                }
                // A different texture was picked meanwhile: the old
                // generation finishes into a slot nobody reads.
            }
            let slot: Slot = Default::default();
            let (out, recipe) = (std::rc::Rc::clone(&slot), (*texture.config).clone());
            let (device, queue) = (device.clone(), queue.clone());
            wasm_bindgen_futures::spawn_local(async move {
                let image = crate::textures::obtain(&device, &queue, &recipe).await.map_err(|e| e.to_string());
                *out.borrow_mut() = Some(image);
            });
            self.pending = Some((key, slot));
            false
        }
    }
}

/// Saving a simulation as a texture caches its image from the live run,
/// so a texture never needs a first generation -- which for a run of
/// hundreds of thousands of steps is minutes.
///
/// The image must be exactly what generating the recipe would make, or
/// the cache would hold a picture its recipe does not reproduce. So it is
/// made by the same path, `render`, continuing the viewport's simulation
/// as a caller-owned engine: a field already at the recipe's step count
/// is coloured and tone-mapped at the recipe's size, and not stepped.
/// That holds only when the live field IS the recipe's: the same grid,
/// exactly its step count, and every step taken under its settings
/// (`SimRenderer::ran_steadily_as` -- a parameter edited mid-run makes a
/// field no recipe reproduces). Otherwise nothing is cached, and the
/// texture generates as any other when asked for. The run's flame (layer
/// transforms) and a terrain are outside what is checked, so a recipe
/// with either is left to generate too.
#[cfg(all(feature = "engine-sim", not(target_arch = "wasm32")))]
impl super::App {
    pub(super) fn cache_saved_texture_from_run(&mut self, name: &str) {
        let Some(entry) = crate::textures::TextureLibrary::load().find(name).cloned() else {
            return;
        };
        let recipe = entry.config;
        let key = crate::textures::cache::key(&recipe);
        if crate::textures::cache::exists(&key) {
            return;
        }
        let Some(sim) = self.sim_renderer.as_ref() else { return };
        let size = crate::textures::texture_size(&recipe);
        let same_field = sim.grid_size() == size
            && sim.step_index() == recipe.sim.steps
            && sim.ran_steadily_as(&recipe.sim)
            && !recipe.sim.use_transforms
            && !recipe.sim.terrain_active();
        if !same_field {
            log::info!("texture '{name}' saved; its image is not the live run's, so it generates when asked for");
            return;
        }
        let mut engines = crate::renderer::RenderEngines::default();
        engines.sim = self.sim_renderer.take();
        let job = crate::renderer::RenderJob::new(&recipe, size.0, size.1).with_engines(&mut engines);
        let out = pollster::block_on(crate::renderer::render(&self.gpu.device, &self.gpu.queue, job, &mut crate::renderer::NoProgress));
        // Back to the viewport, which resizes its output on the next frame.
        self.sim_renderer = engines.sim.take();
        match out.map_err(|e| e.to_string()).and_then(|o| {
            image::RgbaImage::from_raw(o.width, o.height, o.rgba_data).ok_or_else(|| "wrong byte count".to_string())
        }) {
            Ok(image) => {
                crate::textures::cache::save(&key, &image);
                log::info!("texture '{name}' cached from the live run");
            }
            Err(e) => log::warn!("texture '{name}' could not be cached from the live run: {e}"),
        }
    }
}
