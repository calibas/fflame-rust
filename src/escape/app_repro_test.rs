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

    /// Direct vs perturbed agreement: at a zoom where the direct
    /// path is still accurate (16), the perturbation pipeline must
    /// reproduce its image. This is THE correctness check for the
    /// delta math + rebasing — any sign error, scale slip, or
    /// misindexed reference shows up as wholesale pixel differences.
    #[test]
    #[ignore = "needs a GPU"]
    fn perturbed_agrees_with_direct_at_moderate_zoom() {
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
            label: Some("escape agreement"),
            required_features: wgpu::Features::CLEAR_TEXTURE,
            required_limits: limits,
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        }))
        .expect("device");
        device.on_uncaptured_error(std::sync::Arc::new(|e| {
            panic!("wgpu error during agreement test: {e}");
        }));

        let config = crate::config::FractalConfig::default();
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            256,
            192,
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
            256,
            192,
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

        let mut esc_cfg = crate::config::escape::EscapeConfig::default();
        esc_cfg.center_re = "-0.74364388703715".to_string();
        esc_cfg.center_im = "0.13182590420531".to_string();
        esc_cfg.zoom_log2 = 10.0; // shallow: direct is unimpeachable here
        esc_cfg.max_iter = 800;
        esc_cfg.coloring_params.insert("scale".to_string(), 0.01);

        let mut render_once = |force: bool| -> Vec<u8> {
            let mut escape = crate::escape::EscapeRenderer::new(&device, 256, 192);
            escape.force_perturbed = force;
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("agreement frame"),
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
            escape.destroy();
            rgba
        };

        let direct = render_once(false);
        let perturbed = render_once(true);

        // Diagnostic dumps for visual comparison.
        if let Some(img) = image::RgbaImage::from_raw(256, 192, direct.clone()) {
            let _ = img.save("output/agree-direct.png");
        }
        if let Some(img) = image::RgbaImage::from_raw(256, 192, perturbed.clone()) {
            let _ = img.save("output/agree-perturbed.png");
        }

        // Boundary filigree legitimately flips iteration bands (the
        // two paths round differently), and it can cover a large
        // fraction of an interesting view — so compare 8x8 BLOCK
        // MEANS instead of pixels: band noise averages out, while any
        // structural bug (sign error, scale slip, misindexed
        // reference) shifts whole features and fails loudly.
        let (w, h) = (256usize, 192usize);
        let mut bad_blocks = 0usize;
        let mut total_blocks = 0usize;
        for by in 0..h / 8 {
            for bx in 0..w / 8 {
                let mut sum_a = [0i64; 3];
                let mut sum_b = [0i64; 3];
                for y in 0..8 {
                    for x in 0..8 {
                        let idx = ((by * 8 + y) * w + bx * 8 + x) * 4;
                        for ch in 0..3 {
                            sum_a[ch] += direct[idx + ch] as i64;
                            sum_b[ch] += perturbed[idx + ch] as i64;
                        }
                    }
                }
                total_blocks += 1;
                let diff: i64 = (0..3).map(|ch| (sum_a[ch] - sum_b[ch]).abs() / 64).sum();
                // Calibration: filigree band-flips measured at mean
                // diffs of 25-39 on the densest blocks; a structural
                // misalignment shifts whole features and produces
                // contiguous runs in the hundreds.
                if diff > 48 {
                    bad_blocks += 1;
                }
            }
        }
        println!("agreement: {bad_blocks}/{total_blocks} blocks differ structurally");
        assert!(
            bad_blocks < total_blocks / 25,
            "direct and perturbed disagree structurally on {bad_blocks}/{total_blocks} blocks"
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
                // Parameter plane: every escaping formula's home view
                // has both escaping and bounded territory, so the
                // lit-pixel assertion below holds for all of them. (A
                // fixed Julia seed can't promise that — a small-|λ|
                // Lambda basin, for instance, never escapes at all.)
                esc_cfg.center_re = "0".to_string();
                esc_cfg.center_im = "0".to_string();

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
                // Period coloring on a NonEscaping formula maps
                // undetected-cycle pixels to the palette origin (dark
                // in this renderer's constructor palette), and cycle
                // settle time can exceed the probe's 64 iterations —
                // verified visually instead (novaretti-period corpus).
                let period_on_nonescaping = non_escaping && c.name == "period";
                if (!non_escaping || colors_interior) && !period_on_nonescaping {
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
