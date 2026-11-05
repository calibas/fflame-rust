//! The tail every generator's picture goes through: density effects, the
//! tone map into the renderer's picture, then colour effects
//! (docs/projects/architecture-review.md, 2.3).
//!
//! The flame, escape-time and simulation generators each leave an
//! `Rgba32Float` image in the accumulator's layout; from there the
//! stages are the same, and so is their order. They were written out
//! once per path -- the headless flame, the simulation, the escape
//! picture and terrain, the viewport -- and the two video loops that
//! existed until 2026-09-09 disagreed about density effects and depth of
//! field for that reason. A new tail stage goes here, once.
//!
//! What stays with each caller is what differs between them: the
//! viewport records into its frame's encoder with its persistent effect
//! chain and shows the picture; a headless render
//! (`render::finish`) owns a chain for the call and reads the pixels
//! back. `HighResExporter` tone maps its own histogram, in strips when
//! it is large, and keeps its own sequence.

use crate::config::FractalConfig;
use crate::effects::EffectInstance;
use crate::renderer::compute_kernel::FlameRenderer;
use crate::renderer::effect_chain::EffectChainRunner;
use wgpu::{CommandEncoder, Device, Queue, TextureView};

/// The image the tail starts from.
#[derive(Clone, Copy)]
pub enum Picture<'v> {
    /// The renderer's accumulator: a flame as the chaos game left it.
    Accumulator,
    /// The renderer's solid shade pass output.
    Shaded,
    /// The renderer's depth-of-field output.
    Focused,
    /// Another generator's image in the accumulator's layout: an escape
    /// picture or terrain, a simulation.
    Image(&'v TextureView),
}

impl<'v> Picture<'v> {
    /// A flame's picture after its 3D passes: depth of field over the
    /// shade pass over the accumulator, whichever ran last.
    pub fn flame(shade_ran: bool, dof_ran: bool) -> Self {
        if dof_ran {
            Self::Focused
        } else if shade_ran {
            Self::Shaded
        } else {
            Self::Accumulator
        }
    }

    fn view<'a>(&self, renderer: &'a FlameRenderer) -> &'a TextureView
    where
        'v: 'a,
    {
        match *self {
            Self::Accumulator => renderer.get_accumulation_view(),
            Self::Shaded => renderer.shade_output_view(),
            Self::Focused => renderer.dof_output_view(),
            Self::Image(v) => v,
        }
    }
}

/// A config's tail: its effects, and what a density effect measures a
/// pixel's alpha against.
pub struct Tail<'c> {
    density_effects: &'c [EffectInstance],
    color_effects: &'c [EffectInstance],
    density_mean: f32,
}

impl<'c> Tail<'c> {
    /// `density_mean`: a flame's alpha is a hit count, measured against
    /// its mean (`FlameRenderer::mean_density`); an escape or simulation
    /// image's is not, and is read as it is (1).
    pub fn new(config: &'c FractalConfig, density_mean: f32) -> Self {
        Self { density_effects: &config.density_effects, color_effects: &config.color_effects, density_mean }
    }

    pub fn has_density_effects(&self) -> bool {
        EffectChainRunner::has_enabled_effects(self.density_effects)
    }

    pub fn has_color_effects(&self) -> bool {
        EffectChainRunner::has_enabled_effects(self.color_effects)
    }

    /// Whether the tail needs an effect chain at all.
    pub fn has_effects(&self) -> bool {
        self.has_density_effects() || self.has_color_effects()
    }

    /// Density effects on `picture` (before the tone map, on HDR data),
    /// then the tone map of their output -- or of `picture` when none ran
    /// -- into the renderer's picture. The accumulator goes through the
    /// renderer's own bind group, any other image through one made for
    /// it.
    pub fn tone_map(
        &self,
        renderer: &FlameRenderer,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        chain: Option<&mut EffectChainRunner>,
        picture: Picture<'_>,
    ) {
        let input = picture.view(renderer);
        let density_output = chain.and_then(|chain| self.density(device, queue, encoder, chain, input));
        match (density_output, picture) {
            (Some(output), _) => renderer.tonemap_pass_with_input(device, queue, encoder, output),
            (None, Picture::Accumulator) => renderer.tonemap_pass(queue, encoder),
            (None, _) => renderer.tonemap_pass_with_input(device, queue, encoder, input),
        }
    }

    /// Colour effects on the tone-mapped picture. Whether they ran: if
    /// so, the picture is the chain's colour output, not the renderer's.
    pub fn color(
        &self,
        renderer: &FlameRenderer,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        chain: Option<&mut EffectChainRunner>,
    ) -> bool {
        chain.is_some_and(|chain| self.color_on(device, queue, encoder, chain, renderer.get_fractal_texture_view()))
    }

    /// The density stage alone, on any image in the accumulator's
    /// layout: the effects' output when any ran, for the tone map to
    /// read instead of `input`. For a tone map that is not the
    /// renderer's (`HighResExporter`'s, over its own histogram).
    pub fn density<'a>(
        &self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        chain: &'a mut EffectChainRunner,
        input: &TextureView,
    ) -> Option<&'a TextureView> {
        if chain.run_density_effects(device, queue, encoder, input, self.density_effects, self.density_mean) {
            chain.get_density_output()
        } else {
            None
        }
    }

    /// The colour stage alone, on any tone-mapped picture: whether the
    /// effects ran (the picture is then the chain's colour output).
    pub fn color_on(
        &self,
        device: &Device,
        queue: &Queue,
        encoder: &mut CommandEncoder,
        chain: &mut EffectChainRunner,
        picture: &TextureView,
    ) -> bool {
        chain.run_color_effects(device, queue, encoder, picture, self.color_effects)
    }
}
