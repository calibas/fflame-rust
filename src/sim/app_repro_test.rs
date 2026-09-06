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

/// The Ising model must reproduce the phase transition quantitatively,
/// and it is checked against an EXACT result rather than a screenshot.
///
/// The observable is the nearest-neighbour correlation, not the
/// magnetisation. Magnetisation is the obvious choice and the wrong
/// one: it is a global quantity that equilibrates by domain
/// coarsening, so at 600 sweeps on a 128 lattice it was measured at
/// 0.090 for T = 1.5 -- below its own critical value, purely because
/// the lattice was sitting in a multi-domain state. Left running it
/// reaches 0.985, so the dynamics were right and the observable was
/// slow. Correlation is local, equilibrates within ~100 sweeps, and is
/// monotonic in temperature.
///
/// At T_c the 2-D square-lattice correlation is exactly 1/sqrt(2)
/// (Onsager), which is a real reference to check against. Measured
/// here across 100-20,000 sweeps: 0.679-0.728, centred on 0.707.
///
/// A broken checkerboard -- updating both sublattices at once -- still
/// renders plausible domains while giving the wrong statistics, so a
/// baseline image cannot catch it and this can.
#[test]
fn ising_matches_onsagers_correlation_across_the_transition() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: u32 = 128;
    // 100 sweeps: the correlation is flat from there to 20,000.
    const STEPS: u32 = 200;

    let correlation = |t: f32| -> f64 {
        let mut cfg = SimConfig::default();
        cfg.model = "ising".into();
        cfg.grid = crate::config::sim::SimGrid::Fixed { width: N, height: N };
        cfg.init = crate::config::sim::SimInit::Noise { amplitude: 1.0 };
        cfg.seed = 3;
        cfg.model_params.insert("temperature".into(), t);
        let mut r = SimRenderer::new(&device, &cfg, N, N);
        r.seed(&device, &queue, &cfg);
        r.run_steps(&device, &queue, &cfg, STEPS);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let f = read_rgba32f(&device, &queue, r.field_texture(), N, N);
        for px in &f {
            assert!(
                px[0] == 1.0 || px[0] == -1.0,
                "spin {} is not +/-1 at T = {t}",
                px[0]
            );
        }
        let n = N as usize;
        let mut corr = 0.0f64;
        for y in 0..n {
            for x in 0..n {
                let sc = f[y * n + x][0] as f64;
                corr += sc * f[y * n + (x + 1) % n][0] as f64;
                corr += sc * f[((y + 1) % n) * n + x][0] as f64;
            }
        }
        corr / (2 * n * n) as f64
    };

    let cold = correlation(1.5);
    let critical = correlation(2.269);
    let hot = correlation(3.5);
    const ONSAGER: f64 = std::f64::consts::FRAC_1_SQRT_2;
    println!(
        "Ising <s_i s_j>: T=1.5 {cold:.3}   T_c {critical:.3} (exact {ONSAGER:.3})   \
         T=3.5 {hot:.3}"
    );

    assert!(cold > 0.90, "T = 1.5 should be strongly correlated, got {cold:.3}");
    assert!(
        (critical - ONSAGER).abs() < 0.05,
        "at T_c the correlation should be Onsager's {ONSAGER:.4}, got {critical:.3}"
    );
    assert!(
        (0.25..0.45).contains(&hot),
        "T = 3.5 should be weakly correlated but not free, got {hot:.3}"
    );
    assert!(
        cold > critical && critical > hot,
        "correlation must fall monotonically with temperature: \
         {cold:.3} / {critical:.3} / {hot:.3}"
    );
}

/// A cyclic CA must actually cycle: every state occupied, and the
/// field still changing after it has developed.
///
/// Cheap, and it catches the two ways this rule dies silently -- a
/// wrong modulo freezes it on one state, and a wrong threshold
/// comparison makes every cell advance every step, which looks like
/// motion but is just a global counter.
#[test]
fn cyclic_ca_occupies_every_state_and_keeps_moving() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: u32 = 128;
    let mut cfg = SimConfig::default();
    cfg.model = "cyclic_ca".into();
    cfg.grid = crate::config::sim::SimGrid::Fixed { width: N, height: N };
    cfg.init = crate::config::sim::SimInit::Noise { amplitude: 1.0 };
    cfg.seed = 5;
    let states = 14usize;

    let mut r = SimRenderer::new(&device, &cfg, N, N);
    r.seed(&device, &queue, &cfg);
    r.run_steps(&device, &queue, &cfg, 300);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let a = read_rgba32f(&device, &queue, r.field_texture(), N, N);

    let mut seen = vec![0usize; states];
    for px in &a {
        let v = px[0];
        assert!(
            v >= 0.0 && v < states as f32 && v.fract() == 0.0,
            "state {v} is outside 0..{states} or not an integer"
        );
        seen[v as usize] += 1;
    }
    assert!(
        seen.iter().all(|&c| c > 0),
        "every state should be occupied after 300 steps: {seen:?}"
    );

    r.run_steps(&device, &queue, &cfg, 1);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let b = read_rgba32f(&device, &queue, r.field_texture(), N, N);
    let changed = a.iter().zip(&b).filter(|(x, y)| x[0] != y[0]).count();
    assert!(changed > 0, "a spiralled cyclic CA must keep advancing");
    // NOT an upper bound on `changed`. A first version asserted that
    // fewer than all cells advance, reasoning that a threshold which
    // always passed would advance everything -- but phase 0 measured
    // the churn plateau for 1/1/14 at 0.986, so a mature spiral field
    // really does advance almost every cell every step. That is what
    // makes the spirals rotate, and the assertion was encoding an
    // expectation the measurement had already contradicted.
    //
    // The discriminator against a rule that always fires is the STATE
    // distribution checked above: it would turn the lattice into one
    // global counter, so every cell would hold the same value.
    let first = a[0][0];
    assert!(
        a.iter().any(|px| px[0] != first),
        "every cell holds the same state -- the rule has become a global counter"
    );
}

/// Is the Ising lattice coarsening, or stuck?
#[test]
#[ignore = "diagnostic"]
fn ising_coarsening_curve() {
    let Some((device, queue)) = repro_device() else { return; };
    const N: u32 = 128;
    for t in [1.5f32, 2.269, 3.5] {
        let mut cfg = SimConfig::default();
        cfg.model = "ising".into();
        cfg.grid = crate::config::sim::SimGrid::Fixed { width: N, height: N };
        cfg.init = crate::config::sim::SimInit::Noise { amplitude: 1.0 };
        cfg.seed = 3;
        cfg.model_params.insert("temperature".into(), t);
        let mut r = SimRenderer::new(&device, &cfg, N, N);
        r.seed(&device, &queue, &cfg);
        let mut line = format!("T={t:<6}");
        for target in [200u32, 600, 1200, 4000, 12000, 40000] {
            let have = r.step_index();
            r.run_steps(&device, &queue, &cfg, target - have);
            let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
            let f = read_rgba32f(&device, &queue, r.field_texture(), N, N);
            let n = N as usize;
            let mut corr = 0.0f64;
            for y in 0..n {
                for x in 0..n {
                    let sc = f[y * n + x][0] as f64;
                    corr += sc * f[y * n + (x + 1) % n][0] as f64;
                    corr += sc * f[((y + 1) % n) * n + x][0] as f64;
                }
            }
            corr /= (2 * n * n) as f64;
            line.push_str(&format!("  {}:{:.3}", target / 2, corr));
        }
        println!("{line}");
    }
}

/// Rule 90 must be Pascal's triangle mod 2, checked against
/// independently computed binomials.
///
/// The bit convention -- next state is bit (4*left + 2*self + right) --
/// is easy to get backwards, and a reversed one still produces
/// something that looks like a cellular automaton. The CPU prototype
/// checked this on 2,079 cells; this is the same check on the shader.
///
/// Only the first 64 generations are compared: rule 90 on a PERIODIC
/// lattice of width 2^k self-annihilates at t = 2^k, so past the point
/// where the triangle reaches the edge the diagram is the wrapped sum
/// rather than the binomial one. That is correct behaviour, not a bug,
/// and the comparison simply stops before the wrap.
#[test]
fn wolfram_rule_90_matches_binomials_mod_two() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: u32 = 256;
    const GENS: usize = 64;
    let mut cfg = SimConfig::default();
    cfg.model = "wolfram_eca".into();
    cfg.grid = crate::config::sim::SimGrid::Fixed { width: N, height: N };
    cfg.init = crate::config::sim::SimInit::Center;
    cfg.model_params.insert("rule".into(), 90.0);

    let mut r = SimRenderer::new(&device, &cfg, N, N);
    r.seed(&device, &queue, &cfg);
    r.run_steps(&device, &queue, &cfg, GENS as u32);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let f = read_rgba32f(&device, &queue, r.field_texture(), N, N);

    let n = N as usize;
    let centre = n / 2;
    let mut checked = 0usize;
    for t in 1..GENS {
        for d in -(t as i64)..=(t as i64) {
            if (t as i64 + d) % 2 != 0 {
                continue;
            }
            // C(t, k) mod 2 by Kummer's theorem: the binomial is odd
            // exactly when k's bits are a subset of t's. Computing the
            // binomial itself overflowed here -- C(63, 29) times 63 is
            // past u64, and in release that wraps silently, so the test
            // failed at generation 63 while the shader was right.
            let k = ((t as i64 + d) / 2) as u64;
            let want = if (t as u64 & k) == k { 1.0f32 } else { 0.0f32 };
            let x = (centre as i64 + d).rem_euclid(n as i64) as usize;
            let got = f[t * n + x][0];
            assert_eq!(
                got, want,
                "rule 90 at generation {t}, offset {d}: got {got}, binomial says {want}"
            );
            checked += 1;
        }
    }
    println!("rule 90: {checked} cells match C(t, k) mod 2");
    assert!(checked > 2000, "expected a few thousand comparisons, made {checked}");
}

/// Lateral sticking must actually change the physics, not just the
/// picture.
///
/// Ballistic deposition and random deposition are different
/// universality classes: without lateral sticking the columns are
/// independent and the interface width grows as sqrt(t); with it the
/// columns correlate and the width grows more slowly. Measured on the
/// CPU prototype at the same point, 2.84 against 10.59.
///
/// Getting the toggle backwards would still render a rough surface, so
/// this compares the two widths rather than eyeballing either.
#[test]
fn lateral_sticking_correlates_the_interface() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: u32 = 256;

    let width = |sideways: f32| -> f64 {
        let mut cfg = SimConfig::default();
        cfg.model = "ballistic_deposition".into();
        cfg.grid = crate::config::sim::SimGrid::Fixed { width: N, height: N };
        cfg.init = crate::config::sim::SimInit::Center;
        cfg.seed = 11;
        cfg.model_params.insert("sideways".into(), sideways);
        cfg.model_params.insert("p_drop".into(), 0.5);
        let mut r = SimRenderer::new(&device, &cfg, N, N);
        r.seed(&device, &queue, &cfg);
        r.run_steps(&device, &queue, &cfg, 200);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let f = read_rgba32f(&device, &queue, r.field_texture(), N, N);
        // Column heights live in .y of row 0.
        let h: Vec<f64> = (0..N as usize).map(|x| f[x][1] as f64).collect();
        let mean = h.iter().sum::<f64>() / h.len() as f64;
        (h.iter().map(|v| (v - mean) * (v - mean)).sum::<f64>() / h.len() as f64).sqrt()
    };

    let ballistic = width(1.0);
    let random = width(0.0);
    println!("interface width: ballistic {ballistic:.2}   random {random:.2}");
    assert!(
        ballistic > 0.0 && random > 0.0,
        "both variants must produce a rough interface, got {ballistic} and {random}"
    );
    assert!(
        random > ballistic * 1.5,
        "random deposition should be markedly rougher than ballistic at the same time \
         (uncorrelated columns): got {random:.2} against {ballistic:.2}"
    );
}

/// Percolation must label CONNECTED COMPONENTS, checked against a CPU
/// flood fill rather than against a previous run.
///
/// Two open cells must share a label exactly when they are connected
/// through open cells. That is a property no baseline image can check:
/// a labelling that leaks across a closed site, or that stops short of
/// converging, still renders as plausible coloured blobs.
///
/// It also measures how many steps convergence took, because the count
/// is the model's headline cost and phase 0 found it is NOT
/// self-averaging: at p_c a critical cluster's longest chemical path
/// varies four-fold between samples at one size.
#[test]
fn percolation_labels_match_a_cpu_flood_fill() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: usize = 64;
    let mut cfg = SimConfig::default();
    cfg.model = "percolation".into();
    cfg.grid = crate::config::sim::SimGrid::Fixed { width: N as u32, height: N as u32 };
    cfg.init = crate::config::sim::SimInit::Noise { amplitude: 1.0 };
    cfg.boundary = SimBoundary::Zero;
    cfg.seed = 9;

    let mut r = SimRenderer::new(&device, &cfg, N as u32, N as u32);
    r.seed(&device, &queue, &cfg);

    // Run until the labels stop moving, and report how long that took.
    let mut prev: Vec<u32> = Vec::new();
    let mut converged_at = None;
    for round in 1..=400u32 {
        r.run_steps(&device, &queue, &cfg, 1);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let f = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);
        let now: Vec<u32> = f.iter().map(|px| px[0].to_bits()).collect();
        if now == prev {
            converged_at = Some(round - 1);
            break;
        }
        prev = now;
    }
    let rounds = converged_at.expect("labels should stop changing within 400 rounds");
    println!("percolation at p_c converged in {rounds} rounds on {N}x{N} (with path compression)");

    let f = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);
    let open: Vec<bool> = f.iter().map(|px| px[1] > 0.5).collect();
    let label: Vec<f32> = f.iter().map(|px| px[0]).collect();

    // CPU connected components, four-neighbour, on the SAME open field
    // the GPU generated -- so this tests the labelling, not the RNG.
    let mut comp = vec![usize::MAX; N * N];
    let mut next = 0usize;
    for start in 0..N * N {
        if !open[start] || comp[start] != usize::MAX {
            continue;
        }
        let id = next;
        next += 1;
        let mut stack = vec![start];
        comp[start] = id;
        while let Some(i) = stack.pop() {
            let (x, y) = (i % N, i / N);
            let mut push = |nx: usize, ny: usize, st: &mut Vec<usize>, c: &mut Vec<usize>| {
                let j = ny * N + nx;
                if open[j] && c[j] == usize::MAX {
                    c[j] = id;
                    st.push(j);
                }
            };
            if x > 0 { push(x - 1, y, &mut stack, &mut comp); }
            if x + 1 < N { push(x + 1, y, &mut stack, &mut comp); }
            if y > 0 { push(x, y - 1, &mut stack, &mut comp); }
            if y + 1 < N { push(x, y + 1, &mut stack, &mut comp); }
        }
    }
    println!("  {next} components over {} open cells", open.iter().filter(|o| **o).count());

    // Same component => same label, and different component => different
    // label. Checked through a pair of maps so a single leak or a single
    // failure to merge is caught.
    use std::collections::HashMap;
    let mut comp_to_label: HashMap<usize, f32> = HashMap::new();
    let mut label_to_comp: HashMap<u32, usize> = HashMap::new();
    for i in 0..N * N {
        if !open[i] {
            continue;
        }
        let c = comp[i];
        let l = label[i];
        match comp_to_label.get(&c) {
            Some(&seen) => assert_eq!(
                seen.to_bits(),
                l.to_bits(),
                "component {c} has two labels ({seen} and {l}): it did not fully merge"
            ),
            None => {
                comp_to_label.insert(c, l);
            }
        }
        match label_to_comp.get(&l.to_bits()) {
            Some(&seen) => assert_eq!(
                seen, c,
                "label {l} spans components {seen} and {c}: it leaked across a closed site"
            ),
            None => {
                label_to_comp.insert(l.to_bits(), c);
            }
        }
    }
    assert!(next > 20, "expected many clusters at p_c, found {next}");
}

/// What path compression is worth, measured rather than asserted.
///
/// Plain propagation moves a label one cell per step, so it costs the
/// cluster's longest chemical path -- phase 0 measured a median 645
/// rounds at 256² and 1,409 at 512². Reading the cell a label points at
/// short-circuits that.
#[test]
#[ignore = "diagnostic"]
fn percolation_convergence_against_grid_size() {
    let Some((device, queue)) = repro_device() else {
        return;
    };
    for n in [64u32, 128, 256, 512] {
        let mut cfg = SimConfig::default();
        cfg.model = "percolation".into();
        cfg.grid = crate::config::sim::SimGrid::Fixed { width: n, height: n };
        cfg.init = crate::config::sim::SimInit::Noise { amplitude: 1.0 };
        cfg.boundary = SimBoundary::Zero;
        cfg.seed = 9;
        let mut r = SimRenderer::new(&device, &cfg, n, n);
        r.seed(&device, &queue, &cfg);
        let mut prev: Vec<u32> = Vec::new();
        let mut rounds = 0;
        for round in 1..=3000u32 {
            r.run_steps(&device, &queue, &cfg, 1);
            let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
            let f = read_rgba32f(&device, &queue, r.field_texture(), n, n);
            let now: Vec<u32> = f.iter().map(|px| px[0].to_bits()).collect();
            if now == prev {
                rounds = round - 1;
                break;
            }
            prev = now;
        }
        println!("{n}x{n}: converged in {rounds} rounds");
    }
}

/// Review probe: every model's step cost at 1080p, at its defaults,
/// plus the two heavy kernels at their slider extremes. Diagnostic --
/// run with `--test-threads=1` or the numbers are contaminated.
///
/// `SIM_PROBE_ONLY=<name prefix>` / `SIM_PROBE_SKIP=<prefix>` select
/// cases and `SIM_PROBE_STEPS=<n>` overrides the step count. Those
/// knobs are how the watchdog bug was bisected: the poll time is
/// printed because a run that takes the SAME time at 256 and 512
/// steps has been cut off by the 2 s GPU watchdog, and the ms/step it
/// reports is then fiction (R = 5 read 4.7 that way; it is 9.7).
#[test]
#[ignore = "diagnostic"]
fn phase2_review_step_cost_per_model() {
    let Some((device, queue)) = repro_device() else {
        return;
    };
    const W: u32 = 1920;
    const H: u32 = 1080;
    let steps: u32 = std::env::var("SIM_PROBE_STEPS").ok().and_then(|v| v.parse().ok()).unwrap_or(512);
    let mut cases: Vec<(String, SimConfig)> = Vec::new();
    for m in crate::sim::MODELS {
        let mut cfg = SimConfig::default();
        cfg.model = m.name.into();
        cfg.grid = crate::config::sim::SimGrid::Fixed { width: W, height: H };
        cfg.init = crate::config::sim::SimInit::Noise { amplitude: 1.0 };
        cases.push((m.name.to_string(), cfg));
    }
    {
        let mut cfg = SimConfig::default();
        cfg.model = "cyclic_ca".into();
        cfg.grid = crate::config::sim::SimGrid::Fixed { width: W, height: H };
        cfg.init = crate::config::sim::SimInit::Noise { amplitude: 1.0 };
        cfg.model_params.insert("range".into(), 5.0);
        cfg.model_params.insert("neighbourhood".into(), 1.0);
        cases.push(("cyclic_ca R=5 Moore".into(), cfg));
    }
    let only = std::env::var("SIM_PROBE_ONLY").ok();
    let skip = std::env::var("SIM_PROBE_SKIP").ok();
    for (name, cfg) in cases {
        if let Some(o) = &only { if !name.starts_with(o.as_str()) { continue; } }
        if let Some(k) = &skip { if name.starts_with(k.as_str()) { continue; } }
        let mut r = SimRenderer::new(&device, &cfg, W, H);
        r.seed(&device, &queue, &cfg);
        // Warm: compile + first batch.
        r.run_steps(&device, &queue, &cfg, steps.min(64));
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let t0 = std::time::Instant::now();
        r.run_steps(&device, &queue, &cfg, steps);
        let polled = device.poll(PollType::Wait { submission_index: None, timeout: None });
        eprintln!("poll after {steps} steps: {polled:?} ({:.3} s)", t0.elapsed().as_secs_f64());
        let ms = t0.elapsed().as_secs_f64() * 1e3 / steps as f64;
        println!("{name:<24} {ms:.4} ms/step at 1080p");
    }
}

