//! GPU tests: the renderer driven exactly as the app drives it.
//!
//! Twin of `src/escape/app_repro_test.rs`. Everything here needs a real
//! device, so it is `#[cfg(test)]` and desktop-only; the assembler's
//! naga tests cover what can be checked without one.
//!
//! What these are for, in order of how much they would hurt to lose:
//!
//! * **The rule is what the model says it is.** `gray_scott_matches_a_cpu_mirror`
//!   runs one step on the GPU and the same step in Rust and compares.
//!   A shader that produces a plausible-looking field with the wrong
//!   arithmetic is exactly the failure the phase-0 prototypes exist to
//!   prevent, and it would otherwise reach a baseline image unnoticed.
//! * **Batching does not change the result.** An export runs 10,000
//!   steps in submissions of 256; the viewport runs 4 at a time. If
//!   those diverged, a still would not be reproducible.
//! * **A run is a function of its seed.** Two renderers from one config
//!   must agree bit for bit.

use crate::config::sim::{SimBoundary, SimConfig, SimGrid, SimInit};
use crate::sim::{model_or_default, SimRenderer};
use wgpu::*;

fn repro_device() -> Option<(Device, Queue)> {
    let instance = Instance::new(InstanceDescriptor {
        backends: Backends::all(),
        ..InstanceDescriptor::new_without_display_handle()
    });
    let adapter = pollster::block_on(instance.request_adapter(&RequestAdapterOptions {
        power_preference: PowerPreference::HighPerformance,
        force_fallback_adapter: false,
        compatible_surface: None,
    }))
    .ok()?;
    let (device, queue) = pollster::block_on(adapter.request_device(&DeviceDescriptor {
        label: Some("sim repro"),
        required_features: Features::empty(),
        required_limits: adapter.limits(),
        memory_hints: MemoryHints::Performance,
        experimental_features: Default::default(),
        trace: Default::default(),
    }))
    .ok()?;
    device.on_uncaptured_error(std::sync::Arc::new(|e| panic!("wgpu error during sim repro: {e}")));
    Some((device, queue))
}

/// A greyscale ramp, standing in for the flame renderer's palette.
fn test_palette(device: &Device, queue: &Queue) -> TextureView {
    let tex = device.create_texture(&TextureDescriptor {
        label: Some("sim test palette"),
        size: Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    let mut data = vec![0u8; 256 * 4];
    for (i, px) in data.chunks_exact_mut(4).enumerate() {
        px[0] = i as u8;
        px[1] = i as u8;
        px[2] = i as u8;
        px[3] = 255;
    }
    queue.write_texture(
        TexelCopyTextureInfo {
            texture: &tex,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        &data,
        TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(256 * 4), rows_per_image: Some(1) },
        Extent3d { width: 256, height: 1, depth_or_array_layers: 1 },
    );
    tex.create_view(&TextureViewDescriptor::default())
}

/// Read an `Rgba32Float` texture back as `[f32; 4]` per texel.
fn read_rgba32f(device: &Device, queue: &Queue, tex: &Texture, w: u32, h: u32) -> Vec<[f32; 4]> {
    // Copy rows are 256-byte aligned, so a padded staging buffer is
    // required and the padding has to be stripped after mapping.
    let unpadded = (w * 16) as usize;
    let padded = unpadded.div_ceil(256) * 256;
    let buf = device.create_buffer(&BufferDescriptor {
        label: Some("sim readback"),
        size: (padded * h as usize) as u64,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
    enc.copy_texture_to_buffer(
        TexelCopyTextureInfo {
            texture: tex,
            mip_level: 0,
            origin: Origin3d::ZERO,
            aspect: TextureAspect::All,
        },
        TexelCopyBufferInfo {
            buffer: &buf,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(padded as u32),
                rows_per_image: Some(h),
            },
        },
        Extent3d { width: w, height: h, depth_or_array_layers: 1 },
    );
    queue.submit(std::iter::once(enc.finish()));

    let (tx, rx) = std::sync::mpsc::channel();
    buf.slice(..).map_async(MapMode::Read, move |r| {
        let _ = tx.send(r.is_ok());
    });
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    assert!(rx.recv().unwrap_or(false), "readback map failed");

    let view = buf.slice(..).get_mapped_range();
    let mut out = Vec::with_capacity((w * h) as usize);
    for y in 0..h as usize {
        let row = &view[y * padded..y * padded + unpadded];
        for px in row.chunks_exact(16) {
            out.push([
                f32::from_le_bytes(px[0..4].try_into().unwrap()),
                f32::from_le_bytes(px[4..8].try_into().unwrap()),
                f32::from_le_bytes(px[8..12].try_into().unwrap()),
                f32::from_le_bytes(px[12..16].try_into().unwrap()),
            ]);
        }
    }
    drop(view);
    buf.unmap();
    out
}

fn small_config() -> SimConfig {
    let mut cfg = SimConfig::default();
    cfg.grid = SimGrid::Fixed { width: 64, height: 64 };
    cfg.init = SimInit::Blob { radius: 8 };
    cfg.boundary = SimBoundary::Periodic;
    cfg.seed = 7;
    cfg
}

/// The field after a real run is a Gray–Scott field: finite, inside
/// [0, 1], and actually patterned rather than uniform.
#[test]
fn a_run_produces_a_finite_non_uniform_field() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let palette = test_palette(&device, &queue);
    let cfg = small_config();
    let mut r = SimRenderer::new(&device, &cfg, 64, 64);
    r.seed(&device, &queue, &cfg);
    r.run_steps(&device, &queue, &cfg, 400);
    r.color(&device, &queue, &cfg, &palette);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });

    let out = read_rgba32f(&device, &queue, r.output_texture(), 64, 64);
    assert_eq!(out.len(), 64 * 64);
    assert!(
        out.iter().all(|p| p.iter().all(|v| v.is_finite())),
        "the coloured output must be finite everywhere"
    );
    // Coverage is 1.0 for every cell in the `channel` colouring.
    assert!(out.iter().all(|p| (p[3] - 1.0).abs() < 1e-6), "alpha must be coverage = 1");
    let lo = out.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    let hi = out.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max);
    assert!(
        hi - lo > 0.05,
        "a 400-step Gray-Scott blob should not be a flat image (range {lo}..{hi})"
    );
}

