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

        let (w, h) = (160u32, 120u32);
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