/// Every reaction-diffusion model must stay stable with its diffusion
/// sliders at their MAXIMA and dt at the cap the engine enforces there.
///
/// The cap used to be `max_dt` alone, measured at the default diffusion
/// rates. Explicit Euler's diffusion bound scales as 1/D, and the
/// sliders reach 4-5x the defaults: at Brusselator D_Y = 40 under the
/// 0.04 cap, dt·D·1.6 = 2.56 > 2. Measured before the fix, on 128²
/// after 200 steps: Brusselator infinite in 8,172 of 16,384 cells,
/// Schnakenberg in 8,137, and FitzHugh-Nagumo railed at ±3 by its
/// clamp with a checkerboard of rms 5.1 -- a lattice of rails rather
/// than a NaN, so nothing else catches it. Gray-Scott's slider maximum
/// IS its default, and at exactly the bound (dt·D·1.6 = 2.00) it held
/// a 0.445-rms checkerboard in its [0,1] clamp; that is why the cap
/// carries a 0.96 margin.
///
/// A diffusion-only cap (`1.2 / D`) was tried first and FitzHugh-Nagumo
/// still railed under it (rms 3.08 at dt = 0.3, D = 4): the reaction
/// term's stiffness adds to the stencil's, which is what the cap now
/// accounts for.
///
/// The observable is the checkerboard (Nyquist) mode's AMPLITUDE, the
/// alternating-sign mean of the field, sampled at 200 and 400 steps.
/// It is the eigenvector explicit Euler amplifies first, so a run past
/// the bound grows it geometrically whatever the reaction terms; a
/// stable run leaves it at rounding level. Neighbour-difference rms was
/// tried first and rejected as the observable: a legitimate Turing
/// pattern at high D has fine structure too, and the Brusselator at
/// D_Y = 40 read 0.38 on that measure while being stable -- the
/// alternating mean distinguishes the two, a pattern's cancels.
#[test]
fn rd_models_stay_stable_at_the_diffusion_slider_maxima() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: u32 = 128;
    let mut checked = 0;
    for m in crate::sim::MODELS {
        if m.diffusion.is_empty() {
            continue;
        }
        let mut cfg = SimConfig::default();
        cfg.model = m.name.into();
        cfg.grid = crate::config::sim::SimGrid::Fixed { width: N, height: N };
        cfg.init = crate::config::sim::SimInit::Noise { amplitude: 1.0 };
        for name in m.diffusion {
            let def = m.parameters.iter().find(|p| p.name == *name).unwrap();
            cfg.model_params.insert(def.name.into(), def.max);
        }
        // Ask for far more than the cap; the engine must clamp.
        cfg.dt = m.max_dt;
        let cap = m.max_dt_for(&cfg.model_params);
        assert!(cap > 0.0 && cap <= m.max_dt, "{}: cap {cap} vs declared {}", m.name, m.max_dt);

        let mut r = SimRenderer::new(&device, &cfg, N, N);
        r.seed(&device, &queue, &cfg);
        let n = N as usize;
        // Nyquist amplitude of channel x, and the field's scale to
        // judge it against.
        let nyquist = |f: &[[f32; 4]]| -> (f64, f64, usize) {
            let mut alt = 0.0f64;
            let mut mag = 0.0f64;
            let mut nonfinite = 0;
            for y in 0..n {
                for x in 0..n {
                    let v = f[y * n + x][0] as f64;
                    if !v.is_finite() || !f[y * n + x][1].is_finite() {
                        nonfinite += 1;
                        continue;
                    }
                    alt += if (x + y) % 2 == 0 { v } else { -v };
                    mag += v.abs();
                }
            }
            (alt / (n * n) as f64, mag / (n * n) as f64, nonfinite)
        };
        r.run_steps(&device, &queue, &cfg, 200);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let (a200, mag, nf200) = nyquist(&read_rgba32f(&device, &queue, r.field_texture(), N, N));
        r.run_steps(&device, &queue, &cfg, 200);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let (a400, _, nf400) = nyquist(&read_rgba32f(&device, &queue, r.field_texture(), N, N));
        // A third sample, because a Turing pattern forming from noise
        // also raises the alternating mean a little (the Brusselator
        // read 2e-6 -> 4e-4 between 200 and 400 while stable); an
        // unstable mode at even 2% a step would be at the rails by 800.
        r.run_steps(&device, &queue, &cfg, 400);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let (a800, mag800, nf800) =
            nyquist(&read_rgba32f(&device, &queue, r.field_texture(), N, N));
        println!(
            "{:<14} cap {:.4} (declared {})  nonfinite {}  |x| {:.3}  nyquist {:.2e} -> {:.2e} -> {:.2e}",
            m.name,
            cap,
            m.max_dt,
            nf200 + nf400 + nf800,
            mag,
            a200,
            a400,
            a800
        );
        assert_eq!(nf200 + nf400 + nf800, 0, "{}: non-finite cells at the slider maxima", m.name);
        // Rounding level relative to the field, and not growing.
        assert!(
            a800.abs() < 1e-3 * mag800.max(1e-6),
            "{}: Nyquist amplitude {a800:.2e} against a field of {mag800:.3} -- the cap is not a cap",
            m.name
        );
        checked += 1;
    }
    assert_eq!(checked, 4, "expected the four reaction-diffusion models");
}

/// A slow kernel must never lose the device to the GPU watchdog.
///
/// Cyclic CA at range 5 is 121 reads a cell and 9.7 ms a step at
/// 1080p. With a fixed 256-step submission that was 2.5 s in one
/// command buffer, past Windows' 2 s watchdog: the device reset, the
/// fence signalled anyway, and the shipped binary's `export` of this
/// config failed with "Parent device is lost". The submission size is
/// now measured; this runs the reproduction and asks the device
/// whether it survived.
///
/// About 2.5 s of GPU time on the card this was measured on.
#[test]
fn a_slow_kernel_never_trips_the_gpu_watchdog() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;
    let lost = Arc::new(AtomicBool::new(false));
    {
        let lost = lost.clone();
        device.set_device_lost_callback(Box::new(move |reason, msg| {
            eprintln!("device lost: {reason:?}: {msg}");
            lost.store(true, Ordering::SeqCst);
        }));
    }
    const W: u32 = 1920;
    const H: u32 = 1080;
    let mut cfg = SimConfig::default();
    cfg.model = "cyclic_ca".into();
    cfg.grid = crate::config::sim::SimGrid::Fixed { width: W, height: H };
    cfg.init = crate::config::sim::SimInit::Noise { amplitude: 1.0 };
    cfg.model_params.insert("range".into(), 5.0);
    cfg.model_params.insert("neighbourhood".into(), 1.0);

    let mut r = SimRenderer::new(&device, &cfg, W, H);
    r.seed(&device, &queue, &cfg);
    let started = std::time::Instant::now();
    // The count that lost the device: one fixed-size submission's worth.
    r.run_steps(&device, &queue, &cfg, 256);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let secs = started.elapsed().as_secs_f64();
    let f = read_rgba32f(&device, &queue, r.field_texture(), W, H);
    let bad = f.iter().filter(|px| !(px[0] >= 0.0 && px[0] < 14.0)).count();
    println!("256 steps of range-5 cyclic CA at 1080p: {secs:.2} s, {bad} invalid cells");
    assert!(!lost.load(Ordering::SeqCst), "the device was lost: the submissions are too long");
    assert_eq!(bad, 0, "invalid states after the run");
}

/// The two-pass machinery itself, against a CPU mirror of one step.
///
/// A fourth-order model is two dispatches, and the thing that can go
/// wrong is the ORDERING: if pass 2 read the field pass 1 was written
/// from rather than the one it wrote, the result is still a smooth
/// evolving field that looks like a PDE. This mirrors both passes on
/// the CPU -- including the intermediate stored in `.y` -- so a
/// ping-pong that lost a swap cannot pass.
#[test]
fn cahn_hilliard_matches_a_cpu_mirror_through_both_passes() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: usize = 64;
    let mut cfg = SimConfig::default();
    cfg.model = "cahn_hilliard".into();
    cfg.grid = crate::config::sim::SimGrid::Fixed { width: N as u32, height: N as u32 };
    cfg.init = crate::config::sim::SimInit::Noise { amplitude: 1.0 };
    cfg.boundary = SimBoundary::Periodic;
    cfg.seed = 5;
    cfg.dt = 0.04;

    let mut r = SimRenderer::new(&device, &cfg, N as u32, N as u32);
    r.seed(&device, &queue, &cfg);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let start = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);

    // Three steps: enough that a one-step-late read would drift well
    // past tolerance, few enough that f32 rounding has not compounded.
    const STEPS: u32 = 3;
    r.run_steps(&device, &queue, &cfg, STEPS);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let got = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);

    let (d, gamma) = (1.0f32, 0.5f32);
    let dt = 0.04f32;
    let mut c: Vec<f32> = start.iter().map(|px| px[0]).collect();
    let lap = |f: &[f32], x: usize, y: usize| -> f32 {
        let l = f[y * N + (x + N - 1) % N];
        let rr = f[y * N + (x + 1) % N];
        let u = f[((y + N - 1) % N) * N + x];
        let dn = f[((y + 1) % N) * N + x];
        l + rr + u + dn - 4.0 * f[y * N + x]
    };
    for _ in 0..STEPS {
        // Pass 1: the chemical potential, into its own array -- which
        // is exactly what the .y channel is on the GPU.
        let mut mu = vec![0.0f32; N * N];
        for y in 0..N {
            for x in 0..N {
                let v = c[y * N + x];
                mu[y * N + x] = v * v * v - v - gamma * lap(&c, x, y);
            }
        }
        // Pass 2: reads the potential every cell just wrote.
        let mut next = vec![0.0f32; N * N];
        for y in 0..N {
            for x in 0..N {
                next[y * N + x] = (c[y * N + x] + dt * d * lap(&mu, x, y)).clamp(-4.0, 4.0);
            }
        }
        c = next;
    }

    let mut worst = 0.0f32;
    for i in 0..N * N {
        worst = worst.max((c[i] - got[i][0]).abs());
    }
    println!("Cahn-Hilliard {STEPS} steps vs CPU mirror: worst |delta| = {worst:.3e}");
    assert!(
        worst < 1e-5,
        "GPU and CPU disagree by {worst:.3e} after {STEPS} two-pass steps"
    );
}

/// Cahn-Hilliard must conserve the mean composition EXACTLY.
///
/// The update is a discrete divergence: a Laplacian sums to zero over
/// a periodic lattice, so the mean cannot move except by rounding.
/// That is the equation's physical content -- material is transported,
/// not created -- and it is invisible in a picture, because a field
/// that slowly gains material still separates into plausible domains.
/// The CPU prototype holds it to 1.2e-16 in f64 over 40,000 steps;
/// f32 on the GPU is looser, and the tolerance below is against the
/// per-step rounding rather than against zero.
#[test]
fn cahn_hilliard_conserves_the_mean_composition() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: u32 = 128;
    for mean in [0.0f32, 0.4] {
        let mut cfg = SimConfig::default();
        cfg.model = "cahn_hilliard".into();
        cfg.grid = crate::config::sim::SimGrid::Fixed { width: N, height: N };
        cfg.init = crate::config::sim::SimInit::Noise { amplitude: 1.0 };
        cfg.boundary = SimBoundary::Periodic;
        cfg.seed = 5;
        cfg.model_params.insert("mean".into(), mean);
        let mut r = SimRenderer::new(&device, &cfg, N, N);
        r.seed(&device, &queue, &cfg);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let m0 = {
            let f = read_rgba32f(&device, &queue, r.field_texture(), N, N);
            f.iter().map(|px| px[0] as f64).sum::<f64>() / f.len() as f64
        };
        r.run_steps(&device, &queue, &cfg, 4000);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let f = read_rgba32f(&device, &queue, r.field_texture(), N, N);
        let m1 = f.iter().map(|px| px[0] as f64).sum::<f64>() / f.len() as f64;
        let sd = {
            let mu = m1;
            (f.iter().map(|px| (px[0] as f64 - mu).powi(2)).sum::<f64>() / f.len() as f64).sqrt()
        };
        println!(
            "Cahn-Hilliard mean {mean}: {m0:.6} -> {m1:.6}, drift {:.2e}, sd {sd:.4}",
            (m1 - m0).abs()
        );
        // It must have actually separated, or conservation is trivial.
        assert!(sd > 0.5, "mean {mean}: the field did not separate (sd {sd:.4})");
        assert!(
            (m1 - m0).abs() < 2e-4,
            "mean {mean}: composition drifted by {:.2e} over 4,000 steps -- the update \
             is not in divergence form",
            (m1 - m0).abs()
        );
    }
}

/// Swift-Hohenberg must select the wavelength it advertises.
///
/// `lambda = 2*pi/q0` is the model's whole claim and the thing the
/// discretisation is most likely to break: the Sims kernel the other
/// models use is a Laplacian scaled by 0.3, which would move the
/// selected wavelength by 1/sqrt(0.3) -- 83% wrong, and still a
/// perfectly attractive picture. So this measures the wavelength and
/// checks it TRACKS the parameter.
///
/// The observable is zero crossings along rows: a band-limited field
/// crosses its mean twice per wavelength, so a line scan gives
/// `2 * length / crossings` with no FFT and no sensitivity to
/// amplitude. A line scan of an ISOTROPIC 2-D pattern reads long,
/// because a row cuts most of the pattern's wavefronts obliquely and
/// sees `k cos(theta)` rather than `k` -- sqrt(2) for a Gaussian
/// random field, and measured at 1.58-1.72 here across the wavelengths
/// where the field has converged. The test pins that band, which is
/// what makes it discriminating: the Sims kernel would multiply every
/// wavelength by 1/sqrt(0.3) = 1.83 and put the ratio near 2.9.
///
/// Only short wavelengths are checked, and that is not arbitrary. The
/// pattern grows on a 1/r timescale and r is `drive * q0^4`, so
/// doubling the wavelength costs SIXTEEN times the steps: measured at
/// 12,000 steps, lambda = 10 reaches sd 0.45 and lambda = 32 only
/// 0.012 -- still seed noise, and its apparent wavelength is the
/// noise's, not the model's.
#[test]
fn swift_hohenberg_selects_the_wavelength_it_advertises() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: u32 = 256;
    let mut measured = Vec::new();
    for target in [10.0f32, 12.0, 16.0] {
        let mut cfg = SimConfig::default();
        cfg.model = "swift_hohenberg".into();
        cfg.grid = crate::config::sim::SimGrid::Fixed { width: N, height: N };
        cfg.init = crate::config::sim::SimInit::Noise { amplitude: 1.0 };
        cfg.boundary = SimBoundary::Periodic;
        cfg.seed = 7;
        cfg.model_params.insert("wavelength".into(), target);
        cfg.model_params.insert("drive".into(), 2.0);
        let mut r = SimRenderer::new(&device, &cfg, N, N);
        r.seed(&device, &queue, &cfg);
        // Long wavelengths grow on a 1/r timescale and r falls as the
        // fourth power of 1/lambda, so the slower one sets the count.
        r.run_steps(&device, &queue, &cfg, 12000);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let f = read_rgba32f(&device, &queue, r.field_texture(), N, N);

        let n = N as usize;
        let mean = f.iter().map(|px| px[0] as f64).sum::<f64>() / f.len() as f64;
        let sd = (f.iter().map(|px| (px[0] as f64 - mean).powi(2)).sum::<f64>()
            / f.len() as f64)
            .sqrt();
        let mut crossings = 0usize;
        for y in 0..n {
            for x in 0..n {
                let a = f[y * n + x][0] as f64 - mean;
                let b = f[y * n + (x + 1) % n][0] as f64 - mean;
                if (a < 0.0) != (b < 0.0) {
                    crossings += 1;
                }
            }
        }
        let lambda = 2.0 * (n * n) as f64 / crossings.max(1) as f64;
        println!(
            "Swift-Hohenberg wavelength {target}: line-scan {lambda:.2} cells, \
             ratio {:.3}, sd {sd:.4}",
            lambda / target as f64
        );
        assert!(
            sd > 0.15,
            "wavelength {target}: the field has not converged (sd {sd:.4}) -- the \
             measurement below would be reading seed noise"
        );
        let ratio = lambda / target as f64;
        assert!(
            (1.35..1.95).contains(&ratio),
            "wavelength {target}: line-scan/advertised is {ratio:.3}, outside the \
             measured 1.6 line-scan bias -- the Laplacian or its scale is wrong \
             (the Sims kernel would put this near 2.9)"
        );
        measured.push(lambda);
    }
    // And it must TRACK the parameter, not merely sit in the band:
    // a model that ignored the slider entirely would pass every check
    // above at one wavelength and fail this one.
    let tracked = measured[2] / measured[0];
    println!("Swift-Hohenberg tracking: 16/10 measured as {tracked:.3} (advertised 1.6)");
    assert!(
        (tracked - 1.6).abs() < 0.32,
        "the selected wavelength does not track the parameter: 16/10 came out {tracked:.3}"
    );
}