/// The rule the shader runs is the rule the model documents.
///
/// One step, on a field this test seeds itself, against a Rust mirror
/// of Karl Sims' scheme. This is the test that would catch a swapped
/// weight or a missing clamp — the class of bug that still renders a
/// plausible picture.
#[test]
fn gray_scott_matches_a_cpu_mirror() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: usize = 64;
    let cfg = small_config();
    let mut r = SimRenderer::new(&device, &cfg, N as u32, N as u32);
    r.seed(&device, &queue, &cfg);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    // Read the seeded field, mirror one step on the CPU from exactly
    // that, then take one step on the GPU and compare. Starting from
    // the GPU's own seed keeps this a test of the STEP rule rather than
    // of the seeding shape.
    let before = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);
    r.run_steps(&device, &queue, &cfg, 1);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let after = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);

    let m = model_or_default(&cfg.model);
    let p = m.pack_params(&cfg);
    let (f, k, da, db) = (p[0], p[1], p[2], p[3]);
    let at = |x: i32, y: i32| -> [f32; 4] {
        let xi = ((x % N as i32) + N as i32) % N as i32;
        let yi = ((y % N as i32) + N as i32) % N as i32;
        before[yi as usize * N + xi as usize]
    };
    let mut worst = 0.0f32;
    for y in 0..N as i32 {
        for x in 0..N as i32 {
            let s = at(x, y);
            let lap = |c: usize| {
                -s[c]
                    + 0.2 * (at(x, y - 1)[c] + at(x, y + 1)[c] + at(x - 1, y)[c] + at(x + 1, y)[c])
                    + 0.05
                        * (at(x - 1, y - 1)[c]
                            + at(x + 1, y - 1)[c]
                            + at(x - 1, y + 1)[c]
                            + at(x + 1, y + 1)[c])
            };
            let (a, b) = (s[0], s[1]);
            let abb = a * b * b;
            let na = (a + (da * lap(0) - abb + f * (1.0 - a)) * cfg.dt).clamp(0.0, 1.0);
            let nb = (b + (db * lap(1) + abb - (k + f) * b) * cfg.dt).clamp(0.0, 1.0);
            let got = after[y as usize * N + x as usize];
            worst = worst.max((got[0] - na).abs()).max((got[1] - nb).abs());
        }
    }
    // Float arithmetic, not bit-exactness: the GPU may contract a
    // multiply-add the CPU does not. A tolerance this tight still
    // catches a wrong weight, a missing clamp or a swapped channel,
    // which are the mistakes worth catching.
    assert!(
        worst < 1e-6,
        "GPU Gray-Scott step differs from the CPU mirror by {worst}"
    );
}

