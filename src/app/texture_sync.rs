//! Keeping the viewport's escape renderer holding the texture its config
//! names (`EscapeConfig::texture`, docs/projects/sim-textures.md).
//!
//! The config carries the texture's recipe, never its pixels; the image
//! comes from the cache or is generated. Desktop does that blocking -- a
//! texture picked from the panel is almost always a cache hit, since its
//! preview generated it -- and the web spawns it and polls.
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
    /// The web: the generation in flight, by cache key.
    #[cfg(target_arch = "wasm32")]
    pending: Option<(String, Slot)>,
}

impl TextureSync {
    /// Whether a generation is in flight (the frame loop keeps turning).
    pub(crate) fn busy(&self) -> bool {
        #[cfg(target_arch = "wasm32")]
        {
            self.pending.is_some()
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            false
        }
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
            return false;
        }

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