/// Kobayashi's anisotropy must set the crystal's SYMMETRY, and the
/// six-fold preset must actually be six-fold.
///
/// That is the model's whole visual claim and the thing a wrong `j`, a
/// wrong `theta0` or a broken `eps'` term would silently change --
/// every one of those still grows a confident-looking crystal.
///
/// The observable is the crystal's REACH as a function of angle,
/// reduced to its angular harmonics. Counting solid arcs around a
/// circle was tried first and rejected: by 4,000 steps the arms have
/// side branches, and a circle at any radius large enough to reach the
/// arms cuts through those too -- it read 16 "arms" on a crystal that
/// is plainly four-fold. The harmonics do not care, because side
/// branches are high-frequency detail riding on a low-frequency shape,
/// and the dominant low harmonic IS the symmetry.
#[test]
fn kobayashi_grows_the_symmetry_its_parameter_asks_for() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: u32 = 300;
    // (mode index, theta0, expected symmetry)
    for (mode, theta0, want) in [(1.0f32, 0.0f32, 4usize), (2.0, std::f32::consts::FRAC_PI_2, 6)] {
        let mut cfg = SimConfig::default();
        cfg.model = "kobayashi".into();
        cfg.grid = crate::config::sim::SimGrid::Fixed { width: N, height: N };
        cfg.init = crate::config::sim::SimInit::Blob { radius: 4 };
        cfg.boundary = SimBoundary::Clamp;
        cfg.seed = 7;
        cfg.dt = 1.0e-4;
        cfg.model_params.insert("latent_heat".into(), 1.6);
        cfg.model_params.insert("delta".into(), 0.04);
        cfg.model_params.insert("mode".into(), mode);
        cfg.model_params.insert("theta0".into(), theta0);

        let mut r = SimRenderer::new(&device, &cfg, N, N);
        r.seed(&device, &queue, &cfg);
        r.run_steps(&device, &queue, &cfg, 4000);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let f = read_rgba32f(&device, &queue, r.field_texture(), N, N);

        let n = N as usize;
        let c = (n / 2) as f64;
        // How far the solid reaches in each angular bin.
        const BINS: usize = 360;
        let mut reach = [0.0f64; BINS];
        for y in 0..n {
            for x in 0..n {
                if f[y * n + x][0] <= 0.5 {
                    continue;
                }
                let (dx, dy) = (x as f64 - c, y as f64 - c);
                let d = (dx * dx + dy * dy).sqrt();
                if d < 1.0 {
                    continue;
                }
                let a = dy.atan2(dx).rem_euclid(std::f64::consts::TAU);
                let b = ((a / std::f64::consts::TAU) * BINS as f64) as usize % BINS;
                if d > reach[b] {
                    reach[b] = d;
                }
            }
        }
        // NOT "solid in every direction": a four-fold crystal's
        // diagonals are liquid all the way to the centre, and a zero
        // there is the shape rather than a failure. What must hold is
        // that a crystal grew at all.
        let lit = reach.iter().filter(|r| **r > 2.0).count();
        let far = reach.iter().cloned().fold(0.0f64, f64::max);
        assert!(
            lit > BINS / 3 && far > 20.0,
            "mode {mode}: no crystal to measure ({lit} of {BINS} directions occupied,              furthest reach {far:.1})"
        );

        // Angular harmonics of the reach. Harmonic k is a k-fold shape.
        let mut best = (0usize, 0.0f64);
        let mut amps = Vec::new();
        for k in 1..=12usize {
            let (mut re, mut im) = (0.0f64, 0.0f64);
            for (b, rad) in reach.iter().enumerate() {
                let a = b as f64 / BINS as f64 * std::f64::consts::TAU * k as f64;
                re += rad * a.cos();
                im += rad * a.sin();
            }
            let amp = (re * re + im * im).sqrt() / BINS as f64;
            amps.push(amp);
            if amp > best.1 {
                best = (k, amp);
            }
        }
        println!(
            "Kobayashi mode index {mode}: dominant angular harmonic {} (amplitude {:.2}); \
             k=4 {:.2}, k=6 {:.2}",
            best.0, best.1, amps[3], amps[5]
        );
        assert_eq!(
            best.0, want,
            "expected {want}-fold symmetry, the dominant harmonic is {}-fold -- the \
             anisotropy is not doing what its symmetry parameter says",
            best.0
        );
    }
}

/// Kobayashi must not checkerboard, which pins the staggered
/// discretisation.
///
/// The obvious two-pass reading -- a central-difference gradient, then
/// a central-difference divergence -- composes to a stencil that skips
/// the immediate neighbour, so the odd and even sublattices decouple
/// and nothing damps the Nyquist mode. Measured on the CPU mirror,
/// that version filled the field with a diagonal checkerboard while
/// staying inside [0, 1] and finite: an `isfinite` check called it
/// stable and it produced a confident-looking picture. The staggered
/// forward/backward pair composes to the compact Laplacian instead.
#[test]
fn kobayashi_stays_free_of_the_checkerboard_mode() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: u32 = 128;
    let mut cfg = SimConfig::default();
    cfg.model = "kobayashi".into();
    cfg.grid = crate::config::sim::SimGrid::Fixed { width: N, height: N };
    cfg.init = crate::config::sim::SimInit::Blob { radius: 4 };
    cfg.boundary = SimBoundary::Clamp;
    cfg.seed = 7;
    cfg.dt = 1.0e-4;
    let mut r = SimRenderer::new(&device, &cfg, N, N);
    r.seed(&device, &queue, &cfg);
    r.run_steps(&device, &queue, &cfg, 3000);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let f = read_rgba32f(&device, &queue, r.field_texture(), N, N);

    let n = N as usize;
    let mut alt = 0.0f64;
    let mut solid = 0usize;
    for y in 0..n {
        for x in 0..n {
            let v = f[y * n + x][0] as f64;
            assert!(v.is_finite(), "non-finite phase at ({x}, {y})");
            alt += if (x + y) % 2 == 0 { v } else { -v };
            if v > 0.5 {
                solid += 1;
            }
        }
    }
    let nyquist = (alt / (n * n) as f64).abs();
    println!(
        "Kobayashi Nyquist amplitude {nyquist:.2e}, {:.1}% solid",
        solid as f64 / (n * n) as f64 * 100.0
    );
    assert!(solid > 100, "nothing grew, so there is nothing to check");
    assert!(
        nyquist < 1.0e-3,
        "Nyquist amplitude {nyquist:.2e}: the odd and even sublattices have decoupled"
    );
}

/// The Oregonator must carry a travelling WAVE, not a diffusing blob.
///
/// This is the discriminating measurement, because both look like an
/// expanding bright ring in a still image. A reaction-diffusion wave
/// front moves at constant speed, so its radius grows linearly in
/// time; pure diffusion spreads as sqrt(t). Measuring the radius at
/// three times separates them with no ambiguity, and it would catch a
/// reaction term that had been dropped or mis-signed -- which is
/// exactly the failure that still renders a plausible picture.
#[test]
fn the_oregonator_front_travels_at_constant_speed() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: u32 = 256;
    let mut cfg = SimConfig::default();
    cfg.model = "oregonator".into();
    cfg.grid = crate::config::sim::SimGrid::Fixed { width: N, height: N };
    cfg.init = crate::config::sim::SimInit::Blob { radius: 5 };
    cfg.boundary = SimBoundary::Periodic;
    cfg.seed = 7;
    cfg.dt = 1.0e-4;

    let mut r = SimRenderer::new(&device, &cfg, N, N);
    r.seed(&device, &queue, &cfg);
    let n = N as usize;
    let c = (n / 2) as f64;

    // Radius of the outermost excited cell, sampled as the wave runs.
    let mut radii = Vec::new();
    for _ in 0..3 {
        r.run_steps(&device, &queue, &cfg, 6000);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let f = read_rgba32f(&device, &queue, r.field_texture(), N, N);
        let mut far = 0.0f64;
        for y in 0..n {
            for x in 0..n {
                if f[y * n + x][0] > 0.3 {
                    let d = ((x as f64 - c).powi(2) + (y as f64 - c).powi(2)).sqrt();
                    far = far.max(d);
                }
            }
        }
        radii.push(far);
    }
    println!(
        "Oregonator front radius at 6k/12k/18k steps: {:.1}, {:.1}, {:.1} cells",
        radii[0], radii[1], radii[2]
    );
    assert!(radii[0] > 6.0, "the seed never fired (radius {:.1})", radii[0]);
    // Linear growth: equal increments. Diffusion would give sqrt(t),
    // whose second increment is 0.41 of the first.
    let first = radii[1] - radii[0];
    let second = radii[2] - radii[1];
    assert!(first > 5.0, "the front is not advancing ({first:.1} cells in 6,000 steps)");
    let ratio = second / first;
    println!("   increment ratio {ratio:.3} (a wave gives 1.0; diffusion gives 0.41)");
    assert!(
        (0.75..1.25).contains(&ratio),
        "the front's increments are {first:.1} then {second:.1} (ratio {ratio:.2}): \
         that is not a wave travelling at constant speed"
    );
}

/// Both hodgepodge rules, against a CPU mirror of the equations as
/// published.
///
/// The shipped rule was taken from a secondary source and carried a
/// `[verify]` flag for a year; reading Gerhardt and Schuster's own
/// paper showed it differs from theirs in THREE places -- k1 and k2
/// swapped, the sum taken over every cell rather than the infected
/// ones, and the divisor A + B + 1 rather than the infected count.
/// Every one of those still produces a field of plausible BZ scrolls,
/// which is exactly why a baseline image could not catch it and this
/// can.
///
/// The states are integers held in f32 and exact to 2^24, so the
/// comparison is for EQUALITY, not a tolerance.
#[test]
fn both_hodgepodge_rules_match_a_cpu_mirror_of_their_papers() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: usize = 64;
    const STEPS: u32 = 12;
    const Q: i64 = 200;
    const K1: i64 = 2;
    const K2: i64 = 3;
    const G: i64 = 70;

    let run = |variant: f32| -> (Vec<i64>, Vec<i64>) {
        let mut cfg = SimConfig::default();
        cfg.model = "hodgepodge".into();
        cfg.grid = crate::config::sim::SimGrid::Fixed { width: N as u32, height: N as u32 };
        cfg.init = crate::config::sim::SimInit::Noise { amplitude: 1.0 };
        cfg.boundary = SimBoundary::Periodic;
        cfg.seed = 4;
        cfg.model_params.insert("states".into(), Q as f32);
        cfg.model_params.insert("k1".into(), K1 as f32);
        cfg.model_params.insert("k2".into(), K2 as f32);
        cfg.model_params.insert("g".into(), G as f32);
        cfg.model_params.insert("variant".into(), variant);

        let mut r = SimRenderer::new(&device, &cfg, N as u32, N as u32);
        r.seed(&device, &queue, &cfg);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let start: Vec<i64> = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32)
            .iter()
            .map(|px| px[0] as i64)
            .collect();
        r.run_steps(&device, &queue, &cfg, STEPS);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let end: Vec<i64> = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32)
            .iter()
            .map(|px| px[0] as i64)
            .collect();
        (start, end)
    };

    // `paper` selects Gerhardt-Schuster eqs. (3)-(9); otherwise the
    // circulated variant.
    let mirror = |start: &[i64], paper: bool| -> Vec<i64> {
        let mut s = start.to_vec();
        for _ in 0..STEPS {
            let mut next = vec![0i64; N * N];
            for y in 0..N {
                for x in 0..N {
                    let cur = s[y * N + x];
                    let (mut ill, mut infected, mut all_sum, mut inf_sum) = (0i64, 0i64, cur, 0i64);
                    for dy in -1i64..=1 {
                        for dx in -1i64..=1 {
                            if dx == 0 && dy == 0 {
                                continue;
                            }
                            let nx = (x as i64 + dx).rem_euclid(N as i64) as usize;
                            let ny = (y as i64 + dy).rem_euclid(N as i64) as usize;
                            let n = s[ny * N + nx];
                            all_sum += n;
                            if n >= Q {
                                ill += 1;
                            } else if n > 0 {
                                infected += 1;
                                inf_sum += n;
                            }
                        }
                    }
                    next[y * N + x] = if cur >= Q {
                        0
                    } else if cur <= 0 {
                        if paper {
                            ill / K1 + infected / K2
                        } else {
                            infected / K1 + ill / K2
                        }
                    } else if paper {
                        // The cell is its own neighbour (fig. 2), and
                        // it is infected in this branch, so the
                        // divisor is at least one.
                        (inf_sum + cur) / (infected + 1) + G
                    } else {
                        all_sum / (infected + ill + 1) + G
                    }
                    .clamp(0, Q);
                }
            }
            s = next;
        }
        s
    };

    let (start_gs, gpu_gs) = run(0.0);
    let (start_dw, gpu_dw) = run(1.0);
    assert_eq!(start_gs, start_dw, "the two runs must start from the same field");

    let cpu_gs = mirror(&start_gs, true);
    let cpu_dw = mirror(&start_dw, false);
    let bad_gs = (0..N * N).filter(|&i| cpu_gs[i] != gpu_gs[i]).count();
    let bad_dw = (0..N * N).filter(|&i| cpu_dw[i] != gpu_dw[i]).count();
    println!(
        "hodgepodge after {STEPS} steps: Gerhardt-Schuster {bad_gs} mismatches, \
         Dewdney {bad_dw}, of {} cells",
        N * N
    );
    assert_eq!(bad_gs, 0, "the Gerhardt-Schuster rule does not match the paper");
    assert_eq!(bad_dw, 0, "the Dewdney rule does not match its published form");

    // And the two must actually be different rules: a mis-wired enum
    // that ran one of them twice would pass both mirrors above only if
    // the mirror were wired the same wrong way, but it would sail
    // through a baseline image either way.
    let differing = (0..N * N).filter(|&i| gpu_gs[i] != gpu_dw[i]).count();
    println!("   the two rules differ in {differing} of {} cells", N * N);
    assert!(
        differing > N * N / 10,
        "the two variants produced nearly the same field ({differing} cells differ): \
         the selector is not selecting"
    );
}

/// Both large-kernel gathers, against a CPU mirror using the SAME
/// table the GPU was handed.
///
/// The kernel path has more places to be silently wrong than a stencil
/// does: the table's row-major order, the sign of the offset, the
/// radius the uniform carries, the second block's offset, the
/// normalisation. Every one of those still produces a smooth evolving
/// field that looks like the model -- Lenia with a transposed kernel
/// is still Lenia-shaped -- so the mirror compares numbers.
///
/// The kernel comes from `ModelDef::kernel_for`, which is the exact
/// `Vec<f32>` uploaded, so this checks the SHADER's use of it rather
/// than re-deriving the weights and testing two implementations of the
/// same formula against each other.
#[test]
fn the_large_kernel_gathers_match_a_cpu_mirror() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: usize = 96;
    const STEPS: u32 = 3;

    for model_name in ["lenia", "smoothlife"] {
        let mut cfg = SimConfig::default();
        cfg.model = model_name.into();
        cfg.grid = crate::config::sim::SimGrid::Fixed { width: N as u32, height: N as u32 };
        cfg.init = crate::config::sim::SimInit::Noise { amplitude: 1.0 };
        cfg.boundary = SimBoundary::Periodic;
        cfg.seed = 11;
        let model = crate::sim::model_or_default(model_name);
        cfg.dt = model.default_dt;
        // A small radius keeps the mirror quick; the machinery is the
        // same at any size.
        if model_name == "lenia" {
            cfg.model_params.insert("radius".into(), 6.0);
        } else {
            cfg.model_params.insert("inner_radius".into(), 2.0);
        }

        let k = model.kernel_for(&cfg.model_params).expect("declares a kernel");
        let r = k.radius as i64;
        let w = (2 * r + 1) as usize;
        let taps = w * w;

        let mut sim = SimRenderer::new(&device, &cfg, N as u32, N as u32);
        sim.seed(&device, &queue, &cfg);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let start = read_rgba32f(&device, &queue, sim.field_texture(), N as u32, N as u32);
        sim.run_steps(&device, &queue, &cfg, STEPS);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let got = read_rgba32f(&device, &queue, sim.field_texture(), N as u32, N as u32);

        let mut f: Vec<f32> = start.iter().map(|px| px[0]).collect();
        let dt = cfg.dt;
        for _ in 0..STEPS {
            let mut next = vec![0.0f32; N * N];
            for y in 0..N {
                for x in 0..N {
                    let (mut a, mut b) = (0.0f32, 0.0f32);
                    for dy in -r..=r {
                        for dx in -r..=r {
                            let i = ((dy + r) as usize) * w + (dx + r) as usize;
                            let nx = (x as i64 + dx).rem_euclid(N as i64) as usize;
                            let ny = (y as i64 + dy).rem_euclid(N as i64) as usize;
                            let v = f[ny * N + nx];
                            a += k.weights[i] * v;
                            if k.weights.len() > taps {
                                b += k.weights[taps + i] * v;
                            }
                        }
                    }
                    let cur = f[y * N + x];
                    next[y * N + x] = if model_name == "lenia" {
                        let (mu, sg) = (0.15f32, 0.015f32);
                        let d = a - mu;
                        let g = 2.0 * (-(d * d) / (2.0 * sg * sg)).exp() - 1.0;
                        (cur + dt * g).clamp(0.0, 1.0)
                    } else {
                        // a is the inner disc, b the annulus.
                        let sig = |x: f32, c: f32, al: f32| 1.0 / (1.0 + (-(x - c) * 4.0 / al).exp());
                        let pick = sig(a, 0.5, 0.147);
                        let lo = 0.278 + (0.267 - 0.278) * pick;
                        let hi = 0.365 + (0.445 - 0.365) * pick;
                        let alive = sig(b, lo, 0.028) * (1.0 - sig(b, hi, 0.028));
                        (cur + dt * (alive - cur)).clamp(0.0, 1.0)
                    };
                }
            }
            f = next;
        }

        let mut worst = 0.0f32;
        for i in 0..N * N {
            worst = worst.max((f[i] - got[i][0]).abs());
        }
        println!(
            "{model_name}: radius {r}, {taps} taps, {STEPS} steps vs CPU mirror: \
             worst |delta| = {worst:.3e}"
        );
        assert!(
            worst < 2e-4,
            "{model_name}: GPU and CPU disagree by {worst:.3e} -- the gather does not \
             match the table it was given"
        );
    }
}

/// Phase 3's gate: Lenia at R = 13 must run 512² at 60 steps a second.
///
/// 729 taps a cell over 262,144 cells is 1.9e8 texture reads a step,
/// and the budget is 16.7 ms. This is the measurement the phase's
/// plan named, and it decides whether the large-kernel models need the
/// shared-memory tile that was held back as a fallback.
///
/// Run with `--test-threads=1`: a GPU timing test sharing the device
/// with another one measures the other one too.
#[test]
fn lenia_meets_the_phase_3_interactive_budget() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: u32 = 512;
    const STEPS: u32 = 120;
    let mut cfg = SimConfig::default();
    cfg.model = "lenia".into();
    cfg.grid = crate::config::sim::SimGrid::Fixed { width: N, height: N };
    cfg.init = crate::config::sim::SimInit::Noise { amplitude: 1.0 };
    cfg.boundary = SimBoundary::Periodic;
    cfg.model_params.insert("radius".into(), 13.0);

    let mut sim = SimRenderer::new(&device, &cfg, N, N);
    sim.seed(&device, &queue, &cfg);
    // Warm: shader compile and the first submission's sizing.
    sim.run_steps(&device, &queue, &cfg, 20);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });

    let t0 = std::time::Instant::now();
    sim.run_steps(&device, &queue, &cfg, STEPS);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let ms = t0.elapsed().as_secs_f64() * 1e3 / STEPS as f64;
    let taps = 27.0 * 27.0 * (N as f64) * (N as f64);
    println!(
        "Lenia R=13 at {N}²: {ms:.3} ms/step ({:.1} steps/s), {:.2e} taps/step, \
         {:.2e} taps/s",
        1e3 / ms,
        taps,
        taps / (ms / 1e3)
    );
    assert!(
        ms < 16.67,
        "phase 3's gate is 60 steps/s at 512² and this is {:.1} ({ms:.2} ms/step)",
        1e3 / ms
    );
}