/// Batching must not change the sequence: an export submits 256 steps
/// at a time and the viewport submits four, and a still has to be the
/// same picture either way.
#[test]
fn steps_are_batch_invariant() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let cfg = small_config();
    let n = 300; // deliberately more than one STEPS_PER_SUBMIT batch

    let mut a = SimRenderer::new(&device, &cfg, 64, 64);
    a.seed(&device, &queue, &cfg);
    a.run_steps(&device, &queue, &cfg, n);

    let mut b = SimRenderer::new(&device, &cfg, 64, 64);
    b.seed(&device, &queue, &cfg);
    for _ in 0..n {
        b.run_steps(&device, &queue, &cfg, 1);
    }
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });

    let fa = read_rgba32f(&device, &queue, a.field_texture(), 64, 64);
    let fb = read_rgba32f(&device, &queue, b.field_texture(), 64, 64);
    assert_eq!(a.step_index(), b.step_index());
    // EVERY channel, not just the concentration. An earlier version of
    // this compared channel 0 alone and passed while the age channel
    // (.z, which reads the step index from the uniform) was wrong in
    // every batched run -- queue.write_buffer is staged before the
    // command buffer executes, so all the steps in one submission saw
    // the same index. Comparing the whole texel is what catches that.
    let differing = fa
        .iter()
        .zip(&fb)
        .filter(|(x, y)| x.iter().zip(y.iter()).any(|(p, q)| p.to_bits() != q.to_bits()))
        .count();
    assert_eq!(
        differing, 0,
        "one batch of {n} and {n} batches of one must give identical fields;          {differing} texels differ"
    );
}

/// A run is a function of its config. Two renderers from one config
/// must agree bit for bit, which is what makes a visual baseline
/// meaningful at all.
#[test]
fn two_runs_from_one_seed_are_byte_identical() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let palette = test_palette(&device, &queue);
    let cfg = small_config();
    let mut a = SimRenderer::new(&device, &cfg, 96, 72);
    let mut b = SimRenderer::new(&device, &cfg, 96, 72);
    a.render_still(&device, &queue, &cfg, &palette);
    b.render_still(&device, &queue, &cfg, &palette);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });

    let ia = read_rgba32f(&device, &queue, a.output_texture(), 96, 72);
    let ib = read_rgba32f(&device, &queue, b.output_texture(), 96, 72);
    let differing = ia
        .iter()
        .zip(&ib)
        .filter(|(x, y)| x.iter().zip(y.iter()).any(|(p, q)| p.to_bits() != q.to_bits()))
        .count();
    assert_eq!(differing, 0, "{differing} texels differ between two identical runs");
}

/// A different seed must actually produce a different picture —
/// otherwise `seed` is decoration and the reproducibility test above
/// proves nothing.
#[test]
fn a_different_seed_gives_a_different_picture() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let palette = test_palette(&device, &queue);
    let mut cfg = small_config();
    cfg.init = SimInit::Blobs { count: 6, radius: 8 };
    let mut a = SimRenderer::new(&device, &cfg, 64, 64);
    a.render_still(&device, &queue, &cfg, &palette);
    cfg.seed = 12345;
    let mut b = SimRenderer::new(&device, &cfg, 64, 64);
    b.render_still(&device, &queue, &cfg, &palette);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });

    let ia = read_rgba32f(&device, &queue, a.output_texture(), 64, 64);
    let ib = read_rgba32f(&device, &queue, b.output_texture(), 64, 64);
    let differing = ia.iter().zip(&ib).filter(|(x, y)| x[0] != y[0]).count();
    assert!(differing > 100, "only {differing} texels differ between two seeds");
}

/// The grid is not the output: a fixed grid rendered to two different
/// output sizes is the same simulation, resolved twice.
#[test]
fn a_fixed_grid_is_the_same_run_at_any_output_size() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let palette = test_palette(&device, &queue);
    let cfg = small_config();
    let mut a = SimRenderer::new(&device, &cfg, 64, 64);
    let mut b = SimRenderer::new(&device, &cfg, 256, 256);
    a.render_still(&device, &queue, &cfg, &palette);
    b.render_still(&device, &queue, &cfg, &palette);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    assert_eq!(a.grid_size(), (64, 64));
    assert_eq!(b.grid_size(), (64, 64), "a Fixed grid must ignore the output size");

    let fa = read_rgba32f(&device, &queue, a.field_texture(), 64, 64);
    let fb = read_rgba32f(&device, &queue, b.field_texture(), 64, 64);
    assert_eq!(
        fa.iter().map(|p| p[1].to_bits()).collect::<Vec<_>>(),
        fb.iter().map(|p| p[1].to_bits()).collect::<Vec<_>>(),
        "the same Fixed grid must simulate identically regardless of output size"
    );
}

