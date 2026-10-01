//! **The gates of persistent orbits** (docs/projects/persistent-orbits.md
//! §6): each thread's orbit is kept between dispatches, so the picture no
//! longer depends on how the work is sliced into them. Every gate runs the
//! same measurement with the feature off too, which is the baseline it is
//! measured against (step 0), and compares by density
//! (`renderer::density_gates`), never by the tone map.
#![cfg(test)]

use crate::config::FractalConfig;
use crate::renderer::density_gates::{compare_blocks, print_compared, render_blocks, Blocks, Compared, Run};

fn device() -> (wgpu::Device, wgpu::Queue) {
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
    let al = adapter.limits();
    let mut limits = wgpu::Limits::default();
    limits.max_storage_buffers_per_shader_stage = al.max_storage_buffers_per_shader_stage;
    limits.max_storage_buffer_binding_size = al.max_storage_buffer_binding_size;
    limits.max_buffer_size = al.max_buffer_size;
    pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("orbit gates"),
        required_features: wgpu::Features::CLEAR_TEXTURE,
        required_limits: limits,
        ..Default::default()
    }))
    .expect("device")
}

/// The band flame (`output/flame-zoom/julian-disc-blue1.fflame`, untargeted
/// julian-disc at zoom 1,598), with a palette blue in entries 0-1 and red
/// past them: a restarted orbit's colour-0 run is the blue light.
fn band_flame() -> Option<FractalConfig> {
    let text = std::fs::read_to_string("output/flame-zoom/julian-disc-blue1.fflame").ok()?;
    let mut cfg: FractalConfig = serde_json::from_str(&text).expect("a config");
    cfg.render_mode = crate::scene::transforms::RenderMode::TwoD;
    cfg.cylinder_targeting = false;
    cfg.levels_enabled = false;
    cfg.auto_exposure = false;
    let stop = |position: f32, color: [f32; 3]| crate::scene::palette::ColorStop { position, color };
    cfg.palette.stops = vec![stop(0.0, [0.0, 0.0, 1.0]), stop(1.99 / 256.0, [0.0, 0.0, 1.0]), stop(2.0 / 256.0, [1.0, 0.0, 0.0]), stop(1.0, [1.0, 0.0, 0.0])];
    Some(cfg)
}

/// A Sierpinski gasket filling the view: every sample lands, so the whole
/// view's density per iteration is exactly one when the count is right.
fn gasket() -> FractalConfig {
    let mut cfg = FractalConfig::default();
    cfg.flame.transforms.clear();
    for (i, (e, f)) in [(0.0f32, 0.0f32), (0.5, 0.0), (0.25, 0.5)].into_iter().enumerate() {
        let mut t = crate::scene::transforms::Transform::default();
        (t.a, t.b, t.c, t.d, t.e, t.f) = (0.5, 0.0, 0.0, 0.5, e, f);
        t.weight = 1.0;
        t.color = i as f32 / 2.0;
        t.variations.clear();
        t.variation_order.clear();
        t.set_variation("linear", 1.0);
        cfg.flame.transforms.push(t);
    }
    cfg.zoom = 1.6;
    (cfg.pan_x, cfg.pan_y) = (-0.25, -0.25);
    cfg.levels_enabled = false;
    cfg
}

/// The share of a render's light that is blue, pooled over replicates.
fn blue_share(b: &Blocks) -> f64 {
    let sum = |v: &[Vec<f64>]| v.iter().map(|r| r.iter().sum::<f64>()).sum::<f64>();
    let (r, g, bl) = (sum(&b.light[0]), sum(&b.light[1]), sum(&b.light[2]));
    bl / (r + g + bl).max(f64::MIN_POSITIVE)
}

/// Two renders compared in density and in each colour's light, each tiered
/// by its own reference: a block dense in density can hold almost no blue,
/// and is judged as the faint block of blue it is.
fn compare_all(x: &Blocks, y: &Blocks) -> [Compared; 4] {
    [
        compare_blocks(&x.density, &y.density, &y.density),
        compare_blocks(&x.light[0], &y.light[0], &y.light[0]),
        compare_blocks(&x.light[1], &y.light[1], &y.light[1]),
        compare_blocks(&x.light[2], &y.light[2], &y.light[2]),
    ]
}