/// Diagnostic: what the gather costs as the radius grows, for both
/// models, at the size the gate uses.
#[test]
#[ignore = "diagnostic"]
fn large_kernel_cost_against_radius() {
    let Some((device, queue)) = repro_device() else {
        return;
    };
    const N: u32 = 512;
    const STEPS: u32 = 60;
    for (model, param, values) in [
        ("lenia", "radius", vec![6.0f32, 13.0, 21.0, 32.0]),
        ("smoothlife", "inner_radius", vec![2.0, 4.0, 7.0, 10.0]),
    ] {
        for v in values {
            let mut cfg = SimConfig::default();
            cfg.model = model.into();
            cfg.grid = crate::config::sim::SimGrid::Fixed { width: N, height: N };
            cfg.init = crate::config::sim::SimInit::Noise { amplitude: 1.0 };
            cfg.model_params.insert(param.into(), v);
            let k = crate::sim::model_or_default(model)
                .kernel_for(&cfg.model_params)
                .unwrap();
            let mut sim = SimRenderer::new(&device, &cfg, N, N);
            sim.seed(&device, &queue, &cfg);
            sim.run_steps(&device, &queue, &cfg, 20);
            let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
            let t0 = std::time::Instant::now();
            sim.run_steps(&device, &queue, &cfg, STEPS);
            let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
            let ms = t0.elapsed().as_secs_f64() * 1e3 / STEPS as f64;
            let w = 2.0 * k.radius as f64 + 1.0;
            println!(
                "{model:<11} {param}={v:<5} kernel radius {:<3} {:>5.0} taps  \
                 {ms:>7.3} ms/step  {:.2e} taps/s",
                k.radius,
                w * w,
                w * w * (N as f64) * (N as f64) / (ms / 1e3)
            );
        }
    }
}

/// Read `count` u32s from a storage buffer, starting at `offset`
/// bytes. The buffer must carry COPY_SRC.
fn read_u32s(device: &Device, queue: &Queue, src: &Buffer, offset: u64, count: usize) -> Vec<u32> {
    let bytes = (count * 4) as u64;
    let buf = device.create_buffer(&BufferDescriptor {
        label: Some("sim u32 readback"),
        size: bytes,
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&CommandEncoderDescriptor { label: None });
    enc.copy_buffer_to_buffer(src, offset, &buf, 0, bytes);
    queue.submit(std::iter::once(enc.finish()));
    let (tx, rx) = std::sync::mpsc::channel();
    buf.slice(..).map_async(MapMode::Read, move |r| {
        let _ = tx.send(r.is_ok());
    });
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    assert!(rx.recv().unwrap_or(false), "buffer readback map failed");
    let view = buf.slice(..).get_mapped_range();
    view.chunks_exact(4)
        .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
        .collect()
}

/// The reduce pass's ordering map, inverted -- a mirror of
/// `minmax_unord` in the WGSL.
fn minmax_unord(e: u32) -> f32 {
    if e >> 31 != 0 {
        f32::from_bits(e ^ 0x8000_0000)
    } else {
        f32::from_bits(!e)
    }
}

/// CPU mirror of one pyramid level from the one below it: the 5x5
/// [1 4 6 4 1]/16 blur at stride 2, periodic at the SOURCE size.
fn cpu_pyramid_level(src: &[f32], sw: usize, sh: usize) -> (Vec<f32>, usize, usize) {
    const G: [f32; 5] = [0.0625, 0.25, 0.375, 0.25, 0.0625];
    let (dw, dh) = (sw.div_ceil(2), sh.div_ceil(2));
    let mut out = vec![0.0f32; dw * dh];
    for y in 0..dh {
        for x in 0..dw {
            let mut acc = 0.0f32;
            for dy in -2i64..=2 {
                for dx in -2i64..=2 {
                    let sx = (2 * x as i64 + dx).rem_euclid(sw as i64) as usize;
                    let sy = (2 * y as i64 + dy).rem_euclid(sh as i64) as usize;
                    acc += G[(dy + 2) as usize] * G[(dx + 2) as usize] * src[sy * sw + sx];
                }
            }
            out[y * dw + x] = acc;
        }
    }
    (out, dw, dh)
}

fn mccabe_config(n: u32) -> SimConfig {
    let mut cfg = SimConfig::default();
    cfg.model = "mccabe".into();
    cfg.grid = crate::config::sim::SimGrid::Fixed { width: n, height: n };
    cfg.init = crate::config::sim::SimInit::Noise { amplitude: 1.0 };
    cfg.boundary = SimBoundary::Periodic;
    cfg.seed = 7;
    cfg
}

/// The pyramid stage, level by level, against a CPU mirror.
///
/// Level 1 is compared against the decimation of the field the GPU
/// built it from, and level 2 against the decimation of the GPU's
/// OWN level 1 -- so each dispatch is checked on its own inputs and a
/// wrong level uniform (the source size the wrap uses) would fail at
/// the edges.
#[test]
fn the_pyramid_levels_match_a_cpu_gaussian_decimation() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: u32 = 96;
    let cfg = mccabe_config(N);
    let mut r = SimRenderer::new(&device, &cfg, N, N);
    r.seed(&device, &queue, &cfg);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let f0: Vec<f32> = read_rgba32f(&device, &queue, r.field_texture(), N, N)
        .iter()
        .map(|px| px[0])
        .collect();
    // One step builds the pyramid of f0 before it moves the field.
    r.run_steps(&device, &queue, &cfg, 1);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });

    let (l1_cpu, w1, h1) = cpu_pyramid_level(&f0, N as usize, N as usize);
    let l1_gpu: Vec<f32> = read_rgba32f(&device, &queue, r.pyramid_texture(1).unwrap(), w1 as u32, h1 as u32)
        .iter()
        .map(|px| px[0])
        .collect();
    let worst1 = l1_cpu.iter().zip(&l1_gpu).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);

    let (l2_cpu, w2, h2) = cpu_pyramid_level(&l1_gpu, w1, h1);
    let l2_gpu: Vec<f32> = read_rgba32f(&device, &queue, r.pyramid_texture(2).unwrap(), w2 as u32, h2 as u32)
        .iter()
        .map(|px| px[0])
        .collect();
    let worst2 = l2_cpu.iter().zip(&l2_gpu).map(|(a, b)| (a - b).abs()).fold(0.0f32, f32::max);
    println!("pyramid: level 1 ({w1}x{h1}) worst {worst1:.3e}, level 2 ({w2}x{h2}) worst {worst2:.3e}");
    assert!(worst1 < 1e-5, "level 1 differs from the CPU decimation by {worst1:.3e}");
    assert!(worst2 < 1e-5, "level 2 differs from the CPU decimation by {worst2:.3e}");
}

/// The reduce pass, against the CPU's min and max of the same field.
///
/// Exact, not a tolerance: the ordering map is a bijection on the
/// bits, so the decoded slot must equal the CPU's f32 min and max to
/// the bit. It would catch a workgroup reduction that dropped a lane,
/// an atomic on the wrong slot, or a ring that was cleared after being
/// written.
#[test]
fn the_reduce_matches_the_cpu_min_and_max_exactly() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: u32 = 100; // deliberately not a multiple of 8
    let cfg = mccabe_config(N);
    let mut r = SimRenderer::new(&device, &cfg, N, N);
    r.seed(&device, &queue, &cfg);
    // The seed's reduce writes the slot before step 0.
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let seed_field = read_rgba32f(&device, &queue, r.field_texture(), N, N);
    let seed_slot = (crate::sim::MINMAX_RING - 1) as u64;
    let enc = read_u32s(&device, &queue, r.minmax_buffer(), seed_slot * 8, 2);
    let (lo, hi) = (minmax_unord(enc[0]), minmax_unord(enc[1]));
    let cpu_lo = seed_field.iter().map(|px| px[0]).fold(f32::INFINITY, f32::min);
    let cpu_hi = seed_field.iter().map(|px| px[0]).fold(f32::NEG_INFINITY, f32::max);
    println!("reduce after seed: gpu [{lo}, {hi}]  cpu [{cpu_lo}, {cpu_hi}]");
    assert_eq!(lo.to_bits(), cpu_lo.to_bits(), "seed min");
    assert_eq!(hi.to_bits(), cpu_hi.to_bits(), "seed max");

    // And after three steps, the slot of the last step holds the range
    // of the field as it now stands.
    r.run_steps(&device, &queue, &cfg, 3);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let field = read_rgba32f(&device, &queue, r.field_texture(), N, N);
    let enc = read_u32s(&device, &queue, r.minmax_buffer(), 2 * 8, 2);
    let (lo, hi) = (minmax_unord(enc[0]), minmax_unord(enc[1]));
    let cpu_lo = field.iter().map(|px| px[0]).fold(f32::INFINITY, f32::min);
    let cpu_hi = field.iter().map(|px| px[0]).fold(f32::NEG_INFINITY, f32::max);
    println!("reduce after step 2: gpu [{lo}, {hi}]  cpu [{cpu_lo}, {cpu_hi}]");
    assert_eq!(lo.to_bits(), cpu_lo.to_bits(), "step-2 min");
    assert_eq!(hi.to_bits(), cpu_hi.to_bits(), "step-2 max");

    // And across the ring's wrap. The ring has 257 slots and the
    // clearing write splits into two when a batch straddles the end;
    // nothing else exercises that branch, and if it cleared the wrong
    // slots the range would silently fall back to [-1, 1] and the
    // picture would drift rather than fail. 600 steps cross the wrap
    // twice, in batches whose size the renderer chooses itself.
    r.run_steps(&device, &queue, &cfg, 600);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let field = read_rgba32f(&device, &queue, r.field_texture(), N, N);
    let last = 3 + 600 - 1;
    let slot = (last as u64) % (crate::sim::MINMAX_RING as u64);
    let enc = read_u32s(&device, &queue, r.minmax_buffer(), slot * 8, 2);
    let (lo, hi) = (minmax_unord(enc[0]), minmax_unord(enc[1]));
    let cpu_lo = field.iter().map(|px| px[0]).fold(f32::INFINITY, f32::min);
    let cpu_hi = field.iter().map(|px| px[0]).fold(f32::NEG_INFINITY, f32::max);
    println!("reduce after step {last} (slot {slot}): gpu [{lo}, {hi}]  cpu [{cpu_lo}, {cpu_hi}]");
    assert_eq!(lo.to_bits(), cpu_lo.to_bits(), "post-wrap min");
    assert_eq!(hi.to_bits(), cpu_hi.to_bits(), "post-wrap max");
}

/// McCabe's whole step -- pyramid, trilinear reads, scale selection,
/// renormalisation -- against a CPU mirror from the GPU's own seed.
///
/// The one place a mirror can legitimately disagree is a TIE: when
/// two scales' variations are within rounding of each other, the
/// trilinear arithmetic can pick either, and the cell moves by one
/// amount rather than another. Those cells differ by a known quantum;
/// everything else must match to float precision. So the test counts
/// both, and demands the tie fraction be small.
#[test]
fn mccabe_matches_a_cpu_mirror() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: usize = 64;
    let cfg = mccabe_config(N as u32);
    let mut r = SimRenderer::new(&device, &cfg, N as u32, N as u32);
    r.seed(&device, &queue, &cfg);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let f0: Vec<f32> = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32)
        .iter()
        .map(|px| px[0])
        .collect();
    r.run_steps(&device, &queue, &cfg, 1);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let got = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);

    // The CPU pyramid, same rule as the renderer's.
    let levels = crate::sim::pyramid_levels(N as u32, N as u32) as usize;
    let mut pyr: Vec<(Vec<f32>, usize, usize)> = vec![(f0.clone(), N, N)];
    for _ in 1..levels {
        let (src, w, h) = pyr.last().unwrap();
        let next = cpu_pyramid_level(src, *w, *h);
        pyr.push(next);
    }
    let load = |l: usize, qx: i64, qy: i64| -> f32 {
        let (ref d, w, h) = pyr[l];
        d[(qy.rem_euclid(h as i64) as usize) * w + qx.rem_euclid(w as i64) as usize]
    };
    let level_avg = |l: usize, px: f32, py: f32| -> f32 {
        let s = (1u32 << l) as f32;
        let (fx, fy) = (px / s - 0.5, py / s - 0.5);
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let (ix, iy) = (x0 as i64, y0 as i64);
        let a = load(l, ix, iy);
        let b = load(l, ix + 1, iy);
        let c = load(l, ix, iy + 1);
        let d = load(l, ix + 1, iy + 1);
        (a + (b - a) * tx) + ((c + (d - c) * tx) - (a + (b - a) * tx)) * ty
    };
    let sample = |level: f32, px: f32, py: f32| -> f32 {
        let top = (levels - 1) as f32;
        let lf = level.clamp(0.0, top);
        let l0 = lf.floor() as usize;
        let l1 = (l0 + 1).min(levels - 1);
        let t = lf - lf.floor();
        let a = level_avg(l0, px, py);
        let b = level_avg(l1, px, py);
        a + (b - a) * t
    };
    let level_for = |r: f32| (0.55f32 * r).max(1.0).log2();

    let lo = f0.iter().cloned().fold(f32::INFINITY, f32::min);
    let hi = f0.iter().cloned().fold(f32::NEG_INFINITY, f32::max);
    let (n_scales, base, ratio, amount, amount_min) = (5usize, 1.0f32, 2.0f32, 0.05f32, 0.01f32);
    let (mut exact, mut ties, mut other) = (0usize, 0usize, 0usize);
    let mut worst_exact = 0.0f32;
    for y in 0..N {
        for x in 0..N {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let mut best_var = f32::MAX;
            let mut best_dir = 0.0f32;
            for i in 0..n_scales {
                let ra = base * (1u32 << i) as f32;
                let rb = ra * ratio;
                let act = sample(level_for(ra), px, py);
                let inh = sample(level_for(rb), px, py);
                let v = (act - inh).abs();
                let t = i as f32 / (n_scales - 1) as f32;
                let amt = amount + (amount_min - amount) * t;
                if v < best_var {
                    best_var = v;
                    best_dir = if act > inh { amt } else { -amt };
                }
            }
            let f = (f0[y * N + x] - lo) / (hi - lo).max(1e-6) * 2.0 - 1.0;
            let want = f + best_dir;
            let d = (want - got[y * N + x][0]).abs();
            if d < 1e-4 {
                exact += 1;
                worst_exact = worst_exact.max(d);
            } else if d < 0.2 {
                // A different scale fired: the difference is the gap
                // between two amounts.
                ties += 1;
            } else {
                other += 1;
            }
        }
    }
    println!(
        "McCabe vs CPU mirror: {exact} cells match (worst {worst_exact:.2e}), {ties} chose a \
         different scale at a tie, {other} disagree outright"
    );
    assert_eq!(other, 0, "{other} cells disagree by more than any amount difference");
    assert!(
        ties * 200 < N * N,
        "{ties} of {} cells picked a different scale -- far more than rounding ties",
        N * N
    );
}

/// Phase 3's other gate: McCabe at 1080p inside the interactive budget.
///
/// The pipeline doc expected "well under 2 ms" for a box pyramid and
/// named 8 ms as the point past which the fallbacks kick in. The
/// shipped pyramid is Gaussian (25 taps a level rather than 4), and
/// the step reads five scales at 16 loads each.
#[test]
fn mccabe_meets_the_interactive_budget_at_1080p() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    let (w, h) = (1920u32, 1080u32);
    let mut cfg = mccabe_config(256);
    cfg.grid = crate::config::sim::SimGrid::Fixed { width: w, height: h };
    let mut r = SimRenderer::new(&device, &cfg, w, h);
    r.seed(&device, &queue, &cfg);
    r.run_steps(&device, &queue, &cfg, 16);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    const STEPS: u32 = 60;
    let t0 = std::time::Instant::now();
    r.run_steps(&device, &queue, &cfg, STEPS);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let ms = t0.elapsed().as_secs_f64() * 1e3 / STEPS as f64;
    println!("McCabe 5 scales at 1080p: {ms:.3} ms/step ({:.1} steps/s)", 1e3 / ms);
    assert!(ms < 8.0, "McCabe at 1080p is {ms:.2} ms/step, past the 8 ms fallback threshold");
}

/// Review probe: what the per-frame kernel rebuild costs on the CPU.
/// `write_param_arrays` runs `kernel_for` on every `run_steps` and
/// every `color`, so twice a frame.
#[test]
#[ignore = "diagnostic"]
fn kernel_rebuild_cost() {
    for (model, param, v) in [("lenia", "radius", 13.0f32), ("lenia", "radius", 32.0),
                              ("smoothlife", "inner_radius", 7.0), ("smoothlife", "inner_radius", 10.0)] {
        let m = crate::sim::model_or_default(model);
        let mut map = std::collections::BTreeMap::new();
        map.insert(param.to_string(), v);
        let t0 = std::time::Instant::now();
        let mut n = 0usize;
        for _ in 0..200 { n += m.kernel_for(&map).unwrap().weights.len(); }
        let us = t0.elapsed().as_secs_f64() * 1e6 / 200.0;
        let floats = n / 200;
        println!("{model:<11} {param}={v:<5} {floats:>5} floats  {us:>8.1} us per build");
    }
}

/// Phase 4's first gate: an agent model must be REPRODUCIBLE, with a
/// million agents piling into the same cells.
///
/// This is the property the whole deposit design exists for. Thousands
/// of agents land in one cell in an order the hardware chooses; the
/// deposit is an integer atomicAdd, which is associative and
/// commutative, so the total does not depend on that order. An f32
/// accumulation would give a different field every run. The exclusion
/// is resolved the same way -- an atomic MINIMUM over agent indices,
/// so the winner is the lowest index rather than whoever ran first.
///
/// Two independent renderers, same config, compared BIT for BIT.
#[test]
fn an_agent_run_reproduces_bit_for_bit_with_a_million_agents() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    // 2048^2 at the population slider's maximum of 25% is 1,048,576
    // agents -- the gate's million, on a grid big enough that the
    // model's own clamp does not cap it first.
    const N: u32 = 2048;
    let mut cfg = SimConfig::default();
    cfg.model = "physarum".into();
    cfg.grid = crate::config::sim::SimGrid::Fixed { width: N, height: N };
    cfg.init = crate::config::sim::SimInit::Noise { amplitude: 0.0 };
    cfg.boundary = SimBoundary::Periodic;
    cfg.seed = 5;
    cfg.model_params.insert("population".into(), 25.0);

    let model = crate::sim::model_or_default("physarum");
    let agents = (model.agents.unwrap().count)(&model.params_view(&cfg.model_params), N, N);
    assert!(agents >= 1_000_000, "wanted a million agents, got {agents}");

    let run = || -> Vec<[f32; 4]> {
        let mut r = SimRenderer::new(&device, &cfg, N, N);
        r.seed(&device, &queue, &cfg);
        r.run_steps(&device, &queue, &cfg, 40);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        read_rgba32f(&device, &queue, r.field_texture(), N, N)
    };
    let a = run();
    let b = run();
    let differing = (0..a.len()).filter(|&i| a[i][0].to_bits() != b[i][0].to_bits()).count();
    let lit = a.iter().filter(|px| px[0] > 0.0).count();
    println!(
        "Physarum, {agents} agents, 40 steps at {N}²: {differing} of {} cells differ between \
         runs, {lit} carry trail",
        a.len()
    );
    assert!(lit > a.len() / 100, "the run deposited almost nothing ({lit} cells)");
    assert_eq!(differing, 0, "two runs of the same config disagree: the deposit is not \
                              order-independent");
}