/// An absurd grid is refused with a message rather than aborting the
/// process inside wgpu.
#[test]
fn an_impossible_grid_is_refused_before_allocation() {
    let Some((device, _queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let mut cfg = SimConfig::default();
    // 32768^2 exceeds every device's max texture dimension, and its
    // field pair would be 64 GiB, so this is refused whichever check
    // fires first -- the test asserts the refusal, not which one.
    cfg.grid = SimGrid::Fixed { width: 32768, height: 32768 };
    let err = SimRenderer::allocation_error(&device, &cfg, 3840, 2160);
    assert!(err.is_some(), "an unallocatable grid must be refused");
    let msg = err.unwrap();
    assert!(
        msg.contains("grid") || msg.contains("memory"),
        "the refusal should say what to change, got {msg:?}"
    );

    // A large but genuinely allocatable grid is NOT refused: the check
    // exists to stop the impossible, not to second-guess the user.
    cfg.grid = SimGrid::Fixed { width: 2048, height: 2048 };
    assert!(
        SimRenderer::allocation_error(&device, &cfg, 1920, 1080).is_none(),
        "a 2048 grid is ordinary and must be allowed"
    );

    cfg.grid = SimGrid::Fixed { width: 256, height: 256 };
    assert!(
        SimRenderer::allocation_error(&device, &cfg, 1920, 1080).is_none(),
        "an ordinary config must not be refused"
    );
}

/// PHASE-1 GATE: the interactive budget at 1080p.
///
/// The plan's gate is "1080p at >= 60 fps with >= 4 steps per frame".
/// Phase 0 measured the bare stencil at 0.495 ms/step on this card;
/// this measures the SHIPPED path instead -- a real SimRenderer, the
/// real assembled shaders, and the colour+resolve pass that runs every
/// frame whether or not the simulation advanced.
///
/// Reported rather than asserted tightly: the number depends on the
/// machine, and a hard threshold here would fail on a laptop for
/// reasons that are not a regression. The assertion is only that the
/// gate's own bar is cleared.
///
/// **RUN WITH `--test-threads=1`.** cargo runs tests in parallel, and
/// the 4K gate below is 13 seconds of solid GPU work. Sharing a device
/// with it turned 1.38 ms/frame into 72.64 -- a 50x error that looks
/// exactly like a real regression, and which cost a round of
/// investigation before the cause was measured. Any GPU TIMING test
/// has this hazard; correctness tests do not care.
#[test]
#[ignore = "manual: GPU timing, phase-1 gate"]
fn phase1_gate_interactive_budget_at_1080p() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let palette = test_palette(&device, &queue);
    let mut cfg = SimConfig::default();
    cfg.grid = crate::config::sim::SimGrid::Viewport { scale: 1.0 };
    cfg.init = crate::config::sim::SimInit::Blobs { count: 6, radius: 24 };
    let (w, h) = (1920u32, 1080u32);
    let mut r = SimRenderer::new(&device, &cfg, w, h);
    r.seed(&device, &queue, &cfg);
    // Warm up: first frame pays pipeline creation and allocation.
    for _ in 0..3 {
        r.render_frame(&device, &queue, &cfg, &palette, 4);
    }
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });

    const FRAMES: u32 = 60;
    for spf in [1u32, 4, 8, 16] {
        let t0 = std::time::Instant::now();
        for _ in 0..FRAMES {
            r.render_frame(&device, &queue, &cfg, &palette, spf);
        }
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let ms = t0.elapsed().as_secs_f64() * 1000.0 / FRAMES as f64;
        println!(
            "1920x1080, {spf:>2} steps/frame: {ms:6.2} ms/frame  ({:5.1} fps)",
            1000.0 / ms
        );
        if spf == 4 {
            assert!(
                ms < 16.7,
                "PHASE-1 GATE FAILED: 4 steps/frame at 1080p took {ms:.2} ms, over the 16.7 ms \
                 budget for 60 fps"
            );
        }
    }
}

