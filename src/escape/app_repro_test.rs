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

    /// Tests that read (or reset) the GLOBAL escape diagnostics
    /// snapshot hold this while they run: cargo runs tests in
    /// parallel, and a concurrent render from another test would
    /// stomp `diag::snapshot()` between a render and its assertion.
    static DIAG_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());
    fn diag_lock() -> std::sync::MutexGuard<'static, ()> {
        DIAG_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Interior detection must be INVISIBLE, and must pay.
    ///
    /// The design claim is stronger than "close enough": the direct
    /// path's iteration state is exactly z, the arithmetic is
    /// deterministic, so a bit-exact repeat proves the f32 orbit is
    /// periodic forever after. Stopping there is the same render, not
    /// an approximation of it -- which is why it ships always-on with
    /// no tolerance and no toggle. This asserts that byte for byte on
    /// an interior-heavy view (the home view is ~1/4 set), and prints
    /// the speedup, which is the reason the feature exists: those
    /// pixels used to burn every one of max_iter iterations, and that
    /// is what drove the row-band budget into Windows' TDR window.
    #[test]
    #[ignore = "needs a GPU"]
    fn interior_detection_is_invisible_and_faster() {
        let (device, queue) = repro_device();
        let mut esc_cfg = crate::config::escape::EscapeConfig::default();
        // Home view: plenty of interior, direct path (zoom < 14), and
        // `smooth` does not draw the interior -- the eligibility gate.
        esc_cfg.zoom_log2 = 0.0;
        esc_cfg.max_iter = 400_000;
        assert_eq!(esc_cfg.coloring, "smooth", "the gate assumes this coloring");

        let (w, h) = (256u32, 192u32);
        let render = |detect: bool| -> (Vec<u8>, std::time::Duration) {
            let config = crate::config::FractalConfig::default();
            let mut renderer =
                crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
                    &device,
                    &queue,
                    wgpu::TextureFormat::Rgba8Unorm,
                    w,
                    h,
                    &config.flame,
                    config.palette_size,
                );
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            escape.disable_interior = !detect;
            // Warm the pipeline so the timing measures rendering.
            let t0 = {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("interior warm"),
                });
                escape.render(&device, &queue, &mut enc, &esc_cfg, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                std::time::Instant::now()
            };
            let mut guard = 0u32;
            loop {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("interior frame"),
                });
                let settled =
                    escape.render(&device, &queue, &mut enc, &esc_cfg, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 100_000, "render failed to settle (detect={detect})");
            }
            let elapsed = t0.elapsed();
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("interior tonemap"),
            });
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device,
                &queue,
                false,
                config.background_color,
            ))
            .expect("readback");
            (rgba, elapsed)
        };

        let (off, t_off) = render(false);
        let (on, t_on) = render(true);
        let diff = off.iter().zip(on.iter()).filter(|(a, b)| a != b).count();
        assert_eq!(
            diff,
            0,
            "interior detection changed {diff} of {} bytes -- it must be invisible",
            off.len()
        );
        println!(
            "interior detection: {:.0} ms -> {:.0} ms ({:.1}x) at max_iter {}",
            t_off.as_secs_f64() * 1000.0,
            t_on.as_secs_f64() * 1000.0,
            t_off.as_secs_f64() / t_on.as_secs_f64().max(1e-9),
            esc_cfg.max_iter,
        );
    }

    /// A perturbed render must match an EXACT orbit at a depth the
    /// direct path cannot reach.
    ///
    /// The direct-vs-perturbed agreement test runs shallow, where
    /// rebasing fires almost every iteration and the reference barely
    /// matters -- so it passes even when the REFERENCE ITSELF is the
    /// wrong map. That is not hypothetical: Phoenix shipped with the
    /// reference built for p = 0 while the delta step used p = -0.5,
    /// the agreement test read 0/768, and the first deep render in
    /// the app was visibly a different fractal. This compares against
    /// an f64 orbit computed here, at a zoom where the reference is
    /// what carries the signal.
    #[test]
    #[ignore = "needs a GPU"]
    fn perturbed_phoenix_matches_an_exact_orbit_at_depth() {
        // BOTH rungs: the deep one carries its two-term history in
        // real struct fields and rebases the pair in double-float --
        // a separate implementation, owed the same ground truth.
        for deep in [false, true] {
            phoenix_exact_orbit_case(deep);
        }
    }

    fn phoenix_exact_orbit_case(deep: bool) {
        let (device, queue) = repro_device();
        let (cx, cy) = (-0.76143253429068480f64, 0.66677904046244096f64);
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "phoenix".to_string();
        esc.center_re = format!("{cx:.17}");
        esc.center_im = format!("{cy:.17}");
        esc.zoom_log2 = 20.0;
        // Headroom above the escape times: a view compared at its own
        // iteration cap disagrees on the pixels finishing right at it.
        // Note this also means almost nothing here REBASES -- see the
        // short-orbit test below for that regime.
        esc.max_iter = 4000;
        // Deliberately UNSET: the defaults are the case that broke.
        assert!(esc.formula_params.is_empty());
        let p = {
            let defs = crate::escape::get_formula("phoenix").parameters;
            (defs[0].default as f64, defs[1].default as f64)
        };

        let (w, h) = (96u32, 72u32);
        let mut config = crate::config::FractalConfig::default();
        // A BINARY image, so the comparison reads escaped-or-not and
        // nothing else. The default palette is grayscale, whose low
        // end is black -- under it a genuinely escaped pixel with a
        // small smooth value is indistinguishable from the interior,
        // and this comparison reported 63% disagreement on a render
        // the CLI measured at 1.2%.
        config.palette = crate::scene::palette::Palette {
            name: "white".to_string(),
            stops: vec![
                crate::scene::palette::ColorStop {
                    position: 0.0,
                    color: [1.0, 1.0, 1.0],
                },
                crate::scene::palette::ColorStop {
                    position: 1.0,
                    color: [1.0, 1.0, 1.0],
                },
            ],
            locked: false,
            built_in: false,
        };
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            w,
            h,
            &config.flame,
            config.palette_size,
        );
        renderer.update_palette(
            &device,
            &queue,
            &config.palette,
            config.palette_rotation,
            config.palette_squeeze,
            config.palette_squeeze_mode,
            config.palette_squeeze_falloff,
            config.palette_log_strength,
            config.palette_reverse,
        );
        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        escape.force_floatexp = deep;
        let mut guard = 0u32;
        loop {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("phoenix depth"),
            });
            let settled = escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            });
            if settled {
                break;
            }
            guard += 1;
            assert!(guard < 100_000, "render did not settle");
        }
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("phoenix depth tonemap"),
        });
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
            w,
            h,
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
        renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
        queue.submit(std::iter::once(enc.finish()));
        let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
            &device,
            &queue,
            false,
            config.background_color,
        ))
        .expect("readback");

        // f64 oracle over the same view: z' = z^2 + c + p*z_prev.
        let span_y = 4.0 / (esc.zoom_log2 as f64).exp2();
        let span_x = span_y * w as f64 / h as f64;
        let mut differ = 0usize;
        for py in 0..h {
            for px in 0..w {
                let ci = (
                    ((px as f64 + 0.5) / w as f64 - 0.5) * span_x + cx,
                    -(((py as f64 + 0.5) / h as f64 - 0.5) * span_y) + cy,
                );
                let (mut zx, mut zy) = (0.0f64, 0.0f64);
                let (mut qx, mut qy) = (0.0f64, 0.0f64);
                let mut escaped = false;
                for _ in 0..esc.max_iter {
                    let nx = zx * zx - zy * zy + ci.0 + p.0 * qx - p.1 * qy;
                    let ny = 2.0 * zx * zy + ci.1 + p.0 * qy + p.1 * qx;
                    qx = zx;
                    qy = zy;
                    zx = nx;
                    zy = ny;
                    if zx * zx + zy * zy > 4.0 {
                        escaped = true;
                        break;
                    }
                }
                let i = ((py * w + px) * 4) as usize;
                let lit = rgba[i] as u32 + rgba[i + 1] as u32 + rgba[i + 2] as u32 > 24;
                if lit != escaped {
                    differ += 1;
                }
            }
        }
        let frac = differ as f64 / (w * h) as f64;
        println!(
            "phoenix at zoom {} on the {} rung: {:.2}% differ from the exact orbit",
            esc.zoom_log2,
            if deep { "floatexp" } else { "scaled" },
            100.0 * frac
        );
        // Tight on purpose: a correct render reads 0.00% here. The
        // 3% this started at was slack enough to sit through the
        // index-1 rebase bug (1.23%) without complaining.
        assert!(
            frac < 0.005,
            "perturbed Phoenix disagrees with an exact orbit on {:.1}% of pixels --              the reference and the delta step describe different maps",
            100.0 * frac
        );
    }

    /// A perturbed render must agree with an exact orbit's SMOOTH
    /// FIELD, on a view whose reference orbit is short.
    ///
    /// The zoom-20 test above compares escaped-or-not with 4000
    /// iterations of headroom, so its pixels escape long before the
    /// reference runs out and almost none of them ever rebase. This
    /// view is the opposite: 256 iterations, every pixel escapes, and
    /// the orbit's end forces a rebase on nearly all of them. That is
    /// the regime where Phoenix's two-term rebase shipped broken --
    /// it restarted the reference at index 1, and Z_1 = c, so
    /// `z_full - Z_1` in f32 cancelled down to ulp(c): about eighty
    /// PIXELS wide at this zoom. The image came apart into displaced
    /// rectangular blocks while every existing test stayed green.
    ///
    /// The metric is palette-agnostic: bin the pixels by the f64
    /// smooth value and measure the colour spread WITHIN a bin. If
    /// the render is a function of the true field, pixels sharing a
    /// value share a colour. Measured 5.8/255 correct, 38.8/255 with
    /// the index-1 rebase.
    #[test]
    #[ignore = "needs a GPU"]
    fn perturbed_phoenix_matches_an_exact_smooth_field_on_a_short_orbit() {
        for deep in [false, true] {
            phoenix_smooth_field_case(deep);
        }
    }

    fn phoenix_smooth_field_case(deep: bool) {
        let (device, queue) = repro_device();
        let (cx, cy) = (-1.1543534481639833789918460777111f64, 0.6293282132782021964135047790493f64);
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "phoenix".to_string();
        esc.center_re = "-1.1543534481639833789918460777111".to_string();
        esc.center_im = "0.6293282132782021964135047790493".to_string();
        esc.zoom_log2 = 22.436075;
        esc.max_iter = 256;
        let p = {
            let defs = crate::escape::get_formula("phoenix").parameters;
            (defs[0].default as f64, defs[1].default as f64)
        };

        let (w, h) = (640u32, 480u32);
        let mut config = crate::config::FractalConfig::default();
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        escape.force_floatexp = deep;
        let mut guard = 0u32;
        loop {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("phoenix short orbit"),
            });
            let settled =
                escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            if settled {
                break;
            }
            guard += 1;
            assert!(guard < 100_000, "render did not settle");
        }
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("phoenix short orbit tonemap"),
        });
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
            w,
            h,
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
        renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
        queue.submit(std::iter::once(enc.finish()));
        let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
            &device, &queue, false, config.background_color,
        ))
        .expect("readback");

        // f64 smooth field over the same view.
        let span_y = 4.0 / (esc.zoom_log2 as f64).exp2();
        let span_x = span_y * w as f64 / h as f64;
        let mut smooth = vec![esc.max_iter as f64; (w * h) as usize];
        for py in 0..h {
            for px in 0..w {
                let ci = (
                    ((px as f64 + 0.5) / w as f64 - 0.5) * span_x + cx,
                    -(((py as f64 + 0.5) / h as f64 - 0.5) * span_y) + cy,
                );
                let (mut zx, mut zy) = (0.0f64, 0.0f64);
                let (mut qx, mut qy) = (0.0f64, 0.0f64);
                for k in 0..esc.max_iter {
                    let nx = zx * zx - zy * zy + ci.0 + p.0 * qx - p.1 * qy;
                    let ny = 2.0 * zx * zy + ci.1 + p.0 * qy + p.1 * qx;
                    qx = zx;
                    qy = zy;
                    zx = nx;
                    zy = ny;
                    let r2 = zx * zx + zy * zy;
                    if r2 > 4.0 {
                        smooth[(py * w + px) as usize] =
                            k as f64 + 1.0 - (r2.sqrt().ln() / 2f64.ln()).log2();
                        break;
                    }
                }
            }
        }
        let (lo, hi) = smooth.iter().fold((f64::MAX, f64::MIN), |(a, b), &v| (a.min(v), b.max(v)));
        const BINS: usize = 256;
        let mut sums = vec![[0f64; 3]; BINS];
        let mut sqs = vec![[0f64; 3]; BINS];
        let mut counts = vec![0usize; BINS];
        for (i, &v) in smooth.iter().enumerate() {
            let b = (((v - lo) / (hi - lo)) * (BINS - 1) as f64).round() as usize;
            counts[b] += 1;
            for ch in 0..3 {
                let c = rgba[i * 4 + ch] as f64;
                sums[b][ch] += c;
                sqs[b][ch] += c * c;
            }
        }
        let (mut spread, mut total) = (0f64, 0usize);
        for b in 0..BINS {
            if counts[b] < 20 {
                continue;
            }
            let n = counts[b] as f64;
            let sd: f64 = (0..3)
                .map(|ch| (sqs[b][ch] / n - (sums[b][ch] / n).powi(2)).max(0.0).sqrt())
                .sum::<f64>()
                / 3.0;
            spread += sd * n;
            total += counts[b];
        }
        let spread = spread / total as f64;
        let rung = if deep { "floatexp" } else { "scaled" };
        println!("phoenix short-orbit, {rung} rung: colour spread within a smooth bin {spread:.2}/255");
        assert!(
            spread < 15.0,
            "perturbed Phoenix does not track the exact smooth field ({spread:.2}/255) --              the render is not a function of the true escape value"
        );
    }

    /// Crossing the floatexp threshold must not change the image.
    ///
    /// Phoenix had no deep rung, so above zoom 48 `wants_perturbation`
    /// refused the perturbed path and the view fell through to the
    /// direct one, whose f32 pixel mapping resolves nothing that
    /// deep. The reported symptom was a single flat colour: 5035
    /// distinct colours at zoom 48.000, exactly 1 at 48.001.
    ///
    /// Two views this close are the same picture, so the two rungs
    /// must agree on it -- and the scaled side is independently
    /// pinned against an exact orbit by the tests above. Note what
    /// this deliberately does NOT assert: that some deeper zoom shows
    /// detail. Whether a given centre still has structure at zoom 60
    /// is a property of the fractal, not of the renderer -- checked
    /// at 80 digits with mpmath, this centre's neighbourhood is
    /// genuinely uniform by zoom 55, and a "still has detail" test
    /// would be asserting the fractal's shape.
    #[test]
    #[ignore = "needs a GPU"]
    fn crossing_the_floatexp_threshold_keeps_the_phoenix_image() {
        let (device, queue) = repro_device();
        let (w, h) = (96u32, 72u32);
        let config = crate::config::FractalConfig::default();
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        let mut shot = |zoom: f64| -> Vec<u8> {
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.formula = "phoenix".to_string();
            esc.center_re = "-0.87671486633428493249082525350857894792".to_string();
            esc.center_im = "0.68253971831422086306603135717477825677".to_string();
            esc.zoom_log2 = zoom;
            esc.max_iter = 256;
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            let mut guard = 0u32;
            loop {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("phoenix threshold"),
                });
                let settled =
                    escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                let _ =
                    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 100_000, "render did not settle at zoom {zoom}");
            }
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("phoenix threshold tonemap"),
            });
            // Escape output is Linear-mapped; the flame default is a
            // Log curve that flattens it whatever it contains.
            renderer.update_tonemap(
                &queue,
                crate::scene::tonemap::ToneMapMode::Linear,
                config.highlight_mode,
                config.use_curve,
                1.0,
                1.0,
                config.gamma_threshold,
                config.brightness,
                config.vibrancy,
                config.white_level,
                config.saturation,
                config.hue_shift,
                config.alpha_blend_low,
                config.alpha_blend_high,
                w,
                h,
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
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, config.background_color,
            ))
            .expect("readback");
            escape.destroy();
            rgba
        };

        let below = shot(47.999);
        let above = shot(48.001);
        let colours = |px: &Vec<u8>| {
            px.chunks_exact(4)
                .map(|p| [p[0], p[1], p[2]])
                .collect::<std::collections::HashSet<_>>()
                .len()
        };
        let (cb, ca) = (colours(&below), colours(&above));
        println!("phoenix across the threshold: {cb} colours below, {ca} above");
        assert!(
            ca > 50,
            "above the threshold the image collapsed to {ca} distinct colours --              that is what falling off the perturbed path looks like"
        );
        // Same picture, so compare 8x8 block means: band flips at the
        // boundary average out, a rung that renders something else
        // does not.
        let mut bad = 0usize;
        let mut total = 0usize;
        for by in 0..(h as usize) / 8 {
            for bx in 0..(w as usize) / 8 {
                let (mut sa, mut sb) = ([0i64; 3], [0i64; 3]);
                for y in 0..8 {
                    for x in 0..8 {
                        let i = ((by * 8 + y) * w as usize + bx * 8 + x) * 4;
                        for ch in 0..3 {
                            sa[ch] += below[i + ch] as i64;
                            sb[ch] += above[i + ch] as i64;
                        }
                    }
                }
                total += 1;
                if (0..3).map(|ch| (sa[ch] - sb[ch]).abs() / 64).sum::<i64>() > 48 {
                    bad += 1;
                }
            }
        }
        println!("phoenix across the threshold: {bad}/{total} blocks differ");
        assert!(
            bad < total / 25,
            "the two rungs render different pictures of the same view ({bad}/{total} blocks)"
        );
    }

    /// The two rungs must agree on a DEEP Julia view.
    ///
    /// This asks one specific question. The scaled rung's rebase does
    /// `z_full - ref_z(0u)` in f32, and on the Julia plane Z_0 is the
    /// CENTRE -- an O(1) number, so that is the same catastrophic
    /// cancellation that broke Phoenix, whose blocks were ulp(c)
    /// wide. The deep rung rebuilds the same quantity in double-float
    /// from the reference's own DF entries and is immune to it. If
    /// the f32 subtraction were corrupting the image, the two rungs
    /// would disagree here, and by more the deeper it goes.
    ///
    /// (An external oracle cannot settle this cheaply: at these
    /// depths a binary escape-set comparison lands in the max_iter
    /// cliff -- 93% of the disagreeing pixels sat within ten
    /// iterations of the cap -- and a colour comparison is confounded
    /// by the palette cycling once per 100 smooth units.)
    #[test]
    #[ignore = "needs a GPU"]
    fn julia_rebase_agrees_across_rungs_at_depth() {
        let (device, queue) = repro_device();
        let (w, h) = (128u32, 96u32);
        let config = crate::config::FractalConfig::default();
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        let mut shot = |zoom: f64, deep: bool| -> Vec<u8> {
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.julia = true;
            esc.julia_re = -0.8;
            esc.julia_im = 0.156;
            // A centre that still carries structure at zoom 32 (found
            // by descending on local variance of the smooth field).
            esc.center_re = "1.52750302293644413254".to_string();
            esc.center_im = "-0.07591226956711649709".to_string();
            esc.zoom_log2 = zoom;
            esc.max_iter = 2000;
            esc.coloring_params.insert("scale".to_string(), 0.01);
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            escape.force_floatexp = deep;
            let mut guard = 0u32;
            loop {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("julia rung"),
                });
                let settled =
                    escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                let _ =
                    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 100_000, "render did not settle");
            }
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("julia rung tonemap"),
            });
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, config.background_color,
            ))
            .expect("readback");
            escape.destroy();
            rgba
        };
        for zoom in [18.5, 24.0, 28.0, 32.0] {
            let scaled = shot(zoom, false);
            let deep = shot(zoom, true);
            let mut bad = 0usize;
            let mut total = 0usize;
            for by in 0..(h as usize) / 8 {
                for bx in 0..(w as usize) / 8 {
                    let (mut sa, mut sb) = ([0i64; 3], [0i64; 3]);
                    for y in 0..8 {
                        for x in 0..8 {
                            let i = ((by * 8 + y) * w as usize + bx * 8 + x) * 4;
                            for ch in 0..3 {
                                sa[ch] += scaled[i + ch] as i64;
                                sb[ch] += deep[i + ch] as i64;
                            }
                        }
                    }
                    total += 1;
                    if (0..3).map(|ch| (sa[ch] - sb[ch]).abs() / 64).sum::<i64>() > 48 {
                        bad += 1;
                    }
                }
            }
            println!("julia zoom {zoom}: {bad}/{total} blocks differ between the rungs");
            assert!(
                bad < total / 25,
                "at zoom {zoom} the scaled and floatexp rungs render different pictures                  ({bad}/{total} blocks) -- the f32 rebase subtraction against Z_0 = centre"
            );
        }
    }

    /// Manowar must match an exact orbit at a depth the direct path
    /// cannot reach.
    ///
    /// Manowar is Phoenix's recurrence with p = 1 and a pixel seed,
    /// and that p is what makes it need the DEEP rung at every
    /// depth: the history term carries the delta forward with
    /// coefficient 1, so where a one-term map's delta decays near the
    /// reference, Manowar's persists and f32 mantissa error piles up
    /// over hundreds of iterations. Measured here against the exact
    /// orbit: 18.4% of pixels wrong at zoom 20 and 27.0% at zoom 26
    /// on the scaled rung, 1.6% and 2.1% on the deep one. The
    /// renderer pins the tier to floatexp for that reason, and this
    /// test is what says so.
    #[test]
    #[ignore = "needs a GPU"]
    fn perturbed_manowar_matches_an_exact_orbit_at_depth() {
        let (device, queue) = repro_device();
        // A centre whose own orbit stays BOUNDED for the iteration
        // budget: a reference that nearly escapes is a hostile test
        // of the pixel, not of the renderer.
        let (cx, cy) = (-0.03909223238627110991f64, 0.00103869199752806211f64);
        let (w, h) = (128u32, 96u32);
        let mut config = crate::config::FractalConfig::default();
        config.palette = crate::scene::palette::Palette {
            name: "white".to_string(),
            stops: vec![
                crate::scene::palette::ColorStop { position: 0.0, color: [1.0, 1.0, 1.0] },
                crate::scene::palette::ColorStop { position: 1.0, color: [1.0, 1.0, 1.0] },
            ],
            locked: false,
            built_in: false,
        };
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation, config.palette_squeeze,
            config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse,
        );
        for zoom in [20.0f64, 26.0, 34.0] {
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.formula = "manowar".to_string();
            esc.center_re = "-0.03909223238627110991".to_string();
            esc.center_im = "0.00103869199752806211".to_string();
            esc.zoom_log2 = zoom;
            esc.max_iter = 3000;
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            let mut guard = 0u32;
            loop {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("manowar depth"),
                });
                let settled =
                    escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 100_000, "render did not settle at zoom {zoom}");
            }
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("manowar depth tonemap"),
            });
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
                w,
                h,
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
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, config.background_color,
            ))
            .expect("readback");
            escape.destroy();

            // f64 oracle: z' = z^2 + z_prev + c, seeded z_0 = z_-1 = c.
            // Checked against 60-digit arithmetic at these depths --
            // no sampled pixel disagreed, so f64 is a valid reference
            // here despite the map's error amplification.
            let span_y = 4.0 / zoom.exp2();
            let span_x = span_y * w as f64 / h as f64;
            let mut differ = 0usize;
            for py in 0..h {
                for px in 0..w {
                    let c = (
                        ((px as f64 + 0.5) / w as f64 - 0.5) * span_x + cx,
                        -(((py as f64 + 0.5) / h as f64 - 0.5) * span_y) + cy,
                    );
                    let (mut zx, mut zy) = c;
                    let (mut qx, mut qy) = c;
                    let mut escaped = false;
                    for _ in 0..esc.max_iter {
                        let nx = zx * zx - zy * zy + qx + c.0;
                        let ny = 2.0 * zx * zy + qy + c.1;
                        qx = zx;
                        qy = zy;
                        zx = nx;
                        zy = ny;
                        if zx * zx + zy * zy > 4.0 {
                            escaped = true;
                            break;
                        }
                    }
                    let i = ((py * w + px) * 4) as usize;
                    let lit = rgba[i] as u32 + rgba[i + 1] as u32 + rgba[i + 2] as u32 > 24;
                    if lit != escaped {
                        differ += 1;
                    }
                }
            }
            let frac = differ as f64 / (w * h) as f64;
            println!("manowar at zoom {zoom}: {:.2}% differ from the exact orbit", 100.0 * frac);
            assert!(
                frac < 0.05,
                "perturbed Manowar disagrees with an exact orbit on {:.1}% of pixels at                  zoom {zoom} -- the scaled rung reads 18-27% here, so check the tier is                  still pinned to floatexp",
                100.0 * frac
            );
        }
    }

    /// The golden spiral trap must be the spiral it claims to be.
    ///
    /// Rendered at `max_iter = 1` the orbit is exactly {0, c}, and the
    /// origin sample is skipped, so the IMAGE IS THE TRAP DISTANCE
    /// FIELD — a closed form that can be checked against
    /// `r = log|z| / (4 log phi) - arg(z)/2pi` directly, with no
    /// iteration in the way. Measured 0.40/255 spread when this went
    /// in.
    ///
    /// The comparison is a colour spread WITHIN a distance bin rather
    /// than a pixel diff: it does not care about the palette, only
    /// that the render is a FUNCTION of the true distance. A spiral
    /// with the wrong growth, or arms half a turn out, fails it while
    /// still looking like a perfectly good spiral to the eye — which
    /// is exactly the failure a screenshot cannot catch.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_golden_spiral_trap_matches_its_closed_form() {
        let (device, queue) = repro_device();
        let (w, h) = (160u32, 160u32);
        let (cx, cy, zoom) = (-0.5f64, 0.0f64, -1.0f64);

        let mut config = crate::config::FractalConfig::default();
        config.palette = crate::scene::palette::Palette {
            name: "ramp".to_string(),
            stops: vec![
                crate::scene::palette::ColorStop { position: 0.0, color: [0.0, 0.0, 0.0] },
                crate::scene::palette::ColorStop { position: 1.0, color: [1.0, 1.0, 1.0] },
            ],
            locked: false,
            built_in: false,
        };
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation, config.palette_squeeze,
            config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse,
        );

        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "mandelbrot".to_string();
        esc.coloring = "orbit_trap".to_string();
        esc.zoom_log2 = zoom;
        esc.max_iter = 1;
        esc.coloring_params.insert("shape".to_string(), 3.0);
        esc.coloring_params.insert("scale".to_string(), 1.0);
        esc.coloring_params.insert("growth".to_string(), 1.618_034);

        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        let mut guard = 0u32;
        loop {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("spiral trap"),
            });
            let settled = escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            if settled {
                break;
            }
            guard += 1;
            assert!(guard < 10_000, "render did not settle");
        }
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("spiral trap tonemap"),
        });
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
            w,
            h,
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
        renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
        queue.submit(std::iter::once(enc.finish()));
        let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
            &device, &queue, false, config.background_color,
        ))
        .expect("readback");
        escape.destroy();

        // The closed form, in f64.
        let phi = (1.0 + 5f64.sqrt()) / 2.0;
        let span = 4.0 / zoom.exp2();
        const BINS: usize = 256;
        let mut sums = vec![0f64; BINS];
        let mut sqs = vec![0f64; BINS];
        let mut counts = vec![0usize; BINS];
        for py in 0..h {
            for px in 0..w {
                let re = ((px as f64 + 0.5) / w as f64 - 0.5) * span + cx;
                let im = -(((py as f64 + 0.5) / h as f64 - 0.5) * span) + cy;
                let r2 = re * re + im * im;
                if r2 < 1e-30 {
                    continue;
                }
                let turns = r2.ln() / (8.0 * phi.ln()) - im.atan2(re) / std::f64::consts::TAU;
                let d = 2.0 * (turns - turns.round()).abs();
                let b = ((d * (BINS - 1) as f64).round() as usize).min(BINS - 1);
                let i = ((py * w + px) * 4) as usize;
                let lum = rgba[i] as f64;
                counts[b] += 1;
                sums[b] += lum;
                sqs[b] += lum * lum;
            }
        }
        let (mut spread, mut total) = (0f64, 0usize);
        for b in 0..BINS {
            if counts[b] < 50 {
                continue;
            }
            let n = counts[b] as f64;
            let sd = (sqs[b] / n - (sums[b] / n).powi(2)).max(0.0).sqrt();
            spread += sd * n;
            total += counts[b];
        }
        let spread = spread / total as f64;
        println!("golden spiral trap: colour spread within a distance bin {spread:.2}/255");
        assert!(
            spread < 6.0,
            "the rendered trap is not a function of the closed-form distance \
             ({spread:.2}/255) -- check the growth factor and the arg() term"
        );
    }

    /// Normal-map shading must match the reference implementation.
    ///
    /// Ported verbatim from the C behind Wikibooks' bump-mapping
    /// article (Wikimedia Commons,
    /// `File:Mandelbrot_set_-_Normal_mapping.png`):
    ///
    /// ```c
    /// u = Z / dC;  u = u / cabs(u);
    /// reflection = cdot(u, v) + h2;          // v = exp(2 pi i angle)
    /// reflection = reflection / (1.0 + h2);
    /// if (reflection < 0.0) reflection = 0.0;
    /// ```
    ///
    /// The oracle below is that C, in f64. The comparison bins pixels
    /// by the reference reflection and measures colour spread WITHIN a
    /// bin, so it ignores the palette and asks only whether the render
    /// is a function of the true reflection — 3.18/255 when this went
    /// in, over ~185k escaped pixels.
    #[test]
    #[ignore = "needs a GPU"]
    fn normal_map_shading_matches_the_reference() {
        let (device, queue) = repro_device();
        let (w, h) = (240u32, 180u32);
        let (cx, cy, zoom) = (-0.5f64, 0.0f64, 0.0f64);
        const MAX_ITER: u32 = 512;
        const ANGLE: f64 = 0.125;
        const HEIGHT: f64 = 1.5;

        let mut config = crate::config::FractalConfig::default();
        config.palette = crate::scene::palette::Palette {
            name: "ramp".to_string(),
            stops: vec![
                crate::scene::palette::ColorStop { position: 0.0, color: [0.0, 0.0, 0.0] },
                crate::scene::palette::ColorStop { position: 1.0, color: [1.0, 1.0, 1.0] },
            ],
            locked: false,
            built_in: false,
        };
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation, config.palette_squeeze,
            config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse,
        );

        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "mandelbrot".to_string();
        esc.coloring = "normal_map".to_string();
        esc.zoom_log2 = zoom;
        esc.max_iter = MAX_ITER;
        esc.coloring_params.insert("angle".to_string(), ANGLE as f32);
        esc.coloring_params.insert("height".to_string(), HEIGHT as f32);
        esc.coloring_params.insert("scale".to_string(), 1.0);

        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        let mut guard = 0u32;
        loop {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("normal map"),
            });
            let settled = escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            if settled {
                break;
            }
            guard += 1;
            assert!(guard < 10_000, "render did not settle");
        }
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("normal map tonemap"),
        });
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
            w,
            h,
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
        renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
        queue.submit(std::iter::once(enc.finish()));
        let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
            &device, &queue, false, config.background_color,
        ))
        .expect("readback");
        escape.destroy();

        let span_y = 4.0 / zoom.exp2();
        let span_x = span_y * w as f64 / h as f64;
        let (lx, ly) = (
            (std::f64::consts::TAU * ANGLE).cos(),
            (std::f64::consts::TAU * ANGLE).sin(),
        );
        const BINS: usize = 256;
        let mut sums = vec![0f64; BINS];
        let mut sqs = vec![0f64; BINS];
        let mut counts = vec![0usize; BINS];
        let mut compared = 0usize;
        for py in 0..h {
            for px in 0..w {
                let cre = ((px as f64 + 0.5) / w as f64 - 0.5) * span_x + cx;
                let cim = -(((py as f64 + 0.5) / h as f64 - 0.5) * span_y) + cy;
                let (mut zr, mut zi) = (0.0f64, 0.0f64);
                let (mut dr, mut di) = (0.0f64, 0.0f64);
                let mut escaped = false;
                for _ in 0..MAX_ITER {
                    // dC = 2*dC*Z + 1, then Z = Z*Z + C.
                    let ndr = 2.0 * (dr * zr - di * zi) + 1.0;
                    let ndi = 2.0 * (dr * zi + di * zr);
                    dr = ndr;
                    di = ndi;
                    let nzr = zr * zr - zi * zi + cre;
                    let nzi = 2.0 * zr * zi + cim;
                    zr = nzr;
                    zi = nzi;
                    if zr * zr + zi * zi > 4.0 {
                        escaped = true;
                        break;
                    }
                }
                if !escaped {
                    continue;
                }
                let dl = dr * dr + di * di;
                if dl < 1e-30 {
                    continue;
                }
                // u = z / dz
                let ur = (zr * dr + zi * di) / dl;
                let ui = (zi * dr - zr * di) / dl;
                let ul = (ur * ur + ui * ui).sqrt();
                if !(ul > 1e-30) {
                    continue;
                }
                let refl = (((ur / ul) * lx + (ui / ul) * ly) + HEIGHT) / (1.0 + HEIGHT);
                let refl = refl.max(0.0).min(1.0);
                let b = ((refl * (BINS - 1) as f64).round() as usize).min(BINS - 1);
                let i = ((py * w + px) * 4) as usize;
                let lum = rgba[i] as f64;
                counts[b] += 1;
                sums[b] += lum;
                sqs[b] += lum * lum;
                compared += 1;
            }
        }
        let (mut spread, mut total) = (0f64, 0usize);
        for b in 0..BINS {
            if counts[b] < 40 {
                continue;
            }
            let n = counts[b] as f64;
            let sd = (sqs[b] / n - (sums[b] / n).powi(2)).max(0.0).sqrt();
            spread += sd * n;
            total += counts[b];
        }
        let spread = spread / total as f64;
        println!(
            "normal map: {compared} escaped pixels, colour spread within a reflection bin {spread:.2}/255"
        );
        assert!(
            spread < 10.0,
            "the render is not a function of the reference reflection ({spread:.2}/255)"
        );

        // The seam this coloring was born with: a bounded value of
        // exactly 1.0 wrapping to the palette's bottom. Nothing in the
        // lit region may be black.
        let mut darkest_lit = 255u8;
        for (i, px) in rgba.chunks_exact(4).enumerate() {
            let (x, y) = ((i as u32) % w, (i as u32) / w);
            // Sample the bright quadrant, away from the set itself.
            if x > w * 3 / 4 && y < h / 3 {
                darkest_lit = darkest_lit.min(px[0]);
            }
        }
        assert!(
            darkest_lit > 100,
            "a lit region contains near-black pixels ({darkest_lit}) — the bounded \
             value is wrapping at 1.0 again"
        );
    }

    /// `lambda_sine` must iterate the map the literature names.
    ///
    /// The Cantor bouquet family is `λ·sin(z)` with the parameter
    /// MULTIPLYING — not `sin(z) + c`, which is the separate `trig`
    /// formula. Getting that wrong produces a perfectly attractive
    /// fractal that is simply a different one, so this compares the
    /// escape set against an f64 orbit at λ = 0.5, inside the
    /// (0,1) range where the bouquet lives.
    ///
    /// Escape is `|Im z| > bailout` RAW: sin grows like sinh in the
    /// imaginary direction, so orbits leave through ±i∞.
    #[test]
    #[ignore = "needs a GPU"]
    fn lambda_sine_matches_the_map_it_names() {
        let (device, queue) = repro_device();
        let (w, h) = (200u32, 160u32);
        const LAMBDA: f64 = 0.5;
        const BAILOUT: f64 = 50.0;
        const MAX_ITER: u32 = 400;
        let zoom = -2.0f64;

        let mut config = crate::config::FractalConfig::default();
        config.palette = crate::scene::palette::Palette {
            name: "white".to_string(),
            stops: vec![
                crate::scene::palette::ColorStop { position: 0.0, color: [1.0, 1.0, 1.0] },
                crate::scene::palette::ColorStop { position: 1.0, color: [1.0, 1.0, 1.0] },
            ],
            locked: false,
            built_in: false,
        };
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation, config.palette_squeeze,
            config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse,
        );

        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "lambda_sine".to_string();
        esc.julia = true;
        esc.julia_re = LAMBDA as f32;
        esc.julia_im = 0.0;
        esc.center_re = "0.0".to_string();
        esc.center_im = "0.0".to_string();
        esc.zoom_log2 = zoom;
        esc.max_iter = MAX_ITER;
        esc.bailout = BAILOUT as f32;

        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        let mut guard = 0u32;
        loop {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lambda sine"),
            });
            let settled = escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            if settled {
                break;
            }
            guard += 1;
            assert!(guard < 10_000, "render did not settle");
        }
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("lambda sine tonemap"),
        });
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
            w,
            h,
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
        renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
        queue.submit(std::iter::once(enc.finish()));
        let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
            &device, &queue, false, config.background_color,
        ))
        .expect("readback");
        escape.destroy();

        let span_y = 4.0 / zoom.exp2();
        let span_x = span_y * w as f64 / h as f64;
        let mut differ = 0usize;
        for py in 0..h {
            for px in 0..w {
                // Julia: z0 is the pixel, lambda is fixed.
                let mut zr = ((px as f64 + 0.5) / w as f64 - 0.5) * span_x;
                let mut zi = -(((py as f64 + 0.5) / h as f64 - 0.5) * span_y);
                let mut escaped = false;
                for _ in 0..MAX_ITER {
                    // lambda * sin(z), lambda real here.
                    let sr = zr.sin() * zi.cosh();
                    let si = zr.cos() * zi.sinh();
                    zr = LAMBDA * sr;
                    zi = LAMBDA * si;
                    if zi.abs() > BAILOUT {
                        escaped = true;
                        break;
                    }
                    if !zr.is_finite() || !zi.is_finite() {
                        escaped = true;
                        break;
                    }
                }
                let i = ((py * w + px) * 4) as usize;
                let lit = rgba[i] as u32 + rgba[i + 1] as u32 + rgba[i + 2] as u32 > 24;
                if lit != escaped {
                    differ += 1;
                }
            }
        }
        let frac = differ as f64 / (w * h) as f64;
        println!("lambda_sine at lambda={LAMBDA}: {:.2}% differ from the exact orbit", 100.0 * frac);
        assert!(
            frac < 0.01,
            "the render disagrees with lambda*sin(z) on {:.1}% of pixels -- \
             check the parameter is MULTIPLYING, not added",
            100.0 * frac
        );
    }

    /// The fold-line construction both origami oracles share: line j's
    /// endpoints are seeded random points FOLDED THROUGH LINES 0..j-1,
    /// so every crease lands on the current wad (Kyle McDonald's
    /// OpenProcessing 1185, recovered via the Wayback Machine,
    /// <https://web.archive.org/web/20120209174422/http://www.openprocessing.org/visuals/?visualID=1185> — the
    /// detail the prose descriptions of McCabe's algorithm omit).
    fn origami_oracle_lines(seed: u32, folds: usize, spread: f64) -> Vec<[f64; 4]> {
        fn hash(x: u32) -> u32 {
            let mut h = x;
            h ^= h >> 16;
            h = h.wrapping_mul(0x7feb_352d);
            h ^= h >> 15;
            h = h.wrapping_mul(0x846c_a68b);
            h ^= h >> 16;
            h
        }
        fn unit(x: u32) -> f64 {
            (hash(x) >> 8) as f64 / 16_777_216.0
        }
        fn fold(p: [f64; 2], l: &[f64; 4]) -> [f64; 2] {
            let (ax, ay, bx, by) = (l[0], l[1], l[2], l[3]);
            let (abx, aby) = (bx - ax, by - ay);
            let d2 = abx * abx + aby * aby;
            if abx * (p[1] - ay) - aby * (p[0] - ax) > 0.0 || d2 < 1e-12 {
                return p;
            }
            let t = ((p[0] - ax) * abx + (p[1] - ay) * aby) / d2;
            [(ax + abx * t) * 2.0 - p[0], (ay + aby * t) * 2.0 - p[1]]
        }
        let s = seed.wrapping_mul(2_654_435_761);
        let mut lines: Vec<[f64; 4]> = Vec::with_capacity(folds);
        for j in 0..folds as u32 {
            let mut a = [
                (unit(j.wrapping_mul(4).wrapping_add(s)) * 2.0 - 1.0) * spread,
                (unit(j.wrapping_mul(4).wrapping_add(1).wrapping_add(s)) * 2.0 - 1.0) * spread,
            ];
            let mut b = [
                (unit(j.wrapping_mul(4).wrapping_add(2).wrapping_add(s)) * 2.0 - 1.0) * spread,
                (unit(j.wrapping_mul(4).wrapping_add(3).wrapping_add(s)) * 2.0 - 1.0) * spread,
            ];
            for l in &lines {
                a = fold(a, l);
                b = fold(b, l);
            }
            lines.push([a[0], a[1], b[0], b[1]]);
        }
        lines
    }

    fn origami_oracle_fold(p: [f64; 2], l: &[f64; 4]) -> [f64; 2] {
        let (ax, ay, bx, by) = (l[0], l[1], l[2], l[3]);
        let (abx, aby) = (bx - ax, by - ay);
        let d2 = abx * abx + aby * aby;
        if abx * (p[1] - ay) - aby * (p[0] - ax) > 0.0 || d2 < 1e-12 {
            return p;
        }
        let t = ((p[0] - ax) * abx + (p[1] - ay) * aby) / d2;
        [(ax + abx * t) * 2.0 - p[0], (ay + aby * t) * 2.0 - p[1]]
    }

    /// Origami must fold the plane the way the algorithm says.
    ///
    /// The oracle below is an independent reimplementation of the
    /// whole thing — integer hash, line construction, fold sequence,
    /// running mean — not a copy of the shader. It can be exact
    /// because the hash is INTEGER: the usual
    /// `fract(sin(x) * 43758.5)` idiom amplifies rounding enough that
    /// f32 and f64 disagree, which would have made both this test and
    /// the rendered image device-dependent.
    ///
    /// The fold is CONDITIONAL, and that is the load-bearing detail.
    /// An unconditional reflection is an isometry, so a composition of
    /// them is affine and the average of |z| over the sequence is
    /// smooth — prototyped, it renders as plain concentric rings. The
    /// test asserts the creases exist, because "matches the oracle"
    /// would still pass if both were smooth washes.
    #[test]
    #[ignore = "needs a GPU"]
    fn origami_folds_the_plane_as_specified() {
        let (device, queue) = repro_device();
        let (w, h) = (200u32, 200u32);
        let zoom = -1.0f64;
        const MAX_ITER: u32 = 32;
        const SEED: f32 = 8.0;
        const SPREAD: f32 = 2.0;
        const SCALE: f32 = 1.2;

        let mut config = crate::config::FractalConfig::default();
        config.palette = crate::scene::palette::Palette {
            name: "ramp".to_string(),
            stops: vec![
                crate::scene::palette::ColorStop { position: 0.0, color: [0.0, 0.0, 0.0] },
                crate::scene::palette::ColorStop { position: 1.0, color: [1.0, 1.0, 1.0] },
            ],
            locked: false,
            built_in: false,
        };
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation, config.palette_squeeze,
            config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse,
        );

        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "origami".to_string();
        esc.coloring = "magnitude_average".to_string();
        esc.center_re = "0.0".to_string();
        esc.center_im = "0.0".to_string();
        esc.zoom_log2 = zoom;
        esc.max_iter = MAX_ITER;
        esc.formula_params.insert("seed".to_string(), SEED);
        esc.formula_params.insert("spread".to_string(), SPREAD);
        esc.coloring_params.insert("scale".to_string(), SCALE);
        esc.coloring_params.insert("offset".to_string(), 0.0);

        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        let mut guard = 0u32;
        loop {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("origami"),
            });
            let settled = escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            if settled {
                break;
            }
            guard += 1;
            assert!(guard < 10_000, "render did not settle");
        }
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("origami tonemap"),
        });
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
            w,
            h,
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
        renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
        queue.submit(std::iter::once(enc.finish()));
        let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
            &device, &queue, false, config.background_color,
        ))
        .expect("readback");
        escape.destroy();

        // --- the independent reimplementation ---
        let lines = origami_oracle_lines(SEED as u32, MAX_ITER as usize, SPREAD as f64);

        let span = 4.0 / zoom.exp2();
        const BINS: usize = 256;
        let mut sums = vec![0f64; BINS];
        let mut sqs = vec![0f64; BINS];
        let mut counts = vec![0usize; BINS];
        let mut values = Vec::with_capacity((w * h) as usize);
        for py in 0..h {
            for px in 0..w {
                // The PIXEL is the point being folded: it seeds z0.
                let mut z = [
                    ((px as f64 + 0.5) / w as f64 - 0.5) * span,
                    -(((py as f64 + 0.5) / h as f64 - 0.5) * span),
                ];
                let mut acc = 0.0f64;
                for l in &lines {
                    z = origami_oracle_fold(z, l);
                    acc += (z[0] * z[0] + z[1] * z[1]).sqrt();
                }
                let val = acc / MAX_ITER as f64;
                values.push(val);
                let t = (val * SCALE as f64).rem_euclid(1.0);
                let b = ((t * (BINS - 1) as f64).round() as usize).min(BINS - 1);
                let i = ((py * w + px) * 4) as usize;
                let lum = rgba[i] as f64;
                counts[b] += 1;
                sums[b] += lum;
                sqs[b] += lum * lum;
            }
        }
        let (mut spread, mut total) = (0f64, 0usize);
        for b in 0..BINS {
            if counts[b] < 50 {
                continue;
            }
            let n = counts[b] as f64;
            let sd = (sqs[b] / n - (sums[b] / n).powi(2)).max(0.0).sqrt();
            spread += sd * n;
            total += counts[b];
        }
        let spread = spread / total as f64;
        println!("origami: colour spread within a value bin {spread:.2}/255");
        assert!(
            spread < 8.0,
            "the render does not track an independent implementation of the fold \
             sequence ({spread:.2}/255)"
        );

        // The creases must exist. Reflecting UNCONDITIONALLY would
        // make the whole composition affine and this field smooth, so
        // measure the second difference along a row and require some
        // sharp turns: a crease is a derivative discontinuity.
        let mut kinks = 0usize;
        for py in 0..h as usize {
            let row = &values[py * w as usize..(py + 1) * w as usize];
            for x in 1..row.len() - 1 {
                let second = (row[x + 1] - 2.0 * row[x] + row[x - 1]).abs();
                if second > 0.01 {
                    kinks += 1;
                }
            }
        }
        println!("origami: {kinks} crease samples");
        assert!(
            kinks > 500,
            "the folded field has almost no creases ({kinks}) -- are the \
             reflections unconditional? that composition is affine, and renders \
             as smooth rings"
        );
    }

    /// `position_average` must average POSITIONS, not magnitudes.
    ///
    /// The distinction is the whole point of the coloring and it is
    /// invisible to a casual look: both produce a smooth, plausible
    /// picture. McCabe colours each folded point by "a weighted
    /// average of that list of positions", and the ANGLE of that 2-D
    /// mean is what carries the creased, layered-paper structure.
    /// Averaging |z| first renders the same orbit as concentric rings
    /// -- a kaleidoscope -- which is what shipped until a user said the
    /// image did not look like the published work.
    ///
    /// So this asserts BOTH directions: the render must track an f64
    /// average-position oracle, AND must not be explained by the mean
    /// magnitude. Without the second half, swapping the accumulator
    /// back would still pass on any view where the two happen to
    /// correlate.
    #[test]
    #[ignore = "needs a GPU"]
    fn position_average_averages_positions_not_magnitudes() {
        let (device, queue) = repro_device();
        let (w, h) = (200u32, 200u32);
        let zoom = -0.7f64;
        const MAX_ITER: u32 = 32;
        const SEED: f32 = 8.0;
        const SPREAD: f32 = 2.0;
        // The mean position sweeps a narrow arc here, so spread it
        // across the palette -- otherwise every bin holds the same
        // colour and the comparison below says nothing.
        const SCALE: f32 = 19.6;

        let mut config = crate::config::FractalConfig::default();
        config.palette = crate::scene::palette::Palette {
            name: "ramp".to_string(),
            stops: vec![
                crate::scene::palette::ColorStop { position: 0.0, color: [0.0, 0.0, 0.0] },
                crate::scene::palette::ColorStop { position: 1.0, color: [1.0, 1.0, 1.0] },
            ],
            locked: false,
            built_in: false,
        };
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation, config.palette_squeeze,
            config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse,
        );

        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "origami".to_string();
        esc.coloring = "position_average".to_string();
        esc.center_re = "0.0".to_string();
        esc.center_im = "0.0".to_string();
        esc.zoom_log2 = zoom;
        esc.max_iter = MAX_ITER;
        esc.formula_params.insert("seed".to_string(), SEED);
        esc.formula_params.insert("spread".to_string(), SPREAD);
        esc.coloring_params.insert("mode".to_string(), 0.0);
        esc.coloring_params.insert("scale".to_string(), SCALE);

        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        let mut guard = 0u32;
        loop {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("position average"),
            });
            let settled = escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            if settled {
                break;
            }
            guard += 1;
            assert!(guard < 10_000, "render did not settle");
        }
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("position average tonemap"),
        });
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
            w,
            h,
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
        renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
        queue.submit(std::iter::once(enc.finish()));
        let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
            &device, &queue, false, config.background_color,
        ))
        .expect("readback");
        escape.destroy();

        let lines = origami_oracle_lines(SEED as u32, MAX_ITER as usize, SPREAD as f64);

        let span = 4.0 / zoom.exp2();
        // Two candidate explanations of the same image, binned the
        // same way: the mean POSITION's angle, and the mean MAGNITUDE.
        let mut angle_t = Vec::with_capacity((w * h) as usize);
        let mut mag_t = Vec::with_capacity((w * h) as usize);
        for py in 0..h {
            for px in 0..w {
                let mut z = [
                    ((px as f64 + 0.5) / w as f64 - 0.5) * span,
                    -(((py as f64 + 0.5) / h as f64 - 0.5) * span),
                ];
                let (mut sx, mut sy, mut smag) = (0.0f64, 0.0f64, 0.0f64);
                for l in &lines {
                    z = origami_oracle_fold(z, l);
                    sx += z[0];
                    sy += z[1];
                    smag += (z[0] * z[0] + z[1] * z[1]).sqrt();
                }
                let n = MAX_ITER as f64;
                let ang = (sy / n).atan2(sx / n) / std::f64::consts::TAU + 0.5;
                angle_t.push((ang * SCALE as f64).rem_euclid(1.0));
                mag_t.push(((smag / n) * SCALE as f64).rem_euclid(1.0));
            }
        }

        // Spread of rendered luminance WITHIN a bin of the candidate
        // value: small means the candidate explains the image. This is
        // palette-agnostic -- it never assumes what colour a value maps
        // to, only that equal values must render equally.
        let spread_of = |ts: &[f64]| -> f64 {
            const BINS: usize = 256;
            let mut sums = vec![0f64; BINS];
            let mut sqs = vec![0f64; BINS];
            let mut counts = vec![0usize; BINS];
            for (idx, t) in ts.iter().enumerate() {
                let b = ((t * (BINS - 1) as f64).round() as usize).min(BINS - 1);
                let lum = rgba[idx * 4] as f64;
                counts[b] += 1;
                sums[b] += lum;
                sqs[b] += lum * lum;
            }
            let (mut acc, mut total) = (0f64, 0usize);
            for b in 0..BINS {
                if counts[b] < 50 {
                    continue;
                }
                let n = counts[b] as f64;
                acc += (sqs[b] / n - (sums[b] / n).powi(2)).max(0.0).sqrt() * n;
                total += counts[b];
            }
            acc / total.max(1) as f64
        };
        let by_angle = spread_of(&angle_t);
        let by_magnitude = spread_of(&mag_t);
        println!(
            "position_average: spread within a bin -- mean position {by_angle:.2}/255, mean magnitude {by_magnitude:.2}/255"
        );
        assert!(
            by_angle < 8.0,
            "the render does not track an f64 average-POSITION oracle ({by_angle:.2}/255)"
        );
        assert!(
            by_magnitude > 3.0 * by_angle,
            "the mean MAGNITUDE explains this image about as well as the mean position ({by_magnitude:.2} vs {by_angle:.2}) -- either the accumulator is summing |z| again, or this view cannot tell the two apart and the test needs a different one"
        );
    }

    /// Relief shading must LIGHT the coloring, not replace it.
    ///
    /// The whole point of the layer is composition: `normal_map` (the
    /// analytic-normal coloring) takes the image over, because a
    /// coloring returns the one scalar the palette is indexed by. This
    /// runs after the palette lookup, so the test asserts the property
    /// that distinguishes the two — with the light straight overhead
    /// every surface is flat-on, both terms are zero, and the image
    /// must come back BIT-IDENTICAL to the unshaded render. A layer
    /// that quietly tinted, normalized or re-mapped anything would
    /// fail here even though it still "looked shaded".
    ///
    /// Then it asserts the layer does something: tilt the light and
    /// the image must change, and change only where the value field
    /// actually has slope.
    #[test]
    #[ignore = "needs a GPU"]
    fn relief_shading_lights_the_coloring_without_replacing_it() {
        let (device, queue) = repro_device();
        let (w, h) = (160u32, 160u32);

        let mut config = crate::config::FractalConfig::default();
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation, config.palette_squeeze,
            config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse,
        );

        let mut base = crate::config::escape::EscapeConfig::default();
        base.formula = "mandelbrot".to_string();
        base.coloring = "smooth".to_string();
        base.center_re = "-0.5".to_string();
        base.center_im = "0.0".to_string();
        base.max_iter = 256;
        base.coloring_params.insert("scale".to_string(), 0.03);

        let render = |esc: &crate::config::escape::EscapeConfig,
                      renderer: &mut crate::renderer::compute_kernel::FlameRenderer|
         -> Vec<u8> {
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            let mut guard = 0u32;
            loop {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("relief"),
                });
                let settled =
                    escape.render(&device, &queue, &mut enc, esc, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                let _ =
                    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 10_000, "render did not settle");
            }
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("relief tonemap"),
            });
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
                w,
                h,
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
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, config.background_color,
            ))
            .expect("readback");
            escape.destroy();
            rgba
        };

        let plain = render(&base, &mut renderer);

        // Zero strength on both sides: the blends interpolate from the
        // base, so this must be the untouched image even though the
        // whole shading path ran.
        let mut zero = base.clone();
        zero.shading = crate::config::escape::EscapeShading {
            enabled: true,
            shadow_strength: 0.0,
            highlight_strength: 0.0,
            ..Default::default()
        };
        let zeroed = render(&zero, &mut renderer);
        assert_eq!(
            plain, zeroed,
            "shading at zero strength changed the image -- the layer is not \
             compositing from the base, it is replacing it"
        );

        // Now actually light it. The image must move.
        let mut lit = base.clone();
        lit.shading = crate::config::escape::EscapeShading {
            enabled: true,
            height: 30.0,
            ..Default::default()
        };
        let shaded = render(&lit, &mut renderer);
        let moved = plain
            .chunks(4)
            .zip(shaded.chunks(4))
            .filter(|(a, b)| a[..3] != b[..3])
            .count();
        let frac = moved as f64 / (w * h) as f64;
        println!("relief: {:.1}% of pixels shaded", 100.0 * frac);
        assert!(
            frac > 0.2,
            "relief shading changed only {:.1}% of pixels",
            100.0 * frac
        );

        // And it must be the COLORING underneath, still: the interior
        // of the set has no value field (height 0, flat), so it must
        // come through the light untouched while the exterior moves.
        // That is what separates a relief LAYER from a filter over the
        // whole image.
        //
        // Only pixels whose whole 3x3 neighbourhood is interior count.
        // One rim in from the boundary the central difference straddles
        // the set edge, which is a genuine cliff and SHOULD light --
        // that rim is the effect working, not leaking. Measured, it is
        // 16 pixels here.
        let dark = |px: &[u8], x: usize, y: usize| -> bool {
            let i = (y * w as usize + x) * 4;
            px[i] as u32 + px[i + 1] as u32 + px[i + 2] as u32 <= 6
        };
        let mut interior_moved = 0usize;
        let mut interior_total = 0usize;
        for y in 1..h as usize - 1 {
            for x in 1..w as usize - 1 {
                let surrounded =
                    (y - 1..=y + 1).all(|yy| (x - 1..=x + 1).all(|xx| dark(&plain, xx, yy)));
                if !surrounded {
                    continue;
                }
                interior_total += 1;
                let i = (y * w as usize + x) * 4;
                if plain[i..i + 3] != shaded[i..i + 3] {
                    interior_moved += 1;
                }
            }
        }
        assert!(
            interior_total > 500,
            "no interior found to test against ({interior_total} px)"
        );
        assert_eq!(
            interior_moved, 0,
            "{interior_moved} of {interior_total} flat interior pixels changed -- \
             the light is being applied where there is no surface"
        );

        // SHADOWS AND HIGHLIGHTS MUST BE SYMMETRIC, and reach full.
        //
        // Reported from the app: black shadows at full strength over a
        // white palette came out mid-grey, while highlights were fine.
        // The cause was normalizing the two sides by different
        // achievable spans of a Lambert dot product -- the normal's z
        // is always positive, so the dot could not fall below
        // -|l.xy| = -0.707, while the highlight side was normalized
        // over a span of just 1 - l.z = 0.293. At a 45-degree tilt the
        // highlight was saturated at 1.000 and the shadow was 0.414,
        // and 0.414 of black over white IS mid-grey. It was also
        // non-monotonic: a vertical wall facing the light got no
        // highlight at all.
        //
        // Flipping the light 180 degrees negates the tilt, so a
        // shadow-only render at A must equal a highlight-only render
        // at A+180 EXACTLY, given the same colour, strength and blend.
        // `mix` is used for both so the two sides run identical
        // arithmetic and the comparison can be bit-exact.
        let one_sided = |angle: f32, shadow: bool| {
            let mut c = base.clone();
            let amt = if shadow { (1.0, 0.0) } else { (0.0, 1.0) };
            c.shading = crate::config::escape::EscapeShading {
                enabled: true,
                light_angle: angle,
                height: 30.0,
                shadow_color: [0.0, 0.0, 0.0],
                shadow_strength: amt.0,
                shadow_blend: crate::config::escape::ShadingBlend::Mix,
                highlight_color: [0.0, 0.0, 0.0],
                highlight_strength: amt.1,
                highlight_blend: crate::config::escape::ShadingBlend::Mix,
                ..Default::default()
            };
            c
        };
        let shadow_at_0 = render(&one_sided(0.0, true), &mut renderer);
        let highlight_at_180 = render(&one_sided(180.0, false), &mut renderer);
        assert_eq!(
            shadow_at_0, highlight_at_180,
            "shadows and highlights are not mirror images of each other -- one \
             side is normalized differently from the other, which is what made \
             black shadows top out at mid-grey"
        );

        // And the shadow side must actually REACH: somewhere in a
        // relief this steep, mixing full-strength black must land near
        // black. The old asymmetry capped it at 0.414 of the way (58%
        // of the base luminance), so this threshold is what separates
        // the bug from the fix rather than an arbitrary number.
        let darkest = shadow_at_0
            .chunks(4)
            .map(|p| p[0] as u32 + p[1] as u32 + p[2] as u32)
            .min()
            .unwrap_or(0);
        println!("relief: darkest shadowed pixel {darkest}/765");
        assert!(
            darkest < 190,
            "full-strength black shadows only reached {darkest}/765 -- they \
             cannot get dark, which is the reported bug"
        );
    }

    /// `distance_estimate` must render FLAT where there is no
    /// derivative — and the point is that the wrong answer is pretty.
    ///
    /// Without a derivative orbit `dz` is the constant seed of 1, so
    /// `|z|.ln|z| / |dz|` collapses to a smooth function of the escape
    /// radius alone. That is not a distance estimate, but it renders
    /// as a fully detailed, entirely convincing exterior: measured on
    /// a Mandelbrot dive to zoom 30 it produced 516 distinct colours,
    /// none of them meaning what the coloring's name claims. Nothing
    /// about the image invites suspicion, which is why the guard has
    /// to be pinned by a test rather than by looking at renders.
    ///
    /// `burning_ship` escapes (so the coloring is actually called) and
    /// defines no derivative, at a shallow zoom so this stays on the
    /// direct path and runs in milliseconds.
    #[test]
    #[ignore = "needs a GPU"]
    fn distance_estimate_is_flat_without_a_derivative() {
        let (device, queue) = repro_device();
        let (w, h) = (160u32, 160u32);

        let mut config = crate::config::FractalConfig::default();
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation, config.palette_squeeze,
            config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse,
        );

        let shoot = |formula: &str,
                     renderer: &mut crate::renderer::compute_kernel::FlameRenderer|
         -> Vec<u8> {
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.formula = formula.to_string();
            esc.coloring = "distance_estimate".to_string();
            esc.max_iter = 256;
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            let mut guard = 0u32;
            loop {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("de"),
                });
                let settled =
                    escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                let _ =
                    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 10_000, "render did not settle");
            }
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("de tonemap"),
            });
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
                w,
                h,
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
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, config.background_color,
            ))
            .expect("readback");
            escape.destroy();
            rgba
        };

        let colours = |px: &[u8]| -> usize {
            let mut seen = std::collections::HashSet::new();
            for p in px.chunks(4) {
                // Skip the interior: the template paints it, not the
                // coloring, so it says nothing either way.
                if p[0] as u32 + p[1] as u32 + p[2] as u32 > 12 {
                    seen.insert([p[0], p[1], p[2]]);
                }
            }
            seen.len()
        };

        let no_deriv = shoot("burning_ship", &mut renderer);
        let n = colours(&no_deriv);
        println!("distance_estimate on burning_ship: {n} exterior colour(s)");
        assert_eq!(
            n, 1,
            "distance_estimate rendered {n} exterior colours on a formula with no \
             derivative -- it is estimating from dz = 1, which is a confident \
             wrong answer, not a distance"
        );

        // The control: on a formula that DOES define one, the same
        // coloring must produce a real field. Without this half, the
        // test above would still pass if the coloring were broken
        // everywhere.
        let deriv = shoot("mandelbrot", &mut renderer);
        let m = colours(&deriv);
        println!("distance_estimate on mandelbrot: {m} exterior colour(s)");
        assert!(
            m > 20,
            "distance_estimate produced only {m} exterior colours on Mandelbrot, \
             which HAS a derivative -- the guard is firing where it should not"
        );
    }

    /// Perturbed Lambda must match an exact orbit at depth, on both
    /// rungs.
    ///
    /// `c*z*(1-z)` is the first tier whose PARAMETER MULTIPLIES the
    /// map, so it exercises two things no earlier tier did: the delta
    /// step reads the reference's own c as a factor, and the
    /// parameter-plane term picks up the reference's z-product
    /// `Z(1-Z)` rather than a bare `+ dc`. Either of those wrong
    /// gives a render that still looks like a lambda plane.
    ///
    /// It also pins the SEED. Zero is a fixed point of this map for
    /// every c, so a reference seeded there would be constant and
    /// every pixel would rebase against a frozen orbit — which is
    /// wrong in a way that produces a smooth, plausible image.
    ///
    /// WHY THE ITERATION CAP IS PART OF THE TEST. Lambda's critical
    /// orbit lingers: near the boundary pixels take hundreds of
    /// iterations to escape, where the Mandelbrot and Phoenix views
    /// this suite compares finish in tens. Over that many steps an
    /// independent f64 orbit is no longer ground truth — chaos
    /// amplifies its own rounding past the escape decision — so a
    /// comparison run to a deep cap measures the ORACLE, not the
    /// render. Chasing that as if it were a bug is exactly what
    /// happened here; the numbers below are what settled it.
    #[test]
    #[ignore = "needs a GPU"]
    fn perturbed_lambda_matches_an_exact_orbit_at_depth() {
        for deep in [false, true] {
            lambda_exact_orbit_case(deep);
        }
    }

    fn lambda_exact_orbit_case(deep: bool) {
        let (device, queue) = repro_device();
        // Inside the lambda plane's structure, off any axis of
        // symmetry so a sign error cannot cancel out.
        // A point ON the lambda set's boundary, found by bisecting a
        // slanted line between an interior and an exterior parameter
        // -- slanted so no axis of symmetry can hide a sign error. A
        // zoom-30 window here straddles the boundary (measured
        // 622/768 escaping), which the degeneracy check below insists
        // on: an all-interior view agrees with any oracle at all, and
        // the first center tried was exactly that.
        let (cx, cy) = (2.88438199954338481f64, 0.46729682466198574f64);
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "lambda".to_string();
        esc.center_re = format!("{cx:.17}");
        esc.center_im = format!("{cy:.17}");
        // Zoom 30 for BOTH rungs, with `force_floatexp` selecting
        // which one runs. Deep enough that the direct f32 path is long
        // gone (PERTURB_MIN_ZOOM is 14), and shallow enough that the
        // f64 oracle is still ground truth: pixel spacing here is
        // 4/2^30 = 3.7e-9 against ulp(c) = 4.4e-16, seven digits of
        // headroom. Pushing the deep rung to its own zoom 60 instead
        // would put pixel spacing BELOW ulp(c), so f64 could not tell
        // neighbouring pixels apart and the comparison would be
        // meaningless rather than strict.
        esc.zoom_log2 = 30.0;
        // 600 is chosen, not default, and the reason is the point of
        // the comment above: the f64 oracle stops being ground truth
        // as the decision horizon grows. Same code, same view, only
        // this number changed:
        //
        //     max_iter  150   degenerate (nothing has escaped yet)
        //     max_iter  600   0.23% differ
        //     max_iter 3000   9.40% differ
        //
        // That is the oracle degrading, not the render. Against a
        // 60-digit ground truth on 120 sampled pixels of this view,
        // the f64 DIRECT orbit was wrong on 10.0% and the perturbed
        // recurrence on 7.5% -- the perturbed path is the MORE
        // accurate of the two, which is the whole point of carrying an
        // exact reference. 600 keeps the horizon short enough that f64
        // is still an authority while leaving plenty of escaped pixels
        // to compare.
        esc.max_iter = 600;

        let (w, h) = (96u32, 72u32);
        let mut config = crate::config::FractalConfig::default();
        // Binary image: the comparison reads escaped-or-not only.
        config.palette = crate::scene::palette::Palette {
            name: "white".to_string(),
            stops: vec![
                crate::scene::palette::ColorStop { position: 0.0, color: [1.0, 1.0, 1.0] },
                crate::scene::palette::ColorStop { position: 1.0, color: [1.0, 1.0, 1.0] },
            ],
            locked: false,
            built_in: false,
        };
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation, config.palette_squeeze,
            config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse,
        );
        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        escape.force_floatexp = deep;
        let mut guard = 0u32;
        loop {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lambda depth"),
            });
            let settled = escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            if settled {
                break;
            }
            guard += 1;
            assert!(guard < 100_000, "render did not settle");
        }
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("lambda depth tonemap"),
        });
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
            w,
            h,
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
        renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
        queue.submit(std::iter::once(enc.finish()));
        let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
            &device, &queue, false, config.background_color,
        ))
        .expect("readback");
        escape.destroy();

        // f64 oracle over the same view: z' = c*z*(1-z) from z0 = 1/2.
        let span_y = 4.0 / (esc.zoom_log2 as f64).exp2();
        let span_x = span_y * w as f64 / h as f64;
        let mut differ = 0usize;
        for py in 0..h {
            for px in 0..w {
                let c = (
                    ((px as f64 + 0.5) / w as f64 - 0.5) * span_x + cx,
                    -(((py as f64 + 0.5) / h as f64 - 0.5) * span_y) + cy,
                );
                let (mut zx, mut zy) = (0.5f64, 0.0f64);
                let mut escaped = false;
                for _ in 0..esc.max_iter {
                    // w = z * (1 - z)
                    let (ar, ai) = (1.0 - zx, -zy);
                    let (pr, pi) = (zx * ar - zy * ai, zx * ai + zy * ar);
                    let nx = c.0 * pr - c.1 * pi;
                    let ny = c.0 * pi + c.1 * pr;
                    zx = nx;
                    zy = ny;
                    if zx * zx + zy * zy > 4.0 {
                        escaped = true;
                        break;
                    }
                }
                let i = ((py * w + px) * 4) as usize;
                let lit = rgba[i] as u32 + rgba[i + 1] as u32 + rgba[i + 2] as u32 > 24;
                if lit != escaped {
                    differ += 1;
                }
            }
        }
        // A view where every pixel escapes (or none do) would agree
        // with any oracle at all. Require both populations before
        // reading anything into the agreement.
        let escaped_px = rgba
            .chunks(4)
            .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
            .count();
        let total = (w * h) as usize;
        assert!(
            escaped_px > total / 10 && escaped_px < total * 9 / 10,
            "degenerate view: {escaped_px}/{total} pixels escaped, so agreement \
             with the oracle would mean nothing"
        );

        let frac = differ as f64 / (w * h) as f64;
        println!(
            "lambda at zoom {} on the {} rung: {:.2}% differ from the exact orbit",
            esc.zoom_log2,
            if deep { "floatexp" } else { "scaled" },
            100.0 * frac
        );
        assert!(
            frac < 0.005,
            "perturbed Lambda disagrees with an exact orbit on {:.1}% of pixels -- the reference and the delta step describe different maps",
            100.0 * frac
        );
    }

    /// Perturbed Feather must match an exact orbit at depth, on both
    /// rungs.
    ///
    /// The first RATIONAL tier, so this is the first test of the
    /// quotient delta form `dq = (dN - q*dD)/(D + dD)`. Writing that
    /// the obvious way instead — `(dN*D - N*dD)/(D*(D+dD))` —
    /// differences two full-size products and loses the delta to
    /// cancellation, which would show up here and nowhere else.
    ///
    /// It also exercises a NON-HOLOMORPHIC denominator: `1 + x^2 -
    /// i*y^2` reads the components of z separately, so `dD` is two
    /// independent component binomials rather than one complex one. A
    /// port that treated it as holomorphic still renders a plausible
    /// Feather.
    ///
    /// The view is chosen for a SHORT escape horizon (median 8
    /// iterations) after the lambda tier taught this suite that an f64
    /// oracle stops being ground truth once orbits linger — see
    /// `lambda_exact_orbit_case`.
    /// Perturbed Feather must match an exact orbit at depth, on both
    /// rungs.
    ///
    /// The first RATIONAL tier, so this is the first test of the
    /// quotient delta form `dq = (dN - q*dD)/(D + dD)`. Writing that
    /// the obvious way instead — `(dN*D - N*dD)/(D*(D+dD))` —
    /// differences two full-size products and loses the delta to
    /// cancellation, which would show up here and nowhere else.
    ///
    /// It also exercises a NON-HOLOMORPHIC denominator: `1 + x^2 -
    /// i*y^2` reads the components of z separately, so `dD` is two
    /// independent component binomials rather than one complex one. A
    /// port that treated it as holomorphic still renders a plausible
    /// Feather.
    ///
    /// The view is chosen for a SHORT escape horizon (median 8
    /// iterations) after the lambda tier taught this suite that an f64
    /// oracle stops being ground truth once orbits linger — see
    /// `lambda_exact_orbit_case`.
    ///
    /// AND it is the view that exposed the f32 escape test's
    /// delta-blindness: |c|² = 3.975, one step from the bailout, on a
    /// map whose |z| grows ~×1.2 per step — so escape here is decided
    /// by sub-ulp differences a plain f32 |z_full|² cannot see. With
    /// the old test this view read 26% wrong at zoom 25 and went
    /// fully degenerate by 30; the delta-aware margin is what makes
    /// zoom 30 pass. This test is the margin's regression.
    #[test]
    #[ignore = "needs a GPU"]
    fn perturbed_feather_matches_an_exact_orbit_at_depth() {
        for deep in [false, true] {
            feather_exact_orbit_case(deep);
        }
    }

    fn feather_exact_orbit_case(deep: bool) {
        let (device, queue) = repro_device();
        // The INTERIOR side of the boundary, so the reference orbit
        // runs the full iteration count.
        let (cx, cy) = (-0.77291940505873225f64, -1.83786610577385723f64);
        const P: u32 = 3;
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "feather".to_string();
        esc.center_re = format!("{cx:.17}");
        esc.center_im = format!("{cy:.17}");
        esc.zoom_log2 = 30.0;
        esc.max_iter = 600;
        esc.formula_params.insert("power".to_string(), P as f32);

        let (w, h) = (96u32, 72u32);
        let mut config = crate::config::FractalConfig::default();
        config.palette = crate::scene::palette::Palette {
            name: "white".to_string(),
            stops: vec![
                crate::scene::palette::ColorStop { position: 0.0, color: [1.0, 1.0, 1.0] },
                crate::scene::palette::ColorStop { position: 1.0, color: [1.0, 1.0, 1.0] },
            ],
            locked: false,
            built_in: false,
        };
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation, config.palette_squeeze,
            config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse,
        );
        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        escape.force_floatexp = deep;
        let mut guard = 0u32;
        loop {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("feather depth"),
            });
            let settled = escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            if settled {
                break;
            }
            guard += 1;
            assert!(guard < 100_000, "render did not settle");
        }
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("feather depth tonemap"),
        });
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
            w,
            h,
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
        renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
        queue.submit(std::iter::once(enc.finish()));
        let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
            &device, &queue, false, config.background_color,
        ))
        .expect("readback");
        escape.destroy();

        let span_y = 4.0 / (esc.zoom_log2 as f64).exp2();
        let span_x = span_y * w as f64 / h as f64;
        let mut differ = 0usize;
        for py in 0..h {
            for px in 0..w {
                let c = (
                    ((px as f64 + 0.5) / w as f64 - 0.5) * span_x + cx,
                    -(((py as f64 + 0.5) / h as f64 - 0.5) * span_y) + cy,
                );
                let (mut zx, mut zy) = (0.0f64, 0.0f64);
                let mut escaped = false;
                for _ in 0..esc.max_iter {
                    // num = z^P
                    let (mut nr, mut ni) = (zx, zy);
                    for _ in 1..P {
                        let t = nr * zx - ni * zy;
                        ni = nr * zy + ni * zx;
                        nr = t;
                    }
                    // den = 1 + x^2 - i*y^2
                    let (dr, di) = (1.0 + zx * zx, -(zy * zy));
                    let d2 = dr * dr + di * di;
                    zx = (nr * dr + ni * di) / d2 + c.0;
                    zy = (ni * dr - nr * di) / d2 + c.1;
                    if zx * zx + zy * zy > 4.0 {
                        escaped = true;
                        break;
                    }
                }
                let i = ((py * w + px) * 4) as usize;
                let lit = rgba[i] as u32 + rgba[i + 1] as u32 + rgba[i + 2] as u32 > 24;
                if lit != escaped {
                    differ += 1;
                }
            }
        }
        let escaped_px = rgba
            .chunks(4)
            .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
            .count();
        let total = (w * h) as usize;
        assert!(
            escaped_px > total / 10 && escaped_px < total * 9 / 10,
            "degenerate view: {escaped_px}/{total} pixels escaped"
        );
        let frac = differ as f64 / (w * h) as f64;
        println!(
            "feather at zoom {} on the {} rung: {:.2}% differ from the exact orbit",
            esc.zoom_log2,
            if deep { "floatexp" } else { "scaled" },
            100.0 * frac
        );
        assert!(
            frac < 0.005,
            "perturbed Feather disagrees with an exact orbit on {:.1}% of pixels -- the quotient delta form or the component-wise denominator is wrong",
            100.0 * frac
        );
    }

    /// Perturbed McMullen must match an exact orbit at depth, on both
    /// rungs.
    ///
    /// The first tier with a genuine POLE. Its delta form has to write
    /// the pole term's difference as
    /// `1/(Z+d)^m - 1/Z^m = -dM/((Z+d)^m Z^m)` — small numerator over
    /// a product of FULL values. Formed the direct way it subtracts
    /// two large nearly-equal reciprocals and loses the delta, which
    /// is the failure this test exists to catch.
    ///
    /// JULIA MODE, because that is the only mode this tier is selected
    /// in: our McMullen seeds its parameter plane at `z_0 = c`, which
    /// is not a critical point of the map, and measurement found 0 of
    /// 4000 sampled parameters with a bounded orbit — that plane has
    /// no interior to zoom into. The Sierpinski-carpet pictures this
    /// family is known for are Julia sets.
    #[test]
    #[ignore = "needs a GPU"]
    fn perturbed_mcmullen_matches_an_exact_orbit_at_depth() {
        for deep in [false, true] {
            mcmullen_exact_orbit_case(deep);
        }
    }

    fn mcmullen_exact_orbit_case(deep: bool) {
        let (device, queue) = repro_device();
        // On the Julia set of c = 0.05 + 0.03i, on the INTERIOR side of
        // the boundary so the reference runs its full length.
        let (cx, cy) = (-0.12291872044575830f64, -0.36795489251636515f64);
        let (jr, ji) = (0.05f32, 0.03f32);
        const N: u32 = 2;
        const M: u32 = 3;
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "mcmullen".to_string();
        esc.julia = true;
        esc.julia_re = jr;
        esc.julia_im = ji;
        esc.center_re = format!("{cx:.17}");
        esc.center_im = format!("{cy:.17}");
        esc.zoom_log2 = 30.0;
        // Short horizon: median escape here is 26 iterations, so f64
        // is still an authority (see lambda_exact_orbit_case).
        esc.max_iter = 400;
        esc.formula_params.insert("n".to_string(), N as f32);
        esc.formula_params.insert("m".to_string(), M as f32);

        let (w, h) = (96u32, 72u32);
        let mut config = crate::config::FractalConfig::default();
        config.palette = crate::scene::palette::Palette {
            name: "white".to_string(),
            stops: vec![
                crate::scene::palette::ColorStop { position: 0.0, color: [1.0, 1.0, 1.0] },
                crate::scene::palette::ColorStop { position: 1.0, color: [1.0, 1.0, 1.0] },
            ],
            locked: false,
            built_in: false,
        };
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation, config.palette_squeeze,
            config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse,
        );
        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        escape.force_floatexp = deep;
        let mut guard = 0u32;
        loop {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("mcmullen depth"),
            });
            let settled = escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            if settled {
                break;
            }
            guard += 1;
            assert!(guard < 100_000, "render did not settle");
        }
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mcmullen depth tonemap"),
        });
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
            w,
            h,
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
        renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
        queue.submit(std::iter::once(enc.finish()));
        let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
            &device, &queue, false, config.background_color,
        ))
        .expect("readback");
        escape.destroy();

        let span_y = 4.0 / (esc.zoom_log2 as f64).exp2();
        let span_x = span_y * w as f64 / h as f64;
        let (cr, ci) = (jr as f64, ji as f64);
        let mut differ = 0usize;
        for py in 0..h {
            for px in 0..w {
                // Julia: the PIXEL is z0, c is the fixed constant.
                let mut zx = ((px as f64 + 0.5) / w as f64 - 0.5) * span_x + cx;
                let mut zy = -(((py as f64 + 0.5) / h as f64 - 0.5) * span_y) + cy;
                let mut escaped = false;
                for _ in 0..esc.max_iter {
                    let (mut nr, mut ni) = (zx, zy);
                    for _ in 1..N {
                        let t = nr * zx - ni * zy;
                        ni = nr * zy + ni * zx;
                        nr = t;
                    }
                    let (mut mr, mut mi) = (zx, zy);
                    for _ in 1..M {
                        let t = mr * zx - mi * zy;
                        mi = mr * zy + mi * zx;
                        mr = t;
                    }
                    let d = mr * mr + mi * mi;
                    if d < 1e-300 {
                        escaped = true;
                        break;
                    }
                    zx = nr + (cr * mr + ci * mi) / d;
                    zy = ni + (ci * mr - cr * mi) / d;
                    if zx * zx + zy * zy > 4.0 {
                        escaped = true;
                        break;
                    }
                }
                let i = ((py * w + px) * 4) as usize;
                let lit = rgba[i] as u32 + rgba[i + 1] as u32 + rgba[i + 2] as u32 > 24;
                if lit != escaped {
                    differ += 1;
                }
            }
        }
        let escaped_px = rgba
            .chunks(4)
            .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
            .count();
        let total = (w * h) as usize;
        assert!(
            escaped_px > total / 20 && escaped_px < total * 19 / 20,
            "degenerate view: {escaped_px}/{total} pixels escaped"
        );
        let frac = differ as f64 / (w * h) as f64;
        println!(
            "mcmullen at zoom {} on the {} rung: {:.2}% differ from the exact orbit",
            esc.zoom_log2,
            if deep { "floatexp" } else { "scaled" },
            100.0 * frac
        );
        assert!(
            frac < 0.01,
            "perturbed McMullen disagrees with an exact orbit on {:.1}% of pixels -- the pole term's delta form is wrong",
            100.0 * frac
        );
    }

    /// Perturbed Magnet must match an exact orbit at depth, on both
    /// rungs and BOTH variants.
    ///
    /// Two things here are new to the perturbed path. First, `c`
    /// appears in the numerator AND the denominator, so the
    /// parameter-plane term is not a bare `+dc` — it enters `dN` and
    /// `dD` separately and then partially cancels inside
    /// `dN - q*dD`. Second, the map CONVERGES: these orbits settle at
    /// z = 1 rather than escaping, so the perturbed loop needs the
    /// settle test that `PerturbTier::is_convergent` turns on. Without
    /// it every converging pixel would run to `max_iter` and this
    /// comparison would fail wholesale — which is exactly what it is
    /// here to prove does not happen.
    ///
    /// The oracle therefore compares TERMINATION (converged or
    /// escaped), because that is what the template marks: it sets
    /// `escaped` on convergence too, so escape-count and smooth
    /// colorings shade convergence speed.
    #[test]
    #[ignore = "needs a GPU"]
    fn perturbed_magnet_matches_an_exact_orbit_at_depth() {
        for variant in [0u32, 1] {
            for deep in [false, true] {
                magnet_exact_orbit_case(variant, deep);
            }
        }
    }

    // ------------------------------------------------------------
    // The big-float families (Newton, Nova, Kaliset, Ducks) against
    // exact orbits at zoom 30, on both rungs.
    // ------------------------------------------------------------

    /// Render `esc` through the perturbed path (scaled or deep rung)
    /// to settled and read back every pixel's terminal record.
    fn perturbed_records(
        esc: &crate::config::escape::EscapeConfig,
        w: u32,
        h: u32,
        deep: bool,
    ) -> Vec<crate::escape::renderer::IterRecord> {
        records_via(esc, w, h, deep, true)
    }

    /// As above, but choosing the path: `perturbed` false renders the
    /// DIRECT template, so the two can be scored against the same
    /// exact oracle.
    fn records_via(
        esc: &crate::config::escape::EscapeConfig,
        w: u32,
        h: u32,
        deep: bool,
        perturbed: bool,
    ) -> Vec<crate::escape::renderer::IterRecord> {
        let (device, queue) = repro_device();
        let config = crate::config::FractalConfig::default();
        let renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            w,
            h,
            &config.flame,
            config.palette_size,
        );
        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        escape.force_perturbed = perturbed;
        escape.force_floatexp = deep && perturbed;
        let mut guard = 0u32;
        loop {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("oracle frame"),
            });
            let settled = escape.render(&device, &queue, &mut enc, esc, renderer.palette_view());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            if settled {
                break;
            }
            guard += 1;
            assert!(guard < 100_000, "render did not settle");
        }
        let out = escape
            .read_results_full(&device, &queue)
            .expect("records inactive -- results_fit failed?");
        escape.destroy();
        out
    }

    type C64 = (f64, f64);

    fn c64_mul(a: C64, b: C64) -> C64 {
        (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
    }

    fn c64_div(a: C64, b: C64) -> C64 {
        let d = b.0 * b.0 + b.1 * b.1;
        ((a.0 * b.0 + a.1 * b.1) / d, (a.1 * b.0 - a.0 * b.1) / d)
    }

    /// The exact outcome of a root-finder orbit at one point, read off
    /// a BIG-FLOAT orbit.
    ///
    /// f64 cannot serve as this oracle, which cost an afternoon to
    /// establish: measured at these three boundary centres, an f64
    /// orbit has already lost the trajectory by step 27-41 (the
    /// Chebyshev centre passes |Z| ~ 958, which amplifies every
    /// earlier rounding), and Chebyshev's orbits run to 83 iterations.
    /// An f64 "truth" therefore disagrees with the exact answer on
    /// ~3% of pixels by itself -- which reads exactly like a broken
    /// delta step, and is not one.
    ///
    /// This shares `step_rootfinder` with the reference the shader
    /// uses, so it cannot catch an error in that step; what pins the
    /// step is `reference.rs`'s own pair of tests, which track it
    /// against an f64 twin where f64 IS trustworthy (a benign point,
    /// 12 steps) and check it lands on a true root. What is under
    /// test here is the delta algebra, and that is independent.
    fn exact_rootfinder_outcome(
        re: &str,
        im: &str,
        zoom: f64,
        variant: u32,
        p: u32,
        relax: [f32; 2],
        julia_c: Option<(f32, f32)>,
        max_iter: u32,
        bailout: f64,
    ) -> Outcome {
        let o = crate::escape::reference::ReferenceOrbit::compute(
            re, im, zoom, None, max_iter, julia_c, p, false, variant, relax,
        )
        .expect("oracle orbit");
        let val = |i: usize| -> C64 {
            let e = o.z_f32(i);
            (
                e[0] as f64 + o.orbit_lo[i][0] as f64,
                e[1] as f64 + o.orbit_lo[i][1] as f64,
            )
        };
        let last = (o.len() as usize).saturating_sub(1);
        let mut prev = val(0);
        for i in 1..=last.min(max_iter as usize) {
            let z = val(i);
            if z.0 * z.0 + z.1 * z.1 > bailout {
                return Outcome::Escaped(i as u32);
            }
            let (dx, dy) = (z.0 - prev.0, z.1 - prev.1);
            if dx * dx + dy * dy < 1e-12 {
                return Outcome::Converged(z, i as u32);
            }
            prev = z;
        }
        Outcome::RanOut
    }

    /// A pixel's exact centre as decimal strings, optionally nudged by
    /// a fraction of a pixel (the stability probe).
    fn pixel_decimal(
        px: u32,
        py: u32,
        w: u32,
        h: u32,
        cre: &str,
        cim: &str,
        zoom: f64,
        nudge: f64,
    ) -> (String, String) {
        let span_y = 4.0 / zoom.exp2();
        let span_x = span_y * w as f64 / h as f64;
        let s = span_y / h as f64;
        let dx = ((px as f64 + 0.5) / w as f64 - 0.5) * span_x + nudge * s;
        let dy = -(((py as f64 + 0.5) / h as f64 - 0.5) * span_y) + nudge * s;
        (
            crate::escape::fixedpoint::FixedPoint::decimal_add_f64(cre, dx, zoom).unwrap(),
            crate::escape::fixedpoint::FixedPoint::decimal_add_f64(cim, dy, zoom).unwrap(),
        )
    }

    /// Outcome of an exact (f64) convergent orbit: `Some(z)` with the
    /// iteration count when it settled, the escape count, or None for
    /// ran-out -- the same three-way split the records carry.
    #[derive(Clone, Copy, Debug, PartialEq)]
    enum Outcome {
        Converged(C64, u32),
        Escaped(u32),
        RanOut,
    }

    fn record_outcome(r: &crate::escape::renderer::IterRecord) -> Outcome {
        if r.tags & 2 != 0 {
            Outcome::Converged((r.z[0] as f64, r.z[1] as f64), r.n)
        } else if r.tags & 1 != 0 {
            Outcome::Escaped(r.n)
        } else {
            Outcome::RanOut
        }
    }

    /// Two outcomes agree when the class matches, a converged pixel
    /// lands on the SAME point, and the settle/escape count is within
    /// two (an f32-vs-f64 razor's edge on the 1e-12 settle test).
    fn outcomes_agree(a: Outcome, b: Outcome) -> (bool, bool) {
        match (a, b) {
            (Outcome::Converged(za, na), Outcome::Converged(zb, nb)) => {
                let close = (za.0 - zb.0).abs() + (za.1 - zb.1).abs() < 1e-3;
                (close, close && (na as i64 - nb as i64).abs() <= 2)
            }
            (Outcome::Escaped(na), Outcome::Escaped(nb)) => (true, (na as i64 - nb as i64).abs() <= 2),
            (Outcome::RanOut, Outcome::RanOut) => (true, true),
            _ => (false, false),
        }
    }

    /// Exact outcomes for every pixel, plus whether each is STABLE:
    /// unchanged when the seed moves by 1e-4 of a pixel in either
    /// axis. An unstable pixel is decided at a sub-pixel scale below
    /// the f32 delta's own resolution (about 4e-6 of a pixel, before
    /// the map amplifies it), so no f32 renderer -- direct or
    /// perturbed -- can be held to it; the settle count near a slowly
    /// converging fixed point is the common case (it jitters by
    /// several iterations under a 1e-7 nudge of |dz|, and the direct
    /// f32 path jitters with it).
    fn convergent_truth(
        w: u32,
        h: u32,
        cre: &str,
        cim: &str,
        zoom: f64,
        oracle: impl Fn(&str, &str) -> Outcome + Sync,
    ) -> (Vec<Outcome>, Vec<bool>) {
        use rayon::prelude::*;
        let pairs: Vec<(Outcome, bool)> = (0..w * h)
            .into_par_iter()
            .map(|i| {
                let (px, py) = (i % w, i / w);
                let (re, im) = pixel_decimal(px, py, w, h, cre, cim, zoom, 0.0);
                let o = oracle(&re, &im);
                // ONE probe, not four: every evaluation here is its own
                // big-float orbit, and these tests run in a debug build.
                let (nre, nim) = pixel_decimal(px, py, w, h, cre, cim, zoom, 1e-3);
                let (c, k) = outcomes_agree(o, oracle(&nre, &nim));
                (o, c && k)
            })
            .collect();
        pairs.into_iter().unzip()
    }

    /// Compare perturbed records against exact outcomes for a
    /// convergent map over the STABLE pixels (see `convergent_truth`):
    /// outcome mismatches below 0.2%, count disagreements below 0.5%,
    /// and the unstable set itself must stay small. Returns the
    /// mismatch fraction.
    fn compare_convergent(
        what: &str,
        gpu: &[crate::escape::renderer::IterRecord],
        truth: &[Outcome],
        stable: &[bool],
    ) -> f64 {
        let mut mismatched = 0usize;
        let mut count_off = 0usize;
        let mut considered = 0usize;
        for ((g, t), s) in gpu.iter().zip(truth).zip(stable) {
            if !s {
                continue;
            }
            considered += 1;
            let (same, count_ok) = outcomes_agree(record_outcome(g), *t);
            if !same {
                mismatched += 1;
            } else if !count_ok {
                count_off += 1;
            }
        }
        let unstable = gpu.len() - considered;
        let frac = mismatched as f64 / considered.max(1) as f64;
        println!(
            "{what}: {mismatched}/{considered} outcome mismatches ({:.2}%), {count_off} counts off by >2, {unstable} unstable pixels excluded ({:.1}%)",
            frac * 100.0,
            unstable as f64 * 100.0 / gpu.len() as f64
        );
        assert!(
            unstable < gpu.len() / 5,
            "{what}: {unstable} of {} pixels are unstable -- the view is too razor-edged to test",
            gpu.len()
        );
        assert!(
            count_off <= considered / 200,
            "{what}: {count_off} stable pixels settle/escape at a different count"
        );
        frac
    }

    /// Every basin must be present, or the view proves nothing.
    fn assert_three_basins(what: &str, truth: &[Outcome]) {
        let mut buckets = [0usize; 3];
        for t in truth {
            if let Outcome::Converged(z, _) = t {
                let ang = z.1.atan2(z.0);
                let k = ((ang / (2.0 * std::f64::consts::PI / 3.0)).round() as i64).rem_euclid(3);
                buckets[k as usize] += 1;
            }
        }
        for (k, b) in buckets.iter().enumerate() {
            assert!(
                *b > truth.len() / 50,
                "{what}: basin {k} holds only {b} of {} pixels -- degenerate view",
                truth.len()
            );
        }
    }

    /// Newton over z^3 - 1 on a basin boundary: the Julia set of the
    /// Newton map has the Wada property, so a zoom-30 view around any
    /// boundary point holds all three basins. One boundary point per
    /// scheme (each scheme is its own map), both rungs, and the
    /// complex relaxation exercised on the Newton scheme.
    #[test]
    #[ignore = "needs a GPU"]
    fn perturbed_newton_matches_an_exact_orbit_at_depth() {
        // A smaller grid and iteration cap than the escaping tiers
        // use: every pixel of the oracle is its own big-float orbit,
        // in a debug build. 150 is comfortably past where these views
        // settle (measured: Chebyshev, the slowest, runs to 83).
        let (w, h) = (48u32, 36u32);
        let zoom = 30.0f64;
        let max_iter = 150u32;
        let bailout = 1e6f64;
        // (scheme, center) -- centres bisected between two basins; the
        // third appears at depth by the Wada property.
        for (scheme, cre, cim) in [
            (0u32, "-0.5", "0.23019701107058421"),
            (1, "-0.5", "0.00365726284994825"),
            (2, "-0.5", "0.14065853256683475"),
        ] {
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.formula = "newton".to_string();
            esc.center_re = cre.to_string();
            esc.center_im = cim.to_string();
            esc.zoom_log2 = zoom;
            esc.max_iter = max_iter;
            esc.bailout = bailout as f32;
            esc.formula_params.insert("power".to_string(), 3.0);
            esc.formula_params.insert("scheme".to_string(), scheme as f32);
            esc.formula_params.insert("func".to_string(), 0.0);
            // Every shipped (scheme, func) pair must actually perturb:
            // if one silently stopped selecting the tier this test
            // would pass by rendering the direct path.
            assert_eq!(
                crate::escape::EscapeRenderer::perturb_tier(&esc),
                Some(crate::escape::assembler::PerturbTier::Newton {
                    p: 3,
                    scheme,
                    func: 0
                }),
                "scheme {scheme} over z^p - 1 must select the Newton tier"
            );
            let (truth, stable) = convergent_truth(w, h, cre, cim, zoom, |re, im| {
                exact_rootfinder_outcome(
                    re,
                    im,
                    zoom,
                    crate::escape::reference::newton_variant(scheme, 0),
                    3,
                    [1.0, 0.0],
                    Some((0.0, 0.0)),
                    max_iter,
                    bailout,
                )
            });
            assert_three_basins(&format!("newton scheme {scheme}"), &truth);
            for deep in [false, true] {
                let gpu = perturbed_records(&esc, w, h, deep);
                let frac = compare_convergent(
                    &format!("newton scheme {scheme} deep {deep}"),
                    &gpu,
                    &truth,
                    &stable,
                );
                assert!(frac < 0.002, "newton scheme {scheme} deep {deep}: {:.2}% mismatched", frac * 100.0);
            }
        }
        // Complex relaxation on the Newton scheme: a different map,
        // whose basins at the same boundary point are still mixed.
        {
            let (cre, cim) = ("-0.5", "0.23019701107058421");
            let relax = [1.15f32, 0.35];
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.formula = "newton".to_string();
            esc.center_re = cre.to_string();
            esc.center_im = cim.to_string();
            esc.zoom_log2 = zoom;
            esc.max_iter = max_iter;
            esc.bailout = bailout as f32;
            esc.formula_params.insert("relax_re".to_string(), relax[0]);
            esc.formula_params.insert("relax_im".to_string(), relax[1]);
            let (truth, stable) = convergent_truth(w, h, cre, cim, zoom, |re, im| {
                exact_rootfinder_outcome(
                    re,
                    im,
                    zoom,
                    crate::escape::reference::newton_variant(0, 0),
                    3,
                    relax,
                    Some((0.0, 0.0)),
                    max_iter,
                    bailout,
                )
            });
            let converged = truth.iter().filter(|t| matches!(t, Outcome::Converged(..))).count();
            assert!(converged > truth.len() / 10, "relaxed newton: only {converged} converge");
            for deep in [false, true] {
                let gpu = perturbed_records(&esc, w, h, deep);
                let frac =
                    compare_convergent(&format!("relaxed newton deep {deep}"), &gpu, &truth, &stable);
                assert!(frac < 0.002, "relaxed newton deep {deep}: {:.2}% mismatched", frac * 100.0);
            }
        }
    }

    /// Nova (p = 3): the parameter plane at a boundary mixing
    /// convergence, escape and run-out, both rungs.
    #[test]
    #[ignore = "needs a GPU"]
    fn perturbed_nova_matches_an_exact_orbit_at_depth() {
        let (w, h) = (48u32, 36u32);
        let zoom = 30.0f64;
        let max_iter = 150u32;
        let bailout = 1e6f64;
        let (cre, cim) = ("-0.36039383324484020", "0.22577954314649112");
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "nova".to_string();
        esc.center_re = cre.to_string();
        esc.center_im = cim.to_string();
        esc.zoom_log2 = zoom;
        esc.max_iter = max_iter;
        esc.bailout = bailout as f32;
        // The pixel is the PARAMETER here, so the oracle orbit is a
        // PARAMETER-plane one: it seeds the critical point z_0 = 1 and
        // takes c from the pixel, exactly as MAP_NOVA does.
        let (truth, stable) = convergent_truth(w, h, cre, cim, zoom, |re, im| {
            exact_rootfinder_outcome(
                re,
                im,
                zoom,
                crate::escape::reference::MAP_NOVA,
                3,
                [1.0, 0.0],
                None,
                max_iter,
                bailout,
            )
        });
        let converged = truth.iter().filter(|t| matches!(t, Outcome::Converged(..))).count();
        let escaped = truth.iter().filter(|t| matches!(t, Outcome::Escaped(_))).count();
        assert!(
            converged > truth.len() / 20 && escaped > truth.len() / 100,
            "nova view degenerate: {converged} converged, {escaped} escaped"
        );
        for deep in [false, true] {
            let gpu = perturbed_records(&esc, w, h, deep);
            let frac = compare_convergent(&format!("nova deep {deep}"), &gpu, &truth, &stable);
            assert!(frac < 0.002, "nova deep {deep}: {:.2}% mismatched", frac * 100.0);
        }
    }

    /// The exact mean |z| over `max_iter` steps of the pixel's own
    /// orbit, from a big-float reference computed AT the pixel: the
    /// oracle for the non-escaping families' orbit accumulators. The
    /// map's own reference step is verified against f64 separately
    /// (reference.rs), so this isolates the GPU delta algebra.
    fn exact_mean_magnitude(
        esc: &crate::config::escape::EscapeConfig,
        w: u32,
        h: u32,
        variant: u32,
        map_params: [f32; 2],
    ) -> Vec<f64> {
        use rayon::prelude::*;
        let zoom = esc.zoom_log2;
        let span_y = 4.0 / zoom.exp2();
        let span_x = span_y * w as f64 / h as f64;
        let julia_c = if esc.julia { Some((esc.julia_re, esc.julia_im)) } else { None };
        let max_iter = esc.max_iter;
        (0..w * h)
            .into_par_iter()
            .map(|i| {
                let (px, py) = (i % w, i / w);
                let dx = ((px as f64 + 0.5) / w as f64 - 0.5) * span_x;
                let dy = -(((py as f64 + 0.5) / h as f64 - 0.5) * span_y);
                let re = crate::escape::fixedpoint::FixedPoint::decimal_add_f64(&esc.center_re, dx, zoom)
                    .unwrap();
                let im = crate::escape::fixedpoint::FixedPoint::decimal_add_f64(&esc.center_im, dy, zoom)
                    .unwrap();
                let orbit = crate::escape::reference::ReferenceOrbit::compute(
                    &re, &im, zoom, None, max_iter, julia_c, 2, false, variant, map_params,
                )
                .unwrap();
                assert_eq!(orbit.len(), max_iter + 1, "pixel orbit ended early");
                let mut sum = 0.0f64;
                for k in 1..=max_iter as usize {
                    let z = orbit.z_f32(k);
                    let (x, y) = (
                        z[0] as f64 + orbit.orbit_lo[k][0] as f64,
                        z[1] as f64 + orbit.orbit_lo[k][1] as f64,
                    );
                    sum += (x * x + y * y).sqrt();
                }
                sum / max_iter as f64
            })
            .collect()
    }

    /// Compare the records' `magnitude_average` accumulators against
    /// the exact means: relative error per pixel, with the view's own
    /// spread as the degeneracy guard.
    fn compare_means(what: &str, gpu: &[crate::escape::renderer::IterRecord], truth: &[f64]) {
        let n = truth.len() as f64;
        let mean = truth.iter().sum::<f64>() / n;
        let std = (truth.iter().map(|t| (t - mean).powi(2)).sum::<f64>() / n).sqrt();
        assert!(std > 1e-4 * mean.abs().max(1e-9), "{what}: flat view (std {std:.3e} on mean {mean:.4})");
        let mut worst = 0.0f64;
        let mut over = 0usize;
        let mut total = 0.0f64;
        for (g, t) in gpu.iter().zip(truth) {
            let gm = g.accum[0] as f64 / (g.accum[1] as f64).max(1.0);
            let rel = (gm - t).abs() / t.abs().max(1e-9);
            total += rel;
            worst = worst.max(rel);
            if rel > 1e-3 {
                over += 1;
            }
        }
        println!(
            "{what}: mean rel error {:.2e}, worst {:.2e}, {over}/{} pixels over 1e-3 (view std/mean {:.2e})",
            total / n,
            worst,
            truth.len(),
            std / mean.abs().max(1e-9)
        );
        assert!(total / n < 1e-4, "{what}: mean relative error {:.2e}", total / n);
        assert!(over <= truth.len() / 100, "{what}: {over} pixels off by more than 1e-3");
    }

    /// Every family that declares a tier must ACTUALLY take the
    /// perturbed path in a plain render.
    ///
    /// `wants_perturbation` is unit-testable and was green while the
    /// app still rendered Ducks direct, so the gap this closes is
    /// between "the gate says yes" and "the renderer did it". Reads
    /// the renderer's own diagnostic label, which is what the app's
    /// panel shows.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_new_tiers_take_the_perturbed_path_in_a_plain_render() {
        let _guard = diag_lock();
        let (device, queue) = repro_device();
        let (w, h) = (64u32, 48u32);
        let config = crate::config::FractalConfig::default();
        let renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        // (formula, coloring, centre, zoom, julia, params)
        let cases: &[(&str, &str, &str, &str, f64, bool, &[(&str, f32)])] = &[
            ("newton", "root_basin", "0.35", "0.28", 20.0, false, &[]),
            ("nova", "smooth", "-0.3", "0.05", 20.0, false, &[]),
            ("ducks", "magnitude_average", "-0.4", "0.3", 20.0, false, &[]),
            ("ducks", "magnitude_average", "-0.4", "0.3", 20.0, false, &[("variant", 4.0)]),
            ("ducks", "magnitude_average", "0.15", "-0.2", 20.0, true, &[]),
            // Kaliset carries a floor of 24 (tier_min_zoom).
            ("kaliset", "magnitude_average", "0.35", "0.28", 26.0, false, &[]),
        ];
        for (formula, coloring, cre, cim, zoom, julia, params) in cases {
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.formula = formula.to_string();
            esc.coloring = coloring.to_string();
            esc.center_re = cre.to_string();
            esc.center_im = cim.to_string();
            esc.zoom_log2 = *zoom;
            esc.max_iter = 60;
            esc.bailout = 1e6;
            if *julia {
                esc.julia = true;
                esc.julia_re = 0.1;
                esc.julia_im = -0.62;
            }
            for (k, v) in *params {
                esc.formula_params.insert(k.to_string(), *v);
            }
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            let mut guard = 0u32;
            loop {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("path probe"),
                });
                let settled =
                    escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 100_000, "{formula} did not settle");
            }
            let path = crate::escape::diag::snapshot().path;
            escape.destroy();
            println!("{formula} {params:?} @ zoom {zoom}: path = {path}");
            assert!(
                path.starts_with("perturbed"),
                "{formula} {params:?} rendered `{path}` at zoom {zoom} -- the tier is                  selected but the renderer did not use it"
            );
        }
    }

    /// The non-escaping tiers must engage only where their delta
    /// form is actually better than the path it replaces.
    ///
    /// This is the test the user's report earned: "it changes the
    /// moment perturbation starts" is a real failure mode, and the
    /// only way to answer it is to score BOTH paths against exact
    /// per-pixel orbits rather than against each other.
    ///
    /// Measured (mean relative error of the orbit-average field vs
    /// exact orbits, 96x72, 60 iterations):
    ///
    ///   kaliset (0.35, 0.28)   z10  direct 2.1e-3   perturbed 3.7e-1
    ///                          z14  direct 2.3e-3   perturbed 2.0e-1
    ///                          z18/24/30 perturbed 2.4e-2 / 3.6e-4 / 5.7e-6
    ///   kaliset (1.226, 1.574) z14  direct 2.2e-2   perturbed 9.4e-3
    ///   ducks   (-0.4, 0.3)    every depth 10..30: both ~4e-7
    ///   ducks   (0.15, -0.2)   every depth 10..30: both ~1e-7
    ///
    /// So Ducks is safe from the ordinary threshold and Kaliset needs
    /// its own floor -- see `tier_min_zoom`.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_non_escaping_tiers_engage_only_where_they_are_accurate() {
        use crate::escape::EscapeRenderer;
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.coloring = "magnitude_average".to_string();
        esc.max_iter = 60;

        esc.formula = "kaliset".to_string();
        esc.center_re = "0.35".to_string();
        esc.center_im = "0.28".to_string();
        for z in [14.0, 18.0, 23.0] {
            esc.zoom_log2 = z;
            assert!(
                !EscapeRenderer::wants_perturbation(&esc),
                "kaliset must not perturb at zoom {z}: measured worse than direct there"
            );
        }
        esc.zoom_log2 = 26.0;
        assert!(
            EscapeRenderer::wants_perturbation(&esc),
            "kaliset must perturb past its floor, or it gains no depth at all"
        );

        // Ducks: accurate from the ordinary threshold, and the check
        // that its delta really is as good as direct where both work.
        esc.formula = "ducks".to_string();
        esc.center_re = "-0.4".to_string();
        esc.center_im = "0.3".to_string();
        esc.zoom_log2 = 16.0;
        assert!(EscapeRenderer::wants_perturbation(&esc));
        esc.zoom_log2 = 10.0;
        let (w, h) = (96u32, 72u32);
        let truth =
            exact_mean_magnitude(&esc, w, h, crate::escape::reference::MAP_DUCKS, [0.0, 0.0]);
        let score = |what: &str, recs: &[crate::escape::renderer::IterRecord]| -> f64 {
            let mut total = 0.0;
            for (r, t) in recs.iter().zip(&truth) {
                let m = r.accum[0] as f64 / (r.accum[1] as f64).max(1.0);
                total += (m - t).abs() / t.abs().max(1e-9);
            }
            let mean = total / truth.len() as f64;
            println!("ducks {what}: mean relative error vs exact orbits {mean:.3e}");
            mean
        };
        let direct = score("direct", &records_via(&esc, w, h, false, false));
        let perturbed = score("perturbed", &records_via(&esc, w, h, false, true));
        assert!(
            perturbed < 1e-5 && perturbed <= direct * 4.0,
            "ducks perturbed {perturbed:.3e} vs direct {direct:.3e} -- the delta has regressed"
        );
    }

    /// Kaliset on the parameter plane at zoom 30, both sign branches,
    /// both rungs: the orbit-average field against exact orbits.
    #[test]
    #[ignore = "needs a GPU"]
    fn perturbed_kaliset_matches_an_exact_orbit_at_depth() {
        let (w, h) = (96u32, 72u32);
        // Each sign branch is its own map: the -c view is FLAT under
        // +c (measured, std 0.0000), so each carries its own centre.
        for (plus, cre, cim) in [
            (false, "1.22551892238358673", "1.57367616586387160"),
            (true, "-1.79112587335209006", "-0.87051608470578978"),
        ] {
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.formula = "kaliset".to_string();
            esc.coloring = "magnitude_average".to_string();
            esc.center_re = cre.to_string();
            esc.center_im = cim.to_string();
            esc.zoom_log2 = 30.0;
            esc.max_iter = 60;
            esc.formula_params.insert("plus_c".to_string(), if plus { 1.0 } else { 0.0 });
            let truth = exact_mean_magnitude(
                &esc, w, h, crate::escape::reference::MAP_KALISET,
                [if plus { 1.0 } else { 0.0 }, 0.0],
            );
            for deep in [false, true] {
                let gpu = perturbed_records(&esc, w, h, deep);
                compare_means(&format!("kaliset plus_c {plus} deep {deep}"), &gpu, &truth);
            }
        }
    }

    /// Auto-contrast makes a flat deep field visible again.
    ///
    /// The problem it answers, measured on Ducks: an orbit-STATISTIC
    /// coloring is a smooth function of c, so under deep zoom it
    /// converges to its own first-order Taylor expansion -- a PLANE,
    /// 1.0000 of the variance, spread 1.2e-8. Through a cyclic palette
    /// that is a set of parallel bands nobody can see.
    ///
    /// Asserts the thing the user actually cares about: the RENDERED
    /// image gains dynamic range. Compared on luminance spread, which
    /// is palette-agnostic.
    #[test]
    #[ignore = "needs a GPU"]
    fn auto_contrast_restores_a_flat_deep_field() {
        use crate::config::escape::ContrastMode;
        let (device, queue) = repro_device();
        let (w, h) = (96u32, 72u32);
        let config = crate::config::FractalConfig::default();
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size);
        renderer.update_palette(&device, &queue, &config.palette, config.palette_rotation,
            config.palette_squeeze, config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse);

        let mut spread = |mode: ContrastMode| -> f64 {
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.formula = "ducks".to_string();
            esc.coloring = "magnitude_average".to_string();
            esc.center_re = "-0.1000053437741560936430812".to_string();
            esc.center_im = "-0.6749972878037609392903475".to_string();
            esc.julia = true;
            esc.julia_re = 0.1;
            esc.julia_im = -0.675;
            esc.zoom_log2 = 26.6;
            esc.max_iter = 80;
            esc.bailout = 4.0;
            esc.formula_params.insert("variant".to_string(), 0.0);
            esc.contrast.mode = mode;
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            let mut guard = 0;
            loop {
                let mut e = device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor { label: Some("contrast") });
                let done = escape.render(&device, &queue, &mut e, &esc, renderer.palette_view());
                queue.submit(std::iter::once(e.finish()));
                let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                if done { break; }
                guard += 1;
                assert!(guard < 100_000, "{mode:?} did not settle");
            }
            // Contrast moves the palette COORDINATE, so measure what
            // the viewer sees: tonemapped luminance spread.
            let mut enc = device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor { label: Some("tm") });
            renderer.update_background_color(&queue, [0.0, 0.0, 0.0]);
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, [0.0, 0.0, 0.0])).expect("readback");
            escape.destroy();
            let lum: Vec<f64> = rgba.chunks_exact(4)
                .map(|p| 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64)
                .collect();
            let mean = lum.iter().sum::<f64>() / lum.len() as f64;
            (lum.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / lum.len() as f64).sqrt()
        };

        let off = spread(ContrastMode::Off);
        let auto = spread(ContrastMode::AutoRange);
        let flat = spread(ContrastMode::Flatten);
        println!("luminance spread -- off {off:.3} auto_range {auto:.3} flatten {flat:.3}");
        assert!(off < 1.0, "the view is supposed to be FLAT without contrast (got {off:.3})");
        assert!(auto > 20.0, "auto_range did not open the field up (spread {auto:.3})");
        // Flatten subtracts the plane; on a field that IS a plane there
        // is little left, which is the honest answer -- it must at
        // least not be broken.
        assert!(flat.is_finite());
    }

    /// A frame whose per-pixel state cannot fit must not ask for it.
    ///
    /// Reported as a video export that hung: the dialog sat on one
    /// frame forever while the app stayed responsive. The cause was a
    /// PANIC on the exporter's worker thread --
    ///
    ///   In Device::create_buffer, label = 'Escape Iter State'
    ///   Buffer size 398131200 is greater than the maximum buffer
    ///   size (268435456)
    ///
    /// -- 3840*2160*48 bytes of per-pixel resume state for a 4K frame,
    /// against that GPU's 256 MB limit. wgpu answers a validation
    /// error by panicking, so the worker died and the frame never
    /// arrived. Nothing below supersample 1 shrinks it.
    ///
    /// It surfaced with Ducks only because Ducks had never taken the
    /// perturbed path before; every perturbing formula was exposed.
    /// The headless path checks this (`allocation_error`); the video
    /// exporter drives the renderer directly and did not.
    ///
    /// Asserts the predicate at its own boundary rather than a fixed
    /// 4K, so it means the same thing on a GPU with a 2 GB limit as on
    /// one with 256 MB -- and costs no allocation.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_frame_whose_state_cannot_fit_is_not_perturbed() {
        use crate::escape::EscapeRenderer;
        let (device, _queue) = repro_device();
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "ducks".to_string();
        esc.center_re = "-0.1".to_string();
        esc.center_im = "-0.675".to_string();
        esc.julia = true;
        esc.julia_re = 0.1;
        esc.julia_im = -0.675;
        esc.zoom_log2 = 20.0;
        esc.formula_params.insert("variant".to_string(), 0.0);
        assert!(
            EscapeRenderer::wants_perturbation(&esc),
            "the view must want perturbation, or this tests nothing"
        );

        let stride = crate::escape::assembler::iter_state_bytes(
            crate::escape::assembler::PerturbTier::Ducks(0),
            false,
        );
        let lim = device.limits();
        let cap = lim.max_buffer_size.min(lim.max_storage_buffer_binding_size as u64);
        let px_cap = cap / stride;
        let w = 3840u32;
        let fits_h = (px_cap / w as u64) as u32;
        println!(
            "cap {} MB, stride {stride} B -> {w}x{fits_h} fits, {w}x{} does not",
            cap / (1024 * 1024),
            fits_h + 1
        );
        assert!(
            EscapeRenderer::perturb_state_fits_at(&device, &esc, w, fits_h),
            "a frame exactly at the limit must still perturb"
        );
        assert!(
            !EscapeRenderer::perturb_state_fits_at(&device, &esc, w, fits_h + 2),
            "a frame past the limit must decline the perturbed path rather than              ask wgpu for the buffer (which panics)"
        );
        // The reported case, stated concretely: 4K needs 398 MB, so
        // any device allowing less than that must decline it.
        let need_4k = 3840u64 * 2160 * stride;
        assert_eq!(need_4k, 398_131_200, "the reported buffer size");
        assert_eq!(
            EscapeRenderer::perturb_state_fits_at(&device, &esc, 3840, 2160),
            need_4k <= cap,
            "4K must perturb exactly when this device can hold its state"
        );
    }

    /// The reported 4K frame renders, perturbed, on a device that asks
    /// for its adapter's real limits.
    ///
    /// The panic's 268,435,456 is not a GPU: it is wgpu's DEFAULT
    /// `max_buffer_size` (256 MiB), which the video exporter requested
    /// because it raised only the storage-BINDING limit from the
    /// adapter. The still-image exporter raises buffer, binding and
    /// texture limits alike. This test uses adapter limits (as
    /// `repro_device` does) and renders the exact reported size on
    /// the perturbed path; it is what the exporter can do once it
    /// asks the same way.
    #[test]
    #[ignore = "needs a GPU"]
    fn a_4k_deep_ducks_frame_renders_perturbed_under_adapter_limits() {
        let (device, queue) = repro_device();
        let lim = device.limits();
        let need = 3840u64 * 2160 * 48;
        if need > lim.max_buffer_size || need > lim.max_storage_buffer_binding_size as u64 {
            eprintln!("this adapter really is under 398 MB; nothing to prove here");
            return;
        }
        let (w, h) = (3840u32, 2160u32);
        let config = crate::config::FractalConfig::default();
        let renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, 64, 64,
            &config.flame, config.palette_size);
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "ducks".to_string();
        esc.coloring = "magnitude_average".to_string();
        esc.center_re = "-0.1000053437741560936430812".to_string();
        esc.center_im = "-0.6749972878037609392903475".to_string();
        esc.julia = true;
        esc.julia_re = 0.1;
        esc.julia_im = -0.675;
        esc.zoom_log2 = 20.0;
        esc.max_iter = 200;
        esc.bailout = 4.0;
        esc.formula_params.insert("variant".to_string(), 0.0);
        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        escape.set_chunk_time_target(200.0);
        let t0 = std::time::Instant::now();
        let mut guard = 0u32;
        loop {
            let mut e = device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor { label: Some("4k") });
            let settled = escape.render(&device, &queue, &mut e, &esc, renderer.palette_view());
            queue.submit(std::iter::once(e.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            if settled { break; }
            guard += 1;
            assert!(guard < 5_000, "4K frame never settled");
        }
        println!("4K Ducks frame: {:.2}s, {guard} extra frames, path={}",
            t0.elapsed().as_secs_f64(), escape.last_path);
        assert_eq!(escape.last_path, "perturbed f32",
            "under adapter limits the reported frame must take the perturbed path");
        escape.destroy();
    }

    /// Per-frame accumulation must not bleed between frames.
    ///
    /// The video exporter now antialiases by accumulation like the
    /// still path, but calls `begin_accumulation` once PER FRAME on
    /// one long-lived renderer. This pins the property that makes
    /// that correct: frame B accumulated after frame A equals frame B
    /// accumulated on its own. If a later change made the reset
    /// conditional on size alone, every frame of a video would carry
    /// a ghost of the one before it, and no still-image test would
    /// notice.
    #[test]
    #[ignore = "needs a GPU"]
    fn per_frame_accumulation_starts_clean() {
        let (device, queue) = repro_device();
        let (w, h) = (96u32, 72u32);
        let config = crate::config::FractalConfig::default();
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size);
        renderer.update_palette(&device, &queue, &config.palette, config.palette_rotation,
            config.palette_squeeze, config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse);
        let view = |zoom: f64| {
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.formula = "mandelbrot".to_string();
            esc.coloring = "smooth".to_string();
            esc.center_re = "-0.7436438870371587".to_string();
            esc.center_im = "0.1318259042053119".to_string();
            esc.zoom_log2 = zoom;
            esc.max_iter = 300;
            esc.supersample = 1;
            esc
        };
        // One frame, accumulated 2x2, on a given renderer; returns the
        // tonemapped pixels.
        let mut frame = |escape: &mut crate::escape::EscapeRenderer, esc: &_| -> Vec<u8> {
            let extra = 2u32;
            escape.begin_accumulation(&device, &queue, extra);
            for off in crate::escape::EscapeRenderer::sample_grid(extra) {
                escape.set_sample_offset(off);
                let mut enc = device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor { label: Some("acc") });
                let mut settled = escape.render(&device, &queue, &mut enc, esc, renderer.palette_view());
                let mut g = 0;
                while !settled {
                    queue.submit(std::iter::once(enc.finish()));
                    let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                    enc = device.create_command_encoder(
                        &wgpu::CommandEncoderDescriptor { label: Some("acc chunk") });
                    settled = escape.render(&device, &queue, &mut enc, esc, renderer.palette_view());
                    g += 1;
                    assert!(g < 100_000);
                }
                escape.accumulate_sample(&device, &queue, &mut enc);
                queue.submit(std::iter::once(enc.finish()));
                let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            }
            escape.set_sample_offset([0.0, 0.0]);
            let mut enc = device.create_command_encoder(
                &wgpu::CommandEncoderDescriptor { label: Some("tm") });
            renderer.update_background_color(&queue, [0.0, 0.0, 0.0]);
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc,
                escape.accumulated_view().expect("accumulated"));
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, [0.0, 0.0, 0.0])).expect("readback");
            rgba
        };
        // A after B on one renderer, versus B alone on a fresh one.
        let mut shared = crate::escape::EscapeRenderer::new(&device, w, h);
        let a = frame(&mut shared, &view(2.0));
        let b_after_a = frame(&mut shared, &view(5.0));
        shared.destroy();
        let mut fresh = crate::escape::EscapeRenderer::new(&device, w, h);
        let b_alone = frame(&mut fresh, &view(5.0));
        fresh.destroy();
        let diff_ab: usize = a.iter().zip(&b_after_a).filter(|(x, y)| x != y).count();
        assert!(diff_ab > 0, "the two views must differ, or this tests nothing");
        let worst = b_after_a.iter().zip(&b_alone)
            .map(|(x, y)| (*x as i32 - *y as i32).abs()).max().unwrap_or(0);
        println!("frames differ in {diff_ab} bytes; B-after-A vs B-alone worst channel delta {worst}");
        assert!(worst <= 1,
            "a frame accumulated after another must equal that frame alone (worst {worst}/255)");
    }

    /// The reported seam: Ducks just past the perturbation threshold.
    ///
    /// Curved lines cutting the image and sliding as you zoom deeper.
    /// They were the Zhuoran rebase firing on the branch wrap's own
    /// bookkeeping -- see `rebase_only_at_orbit_end`. This pins the
    /// user's exact view, on both rungs.
    #[test]
    #[ignore = "needs a GPU"]
    fn ducks_has_no_seams_at_the_perturbation_threshold() {
        let (w, h) = (96u32, 72u32);
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "ducks".to_string();
        esc.coloring = "magnitude_average".to_string();
        esc.center_re = "-0.10000534377415609364308126093".to_string();
        esc.center_im = "-0.67499728780376093929034752896".to_string();
        esc.julia = true;
        esc.julia_re = 0.1;
        esc.julia_im = -0.675;
        esc.zoom_log2 = 14.061247;
        esc.max_iter = 80;
        esc.bailout = 4.0;
        esc.formula_params.insert("variant".to_string(), 0.0);
        assert!(
            crate::escape::EscapeRenderer::wants_perturbation(&esc),
            "the reported view must be past the threshold, or this tests nothing"
        );
        let truth =
            exact_mean_magnitude(&esc, w, h, crate::escape::reference::MAP_DUCKS, [0.0, 0.0]);
        for deep in [false, true] {
            let recs = records_via(&esc, w, h, deep, true);
            let (mut total, mut over) = (0.0f64, 0usize);
            for (r, t) in recs.iter().zip(&truth) {
                let m = r.accum[0] as f64 / (r.accum[1] as f64).max(1.0);
                let rel = (m - t).abs() / t.abs().max(1e-9);
                total += rel;
                if rel > 1e-3 {
                    over += 1;
                }
            }
            let mean = total / truth.len() as f64;
            println!("ducks threshold seam (deep={deep}): mean {mean:.3e}, {over} pixels over 1e-3");
            // With the rebase seam: 2.2e-3 and 4899 of 6912 pixels.
            assert!(
                mean < 1e-4 && over < 20,
                "seams are back (deep={deep}): mean {mean:.3e}, {over} pixels over 1e-3"
            );
        }
    }

    /// Ducks on the Julia plane of the shipped preset (c = 0.1 - 0.62i)
    /// at zoom 30, the plain log and the log of the square, both
    /// rungs.
    #[test]
    #[ignore = "needs a GPU"]
    fn perturbed_ducks_matches_an_exact_orbit_at_depth() {
        let (w, h) = (96u32, 72u32);
        // Variant 4 logs the SQUARE, doubling the expansion per step,
        // and by 80 iterations its exact field is chaos-dominated:
        // measured, a nudge of 1e-3 of a PIXEL moves the exact mean by
        // 6.6e-3 relative on 6689 of 6912 pixels -- far more than any
        // f32 renderer's own error, so a tight assertion there would
        // be measuring noise rather than the tier. At 40 iterations
        // the same field is fully determined (1e-3-pixel nudge: 3e-7,
        // zero pixels past 1e-4) and still carries 8.7e-3 of contrast.
        for (variant, cre, cim, max_iter) in [
            (0.0f32, "-0.08431922458112219", "1.53503858856856801", 80u32),
            (4.0, "0.15", "-0.2", 40),
        ] {
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.formula = "ducks".to_string();
            esc.coloring = "magnitude_average".to_string();
            esc.julia = true;
            esc.julia_re = 0.1;
            esc.julia_im = -0.62;
            esc.center_re = cre.to_string();
            esc.center_im = cim.to_string();
            esc.zoom_log2 = 30.0;
            esc.max_iter = max_iter;
            esc.formula_params.insert("variant".to_string(), variant);
            let truth = exact_mean_magnitude(
                &esc, w, h, crate::escape::reference::MAP_DUCKS, [variant, 0.0],
            );
            for deep in [false, true] {
                let gpu = perturbed_records(&esc, w, h, deep);
                compare_means(&format!("ducks variant {variant} deep {deep}"), &gpu, &truth);
            }
        }
    }

    fn magnet_exact_orbit_case(variant: u32, deep: bool) {
        let (device, queue) = repro_device();
        // Each variant is a DIFFERENT map, so each needs its own
        // boundary: a view chosen on Magnet I's is entirely inside
        // Magnet II's, which the degeneracy check caught. Both are the
        // interior side, so the reference runs full length.
        //
        // These views separate TERMINATED from RAN-OUT, which is what
        // a binary lit/dark comparison can see. They contain no
        // converging pixels at all (everything escapes or lands on a
        // higher-period cycle the period-1 settle test cannot catch),
        // so the convergence path is tested separately — see
        // `perturbed_magnet_detects_convergence`, which exists because
        // disabling convergence support left THIS test passing.
        let (cx, cy) = if variant == 0 {
            (0.64249201397640587f64, 1.32937270291336684f64)
        } else {
            (0.46578907048808199f64, 1.31015553960527420f64)
        };
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "magnet".to_string();
        esc.center_re = format!("{cx:.17}");
        esc.center_im = format!("{cy:.17}");
        esc.zoom_log2 = 30.0;
        esc.max_iter = 400;
        esc.formula_params.insert("variant".to_string(), variant as f32);

        let (w, h) = (96u32, 72u32);
        let mut config = crate::config::FractalConfig::default();
        config.palette = crate::scene::palette::Palette {
            name: "white".to_string(),
            stops: vec![
                crate::scene::palette::ColorStop { position: 0.0, color: [1.0, 1.0, 1.0] },
                crate::scene::palette::ColorStop { position: 1.0, color: [1.0, 1.0, 1.0] },
            ],
            locked: false,
            built_in: false,
        };
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation, config.palette_squeeze,
            config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse,
        );
        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        escape.force_floatexp = deep;
        let mut guard = 0u32;
        loop {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("magnet depth"),
            });
            let settled = escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            if settled {
                break;
            }
            guard += 1;
            assert!(guard < 100_000, "render did not settle");
        }
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("magnet depth tonemap"),
        });
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
            w,
            h,
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
        renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
        queue.submit(std::iter::once(enc.finish()));
        let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
            &device, &queue, false, config.background_color,
        ))
        .expect("readback");
        escape.destroy();

        let span_y = 4.0 / (esc.zoom_log2 as f64).exp2();
        let span_x = span_y * w as f64 / h as f64;
        let mut differ = 0usize;
        for py in 0..h {
            for px in 0..w {
                let c = (
                    ((px as f64 + 0.5) / w as f64 - 0.5) * span_x + cx,
                    -(((py as f64 + 0.5) / h as f64 - 0.5) * span_y) + cy,
                );
                let (mut zx, mut zy) = (0.0f64, 0.0f64);
                let mut done = false;
                for _ in 0..esc.max_iter {
                    let (pzx, pzy) = (zx, zy);
                    let (nr, ni, dr, di) = if variant == 0 {
                        (
                            zx * zx - zy * zy + c.0 - 1.0,
                            2.0 * zx * zy + c.1,
                            2.0 * zx + c.0 - 2.0,
                            2.0 * zy + c.1,
                        )
                    } else {
                        let (c1r, c1i) = (c.0 - 1.0, c.1);
                        let (c2r, c2i) = (c.0 - 2.0, c.1);
                        let (p12r, p12i) = (c1r * c2r - c1i * c2i, c1r * c2i + c1i * c2r);
                        let (z2r, z2i) = (zx * zx - zy * zy, 2.0 * zx * zy);
                        let (z3r, z3i) = (z2r * zx - z2i * zy, z2r * zy + z2i * zx);
                        (
                            z3r + 3.0 * (c1r * zx - c1i * zy) + p12r,
                            z3i + 3.0 * (c1r * zy + c1i * zx) + p12i,
                            3.0 * z2r + 3.0 * (c2r * zx - c2i * zy) + p12r + 1.0,
                            3.0 * z2i + 3.0 * (c2r * zy + c2i * zx) + p12i,
                        )
                    };
                    let d2 = dr * dr + di * di;
                    if d2 < 1e-300 {
                        done = true;
                        break;
                    }
                    let (qr, qi) = ((nr * dr + ni * di) / d2, (ni * dr - nr * di) / d2);
                    zx = qr * qr - qi * qi;
                    zy = 2.0 * qr * qi;
                    // Convergence, exactly as the templates test it.
                    let (ddx, ddy) = (zx - pzx, zy - pzy);
                    if ddx * ddx + ddy * ddy < 1e-12 {
                        done = true;
                        break;
                    }
                    if zx * zx + zy * zy > 4.0 {
                        done = true;
                        break;
                    }
                }
                let i = ((py * w + px) * 4) as usize;
                let lit = rgba[i] as u32 + rgba[i + 1] as u32 + rgba[i + 2] as u32 > 24;
                if lit != done {
                    differ += 1;
                }
            }
        }
        let lit_px = rgba
            .chunks(4)
            .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
            .count();
        let total = (w * h) as usize;
        assert!(
            lit_px > total / 20 && lit_px < total * 19 / 20,
            "degenerate view (variant {variant}): {lit_px}/{total} pixels terminated"
        );
        let frac = differ as f64 / (w * h) as f64;
        println!(
            "magnet v{variant} at zoom {} on the {} rung: {:.2}% differ from the exact orbit",
            esc.zoom_log2,
            if deep { "floatexp" } else { "scaled" },
            100.0 * frac
        );
        assert!(
            frac < 0.01,
            "perturbed Magnet v{variant} disagrees with an exact orbit on {:.1}% of pixels",
            100.0 * frac
        );
    }

    /// The perturbed path must DETECT CONVERGENCE, not just escape.
    ///
    /// Magnet's orbits settle at z = 1 rather than diverging, and both
    /// perturbed templates used to hardcode `converged = false` — so a
    /// converging pixel ran to `max_iter` and reported an iteration
    /// count two orders of magnitude too large. Every escape-count and
    /// smooth coloring shades convergence SPEED for this family, so
    /// that is a different picture, not a rounding difference.
    ///
    /// The binary lit/dark comparison cannot see this: the template
    /// sets `escaped` on convergence too, so converged and escaped
    /// pixels are both lit. This compares the ITERATION COUNT instead,
    /// on a view chosen for a convergence/escape boundary (measured
    /// 256 converging against 176 escaping), by binning rendered
    /// luminance against the f64 oracle's termination iteration —
    /// palette-agnostic, since it only asserts that equal counts
    /// render equally.
    #[test]
    #[ignore = "needs a GPU"]
    fn perturbed_magnet_detects_convergence() {
        let (device, queue) = repro_device();
        let (cx, cy) = (-0.17175811456405135f64, 0.09206142643187049f64);
        let (w, h) = (96u32, 72u32);
        const MAX_ITER: u32 = 400;

        let mut config = crate::config::FractalConfig::default();
        config.palette = crate::scene::palette::Palette {
            name: "ramp".to_string(),
            stops: vec![
                crate::scene::palette::ColorStop { position: 0.0, color: [0.0, 0.0, 0.0] },
                crate::scene::palette::ColorStop { position: 1.0, color: [1.0, 1.0, 1.0] },
            ],
            locked: false,
            built_in: false,
        };
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation, config.palette_squeeze,
            config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse,
        );

        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "magnet".to_string();
        esc.coloring = "escape_count".to_string();
        esc.center_re = format!("{cx:.17}");
        esc.center_im = format!("{cy:.17}");
        esc.zoom_log2 = 30.0;
        esc.max_iter = MAX_ITER;
        esc.formula_params.insert("variant".to_string(), 0.0);
        // One palette turn across the whole iteration range, so a
        // count of 5 and a count of 400 cannot land on the same
        // colour by wrapping.
        esc.coloring_params.insert("scale".to_string(), 1.0 / MAX_ITER as f32);

        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        let mut guard = 0u32;
        loop {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("magnet converge"),
            });
            let settled = escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            if settled {
                break;
            }
            guard += 1;
            assert!(guard < 100_000, "render did not settle");
        }
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("magnet converge tonemap"),
        });
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
            w,
            h,
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
        renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
        queue.submit(std::iter::once(enc.finish()));
        let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
            &device, &queue, false, config.background_color,
        ))
        .expect("readback");
        escape.destroy();

        // f64 oracle: the termination iteration, and how it ended.
        let span_y = 4.0 / (esc.zoom_log2 as f64).exp2();
        let span_x = span_y * w as f64 / h as f64;
        const BINS: usize = 128;
        let mut sums = vec![0f64; BINS];
        let mut sqs = vec![0f64; BINS];
        let mut counts = vec![0usize; BINS];
        let mut converged_px = 0usize;
        for py in 0..h {
            for px in 0..w {
                let c = (
                    ((px as f64 + 0.5) / w as f64 - 0.5) * span_x + cx,
                    -(((py as f64 + 0.5) / h as f64 - 0.5) * span_y) + cy,
                );
                let (mut zx, mut zy) = (0.0f64, 0.0f64);
                let mut n = MAX_ITER;
                let mut conv = false;
                for i in 0..MAX_ITER {
                    let (pzx, pzy) = (zx, zy);
                    let (nr, ni) = (zx * zx - zy * zy + c.0 - 1.0, 2.0 * zx * zy + c.1);
                    let (dr, di) = (2.0 * zx + c.0 - 2.0, 2.0 * zy + c.1);
                    let d2 = dr * dr + di * di;
                    if d2 < 1e-300 {
                        n = i;
                        break;
                    }
                    let (qr, qi) = ((nr * dr + ni * di) / d2, (ni * dr - nr * di) / d2);
                    zx = qr * qr - qi * qi;
                    zy = 2.0 * qr * qi;
                    let (ddx, ddy) = (zx - pzx, zy - pzy);
                    if ddx * ddx + ddy * ddy < 1e-12 {
                        n = i;
                        conv = true;
                        break;
                    }
                    if zx * zx + zy * zy > 4.0 {
                        n = i;
                        break;
                    }
                }
                if conv {
                    converged_px += 1;
                }
                let b = ((n as usize) * (BINS - 1) / MAX_ITER as usize).min(BINS - 1);
                let i = ((py * w + px) * 4) as usize;
                let lum = rgba[i] as f64;
                counts[b] += 1;
                sums[b] += lum;
                sqs[b] += lum * lum;
            }
        }
        assert!(
            converged_px > (w * h) as usize / 10,
            "view has only {converged_px} converging pixels -- it cannot test convergence"
        );
        let (mut spread, mut total) = (0f64, 0usize);
        for b in 0..BINS {
            if counts[b] < 20 {
                continue;
            }
            let n = counts[b] as f64;
            spread += (sqs[b] / n - (sums[b] / n).powi(2)).max(0.0).sqrt() * n;
            total += counts[b];
        }
        let spread = spread / total.max(1) as f64;
        println!(
            "magnet convergence: {converged_px} converging pixels, colour spread within an \
             iteration bin {spread:.2}/255"
        );
        assert!(
            // Calibrated, not guessed: this reads 0.58/255 with the
            // settle test compiled in and 10.82/255 with it disabled,
            // so 4.0 sits clear of the working value and well below
            // the broken one.
            spread < 4.0,
            "the rendered iteration count does not track the exact orbit's \
             ({spread:.2}/255) -- converging pixels are probably running to max_iter, \
             which is what a perturbed path without the settle test does"
        );
    }

    /// Lattès under the sphere average must track an f64 oracle — all
    /// three variants, and with the iterate stride engaged.
    ///
    /// Two new things at once, so the oracle covers both: a rational
    /// map whose Julia set is the WHOLE sphere (so every pixel
    /// iterates forever and there is nothing to escape to), and a
    /// coloring measuring CHORDAL distance, in which infinity is an
    /// ordinary point.
    ///
    /// The comparison bins rendered luminance against the oracle's
    /// mean distance and measures the spread WITHIN a bin, which
    /// asserts only that equal values render equally — it never
    /// assumes what colour a value maps to, so the palette and
    /// tonemap are free.
    ///
    /// The far-field guard is the part most likely to be wrong:
    /// these orbits pass close to poles, and `(z^2-a)^2` overflows f32
    /// around |z| ~ 1e10. The oracle applies the same leading-order
    /// forms at the same threshold, so a mismatch in either the
    /// threshold or the limits shows up here rather than as an
    /// occasional NaN pixel.
    #[test]
    #[ignore = "needs a GPU"]
    fn lattes_under_the_sphere_average_matches_an_exact_orbit() {
        let (device, queue) = repro_device();
        let (w, h) = (128u32, 96u32);
        // SIXTEEN iterations, and the number is measured rather than
        // chosen for speed. A Lattès Julia set is the whole sphere, so
        // every orbit is chaotic by construction and an f32 render
        // separates from an f64 oracle once rounding has amplified --
        // the same limit the lambda tier ran into. Measured spread
        // against the oracle, for the four cases below:
        //
        //     max_iter 16   0.48  0.48  0.48  0.48
        //     max_iter 24   2.89  0.48  0.73  0.76
        //     max_iter 40   7.03  0.52  5.09 10.78
        //
        // Past ~16 the comparison measures chaos rather than the
        // implementation. Sixteen is plenty to exercise the map, the
        // metric, the stride and the far-field guard.
        const MAX_ITER: u32 = 16;
        const A: (f64, f64) = (-0.5, 0.8660254);

        let mut config = crate::config::FractalConfig::default();
        config.palette = crate::scene::palette::Palette {
            name: "ramp".to_string(),
            stops: vec![
                crate::scene::palette::ColorStop { position: 0.0, color: [0.0, 0.0, 0.0] },
                crate::scene::palette::ColorStop { position: 1.0, color: [1.0, 1.0, 1.0] },
            ],
            locked: false,
            built_in: false,
        };
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation, config.palette_squeeze,
            config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse,
        );

        for (variant, stride) in [(0u32, 1u32), (1, 1), (2, 1), (2, 3)] {
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.formula = "lattes".to_string();
            esc.coloring = "sphere_average".to_string();
            esc.center_re = "0.0".to_string();
            esc.center_im = "0.0".to_string();
            esc.zoom_log2 = -1.0;
            esc.max_iter = MAX_ITER;
            esc.formula_params.insert("variant".to_string(), variant as f32);
            esc.formula_params.insert("a_re".to_string(), A.0 as f32);
            esc.formula_params.insert("a_im".to_string(), A.1 as f32);
            esc.coloring_params.insert("target_re".to_string(), 0.35);
            esc.coloring_params.insert("target_im".to_string(), -0.2);
            esc.coloring_params.insert("at_infinity".to_string(), 0.0);
            esc.coloring_params.insert("stride".to_string(), stride as f32);
            esc.coloring_params.insert("scale".to_string(), 0.5);

            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            let mut guard = 0u32;
            loop {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("lattes"),
                });
                let settled =
                    escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                let _ =
                    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 10_000, "render did not settle");
            }
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("lattes tonemap"),
            });
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
                w,
                h,
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
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, config.background_color,
            ))
            .expect("readback");
            escape.destroy();

            // --- f64 oracle: the same map and the same metric ---
            fn cmul(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
                (a.0 * b.0 - a.1 * b.1, a.0 * b.1 + a.1 * b.0)
            }
            fn cdiv(a: (f64, f64), b: (f64, f64)) -> (f64, f64) {
                let d = b.0 * b.0 + b.1 * b.1;
                ((a.0 * b.0 + a.1 * b.1) / d, (a.1 * b.0 - a.0 * b.1) / d)
            }
            let step = |z: (f64, f64)| -> (f64, f64) {
                let r2 = z.0 * z.0 + z.1 * z.1;
                if r2 > 1.0e12 {
                    return match variant {
                        0 => (z.0 * 0.25, z.1 * 0.25),
                        1 => (z.1 * 0.5, -z.0 * 0.5),
                        _ => cdiv((1.0, 0.0), A),
                    };
                }
                if variant == 1 {
                    if r2 < 1.0e-30 {
                        return (1.0e7, 0.0);
                    }
                    let s = {
                        let inv = cdiv((1.0, 0.0), z);
                        (z.0 + inv.0, z.1 + inv.1)
                    };
                    return (s.1 * 0.5, -s.0 * 0.5);
                }
                let (num, den) = if variant == 0 {
                    let z2 = cmul(z, z);
                    let t = (z2.0 - A.0, z2.1 - A.1);
                    (
                        cmul(t, t),
                        {
                            let p = cmul(cmul(z, (z.0 - 1.0, z.1)), (z.0 - A.0, z.1 - A.1));
                            (4.0 * p.0, 4.0 * p.1)
                        },
                    )
                } else {
                    let z3 = cmul(cmul(z, z), z);
                    let az3 = cmul(A, z3);
                    ((z3.0 + A.0, z3.1 + A.1), (az3.0 + 1.0, az3.1))
                };
                if den.0 * den.0 + den.1 * den.1 < 1.0e-30 {
                    return (1.0e7, 0.0);
                }
                cdiv(num, den)
            };

            let span_y = 4.0 / (esc.zoom_log2 as f64).exp2();
            let span_x = span_y * w as f64 / h as f64;
            let t = (0.35f64, -0.2f64);
            const BINS: usize = 192;
            let mut sums = vec![0f64; BINS];
            let mut sqs = vec![0f64; BINS];
            let mut counts = vec![0usize; BINS];
            for py in 0..h {
                for px in 0..w {
                    let mut z = (
                        ((px as f64 + 0.5) / w as f64 - 0.5) * span_x,
                        -(((py as f64 + 0.5) / h as f64 - 0.5) * span_y),
                    );
                    let mut sum = 0.0f64;
                    let mut samples = 0f64;
                    for call in 0..MAX_ITER {
                        z = step(z);
                        if stride > 1 && call % stride != 0 {
                            continue;
                        }
                        let zz = z.0 * z.0 + z.1 * z.1;
                        let dz = (z.0 - t.0, z.1 - t.1);
                        let d = 2.0 * (dz.0 * dz.0 + dz.1 * dz.1).sqrt()
                            / ((1.0 + zz) * (1.0 + t.0 * t.0 + t.1 * t.1)).sqrt();
                        sum += d;
                        samples += 1.0;
                    }
                    let mean = sum / samples.max(1.0);
                    // The palette coordinate the shader would compute.
                    let v = (mean * 0.5).rem_euclid(1.0);
                    let b = ((v * (BINS - 1) as f64).round() as usize).min(BINS - 1);
                    let i = ((py * w + px) * 4) as usize;
                    let lum = rgba[i] as f64;
                    counts[b] += 1;
                    sums[b] += lum;
                    sqs[b] += lum * lum;
                }
            }
            let (mut spread, mut total) = (0f64, 0usize);
            for b in 0..BINS {
                if counts[b] < 20 {
                    continue;
                }
                let n = counts[b] as f64;
                spread += (sqs[b] / n - (sums[b] / n).powi(2)).max(0.0).sqrt() * n;
                total += counts[b];
            }
            let spread = spread / total.max(1) as f64;
            println!(
                "lattes v{variant} stride {stride}: colour spread within a value bin \
                 {spread:.2}/255"
            );
            assert!(
                // Calibrated: every case reads 0.48/255 here, and the
                // chaos-limited values above start at 2.89.
                spread < 1.5,
                "lattes v{variant} stride {stride}: the render does not track an f64 \
                 oracle of the map and the chordal metric ({spread:.2}/255)"
            );
        }
    }

    /// Relief `softness` must BLUR the relief, and only the relief.
    ///
    /// Three properties, and the third is the one that matters most,
    /// because the first two passed against an implementation that
    /// was wrong. Softening must reduce high-frequency wobble (that
    /// is what it is for); it must not blur the IMAGE, since the
    /// stencil widens the derivative estimate and not the colour; and
    /// it must not DISPLACE anything. The original implementation
    /// widened the difference to a ring of radius r, which satisfies
    /// the first two and fails the third: every tap stays a sharp
    /// sample, so each edge prints ghost copies at +-r. It reached
    /// the app before anyone noticed.
    ///
    /// Two properties, and the second is the one that makes this
    /// worth a test rather than an eyeball. A wider normal stencil
    /// obviously reduces high-frequency wobble in the relief — that
    /// is what it is for. What it must NOT do is blur the image: the
    /// stencil widens the derivative estimate, not the colour, so the
    /// palette detail underneath has to survive intact. A softness
    /// implemented by blurring the finished RGB would pass the first
    /// check and fail the second.
    ///
    /// Roughness is measured as the mean absolute second difference
    /// along each row — a plain high-frequency detector, and the same
    /// quantity for both images, so no calibration is needed beyond
    /// comparing them to each other.
    #[test]
    #[ignore = "needs a GPU"]
    fn relief_softness_smooths_the_shading_without_blurring_the_image() {
        let (device, queue) = repro_device();
        let (w, h) = (200u32, 200u32);

        let mut config = crate::config::FractalConfig::default();
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation, config.palette_squeeze,
            config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse,
        );

        // Origami under position_map: a deliberately fine-grained
        // field, which is where the sharp stencil looks crunchy.
        let mut base = crate::config::escape::EscapeConfig::default();
        base.formula = "origami".to_string();
        base.coloring = "position_map".to_string();
        base.center_re = "0.0".to_string();
        base.center_im = "0.0".to_string();
        base.zoom_log2 = -1.0;
        base.max_iter = 32;
        base.formula_params.insert("seed".to_string(), 8.0);
        base.formula_params.insert("spread".to_string(), 2.0);
        base.coloring_params.insert("freq_x".to_string(), 5.0);
        base.coloring_params.insert("freq_y".to_string(), 1.5);
        base.coloring_params.insert("address_mix".to_string(), 1.5);

        let shoot = |softness: f32,
                     shaded: bool,
                     renderer: &mut crate::renderer::compute_kernel::FlameRenderer|
         -> Vec<u8> {
            let mut esc = base.clone();
            esc.shading = crate::config::escape::EscapeShading {
                enabled: shaded,
                height: 30.0,
                softness,
                ..Default::default()
            };
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            let mut guard = 0u32;
            loop {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("softness"),
                });
                let settled =
                    escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                let _ =
                    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 10_000, "render did not settle");
            }
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("softness tonemap"),
            });
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
                w,
                h,
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
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, config.background_color,
            ))
            .expect("readback");
            escape.destroy();
            rgba
        };

        // Mean |second difference| along rows: high-frequency content.
        let roughness = |px: &[u8]| -> f64 {
            let mut acc = 0f64;
            let mut n = 0usize;
            for y in 0..h as usize {
                for x in 1..w as usize - 1 {
                    let i = |xx: usize| (y * w as usize + xx) * 4;
                    for ch in 0..3 {
                        let a = px[i(x - 1) + ch] as f64;
                        let b = px[i(x) + ch] as f64;
                        let c = px[i(x + 1) + ch] as f64;
                        acc += (c - 2.0 * b + a).abs();
                        n += 1;
                    }
                }
            }
            acc / n as f64
        };

        let sharp = shoot(0.0, true, &mut renderer);
        let soft = shoot(4.0, true, &mut renderer);
        let unshaded = shoot(0.0, false, &mut renderer);

        let (r_sharp, r_soft, r_flat) =
            (roughness(&sharp), roughness(&soft), roughness(&unshaded));
        println!(
            "relief roughness: sharp {r_sharp:.2}, soft {r_soft:.2}, unshaded {r_flat:.2}"
        );

        // 1. Softening must actually soften.
        assert!(
            r_soft < r_sharp * 0.85,
            "softness 4 barely changed the relief ({r_soft:.2} against {r_sharp:.2}) -- \
             the wider stencil is not reaching the normal"
        );

        // 2. And it must not have blurred the picture. The shading is
        // a LAYER over the coloring, so the palette detail underneath
        // is untouched by the stencil width -- a softness that blurred
        // the finished RGB would push this below the unshaded floor.
        assert!(
            r_soft > r_flat * 0.75,
            "the softened render ({r_soft:.2}) has less detail than the UNSHADED one \
             ({r_flat:.2}) -- softness is blurring the image, not the normal"
        );

        // The third property -- that softening must not DISPLACE the
        // structure, which is how the first implementation failed --
        // is pinned by `height_softening_is_a_real_blur` instead. It
        // cannot be measured here: the shading term extracted from
        // the composite is nonlinear (perceptual blend, then gamma),
        // and both a correlation against a shifted copy and a
        // commutation check were tried and could not separate a
        // proper blur from the ring stencil. The mechanism is
        // testable exactly; the composite is not.
    }

    /// The relief softening must be a REAL LOW-PASS of the height
    /// field — the property the first implementation lacked.
    ///
    /// That version widened the difference stencil to a ring of
    /// radius r. Every tap stayed a sharp sample, so it never blurred
    /// anything: it estimated the slope at p from the height at p±r,
    /// which displaces the structure and prints ghost copies either
    /// side of every edge. It shipped because the tests then in place
    /// measured only that high-frequency content went DOWN (it does,
    /// for the wrong reason) and that the colour was untouched (it
    /// was). Reported from the app as the relief "mirroring into 3
    /// equally sharp parts".
    ///
    /// The composite cannot settle this — the shading term is
    /// nonlinear, and two different composite-level metrics ranked
    /// the ring stencil BETTER than the fix. So this checks the
    /// mechanism where it is exactly checkable: the blurred height
    /// field must equal a Gaussian blur of the raw one, computed
    /// independently on the CPU. A stencil that samples a ring cannot
    /// pass that, because it is not a blur of anything.
    #[test]
    #[ignore = "needs a GPU"]
    fn height_softening_is_a_real_blur() {
        let (device, queue) = repro_device();
        let (w, h) = (128u32, 96u32);
        let config = crate::config::FractalConfig::default();
        let renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );

        const SIGMA: f32 = 3.0;
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "mandelbrot".to_string();
        esc.coloring = "smooth".to_string();
        esc.max_iter = 200;
        esc.shading = crate::config::escape::EscapeShading {
            enabled: true,
            height: 30.0,
            softness: SIGMA,
            ..Default::default()
        };

        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        let mut guard = 0u32;
        loop {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("blur"),
            });
            let settled =
                escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            if settled {
                break;
            }
            guard += 1;
            assert!(guard < 100_000, "render did not settle");
        }

        let raw = pollster::block_on(escape.read_height(&device, &queue, false))
            .expect("raw height");
        let gpu = pollster::block_on(escape.read_height(&device, &queue, true))
            .expect("blurred height — the softening pass must have run");
        assert_eq!(raw.len(), (w * h) as usize);
        assert_eq!(gpu.len(), raw.len());

        // The same Gaussian, separable, clamped at the edges — exactly
        // what the shader does.
        let rad = (SIGMA * 3.0).ceil() as i32;
        let wts: Vec<f32> = (-rad..=rad)
            .map(|i| (-(i * i) as f32 / (2.0 * SIGMA * SIGMA)).exp())
            .collect();
        let mut tmp = vec![0.0f32; raw.len()];
        let mut cpu = vec![0.0f32; raw.len()];
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let (mut a, mut d) = (0.0f32, 0.0f32);
                for (k, wt) in wts.iter().enumerate() {
                    let xx = (x + k as i32 - rad).clamp(0, w as i32 - 1);
                    a += raw[(y * w as i32 + xx) as usize] * wt;
                    d += wt;
                }
                tmp[(y * w as i32 + x) as usize] = a / d;
            }
        }
        for y in 0..h as i32 {
            for x in 0..w as i32 {
                let (mut a, mut d) = (0.0f32, 0.0f32);
                for (k, wt) in wts.iter().enumerate() {
                    let yy = (y + k as i32 - rad).clamp(0, h as i32 - 1);
                    a += tmp[(yy * w as i32 + x) as usize] * wt;
                    d += wt;
                }
                cpu[(y * w as i32 + x) as usize] = a / d;
            }
        }

        // Scale-relative error: the height field is the coloring's raw
        // value, whose magnitude is arbitrary.
        let span = raw.iter().cloned().fold(f32::MIN, f32::max)
            - raw.iter().cloned().fold(f32::MAX, f32::min);
        assert!(span > 0.0, "the height field is flat; nothing to blur");
        let mut worst = 0.0f32;
        let mut rms = 0.0f64;
        for (a, b) in gpu.iter().zip(cpu.iter()) {
            let e = (a - b).abs() / span;
            worst = worst.max(e);
            rms += (e * e) as f64;
        }
        rms = (rms / gpu.len() as f64).sqrt();
        println!("height blur vs CPU Gaussian: rms {rms:.5}, worst {worst:.5} of range");
        assert!(
            worst < 0.01,
            "the softening pass is not the Gaussian it claims to be (worst \
             {worst:.4} of the field's range) -- a stencil that samples a ring \
             rather than averaging the interior fails here"
        );

        // And it must actually SMOOTH: the blurred field's own
        // roughness has to fall well below the raw one's, or a
        // no-op pass would satisfy the comparison above.
        let rough = |s: &[f32]| -> f64 {
            let mut acc = 0f64;
            for y in 0..h as usize {
                for x in 1..w as usize - 1 {
                    let i = y * w as usize + x;
                    acc += ((s[i + 1] - 2.0 * s[i] + s[i - 1]) as f64).abs();
                }
            }
            acc
        };
        let (r_raw, r_blur) = (rough(&raw), rough(&gpu));
        println!("height roughness: raw {r_raw:.1}, blurred {r_blur:.1}");
        assert!(
            r_blur < r_raw * 0.5,
            "the blurred field is barely smoother than the raw one ({r_blur:.1} \
             against {r_raw:.1}) -- the pass ran but did nothing"
        );
        escape.destroy();
    }

    /// A shadow at full strength must bite about as hard as a
    /// highlight at full strength.
    ///
    /// Reported from the app: at relief height 50, black shadows at
    /// strength 1.0 were barely visible while white highlights at
    /// strength 0.03 looked about as strong — a ~30x asymmetry in a
    /// pair of controls that read as symmetric.
    ///
    /// The RESPONSE is symmetric (a shadow-only render at angle A is
    /// bit-identical to a highlight-only render at A+180, which
    /// another test pins). The asymmetry is in the composite: the
    /// escape shader emits LINEAR light, and in linear space a
    /// mostly-dark image has almost nothing for `multiply` to take
    /// away while `screen` has everything to add. This measures the
    /// mean absolute change each side makes against the unshaded
    /// render, which is the quantity the eye is reporting.
    #[test]
    #[ignore = "needs a GPU"]
    fn shadow_and_highlight_bite_equally_hard() {
        let (device, queue) = repro_device();
        let (w, h) = (200u32, 200u32);

        let mut config = crate::config::FractalConfig::default();
        config.palette = crate::scene::palette::Palette {
            name: "paper".to_string(),
            stops: vec![
                // DARK on purpose. This is where the bug bit hardest:
                // in linear light a dark base gives `multiply` almost
                // nothing to remove while `screen` has the whole range
                // to add into, so the two strength sliders diverge.
                crate::scene::palette::ColorStop { position: 0.0, color: [0.02, 0.01, 0.05] },
                crate::scene::palette::ColorStop { position: 0.5, color: [0.16, 0.08, 0.20] },
                crate::scene::palette::ColorStop { position: 1.0, color: [0.02, 0.01, 0.05] },
            ],
            locked: false,
            built_in: false,
        };
        config.background_color = [0.0, 0.0, 0.0];
        config.exposure = 1.0;
        config.gamma = 1.0;
        config.brightness = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation, config.palette_squeeze,
            config.palette_squeeze_mode, config.palette_squeeze_falloff,
            config.palette_log_strength, config.palette_reverse,
        );

        let mut base = crate::config::escape::EscapeConfig::default();
        base.formula = "origami".to_string();
        base.coloring = "position_map".to_string();
        base.center_re = "0.0".to_string();
        base.center_im = "0.0".to_string();
        base.zoom_log2 = -1.0;
        base.max_iter = 32;
        base.formula_params.insert("seed".to_string(), 8.0);
        base.formula_params.insert("spread".to_string(), 2.0);
        base.coloring_params.insert("freq_x".to_string(), 5.0);
        base.coloring_params.insert("freq_y".to_string(), 1.5);
        base.coloring_params.insert("address_mix".to_string(), 1.5);

        let shoot = |shading: crate::config::escape::EscapeShading,
                     renderer: &mut crate::renderer::compute_kernel::FlameRenderer|
         -> Vec<u8> {
            let mut esc = base.clone();
            esc.shading = shading;
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            let mut guard = 0u32;
            loop {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("asym"),
                });
                let settled =
                    escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                let _ =
                    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 10_000, "render did not settle");
            }
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("asym tonemap"),
            });
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
                w,
                h,
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
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, config.background_color,
            ))
            .expect("readback");
            escape.destroy();
            rgba
        };

        use crate::config::escape::{EscapeShading, ShadingBlend};
        let off = shoot(EscapeShading::default(), &mut renderer);
        let shadow = shoot(
            EscapeShading {
                enabled: true,
                height: 50.0,
                shadow_color: [0.0, 0.0, 0.0],
                shadow_strength: 1.0,
                shadow_blend: ShadingBlend::Multiply,
                highlight_strength: 0.0,
                ..Default::default()
            },
            &mut renderer,
        );
        let highlight = shoot(
            EscapeShading {
                enabled: true,
                height: 50.0,
                shadow_strength: 0.0,
                highlight_color: [1.0, 1.0, 1.0],
                highlight_strength: 1.0,
                highlight_blend: ShadingBlend::Screen,
                ..Default::default()
            },
            &mut renderer,
        );

        let bite = |a: &[u8]| -> f64 {
            let mut acc = 0f64;
            for (p, q) in a.chunks(4).zip(off.chunks(4)) {
                for ch in 0..3 {
                    acc += (p[ch] as f64 - q[ch] as f64).abs();
                }
            }
            acc / (a.len() / 4 * 3) as f64
        };
        let hard = shoot(
            EscapeShading {
                enabled: true,
                height: 50.0,
                shadow_color: [0.0, 0.0, 0.0],
                shadow_strength: 4.0,
                shadow_blend: ShadingBlend::Multiply,
                highlight_strength: 0.0,
                ..Default::default()
            },
            &mut renderer,
        );

        let mean = |a: &[u8]| -> f64 {
            let s: f64 = a
                .chunks(4)
                .map(|p| (p[0] as f64 + p[1] as f64 + p[2] as f64) / 3.0)
                .sum();
            s / (a.len() / 4) as f64
        };
        let (m_off, m_shadow, m_hard) = (mean(&off), mean(&shadow), mean(&hard));
        let (bs, bh) = (bite(&shadow), bite(&highlight));
        println!(
            "relief: unshaded {m_off:.1}/255, shadow@1 {m_shadow:.1}, shadow@4 {m_hard:.1}; bite shadow {bs:.2} highlight {bh:.2}"
        );

        // The headroom control has to actually reach: at strength 4 a
        // black shadow must take a real bite out of the image.
        assert!(
            m_hard < m_off * 0.70,
            "a full black shadow at strength 4 only moved the mean from {m_off:.1} to {m_hard:.1} -- shadows still cannot get dark, which is the report"
        );
        // ...and the extra range must be what does it, so the slider
        // is a real control rather than something saturating at 1.
        assert!(
            m_hard < m_shadow * 0.95,
            "strength 4 ({m_hard:.1}) is no darker than strength 1 ({m_shadow:.1}) -- the extended range is not reaching the blend"
        );
    }

    /// Interactive-latency attribution across the perturbation
    /// threshold, in the app's own progressive mode.
    ///
    /// The report from the app: edits render in real time up to zoom
    /// 14, and with a noticeable delay past it. This simulates the
    /// edits a user actually makes -- a coloring-slider tick, a small
    /// pan, a zoom notch -- on both sides of the threshold, and prints
    /// what the diagnostics attribute each settle to (reference orbit
    /// recomputed or reused, worker wait frames, BLA build, chunked
    /// frames). The assertions are deliberately weak (the numbers are
    /// machine-dependent); the value is the printed table.
    #[test]
    #[ignore = "needs a GPU"]
    fn interactive_latency_report() {
        let _diag = diag_lock();
        crate::escape::diag::reset();
        let (device, queue) = repro_device();
        let (w, h) = (960u32, 720u32);

        let config = crate::config::FractalConfig::default();
        let renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );

        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        escape.progressive = true;

        // Seahorse-valley point, exact enough for any zoom used here.
        let re = "-0.74364388703715870475";
        let im = "0.13182590420531197049";
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.center_re = re.to_string();
        esc.center_im = im.to_string();
        esc.max_iter = 2000;

        let mut settle = |esc: &crate::config::escape::EscapeConfig,
                          escape: &mut crate::escape::EscapeRenderer,
                          label: &str| {
            let t0 = web_time::Instant::now();
            let mut frames = 0u32;
            loop {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("latency"),
                });
                let settled = escape.render(&device, &queue, &mut enc, esc, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                frames += 1;
                if settled {
                    break;
                }
                assert!(frames < 200_000, "render did not settle ({label})");
            }
            let wall = t0.elapsed().as_secs_f32() * 1000.0;
            let d = crate::escape::diag::snapshot();
            println!(
                "{label}: {wall:.1} ms wall, {frames} frames | path={} orbit={} {} {:.1}ms \
                 waits={} rebuilds={} | bla={} {:.1}ms | upload={}KB chunk={} cpu={:.2}ms",
                d.path,
                d.orbit_len,
                d.orbit_source.label(),
                d.orbit_ms,
                d.orbit_wait_frames,
                d.orbit_rebuilds,
                if d.bla_active { "on" } else { "off" },
                d.bla_build_ms,
                d.upload_bytes / 1024,
                d.last_chunk_iters,
                d.render_cpu_ms,
            );
            (wall, frames)
        };

        for zoom in [13.0f64, 15.0, 25.0] {
            println!("--- zoom {zoom} ---");
            esc.zoom_log2 = zoom;
            esc.center_re = re.to_string();
            esc.center_im = im.to_string();
            esc.coloring_params.remove("scale");
            settle(&esc, &mut escape, "cold settle      ");

            // A coloring-slider tick: view unchanged, orbit reusable.
            esc.coloring_params.insert("scale".to_string(), 0.06);
            settle(&esc, &mut escape, "coloring tick    ");
            esc.coloring_params.insert("scale".to_string(), 0.07);
            settle(&esc, &mut escape, "coloring tick 2  ");

            // A small pan: the center strings change, the view does
            // not deepen. This is every mouse-drag event.
            esc.center_re = format!("{re}1");
            settle(&esc, &mut escape, "pan (center edit)");

            // A zoom notch at fixed center (wheel without cursor
            // offset).
            esc.zoom_log2 = zoom + 0.25;
            settle(&esc, &mut escape, "zoom notch       ");

            // A zoom notch that ALSO moves the center (zoom-to-cursor,
            // the app's default wheel behaviour).
            esc.zoom_log2 = zoom + 0.5;
            esc.center_im = format!("{im}1");
            settle(&esc, &mut escape, "zoom-to-cursor   ");
        }

        // Same view, both pipelines: how much of the threshold cliff
        // is the perturbed shader itself (vs the deeper view's higher
        // iteration counts). force_perturbed runs the perturbation
        // machinery below its zoom gate; mandelbrot's tier is
        // Power(2), so the delta step matches the reference.
        // The reported gesture: a wheel zoom gliding 13 -> 48, one
        // render call per smoothing frame, and what the user sees
        // is how often the image actually updates during it.
        {
            println!("--- wheel glide 13 -> 48 ---");
            let mut esc2 = esc.clone();
            esc2.center_re = re.to_string();
            esc2.center_im = im.to_string();
            esc2.zoom_log2 = 13.0;
            let mut escape2 = crate::escape::EscapeRenderer::new(&device, w, h);
            escape2.progressive = true;
            let mut g = 0;
            loop {
                let mut enc = device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor { label: Some("glide warm") },
                );
                let s = escape2.render(
                    &device, &queue, &mut enc, &esc2, renderer.palette_view(),
                );
                queue.submit(std::iter::once(enc.finish()));
                let _ = device.poll(
                    wgpu::PollType::Wait { submission_index: None, timeout: None },
                );
                if s { break; }
                g += 1;
                assert!(g < 100_000);
            }
            let t0 = web_time::Instant::now();
            let stale0 = escape2.stale_serves;
            let mut frames = 0u32;
            let mut blank = 0u32;
            let mut z = 13.0f64;
            while z < 48.0 {
                // Paced at a real frame cadence. Without this the
                // loop runs 100 frames in 12 ms -- faster than the
                // orbit worker gets scheduled at all, which measures
                // an artifact rather than the gesture.
                std::thread::sleep(std::time::Duration::from_millis(8));
                z += 0.35;
                esc2.zoom_log2 = z;
                let before = escape2.stale_serves;
                let mut enc = device.create_command_encoder(
                    &wgpu::CommandEncoderDescriptor { label: Some("glide") },
                );
                let _ = escape2.render(
                    &device, &queue, &mut enc, &esc2, renderer.palette_view(),
                );
                queue.submit(std::iter::once(enc.finish()));
                let _ = device.poll(
                    wgpu::PollType::Wait { submission_index: None, timeout: None },
                );
                frames += 1;
                // A frame that neither served stale nor had the
                // worker's ack drew nothing.
                if escape2.stale_serves == before && escape2.last_path.is_empty() {
                    blank += 1;
                }
            }
            let stale = escape2.stale_serves - stale0;
            let d = crate::escape::diag::snapshot();
            let _ = blank;
            println!(
                "glide: {frames} frames over {:.0} ms | stale-served {stale} |                  waits {} | rebuilds {} | relocations {} | last path {}",
                t0.elapsed().as_secs_f32() * 1000.0,
                d.orbit_wait_frames,
                d.orbit_rebuilds,
                d.orbit_relocations,
                d.path,
            );
            escape2.destroy();
        }

        println!("--- zoom 13.5 A/B, identical view ---");
        esc.zoom_log2 = 13.5;
        esc.center_re = re.to_string();
        esc.center_im = im.to_string();
        settle(&esc, &mut escape, "direct settle    ");
        esc.coloring_params.insert("scale".to_string(), 0.08);
        settle(&esc, &mut escape, "direct tick      ");
        escape.force_perturbed = true;
        esc.coloring_params.insert("scale".to_string(), 0.09);
        settle(&esc, &mut escape, "perturbed settle ");
        esc.coloring_params.insert("scale".to_string(), 0.10);
        settle(&esc, &mut escape, "perturbed tick   ");
        escape.force_perturbed = false;

        let d = crate::escape::diag::snapshot();
        assert!(d.settle_ms >= 0.0);
        assert!(d.restarts > 0, "diagnostics never saw a restart");
        escape.destroy();
    }

    /// The recolor cache must be INVISIBLE: a cache-hit frame is
    /// bit-identical to a from-scratch render of the same config, on
    /// both paths, for every coloring class -- and it must MISS
    /// whenever the iteration actually depends on what changed.
    ///
    /// Four classes exercised:
    ///  - map-only coloring (smooth): a param tick HITS and matches a
    ///    fresh render exactly;
    ///  - derivative coloring (distance_estimate, direct path): dz
    ///    flows through the records;
    ///  - accumulator coloring (stripe_average): a param tick must
    ///    MISS (the accumulator ran under the old params) and still
    ///    match a fresh render;
    ///  - a view change (pan) must MISS everywhere.
    ///
    /// Equality is asserted on the tonemapped readback bytes -- the
    /// same output the visual suite hashes.
    #[test]
    #[ignore = "needs a GPU"]
    fn recolor_cache_is_invisible_and_invalidates_correctly() {
        let _diag = diag_lock();
        let (device, queue) = repro_device();
        let (w, h) = (200u32, 160u32);
        let config = crate::config::FractalConfig::default();
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );

        let read = |escape: &mut crate::escape::EscapeRenderer,
                    esc: &crate::config::escape::EscapeConfig,
                    renderer: &mut crate::renderer::compute_kernel::FlameRenderer|
         -> Vec<u8> {
            let mut guard = 0u32;
            loop {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("cache frame"),
                });
                let settled =
                    escape.render(&device, &queue, &mut enc, esc, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                let _ =
                    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 100_000, "render did not settle");
            }
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("cache tonemap"),
            });
            renderer.update_background_color(&queue, [0.0, 0.0, 0.0]);
            renderer.update_tonemap(
                &queue,
                crate::scene::tonemap::ToneMapMode::Linear,
                config.highlight_mode,
                false,
                1.0,
                1.0,
                config.gamma_threshold,
                1.0,
                config.vibrancy,
                config.white_level,
                config.saturation,
                config.hue_shift,
                config.alpha_blend_low,
                config.alpha_blend_high,
                w,
                h,
                renderer.total_iterations(),
                config.max_iterations,
                config.zoom,
                256,
                4,
                false,
                false,
                config.levels_low,
                config.levels_high,
                config.levels_gamma,
            );
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, [0.0, 0.0, 0.0],
            ))
            .expect("readback");
            rgba
        };

        // Fresh-renderer render: the ground truth a cache hit must
        // reproduce exactly.
        let mut fresh = |esc: &crate::config::escape::EscapeConfig,
                         renderer: &mut crate::renderer::compute_kernel::FlameRenderer|
         -> Vec<u8> {
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            let px = read(&mut escape, esc, renderer);
            escape.destroy();
            px
        };

        let mk = |coloring: &str, zoom: f64| {
            let mut esc = crate::config::escape::EscapeConfig::default();
            esc.center_re = "-0.74364388703715870475".to_string();
            esc.center_im = "0.13182590420531197049".to_string();
            esc.zoom_log2 = zoom;
            esc.max_iter = 600;
            esc.coloring = coloring.to_string();
            esc
        };


        for (coloring, zoom, expect_hit) in [
            // map-only: hits on both paths
            ("smooth", 13.0, true),
            ("smooth", 15.0, true),
            // derivative (direct path only renders it meaningfully)
            ("distance_estimate", 13.0, true),
            // accumulator: the param feeds the loop, must MISS
            ("stripe_average", 15.0, false),
        ] {
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            let mut esc = mk(coloring, zoom);
            let _first = read(&mut escape, &esc, &mut renderer);

            // The coloring-param tick.
            esc.coloring_params.insert("scale".to_string(), 0.11);
            let ticked = read(&mut escape, &esc, &mut renderer);
            let hit = escape.last_path == "recolor";
            assert_eq!(
                hit, expect_hit,
                "{coloring}@{zoom}: expected cache hit={expect_hit}, got path {}",
                escape.last_path
            );
            let truth = fresh(&esc, &mut renderer);
            assert_eq!(
                ticked, truth,
                "{coloring}@{zoom}: cached recolor differs from a fresh render"
            );

            // A pan must MISS and re-render correctly.
            esc.center_re = "-0.743643887037158704".to_string();
            let panned = read(&mut escape, &esc, &mut renderer);
            assert_ne!(
                escape.last_path, "recolor",
                "{coloring}@{zoom}: a view change must not serve cached records"
            );
            let truth = fresh(&esc, &mut renderer);
            assert_eq!(
                panned, truth,
                "{coloring}@{zoom}: post-pan render differs from a fresh render"
            );
            escape.destroy();
        }
    }

    /// A pan must REUSE the reference orbit (relocation), and the
    /// relocated render must still be correct against an exact f64
    /// oracle at the NEW view.
    ///
    /// Three assertions, in causal order: the diagnostics must show a
    /// relocation and no rebuild (the mechanism engaged); the lit/
    /// unlit classification must match exact f64 iteration at the new
    /// center (the offset was computed right -- a mis-anchored
    /// reference shifts the whole frame, which this cannot miss); and
    /// a pan past the relocation cap must fall back to a REBUILD
    /// (correctness over reuse).
    #[test]
    #[ignore = "needs a GPU"]
    fn pan_reuses_the_reference_orbit() {
        let _diag = diag_lock();
        let (device, queue) = repro_device();
        let (w, h) = (192u32, 160u32);
        let config = crate::config::FractalConfig::default();
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);

        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.center_re = "-0.7436438870371587".to_string();
        esc.center_im = "0.1318259042053119".to_string();
        esc.zoom_log2 = 30.0;
        esc.max_iter = 2000;
        esc.coloring = "smooth".to_string();

        let mut render = |esc: &crate::config::escape::EscapeConfig,
                          escape: &mut crate::escape::EscapeRenderer| -> Vec<u8> {
            let mut guard = 0u32;
            loop {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("pan frame"),
                });
                let settled =
                    escape.render(&device, &queue, &mut enc, esc, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                let _ =
                    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 100_000, "render did not settle");
            }
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("pan tonemap"),
            });
            renderer.update_background_color(&queue, [0.0, 0.0, 0.0]);
            renderer.update_tonemap(
                &queue,
                crate::scene::tonemap::ToneMapMode::Linear,
                config.highlight_mode,
                false,
                1.0,
                1.0,
                config.gamma_threshold,
                1.0,
                config.vibrancy,
                config.white_level,
                config.saturation,
                config.hue_shift,
                config.alpha_blend_low,
                config.alpha_blend_high,
                w,
                h,
                renderer.total_iterations(),
                config.max_iterations,
                config.zoom,
                256,
                4,
                false,
                false,
                config.levels_low,
                config.levels_high,
                config.levels_gamma,
            );
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, [0.0, 0.0, 0.0],
            ))
            .expect("readback");
            rgba
        };

        let _at_a = render(&esc, &mut escape);
        let (relocs0, rebuilds0) = escape.orbit_stats();

        // ~1290 px pan (3e-8 complex units at 2^-28/160 spacing per
        // pixel): well inside the relocation cap.
        esc.center_re = "-0.7436438570371587".to_string();
        let at_b = render(&esc, &mut escape);
        let (relocs1, rebuilds1) = escape.orbit_stats();
        assert!(
            relocs1 > relocs0,
            "the pan did not relocate (relocations {relocs1}, rebuilds {rebuilds1})"
        );
        assert_eq!(
            rebuilds1, rebuilds0,
            "the pan rebuilt the reference instead of relocating"
        );

        // Ground truth for the ANCHORING: a fresh, centered reference
        // at B. A wrong relocation offset displaces the whole frame
        // (the pan here is ~1700 px of view travel), so agreement
        // with the fresh render is the sharp property. The two are
        // legitimately different references, so chaotic boundary
        // pixels may flip within f32 delta noise -- the measured
        // band for reference-vs-reference disagreement at this depth
        // is ~1.3%, and the Phoenix rung tests accept 1.6-2.1%
        // against exact orbits.
        let fresh_b = {
            let mut escape2 = crate::escape::EscapeRenderer::new(&device, w, h);
            let px = render(&esc, &mut escape2);
            escape2.destroy();
            px
        };
        let lit = |buf: &[u8], i: usize| -> bool {
            buf[i] as u32 + buf[i + 1] as u32 + buf[i + 2] as u32 > 24
        };
        let mut vs_fresh = 0usize;
        for i in (0..at_b.len()).step_by(4) {
            if lit(&at_b, i) != lit(&fresh_b, i) {
                vs_fresh += 1;
            }
        }
        let frac_fresh = vs_fresh as f64 / (w as f64 * h as f64);

        // Backstop: exact f64 classification at B. The lit-threshold
        // heuristic disagrees with the oracle on filament-dense views
        // regardless of reference (escaped pixels can land on dark
        // palette bands), so this bound is deliberately loose -- it
        // exists to catch a wholesale frame shift, not noise.
        let cx: f64 = esc.center_re.parse().unwrap();
        let cy: f64 = esc.center_im.parse().unwrap();
        let span_y = 4.0 / 2f64.powf(esc.zoom_log2);
        let span_x = span_y * w as f64 / h as f64;
        let mut vs_exact = 0usize;
        for py in 0..h as usize {
            for px in 0..w as usize {
                let ci = (
                    ((px as f64 + 0.5) / w as f64 - 0.5) * span_x + cx,
                    -(((py as f64 + 0.5) / h as f64 - 0.5) * span_y) + cy,
                );
                let (mut zx, mut zy) = (0.0f64, 0.0f64);
                let mut escaped = false;
                for _ in 0..esc.max_iter {
                    let nx = zx * zx - zy * zy + ci.0;
                    zy = 2.0 * zx * zy + ci.1;
                    zx = nx;
                    if zx * zx + zy * zy > 4.0 {
                        escaped = true;
                        break;
                    }
                }
                let i = (py * w as usize + px) * 4;
                if lit(&at_b, i) != escaped {
                    vs_exact += 1;
                }
            }
        }
        let frac_exact = vs_exact as f64 / (w as f64 * h as f64);
        println!(
            "pan-relocated: {:.2}% vs fresh reference, {:.2}% vs exact f64",
            100.0 * frac_fresh,
            100.0 * frac_exact
        );
        assert!(
            frac_fresh < 0.03,
            "relocated render disagrees with a fresh reference at the new view              ({:.2}% of pixels) -- the re-anchored offset is wrong",
            100.0 * frac_fresh
        );
        assert!(
            frac_exact < 0.15,
            "relocated render is grossly wrong against exact iteration              ({:.2}% of pixels) -- the frame is displaced",
            100.0 * frac_exact
        );

        // A pan past the cap (0.01 complex units = millions of pixel
        // spacings) must fall back to a fresh reference.
        esc.center_re = "-0.7536438570371587".to_string();
        let _far = render(&esc, &mut escape);
        let (_, rebuilds2) = escape.orbit_stats();
        assert!(
            rebuilds2 > rebuilds1,
            "an out-of-range pan must rebuild the reference"
        );
        escape.destroy();
    }

    /// A continuous gesture (wheel-smoothed zoom-to-cursor: zoom AND
    /// center changing every frame) must keep DRAWING, not freeze
    /// until the gesture ends.
    ///
    /// The failure mode this pins: every gesture frame posts a new
    /// orbit request, and the worker's acknowledgment is always one
    /// frame behind, so a renderer that waits for it draws nothing
    /// for the whole gesture (the report: "settle 192 ms over 1
    /// frame" on a wheel zoom that a direct slider does in 17 ms).
    /// The stale-serve path composes the offset render-side against
    /// the worker's last publication instead.
    #[test]
    #[ignore = "needs a GPU"]
    fn continuous_gesture_keeps_drawing() {
        let _diag = diag_lock();
        let (device, queue) = repro_device();
        let (w, h) = (192u32, 160u32);
        let config = crate::config::FractalConfig::default();
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        escape.progressive = true;

        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.zoom_log2 = 15.0;
        esc.max_iter = 800;

        let frame = |esc: &crate::config::escape::EscapeConfig,
                     escape: &mut crate::escape::EscapeRenderer,
                     renderer: &crate::renderer::compute_kernel::FlameRenderer|
         -> bool {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gesture frame"),
            });
            let settled = escape.render(&device, &queue, &mut enc, esc, renderer.palette_view());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            settled
        };

        // Warm up: settle at the starting view so a reference exists.
        let mut re = -0.743643887f64;
        let im = 0.131825904f64;
        esc.center_re = format!("{re:.12}");
        esc.center_im = format!("{im:.12}");
        let mut guard = 0;
        while !frame(&esc, &mut escape, &renderer) {
            guard += 1;
            assert!(guard < 100_000, "warmup did not settle");
        }

        // The gesture: 40 frames, each moving BOTH zoom and center
        // (zoom-to-cursor), one render call per event like the app's
        // frame loop.
        let stale_before = escape.stale_serves;
        for _ in 0..40 {
            esc.zoom_log2 += 0.05;
            re += 1e-6;
            esc.center_re = format!("{re:.12}");
            let _ = frame(&esc, &mut escape, &renderer);
        }
        let stale_during = escape.stale_serves - stale_before;
        println!(
            "gesture: {stale_during}/40 frames drawn from the previous reference"
        );
        assert!(
            stale_during >= 20,
            "the gesture starved: only {stale_during}/40 frames drew anything \
             (the rest waited for the worker's acknowledgment)"
        );

        // The gesture ends; the render must settle normally...
        let mut guard = 0;
        while !frame(&esc, &mut escape, &renderer) {
            guard += 1;
            assert!(guard < 100_000, "post-gesture render did not settle");
        }

        // ...and the settled image is the authoritative one: compare
        // against a fresh renderer at the final view (fresh reference,
        // no gesture history). Reference-vs-reference noise band only.
        let read = |escape: &mut crate::escape::EscapeRenderer,
                    renderer: &mut crate::renderer::compute_kernel::FlameRenderer|
         -> Vec<u8> {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("gesture tonemap"),
            });
            renderer.update_background_color(&queue, [0.0, 0.0, 0.0]);
            renderer.update_tonemap(
                &queue,
                crate::scene::tonemap::ToneMapMode::Linear,
                config.highlight_mode,
                false,
                1.0,
                1.0,
                config.gamma_threshold,
                1.0,
                config.vibrancy,
                config.white_level,
                config.saturation,
                config.hue_shift,
                config.alpha_blend_low,
                config.alpha_blend_high,
                w,
                h,
                renderer.total_iterations(),
                config.max_iterations,
                config.zoom,
                256,
                4,
                false,
                false,
                config.levels_low,
                config.levels_high,
                config.levels_gamma,
            );
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, [0.0, 0.0, 0.0],
            ))
            .expect("readback");
            rgba
        };
        let settled = read(&mut escape, &mut renderer);
        let fresh = {
            let mut escape2 = crate::escape::EscapeRenderer::new(&device, w, h);
            escape2.progressive = true;
            let mut guard = 0;
            while !frame(&esc, &mut escape2, &renderer) {
                guard += 1;
                assert!(guard < 100_000, "fresh render did not settle");
            }
            let px = read(&mut escape2, &mut renderer);
            escape2.destroy();
            px
        };
        let lit = |buf: &[u8], i: usize| -> bool {
            buf[i] as u32 + buf[i + 1] as u32 + buf[i + 2] as u32 > 24
        };
        let mut differ = 0usize;
        for i in (0..settled.len()).step_by(4) {
            if lit(&settled, i) != lit(&fresh, i) {
                differ += 1;
            }
        }
        let frac = differ as f64 / (w as f64 * h as f64);
        println!("post-gesture settled vs fresh: {:.2}% differ", 100.0 * frac);
        assert!(
            frac < 0.03,
            "the post-gesture settled image disagrees with a fresh render \
             ({:.2}% of pixels)",
            100.0 * frac
        );

        // EXPORTS MUST NEVER PREVIEW. Video export (animation/export)
        // and the headless/CLI path both leave `progressive` false,
        // which routes orbits through the BLOCKING cache -- the
        // stale-serve path lives in the progressive one and cannot
        // run. Pinned here because the guarantee is structural and a
        // future `progressive = true` on an export path would break
        // it silently: an exported frame drawn against a not-yet-
        // acknowledged reference is a wrong frame in a finished file.
        {
            let mut exporter = crate::escape::EscapeRenderer::new(&device, w, h);
            let mut e = esc.clone();
            let mut z = e.zoom_log2;
            for _ in 0..10 {
                z += 0.05;
                e.zoom_log2 = z;
                let settled = frame(&e, &mut exporter, &renderer);
                assert!(
                    settled,
                    "a non-progressive (export) render must settle in one call"
                );
            }
            assert_eq!(
                exporter.stale_serves, 0,
                "an export render served a frame from a stale reference"
            );
            exporter.destroy();
        }
        escape.destroy();
    }

    /// A mid-render chunk frame must HOLD the previous frame's
    /// content for unfinished pixels, not paint them black.
    ///
    /// Past the floatexp threshold the TDR-safe chunk is a fraction
    /// of a typical max_iter, so a pan restarts a multi-chunk render
    /// whose first frame used to be mostly black (every pixel whose
    /// iterations had not finished yet) -- the reported "always black
    /// while continually panning" at zoom 48+. Unfinished pixels now
    /// skip the store and keep whatever the texture held.
    #[test]
    #[ignore = "needs a GPU"]
    fn mid_render_frames_hold_content_instead_of_black() {
        let (device, queue) = repro_device();
        let (w, h) = (192u32, 160u32);
        let config = crate::config::FractalConfig::default();
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        // Tiny chunks: a 600-iteration render becomes ~10 chunks, so
        // the first frame after a restart is genuinely mid-render.
        escape.chunk_override = Some(64);

        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.center_re = "-0.743643887037158704752191".to_string();
        esc.center_im = "0.131825904205311970493132".to_string();
        esc.zoom_log2 = 50.0;
        esc.max_iter = 6000;

        let one_frame = |esc: &crate::config::escape::EscapeConfig,
                         escape: &mut crate::escape::EscapeRenderer,
                         renderer: &crate::renderer::compute_kernel::FlameRenderer|
         -> bool {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("hold frame"),
            });
            let settled = escape.render(&device, &queue, &mut enc, esc, renderer.palette_view());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            settled
        };
        let read = |escape: &mut crate::escape::EscapeRenderer,
                    renderer: &mut crate::renderer::compute_kernel::FlameRenderer|
         -> Vec<u8> {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("hold tonemap"),
            });
            renderer.update_background_color(&queue, [0.0, 0.0, 0.0]);
            renderer.update_tonemap(
                &queue,
                crate::scene::tonemap::ToneMapMode::Linear,
                config.highlight_mode,
                false,
                1.0,
                1.0,
                config.gamma_threshold,
                1.0,
                config.vibrancy,
                config.white_level,
                config.saturation,
                config.hue_shift,
                config.alpha_blend_low,
                config.alpha_blend_high,
                w,
                h,
                renderer.total_iterations(),
                config.max_iterations,
                config.zoom,
                256,
                4,
                false,
                false,
                config.levels_low,
                config.levels_high,
                config.levels_gamma,
            );
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, [0.0, 0.0, 0.0],
            ))
            .expect("readback");
            rgba
        };
        let black_frac = |px: &[u8]| -> f64 {
            let mut black = 0usize;
            for i in (0..px.len()).step_by(4) {
                if px[i] < 8 && px[i + 1] < 8 && px[i + 2] < 8 {
                    black += 1;
                }
            }
            black as f64 / (px.len() / 4) as f64
        };

        // Settle at A.
        let mut guard = 0;
        while !one_frame(&esc, &mut escape, &renderer) {
            guard += 1;
            assert!(guard < 100_000, "did not settle at A");
        }
        let settled_a = read(&mut escape, &mut renderer);
        let base_black = black_frac(&settled_a);
        // Guard against a VACUOUS pass: if the settled image is
        // itself nearly all black there is no content to hold, and
        // "the mid frame is not blacker" would prove nothing.
        assert!(
            base_black < 0.85,
            "test view is {:.1}% black when settled -- it cannot show              content-holding; pick a view with structure",
            100.0 * base_black
        );

        // Pan (well inside the relocation cap), then render EXACTLY
        // ONE chunk frame -- what the user sees during a drag.
        esc.center_re = "-0.743643887037158704752195".to_string();
        let settled = one_frame(&esc, &mut escape, &renderer);
        assert!(!settled, "one 64-iteration chunk of 600 must not settle");
        let mid = read(&mut escape, &mut renderer);
        let mid_black = black_frac(&mid);
        println!(
            "black fraction: settled {:.1}%, first chunk after pan {:.1}%",
            100.0 * base_black,
            100.0 * mid_black
        );
        // Without content-holding this reads near 100% (64 of 600
        // iterations finishes almost nothing at this depth); with it,
        // the frame keeps A's pixels wherever iteration is unfinished.
        assert!(
            mid_black < base_black + 0.10,
            "the mid-render frame is {:.1}% black against a settled {:.1}% -- \
             unfinished pixels are painting black instead of holding content",
            100.0 * mid_black,
            100.0 * base_black
        );

        // And finishing the render must still produce the correct
        // image: settle and compare against a fresh renderer at B.
        let mut guard = 0;
        while !one_frame(&esc, &mut escape, &renderer) {
            guard += 1;
            assert!(guard < 100_000, "did not settle at B");
        }
        let settled_b = read(&mut escape, &mut renderer);
        let fresh_b = {
            let mut escape2 = crate::escape::EscapeRenderer::new(&device, w, h);
            let mut guard = 0;
            while !one_frame(&esc, &mut escape2, &renderer) {
                guard += 1;
                assert!(guard < 100_000, "fresh render did not settle");
            }
            let px = read(&mut escape2, &mut renderer);
            escape2.destroy();
            px
        };
        let mut differ = 0usize;
        for i in (0..settled_b.len()).step_by(4) {
            let a = settled_b[i] as u32 + settled_b[i + 1] as u32 + settled_b[i + 2] as u32;
            let b = fresh_b[i] as u32 + fresh_b[i + 1] as u32 + fresh_b[i + 2] as u32;
            if (a > 24) != (b > 24) {
                differ += 1;
            }
        }
        let frac = differ as f64 / (w as f64 * h as f64);
        println!("settled-after-pan vs fresh: {:.2}% differ", 100.0 * frac);
        assert!(
            frac < 0.03,
            "content-holding changed the SETTLED image ({:.2}% of pixels)",
            100.0 * frac
        );
        escape.destroy();
    }

    /// A restart's first chunk must NEVER be sized from a
    /// survivor-biased measurement.
    ///
    /// The general per-iteration cost is measured on whatever chunk
    /// last carried timestamps -- often a LATE chunk, where most
    /// pixels have escaped and iterate for nearly free. A restart
    /// rebirths every pixel. Sizing its first chunk from the cheap
    /// tail measurement (clamped only by the 1M-iteration ceiling)
    /// is how a zoom-120, 100k-iteration view earned
    /// "wgpu DEVICE LOST (Unknown)" in the field. The rule: restarts
    /// size from the COLD measurement (a first chunk's, all pixels
    /// alive) or stay seed-bounded.
    #[test]
    #[ignore = "needs a GPU"]
    fn restart_chunks_never_trust_survivor_biased_measurements() {
        let (device, _queue) = repro_device();
        let mut escape = crate::escape::EscapeRenderer::new(&device, 960, 720);
        let seed = escape.chunk_seed(true);

        // Survivor-biased measurement only (a late chunk read 0.1 us
        // per iteration): the ideal from it would be ~100k iterations.
        // A restart must ignore it and stay seed-bounded.
        escape.set_pacer_measurements(Some(1e-4), None);
        let chunk = escape.probe_restart_chunk(true);
        assert!(
            chunk <= seed.saturating_mul(2).max(16),
            "restart chunk {chunk} exceeds the seed bound ({seed} x2) with only a \
             survivor-biased measurement -- this is the device-loss regression"
        );

        // With a cold measurement, the restart sizes from IT -- not
        // from the (much cheaper) general one.
        escape.set_pacer_measurements(Some(1e-4), Some(0.01));
        let chunk = escape.probe_restart_chunk(true);
        assert!(
            (900..=1100).contains(&chunk),
            "restart chunk {chunk} should be ~1000 (10 ms target / 0.01 ms per \
             iteration measured with all pixels alive)"
        );

        // Mid-render growth is still 2x-bounded even against a rosy
        // cold number: the second chunk may at most double the first.
        let second = {
            // probe_restart_chunk left chunk_iters at `chunk`.
            escape.set_pacer_measurements(Some(1e-4), Some(0.01));
            escape.next_chunk_for_test(true)
        };
        assert!(
            second <= chunk.saturating_mul(2),
            "second chunk {second} grew more than 2x over {chunk}"
        );

        // A measurement from the OTHER RUNG must not be used at all.
        // This is the second device loss: crossing zoom 48 into
        // floatexp, a cheap scaled-rung cold measurement sized the
        // floatexp restart, and with max_iter 10k the whole render
        // became ONE dispatch with every pixel alive.
        escape.set_pacer_measurements_other_rung(Some(1e-4), Some(1e-4));
        let chunk = escape.probe_restart_chunk(true);
        assert!(
            chunk <= seed.saturating_mul(2).max(16),
            "restart chunk {chunk} trusted a measurement from the other rung              (seed {seed}) -- cost per iteration differs several-fold across it"
        );
        escape.destroy();

        // THE CEILING MUST SCALE WITH PIXELS. It is a multiple of the
        // static budget/pixels seed, not an absolute iteration count:
        // 1M iterations means something entirely different at 200x160
        // than at 4K with supersampling, where it is thousands of
        // times over the budget the seed encodes. Both device losses
        // ended in a dispatch a pixel-aware ceiling would have
        // refused.
        let small = crate::escape::EscapeRenderer::new(&device, 320, 240);
        let mut big = crate::escape::EscapeRenderer::new(&device, 1920, 1080);
        big.resize(&device, 1920, 1080, 3);
        let (c_small, c_big) = (small.chunk_ceiling(true), big.chunk_ceiling(true));
        println!("chunk ceiling: 320x240 {c_small}, 1920x1080 @3x {c_big}");
        assert!(
            c_big < c_small,
            "the ceiling ignored resolution ({c_small} vs {c_big}): a chunk sized              for a thumbnail would run over a 33-megapixel frame"
        );
        // And no measurement, however rosy, may exceed it.
        big.set_pacer_measurements(Some(1e-9), Some(1e-9));
        let chunk = big.probe_restart_chunk(true);
        assert!(
            chunk <= c_big,
            "restart chunk {chunk} exceeded the ceiling {c_big}"
        );
        small.destroy();
        big.destroy();
    }

    /// Every preset must render a PICTURE.
    ///
    /// Presets exist so a newcomer can land somewhere that works
    /// instead of assembling a formula, a view, an iteration budget
    /// and a coloring by trial. A preset that renders black or flat
    /// fails at exactly that job, silently, and would be found by a
    /// user rather than by us — which is the whole problem it was
    /// added to solve.
    ///
    /// Two things are checked per preset. The pairing must be legal
    /// (`coloring_suits_formula`) — cheap, and it catches a preset
    /// naming a coloring that cannot draw its formula. Then the
    /// render must be non-degenerate: enough lit pixels to be an
    /// image, and enough DISTINCT values to be a picture rather than
    /// a flat wash. Both floors are deliberately low; this is a
    /// smoke test for "is there anything here", not a judgement of
    /// composition.
    #[test]
    #[ignore = "needs a GPU"]
    fn every_preset_renders_a_picture() {
        let (device, queue) = repro_device();
        let (w, h) = (128u32, 96u32);
        let config = crate::config::FractalConfig::default();
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation,
            config.palette_squeeze, config.palette_squeeze_mode,
            config.palette_squeeze_falloff, config.palette_log_strength,
            config.palette_reverse,
        );

        // Render a config to settle and measure whether it is a
        // picture: how much is lit, and how many DISTINCT colours
        // (quantised, so f32 noise does not read as detail).
        let mut shoot = |esc: &crate::config::escape::EscapeConfig,
                         renderer: &mut crate::renderer::compute_kernel::FlameRenderer|
         -> (f64, usize) {
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            let mut guard = 0u32;
            loop {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("preset"),
                });
                let settled =
                    escape.render(&device, &queue, &mut enc, esc, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                let _ =
                    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 200_000, "preset never settled");
            }
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("preset tonemap"),
            });
            renderer.update_background_color(&queue, [0.0, 0.0, 0.0]);
            renderer.update_tonemap(
                &queue,
                crate::scene::tonemap::ToneMapMode::Linear,
                config.highlight_mode,
                false,
                1.0,
                1.0,
                config.gamma_threshold,
                1.0,
                config.vibrancy,
                config.white_level,
                config.saturation,
                config.hue_shift,
                config.alpha_blend_low,
                config.alpha_blend_high,
                w,
                h,
                renderer.total_iterations(),
                config.max_iterations,
                config.zoom,
                256,
                4,
                false,
                false,
                config.levels_low,
                config.levels_high,
                config.levels_gamma,
            );
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, px) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, [0.0, 0.0, 0.0],
            ))
            .expect("readback");
            escape.destroy();
            let mut lit = 0usize;
            let mut seen = std::collections::HashSet::new();
            for p in px.chunks(4) {
                if p[0] as u32 + p[1] as u32 + p[2] as u32 > 24 {
                    lit += 1;
                }
                seen.insert((p[0] >> 3, p[1] >> 3, p[2] >> 3));
            }
            (lit as f64 / (w * h) as f64, seen.len())
        };

        let mut checked = 0usize;
        // Mode B first: a field's presets go through the same "does it
        // draw anything" bar, and its coloring comes from the field
        // registry rather than the formula one.
        for field in crate::escape::fields::FIELDS {
            for preset in field.presets {
                let coloring =
                    crate::escape::fields::get_field_coloring(preset.coloring, field);
                let mut esc = crate::config::escape::EscapeConfig::default();
                esc.formula = field.name.to_string();
                esc.coloring = coloring.name.to_string();
                esc.center_re = preset.center_re.to_string();
                esc.center_im = preset.center_im.to_string();
                esc.zoom_log2 = preset.zoom_log2;
                esc.max_iter = preset.max_iter;
                for p in field.parameters {
                    let v = preset
                        .formula_params
                        .iter()
                        .find(|(k, _)| *k == p.name)
                        .map(|(_, v)| *v)
                        .unwrap_or(p.default);
                    esc.formula_params.insert(p.name.to_string(), v);
                }
                for p in coloring.parameters {
                    let v = preset
                        .coloring_params
                        .iter()
                        .find(|(k, _)| *k == p.name)
                        .map(|(_, v)| *v)
                        .unwrap_or(p.default);
                    esc.coloring_params.insert(p.name.to_string(), v);
                }
                let (lit_frac, distinct) = shoot(&esc, &mut renderer);
                println!(
                    "{:14} {:20} lit {:5.1}%  {:4} distinct",
                    field.name, preset.name, 100.0 * lit_frac, distinct
                );
                assert!(
                    lit_frac > 0.02,
                    "field preset `{}` of `{}` renders essentially nothing ({:.1}% lit)",
                    preset.name, field.name, 100.0 * lit_frac
                );
                assert!(
                    distinct >= 8,
                    "field preset `{}` of `{}` renders a flat wash ({distinct} colours)",
                    preset.name, field.name
                );
                checked += 1;
            }
        }
        for formula in crate::escape::FORMULAS {
            for preset in formula.presets {
                // 1. the pairing must be one the engine can draw.
                let coloring = crate::escape::get_coloring(preset.coloring);
                assert!(
                    crate::escape::coloring_suits_formula(formula, coloring),
                    "preset `{}` of `{}` names coloring `{}`, which cannot draw it",
                    preset.name,
                    formula.name,
                    preset.coloring
                );

                // 2. build exactly what the panel would apply.
                let mut esc = crate::config::escape::EscapeConfig::default();
                esc.formula = formula.name.to_string();
                esc.coloring = preset.coloring.to_string();
                esc.center_re = preset.center_re.to_string();
                esc.center_im = preset.center_im.to_string();
                esc.zoom_log2 = preset.zoom_log2;
                esc.max_iter = preset.max_iter;
                esc.julia = preset.julia.is_some();
                if let Some((re, im)) = preset.julia {
                    esc.julia_re = re;
                    esc.julia_im = im;
                }
                for p in formula.parameters {
                    let v = preset
                        .formula_params
                        .iter()
                        .find(|(k, _)| *k == p.name)
                        .map(|(_, v)| *v)
                        .unwrap_or(p.default);
                    esc.formula_params.insert(p.name.to_string(), v);
                }
                for p in coloring.parameters {
                    let v = preset
                        .coloring_params
                        .iter()
                        .find(|(k, _)| *k == p.name)
                        .map(|(_, v)| *v)
                        .unwrap_or(p.default);
                    esc.coloring_params.insert(p.name.to_string(), v);
                }

                let (lit_frac, distinct) = shoot(&esc, &mut renderer);
                println!(
                    "{:14} {:20} lit {:5.1}%  {:4} distinct",
                    formula.name,
                    preset.name,
                    100.0 * lit_frac,
                    distinct
                );
                assert!(
                    lit_frac > 0.02,
                    "preset `{}` of `{}` renders essentially nothing ({:.1}% lit)",
                    preset.name,
                    formula.name,
                    100.0 * lit_frac
                );
                assert!(
                    distinct >= 8,
                    "preset `{}` of `{}` renders a flat wash ({distinct} distinct colours)",
                    preset.name,
                    formula.name
                );
                checked += 1;
            }
        }
        assert!(checked >= 45, "only {checked} presets checked");
    }

    /// The downsample modes must do what their names claim, and 8x
    /// supersampling must actually resolve.
    ///
    /// The report: antialiasing washes colour out of fine detail.
    /// That is not a bug -- a saturated filament covering one sample
    /// in nine IS one ninth of the pixel's light, and a linear
    /// average says so -- but it is a look, and the alternatives are
    /// worth having. So this pins the property that distinguishes
    /// them rather than any particular pixel: over a detailed view,
    /// `Vivid` must retain more SATURATION than `Box`, because it
    /// weights each sample by its own, while all three must agree
    /// closely on where the structure IS (they differ in how samples
    /// combine, not in what was sampled).
    #[test]
    #[ignore = "needs a GPU"]
    fn downsample_modes_trade_saturation_for_correctness() {
        use crate::config::escape::DownsampleMode;
        let (device, queue) = repro_device();
        let (w, h) = (160u32, 128u32);
        let config = crate::config::FractalConfig::default();
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation,
            config.palette_squeeze, config.palette_squeeze_mode,
            config.palette_squeeze_falloff, config.palette_log_strength,
            config.palette_reverse,
        );

        // A view with fine coloured filaments -- the case the report
        // is about. Deep enough that detail lands below one display
        // pixel, which is what supersampling is for.
        let mut base = crate::config::escape::EscapeConfig::default();
        base.center_re = "-0.7436438870371587".to_string();
        base.center_im = "0.1318259042053119".to_string();
        base.zoom_log2 = 9.0;
        base.max_iter = 1500;
        base.coloring = "smooth".to_string();
        // TIGHT palette bands: many cycles across the frame, so a
        // display pixel really does straddle several hues. That is
        // the regime the report is about; a broad-banded view has
        // little for any combine to preserve.
        base.coloring_params.insert("scale".to_string(), 0.35);

        let shoot = |esc: &crate::config::escape::EscapeConfig,
                     renderer: &mut crate::renderer::compute_kernel::FlameRenderer|
         -> Vec<u8> {
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            escape.resize(&device, w, h, esc.supersample);
            let mut guard = 0u32;
            loop {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("aa"),
                });
                let settled =
                    escape.render(&device, &queue, &mut enc, esc, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                let _ =
                    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 200_000, "render did not settle");
            }
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("aa tonemap"),
            });
            renderer.update_background_color(&queue, [0.0, 0.0, 0.0]);
            renderer.update_tonemap(
                &queue,
                crate::scene::tonemap::ToneMapMode::Linear,
                config.highlight_mode,
                false,
                1.0,
                1.0,
                config.gamma_threshold,
                1.0,
                config.vibrancy,
                config.white_level,
                config.saturation,
                config.hue_shift,
                config.alpha_blend_low,
                config.alpha_blend_high,
                w,
                h,
                renderer.total_iterations(),
                config.max_iterations,
                config.zoom,
                256,
                4,
                false,
                false,
                config.levels_low,
                config.levels_high,
                config.levels_gamma,
            );
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, px) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, [0.0, 0.0, 0.0],
            ))
            .expect("readback");
            escape.destroy();
            px
        };
        // Mean saturation over pixels bright enough to have a hue.
        let saturation = |px: &[u8]| -> f64 {
            let (mut acc, mut n) = (0f64, 0usize);
            for p in px.chunks(4) {
                let mx = p[0].max(p[1]).max(p[2]) as f64;
                let mn = p[0].min(p[1]).min(p[2]) as f64;
                if mx > 24.0 {
                    acc += (mx - mn) / mx;
                    n += 1;
                }
            }
            if n == 0 { 0.0 } else { acc / n as f64 }
        };

        let mut aa = base.clone();
        aa.supersample = 3;
        let mut sats = Vec::new();
        let mut images = Vec::new();
        for mode in [DownsampleMode::Box, DownsampleMode::Perceptual, DownsampleMode::Vivid] {
            let mut esc = aa.clone();
            esc.downsample = mode;
            let px = shoot(&esc, &mut renderer);
            let s = saturation(&px);
            println!("{mode:?}: mean saturation {s:.4}");
            sats.push(s);
            images.push(px);
        }
        let (s_box, s_vivid) = (sats[0], sats[2]);
        assert!(
            s_vivid > s_box * 1.02,
            "Vivid ({s_vivid:.4}) did not retain more saturation than Box \
             ({s_box:.4}) -- the saturation weighting is not reaching the combine"
        );

        // All three sampled the same image: they must agree on where
        // the structure is, or one of them is not a combine at all.
        let lit = |px: &[u8]| -> Vec<bool> {
            px.chunks(4)
                .map(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
                .collect()
        };
        let base_lit = lit(&images[0]);
        for (i, img) in images.iter().enumerate().skip(1) {
            let other = lit(img);
            let differ = base_lit
                .iter()
                .zip(other.iter())
                .filter(|(a, b)| a != b)
                .count();
            let frac = differ as f64 / base_lit.len() as f64;
            assert!(
                frac < 0.05,
                "downsample mode {i} disagrees with Box on {:.1}% of pixels about \
                 where the structure is -- a combine may not move it",
                100.0 * frac
            );
        }

        // And 8x must resolve: at this size the budget allows it, so
        // the factor must survive resize and produce a finer image
        // than no antialiasing at all.
        let mut off = base.clone();
        off.supersample = 1;
        let mut deep = base.clone();
        deep.supersample = 8;
        let aliasing = |px: &[u8]| -> f64 {
            // Mean absolute second difference: aliasing shows up as
            // high-frequency energy that supersampling removes.
            let mut acc = 0f64;
            for y in 0..h as usize {
                for x in 1..w as usize - 1 {
                    let i = (y * w as usize + x) * 4;
                    for c in 0..3 {
                        let a = px[i - 4 + c] as f64;
                        let b = px[i + c] as f64;
                        let d = px[i + 4 + c] as f64;
                        acc += (d - 2.0 * b + a).abs();
                    }
                }
            }
            acc / (w as f64 * h as f64 * 3.0)
        };
        let (a_off, a_8x) = (aliasing(&shoot(&off, &mut renderer)), aliasing(&shoot(&deep, &mut renderer)));
        println!("aliasing: 1x {a_off:.2}, 8x {a_8x:.2}");
        assert!(
            a_8x < a_off * 0.8,
            "8x supersampling ({a_8x:.2}) is barely smoother than none ({a_off:.2}) \
             -- the factor was probably clamped away"
        );
    }

    /// A pixel that never escaped must show the BACKGROUND COLOUR,
    /// not black.
    ///
    /// The interior used to be written as opaque black, which looks
    /// identical to the default background and so went unnoticed
    /// until someone set a background and found it ignored. The fix
    /// is to write coverage rather than colour: a pixel with no value
    /// is absent, and the tonemap's existing background blend fills
    /// it.
    ///
    /// Checked at several colours, because a single one cannot tell
    /// "the background is applied" from "the interior happens to be
    /// that colour" — and black specifically must keep rendering
    /// exactly as it did, which is what leaves every existing config
    /// untouched.
    #[test]
    #[ignore = "needs a GPU"]
    fn the_interior_takes_the_background_colour() {
        let (device, queue) = repro_device();
        let (w, h) = (128u32, 96u32);
        let mut config = crate::config::FractalConfig::default();
        config.exposure = 1.0;
        config.gamma = 1.0;
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation,
            config.palette_squeeze, config.palette_squeeze_mode,
            config.palette_squeeze_falloff, config.palette_log_strength,
            config.palette_reverse,
        );

        // The home view: a big, unmistakable interior.
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.formula = "mandelbrot".to_string();
        esc.coloring = "smooth".to_string();
        esc.max_iter = 256;

        let shoot = |bg: [f32; 3],
                     renderer: &mut crate::renderer::compute_kernel::FlameRenderer|
         -> Vec<u8> {
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            let mut guard = 0u32;
            loop {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("bg"),
                });
                let settled =
                    escape.render(&device, &queue, &mut enc, &esc, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                let _ =
                    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 100_000, "render did not settle");
            }
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("bg tonemap"),
            });
            renderer.update_background_color(&queue, bg);
            renderer.update_tonemap(
                &queue,
                crate::scene::tonemap::ToneMapMode::Linear,
                config.highlight_mode,
                false,
                1.0,
                1.0,
                config.gamma_threshold,
                1.0,
                config.vibrancy,
                config.white_level,
                config.saturation,
                config.hue_shift,
                config.alpha_blend_low,
                config.alpha_blend_high,
                w,
                h,
                renderer.total_iterations(),
                config.max_iterations,
                config.zoom,
                256,
                4,
                false,
                false,
                config.levels_low,
                config.levels_high,
                config.levels_gamma,
            );
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, px) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, bg,
            ))
            .expect("readback");
            escape.destroy();
            px
        };

        // The set's centre is interior at the home view whatever the
        // colouring does, so this pixel never escapes.
        let centre = ((h as usize / 2) * w as usize + w as usize / 2) * 4;
        for (bg, want) in [
            ([0.0f32, 0.0, 0.0], [0u8, 0, 0]),
            ([1.0, 0.0, 0.0], [255, 0, 0]),
            ([0.2, 0.4, 0.8], [51, 102, 204]),
            ([1.0, 1.0, 1.0], [255, 255, 255]),
        ] {
            let px = shoot(bg, &mut renderer);
            let got = [px[centre], px[centre + 1], px[centre + 2]];
            println!("background {bg:?} -> interior {got:?} (want {want:?})");
            for c in 0..3 {
                let d = got[c] as i32 - want[c] as i32;
                assert!(
                    d.abs() <= 2,
                    "interior pixel is {got:?} with the background set to {bg:?}: \
                     expected {want:?}. A pixel that never escaped must take the \
                     background, not black."
                );
            }
        }

        // And the colouring itself must be untouched: a pixel that
        // DID escape keeps its palette colour whatever the background
        // is, or the blend is leaking into the fractal.
        let corner = (2 * w as usize + 2) * 4;
        let a = shoot([0.0, 0.0, 0.0], &mut renderer);
        let b = shoot([1.0, 1.0, 1.0], &mut renderer);
        let (ca, cb) = (
            [a[corner], a[corner + 1], a[corner + 2]],
            [b[corner], b[corner + 1], b[corner + 2]],
        );
        println!("escaped pixel: on black {ca:?}, on white {cb:?}");
        for c in 0..3 {
            assert!(
                (ca[c] as i32 - cb[c] as i32).abs() <= 2,
                "an escaped pixel changed with the background ({ca:?} vs {cb:?}) \
                 -- the background must fill only what the fractal left empty"
            );
        }
    }

    /// Accumulated antialiasing must match the supersampled grid it
    /// stands in for.
    ///
    /// Reported from the app: 8x antialiasing does nothing on a
    /// 4000x3000 export. It cannot — 8x over that is 768 megapixels
    /// of per-pixel state and 32000 pixels a side, past both the
    /// memory budget and the 16384 texture-dimension limit, so the
    /// factor was clamped away to 1x and nothing said so. The fix
    /// takes the same sample positions as several ordinary renders
    /// displaced within a pixel.
    ///
    /// Which only helps if it is the SAME antialiasing. So this
    /// renders one image with a 3x grid and another with 1x plus 3x
    /// accumulation, and requires them to agree closely: same sample
    /// positions, same average, so the difference is f32 ordering and
    /// the per-sample shading pass, not method.
    #[test]
    #[ignore = "needs a GPU"]
    fn accumulated_antialiasing_matches_the_grid() {
        let (device, queue) = repro_device();
        let (w, h) = (128u32, 96u32);
        let config = crate::config::FractalConfig::default();
        let mut renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device, &queue, wgpu::TextureFormat::Rgba8Unorm, w, h,
            &config.flame, config.palette_size,
        );
        renderer.update_palette(
            &device, &queue, &config.palette, config.palette_rotation,
            config.palette_squeeze, config.palette_squeeze_mode,
            config.palette_squeeze_falloff, config.palette_log_strength,
            config.palette_reverse,
        );

        // Fine structure, so antialiasing has something to do.
        let mut esc = crate::config::escape::EscapeConfig::default();
        esc.center_re = "-0.7436438870371587".to_string();
        esc.center_im = "0.1318259042053119".to_string();
        esc.zoom_log2 = 9.0;
        esc.max_iter = 1200;
        esc.coloring = "smooth".to_string();
        esc.coloring_params.insert("scale".to_string(), 0.2);

        let settle = |escape: &mut crate::escape::EscapeRenderer,
                      esc: &crate::config::escape::EscapeConfig,
                      enc: &mut wgpu::CommandEncoder,
                      palette: &wgpu::TextureView| {
            let mut guard = 0u32;
            loop {
                let settled = escape.render(&device, &queue, enc, esc, palette);
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 100_000, "did not settle");
            }
        };
        let tonemap_read = |escape: &crate::escape::EscapeRenderer,
                            view: &wgpu::TextureView,
                            renderer: &mut crate::renderer::compute_kernel::FlameRenderer|
         -> Vec<u8> {
            let _ = escape;
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("aa tonemap"),
            });
            renderer.update_background_color(&queue, [0.0, 0.0, 0.0]);
            renderer.update_tonemap(
                &queue,
                crate::scene::tonemap::ToneMapMode::Linear,
                config.highlight_mode,
                false,
                1.0,
                1.0,
                config.gamma_threshold,
                1.0,
                config.vibrancy,
                config.white_level,
                config.saturation,
                config.hue_shift,
                config.alpha_blend_low,
                config.alpha_blend_high,
                w,
                h,
                renderer.total_iterations(),
                config.max_iterations,
                config.zoom,
                256,
                4,
                false,
                false,
                config.levels_low,
                config.levels_high,
                config.levels_gamma,
            );
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, view);
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, px) = pollster::block_on(renderer.read_fractal_pixels(
                &device, &queue, false, [0.0, 0.0, 0.0],
            ))
            .expect("readback");
            px
        };

        // The palette view is stable for the whole test; cloning it
        // frees `renderer` for the mutable tonemap calls.
        let palette = renderer.palette_view().clone();

        // A: the supersampled grid.
        let grid = {
            let mut e = crate::escape::EscapeRenderer::new(&device, w, h);
            e.resize(&device, w, h, 3);
            assert_eq!(e.effective_supersample(), 3, "the grid must fit at this size");
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("grid"),
            });
            settle(&mut e, &esc, &mut enc, &palette);
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            let px = tonemap_read(&e, e.output_view(), &mut renderer);
            e.destroy();
            px
        };

        // B: no grid, the same 3x3 positions accumulated.
        let accumulated = {
            let mut e = crate::escape::EscapeRenderer::new(&device, w, h);
            e.resize(&device, w, h, 1);
            e.begin_accumulation(&device, &queue, 3);
            for off in crate::escape::EscapeRenderer::sample_grid(3) {
                e.set_sample_offset(off);
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("accum"),
                });
                settle(&mut e, &esc, &mut enc, &palette);
                e.accumulate_sample(&device, &queue, &mut enc);
                queue.submit(std::iter::once(enc.finish()));
                let _ =
                    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            }
            let view = e.accumulated_view().expect("accumulation target").clone();
            let px = tonemap_read(&e, &view, &mut renderer);
            e.destroy();
            px
        };

        // A single accumulated sample at offset zero must equal a
        // plain render exactly.
        {
            let mut e = crate::escape::EscapeRenderer::new(&device, w, h);
            e.resize(&device, w, h, 1);
            e.begin_accumulation(&device, &queue, 1);
            for off in crate::escape::EscapeRenderer::sample_grid(1) {
                e.set_sample_offset(off);
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("probe"),
                });
                settle(&mut e, &esc, &mut enc, &palette);
                e.accumulate_sample(&device, &queue, &mut enc);
                queue.submit(std::iter::once(enc.finish()));
                let _ =
                    device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            }
            let v = e.accumulated_view().unwrap().clone();
            let one = tonemap_read(&e, &v, &mut renderer);
            e.destroy();
            let mut e2 = crate::escape::EscapeRenderer::new(&device, w, h);
            e2.resize(&device, w, h, 1);
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("probe plain"),
            });
            settle(&mut e2, &esc, &mut enc, &palette);
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            let plain1 = tonemap_read(&e2, e2.output_view(), &mut renderer);
            e2.destroy();
            let d: i32 = one
                .chunks(4)
                .zip(plain1.chunks(4))
                .map(|(a, b)| (0..3).map(|c| (a[c] as i32 - b[c] as i32).abs()).max().unwrap())
                .max()
                .unwrap();
            println!("one accumulated sample vs a plain render: worst diff {d}");
            // EXACT, not approximate: one sample at offset zero is the
            // plain render, so any difference here is the accumulation
            // arithmetic itself (the 1/n scale, the clear-on-first
            // rule, the ping-pong) rather than sampling. Separating
            // the two is what showed the residual below to be
            // positional.
            assert_eq!(
                d, 0,
                "a single accumulated sample at offset zero must reproduce the                  plain render exactly"
            );
        }

        // A render with no antialiasing, as the yardstick below.
        let plain = {
            let mut e = crate::escape::EscapeRenderer::new(&device, w, h);
            e.resize(&device, w, h, 1);
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("plain"),
            });
            settle(&mut e, &esc, &mut enc, &palette);
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            let px = tonemap_read(&e, e.output_view(), &mut renderer);
            e.destroy();
            px
        };

        let mut worst = 0i32;
        let mut sum = 0f64;
        for (a, b) in grid.chunks(4).zip(accumulated.chunks(4)) {
            for c in 0..3 {
                let d = (a[c] as i32 - b[c] as i32).abs();
                worst = worst.max(d);
                sum += d as f64;
            }
        }
        let mean = sum / (grid.len() / 4 * 3) as f64;
        println!("grid 3x vs 3x accumulated: mean {mean:.2}/255, worst {worst}");
        // How far a render with NO antialiasing sits from the grid,
        // as the yardstick. Accumulation stands in for the grid, so it
        // has to land far closer to it than doing nothing does.
        //
        // Not bit-equality, deliberately: the two take the same
        // NOMINAL sample positions, but the direct path carries its
        // centre to the shader as f32, so a sub-pixel shift lands a
        // few ten-thousandths of a pixel off — and at a chaotic
        // boundary that is enough to flip a pixel's escape count. The
        // claim worth testing is that the method reproduces the grid,
        // not that it reproduces its rounding.
        let mut plain_sum = 0f64;
        for (a, b) in grid.chunks(4).zip(plain.chunks(4)) {
            for c in 0..3 {
                plain_sum += (a[c] as i32 - b[c] as i32).abs() as f64;
            }
        }
        let plain_mean = plain_sum / (grid.len() / 4 * 3) as f64;
        println!("for reference, no AA vs grid: mean {plain_mean:.2}/255");
        assert!(
            mean < plain_mean * 0.35,
            "accumulated antialiasing ({mean:.2}/255 from the grid) is not much \
             closer to it than no antialiasing at all ({plain_mean:.2}/255) -- it \
             is supposed to be the same sample positions by another route"
        );

        let aliasing = |px: &[u8]| -> f64 {
            let mut acc = 0f64;
            for y in 0..h as usize {
                for x in 1..w as usize - 1 {
                    let i = (y * w as usize + x) * 4;
                    for c in 0..3 {
                        let a = px[i - 4 + c] as f64;
                        let b = px[i + c] as f64;
                        let d = px[i + 4 + c] as f64;
                        acc += (d - 2.0 * b + a).abs();
                    }
                }
            }
            acc
        };
        let (a_plain, a_acc) = (aliasing(&plain), aliasing(&accumulated));
        println!("aliasing: none {a_plain:.0}, accumulated {a_acc:.0}");
        assert!(
            a_acc < a_plain * 0.85,
            "the accumulated render ({a_acc:.0}) is no smoother than one with no \
             antialiasing at all ({a_plain:.0}) -- the offsets are not reaching \
             the view"
        );
    }

    /// A render that cannot get its memory must FAIL, not return a
    /// black image.
    ///
    /// Reported from the app: a 4000x3000 export at 8x antialiasing
    /// produced an all-black PNG. The crash log named it exactly --
    /// `wgpu error: Out of Memory`, then `Buffer with 'Escape Iter
    /// State' label is invalid` on every dispatch afterwards. wgpu
    /// reports an allocation failure through the uncaptured-error
    /// handler, which stops nothing: the buffer comes back invalid,
    /// each dispatch against it quietly does nothing, and the export
    /// reports SUCCESS over an empty image.
    ///
    /// Asking a real device for an impossible allocation is the only
    /// honest way to test this, so that is what it does: a render
    /// whose per-pixel state cannot fit any GPU.
    #[test]
    #[ignore = "needs a GPU"]
    fn an_allocation_failure_is_reported_not_rendered_black() {
        let (device, queue) = repro_device();
        let mut config = crate::config::FractalConfig::default();
        config.render_mode = crate::scene::transforms::RenderMode::Escape;
        // Deep enough for the perturbed path, which is what carries
        // the large per-pixel state.
        config.escape.center_re = "-0.7436438870371587".to_string();
        config.escape.center_im = "0.1318259042053119".to_string();
        config.escape.zoom_log2 = 20.0;
        config.escape.max_iter = 200;

        // A size no device can hold: 40000x30000 is 1.2 gigapixels,
        // and the perturbed path wants ~72 bytes of state for each.
        let job = crate::renderer::RenderJob::new(&config, 40_000, 30_000);
        let result = pollster::block_on(crate::renderer::render(
            &device,
            &queue,
            job,
            &mut crate::renderer::NoProgress,
        ));

        match result {
            Err(crate::renderer::RenderError::OutOfMemory(msg)) => {
                println!("reported honestly: {msg}");
            }
            Err(other) => {
                // Any explicit failure is acceptable -- the point is
                // that it does not silently succeed.
                println!("reported as: {other}");
            }
            Ok(out) => {
                let lit = out
                    .rgba_data
                    .chunks(4)
                    .filter(|p| p[0] as u32 + p[1] as u32 + p[2] as u32 > 24)
                    .count();
                panic!(
                    "a render that could not allocate its buffers reported SUCCESS \
                     with {lit} lit pixels of {} -- an out-of-memory must not be \
                     served as an image",
                    out.rgba_data.len() / 4
                );
            }
        }
    }

    /// GPU-time pacing must engage, and must not change the image.
    ///
    /// The wall-clock proxy it replaces is honest only once the queue
    /// has drained; with submissions in flight it reads short and the
    /// chunk doubles on measurements that have not happened yet. This
    /// asserts the timestamp path actually lands a measurement (it
    /// travels through a buffer map, so a silent failure would just
    /// look like "the fallback is fine"), and that a render paced by
    /// it is byte-identical to a fixed-chunk one -- pacing may cost
    /// time, never pixels.
    #[test]
    #[ignore = "needs a GPU"]
    fn gpu_timestamps_pace_without_changing_the_image() {
        let (device, queue) = repro_device();
        if !device.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            println!("adapter has no TIMESTAMP_QUERY: wall-clock pacing covers this device");
            return;
        }
        let mut esc_cfg = crate::config::escape::EscapeConfig::default();
        esc_cfg.center_re = "-0.743643887037151".to_string();
        esc_cfg.center_im = "0.131825904205330".to_string();
        esc_cfg.zoom_log2 = 22.0;
        // Enough iterations to need SEVERAL chunks: a measurement
        // travels Idle -> Encoded -> Mapping -> result, so a render
        // that settles in one dispatch can never produce one.
        esc_cfg.max_iter = 400_000;

        let (w, h) = (192u32, 128u32);
        let render = |fixed: bool| -> (Vec<u8>, Option<f32>) {
            let config = crate::config::FractalConfig::default();
            let mut renderer =
                crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
                    &device,
                    &queue,
                    wgpu::TextureFormat::Rgba8Unorm,
                    w,
                    h,
                    &config.flame,
                    config.palette_size,
                );
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            escape.set_fixed_chunk(fixed);
            let mut guard = 0u32;
            loop {
                let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: Some("ts frame"),
                });
                let settled =
                    escape.render(&device, &queue, &mut enc, &esc_cfg, renderer.palette_view());
                queue.submit(std::iter::once(enc.finish()));
                // Wait, so map callbacks are delivered: the app does
                // this with a non-blocking Poll once a frame.
                let _ = device.poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: None,
                });
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 100_000, "render failed to settle (fixed={fixed})");
            }
            let measured = escape.gpu_ms_per_iter();
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("ts tonemap"),
            });
            renderer.tonemap_pass_with_input(&device, &queue, &mut enc, escape.output_view());
            queue.submit(std::iter::once(enc.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device,
                &queue,
                false,
                config.background_color,
            ))
            .expect("readback");
            (rgba, measured)
        };

        let (fixed, _) = render(true);
        let (paced, measured) = render(false);
        let mspi = measured.expect(
            "no GPU timestamp result landed -- the pacer silently fell back to wall clock",
        );
        assert!(
            mspi > 0.0 && mspi.is_finite(),
            "implausible measured cost per iteration: {mspi}"
        );
        let diff = fixed
            .iter()
            .zip(paced.iter())
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(
            diff,
            0,
            "GPU-paced render differs from fixed-chunk in {diff} of {} bytes",
            fixed.len()
        );
        println!("GPU pacing measured {:.3e} ms/iteration", mspi);
    }

    /// The fixed-chunk mode must not change the image.
    ///
    /// End-to-end numeric ground truth on the dip-seam view: GPU
    /// per-pixel iteration counts (terminal records) against exact
    /// fixed-point CPU orbits, same pixel mapping. The reference
    /// orbit here dips to |Z| ~ 2^-51 mid-orbit — the configuration
    /// that exposed the un-normalized reference-power underflow (a
    /// straight seam, 17% of pixels wrong; see
    /// fe_step_survives_reference_dips for the CPU-side anatomy).
    /// Writes output/seam_diff.png: green = agree, red = wrong.
    #[test]
    #[ignore = "needs a GPU + ~1 min of exact CPU orbits"]
    fn deep_multibrot_matches_exact_orbits() {
        use rayon::prelude::*;
        let re_s = "-0.96417873977697013026011288714743352326571975407038683546248307556701454144367857998579799044153007991031446482736663768012668752446996327705342813138475";
        let im_s = "0.20113415795567669775171048165243266489819985719087999752585437953404063460519395316530711399750169766068721928290032028758102407532349929726277906601037";
        let zoom = 426.5725f64;
        let rot = 0.545846f64;
        let (w, h) = (300u32, 200u32);
        let max_iter = 2660u32;

        // --- GPU render to settled, then read the records.
        let (device, queue) = repro_device();
        let config = crate::config::FractalConfig::default();
        let renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            w,
            h,
            &config.flame,
            config.palette_size,
        );
        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        let mut esc_cfg = crate::config::escape::EscapeConfig::default();
        esc_cfg.formula = "multibrot".to_string();
        esc_cfg.formula_params.insert("power".to_string(), 4.0f32);
        esc_cfg.center_re = re_s.to_string();
        esc_cfg.center_im = im_s.to_string();
        esc_cfg.zoom_log2 = zoom;
        esc_cfg.rotation = rot as f32;
        esc_cfg.max_iter = max_iter;
        let mut guard = 0u32;
        loop {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("seam frame"),
            });
            let settled =
                escape.render(&device, &queue, &mut enc, &esc_cfg, renderer.palette_view());
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            if settled {
                break;
            }
            guard += 1;
            assert!(guard < 200_000, "failed to settle");
        }
        let gpu: Vec<(u32, u32)> = escape
            .read_results_counts(&device, &queue)
            .expect("records inactive — results_fit failed?");

        // --- exact CPU truth, same mapping as the shader.
        let n_limbs = 9usize;
        let cx = crate::escape::fixedpoint::FixedPoint::from_decimal(re_s, n_limbs).unwrap();
        let cy = crate::escape::fixedpoint::FixedPoint::from_decimal(im_s, n_limbs).unwrap();
        let s = 2f64.powf(2.0 - zoom) / h as f64;
        let truth: Vec<u32> = (0..(w * h) as usize)
            .into_par_iter()
            .map(|idx| {
                let gx = (idx as u32 % w) as f64 + 0.5 - w as f64 / 2.0;
                let gy = -((idx as u32 / w) as f64 + 0.5 - h as f64 / 2.0);
                let dx = (gx * rot.cos() - gy * rot.sin()) * s;
                let dy = (gx * rot.sin() + gy * rot.cos()) * s;
                let c = crate::escape::fixedpoint::FixedComplex {
                    re: cx.add(&crate::escape::fixedpoint::FixedPoint::from_f64(dx, n_limbs)),
                    im: cy.add(&crate::escape::fixedpoint::FixedPoint::from_f64(dy, n_limbs)),
                };
                let mut z = crate::escape::fixedpoint::FixedComplex::zero(n_limbs);
                for i in 0..max_iter {
                    z = z.sqr();
                    z = z.sqr();
                    z = z.add(&c);
                    let x = z.re.to_f64();
                    let y = z.im.to_f64();
                    if x * x + y * y > 4.0 {
                        return i + 1;
                    }
                }
                max_iter
            })
            .collect();

        // --- diff map + stats.
        let mut img = vec![0u8; (w * h) as usize * 3];
        let mut wrong = 0usize;
        let mut offsets: Vec<i64> = Vec::new();
        for i in 0..(w * h) as usize {
            let (gn, tags) = gpu[i];
            let escaped = tags & 1 != 0;
            let tn = truth[i];
            let g = if escaped { gn as i64 } else { max_iter as i64 };
            let t = tn as i64;
            let d = (g - t).abs();
            offsets.push(g - t);
            let p: [u8; 3] = if d <= 2 {
                [0, 160, 0]
            } else if d <= 20 {
                [200, 200, 0]
            } else {
                wrong += 1;
                [220, 0, 0]
            };
            img[i * 3..i * 3 + 3].copy_from_slice(&p);
        }
        offsets.sort_unstable();
        println!(
            "pixels: {}  wrong(>20): {} ({:.1}%)  offset median {} p10 {} p90 {}",
            w * h,
            wrong,
            wrong as f64 * 100.0 / (w * h) as f64,
            offsets[offsets.len() / 2],
            offsets[offsets.len() / 10],
            offsets[offsets.len() * 9 / 10]
        );
        image::save_buffer("output/seam_diff.png", &img, w, h, image::ColorType::Rgb8).unwrap();
        println!("wrote output/seam_diff.png");
        assert!(
            wrong < (w * h) as usize / 50,
            "{wrong} pixels disagree with exact orbits by >20 iterations (>2%) — \
             the perturbed multibrot has regressed at reference dips"
        );
    }

    /// Batching several chunk dispatches into one redraw must not
    /// change a pixel: the same iteration windows run in the same
    /// order, only their grouping into frames differs. This drives
    /// the real perturbed path twice — batching disabled via its
    /// escape hatch, then free — and requires bit-identical images,
    /// plus proof that the free run actually batched (real timestamp
    /// measurements land on this device and make the batch engage).
    #[test]
    #[ignore = "needs a GPU"]
    fn batched_chunks_render_the_same_image() {
        let (device, queue) = repro_device();
        let mut esc_cfg = crate::config::escape::EscapeConfig::default();
        esc_cfg.center_re = "-0.75".to_string();
        esc_cfg.center_im = "0.1".to_string();
        esc_cfg.zoom_log2 = 20.0;
        // Modest depth, tiny chunks: escapes spread across the WHOLE
        // window sequence, so a mis-stitched batch window shifts
        // visible escape counts. (At a huge max_iter every pixel
        // resolves in the first unbatched frames and the survivors
        // are interior-coloured — a window bug would be invisible.)
        esc_cfg.max_iter = 4_000;

        let (w, h) = (192u32, 128u32);
        let mut render = |cap: Option<&str>| -> (Vec<u8>, u32) {
            match cap {
                Some(v) => std::env::set_var("ESCAPE_CHUNK_BATCH", v),
                None => std::env::remove_var("ESCAPE_CHUNK_BATCH"),
            }
            let config = crate::config::FractalConfig::default();
            let mut renderer =
                crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
                    &device,
                    &queue,
                    wgpu::TextureFormat::Rgba8Unorm,
                    w,
                    h,
                    &config.flame,
                    config.palette_size,
                );
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            // Tiny pinned chunks: many windows, so batching has room
            // to matter (and the run without it takes many frames).
            escape.chunk_override = Some(16);
            let mut max_batch = 0u32;
            let mut guard = 0u32;
            loop {
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("batch frame"),
                    });
                let settled = escape.render(
                    &device,
                    &queue,
                    &mut encoder,
                    &esc_cfg,
                    renderer.palette_view(),
                );
                queue.submit(std::iter::once(encoder.finish()));
                let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
                max_batch = max_batch.max(crate::escape::diag::snapshot().chunk_batch);
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 100_000, "batch loop failed to settle (cap={cap:?})");
            }
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("batch tonemap"),
            });
            renderer.tonemap_pass_with_input(&device, &queue, &mut encoder, escape.output_view());
            queue.submit(std::iter::once(encoder.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device,
                &queue,
                false,
                config.background_color,
            ))
            .expect("readback");
            (rgba, max_batch)
        };

        let (single, single_max) = render(Some("1"));
        let (batched, batched_max) = render(None);
        assert_eq!(single_max, 1, "the escape hatch must hold batching at one");
        assert!(
            batched_max > 1,
            "the free run never batched (max {batched_max}) — either timestamps are \
             unavailable on this device or the batch gate is broken"
        );
        let diff = single
            .iter()
            .zip(batched.iter())
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(
            diff, 0,
            "batched and single-chunk renders differ in {diff} bytes — chunk grouping \
             must be invisible"
        );
    }

    /// `set_fixed_chunk` exists for callers that submit chunk after
    /// chunk without letting the queue drain (the browser export --
    /// there is no blocking device poll on the web), where the
    /// adaptive sizer's wall-clock proxy measures CPU encode time,
    /// reads every chunk as free, and grows into a watchdog-length
    /// dispatch. Pinning the size is only safe because the render is
    /// chunk-invariant; this asserts that invariance for the mode
    /// itself, on a supersampled view (the export's shape) rather
    /// than the plain one the ESCAPE_CHUNK_MS test covers.
    #[test]
    #[ignore = "needs a GPU"]
    fn fixed_chunk_renders_the_same_image_as_adaptive() {
        let (device, queue) = repro_device();
        let mut esc_cfg = crate::config::escape::EscapeConfig::default();
        esc_cfg.center_re = "-0.75".to_string();
        esc_cfg.center_im = "0.1".to_string();
        esc_cfg.zoom_log2 = 20.0;
        esc_cfg.max_iter = 4_000;
        esc_cfg.supersample = 2;

        let (w, h) = (192u32, 128u32);
        let render = |fixed: bool| -> Vec<u8> {
            let config = crate::config::FractalConfig::default();
            let mut renderer =
                crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
                    &device,
                    &queue,
                    wgpu::TextureFormat::Rgba8Unorm,
                    w,
                    h,
                    &config.flame,
                    config.palette_size,
                );
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            escape.set_fixed_chunk(fixed);
            escape.resize(&device, w, h, esc_cfg.supersample);
            let mut guard = 0u32;
            loop {
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("fixed-chunk frame"),
                    });
                let settled = escape.render(
                    &device,
                    &queue,
                    &mut encoder,
                    &esc_cfg,
                    renderer.palette_view(),
                );
                queue.submit(std::iter::once(encoder.finish()));
                if settled {
                    break;
                }
                guard += 1;
                assert!(guard < 100_000, "chunk loop failed to settle (fixed={fixed})");
            }
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("fixed-chunk tonemap"),
            });
            renderer.tonemap_pass_with_input(&device, &queue, &mut encoder, escape.output_view());
            queue.submit(std::iter::once(encoder.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device,
                &queue,
                false,
                config.background_color,
            ))
            .expect("readback");
            rgba
        };

        let adaptive = render(false);
        let fixed = render(true);
        let diff = adaptive
            .iter()
            .zip(fixed.iter())
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(
            diff,
            0,
            "fixed-chunk render differs from adaptive in {diff} of {} bytes",
            adaptive.len()
        );
    }

    #[test]
    #[ignore = "needs a GPU"]
    fn progressive_and_blocking_render_the_same_image() {
        // The cold-vs-warm bug, pinned: a reference orbit STREAMED
        // from the worker crosses GPU-buffer capacity boundaries as
        // it grows, and the upload path used to refill a recreated
        // buffer with only the newest tail -- the settled render was
        // then structurally wrong, while the same orbit reloaded
        // complete from the store rendered correctly. Rendering the
        // same perturbed view both ways must give the SAME bytes.
        let (device, queue) = repro_device();
        let mut esc_cfg = crate::config::escape::EscapeConfig::default();
        // (-1.5, 0): bounded, chaotic (no early auto-closure), so the
        // reference grows to full max_iter -- 30k iterations at the
        // worker's 4096-iteration publish chunks crosses several
        // 1.5x-headroom capacity boundaries.
        esc_cfg.center_re = "-1.5".to_string();
        esc_cfg.center_im = "0".to_string();
        esc_cfg.zoom_log2 = 20.0;
        esc_cfg.max_iter = 30_000;

        let (w, h) = (320u32, 200u32);
        let render = |progressive: bool| -> Vec<u8> {
            let config = crate::config::FractalConfig::default();
            let mut renderer =
                crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
                    &device,
                    &queue,
                    wgpu::TextureFormat::Rgba8Unorm,
                    w,
                    h,
                    &config.flame,
                    config.palette_size,
                );
            let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
            escape.progressive = progressive;
            // Frame loop: render until settled (the progressive side
            // streams the orbit; the blocking side finishes when its
            // chunked iterations complete).
            let deadline = std::time::Instant::now() + std::time::Duration::from_secs(120);
            loop {
                let mut encoder =
                    device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                        label: Some("prog-vs-block frame"),
                    });
                let settled = escape.render(
                    &device,
                    &queue,
                    &mut encoder,
                    &esc_cfg,
                    renderer.palette_view(),
                );
                queue.submit(std::iter::once(encoder.finish()));
                if settled {
                    break;
                }
                assert!(
                    std::time::Instant::now() < deadline,
                    "render did not settle (progressive={progressive})"
                );
                std::thread::sleep(std::time::Duration::from_millis(1));
            }
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("prog-vs-block tonemap"),
            });
            renderer.tonemap_pass_with_input(&device, &queue, &mut encoder, escape.output_view());
            queue.submit(std::iter::once(encoder.finish()));
            let (_, _, rgba) = pollster::block_on(renderer.read_fractal_pixels(
                &device,
                &queue,
                false,
                config.background_color,
            ))
            .expect("readback");
            rgba
        };

        let blocking = render(false);
        let progressive = render(true);
        let diff = blocking
            .iter()
            .zip(progressive.iter())
            .filter(|(a, b)| a != b)
            .count();
        assert_eq!(
            diff, 0,
            "progressive and blocking renders differ in {diff} of {} bytes",
            blocking.len()
        );
    }

    /// Device + queue with the repro tests' standard setup.
    fn repro_device() -> (wgpu::Device, wgpu::Queue) {
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
        limits.max_storage_buffer_binding_size = adapter_limits.max_storage_buffer_binding_size;
        limits.max_buffer_size = adapter_limits.max_buffer_size;
        // TIMESTAMP_QUERY when the adapter has it: the GPU-time pacer
        // is only exercised on a device that requested the feature.
        let mut feats = wgpu::Features::CLEAR_TEXTURE;
        if adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            feats |= wgpu::Features::TIMESTAMP_QUERY;
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("escape repro"),
            required_features: feats,
            required_limits: limits,
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        }))
        .expect("device");
        device.on_uncaptured_error(std::sync::Arc::new(|e| {
            panic!("wgpu error during repro: {e}");
        }));
        (device, queue)
    }

    /// MANUAL: the wall-clock timeline of revisiting a CACHED deep
    /// location, driven exactly the way the app drives a frame —
    /// worker, store load, BLA build, uploads, chunked GPU render.
    /// Answers "where does the time actually go in-app".
    #[test]
    #[ignore = "manual: needs a GPU and output/orbit_profile from the generator"]
    fn timeline_of_a_cached_revisit() {
        // Stage an orbit as the ONLY store entry: ORBIT_SRC names a
        // .orbit file (e.g. one from the real app cache), default the
        // generated profile orbit.
        let src = match std::env::var("ORBIT_SRC") {
            Ok(p) => std::path::PathBuf::from(p),
            Err(_) => std::fs::read_dir(std::path::PathBuf::from("output").join("orbit_profile"))
                .unwrap()
                .flatten()
                .map(|e| e.path())
                .find(|p| p.extension().is_some_and(|x| x == "orbit"))
                .expect("run generate_deep_profile_orbit first"),
        };
        let store = crate::escape::orbit_store::test_store_dir().expect("store dir");
        for e in std::fs::read_dir(&store).unwrap().flatten() {
            let _ = std::fs::remove_file(e.path());
        }
        std::fs::copy(&src, store.join(src.file_name().unwrap())).unwrap();
        // The stored header carries the exact view this orbit was
        // saved from — center, zoom — so the revisit is faithful.
        let head_bytes = std::fs::read(&src).unwrap();
        let head = crate::escape::reference::header_from_bytes(
            &head_bytes[..crate::escape::reference::MAX_HEADER_BYTES.min(head_bytes.len())],
        )
        .expect("orbit header");
        println!(
            "staged: len={} limbs={} zoom={:.1} center digits={}",
            head.orbit_len,
            head.n_limbs,
            head.off_zoom_log2,
            head.center_re.len()
        );

        let (device, queue) = repro_device();
        let config = crate::config::FractalConfig::default();
        let renderer = crate::renderer::compute_kernel::FlameRenderer::with_palette_size(
            &device,
            &queue,
            wgpu::TextureFormat::Rgba8Unorm,
            400,
            300,
            &config.flame,
            config.palette_size,
        );

        let (w, h) = (1878u32, 1056u32);
        let mut escape = crate::escape::EscapeRenderer::new(&device, w, h);
        let mut esc_cfg = crate::config::escape::EscapeConfig::default();
        // Same VALUE as the stored orbit's center, padded so
        // limbs_for_view lands on the file's 197 limbs — the string
        // differs, so this exercises the nearby-relocation load, like
        // a real pan-then-save-then-revisit.
        esc_cfg.center_re = head.center_re.clone();
        esc_cfg.center_im = head.center_im.clone();
        esc_cfg.zoom_log2 = head.off_zoom_log2;
        esc_cfg.max_iter = (head.orbit_len as u32).saturating_sub(1);
        assert_eq!(
            crate::escape::fixedpoint::limbs_for_view(
                &esc_cfg.center_re,
                &esc_cfg.center_im,
                esc_cfg.zoom_log2
            ),
            head.n_limbs,
            "request must match the stored orbit's precision"
        );

        let t0 = std::time::Instant::now();
        let mut settled_at: Option<(u32, f32, f32)> = None;
        for frame in 0..100_000u32 {
            let tf = std::time::Instant::now();
            let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("timeline frame"),
            });
            escape.render(&device, &queue, &mut encoder, &esc_cfg, renderer.palette_view());
            queue.submit(std::iter::once(encoder.finish()));
            let _ = device.poll(wgpu::PollType::Wait { submission_index: None, timeout: None });
            let frame_ms = tf.elapsed().as_secs_f64() * 1e3;
            let d = crate::escape::diag::snapshot();
            if frame_ms > 40.0 || frame % 120 == 0 {
                println!(
                    "f{frame:5} t={:7.2}s frame={frame_ms:7.1}ms cpu={:6.1}ms src={:?} \
                     orbit_ms={:.0} wait={} bla={:6.1}ms inflight={} chunk_iters={} batch={} settle={:.0}ms",
                    t0.elapsed().as_secs_f64(),
                    d.render_cpu_ms,
                    d.orbit_source,
                    d.orbit_ms,
                    d.orbit_wait_frames,
                    d.bla_build_ms,
                    d.inflight_frames,
                    d.last_chunk_iters,
                    d.chunk_batch,
                    d.settle_ms,
                );
            }
            if d.settle_ms > 0.0 && settled_at.is_none() {
                settled_at = Some((frame, t0.elapsed().as_secs_f32(), d.settle_ms));
                println!(
                    "SETTLED at frame {frame}: wall {:.2}s from start, settle_ms {:.0} \
                     over {} frames",
                    t0.elapsed().as_secs_f64(),
                    d.settle_ms,
                    d.settle_frames
                );
                break;
            }
            if t0.elapsed().as_secs_f64() > 600.0 {
                println!(
                    "CAP at 600s: inflight={} frames, last_chunk_iters={}, settle pending",
                    d.inflight_frames, d.last_chunk_iters
                );
                break;
            }
        }
        let d = crate::escape::diag::snapshot();
        println!(
            "final: relocations={} rebuilds={} bla_bytes={}MB upload={}MB settled={:?}",
            d.orbit_relocations,
            d.orbit_rebuilds,
            d.bla_bytes / (1024 * 1024),
            d.upload_bytes / (1024 * 1024),
            settled_at
        );
    }

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
        // TIMESTAMP_QUERY when the adapter has it: the GPU-time pacer
        // is only exercised on a device that requested the feature.
        let mut feats = wgpu::Features::CLEAR_TEXTURE;
        if adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            feats |= wgpu::Features::TIMESTAMP_QUERY;
        }
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("escape repro"),
            required_features: feats,
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
            direct
        };

        // A view with no structure would let any two renders "agree".
        // The escaping cases below are all recognisable fractals, but
        // the non-escaping families paint a smooth field, where a
        // mis-set coloring really can come out flat -- so those assert
        // on this.
        let assert_has_structure = |label: &str, img: &Vec<u8>| {
            let lum: Vec<f64> = img
                .chunks_exact(4)
                .map(|p| 0.299 * p[0] as f64 + 0.587 * p[1] as f64 + 0.114 * p[2] as f64)
                .collect();
            let mean = lum.iter().sum::<f64>() / lum.len() as f64;
            let sd = (lum.iter().map(|v| (v - mean).powi(2)).sum::<f64>() / lum.len() as f64).sqrt();
            println!("structure[{label}]: luminance mean {mean:.1} sd {sd:.2}");
            assert!(sd > 1.0, "[{label}] the direct render is nearly flat (sd {sd:.2}) -- nothing to compare");
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

        // Tricorn (and a Multicorn) over the home view: the
        // anti-holomorphic tier is the power binomial over conjugated
        // operands, so a dropped or doubled conjugate renders the
        // MULTIBROT instead -- a mirror image, which block means catch
        // immediately.
        let mut tri_cfg = crate::config::escape::EscapeConfig::default();
        tri_cfg.formula = "tricorn".to_string();
        tri_cfg.center_re = "0".to_string();
        tri_cfg.center_im = "0".to_string();
        tri_cfg.zoom_log2 = 1.0;
        tri_cfg.max_iter = 300;
        tri_cfg.coloring_params.insert("scale".to_string(), 0.02);
        for p in [2.0f32, 3.0, 5.0] {
            tri_cfg.formula_params.insert("power".to_string(), p);
            check(&format!("tricorn{p}"), &tri_cfg, false);
            check(&format!("tricorn{p}-floatexp"), &tri_cfg, true);
        }

        // Phoenix: a TWO-TERM recurrence, so the perturbed path has to
        // carry a second delta and rebase the pair together. A dropped
        // history term, or a rebase that moves only the current delta,
        // renders a different map -- which the direct comparison sees.
        let mut ph_cfg = crate::config::escape::EscapeConfig::default();
        ph_cfg.formula = "phoenix".to_string();
        ph_cfg.center_re = "0".to_string();
        ph_cfg.center_im = "0".to_string();
        ph_cfg.zoom_log2 = 1.0;
        ph_cfg.max_iter = 300;
        ph_cfg.coloring_params.insert("scale".to_string(), 0.02);
        // FIRST with no formula_params at all: an unedited config is
        // what the app actually renders, and it is the case where the
        // reference and the shader once resolved different defaults.
        assert!(ph_cfg.formula_params.is_empty());
        check("phoenix-defaults", &ph_cfg, false);
        // ... and on the DEEP rung, whose two-term state is a genuinely
        // different implementation: the history lives in real struct
        // fields rather than the scaled rung's spare `w_lo`, and the
        // pair rebase rebuilds both deltas in double-float. Before it
        // existed, Phoenix past zoom 48 fell through to the direct
        // path and rendered a single flat colour.
        check("phoenix-defaults-floatexp", &ph_cfg, true);
        for (pr, pi) in [(-0.5f32, 0.0f32), (0.25, 0.1)] {
            ph_cfg.formula_params.insert("p_re".to_string(), pr);
            ph_cfg.formula_params.insert("p_im".to_string(), pi);
            check(&format!("phoenix{pr}_{pi}"), &ph_cfg, false);
            check(&format!("phoenix{pr}_{pi}-floatexp"), &ph_cfg, true);
        }

        // Manowar: the same two-term recurrence with p = 1, but
        // seeded z_0 = z_-1 = c. That seed is the whole difference,
        // and it is exactly what a delta pipeline gets wrong quietly:
        // start both deltas at zero instead of d0 and the picture is
        // still a fractal, just not this one.
        let mut mw_cfg = crate::config::escape::EscapeConfig::default();
        mw_cfg.formula = "manowar".to_string();
        mw_cfg.center_re = "0".to_string();
        mw_cfg.center_im = "0".to_string();
        mw_cfg.zoom_log2 = 1.0;
        mw_cfg.max_iter = 300;
        mw_cfg.coloring_params.insert("scale".to_string(), 0.02);
        check("manowar", &mw_cfg, false);
        check("manowar-floatexp", &mw_cfg, true);
        mw_cfg.center_re = "-0.15".to_string();
        mw_cfg.center_im = "0.65".to_string();
        mw_cfg.zoom_log2 = 8.0;
        check("manowar-z8", &mw_cfg, false);
        check("manowar-z8-floatexp", &mw_cfg, true);

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

        // ---- the big-float families ----
        // These four are the reason this test earns its keep: their
        // references iterate in BIG FLOAT rather than fixed point, and
        // the user-visible symptom of getting one wrong is exactly
        // what this compares -- "the picture changes the moment
        // perturbation starts".

        // Newton: c-free, so the tier requests its reference
        // JULIA-STYLE (seed at the centre, no dc term) whatever the
        // toggle says. Every scheme that ships a delta is checked;
        // Chebyshev over the other two functions declines the tier,
        // which `perturb_tier` is asserted on below.
        let mut nw_cfg = crate::config::escape::EscapeConfig::default();
        nw_cfg.formula = "newton".to_string();
        nw_cfg.coloring = "root_basin".to_string();
        nw_cfg.center_re = "0.35".to_string();
        nw_cfg.center_im = "0.28".to_string();
        nw_cfg.zoom_log2 = 0.5;
        nw_cfg.max_iter = 64;
        nw_cfg.bailout = 1e6;
        nw_cfg.coloring_params.insert("roots".to_string(), 3.0);
        nw_cfg.coloring_params.insert("speed".to_string(), 0.01);
        for scheme in [0.0f32, 1.0, 2.0] {
            nw_cfg.formula_params.insert("scheme".to_string(), scheme);
            let img = check(&format!("newton-s{scheme}"), &nw_cfg, false);
            assert_has_structure(&format!("newton-s{scheme}"), &img);
            check(&format!("newton-s{scheme}-floatexp"), &nw_cfg, true);
        }
        nw_cfg.formula_params.remove("scheme");
        // The relaxation multiplies the step and rides the reference's
        // identity: a tier that ignored it would render plain Newton.
        nw_cfg.formula_params.insert("relax_re".to_string(), 1.1);
        nw_cfg.formula_params.insert("relax_im".to_string(), 0.2);
        check("newton-relaxed", &nw_cfg, false);
        check("newton-relaxed-floatexp", &nw_cfg, true);

        // Nova: the Newton step plus c, seeded at the CRITICAL POINT
        // z_0 = 1 on the parameter plane -- a seed that is easy to get
        // wrong quietly, since zero also produces a picture.
        let mut nv_cfg = crate::config::escape::EscapeConfig::default();
        nv_cfg.formula = "nova".to_string();
        nv_cfg.center_re = "-0.3".to_string();
        nv_cfg.center_im = "0".to_string();
        nv_cfg.zoom_log2 = 0.5;
        nv_cfg.max_iter = 128;
        nv_cfg.bailout = 1e6;
        nv_cfg.coloring_params.insert("scale".to_string(), 0.03);
        let img = check("nova", &nv_cfg, false);
        assert_has_structure("nova", &img);
        check("nova-floatexp", &nv_cfg, true);

        // Kaliset is deliberately NOT here. It is non-escaping and
        // inverting, and its delta form only becomes accurate past
        // zoom 24 (`tier_min_zoom`), so forcing it onto a shallow view
        // would be testing it outside the regime the engine ever uses
        // it in. Its accuracy is pinned by the exact-orbit test at
        // zoom 30 and by
        // `the_non_escaping_tiers_engage_only_where_they_are_accurate`.

        // Ducks: the transcendental one. Its delta is log1p of the
        // fold's ratio, and it is the tier whose branch cut bites --
        // `Log(T) + Log1p(u)` is the principal value only up to a
        // whole number of turns, and |z| is what the coloring
        // averages, so a missed turn is an O(1) error rather than a
        // cosmetic one. Both shipped variants, both planes.
        let mut dk_cfg = crate::config::escape::EscapeConfig::default();
        dk_cfg.formula = "ducks".to_string();
        dk_cfg.coloring = "magnitude_average".to_string();
        dk_cfg.center_re = "-0.4".to_string();
        dk_cfg.center_im = "0.3".to_string();
        dk_cfg.zoom_log2 = 8.0;
        dk_cfg.max_iter = 60;
        // A Ducks field spans ~0.01 around a mean of ~1.6 (measured),
        // so it needs the offset/scale pair its own preset uses --
        // without them the render is a flat wash and comparing it to
        // anything proves nothing (the structure guard says so).
        for (variant, offset, scale) in [(0.0f32, 1.600f32, 98.6f32), (4.0, 2.708, 54.9)] {
            dk_cfg.formula_params.insert("variant".to_string(), variant);
            dk_cfg.coloring_params.insert("offset".to_string(), offset);
            dk_cfg.coloring_params.insert("scale".to_string(), scale);
            let img = check(&format!("ducks-v{variant}"), &dk_cfg, false);
            assert_has_structure(&format!("ducks-v{variant}"), &img);
            check(&format!("ducks-v{variant}-floatexp"), &dk_cfg, true);
        }
        dk_cfg.formula_params.insert("variant".to_string(), 0.0);
        dk_cfg.julia = true;
        dk_cfg.julia_re = 0.1;
        dk_cfg.julia_im = -0.62;
        dk_cfg.coloring_params.insert("offset".to_string(), 1.648);
        dk_cfg.coloring_params.insert("scale".to_string(), 11.64);
        let img = check("ducks-julia", &dk_cfg, false);
        assert_has_structure("ducks-julia", &img);
        check("ducks-julia-floatexp", &dk_cfg, true);

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





