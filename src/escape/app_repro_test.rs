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
    /// 8x8 block-mean compare (same calibration as the direct-vs-
    /// perturbed check): band noise averages out, structural shifts
    /// fail. Returns (bad_blocks, total_blocks).
    fn block_diff(a: &[u8], b: &[u8], w: usize, h: usize) -> (usize, usize) {
        let mut bad = 0usize;
        let mut total = 0usize;
        for by in 0..h / 8 {
            for bx in 0..w / 8 {
                let mut sum_a = [0i64; 3];
                let mut sum_b = [0i64; 3];
                for y in 0..8 {
                    for x in 0..8 {
                        let idx = ((by * 8 + y) * w + bx * 8 + x) * 4;
                        for ch in 0..3 {
                            sum_a[ch] += a[idx + ch] as i64;
                            sum_b[ch] += b[idx + ch] as i64;
                        }
                    }
                }
                total += 1;
                let diff: i64 = (0..3).map(|ch| (sum_a[ch] - sum_b[ch]).abs() / 64).sum();
                if diff > 48 {
                    bad += 1;
                }
            }
        }
        (bad, total)
    }

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

        let bla_off = std::cell::Cell::new(false);
        let mut render_once = |esc_cfg: &crate::config::escape::EscapeConfig,
                               force: bool,
                               floatexp: bool|
         -> Vec<u8> {
            let mut escape = crate::escape::EscapeRenderer::new(&device, 256, 192);
            escape.force_perturbed = force;
            escape.force_floatexp = floatexp;
            escape.disable_bla = bla_off.get();
            // Tiny chunks force the multi-dispatch path on the
            // perturbed renders: state save/restore must reproduce
            // the single-pass images exactly.
            escape.chunk_override = Some(64);
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("agreement frame"),
            });
            let mut settled =
                escape.render(&device, &queue, &mut encoder, esc_cfg, renderer.palette_view());
            let mut guard = 0;
            while !settled {
                queue.submit(std::iter::once(encoder.finish()));
                encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("agreement chunk"),
                });
                settled = escape.render(
                    &device,
                    &queue,
                    &mut encoder,
                    esc_cfg,
                    renderer.palette_view(),
                );
                guard += 1;
                assert!(guard < 10_000, "chunk loop failed to settle");
            }
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

        let mut check = |label: &str,
                         esc_cfg: &crate::config::escape::EscapeConfig,
                         floatexp: bool| {
            let direct = render_once(esc_cfg, false, false);
            let perturbed = render_once(esc_cfg, true, floatexp);
            if let Some(img) = image::RgbaImage::from_raw(256, 192, direct.clone()) {
                let _ = img.save(format!("output/agree-{label}-direct.png"));
            }
            if let Some(img) = image::RgbaImage::from_raw(256, 192, perturbed.clone()) {
                let _ = img.save(format!("output/agree-{label}-perturbed.png"));
            }
            // Boundary filigree legitimately flips iteration bands;
            // compare 8x8 BLOCK MEANS so band noise averages out while
            // structural bugs (sign, scale, misindexed or misrebased
            // reference) shift whole features and fail loudly.
            // Calibration: filigree band-flips measured at 25-39
            // mean-diff on the densest blocks.
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
                    if diff > 48 {
                        bad_blocks += 1;
                    }
                }
            }
            println!("agreement[{label}]: {bad_blocks}/{total_blocks} blocks differ structurally");
            assert!(
                bad_blocks < total_blocks / 25,
                "[{label}] direct and perturbed disagree structurally on {bad_blocks}/{total_blocks} blocks"
            );
        };

        // Parameter plane at a seahorse-valley center.
        let mut esc_cfg = crate::config::escape::EscapeConfig::default();
        esc_cfg.center_re = "-0.74364388703715".to_string();
        esc_cfg.center_im = "0.13182590420531".to_string();
        esc_cfg.zoom_log2 = 10.0; // shallow: direct is unimpeachable here
        esc_cfg.max_iter = 800;
        esc_cfg.coloring_params.insert("scale".to_string(), 0.01);
        check("param", &esc_cfg, false);
        // The floatexp rung must reproduce the same shallow view too.
        check("param-floatexp", &esc_cfg, true);

        // Julia plane, centered on the repelling fixed point of
        // c = -0.8 + 0.156i (guaranteed on the Julia set, so the view
        // has boundary structure at every depth). This is the case
        // that caught the rebase Z_0 bug.
        let mut julia_cfg = crate::config::escape::EscapeConfig::default();
        julia_cfg.julia = true;
        julia_cfg.julia_re = -0.8;
        julia_cfg.julia_im = 0.156;
        julia_cfg.center_re = "1.5275031186435346".to_string();
        julia_cfg.center_im = "-0.07591217835228786".to_string();
        julia_cfg.zoom_log2 = 10.0;
        julia_cfg.max_iter = 800;
        julia_cfg.coloring_params.insert("scale".to_string(), 0.01);
        check("julia", &julia_cfg, false);
        check("julia-floatexp", &julia_cfg, true);

        // Multibrot p=3 over the WHOLE home view (force flag ignores
        // the zoom gate): every boundary pixel of the set exercises
        // the binomial delta step against the direct render.
        let mut multi_cfg = crate::config::escape::EscapeConfig::default();
        multi_cfg.formula = "multibrot".to_string();
        multi_cfg.formula_params.insert("power".to_string(), 3.0);
        multi_cfg.center_re = "0".to_string();
        multi_cfg.center_im = "0".to_string();
        multi_cfg.zoom_log2 = 2.0;
        multi_cfg.max_iter = 300;
        multi_cfg.coloring_params.insert("scale".to_string(), 0.02);
        check("multibrot3", &multi_cfg, false);
        check("multibrot3-floatexp", &multi_cfg, true);

        // Burning Ship (plain variant) over its home view: every
        // boundary pixel exercises the diffabs case analysis, for every
        // fold variant, on both rungs.
        let mut ship_cfg = crate::config::escape::EscapeConfig::default();
        ship_cfg.formula = "burning_ship".to_string();
        ship_cfg.center_re = "-0.5".to_string();
        ship_cfg.center_im = "0.5".to_string();
        ship_cfg.zoom_log2 = 2.0;
        ship_cfg.max_iter = 300;
        ship_cfg.coloring_params.insert("scale".to_string(), 0.02);
        for v in 0..=5u32 {
            ship_cfg
                .formula_params
                .insert("variant".to_string(), v as f32);
            check(&format!("ship-v{v}"), &ship_cfg, false);
        }
        ship_cfg.formula_params.insert("variant".to_string(), 0.0);
        check("ship-floatexp", &ship_cfg, true);

        // BLA on-vs-off agreement: iteration skips must reproduce the
        // per-step images, including PAST direct's reach (the shallow
        // checks above already run the perturbed arm with BLA active,
        // so skips are also held against direct there).
        let mut bla_case = crate::config::escape::EscapeConfig::default();
        bla_case.center_re = "-0.74364388703715".to_string();
        bla_case.center_im = "0.13182590420531".to_string();
        bla_case.max_iter = 3000;
        bla_case.coloring_params.insert("scale".to_string(), 0.01);
        for (label, zoom, fe) in
            [("bla-scaled", 40.0f64, false), ("bla-floatexp", 60.0, true)]
        {
            bla_case.zoom_log2 = zoom;
            bla_off.set(false);
            let with_bla = render_once(&bla_case, true, fe);
            bla_off.set(true);
            let without = render_once(&bla_case, true, fe);
            bla_off.set(false);
            let (bad, total) = block_diff(&with_bla, &without, 256, 192);
            println!("agreement[{label}]: {bad}/{total} blocks differ structurally");
            assert!(
                bad < total / 25,
                "[{label}] BLA on/off disagree on {bad}/{total} blocks"
            );
        }
        // Deep BLA at the field-reported glitch depth (zoom 484,
        // floatexp rung, ~10-limb reference) on a real curated
        // location — earlier BLA coverage stopped at zoom 60.
        let mut deep_cfg = crate::config::escape::EscapeConfig::default();
        deep_cfg.center_re = "-1.94156484721061838178274553314663068785257733081147918532807665584651847303430909385256500685469587446965379269662621640024354437".to_string();
        deep_cfg.center_im = "0.0002348911956401652748611382363072520535146733491918842206467215035002992134677528497852082859490167037836900129822995".to_string();
        deep_cfg.zoom_log2 = 484.0;
        deep_cfg.max_iter = 20000;
        deep_cfg.coloring_params.insert("scale".to_string(), 0.05);
        bla_off.set(false);
        let with_bla = render_once(&deep_cfg, true, true);
        bla_off.set(true);
        let without = render_once(&deep_cfg, true, true);
        bla_off.set(false);
        let (bad, total) = block_diff(&with_bla, &without, 256, 192);
        println!("agreement[bla-z484]: {bad}/{total} blocks differ structurally");
        assert!(
            bad < total / 25,
            "[bla-z484] BLA on/off disagree on {bad}/{total} blocks"
        );

        // BLA past the old zoom-900 gate: the log-space |δc| bound
        // must keep tables valid at any depth (430-digit center from
        // the same field location).
        deep_cfg.center_re = "-1.9415648472106183817827455331466306878525773308114791853287171106263154653138889844065700912718617763788260927901438262039941523255909231478771330222244384505055953923324421692687866048802396828480134068979835794320627022921996449325642064207757630337300264109603930340243794485583132951277844263815922780809251921981665064149459854149137453666056576556104770782432234331286505619021491097669553415414488892520906440504495875324".to_string();
        deep_cfg.center_im = "0.0002348911956401652748611382363072520535146733491918842206389055226478822558334356028474458306453568269131543696797365302213154106976514279082244760267169482925324526783567612979671556935057632055950984996909780142673870494806718441563468971222881465156907737846885411815804623686136775248121351602452938196791632141551203544924477065181043689768585002934501366247348894440025575034790977798556673982209118819387316634056673728437".to_string();
        deep_cfg.zoom_log2 = 1000.0;
        deep_cfg.max_iter = 30000;
        bla_off.set(false);
        let deep_b = render_once(&deep_cfg, true, true);
        bla_off.set(true);
        let deep_n = render_once(&deep_cfg, true, true);
        bla_off.set(false);
        let (bad, total) = block_diff(&deep_b, &deep_n, 256, 192);
        println!("agreement[bla-z1000]: {bad}/{total} blocks differ structurally");
        assert!(
            bad < total / 25,
            "[bla-z1000] BLA on/off disagree on {bad}/{total} blocks"
        );

        // Julia (dc_max = 0: every skip's B term is exact).
        julia_cfg.zoom_log2 = 40.0;
        bla_off.set(false);
        let jb = render_once(&julia_cfg, true, false);
        bla_off.set(true);
        let jn = render_once(&julia_cfg, true, false);
        bla_off.set(false);
        let (bad, total) = block_diff(&jb, &jn, 256, 192);
        println!("agreement[bla-julia]: {bad}/{total} blocks differ structurally");
        assert!(
            bad < total / 25,
            "[bla-julia] BLA on/off disagree on {bad}/{total} blocks"
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