/// PHASE-1 GATE: a 4K export of 10,000 steps completes.
///
/// The risk is the ~2 s GPU watchdog: an export that submits its whole
/// run as one pass resets the device. `run_steps` batches internally,
/// and this is the test that the batching is actually sized for it.
#[test]
#[ignore = "manual: GPU timing, phase-1 gate"]
fn phase1_gate_4k_ten_thousand_steps() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let palette = test_palette(&device, &queue);
    let mut cfg = SimConfig::default();
    cfg.grid = crate::config::sim::SimGrid::Fixed { width: 3840, height: 2160 };
    cfg.init = crate::config::sim::SimInit::Blobs { count: 6, radius: 24 };
    cfg.steps = 10_000;
    if let Some(why) = SimRenderer::allocation_error(&device, &cfg, 3840, 2160) {
        eprintln!("device cannot hold a 4K grid, skipping: {why}");
        return;
    }
    let t0 = std::time::Instant::now();
    let mut r = SimRenderer::new(&device, &cfg, 3840, 2160);
    r.render_still(&device, &queue, &cfg, &palette);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let secs = t0.elapsed().as_secs_f64();
    println!("4K grid, 10,000 steps: {secs:.1} s ({:.2} ms/step)", secs * 1000.0 / 10_000.0);
    assert_eq!(r.step_index(), 10_000, "every step must have run");

    // The field must still be a field: a watchdog reset or a lost
    // device shows up here as NaN or a uniform image, not as an error.
    let out = read_rgba32f(&device, &queue, r.output_texture(), 3840, 2160);
    assert!(
        out.iter().all(|p| p.iter().all(|v| v.is_finite())),
        "4K export produced non-finite pixels"
    );
    let lo = out.iter().map(|p| p[0]).fold(f32::INFINITY, f32::min);
    let hi = out.iter().map(|p| p[0]).fold(f32::NEG_INFINITY, f32::max);
    assert!(hi - lo > 0.05, "4K export is a flat image ({lo}..{hi})");
}

/// Where an interactive frame's time actually goes.
#[test]
#[ignore = "diagnostic"]
fn frame_cost_breakdown_at_1080p() {
    let Some((device, queue)) = repro_device() else {
        return;
    };
    let palette = test_palette(&device, &queue);
    let mut cfg = SimConfig::default();
    cfg.grid = crate::config::sim::SimGrid::Viewport { scale: 1.0 };
    let (w, h) = (1920u32, 1080u32);
    let mut r = SimRenderer::new(&device, &cfg, w, h);
    r.seed(&device, &queue, &cfg);
    for _ in 0..3 {
        r.render_frame(&device, &queue, &cfg, &palette, 4);
    }
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });

    let mut report = |label: &str, n: f64, secs: f64| {
        println!("{label:<38} {:8.3} ms", secs * 1000.0 / n);
    };

    let t = std::time::Instant::now();
    r.run_steps(&device, &queue, &cfg, 240);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    report("240 steps, one call, per step", 240.0, t.elapsed().as_secs_f64());

    let t = std::time::Instant::now();
    for _ in 0..60 {
        r.run_steps(&device, &queue, &cfg, 4);
    }
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    report("4 steps x 60 calls, per step", 240.0, t.elapsed().as_secs_f64());

    let t = std::time::Instant::now();
    for _ in 0..60 {
        r.color(&device, &queue, &cfg, &palette);
    }
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    report("colour pass alone, per call", 60.0, t.elapsed().as_secs_f64());

    let t = std::time::Instant::now();
    for _ in 0..60 {
        r.render_frame(&device, &queue, &cfg, &palette, 0);
    }
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    report("render_frame with 0 steps, per frame", 60.0, t.elapsed().as_secs_f64());

    let t = std::time::Instant::now();
    for _ in 0..60 {
        r.render_frame(&device, &queue, &cfg, &palette, 4);
    }
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    report("render_frame with 4 steps, per frame", 60.0, t.elapsed().as_secs_f64());
}

/// Does the boundary mode's address arithmetic cost anything?
///
/// Periodic does four integer modulos per neighbour read -- 32 per
/// cell -- and an interior fast-path would remove them for every cell
/// not on the border. Whether that is worth its complexity depends on
/// whether the modulos are visible at all against a bandwidth-bound
/// kernel, which is a measurement rather than an argument.
#[test]
#[ignore = "diagnostic"]
fn boundary_mode_step_cost_at_1080p() {
    let Some((device, queue)) = repro_device() else {
        return;
    };
    let (w, h) = (1920u32, 1080u32);
    for boundary in [
        SimBoundary::Clamp,
        SimBoundary::Periodic,
        SimBoundary::Zero,
        SimBoundary::Mirror,
    ] {
        let mut cfg = SimConfig::default();
        cfg.grid = crate::config::sim::SimGrid::Viewport { scale: 1.0 };
        cfg.boundary = boundary;
        let mut r = SimRenderer::new(&device, &cfg, w, h);
        r.seed(&device, &queue, &cfg);
        r.run_steps(&device, &queue, &cfg, 64);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let t = std::time::Instant::now();
        r.run_steps(&device, &queue, &cfg, 1000);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        println!(
            "{:<10} {:7.4} ms/step",
            format!("{boundary:?}"),
            t.elapsed().as_secs_f64() * 1000.0 / 1000.0
        );
    }
}