/// Phase 4's second gate: DLA's cluster must have the fractal
/// dimension DLA has, about 1.71.
///
/// The parallel variant here advances many walkers at once, which is
/// not Witten and Sander's sequential process, so the dimension is
/// something to MEASURE rather than assume -- crowding near the
/// cluster thickens branches and drives it toward 2, and relaunching
/// walkers inside the fjords would do the same.
///
/// Box counting on the frozen cells: the count of occupied boxes of
/// side s scales as s^-D, so the slope of log N against log(1/s) is
/// the dimension. Measured over the box sizes that sit inside the
/// cluster, away from the single-cell and whole-cluster ends where
/// box counting always bends.
#[test]
fn dla_grows_a_cluster_of_the_right_fractal_dimension() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: usize = 512;
    let mut cfg = SimConfig::default();
    cfg.model = "dla".into();
    cfg.grid = crate::config::sim::SimGrid::Fixed { width: N as u32, height: N as u32 };
    cfg.init = crate::config::sim::SimInit::Center;
    cfg.boundary = SimBoundary::Clamp;
    cfg.seed = 3;
    cfg.model_params.insert("walkers".into(), 4.0);
    cfg.model_params.insert("p_stick".into(), 1.0);
    cfg.model_params.insert("crowding".into(), 2.0);
    cfg.model_params.insert("launch_gap".into(), 5.0);

    let mut r = SimRenderer::new(&device, &cfg, N as u32, N as u32);
    r.seed(&device, &queue, &cfg);
    // The cluster's size is what decides whether this reads a
    // dimension at all: a cluster that reaches the wall densifies at
    // the rim and reads the wall's shape rather than its own.
    r.run_steps(&device, &queue, &cfg, 1200);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let f = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);
    let frozen: Vec<bool> = f.iter().map(|px| px[0] > 0.0).collect();
    let stuck = frozen.iter().filter(|b| **b).count();
    // The cluster's extent, for choosing the box range.
    let mut radius = 0.0f64;
    for y in 0..N {
        for x in 0..N {
            if frozen[y * N + x] {
                let d = (((x as f64) - 256.0).powi(2) + ((y as f64) - 256.0).powi(2)).sqrt();
                radius = radius.max(d);
            }
        }
    }
    println!("DLA at {N}²: {stuck} particles stuck, cluster radius {radius:.0} cells");
    assert!(stuck > 3000, "only {stuck} particles stuck; nothing to measure");
    assert!(
        radius < 240.0,
        "the cluster reached the wall at {radius:.0} cells; the dimension would be the wall's"
    );
    assert!(radius > 60.0, "the cluster is only {radius:.0} cells across");

    // Box counting.
    let mut pts = Vec::new();
    for s in [2usize, 4, 8, 16, 32] {
        let mut seen = std::collections::HashSet::new();
        for y in 0..N {
            for x in 0..N {
                if frozen[y * N + x] {
                    seen.insert((x / s, y / s));
                }
            }
        }
        pts.push(((1.0 / s as f64).ln(), (seen.len() as f64).ln()));
        println!("   box {s:>3}: {} occupied", seen.len());
    }
    // Least squares slope.
    let n = pts.len() as f64;
    let sx: f64 = pts.iter().map(|p| p.0).sum();
    let sy: f64 = pts.iter().map(|p| p.1).sum();
    let sxx: f64 = pts.iter().map(|p| p.0 * p.0).sum();
    let sxy: f64 = pts.iter().map(|p| p.0 * p.1).sum();
    let d = (n * sxy - sx * sy) / (n * sxx - sx * sx);
    println!("   box-counting dimension {d:.3} (DLA is about 1.71)");
    assert!(
        (1.55..1.90).contains(&d),
        "box-counting dimension {d:.3}, which is not DLA's ~1.71 -- a cluster that is too \
         dense reads toward 2 and one that is too sparse toward 1"
    );
}

/// Review probe: what an agent step costs at 1080p, with the agent
/// passes and the field pass separated.
#[test]
#[ignore = "diagnostic"]
fn agent_step_cost_at_1080p() {
    let Some((device, queue)) = repro_device() else { return; };
    let (w, h) = (1920u32, 1080u32);
    for (model, param, v, boundary) in [
        ("physarum", "population", 5.0f32, SimBoundary::Periodic),
        ("physarum", "population", 15.0, SimBoundary::Periodic),
        ("dla", "walkers", 4.0, SimBoundary::Clamp),
    ] {
        let mut cfg = SimConfig::default();
        cfg.model = model.into();
        cfg.grid = crate::config::sim::SimGrid::Fixed { width: w, height: h };
        cfg.init = if model == "dla" { crate::config::sim::SimInit::Center }
                   else { crate::config::sim::SimInit::Noise { amplitude: 0.0 } };
        cfg.boundary = boundary;
        cfg.model_params.insert(param.into(), v);
        let m = crate::sim::model_or_default(model);
        let agents = (m.agents.unwrap().count)(&m.params_view(&cfg.model_params), w, h);
        let mut r = SimRenderer::new(&device, &cfg, w, h);
        r.seed(&device, &queue, &cfg);
        r.run_steps(&device, &queue, &cfg, 30);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        const STEPS: u32 = 120;
        let t0 = std::time::Instant::now();
        r.run_steps(&device, &queue, &cfg, STEPS);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let ms = t0.elapsed().as_secs_f64() * 1e3 / STEPS as f64;
        println!("{model:<9} {param}={v:<5} {agents:>8} agents  {ms:>7.3} ms/step  ({:.0} steps/s)", 1e3 / ms);
    }
}

/// The shader's PCG, mirrored: WGSL u32 arithmetic wraps.
fn sim_pcg(v: u32) -> u32 {
    let state = v.wrapping_mul(747796405).wrapping_add(2891336453);
    let word = ((state >> ((state >> 28) + 4)) ^ state).wrapping_mul(277803737);
    (word >> 22) ^ word
}

/// `agent_rand(i, salt)` as the shader computes it for a given step.
fn agent_rand(seed: u64, step: u32, i: u32, salt: u32) -> f32 {
    let mut h = sim_pcg(i ^ (seed as u32));
    h = sim_pcg(h ^ ((seed >> 32) as u32) ^ salt);
    h = sim_pcg(h ^ step);
    (h >> 8) as f32 * (1.0 / 16777216.0)
}

/// The agent buffer, as (pos.x, pos.y, heading, state) per agent.
fn read_agents(device: &Device, queue: &Queue, buf: &Buffer, n: usize) -> Vec<[f32; 4]> {
    read_u32s(device, queue, buf, 0, n * 4)
        .chunks_exact(4)
        .map(|c| [f32::from_bits(c[0]), f32::from_bits(c[1]), f32::from_bits(c[2]), f32::from_bits(c[3])])
        .collect()
}

/// Physarum's whole step -- sense, turn, claim, move, deposit, diffuse,
/// decay -- against a CPU mirror of Jones' rule from the GPU's own
/// seeded population, for two steps so the turn rule sees a trail.
///
/// This is the test the wave lacked. Its gates checked that a run
/// REPRODUCES and that DLA's dimension is right; nothing checked that
/// the rule was Jones'. The phase-4 review found the shader turning
/// toward the stronger side when both sides beat the front, where the
/// paper's figure 3 (and the prototype that validated every
/// parameter) turns at random. That still builds a network. This
/// would have failed on it.
///
/// The claim is mirrored as the atomic minimum it is, and the random
/// draws through a mirror of the shader's PCG, so agents and field are
/// compared to float precision rather than statistically.
#[test]
fn physarum_matches_a_cpu_mirror_of_jones_rule() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: usize = 64;
    let mut cfg = SimConfig::default();
    cfg.model = "physarum".into();
    cfg.grid = crate::config::sim::SimGrid::Fixed { width: N as u32, height: N as u32 };
    cfg.init = crate::config::sim::SimInit::Noise { amplitude: 0.0 };
    cfg.boundary = SimBoundary::Periodic;
    cfg.seed = 7;
    // Dense, so claims collide and the exclusion is exercised.
    cfg.model_params.insert("population".into(), 20.0);
    let model = crate::sim::model_or_default("physarum");
    let count = (model.agents.unwrap().count)(&model.params_view(&cfg.model_params), N as u32, N as u32) as usize;
    let p = model.pack_params(&cfg);
    let (sa, ra, so, ss, dep, decay) = (
        p[1] * std::f32::consts::PI / 180.0,
        p[2] * std::f32::consts::PI / 180.0,
        p[3], p[4], p[5], p[6],
    );

    let mut r = SimRenderer::new(&device, &cfg, N as u32, N as u32);
    r.seed(&device, &queue, &cfg);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let mut agents = read_agents(&device, &queue, r.agent_buffer().unwrap(), count);
    let mut trail: Vec<f32> = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32)
        .iter().map(|px| px[0]).collect();

    const STEPS: u32 = 2;
    r.run_steps(&device, &queue, &cfg, STEPS);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let got_agents = read_agents(&device, &queue, r.agent_buffer().unwrap(), count);
    let got_field = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);

    let n = N as i64;
    let g = N as f32;
    let mut lost = 0usize;
    let cell_of = |x: f32, y: f32| -> (usize, usize) {
        let cx = ((x + 0.5).floor() as i64).rem_euclid(n) as usize;
        let cy = ((y + 0.5).floor() as i64).rem_euclid(n) as usize;
        (cx, cy)
    };
    for step in 0..STEPS {
        // Pass 1: sense, turn, claim.
        let mut claim = vec![u32::MAX; N * N];
        let mut turned: Vec<[f32; 4]> = agents.clone();
        for (i, a) in agents.iter().enumerate() {
            let sense = |off: f32| -> f32 {
                let h = a[2] + off;
                let (cx, cy) = cell_of(a[0] + h.cos() * so, a[1] + h.sin() * so);
                trail[cy * N + cx]
            };
            let (f, l, rr) = (sense(0.0), sense(sa), sense(-sa));
            let turn = if f >= l && f >= rr {
                0.0
            } else if f < l && f < rr {
                if agent_rand(cfg.seed, step, i as u32, 0x14) < 0.5 { ra } else { -ra }
            } else if l > rr {
                ra
            } else if rr > l {
                -ra
            } else {
                0.0
            };
            turned[i][2] = a[2] + turn;
            let h = turned[i][2];
            let (cx, cy) = cell_of(a[0] + h.cos() * ss, a[1] + h.sin() * ss);
            let slot = &mut claim[cy * N + cx];
            *slot = (*slot).min(i as u32);
        }
        // Pass 2: move if won, else a new heading. Deposits.
        let mut deposit = vec![0.0f32; N * N];
        let mut moved = turned.clone();
        for (i, a) in turned.iter().enumerate() {
            let h = a[2];
            let (dx, dy) = (a[0] + h.cos() * ss, a[1] + h.sin() * ss);
            let (cx, cy) = cell_of(dx, dy);
            if claim[cy * N + cx] == i as u32 {
                let wx = dx - g * (dx / g).floor();
                let wy = dy - g * (dy / g).floor();
                moved[i][0] = wx;
                moved[i][1] = wy;
                let (px, py) = cell_of(wx, wy);
                deposit[py * N + px] += dep;
            } else {
                lost += 1;
                moved[i][2] = agent_rand(cfg.seed, step, i as u32, 0x15) * std::f32::consts::TAU;
            }
        }
        // The field: 3x3 mean of the old trail, plus the deposit, decayed.
        let mut next = vec![0.0f32; N * N];
        for y in 0..N {
            for x in 0..N {
                let mut acc = 0.0f32;
                for ddy in -1i64..=1 {
                    for ddx in -1i64..=1 {
                        let sx = (x as i64 + ddx).rem_euclid(n) as usize;
                        let sy = (y as i64 + ddy).rem_euclid(n) as usize;
                        acc += trail[sy * N + sx];
                    }
                }
                next[y * N + x] = (acc / 9.0 + deposit[y * N + x]) * (1.0 - decay);
            }
        }
        agents = moved;
        trail = next;
    }

    let mut worst_pos = 0.0f32;
    let mut worst_head = 0.0f32;
    for i in 0..count {
        // Around the torus: the shorter way.
        let dx = (agents[i][0] - got_agents[i][0]).abs();
        let dy = (agents[i][1] - got_agents[i][1]).abs();
        worst_pos = worst_pos.max(dx.min(g - dx)).max(dy.min(g - dy));
        worst_head = worst_head.max((agents[i][2] - got_agents[i][2]).abs());
    }
    let worst_field = (0..N * N)
        .map(|k| (trail[k] - got_field[k][0]).abs())
        .fold(0.0f32, f32::max);
    println!(
        "Physarum {count} agents, {STEPS} steps vs CPU mirror: positions {worst_pos:.2e}, \
         headings {worst_head:.2e}, trail {worst_field:.2e}; {lost} moves lost to the exclusion"
    );
    assert!(lost > 0, "no agent ever lost a claim, so the exclusion was not exercised");
    assert!(worst_pos < 1e-4, "agent positions diverge from Jones' rule by {worst_pos:.2e}");
    assert!(worst_head < 1e-4, "agent headings diverge from Jones' rule by {worst_head:.2e}");
    assert!(worst_field < 1e-4, "the trail diverges from the mirror by {worst_field:.2e}");
}

/// A wall is a wall. Under a non-periodic boundary an agent must not
/// leave the grid, and it must not reappear on the far side.
///
/// The first version wrapped every position periodically whatever the
/// boundary, while the DEPOSIT clamped -- so under Clamp an agent that
/// walked off the left edge reappeared on the right. An in-range check
/// cannot catch that (a wrapped position is in range); a per-step
/// displacement can, because a wrap is a jump of nearly the grid.
///
/// The FIX to that had two bugs of its own. This test caught one on
/// its first run: a destination of x = -0.4 is inside cell 0 and
/// passes the wall check, and the unconditional float wrap then put
/// the agent at 63.6 -- so the range assertion below is not redundant
/// after all. The other was found reading the code while chasing it:
/// refusing the move in pass 2 but still claiming the clamped edge
/// cell in pass 1 leaks the claim, since only its owner's check
/// releases one, and the edge silts up until an agent beside the wall
/// can never move again. Hence the last assertion: after a run the
/// claim buffer is empty. Measured with the pass-1 guard removed, it
/// reads 129 stale cells on this grid, so the assertion does fire.
#[test]
fn physarum_agents_do_not_pass_through_a_wall() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: usize = 64;
    let mut cfg = SimConfig::default();
    cfg.model = "physarum".into();
    cfg.grid = crate::config::sim::SimGrid::Fixed { width: N as u32, height: N as u32 };
    cfg.init = crate::config::sim::SimInit::Noise { amplitude: 0.0 };
    cfg.boundary = SimBoundary::Clamp;
    cfg.seed = 11;
    cfg.model_params.insert("population".into(), 5.0);
    let model = crate::sim::model_or_default("physarum");
    let count = (model.agents.unwrap().count)(&model.params_view(&cfg.model_params), N as u32, N as u32) as usize;
    let ss = model.pack_params(&cfg)[4];

    let mut r = SimRenderer::new(&device, &cfg, N as u32, N as u32);
    r.seed(&device, &queue, &cfg);
    r.run_steps(&device, &queue, &cfg, 100);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let mut prev = read_agents(&device, &queue, r.agent_buffer().unwrap(), count);
    let mut worst_jump = 0.0f32;
    let mut blocked = 0usize;
    for _ in 0..40 {
        r.run_steps(&device, &queue, &cfg, 1);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let now = read_agents(&device, &queue, r.agent_buffer().unwrap(), count);
        for i in 0..count {
            let d = ((now[i][0] - prev[i][0]).powi(2) + (now[i][1] - prev[i][1]).powi(2)).sqrt();
            worst_jump = worst_jump.max(d);
            assert!(
                now[i][0] >= -0.5 && now[i][0] < N as f32 - 0.5 && now[i][1] >= -0.5 && now[i][1] < N as f32 - 0.5,
                "agent {i} is outside the grid at ({}, {})", now[i][0], now[i][1]
            );
            if d == 0.0 && now[i][2] != prev[i][2] {
                blocked += 1;
            }
        }
        prev = now;
    }
    let stale = read_u32s(&device, &queue, r.claim_buffer(), 0, N * N)
        .iter()
        .filter(|&&c| c != u32::MAX)
        .count();
    println!(
        "Physarum under Clamp: largest per-step move {worst_jump:.3} (step size {ss}), \
         {blocked} blocked moves in 40 steps, {stale} stale claims"
    );
    assert!(worst_jump <= ss * 1.001, "an agent jumped {worst_jump:.2} cells in one step: it passed through a wall");
    assert!(blocked > 0, "no agent was ever blocked, so the wall was never tested");
    assert_eq!(stale, 0, "{stale} cells still hold a claim after the step: some claim was never checked");
}

/// Bulk toppling on the CPU: the same parallel schedule the shader
/// runs, in exact integers. Returns the number of rounds to a stable
/// configuration and the final heights.
///
/// The edges are sinks: a grain sent off the grid is added nowhere.
fn cpu_sandpile(n: usize, grains: i64, moore: bool) -> (u32, Vec<i64>) {
    let thresh: i64 = if moore { 8 } else { 4 };
    let mut h = vec![0i64; n * n];
    h[(n / 2) * n + n / 2] = grains;
    let mut fires = vec![0i64; n * n];
    let mut rounds = 0u32;
    loop {
        let mut any = false;
        for k in 0..n * n {
            fires[k] = h[k] / thresh;
            any |= fires[k] > 0;
        }
        if !any {
            return (rounds, h);
        }
        for y in 0..n {
            for x in 0..n {
                let f = fires[y * n + x];
                let mut got = 0i64;
                let mut add = |dx: i64, dy: i64| {
                    let (sx, sy) = (x as i64 + dx, y as i64 + dy);
                    if sx >= 0 && sy >= 0 && (sx as usize) < n && (sy as usize) < n {
                        got += fires[sy as usize * n + sx as usize];
                    }
                };
                add(1, 0);
                add(-1, 0);
                add(0, 1);
                add(0, -1);
                if moore {
                    add(1, 1);
                    add(1, -1);
                    add(-1, 1);
                    add(-1, -1);
                }
                h[y * n + x] += got - f * thresh;
            }
        }
        rounds += 1;
    }
}

fn sandpile_config(n: u32, log2: f32, moore: bool) -> SimConfig {
    let mut cfg = SimConfig::default();
    cfg.model = "sandpile".into();
    cfg.grid = SimGrid::Fixed { width: n, height: n };
    cfg.init = SimInit::Center;
    // The edges must be sinks. Under Clamp an edge site would receive
    // copies of its own topplings and the pile would GAIN mass.
    cfg.boundary = SimBoundary::Zero;
    cfg.model_params.insert("grains_log2".into(), log2);
    cfg.model_params.insert("neighbourhood".into(), if moore { 1.0 } else { 0.0 });
    cfg
}

