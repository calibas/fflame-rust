//! **Render scale** (docs/projects/quality-governor.md): the size the
//! viewport's picture renders at, and the size it is shown at.
//!
//! The viewport panel reports its size in egui's logical points
//! (`fractal_viewport_size`). Shown, the picture covers that many points
//! times `pixels_per_point` physical pixels: the display size. It renders
//! at the logical size (or, with `render_physical_pixels`, the physical
//! one) times `render_scale`, and below the display size the upscale pass
//! (`renderer::upscale`) draws it up to it. At 100% on a 1x screen the
//! two sizes are one and nothing runs.
//!
//! A simulation renders at the logical size whatever the setting: its
//! grid is the content, a bound grid's cells follow this size, and its
//! own grid scale is its render scale (`visibility::Control::RenderScale`).

use crate::renderer::upscale::scaled_size;

/// The physical pixels a viewport of `viewport` points covers at
/// `pixels_per_point`, within `max` a side.
pub(super) fn display_size(viewport: (u32, u32), pixels_per_point: f32, max: u32) -> (u32, u32) {
    let ppp = if pixels_per_point.is_finite() && pixels_per_point > 0.0 { pixels_per_point } else { 1.0 };
    let side = |n: u32| ((n as f32 * ppp).round() as u32).clamp(1, max.max(1));
    (side(viewport.0), side(viewport.1))
}

/// The size the picture renders at: the logical size, or the display's
/// with `physical`, at `scale`; a simulation's at the logical size.
pub(super) fn render_size(
    viewport: (u32, u32),
    pixels_per_point: f32,
    scale: f32,
    physical: bool,
    simulation: bool,
    max: u32,
) -> (u32, u32) {
    if simulation {
        return (viewport.0.clamp(1, max.max(1)), viewport.1.clamp(1, max.max(1)));
    }
    let base = if physical { display_size(viewport, pixels_per_point, max) } else { viewport };
    let (w, h) = scaled_size(base, scale);
    (w.min(max.max(1)), h.min(max.max(1)))
}

/// Whether a picture rendered at `render` is drawn up to `display` by the
/// upscale pass. Not when it is the display's size, nor when it is
/// larger (a screen below 1x: egui shrinks it, as it always did).
pub(super) fn upscales(render: (u32, u32), display: (u32, u32)) -> bool {
    render != display && render.0 <= display.0 && render.1 <= display.1
}

impl super::App {
    /// The physical pixels the viewport's picture covers.
    pub(super) fn display_size(&self) -> (u32, u32) {
        display_size(
            self.fractal_viewport_size,
            self.egui_layer.ctx.pixels_per_point(),
            self.gpu.device.limits().max_texture_dimension_2d,
        )
    }

    /// The size the picture renders at for a viewport of `viewport` points.
    pub(super) fn render_size_for(&self, viewport: (u32, u32)) -> (u32, u32) {
        let settings = self.config_manager.system_settings();
        render_size(
            viewport,
            self.egui_layer.ctx.pixels_per_point(),
            settings.render_scale,
            settings.render_physical_pixels,
            self.config_manager.active_config().render_mode == crate::scene::transforms::RenderMode::Simulation,
            self.gpu.device.limits().max_texture_dimension_2d,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX: u32 = 8192;

    #[test]
    fn full_size_on_a_1x_screen_is_the_viewport_and_nothing_upscales() {
        let v = (1079, 1056);
        assert_eq!(display_size(v, 1.0, MAX), v);
        assert_eq!(render_size(v, 1.0, 1.0, false, false, MAX), v);
        assert!(!upscales(render_size(v, 1.0, 1.0, false, false, MAX), display_size(v, 1.0, MAX)));
    }

    #[test]
    fn a_high_dpi_screen_renders_points_and_shows_pixels() {
        let v = (1000, 800);
        let display = display_size(v, 1.5, MAX);
        assert_eq!(display, (1500, 1200));
        let render = render_size(v, 1.5, 1.0, false, false, MAX);
        assert_eq!(render, v, "points, as before");
        assert!(upscales(render, display), "drawn up by the pass, not by egui");
        assert_eq!(render_size(v, 1.5, 1.0, true, false, MAX), display, "physical pixels, rendered");
        assert_eq!(render_size(v, 1.5, 0.5, true, false, MAX), (750, 600));
    }

    #[test]
    fn a_scale_renders_its_share_of_each_side() {
        let v = (1079, 1056);
        assert_eq!(render_size(v, 1.0, 0.5, false, false, MAX), (540, 528));
        assert_eq!(render_size(v, 1.0, 0.75, false, false, MAX), (809, 792));
        assert!(upscales((540, 528), v));
    }

    #[test]
    fn a_simulation_renders_at_the_viewport_whatever_the_setting() {
        let v = (1079, 1056);
        assert_eq!(render_size(v, 1.0, 0.5, false, true, MAX), v);
        assert_eq!(render_size(v, 2.0, 0.5, true, true, MAX), v);
        assert!(upscales(render_size(v, 2.0, 1.0, false, true, MAX), display_size(v, 2.0, MAX)));
    }

    #[test]
    fn below_1x_egui_shrinks_the_picture_as_before() {
        let v = (1000, 800);
        let display = display_size(v, 0.5, MAX);
        assert_eq!(display, (500, 400));
        assert!(!upscales(render_size(v, 0.5, 1.0, false, false, MAX), display));
    }

    #[test]
    fn sizes_stay_within_the_device() {
        assert_eq!(display_size((4096, 2048), 3.0, MAX), (8192, 6144));
        assert_eq!(render_size((4096, 2048), 3.0, 1.0, true, false, MAX), (8192, 6144));
        assert_eq!(display_size((0, 0), 1.0, MAX), (1, 1));
        assert_eq!(display_size((100, 100), f32::NAN, MAX), (100, 100));
    }
}
