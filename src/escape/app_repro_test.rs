//! Headless reproduction of the APP's escape frame sequence.
//!
//! The CLI path (render_with → load_config → escape pass → tonemap)
//! renders correctly; the interactive app shows only background. This
//! test mimics the app's exact sequence — fresh renderer, NO
//! load_config, per-frame update_tonemap with app-style arguments,
//! escape pass + tonemap_pass_with_input in ONE encoder — and reads
//! the fractal texture back. Ignored by default: needs a GPU.

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use egui_wgpu::wgpu;

    #[test]
    #[ignore = "needs a GPU"]
    fn app_style_escape_frame_produces_pixels() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .expect("adapter");
        // Same limits expansion the app / headless export perform: the
        // flame compute bind group needs more storage buffers than the
        // WebGPU floor of 8.
        let adapter_limits = adapter.limits();
        let mut limits = wgpu::Limits::default();
        limits.max_storage_buffers_per_shader_stage =
            adapter_limits.max_storage_buffers_per_shader_stage;
        limits.max_storage_buffer_binding_size = adapter_limits.max_storage_buffer_binding_size;
        limits.max_buffer_size = adapter_limits.max_buffer_size;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("escape repro"),
            required_features: wgpu::Features::CLEAR_TEXTURE,
            required_limits: limits,
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        }))
        .expect("device");
        device.on_uncaptured_error(std::sync::Arc::new(|e| {
            panic!("wgpu error during repro: {e}");
        }));

        // App startup: renderer built for the default config's flame.
        let config = crate::config::FractalConfig::default();
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            400,
            300,
            &config.flame,
            config.palette_size,
        );

        // The app frame, escape mode, in call order.
        let mut escape = crate::escape::EscapeRenderer::new(&device, 400, 300);
        let mut esc_cfg = crate::config::escape::EscapeConfig::default();
        esc_cfg.max_iter = 256;

        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("repro frame"),
        });
        escape.render(&device, &queue, &mut encoder, &esc_cfg, renderer.palette_view());

        // App-style tonemap update: Linear mode, total_iterations = 0.
        renderer.update_density_scale(&queue, config.density_scale);
        renderer.update_background_color(&queue, config.background_color);
        renderer.update_tonemap(
            &queue,
            crate::scene::tonemap::ToneMapMode::Linear,
            config.highlight_mode,
            config.use_curve,
            config.exposure,
            config.gamma,
            config.gamma_threshold,
            config.brightness,
            config.vibrancy,
            config.white_level,
            config.saturation,
            config.hue_shift,
            config.alpha_blend_low,
            config.alpha_blend_high,
            400,
            300,
            renderer.total_iterations(),
            config.max_iterations,
            config.zoom,
            256,
            4,
            false,
            config.levels_enabled,
            config.levels_low,
            config.levels_high,
            config.levels_gamma,
        );
        renderer.tonemap_pass_with_input(&device, &queue, &mut encoder, escape.output_view());
        queue.submit(std::iter::once(encoder.finish()));

        let pixels = pollster::block_on(renderer.read_fractal_pixels(
            &device,
            &queue,
            false,
            config.background_color,
        ))
        .expect("readback");
        let (_, _, rgba) = pixels;

        // Count pixels that differ from the flat background.
        let bg = [
            (config.background_color[0] * 255.0) as i32,
            (config.background_color[1] * 255.0) as i32,
            (config.background_color[2] * 255.0) as i32,
        ];
        let mut non_bg = 0usize;
        for px in rgba.chunks_exact(4) {
            let d = (px[0] as i32 - bg[0]).abs()
                + (px[1] as i32 - bg[1]).abs()
                + (px[2] as i32 - bg[2]).abs();
            if d > 12 {
                non_bg += 1;
            }
        }
        let total = rgba.len() / 4;
        println!(
            "repro: {}/{} non-background pixels; first px {:?}; bg {:?}",
            non_bg,
            total,
            &rgba[..4],
            bg
        );
        assert!(
            non_bg > total / 20,
            "escape frame rendered (almost) nothing: {non_bg}/{total} non-background pixels"
        );
    }

    /// The GPU half of the plan's formula x coloring probe: every
    /// combination dispatches on a real device (the naga test already
    /// guarantees they validate). Content is asserted only for
    /// combinations expected to produce it — escape-based colorings on
    /// a NonEscaping formula legitimately render black.
    #[test]
    #[ignore = "needs a GPU"]
    fn every_formula_coloring_combination_dispatches() {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .expect("adapter");
        let adapter_limits = adapter.limits();
        let mut limits = wgpu::Limits::default();
        limits.max_storage_buffers_per_shader_stage =
            adapter_limits.max_storage_buffers_per_shader_stage;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("escape combo probe"),
            required_features: wgpu::Features::CLEAR_TEXTURE,
            required_limits: limits,
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        }))
        .expect("device");
        device.on_uncaptured_error(std::sync::Arc::new(|e| {
            panic!("wgpu error during combo probe: {e}");
        }));

        let config = crate::config::FractalConfig::default();
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            128,
            96,
            &config.flame,
            config.palette_size,
        );
        renderer.update_tonemap(
            &queue,
            crate::scene::tonemap::ToneMapMode::Linear,
            config.highlight_mode,
            config.use_curve,
            config.exposure,
            config.gamma,
            config.gamma_threshold,
            config.brightness,
            config.vibrancy,
            config.white_level,
            config.saturation,
            config.hue_shift,
            config.alpha_blend_low,
            config.alpha_blend_high,
            128,
            96,
            0,
            config.max_iterations,
            config.zoom,
            256,
            1,
            false,
            config.levels_enabled,
            config.levels_low,
            config.levels_high,
            config.levels_gamma,
        );
        let mut escape = crate::escape::EscapeRenderer::new(&device, 128, 96);

        for f in crate::escape::FORMULAS {
            for c in crate::escape::COLORINGS {
                let mut esc_cfg = crate::config::escape::EscapeConfig::default();
                esc_cfg.formula = f.name.to_string();
                esc_cfg.coloring = c.name.to_string();
                esc_cfg.max_iter = 64;
                // Julia mode with a seed that keeps every formula's
                // dynamical plane interesting enough to light pixels.
                esc_cfg.julia = true;
                esc_cfg.julia_re = -0.4;
                esc_cfg.julia_im = 0.35;

                let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("combo frame"),
                });
                escape.render(&device, &queue, &mut encoder, &esc_cfg, renderer.palette_view());
                renderer.tonemap_pass_with_input(&device, &queue, &mut encoder, escape.output_view());
                queue.submit(std::iter::once(encoder.finish()));

                let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                    &device,
                    &queue,
                    false,
                    [0.0, 0.0, 0.0],
                ))
                .expect("readback");
                let lit = rgba
                    .chunks_exact(4)
                    .filter(|px| px[0] > 8 || px[1] > 8 || px[2] > 8)
                    .count();

                // NonEscaping formulas only produce content through
                // interior-coloring colorings; every other pairing must
                // light a meaningful share of a 128x96 frame.
                let non_escaping = f.has_feature(crate::escape::FormulaFeature::NonEscaping);
                let colors_interior = c.has_feature(crate::escape::ColoringFeature::ColorsInterior);
                if !non_escaping || colors_interior {
                    assert!(
                        lit > (128 * 96) / 50,
                        "{} x {} lit only {lit} pixels",
                        f.name,
                        c.name
                    );
                }
                println!("combo {} x {}: {lit} lit", f.name, c.name);
            }
        }
        escape.destroy();
        renderer.destroy();
    }
}