/// The sandpile against an exact-integer CPU mirror of the same
/// parallel schedule: every cell of the final pile, both
/// neighbourhoods, and the round count pinned from both sides.
///
/// Three things are checked that a picture would not show. The pile is
/// IDENTICAL, not close -- heights are small integers and f32 counts
/// them exactly, so any disagreement is a rule difference. MASS IS
/// CONSERVED: the grid is large enough that nothing reaches the edge,
/// so the grains are all still there, and a boundary that created or
/// destroyed any would show up here. And the round count is TIGHT: the
/// pile is stable after `rounds` and is not after `rounds - 1`, which
/// is what lets a preset's step count be a measurement rather than a
/// guess.
#[test]
fn sandpile_matches_an_exact_cpu_mirror() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: usize = 128;
    const LOG2: u32 = 12;
    let grains = 1i64 << LOG2;
    for moore in [false, true] {
        let (rounds, want) = cpu_sandpile(N, grains, moore);
        let cfg = sandpile_config(N as u32, LOG2 as f32, moore);
        let mut r = SimRenderer::new(&device, &cfg, N as u32, N as u32);
        r.seed(&device, &queue, &cfg);
        r.run_steps(&device, &queue, &cfg, rounds);
        let got = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);

        let mut mass = 0i64;
        let mut differ = 0usize;
        let mut worst = 0i64;
        for k in 0..N * N {
            let h = got[k][0] as i64;
            mass += h;
            if h != want[k] {
                differ += 1;
                worst = worst.max((h - want[k]).abs());
            }
        }
        let edge: i64 = (0..N)
            .map(|i| got[i][0] as i64 + got[(N - 1) * N + i][0] as i64 + got[i * N][0] as i64 + got[i * N + N - 1][0] as i64)
            .sum();
        // One round short: still over-full somewhere, so the count is
        // not merely sufficient but exact.
        let mut r2 = SimRenderer::new(&device, &cfg, N as u32, N as u32);
        r2.seed(&device, &queue, &cfg);
        r2.run_steps(&device, &queue, &cfg, rounds - 1);
        let early = read_rgba32f(&device, &queue, r2.field_texture(), N as u32, N as u32);
        let thresh = if moore { 8.0 } else { 4.0 };
        let unstable = early.iter().filter(|p| p[0] >= thresh).count();

        let name = if moore { "Moore" } else { "von Neumann" };
        println!(
            "sandpile 2^{LOG2} {name}: {rounds} rounds, {differ} cells differ (worst {worst}), \
             mass {mass} of {grains}, {unstable} sites still over-full one round earlier"
        );
        assert_eq!(differ, 0, "{name}: the shader is not toppling the CPU rule");
        assert_eq!(edge, 0, "{name}: the pile reached the edge, so mass left the grid");
        assert_eq!(mass, grains, "{name}: mass is not conserved");
        assert!(unstable > 0, "{name}: stable a round early, so {rounds} overstates the count");
    }
}

/// The round counts the presets are built on, at the size they ship
/// at, and the confirmation that the shipped `steps` actually finishes
/// the pile.
///
/// The CPU mirror gives the count (the schedule is deterministic, and
/// `sandpile_matches_an_exact_cpu_mirror` establishes the GPU runs the
/// same schedule); the GPU then runs the preset's own step count and
/// must come out stable with its mass intact. 2^16 is a few seconds of
/// CPU, so this is a diagnostic rather than a gate.
#[test]
#[ignore]
fn sandpile_preset_step_counts() {
    let Some((device, queue)) = repro_device() else { return; };
    const N: usize = 256;
    for moore in [false, true] {
        let (rounds, want) = cpu_sandpile(N, 1 << 16, moore);
        let span = {
            let occ: Vec<usize> = (0..N * N).filter(|&k| want[k] > 0).collect();
            let (mut lo, mut hi) = (N, 0usize);
            for k in occ {
                lo = lo.min(k % N).min(k / N);
                hi = hi.max(k % N).max(k / N);
            }
            hi + 1 - lo
        };
        let model = crate::sim::model_or_default("sandpile");
        let preset = model.preset(if moore { "moore" } else { "pile" }).unwrap();
        let cfg = sandpile_config(N as u32, 16.0, moore);
        let mut r = SimRenderer::new(&device, &cfg, N as u32, N as u32);
        r.seed(&device, &queue, &cfg);
        r.run_steps(&device, &queue, &cfg, preset.steps);
        let got = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);
        let thresh = if moore { 8.0 } else { 4.0 };
        let unstable = got.iter().filter(|p| p[0] >= thresh).count();
        let mass: i64 = got.iter().map(|p| p[0] as i64).sum();
        println!(
            "sandpile 2^16 {}: {rounds} rounds, spans {span} cells; preset runs {} steps -> \
             {unstable} over-full, mass {mass} of 65536",
            if moore { "Moore" } else { "von Neumann" },
            preset.steps
        );
        assert_eq!(unstable, 0, "the preset's step count does not finish the pile");
        assert_eq!(mass, 1 << 16, "mass is not conserved at the preset's size");
        assert!(span < N, "the pile fills the grid at the shipped size");
    }
}

/// Invasion percolation must invade the CLUSTER, not a disc.
///
/// The rising-threshold rule is only equal to Wilkinson-Willemsen's
/// once its one-cell-per-step front has caught up with the threshold.
/// So this floods the grid on the CPU from the seed, through the
/// shader's OWN threshold field read back from the texture -- no RNG
/// to mirror, and the comparison is exact -- and asserts the GPU
/// reached that connected component and nothing else. A run that ends
/// while the front is still moving fails, which is what pins the
/// preset's step count.
///
/// It also reports the cluster's box-counting dimension, as a
/// RAMIFICATION check and not as evidence of criticality: at 256² the
/// measurement does not resolve the exact 91/48 = 1.896 (it reads
/// nearer 1.7 close to the threshold, a finite-size crossover, and
/// climbs past 1.89 once the cluster is merely dense). What it does
/// catch is the failure that matters -- a front that ran away from
/// the threshold and left a disc, which reads 2.
#[test]
fn invasion_percolation_invades_the_true_component() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: usize = 256;
    // The preset's ceiling, and the value the flood fill must use.
    const P_MAX: f32 = 0.60;
    let mut cfg = SimConfig::default();
    cfg.model = "invasion_percolation".into();
    cfg.grid = SimGrid::Fixed { width: N as u32, height: N as u32 };
    // The preset's own geometry and parameters: injection from an
    // edge, which is the paper's and, unlike a point seed, the same
    // picture from any seed.
    cfg.init = SimInit::Line;
    cfg.boundary = SimBoundary::Zero;
    cfg.seed = 3;
    let model = crate::sim::model_or_default("invasion_percolation");
    let preset = model.preset("front").unwrap();
    for (k, v) in preset.params {
        cfg.model_params.insert((*k).into(), *v);
    }
    let steps = preset.steps;

    let mut r = SimRenderer::new(&device, &cfg, N as u32, N as u32);
    r.seed(&device, &queue, &cfg);
    r.run_steps(&device, &queue, &cfg, steps);
    let got = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);

    // The component of {r < p_max} reachable from the injected edge,
    // by flood fill through the field's OWN thresholds read back from
    // the texture -- so there is no RNG to mirror and the comparison
    // is exact. `Line` seeds the top two rows.
    let mut want = vec![false; N * N];
    let mut stack: Vec<usize> = Vec::new();
    for k in (N - 2) * N..N * N {
        want[k] = true;
        stack.push(k);
    }
    while let Some(k) = stack.pop() {
        let (x, y) = ((k % N) as i64, (k / N) as i64);
        for (dx, dy) in [(1i64, 0i64), (-1, 0), (0, 1), (0, -1)] {
            let (sx, sy) = (x + dx, y + dy);
            if sx < 0 || sy < 0 || sx as usize >= N || sy as usize >= N {
                continue;
            }
            let j = sy as usize * N + sx as usize;
            if !want[j] && got[j][1] < P_MAX {
                want[j] = true;
                stack.push(j);
            }
        }
    }

    let invaded: Vec<bool> = got.iter().map(|p| p[0] > 0.5).collect();
    let missing = (0..N * N).filter(|&k| want[k] && !invaded[k]).count();
    let extra = (0..N * N).filter(|&k| !want[k] && invaded[k]).count();
    let size = invaded.iter().filter(|&&b| b).count();
    // When did the front finish? The last step any cell was invaded.
    let last = got
        .iter()
        .filter(|p| p[0] > 0.5)
        .map(|p| p[2] as u32)
        .max()
        .unwrap_or(0);

    // Box counting over the cluster's own bounding box.
    let occ: Vec<(usize, usize)> = (0..N * N).filter(|&k| invaded[k]).map(|k| (k % N, k / N)).collect();
    let (x0, x1) = (occ.iter().map(|c| c.0).min().unwrap(), occ.iter().map(|c| c.0).max().unwrap());
    let (y0, y1) = (occ.iter().map(|c| c.1).min().unwrap(), occ.iter().map(|c| c.1).max().unwrap());
    let side = (x1 - x0).max(y1 - y0) + 1;
    let mut pts = Vec::new();
    let mut b = 1usize;
    while b * 8 <= side {
        let mut seen = std::collections::HashSet::new();
        for &(x, y) in &occ {
            seen.insert(((x - x0) / b, (y - y0) / b));
        }
        pts.push(((1.0 / b as f64).ln(), (seen.len() as f64).ln()));
        b *= 2;
    }
    let m = pts.len() as f64;
    let (sx, sy) = (pts.iter().map(|p| p.0).sum::<f64>(), pts.iter().map(|p| p.1).sum::<f64>());
    let sxy: f64 = pts.iter().map(|p| p.0 * p.1).sum();
    let sxx: f64 = pts.iter().map(|p| p.0 * p.0).sum();
    let dim = (m * sxy - sx * sy) / (m * sxx - sx * sx);

    println!(
        "invasion percolation at p = {P_MAX}: {size} sites, {missing} missing, {extra} extra; \
         front finished at step {last} of {steps}; box dimension {dim:.3} (91/48 = 1.896)"
    );
    assert_eq!(extra, 0, "the GPU invaded {extra} sites outside the component");
    assert_eq!(missing, 0, "the front had not finished: {missing} sites of the component missing");
    assert!(last < steps, "the front was still moving at the last step");
    assert!(
        (1.60..1.85).contains(&dim),
        "box dimension {dim:.3}: the cluster is not ramified (a disc reads 2.0)"
    );
    assert!(
        (0.15..0.35).contains(&(size as f64 / (N * N) as f64)),
        "{size} sites of {} is not the pre-spanning cluster the preset is set for",
        N * N
    );
}

#[test]
fn snow_probe_tmp() {
    let Some((device, queue)) = repro_device() else { return; };
    const N: usize = 128;
    let mut cfg = SimConfig::default();
    cfg.model = "snowfake".into();
    cfg.grid = SimGrid::Fixed { width: N as u32, height: N as u32 };
    cfg.init = SimInit::Center;
    cfg.boundary = SimBoundary::Clamp;
    cfg.seed = 1;
    cfg.steps = 4000;
    let m = crate::sim::model_or_default("snowfake");
    for (k, v) in m.preset("simple_star").unwrap().params { cfg.model_params.insert((*k).into(), *v); }
    let count = |f: &Vec<[f32; 4]>| f.iter().filter(|p| p[0] > 0.5).count();

    // (a) exactly what render.rs does: seed, then 512-step batches.
    let mut r = SimRenderer::new(&device, &cfg, 512, 512);
    r.seed(&device, &queue, &cfg);
    let mut done = 0u32;
    while done < cfg.steps {
        let batch = 512.min(cfg.steps - done);
        r.run_steps(&device, &queue, &cfg, batch);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        done += batch;
    }
    let f = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);
    println!("(a) render.rs loop, out 512: attached {}", count(&f));

    // (b) one call, out = grid.
    let mut r2 = SimRenderer::new(&device, &cfg, N as u32, N as u32);
    r2.seed(&device, &queue, &cfg);
    r2.run_steps(&device, &queue, &cfg, 4000);
    let f2 = read_rgba32f(&device, &queue, r2.field_texture(), N as u32, N as u32);
    println!("(b) one 4000-step call, out 128: attached {}", count(&f2));

    // (c) one call, out 512 -- the only difference from (b).
    let mut r3 = SimRenderer::new(&device, &cfg, 512, 512);
    r3.seed(&device, &queue, &cfg);
    r3.run_steps(&device, &queue, &cfg, 4000);
    let f3 = read_rgba32f(&device, &queue, r3.field_texture(), N as u32, N as u32);
    println!("(c) one 4000-step call, out 512: attached {}", count(&f3));

    // (d) 512-batches, out = grid.
    let mut r4 = SimRenderer::new(&device, &cfg, N as u32, N as u32);
    r4.seed(&device, &queue, &cfg);
    let mut done = 0u32;
    while done < cfg.steps {
        let batch = 512.min(cfg.steps - done);
        r4.run_steps(&device, &queue, &cfg, batch);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        done += batch;
    }
    let f4 = read_rgba32f(&device, &queue, r4.field_texture(), N as u32, N as u32);
    println!("(d) 512-batches, out 128: attached {}", count(&f4));
}

/// The Gravner-Griffeath rule on the CPU, in the paper's four substeps
/// and its own order, against which the two-pass shader is checked.
///
/// Channels as the shader stores them: a, b, c, d.
fn cpu_snowfake(n: usize, p: &[f32; 8], steps: u32) -> Vec<[f32; 4]> {
    let (rho, beta, alpha, theta, kappa, mu, gamma) =
        (p[0], p[1], p[2], p[3], p[4], p[5], p[6]);
    let mut f = vec![[0.0f32, 0.0, 0.0, rho]; n * n];
    f[(n / 2) * n + n / 2] = [1.0, 0.0, 1.0, 0.0];
    // Offset-row hex neighbours, clamped at the grid edge exactly as
    // the Clamp boundary body does.
    let nb = |x: usize, y: usize, i: usize| -> usize {
        let k: i64 = if y % 2 == 1 { 0 } else { -1 };
        let (dx, dy): (i64, i64) = match i {
            0 => (-1, 0),
            1 => (1, 0),
            2 => (k, -1),
            3 => (k + 1, -1),
            4 => (k, 1),
            _ => (k + 1, 1),
        };
        let sx = (x as i64 + dx).clamp(0, n as i64 - 1) as usize;
        let sy = (y as i64 + dy).clamp(0, n as i64 - 1) as usize;
        sy * n + sx
    };
    for _ in 0..steps {
        // (i) diffusion, then (ii) freezing.
        let old = f.clone();
        for y in 0..n {
            for x in 0..n {
                let k = y * n + x;
                if old[k][0] > 0.5 {
                    continue;
                }
                let mut attached = 0;
                let mut sum = old[k][3];
                for i in 0..6 {
                    let q = old[nb(x, y, i)];
                    if q[0] > 0.5 {
                        attached += 1;
                        sum += old[k][3];
                    } else {
                        sum += q[3];
                    }
                }
                let mut d = sum * (1.0 / 7.0);
                let (mut b, mut c) = (old[k][1], old[k][2]);
                if attached > 0 {
                    b += (1.0 - kappa) * d;
                    c += kappa * d;
                    d = 0.0;
                }
                f[k] = [0.0, b, c, d];
            }
        }
        // (iii) attachment, then (iv) melting.
        let old = f.clone();
        for y in 0..n {
            for x in 0..n {
                let k = y * n + x;
                if old[k][0] > 0.5 {
                    continue;
                }
                let mut cnt = 0;
                let mut dsum = old[k][3];
                for i in 0..6 {
                    let q = old[nb(x, y, i)];
                    if q[0] > 0.5 {
                        cnt += 1;
                    }
                    dsum += q[3];
                }
                if cnt == 0 {
                    continue;
                }
                let attach = if cnt >= 4 {
                    true
                } else if cnt == 3 {
                    old[k][1] >= 1.0 || (dsum < theta && old[k][1] >= alpha)
                } else {
                    old[k][1] >= beta
                };
                f[k] = if attach {
                    [1.0, 0.0, old[k][1] + old[k][2], 0.0]
                } else {
                    [
                        0.0,
                        (1.0 - mu) * old[k][1],
                        (1.0 - gamma) * old[k][2],
                        old[k][3] + mu * old[k][1] + gamma * old[k][2],
                    ]
                };
            }
        }
    }
    f
}

fn snowfake_config(n: u32, preset: &str) -> (SimConfig, [f32; 8]) {
    let mut cfg = SimConfig::default();
    cfg.model = "snowfake".into();
    cfg.grid = SimGrid::Fixed { width: n, height: n };
    cfg.init = SimInit::Center;
    // Reflecting at the box edge, so no vapour leaves: the paper's own
    // conservation check only means anything under a closed boundary.
    // Closed, so no vapour enters or leaves and the paper's
    // conservation check means something. The paper uses a periodic
    // box; measured, Clamp gives the same mass drift to two figures,
    // and it degrades more gracefully when a crystal does reach the
    // edge -- a wrap grows the crystal into itself.
    cfg.boundary = SimBoundary::Clamp;
    cfg.seed = 1;
    let m = crate::sim::model_or_default("snowfake");
    for (k, v) in m.preset(preset).unwrap().params {
        cfg.model_params.insert((*k).into(), *v);
    }
    let packed = m.pack_params(&cfg);
    let mut p = [0.0f32; 8];
    p.copy_from_slice(&packed[..8]);
    (cfg, p)
}

/// The snowfake against a CPU mirror of the paper's four substeps, run
/// in the paper's order.
///
/// The shader merges them into two dispatches — freezing needs only the
/// vapour diffusion just left at the site itself, and melting only the
/// site's own masses — and this is what says that merge is exact. The
/// mirror keeps the substeps separate and clones the field between
/// them, so if (ii) or (iv) secretly read a neighbour, or if the pass
/// boundary sat in the wrong place, the two would part company.
///
/// Run long enough to attach thousands of cells, so the comparison
/// covers all three attachment cases and the knife-edge, not just the
/// first ring.
#[test]
fn snowfake_matches_a_cpu_mirror_of_the_four_substeps() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: usize = 64;
    const STEPS: u32 = 400;
    let (cfg, p) = snowfake_config(N as u32, "simple_star");
    let want = cpu_snowfake(N, &p, STEPS);

    let mut r = SimRenderer::new(&device, &cfg, N as u32, N as u32);
    r.seed(&device, &queue, &cfg);
    r.run_steps(&device, &queue, &cfg, STEPS);
    let got = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);

    let attached = got.iter().filter(|q| q[0] > 0.5).count();
    let wrong_a = (0..N * N).filter(|&k| (want[k][0] > 0.5) != (got[k][0] > 0.5)).count();
    // Both machines' mass drift, against the mass they started with.
    // The point of printing both: the drift is f32 arithmetic, not the
    // shader. A CPU mirror in the same precision drifts the same way,
    // so `the_snowfake_conserves_mass` is bounded by what f32 can do
    // rather than by what the paper's algebra says.
    let start = p[0] as f64 * (N * N - 1) as f64 + 1.0;
    let mass = |f: &Vec<[f32; 4]>| -> f64 {
        f.iter().map(|q| q[1] as f64 + q[2] as f64 + q[3] as f64).sum()
    };
    let (dc, dg) = ((mass(&want) - start) / start, (mass(&got) - start) / start);
    let mut worst = 0.0f32;
    for k in 0..N * N {
        for ch in 1..4 {
            worst = worst.max((want[k][ch] - got[k][ch]).abs());
        }
    }
    println!(
        "snowfake {STEPS} steps at {N}^2: {attached} attached, {wrong_a} disagree on attachment, \
         worst mass difference {worst:.2e}; mass drift CPU {dc:.1e} GPU {dg:.1e}"
    );
    assert!(attached > 400, "only {attached} cells attached; the run is too short to prove much");
    assert_eq!(wrong_a, 0, "the shader attaches different cells from the paper's rule");
    // f32 arithmetic in a different order on the two machines, over
    // hundreds of steps of a diffusion that mixes every cell.
    assert!(worst < 1e-3, "masses diverge from the mirror by {worst:.2e}");
}