/// **The picture does not depend on iterations per thread** (§6's general
/// invariant; the band is its first case): the band flame at 64 and 256
/// iterations a thread against 1,024, at the same total work, block by
/// block in density and in each colour's light. Restarted orbits fail it
/// -- a thread 20 iterations past a restart is not yet on the attractor --
/// and persistent ones must not.
#[test]
#[ignore = "needs a GPU; reads output/flame-zoom"]
fn the_picture_does_not_depend_on_iterations_per_thread() {
    let Some(cfg) = band_flame() else {
        println!("  no julian-disc-blue1.fflame");
        return;
    };
    const N: u32 = 128;
    let (device, queue) = device();
    // 256 workgroups x 4,096 iterations x 4 frames a replicate, however sliced.
    let run = |ipt: u32, persistent: bool| {
        render_blocks(&device, &queue, &cfg, N, &Run { ipt, frames: (16384 / ipt) as usize, reps: 16, persistent, ..Default::default() })
    };
    let mut failures = Vec::new();
    for persistent in [false, true] {
        let reference = run(1024, persistent);
        println!("== persistent orbits {}: blue share {:.4} at 1,024 a thread", if persistent { "ON" } else { "off" }, blue_share(&reference));
        for ipt in [64u32, 256] {
            let b = run(ipt, persistent);
            let [d, red, _, blue] = compare_all(&b, &reference);
            println!("  {ipt} a thread: blue share {:.4}", blue_share(&b));
            print_compared(&format!("density, {ipt} against 1,024"), &d);
            print_compared(&format!("red light, {ipt} against 1,024"), &red);
            print_compared(&format!("blue light, {ipt} against 1,024"), &blue);
            if persistent {
                for (what, c) in [("density", &d), ("red light", &red)] {
                    if c.worst_dense > 0.05 {
                        failures.push(format!("{ipt} a thread: a dense block's {what} is {:.3} off", c.worst_dense));
                    }
                    let (_, _, mx, _, my, _, z) = c.pooled[0];
                    if z.abs() > 4.0 && (mx / my - 1.0).abs() > 0.01 {
                        failures.push(format!("{ipt} a thread: the whole view's {what} is {:.4} of 1,024's", mx / my));
                    }
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// **The band** (§6), against the truth: the band flame at the app's 256
/// iterations a thread and at 64, against 16,384 a thread with restarts --
/// whose burn-in is 64 times rarer, as near the converged chaos game as a
/// render gets -- block by block in density and in each colour's light.
/// Restarted orbits at 256 are the blue band (measured: blue light 25% short
/// overall, and 19 blocks of density off by up to 62%); persistent ones are
/// the long orbits' picture.
///
/// To 10% a block, not 5%: the reference restarts too, once every 16,384
/// iterations, and at 16 replicates its own residue shows -- single blocks
/// up to 6% from persistent orbits at 64 and at 256 a thread, which agree
/// with each other to 4 standard errors (`dbg_short_dispatches_precisely`).
/// And blue on the whole view only, since blue is the residue: block by
/// block the reference's band holds up to 48% more of it.
#[test]
#[ignore = "needs a GPU; reads output/flame-zoom"]
fn the_band_is_gone() {
    let Some(cfg) = band_flame() else {
        println!("  no julian-disc-blue1.fflame");
        return;
    };
    const N: u32 = 128;
    let (device, queue) = device();
    let run = |ipt: u32, persistent: bool| {
        render_blocks(&device, &queue, &cfg, N, &Run { ipt, frames: (65536 / ipt) as usize, reps: 16, persistent, ..Default::default() })
    };
    let truth = run(16384, false);
    let mut failures = Vec::new();
    for (ipt, persistent) in [(256u32, false), (256, true), (64, true)] {
        let b = run(ipt, persistent);
        let tag = format!("{ipt} a thread, {}", if persistent { "persistent" } else { "restarted" });
        let blue_ratio = blue_share(&b) / blue_share(&truth);
        println!("  {tag}: blue share {:.4} (the long orbits' {:.4}), ratio {blue_ratio:.3}", blue_share(&b), blue_share(&truth));
        let [d, red, _, blue] = compare_all(&b, &truth);
        for (what, c) in [("density", &d), ("red light", &red), ("blue light", &blue)] {
            print_compared(&format!("{what}, {tag}, against 16,384 restarted"), c);
        }
        if persistent {
            for (what, c) in [("density", &d), ("red light", &red)] {
                if c.worst_dense > 0.10 || c.worst_middle > 0.20 {
                    failures.push(format!("{tag}: a block's {what} is {:.3} off", c.worst_dense.max(c.worst_middle)));
                }
            }
            // Blue on the whole view only: blue is the light a restart
            // makes, and the reference restarts too -- every 16,384
            // iterations, so its band blocks hold a little of it.
            if (blue_ratio - 1.0).abs() > 0.10 {
                failures.push(format!("{tag}: the blue share is {blue_ratio:.3} of the long orbits'"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// **How long a restart shows** (§4, question 2). The first dispatch
/// after a restart is a restarted dispatch, band and all, and stays in the
/// accumulator as `1/k` of it after `k` dispatches. The band flame after
/// 1, 10 and 100 app frames from a fresh start (each replicate its own
/// renderer, so each pays the restart), against the long orbits.
#[test]
#[ignore = "measurement: needs a GPU and output/flame-zoom"]
fn dbg_how_long_a_restart_shows() {
    let Some(cfg) = band_flame() else {
        println!("  no julian-disc-blue1.fflame");
        return;
    };
    const N: u32 = 128;
    let (device, queue) = device();
    let truth = render_blocks(&device, &queue, &cfg, N, &Run { ipt: 16384, frames: 4, reps: 8, persistent: false, ..Default::default() });
    for frames in [1usize, 10, 100] {
        let mut b = Blocks { density: Vec::new(), light: [Vec::new(), Vec::new(), Vec::new()], image: Vec::new() };
        for _ in 0..8 {
            let one = render_blocks(&device, &queue, &cfg, N, &Run { ipt: 256, frames, reps: 1, persistent: true, ..Default::default() });
            b.density.extend(one.density);
            for c in 0..3 {
                b.light[c].extend(one.light[c].iter().cloned());
            }
        }
        println!("  {frames} frames from a restart: blue share {:.4} (the long orbits' {:.4})", blue_share(&b), blue_share(&truth));
        let [d, _, _, blue] = compare_all(&b, &truth);
        print_compared(&format!("density, {frames} frames"), &d);
        print_compared(&format!("blue light, {frames} frames"), &blue);
    }
}

/// **A varying dispatch keeps its brightness** (§6, Brightness; step 6):
/// the governor changes the workgroup count and, since it shortens every
/// dispatch first, the length too, between frames, and the count
/// (`OrbitFuses`) must follow every thread's burn-in through it. Widths of
/// 256, 16, 128, 64, 256 and 8 workgroups, then the same with lengths of
/// 64, 256, 1,000, 64, 20 and 512 a thread -- one under the burn-in --
/// against 256 workgroups of 64 every time, by density.
#[test]
#[ignore = "needs a GPU"]
fn a_varying_dispatch_keeps_its_brightness() {
    let cfg = gasket();
    const N: u32 = 128;
    let (device, queue) = device();
    let steady = render_blocks(&device, &queue, &cfg, N, &Run { ipt: 64, frames: 24, persistent: true, ..Default::default() });
    let widths = [256, 16, 128, 64, 256, 8];
    for (what, lengths) in [("width", &[][..]), ("width and length", &[64, 256, 1000, 64, 20, 512][..])] {
        let varying = render_blocks(&device, &queue, &cfg, N, &Run { ipt: 64, frames: 48, persistent: true, workgroups: &widths, lengths, ..Default::default() });
        let c = compare_blocks(&varying.density, &steady.density, &steady.density);
        print_compared(&format!("varying {what} against 256 workgroups of 64, persistent"), &c);
        let (_, _, mx, _, my, _, z) = c.pooled[0];
        assert!(!(z.abs() > 4.0 && (mx / my - 1.0).abs() > 0.005), "varying {what}: the whole view's density is {:.4} of the steady dispatch's", mx / my);
        assert!(c.worst_dense < 0.05, "varying {what}: a dense block is {:.3} off", c.worst_dense);
    }
}

/// **Restarts** (§6): a flame edit starts a new generation, and a render of
/// flame A then flame B is a fresh render of B; a pan or a zoom keeps the
/// generation -- the attractor does not depend on the view.
#[test]
#[ignore = "needs a GPU"]
fn an_edit_restarts_the_orbits_and_a_pan_does_not() {
    let (device, queue) = device();
    let a = gasket();
    let mut b = a.clone();
    b.flame.transforms[0].a *= 0.9;
    let mut r = crate::renderer::FlameRenderer::with_palette_size(&device, &queue, wgpu::TextureFormat::Rgba8Unorm, 64, 64, &a.flame, a.palette_size);
    r.set_persistent_orbits(true);
    let load = |r: &mut crate::renderer::FlameRenderer, cfg: &FractalConfig| {
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("load") });
        r.load_config(&device, &mut enc, &queue, cfg, &cfg.palette, 1, 0);
        queue.submit(Some(enc.finish()));
    };
    let frame = |r: &mut crate::renderer::FlameRenderer, cfg: &FractalConfig| {
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        let k = r.compute_pass(&mut enc, &queue, &device, 64, 256, 20, cfg.zoom, cfg.pan_x as f32, cfg.pan_y as f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, cfg.speed_factor, true, false);
        r.accumulate_pass(&mut enc, &queue, &device, k);
        queue.submit(Some(enc.finish()));
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        k
    };
    load(&mut r, &a);
    let first = frame(&mut r, &a);
    let gen_a = r.orbit_generation();
    let second = frame(&mut r, &a);
    assert_eq!(first, 64 * 64 * (256 - 20), "the first dispatch pays the burn-in");
    assert_eq!(second, 64 * 64 * 256, "a resumed dispatch pays none");
    // A pan and a zoom: the same generation, no burn-in.
    let mut moved = a.clone();
    moved.pan_x += 0.3;
    moved.zoom *= 2.0;
    let after_pan = frame(&mut r, &moved);
    assert_eq!(r.orbit_generation(), gen_a, "a pan restarted the orbits");
    assert_eq!(after_pan, 64 * 64 * 256, "a pan paid a burn-in");
    // An edit: a new generation, and the burn-in again.
    load(&mut r, &b);
    let after_edit = frame(&mut r, &b);
    assert_ne!(r.orbit_generation(), gen_a, "an edit kept the orbits");
    assert_eq!(after_edit, 64 * 64 * (256 - 20), "a restart pays the burn-in once");
}

/// **The count is the GPU's** (§3.5): the CPU's count (`OrbitFuses`) of plotted
/// samples equals the shader's own tally of plot attempts, exactly,
/// through a sequence whose width and length both vary -- burn-in longer
/// than a dispatch, threads above a narrow dispatch waiting, a wider one
/// starting new ones -- through an edit, and through a refresh (the hub
/// flame, whose xaos walk decides its orbits' groups). The tally is the frame
/// coverage's (`FRAME_COVERAGE`, switched on by auto exposure), which
/// counts every iteration past the burn-in that is plotted; a gasket at
/// full opacity never respawns, so it is every one -- but for the opacity
/// draw's one in 2^25 (see the assertion).
#[test]
#[ignore = "GPU gate: persistent orbits"]
fn the_count_is_the_gpus() {
    let (device, queue) = device();
    let mut a = gasket();
    a.auto_exposure = true;
    let mut b = a.clone();
    b.flame.transforms[1].e *= 0.9;
    // The hub refreshes a quarter of its threads each dispatch.
    let mut hub = hub_groups(0.1);
    hub.auto_exposure = true;
    assert!(hub.flame.xaos_walk_splits_orbits());
    let mut r = crate::renderer::FlameRenderer::with_palette_size(&device, &queue, wgpu::TextureFormat::Rgba8Unorm, 64, 64, &a.flame, a.palette_size);
    r.set_persistent_orbits(true);
    let mut checked = 0;
    for cfg in [&a, &b, &hub] {
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("load") });
        r.load_config(&device, &mut enc, &queue, cfg, &cfg.palette, 1, 0);
        queue.submit(Some(enc.finish()));
        for (groups, ipt) in [(64u32, 8u32), (64, 8), (16, 64), (256, 16), (128, 64), (8, 1024), (512, 64), (256, 256)] {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
            let counted = r.compute_pass(&mut enc, &queue, &device, groups, ipt, 20, cfg.zoom, cfg.pan_x as f32, cfg.pan_y as f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, cfg.speed_factor, true, false);
            queue.submit(Some(enc.finish()));
            let words = r.read_coverage_blocking(&device, &queue).expect("the coverage counters");
            // Exact but for the opacity draw: `rng_nextf` returns exactly
            // 1.0 one draw in 2^25 (`f32` rounds the top 128 `u32`s up to
            // 2^32), and `1.0 < opacity` drops that plot even at opacity 1.
            let gpu = words[1] as u64;
            assert!(
                gpu <= counted && counted - gpu <= 2 + counted / 10_000_000,
                "{groups} workgroups of {ipt}: the CPU counted {counted}, the GPU plotted {gpu}"
            );
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("accumulate") });
            r.accumulate_pass(&mut enc, &queue, &device, counted);
            queue.submit(Some(enc.finish()));
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
            checked += 1;
        }
    }
    println!("{checked} dispatches: the CPU's count is the GPU's");
}

/// A cubic Julia set coloured by `cubic_julia`'s Branch Blend: a register
/// carried from call to call in the variation's thread state, pulled
/// toward each branch's colour at `color_speed`. At 0.02 it remembers
/// about 50 calls, longer than the burn-in, so a restarted orbit plots
/// with the register still near the 0 it was reset to.
fn branch_blend() -> FractalConfig {
    let mut cfg = FractalConfig::default();
    cfg.flame.transforms.clear();
    let mut t = crate::scene::transforms::Transform::default();
    (t.a, t.b, t.c, t.d, t.e, t.f) = (1.0, 0.0, 0.0, 1.0, 0.0, 0.0);
    t.weight = 1.0;
    t.direct_color = 1.0;
    t.variations.clear();
    t.variation_order.clear();
    t.set_variation("cubic_julia", 1.0);
    t.set_variation_param("cubic_julia", "dc_mode", 2.0);
    t.set_variation_param("cubic_julia", "color_speed", 0.02);
    cfg.flame.transforms.push(t);
    cfg.zoom = 0.6;
    (cfg.pan_x, cfg.pan_y) = (0.0, 0.0);
    cfg.levels_enabled = false;
    cfg
}

/// **Stateful registers** (§6): a variation's per-thread state is part of
/// the orbit. The Branch Blend flame at 64 and 256 iterations a thread
/// against 1,024, at the same total work, in density and each colour's
/// light. Restarted, every dispatch resets the register; persistent, it is
/// carried with the point.
#[test]
#[ignore = "GPU gate: persistent orbits"]
fn a_stateful_register_does_not_depend_on_iterations_per_thread() {
    const N: u32 = 128;
    let (device, queue) = device();
    let cfg = branch_blend();
    let mut failures = Vec::new();
    for persistent in [false, true] {
        let run = |ipt: u32| render_blocks(&device, &queue, &cfg, N, &Run { ipt, frames: (8192 / ipt) as usize, reps: 16, persistent, ..Default::default() });
        let long = run(1024);
        let how = if persistent { "persistent" } else { "restarted" };
        for ipt in [64u32, 256] {
            let b = run(ipt);
            let [d, red, green, blue] = compare_all(&b, &long);
            for (what, c) in [("density", &d), ("red light", &red), ("green light", &green), ("blue light", &blue)] {
                print_compared(&format!("{what}, {ipt} a thread against 1,024, {how}"), c);
                if persistent && (c.worst_dense > 0.05 || c.worst_middle > 0.15) {
                    failures.push(format!("{how}, {ipt} a thread: a block's {what} is {:.3} off", c.worst_dense.max(c.worst_middle)));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

struct Quiet;

/// Mixed into every exported replicate's seeds, so a rerun with another
/// salt draws other replicates.
const SEED_SALT: u32 = 0x5EED_0001;

impl crate::export::ExportReporter for Quiet {
    fn progress(&mut self, _fraction: f32, _detail: &str) {}
}

/// `cfg` exported at `n` x `n` through the high-res exporter
/// (`export/high_res.rs`, the tiled sample-emit path), `reps` times on
/// independent seeds, as `render_blocks` renders: each replicate's 16x16
/// block sums of density and each colour's light, per sample the export
/// counted. Returns the storage buffers the exporter's shader binds too.
fn export_blocks(cfg: &FractalConfig, n: u32, ipt: u32, iterations: u64, persistent: bool, reps: usize) -> (Blocks, usize) {
    let side = (n / 16) as usize;
    let mut out = Blocks { density: Vec::new(), light: [Vec::new(), Vec::new(), Vec::new()], image: Vec::new() };
    let mut storage = 0;
    for rep in 0..reps {
        let mut ex = pollster::block_on(crate::export::HighResExporter::new_with_orbits(cfg, n, n, Some(ipt), persistent)).expect("an exporter");
        ex.set_seed_base((rep as u32 + 1).wrapping_mul(0x9E37_79B9) ^ SEED_SALT);
        storage = ex.storage_bindings();
        out.image = pollster::block_on(ex.export(cfg, iterations, false, false, &mut Quiet)).expect("an export");
        let (hist, count) = ex.last_density().expect("the export's histogram");
        let per = 1.0 / (*count).max(1) as f64;
        let mut d = vec![0.0f64; side * side];
        let mut l = [vec![0.0f64; side * side], vec![0.0f64; side * side], vec![0.0f64; side * side]];
        for (i, px) in hist.iter().enumerate() {
            let b = (i / n as usize / 16) * side + i % n as usize / 16;
            d[b] += px.count * per;
            l[0][b] += px.r * per;
            l[1][b] += px.g * per;
            l[2][b] += px.b * per;
        }
        out.density.push(d);
        for (c, v) in l.into_iter().enumerate() {
            out.light[c].push(v);
        }
    }
    (out, storage)
}

/// **The exporter does not depend on iterations per thread** (§3.8, step
/// 3's gate): the band flame through the high-res exporter at 64 and 256
/// iterations a thread against 1,024, at the same total work, in density
/// and each colour's light; restarted for the baseline, and persistent,
/// which must agree. Its shader binds at most 10 storage buffers.
#[test]
#[ignore = "GPU gate: persistent orbits; reads output/flame-zoom"]
fn the_exporter_does_not_depend_on_iterations_per_thread() {
    let Some(cfg) = band_flame() else {
        println!("  no julian-disc-blue1.fflame");
        return;
    };
    const N: u32 = 128;
    const ITERATIONS: u64 = 128 * 64 * 1024 * 16;
    const REPS: usize = 16;
    let mut failures = Vec::new();
    for persistent in [false, true] {
        let how = if persistent { "persistent" } else { "restarted" };
        let (long, storage) = export_blocks(&cfg, N, 1024, ITERATIONS, persistent, REPS);
        println!("  {how}: the exporter's shader binds {storage} storage buffers");
        if storage > 10 {
            failures.push(format!("{how}: the exporter binds {storage} storage buffers"));
        }
        for ipt in [64u32, 256] {
            let (b, _) = export_blocks(&cfg, N, ipt, ITERATIONS, persistent, REPS);
            println!("  {ipt} a thread, {how}: blue share {:.4} (1,024's {:.4})", blue_share(&b), blue_share(&long));
            let [d, red, _, blue] = compare_all(&b, &long);
            for (what, c) in [("density", &d), ("red light", &red), ("blue light", &blue)] {
                print_compared(&format!("{what}, exported at {ipt} a thread against 1,024, {how}"), c);
                if persistent && (c.worst_dense > 0.05 || c.worst_middle > 0.15) {
                    failures.push(format!("{how}, {ipt} a thread: a block's {what} is {:.3} off", c.worst_dense.max(c.worst_middle)));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// **An export counts what the app counts** with persistent orbits: the
/// band flame through the exporter and through `FlameRenderer`, both at
/// 256 iterations a thread and the same total work. Both normalise by the
/// samples that plotted, so the whole view's density and light per sample
/// agree. Restarted, the exporter counts the burn-in too, and reads 2% dark
/// at 1,024 a thread (`dbg_export_against_app`).
///
/// Whole view only: block by block the exporter's picture of this view
/// differs from the app's -- dense blocks up to 59% -- with persistence
/// off as much as on, and the app's 2D and 3D shaders agree with each
/// other. A difference between the exporter and the app that predates this
/// plan (`persistent-orbits.md` §8, step 3), measured by
/// `dbg_export_against_app`.
#[test]
#[ignore = "GPU gate: persistent orbits; reads output/flame-zoom"]
fn an_export_counts_what_the_app_counts() {
    let Some(cfg) = band_flame() else {
        println!("  no julian-disc-blue1.fflame");
        return;
    };
    const N: u32 = 128;
    let (device, queue) = device();
    let app = render_blocks(&device, &queue, &cfg, N, &Run { ipt: 256, frames: 64, reps: 16, persistent: true, ..Default::default() });
    let (export, _) = export_blocks(&cfg, N, 256, 256 * 64 * 256 * 64, true, 16);
    let mut failures = Vec::new();
    let [d, red, _, blue] = compare_all(&export, &app);
    for (what, c) in [("density", &d), ("red light", &red), ("blue light", &blue)] {
        print_compared(&format!("{what}, exported against the app"), c);
        // Off by more than 1%, and by more than 3 standard errors.
        let (_, _, x, _, y, _, z) = c.pooled[0];
        if (x / y - 1.0).abs() > 0.01 && z.abs() > 3.0 {
            failures.push(format!("{what}: the export's whole view is {:.4} of the app's", x / y));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

/// Two gaskets in isolated xaos groups, side by side: transforms 0-2 on
/// the left, 3-5 on the right, no transition between the groups. An orbit
/// stays in the group of its first transform, drawn by weight, so the
/// right group holds `share` of the orbits and of the light.
fn isolated_groups(share: f32) -> FractalConfig {
    let mut cfg = FractalConfig::default();
    cfg.flame.transforms.clear();
    for group in 0..2 {
        let w = if group == 0 { 1.0 - share } else { share };
        for (i, (e, f)) in [(0.0f32, 0.0f32), (0.5, 0.0), (0.25, 0.5)].into_iter().enumerate() {
            let mut t = crate::scene::transforms::Transform::default();
            (t.a, t.b, t.c, t.d) = (0.5, 0.0, 0.0, 0.5);
            (t.e, t.f) = (e * 0.9 + if group == 0 { -1.0 } else { 0.1 }, f * 0.9 - 0.45);
            t.weight = w / 3.0;
            t.color = i as f32 / 2.0;
            t.variations.clear();
            t.variation_order.clear();
            t.set_variation("linear", 1.0);
            cfg.flame.transforms.push(t);
        }
    }
    let mut xaos = vec![vec![0.0f32; 6]; 6];
    for (i, row) in xaos.iter_mut().enumerate() {
        for (j, x) in row.iter_mut().enumerate() {
            if i / 3 == j / 3 {
                *x = 1.0;
            }
        }
    }
    cfg.flame.xaos = Some(xaos);
    cfg.zoom = 0.9;
    (cfg.pan_x, cfg.pan_y) = (0.0, 0.0);
    cfg.levels_enabled = false;
    cfg
}

/// Two gaskets reached through a hub: transform 0 (most of the weight)
/// leads to either gasket, and each gasket only repeats itself, so the
/// walk's second step decides which gasket an orbit stays in. The right
/// one gets `share` of them.
fn hub_groups(share: f32) -> FractalConfig {
    let mut cfg = isolated_groups(0.5);
    let mut hub = crate::scene::transforms::Transform::default();
    (hub.a, hub.b, hub.c, hub.d, hub.e, hub.f) = (0.5, 0.0, 0.0, 0.5, 0.0, 0.0);
    hub.weight = 1000.0;
    hub.variations.clear();
    hub.variation_order.clear();
    hub.set_variation("linear", 1.0);
    cfg.flame.transforms.insert(0, hub);
    for (k, t) in cfg.flame.transforms.iter_mut().enumerate().skip(1) {
        t.weight = if k <= 3 { (1.0 - share) / 3.0 } else { share / 3.0 };
    }
    let mut xaos = vec![vec![0.0f32; 7]; 7];
    for (i, row) in xaos.iter_mut().enumerate() {
        for (j, x) in row.iter_mut().enumerate() {
            let group = |k: usize| if k == 0 { 0 } else { 1 + (k - 1) / 3 };
            if (i == 0 && j > 0) || (i > 0 && group(i) == group(j)) {
                *x = 1.0;
            }
        }
    }
    cfg.flame.xaos = Some(xaos);
    cfg
}

/// **How a frozen split moves a small group's light** (§3.7, step 4's
/// measurement). Each replicate is a fresh renderer -- one generation, so
/// one draw of which orbits went to which group -- rendering 64 or 256 app
/// frames of 256 a thread, at 128 workgroups and at 16. The right group's
/// share of the density, mean and spread over replicates, at weight shares
/// of 1% and 10%: restarted orbits redraw the split every dispatch and
/// average it; persistent ones draw it once a generation, and a refresh
/// every `P` dispatches redraws a `1/P` share of it each dispatch.
#[test]
#[ignore = "measurement: needs a GPU"]
fn dbg_isolated_groups_share() {
    const N: u32 = 128;
    const REPS: usize = 24;
    let (device, queue) = device();
    let share_of = |cfg: &FractalConfig, persistent: bool, refresh: Option<u32>, frames: usize, groups: u32| -> (f64, f64) {
        let shares: Vec<f64> = (0..REPS)
            .map(|_| {
                let b = render_blocks(&device, &queue, cfg, N, &Run { ipt: 256, frames, reps: 1, persistent, workgroups: &[groups], refresh, ..Default::default() });
                let side = (N / 16) as usize;
                let (mut left, mut right) = (0.0, 0.0);
                for (i, d) in b.density[0].iter().enumerate() {
                    if i % side < side / 2 {
                        left += d;
                    } else {
                        right += d;
                    }
                }
                right / (left + right)
            })
            .collect();
        let mean = shares.iter().sum::<f64>() / REPS as f64;
        (mean, (shares.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (REPS - 1) as f64).sqrt())
    };
    for (what, cfg) in [("isolated", isolated_groups(0.01)), ("hub", hub_groups(0.01))] {
        println!(
            "  {what}, 1%: {} closed groups, the walk splits orbits: {}",
            cfg.flame.xaos_closed_classes(),
            cfg.flame.xaos_walk_splits_orbits()
        );
        let arms: Vec<(&str, bool, Option<u32>)> = if what == "isolated" {
            vec![("restarted", false, None), ("persistent, stratified", true, Some(0))]
        } else {
            vec![("restarted", false, None), ("persistent, no refresh", true, Some(0)), ("persistent, refresh 4", true, Some(4)), ("persistent, refresh 16", true, Some(16))]
        };
        for (name, persistent, refresh) in arms {
            for frames in [64usize, 256] {
                for groups in [128u32, 16] {
                    let (mean, sd) = share_of(&cfg, persistent, refresh, frames, groups);
                    println!("    {name:<24} {groups:>3} workgroups, {frames:>3} frames: share {mean:.5} +- {sd:.5} ({:.1}%)", sd / mean * 100.0);
                }
            }
        }
    }
}

/// **Short dispatches, at the band gate's precision**: the band flame,
/// persistent, at 64 against 256 iterations a thread, 16 replicates of
/// 65,536 iterations a thread each.
#[test]
#[ignore = "debug: needs a GPU; reads output/flame-zoom"]
fn dbg_short_dispatches_precisely() {
    let Some(cfg) = band_flame() else { return };
    const N: u32 = 128;
    let (device, queue) = device();
    let run = |ipt: u32| render_blocks(&device, &queue, &cfg, N, &Run { ipt, frames: (65536 / ipt) as usize, reps: 16, persistent: true, ..Default::default() });
    let (a, b, c) = (run(256), run(64), run(256));
    for (what, x, y) in [("64 against 256", &b, &a), ("256 against 256", &c, &a)] {
        let [d, red, _, blue] = compare_all(x, y);
        print_compared(&format!("density, {what}"), &d);
        print_compared(&format!("red light, {what}"), &red);
        print_compared(&format!("blue light, {what}"), &blue);
    }
}

/// **The exporter against the app, block by block** -- a difference that
/// predates persistent orbits (§8, step 3). The band flame through both at
/// the same total work: restarted at 1,024 a thread, then persistent at
/// 1,024 and 256. Dense blocks differ by up to 59% every way; the whole
/// view agrees once both count what plotted.
#[test]
#[ignore = "debug: needs a GPU; reads output/flame-zoom"]
fn dbg_export_against_app() {
    let Some(cfg) = band_flame() else { return };
    const N: u32 = 128;
    let (device, queue) = device();
    for (persistent, ipt) in [(false, 1024u32), (true, 1024), (true, 256)] {
        let frames = (128 * 256 / ipt) as usize;
        let app = render_blocks(&device, &queue, &cfg, N, &Run { ipt, frames, reps: 8, persistent, workgroups: &[128], ..Default::default() });
        let (export, _) = export_blocks(&cfg, N, ipt, 128 * 64 * ipt as u64 * frames as u64, persistent, 8);
        print_compared(&format!("density, exported against the app, {ipt} a thread, persistent {persistent}"), &compare_all(&export, &app)[0]);
    }
}

/// **Isolated groups keep their share** (§6; step 4): a group holding 1% of
/// the weight holds 1% of the light, within about what restarted orbits
/// manage, at the app's 128 workgroups after 64 frames. Isolated groups,
/// where the first pick decides an orbit's group, by the stratified first
/// pick alone; the hub flame, where the walk decides it, by the refresh.
/// Without either, persistent orbits froze the split at +-11% (measured,
/// `dbg_isolated_groups_share`).
#[test]
#[ignore = "GPU gate: persistent orbits"]
fn isolated_groups_keep_their_share() {
    const N: u32 = 128;
    const REPS: usize = 24;
    let (device, queue) = device();
    let mut failures = Vec::new();
    for (what, cfg) in [("isolated groups", isolated_groups(0.01)), ("the hub", hub_groups(0.01))] {
        let shares: Vec<f64> = (0..REPS)
            .map(|_| {
                let b = render_blocks(&device, &queue, &cfg, N, &Run { ipt: 256, frames: 64, reps: 1, persistent: true, workgroups: &[128], ..Default::default() });
                let side = (N / 16) as usize;
                let (mut left, mut right) = (0.0, 0.0);
                for (i, d) in b.density[0].iter().enumerate() {
                    if i % side < side / 2 {
                        left += d;
                    } else {
                        right += d;
                    }
                }
                right / (left + right)
            })
            .collect();
        let mean = shares.iter().sum::<f64>() / REPS as f64;
        let sd = (shares.iter().map(|x| (x - mean).powi(2)).sum::<f64>() / (REPS - 1) as f64).sqrt();
        println!("  {what}: the 1% group's share {mean:.5} +- {sd:.5} ({:.1}%)", sd / mean * 100.0);
        // Restarted orbits: +-1.1%. Measured persistent: 0.9% isolated,
        // 2.8% through the hub.
        if sd / mean > 0.05 {
            failures.push(format!("{what}: the share varies {:.1}% between renders", sd / mean * 100.0));
        }
        if (mean / 0.01 - 1.0).abs() > 4.0 * sd / mean / (REPS as f64).sqrt() + 0.01 {
            failures.push(format!("{what}: the share is {mean:.5}, not 0.01"));
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("
"));
}

/// **What persistent orbits cost** (§6, Performance; the decision between
/// always on and a setting turns on it). The app's frame -- 128 workgroups,
/// a compute pass and an accumulate pass -- timed with the feature off and
/// on, alternating over three rounds, on a cheap affine flame, the slowly
/// mixing band flame, a heavy random flame, a stateful one and a 3D one,
/// at 64, 256 and 1,024 iterations a thread. Reports the time a dispatch
/// takes and the plotted samples a second: on, every iteration past a
/// thread's first burn-in plots.
#[test]
#[ignore = "measurement: needs a GPU; reads output/flame-zoom and tests/visual"]
fn dbg_what_persistent_orbits_cost() {
    use std::time::Instant;
    let (device, queue) = device();
    let mut flames: Vec<(String, FractalConfig)> = vec![("gasket".into(), gasket())];
    if let Some(c) = band_flame() {
        flames.push(("julian-disc (band)".into(), c));
    }
    for (name, path) in [("random1", "output/flame-zoom/random1.fflame"), ("bubble-3d", "tests/visual/configs/3d/bubble-3d.fflame")] {
        if let Ok(text) = std::fs::read_to_string(path) {
            let mut c: FractalConfig = serde_json::from_str(&text).expect("a config");
            c.levels_enabled = false;
            flames.push((name.into(), c));
        }
    }
    let mut stateful = gasket();
    stateful.flame.transforms[0].set_variation("cubic_julia", 0.5);
    flames.push(("gasket + cubic_julia (stateful)".into(), stateful));

    println!("  flame                            ipt   ms/dispatch off -> on   Msamples/s off -> on");
    for (name, cfg) in &flames {
        for ipt in [64u32, 256, 1024] {
            let frames = (400_000_000u64 / (8192 * ipt as u64)).max(20) as usize;
            let mut acc = [[0.0f64; 2]; 2]; // [off, on] x [seconds, samples]
            for _round in 0..3 {
                for (k, persistent) in [false, true].into_iter().enumerate() {
                    let mut r = crate::renderer::FlameRenderer::with_palette_size(&device, &queue, wgpu::TextureFormat::Rgba8Unorm, 1280, 720, &cfg.flame, cfg.palette_size);
                    r.set_persistent_orbits(persistent);
                    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("load") });
                    r.load_config(&device, &mut enc, &queue, cfg, &cfg.palette, ipt, 20);
                    queue.submit(Some(enc.finish()));
                    let mut go = |r: &mut crate::renderer::FlameRenderer, n: usize| -> u64 {
                        let mut samples = 0u64;
                        for _ in 0..n {
                            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
                            let s = r.compute_pass(&mut enc, &queue, &device, 128, ipt, 20, cfg.zoom, cfg.pan_x as f32, cfg.pan_y as f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, cfg.speed_factor, true, false);
                            r.accumulate_pass(&mut enc, &queue, &device, s);
                            queue.submit(Some(enc.finish()));
                            samples += s;
                        }
                        let _ = device.poll(wgpu::PollType::wait_indefinitely());
                        samples
                    };
                    go(&mut r, 8);
                    let t0 = Instant::now();
                    let samples = go(&mut r, frames);
                    acc[k][0] += t0.elapsed().as_secs_f64() / frames as f64;
                    acc[k][1] += samples as f64 / t0.elapsed().as_secs_f64();
                }
            }
            let (ms_off, ms_on) = (acc[0][0] / 3.0 * 1e3, acc[1][0] / 3.0 * 1e3);
            let (rate_off, rate_on) = (acc[0][1] / 3.0 / 1e6, acc[1][1] / 3.0 / 1e6);
            println!(
                "  {name:<32} {ipt:>4}   {ms_off:>7.3} -> {ms_on:>7.3} ({:+.1}%)   {rate_off:>8.1} -> {rate_on:>8.1} ({:+.1}%)",
                (ms_on / ms_off - 1.0) * 100.0,
                (rate_on / rate_off - 1.0) * 100.0
            );
        }
    }
}