/// "Without noise, note that total mass is conserved. Not only is this
/// property appealing from a physical perspective; it also helps in
/// debugging code and checking numerical stability." — the paper, §5.
///
/// So: b + c + d summed over the grid must still equal the vapour it
/// started with plus the seed's one unit of ice, after tens of
/// thousands of steps of a crystal that has eaten most of it. Every
/// substep moves mass between the three fields and none creates or
/// destroys any, so a sign or a factor wrong anywhere shows up here
/// even when the picture still looks like a snowflake.
#[test]
fn the_snowfake_conserves_mass() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: usize = 256;
    for preset in ["primitive", "simple_star", "dendrite_ends"] {
        let (cfg, p) = snowfake_config(N as u32, preset);
        let rho = p[0] as f64;
        let want = rho * (N * N - 1) as f64 + 1.0;

        let mut r = SimRenderer::new(&device, &cfg, N as u32, N as u32);
        r.seed(&device, &queue, &cfg);
        r.run_steps(&device, &queue, &cfg, 4_000);
        let f = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);
        let got: f64 = f.iter().map(|q| q[1] as f64 + q[2] as f64 + q[3] as f64).sum();
        let attached = f.iter().filter(|q| q[0] > 0.5).count();
        let rel = (got - want).abs() / want;
        println!(
            "snowfake {preset}: {attached} cells attached, mass {got:.1} against {want:.1} \
             ({:.1e} relative)",
            rel
        );
        assert!(attached > 1_000, "{preset}: barely grew, so this proves little");
        // Measured on Windows/Vulkan: about 1e-4 after 4,000 steps,
        // in EITHER direction -- -6.8e-5 at 64^2 over 400 steps,
        // +1.2e-4 at 256^2 over 4,000 -- because which way half an ulp
        // falls depends on rho's binary expansion. It is f32, not the
        // rule: the CPU mirror in
        // `snowfake_matches_a_cpu_mirror_of_the_four_substeps` drifts
        // the same way and prints both. A uniform far field is a fixed
        // point of the exact average but not of the rounded one, so
        // every untouched cell gains an ulp a step. The
        // bound is looser than that on purpose -- Metal's fast-math
        // may turn the division by 7 back into a multiply by the
        // biased reciprocal, which drifts about 1e-4 over this run --
        // and it is still three orders inside anything a wrong factor
        // or sign in one of the four substeps would produce.
        assert!(rel < 1e-3, "{preset}: mass is not conserved ({rel:.2e} relative)");
    }
}

/// The paper's own dimension estimator: the number of pattern sites
/// within radius r of the seed, against r, on a log-log fit.
///
/// "we have plotted the logarithm of the total number of points within
/// a certain radius as a function of the logarithm of this radius"
/// — Niemeyer, Pietronero and Wiesmann, on their figure 3.
///
/// The fit skips the innermost cells, where the count is a handful and
/// the lattice shows, and stops before the cluster's own edge, where it
/// saturates.
fn radial_dimension(pattern: &[bool], n: usize) -> f64 {
    let c = (n / 2) as f64;
    let mut radii: Vec<f64> = pattern
        .iter()
        .enumerate()
        .filter(|(_, &b)| b)
        .map(|(k, _)| {
            let (x, y) = ((k % n) as f64 - c, (k / n) as f64 - c);
            (x * x + y * y).sqrt()
        })
        .collect();
    radii.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let outer = radii[radii.len() * 9 / 10];
    let mut pts = Vec::new();
    let mut r = 6.0;
    while r <= outer {
        let cnt = radii.partition_point(|&v| v <= r) as f64;
        if cnt >= 10.0 {
            pts.push((r.ln(), cnt.ln()));
        }
        r *= 1.25;
    }
    let m = pts.len() as f64;
    let (sx, sy) = (pts.iter().map(|p| p.0).sum::<f64>(), pts.iter().map(|p| p.1).sum::<f64>());
    let sxy: f64 = pts.iter().map(|p| p.0 * p.1).sum();
    let sxx: f64 = pts.iter().map(|p| p.0 * p.0).sum();
    (m * sxy - sx * sy) / (m * sxx - sx * sx)
}

fn dbm_config(n: u32, eta: f32, steps: u32) -> SimConfig {
    let mut cfg = SimConfig::default();
    cfg.model = "dbm".into();
    cfg.grid = SimGrid::Fixed { width: n, height: n };
    cfg.init = SimInit::Center;
    cfg.boundary = SimBoundary::Clamp;
    cfg.seed = 5;
    cfg.steps = steps;
    for (k, v) in [
        ("eta", eta),
        ("relax", 20.0),
        ("surface_tension", 0.0),
        ("selection", 0.0),
        ("rate", 0.05),
        ("electrode", 0.0),
    ] {
        cfg.model_params.insert(k.into(), v);
    }
    cfg
}

/// THE GATE for phase 5: the dielectric breakdown model's Hausdorff
/// dimension against the paper's Table I.
///
///   eta   0     0.5          1            2
///   D     2     1.89±0.01    1.75±0.02    ~1.6
///
/// Measured the paper's way (N(r) against r) on its own scale — about
/// 5,000 sites. The eta = 1 row is also the row the whole phase turns
/// on: it is the photographed Lichtenberg figure's dimension (≈1.7)
/// and DLA's, which phase 4 measured at 1.753 by box counting.
///
/// Each run also confirms the selection rule is EXACT: one site joins
/// per step and no more, which is what the exponential race buys over
/// the parallel approximation the plan expected to ship.
#[test]
#[ignore]
fn dbm_dimension_matches_the_papers_table() {
    let Some((device, queue)) = repro_device() else { return; };
    const N: usize = 512;
    const STEPS: u32 = 5_000;
    println!("  eta      D      paper");
    let mut rows = Vec::new();
    // The paper averages "over five large samples of about 5000 points
    // each"; three is enough here to stop one realisation deciding it.
    // eta = 2 gets fewer sites because the structure is nearly linear
    // and 5,000 of them reach the electrode, where growth is no longer
    // free and the radial fit is truncated -- measured, that reads
    // 1.375 instead of 1.6.
    // The last column: whether the row is asserted. eta = 2 is not,
    // twice over. The paper's own value there is quoted from its
    // reference 13 rather than measured, and at this size our estimate
    // is dominated by sample noise -- measured, single runs at 1,500
    // sites gave 1.49, 1.63 and 1.54 for 20, 60 and 150 relaxation
    // sweeps, a spread of 0.14 with no trend in it. A number that
    // moves by 0.14 between samples cannot test a claim of 0.05.
    for (eta, want, label, steps, gated) in [
        (0.0f32, 2.0f64, "2", STEPS, true),
        (0.5, 1.89, "1.89 +- 0.01", STEPS, true),
        (1.0, 1.75, "1.75 +- 0.02", STEPS, true),
        (2.0, 1.6, "~1.6 (their ref 13)", 1_500, false),
    ] {
        let mut ds = Vec::new();
        let mut sites = 0usize;
        let mut pattern = Vec::new();
        for seed in 0..3u64 {
            let mut cfg = dbm_config(N as u32, eta, steps);
            cfg.seed = 5 + seed;
            let mut r = SimRenderer::new(&device, &cfg, N as u32, N as u32);
            r.seed(&device, &queue, &cfg);
            r.run_steps(&device, &queue, &cfg, steps);
            let f = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);
            pattern = f.iter().map(|q| q[3] > 0.5).collect();
            sites = pattern.iter().filter(|&&b| b).count();
            ds.push(radial_dimension(&pattern, N));
        }
        let d = ds.iter().sum::<f64>() / ds.len() as f64;
        // How close the pattern came to the electrode circle, which is
        // where growth stops being free.
        let c = (N / 2) as f64;
        let reach = (0..N * N)
            .filter(|&k| pattern[k])
            .map(|k| (((k % N) as f64 - c).powi(2) + ((k / N) as f64 - c).powi(2)).sqrt())
            .fold(0.0f64, f64::max);
        println!(
            "  {eta:3}   {d:.3}    {label:12}  ({sites} sites, reach {reach:.0} of {:.0})",
            N as f64 * 0.48
        );
        // One site per step, exactly: the seed is one cell and the
        // first step has no race to read.
        assert!(
            sites as u32 >= steps - 2 && sites as u32 <= steps,
            "eta {eta}: {sites} sites from {steps} steps -- the selection is not one per step"
        );
        // Only believe a dimension measured on a cluster that never
        // touched the electrode. The same discipline DLA's gate uses
        // for the walls.
        assert!(
            reach < N as f64 * 0.48 * 0.85,
            "eta {eta}: the pattern reached {reach:.0} of {:.0}, so the fit is truncated",
            N as f64 * 0.48
        );
        rows.push((eta, d, want, label, gated));
    }
    for (eta, d, want, label, gated) in &rows {
        if !gated {
            continue;
        }
        // 0.08, against the paper's own 0.01-0.02 statistical bars.
        // Measured, we read low by 0.02, 0.034 and 0.046 as eta rises
        // through 0, 0.5 and 1, and the eta = 1 value does not move
        // when the relaxation goes from 20 sweeps to 150 (1.704,
        // 1.718, 1.715), so it is not the solver. The paper says the
        // same of itself: "the possibility of a larger systematic
        // error due to the finite size of the systems considered
        // cannot be excluded".
        assert!(
            (d - want).abs() < 0.08,
            "eta {eta}: D = {d:.3}, the paper says {label}"
        );
    }
}

/// One site per step and no more, at the size the visual baselines use
/// — the cheap always-on half of the gate above.
///
/// This is what says the exponential race is the paper's rule rather
/// than an approximation of it: argmin of E/w with E ~ Exp(1) draws
/// exactly in proportion to w, and it needs only the global minimum
/// the reduce stage already computes. The plan had budgeted a prefix
/// scan for this and planned to ship a parallel approximation first.
#[test]
fn dbm_grows_exactly_one_site_per_step() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: usize = 128;
    const STEPS: u32 = 600;
    let cfg = dbm_config(N as u32, 1.0, STEPS);
    let mut r = SimRenderer::new(&device, &cfg, N as u32, N as u32);
    r.seed(&device, &queue, &cfg);
    let mut prev = 1usize;
    for k in 1..=6 {
        r.run_steps(&device, &queue, &cfg, STEPS / 6);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let f = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);
        let sites = f.iter().filter(|q| q[3] > 0.5).count();
        let grown = sites - prev;
        // The first step has no race behind it, so it grows nothing.
        let want = STEPS as usize / 6 - usize::from(k == 1);
        assert_eq!(grown, want, "batch {k}: {grown} sites joined, expected {want}");
        prev = sites;
    }
    // And the field is a solution of Laplace's equation between them:
    // every site outside the pattern and inside the electrode is the
    // average of its four neighbours, to the tolerance 20 warm-started
    // Jacobi sweeps reach.
    let f = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);
    let c = (N as f64 / 2.0, N as f64 / 2.0);
    let rad = N as f64 * 0.48;
    let mut worst = 0.0f64;
    let mut checked = 0;
    for y in 1..N - 1 {
        for x in 1..N - 1 {
            let k = y * N + x;
            if f[k][3] > 0.5 {
                continue;
            }
            let d = ((x as f64 - c.0).powi(2) + (y as f64 - c.1).powi(2)).sqrt();
            // Away from both the pattern and the electrode circle.
            if d > rad - 3.0 {
                continue;
            }
            let nb = [k + 1, k - 1, k + N, k - N];
            if nb.iter().any(|&j| f[j][3] > 0.5) {
                continue;
            }
            let avg: f64 = nb.iter().map(|&j| f[j][1] as f64).sum::<f64>() / 4.0;
            worst = worst.max((f[k][1] as f64 - avg).abs());
            checked += 1;
        }
    }
    println!(
        "DBM: {prev} sites in {STEPS} steps; Laplace residual {worst:.2e} over {checked} interior cells"
    );
    assert!(worst < 1e-3, "the potential is not a solution of Laplace's equation: {worst:.2e}");
}

/// Phase 5's models at 1080p, ms per step. A diagnostic, not a gate:
/// run it with `--ignored --nocapture` after touching any of them.
///
/// The dielectric breakdown model is the one to watch: a step is
/// `relax` + 2 dispatches plus the reduce, and one site joins per
/// step, so a 5,000-site figure is 5,000 of them.
#[test]
#[ignore]
fn phase5_step_cost_at_1080p() {
    let Some((device, queue)) = repro_device() else { return; };
    let (w, h) = (1920u32, 1080u32);
    for (model, boundary, init, extra) in [
        ("sandpile", SimBoundary::Zero, SimInit::Center, vec![("grains_log2", 20.0f32)]),
        ("invasion_percolation", SimBoundary::Zero, SimInit::Line, vec![]),
        ("snowfake", SimBoundary::Clamp, SimInit::Center, vec![]),
        ("dbm", SimBoundary::Clamp, SimInit::Center, vec![("relax", 5.0)]),
        ("dbm", SimBoundary::Clamp, SimInit::Center, vec![("relax", 20.0)]),
        ("dbm", SimBoundary::Clamp, SimInit::Center, vec![("relax", 50.0)]),
    ] {
        let mut cfg = SimConfig::default();
        cfg.model = model.into();
        cfg.grid = SimGrid::Fixed { width: w, height: h };
        cfg.init = init;
        cfg.boundary = boundary;
        let mut label = String::new();
        for (k, v) in &extra {
            cfg.model_params.insert((*k).into(), *v);
            label = format!("{k}={v}");
        }
        let mut r = SimRenderer::new(&device, &cfg, w, h);
        r.seed(&device, &queue, &cfg);
        r.run_steps(&device, &queue, &cfg, 20);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        const STEPS: u32 = 100;
        let t0 = std::time::Instant::now();
        r.run_steps(&device, &queue, &cfg, STEPS);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let ms = t0.elapsed().as_secs_f64() * 1e3 / STEPS as f64;
        println!("{model:<21} {label:<10} {ms:>8.3} ms/step  ({:.0} steps/s)", 1e3 / ms);
    }
}

fn fingering_config(n: u32, log_mobility: f32) -> SimConfig {
    let mut cfg = SimConfig::default();
    cfg.model = "fingering".into();
    cfg.grid = SimGrid::Fixed { width: n, height: n };
    cfg.init = SimInit::Line;
    cfg.boundary = SimBoundary::Clamp;
    cfg.seed = 3;
    for (k, v) in [
        ("log_mobility", log_mobility),
        ("speed", 0.3),
        ("diffusion", 0.02),
        ("disturbance", 0.05),
        ("relax", 20.0),
    ] {
        cfg.model_params.insert(k.into(), v);
    }
    cfg
}

/// Where the front is in each column: the first row from the outlet
/// side where c crosses one half. Returns (mean position, its standard
/// deviation across columns) -- the roughness of the interface.
fn front_roughness(f: &[[f32; 4]], n: usize) -> (f64, f64) {
    let pos: Vec<f64> = (0..n)
        .map(|x| {
            for y in 0..n {
                if f[y * n + x][2] >= 0.5 {
                    return y as f64;
                }
            }
            n as f64
        })
        .collect();
    let mean = pos.iter().sum::<f64>() / n as f64;
    let var = pos.iter().map(|p| (p - mean).powi(2)).sum::<f64>() / n as f64;
    (mean, var.sqrt())
}

/// Saffman and Taylor, 1958, on the interface between two fluids in a
/// porous medium or Hele-Shaw cell: it "is liable to be unstable if
/// the driving fluid is the less viscous of the two". This is that
/// sentence as a test, on Holzbecher's miscible formulation.
///
/// The same seeded disturbance is pushed by a thinner fluid (R > 0)
/// and by a thicker one (R < 0), and the interface's roughness -- the
/// spread of the front's position across the width -- must GROW in
/// the first case and SHRINK in the second. Both fronts advance the
/// same distance, because the drive is normalised to peak speed; only
/// their shape differs. A model that fingered both ways, or neither,
/// would be advecting a random field rather than modelling the
/// instability.
#[test]
fn fingering_is_unstable_only_when_the_driving_fluid_is_less_viscous() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: usize = 128;
    const STEPS: u32 = 300;
    let mut results = Vec::new();
    for r in [2.0f32, -2.0] {
        let cfg = fingering_config(N as u32, r);
        let mut sim = SimRenderer::new(&device, &cfg, N as u32, N as u32);
        sim.seed(&device, &queue, &cfg);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let f0 = read_rgba32f(&device, &queue, sim.field_texture(), N as u32, N as u32);
        sim.run_steps(&device, &queue, &cfg, STEPS);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let f1 = read_rgba32f(&device, &queue, sim.field_texture(), N as u32, N as u32);
        let (m0, s0) = front_roughness(&f0, N);
        let (m1, s1) = front_roughness(&f1, N);
        // Concentration stays a fraction, and every cell has one.
        assert!(f1.iter().all(|q| (0.0..=1.0).contains(&q[2]) && q[2].is_finite()));
        println!(
            "fingering R = {r:+}: front {m0:.1} -> {m1:.1}, roughness {s0:.2} -> {s1:.2} cells"
        );
        results.push((r, m0, m1, s0, s1));
    }
    let (_, _, adv_u, s0_u, s1_u) = results[0];
    let (_, _, adv_s, s0_s, s1_s) = results[1];
    assert!(adv_u < N as f64 - 20.0 && adv_s < N as f64 - 20.0, "both fronts should have advanced");
    assert!(s1_u > 4.0 * s0_u.max(0.5), "unstable case: roughness {s0_u:.2} -> {s1_u:.2} did not grow");
    assert!(s1_s < s0_s.max(1.0) * 1.5, "stable case: roughness {s0_s:.2} -> {s1_s:.2} did not stay flat");
    assert!(s1_u > 3.0 * s1_s, "the two cases should be unmistakably different: {s1_u:.2} vs {s1_s:.2}");
}

/// The warp's inverse map on the CPU: where destination cell `p` reads
/// from, in source coordinates. Mirrors the shader exactly.
fn warp_source(p: (usize, usize), n: usize, w: &crate::config::sim::SimWarp) -> (f32, f32) {
    let centre = (n as f32 - 1.0) * 0.5;
    let (dx, dy) = (p.0 as f32 - centre, p.1 as f32 - centre);
    let rim = (n as f32 * 0.5).max(1.0);
    let theta = w.rotation + w.flow * ((dx * dx + dy * dy).sqrt() / rim);
    let (qx, qy) = ((dx - w.pan_x) / w.zoom, (dy - w.pan_y) / w.zoom);
    let (cs, sn) = (theta.cos(), theta.sin());
    (centre + cs * qx + sn * qy, centre - sn * qx + cs * qy)
}

/// A periodic read of channel .y, as the Periodic boundary body does it.
fn wrap_read(f: &[[f32; 4]], n: usize, x: i64, y: i64) -> f32 {
    let (x, y) = (x.rem_euclid(n as i64) as usize, y.rem_euclid(n as i64) as usize);
    f[y * n + x][1]
}

/// The warp stage against a CPU resample of the same field through
/// the same affine, in both filters.
///
/// The field is invasion percolation's channel .y: a per-cell random
/// threshold the seed draws and the step NEVER writes, whatever the
/// step does to the other channels. So after one step that channel is
/// exactly the warp of what it was, with no assumption about the
/// model at all. Nearest must match EXACTLY (it moves values, it does
/// not make new ones); bilinear to float precision. The affine has
/// every term switched on at once -- zoom, rotation, pan and swirl --
/// so a wrong sign or a swapped axis anywhere in the inverse map shows
/// up.
#[test]
fn the_warp_matches_a_cpu_resample_of_the_same_field() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: usize = 64;
    for filter in [
        crate::config::sim::SimWarpFilter::Nearest,
        crate::config::sim::SimWarpFilter::Bilinear,
    ] {
        let mut cfg = SimConfig::default();
        cfg.model = "invasion_percolation".into();
        cfg.grid = SimGrid::Fixed { width: N as u32, height: N as u32 };
        cfg.init = SimInit::Center;
        cfg.boundary = SimBoundary::Periodic;
        cfg.seed = 9;
        cfg.warp = crate::config::sim::SimWarp {
            zoom: 0.97,
            rotation: 0.08,
            pan_x: 1.5,
            pan_y: -0.75,
            flow: 0.05,
            filter,
        };
        let mut r = SimRenderer::new(&device, &cfg, N as u32, N as u32);
        r.seed(&device, &queue, &cfg);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let before = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);
        r.run_steps(&device, &queue, &cfg, 1);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        let after = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);

        let mut worst = 0.0f32;
        let mut moved = 0usize;
        let mut wrong = 0usize;
        for y in 0..N {
            for x in 0..N {
                let (sx, sy) = warp_source((x, y), N, &cfg.warp);
                let want = match filter {
                    crate::config::sim::SimWarpFilter::Nearest => {
                        wrap_read(&before, N, (sx + 0.5).floor() as i64, (sy + 0.5).floor() as i64)
                    }
                    crate::config::sim::SimWarpFilter::Bilinear => {
                        let (ix, iy) = (sx.floor(), sy.floor());
                        let (fx, fy) = (sx - ix, sy - iy);
                        let (ix, iy) = (ix as i64, iy as i64);
                        let a = wrap_read(&before, N, ix, iy);
                        let b = wrap_read(&before, N, ix + 1, iy);
                        let c = wrap_read(&before, N, ix, iy + 1);
                        let d = wrap_read(&before, N, ix + 1, iy + 1);
                        (a * (1.0 - fx) + b * fx) * (1.0 - fy) + (c * (1.0 - fx) + d * fx) * fy
                    }
                };
                let got = after[y * N + x][1];
                let diff = (got - want).abs();
                worst = worst.max(diff);
                if diff > 1e-6 {
                    wrong += 1;
                }
                if (got - before[y * N + x][1]).abs() > 1e-6 {
                    moved += 1;
                }
            }
        }
        println!(
            "warp {filter:?}: worst difference {worst:.2e}, {wrong} cells off, {moved} of {} moved",
            N * N
        );
        assert!(moved > N * N / 2, "{filter:?}: the warp moved almost nothing");
        match filter {
            // A source that lands within an ulp of a cell boundary can
            // round the other way on the GPU; a couple in 4,096 is
            // rounding, a hundred is a wrong map.
            crate::config::sim::SimWarpFilter::Nearest => {
                assert!(wrong <= 4, "{filter:?}: {wrong} cells read from the wrong source cell")
            }
            crate::config::sim::SimWarpFilter::Bilinear => {
                assert!(worst < 2e-5, "{filter:?}: worst difference {worst:.2e}")
            }
        }
    }
}

/// An identity warp is no dispatch at all: the run is bit-identical
/// to one with no warp configured, which is what lets the field stay
/// exactly where it was for every model that does not ask.
#[test]
fn an_identity_warp_changes_nothing() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: usize = 64;
    let mut cfg = SimConfig::default();
    cfg.grid = SimGrid::Fixed { width: N as u32, height: N as u32 };
    cfg.seed = 4;
    let mut r = SimRenderer::new(&device, &cfg, N as u32, N as u32);
    r.seed(&device, &queue, &cfg);
    r.run_steps(&device, &queue, &cfg, 50);
    let plain = read_rgba32f(&device, &queue, r.field_texture(), N as u32, N as u32);

    // Identity rates with a non-default filter: still the identity.
    cfg.warp.filter = crate::config::sim::SimWarpFilter::Nearest;
    assert!(cfg.warp.is_identity());
    let mut r2 = SimRenderer::new(&device, &cfg, N as u32, N as u32);
    r2.seed(&device, &queue, &cfg);
    r2.run_steps(&device, &queue, &cfg, 50);
    let with = read_rgba32f(&device, &queue, r2.field_texture(), N as u32, N as u32);
    assert_eq!(plain, with, "an identity warp altered the run");
}

/// The matte turns cells below its cutoff into background: the colour
/// pass reports zero coverage for them, which is the channel the
/// shared tonemap composites the configured background into (and the
/// channel a transparent PNG takes its alpha from).
///
/// Run at 1:1 with Nearest upscale, so one output pixel is one grid
/// cell and the resolve filter cannot blur the answer. DLA's `.x` is
/// the occupancy channel -- 0 in the melt, distance-plus-one in the
/// cluster -- so a cutoff of 0.5 is exactly "is this cell part of the
/// dendrite". Both directions are checked, and the soft case in
/// between.
#[test]
fn the_matte_makes_background_of_the_cells_it_cuts() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: u32 = 128;
    let palette = test_palette(&device, &queue);
    let mut cfg = SimConfig::default();
    cfg.model = "dla".into();
    cfg.coloring = "age".into();
    cfg.grid = SimGrid::Fixed { width: N, height: N };
    cfg.init = SimInit::Center;
    cfg.boundary = SimBoundary::Clamp;
    cfg.seed = 2;
    let mut r = SimRenderer::new(&device, &cfg, N, N);
    r.seed(&device, &queue, &cfg);
    r.run_steps(&device, &queue, &cfg, 400);
    let field = read_rgba32f(&device, &queue, r.field_texture(), N, N);
    let frozen = field.iter().filter(|c| c[0] >= 0.5).count();
    assert!(frozen > 50 && frozen < (N * N) as usize / 2, "{frozen} frozen cells is not a cluster");

    // Off: every cell in the grid is drawn, which is what colourings
    // did before the matte existed.
    r.color(&device, &queue, &cfg, &palette);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let plain = read_rgba32f(&device, &queue, r.output_texture(), N, N);
    assert!(plain.iter().all(|p| p[3] == 1.0), "with no matte every cell should be figure");

    // On: the cluster is figure, the melt is background.
    cfg.matte = crate::config::sim::SimMatte {
        channel: crate::config::sim::SimMatteChannel::X,
        cutoff: 0.5,
        softness: 0.0,
        invert: false,
    };
    r.color(&device, &queue, &cfg, &palette);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let matted = read_rgba32f(&device, &queue, r.output_texture(), N, N);
    let wrong = (0..(N * N) as usize)
        .filter(|&k| {
            let want = if field[k][0] >= 0.5 { 1.0 } else { 0.0 };
            matted[k][3] != want
        })
        .count();
    assert_eq!(wrong, 0, "{wrong} cells took the wrong side of the matte");
    // And the colour of a drawn cell is untouched -- the matte decides
    // WHETHER a cell is drawn, not what colour it is.
    let lit = (0..(N * N) as usize).find(|&k| field[k][0] >= 0.5).unwrap();
    assert_eq!(matted[lit][..3], plain[lit][..3], "the matte changed a figure cell's colour");

    // Inverted: exactly the other cells.
    cfg.matte.invert = true;
    r.color(&device, &queue, &cfg, &palette);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let inverted = read_rgba32f(&device, &queue, r.output_texture(), N, N);
    let disagree = (0..(N * N) as usize)
        .filter(|&k| inverted[k][3] != 1.0 - matted[k][3])
        .count();
    assert_eq!(disagree, 0, "inverting the matte did not swap exactly the two sides");

    // Soft: the feather is centred on the cutoff, so a cell at the
    // cutoff is half covered and the edge does not move.
    cfg.matte.invert = false;
    cfg.matte.cutoff = 4.0;
    cfg.matte.softness = 8.0;
    r.color(&device, &queue, &cfg, &palette);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    let soft = read_rgba32f(&device, &queue, r.output_texture(), N, N);
    let partial = soft.iter().filter(|p| p[3] > 0.01 && p[3] < 0.99).count();
    assert!(partial > 20, "a soft matte should feather many cells, not {partial}");
    let mut worst = 0.0f32;
    for k in 0..(N * N) as usize {
        let want = ((field[k][0] - 4.0) / 8.0 + 0.5).clamp(0.0, 1.0);
        worst = worst.max((soft[k][3] - want).abs());
    }
    println!(
        "matte: {frozen} frozen of {}, {partial} feathered, worst coverage error {worst:.2e}",
        N * N
    );
    assert!(worst < 1e-6, "the feather does not match its own formula: {worst:.2e}");
}

/// Running to Max Steps in the app gives EXACTLY what exporting the
/// same config gives — which is the whole reason the cap exists.
///
/// The app advances a running simulation `steps_per_frame` at a time,
/// so a cap that is not a multiple of that would be overshot by part
/// of a frame. Here 250 is deliberately not a multiple of 100: the
/// frames run 100, 100, then FIFTY, and the field that leaves is bit
/// for bit the field `render_still` produces from the seed in one
/// run of 250.
///
/// The rest of the contract is checked around it: an uncapped run
/// (`steps == 0`) is never held back, a run that has reached the cap
/// is not stuck there — `steps_remaining` stops holding it, which is
/// what lets Run resume past — and a reseed arms the cap again.
#[test]
fn a_capped_run_stops_on_the_step_an_export_stops_on() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: u32 = 64;
    const CAP: u32 = 250;
    const PER_FRAME: u32 = 100;
    let palette = test_palette(&device, &queue);
    let mut cfg = SimConfig::default();
    cfg.grid = SimGrid::Fixed { width: N, height: N };
    cfg.seed = 6;
    cfg.steps = CAP;
    cfg.steps_per_frame = PER_FRAME;

    // The app's loop: frames of `steps_per_frame`, each clamped.
    let mut app = SimRenderer::new(&device, &cfg, N, N);
    let mut ran = Vec::new();
    for _ in 0..3 {
        let before = app.step_index();
        app.render_frame(&device, &queue, &cfg, &palette, PER_FRAME);
        ran.push(app.step_index() - before);
    }
    assert_eq!(
        ran,
        vec![PER_FRAME, PER_FRAME, CAP - 2 * PER_FRAME],
        "the cap should cut the third frame short rather than overshoot"
    );
    assert_eq!(app.step_index(), CAP);
    // The app pauses here; the rule that decides so is unit-tested in
    // `sim::tests`. What this test is for is that the field it pauses
    // on is the exported one.
    assert!(crate::sim::should_pause_at_limit(CAP, true, app.step_index()));

    // The export: exactly `steps` from the seed, in one run.
    let mut export = SimRenderer::new(&device, &cfg, N, N);
    export.render_still(&device, &queue, &cfg, &palette);
    let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
    assert_eq!(export.step_index(), CAP);

    let on_screen = read_rgba32f(&device, &queue, app.output_texture(), N, N);
    let exported = read_rgba32f(&device, &queue, export.output_texture(), N, N);
    assert_eq!(
        on_screen, exported,
        "what the app shows at Max Steps is not what an export of the same config gives"
    );

    // Past the cap, nothing holds the run back: this is Run resuming.
    assert_eq!(app.steps_remaining(&cfg), None);
    app.render_frame(&device, &queue, &cfg, &palette, PER_FRAME);
    assert_eq!(app.step_index(), CAP + PER_FRAME, "Run should carry on past the cap");

    // A reseed arms it again.
    app.request_seed();
    assert!(app.will_reseed(&cfg));
    app.render_frame(&device, &queue, &cfg, &palette, PER_FRAME);
    assert_eq!(app.step_index(), PER_FRAME, "a reseed restarts the count");

    // Uncapped: never held back, however many frames.
    let mut free = cfg.clone();
    free.steps = 0;
    let mut r = SimRenderer::new(&device, &cfg, N, N);
    assert_eq!(r.steps_remaining(&free), None);
    for _ in 0..3 {
        r.render_frame(&device, &queue, &free, &palette, PER_FRAME);
    }
    assert_eq!(r.step_index(), 3 * PER_FRAME, "an uncapped run should not stop");
}

/// A field belongs to the config that produced it.
///
/// Loading a file does not come through the delta path that decides
/// `UpdateType::SimReseed` — it replaces the whole config at once —
/// and before `SeedIdentity` the previous simulation's field, step
/// count and all simply carried over into the new one. This is that,
/// at the renderer: hand `render_frame` a config describing a
/// different field and it must start over, whatever nobody told it.
///
/// And the other half, which is just as important: a config that
/// describes the SAME field must NOT restart. Turning a model's
/// parameter is what its slider is for, and a reseed on every drag
/// would make Gray–Scott unusable.
#[test]
fn a_config_that_means_a_different_field_restarts_the_run() {
    let Some((device, queue)) = repro_device() else {
        eprintln!("no GPU adapter; skipping");
        return;
    };
    const N: u32 = 64;
    let palette = test_palette(&device, &queue);
    let mut a = SimConfig::default();
    a.grid = SimGrid::Fixed { width: N, height: N };
    a.seed = 11;
    a.steps = 0; // uncapped, so only the reseed can move the index

    let mut r = SimRenderer::new(&device, &a, N, N);
    r.render_frame(&device, &queue, &a, &palette, 120);
    assert_eq!(r.step_index(), 120);
    assert!(!r.will_reseed(&a), "nothing changed; the run should continue");

    // Each of these is a different field, and each must restart the
    // run on its own.
    for (label, cfg) in [
        ("model", SimConfig { model: "fitzhugh_nagumo".into(), ..a.clone() }),
        ("seed", SimConfig { seed: 12, ..a.clone() }),
        ("init", SimConfig { init: SimInit::Center, ..a.clone() }),
        ("boundary", SimConfig { boundary: SimBoundary::Zero, ..a.clone() }),
    ] {
        let mut r = SimRenderer::new(&device, &a, N, N);
        r.render_frame(&device, &queue, &a, &palette, 120);
        assert_eq!(r.step_index(), 120);
        assert!(r.will_reseed(&cfg), "{label}: should restart");
        r.render_frame(&device, &queue, &cfg, &palette, 10);
        assert_eq!(r.step_index(), 10, "{label}: the run did not restart from zero");
        // And the field is the new config's, not the old one's carried
        // forward: seeding the same config from scratch gives the same
        // ten steps.
        let mut fresh = SimRenderer::new(&device, &cfg, N, N);
        fresh.render_frame(&device, &queue, &cfg, &palette, 10);
        let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
        assert_eq!(
            read_rgba32f(&device, &queue, r.field_texture(), N, N),
            read_rgba32f(&device, &queue, fresh.field_texture(), N, N),
            "{label}: the loaded config inherited the previous run's field"
        );
    }

    // A model parameter is NOT a different field: the run continues.
    let mut tuned = a.clone();
    tuned.model_params.insert("feed".into(), 0.03);
    let mut r = SimRenderer::new(&device, &a, N, N);
    r.render_frame(&device, &queue, &a, &palette, 120);
    assert!(!r.will_reseed(&tuned), "a parameter change must not throw away the run");
    r.render_frame(&device, &queue, &tuned, &palette, 10);
    assert_eq!(r.step_index(), 130, "a parameter change restarted the run");

    // Nor is a colouring, a warp, a matte or the step cap.
    let mut recoloured = a.clone();
    recoloured.coloring = "age".into();
    recoloured.warp.zoom = 0.99;
    recoloured.matte.channel = crate::config::sim::SimMatteChannel::X;
    recoloured.steps = 500;
    assert!(!r.will_reseed(&recoloured), "presentation must not restart the run");
}

/// Every preset, rendered through the colouring it names, reported as
/// the contrast of the picture it makes.
///
/// A preset's colouring is only right if it DRAWS something: a channel
/// scale an order of magnitude off, or a channel that holds nothing
/// for this model, gives a flat frame — one colour everywhere — and no
/// invariant about names can see that. This one can. Ignored because
/// it runs every preset to its own step count; run it after touching
/// any preset's colouring.
///
/// The gate is deliberately weak (some coverage, some variation)
/// because "is this a good picture" is not a number. It catches the
/// failure that matters: a preset that comes up blank.
#[test]
#[ignore]
fn every_preset_draws_something() {
    let Some((device, queue)) = repro_device() else { return; };
    // 256, not 128: several models need room to be themselves —
    // Lenia's kernel is 13 cells across and its soup dies on a small
    // grid — and a preset that works at the size a user gets should
    // not be failed here for a size nobody runs.
    const N: u32 = 256;
    let palette = test_palette(&device, &queue);
    let mut flat = Vec::new();
    for m in crate::sim::MODELS {
        for pre in m.presets {
            let mut cfg = SimConfig::default();
            cfg.model = m.name.into();
            cfg.grid = SimGrid::Fixed { width: N, height: N };
            cfg.seed = 7;
            cfg.steps = pre.steps;
            // Selecting a model sets its dt (manager.rs), and so does
            // applying a preset; Lenia runs at 0.1 and dies at 1.0.
            cfg.dt = m.default_dt;
            for (k, v) in pre.params {
                cfg.model_params.insert((*k).into(), *v);
            }
            if let Some(init) = pre.init {
                cfg.init = init;
            }
            if let Some(c) = pre.coloring {
                cfg.coloring = c.into();
                for (k, v) in pre.coloring_params {
                    cfg.coloring_params.insert((*k).into(), *v);
                }
            }
            cfg.matte = pre.matte.unwrap_or_default();
            let mut r = SimRenderer::new(&device, &cfg, N, N);
            r.render_still(&device, &queue, &cfg, &palette);
            let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
            let out = read_rgba32f(&device, &queue, r.output_texture(), N, N);

            // Luminance of what is actually drawn, weighted by
            // coverage: a matte-less sim covers everything, so this is
            // just the picture.
            let lum: Vec<f32> = out.iter().map(|p| (p[0] + p[1] + p[2]) / 3.0 * p[3]).collect();
            let mean = lum.iter().sum::<f32>() / lum.len() as f32;
            let sd = (lum.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / lum.len() as f32).sqrt();
            let covered = out.iter().filter(|p| p[3] > 0.5).count() as f32 / lum.len() as f32;
            assert!(
                out.iter().all(|p| p.iter().all(|v| v.is_finite())),
                "{}/{}: the coloured output is not finite",
                m.name,
                pre.name
            );
            println!(
                "{:<22} {:<16} {:>6} steps  mean {mean:.3}  sd {sd:.3}  covered {:.0}%",
                m.name,
                pre.name,
                pre.steps,
                covered * 100.0
            );
            if sd < 0.01 {
                flat.push(format!("{}/{} (sd {sd:.4})", m.name, pre.name));
            }
        }
    }
    assert!(flat.is_empty(), "these presets draw a flat frame: {flat:#?}");
}
