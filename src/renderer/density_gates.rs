//! **Comparing renders by density, in replicates** -- the measure the
//! gates of Focused Rendering and persistent orbits hold pictures to.
//!
//! A tone map lifts faint light and darkens sparse light, so of two renders
//! of the same measure the sparser reads darker, and a sparse reference
//! reads wrong in exactly its faint regions. These read the accumulator
//! itself -- density, and colour, per equivalent iteration -- sum it in
//! 16x16 blocks, and give each block a mean and a standard error over
//! independent replicates, so two renders are compared where they are
//! precise and not where they are noise.
#![cfg(test)]

use crate::config::FractalConfig;
use crate::scene::cylinder::Cylinders;

/// **A render's density, in replicates.** `reps` independent renders of
/// `cfg` (drawing `plan` if given, untargeted otherwise), each `frames`
/// dispatches of 256 x `ipt`, read as density per equivalent iteration
/// (`read_density_blocking`) and summed in 16x16 blocks. From the
/// replicates each block has a mean and a standard error, so two renders
/// can be compared where they are precise and not where they are noise.
/// Also the last replicate, tone-mapped, for looking at.
#[allow(clippy::too_many_arguments)]
pub(crate) fn replicated_blocks(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    cfg: &FractalConfig,
    plan: Option<&Cylinders>,
    n: u32,
    ipt: u32,
    frames: usize,
    reps: usize,
) -> (Vec<Vec<f64>>, Vec<u8>) {
    let b = render_blocks(device, queue, cfg, n, &Run { plan, ipt, frames, reps, ..Default::default() });
    (b.density, b.image)
}

/// Mean and standard error of the mean, over replicates.
pub(crate) fn mean_se(xs: &[f64]) -> (f64, f64) {
    let k = xs.len() as f64;
    let m = xs.iter().sum::<f64>() / k;
    let var = xs.iter().map(|x| (x - m) * (x - m)).sum::<f64>() / (k - 1.0).max(1.0);
    (m, (var / k).sqrt())
}

/// **Two renders' densities, compared where they are precise.** Blocks
/// in three tiers by `tiers_of`'s density against its mean block --
/// dense (at least a tenth of it), middle, faint (under a hundredth,
/// not empty) -- each tier pooled over its blocks replicate by
/// replicate; and every block's own difference in standard errors.
pub(crate) struct Compared {
    /// (x mean, x se, y mean, y se, z) for the whole view and each tier.
    pub pooled: Vec<(&'static str, usize, f64, f64, f64, f64, f64)>,
    /// The largest block |z|, and how many blocks pass 5.
    pub worst_z: f64,
    pub over_5: usize,
    pub judged: usize,
    /// The largest relative difference of a block that differs by more
    /// than 5 standard errors, in the dense tier and the middle one.
    pub worst_dense: f64,
    pub worst_middle: f64,
}

pub(crate) fn compare_blocks(x: &[Vec<f64>], y: &[Vec<f64>], tiers_of: &[Vec<f64>]) -> Compared {
    let nb = x[0].len();
    let per_block = |reps: &[Vec<f64>], b: usize| mean_se(&reps.iter().map(|r| r[b]).collect::<Vec<_>>());
    let reference: Vec<f64> = (0..nb).map(|b| per_block(tiers_of, b).0).collect();
    let mean_block = reference.iter().sum::<f64>() / nb as f64;
    let tier = |b: usize| -> &'static str {
        let v = reference[b];
        if v >= 0.1 * mean_block {
            "dense"
        } else if v >= 0.01 * mean_block {
            "middle"
        } else if v > 0.0 {
            "faint"
        } else {
            "empty"
        }
    };
    let mut pooled = Vec::new();
    for name in ["all", "dense", "middle", "faint", "empty"] {
        let in_tier: Vec<usize> = (0..nb).filter(|&b| name == "all" || tier(b) == name).collect();
        let sums = |reps: &[Vec<f64>]| mean_se(&reps.iter().map(|r| in_tier.iter().map(|&b| r[b]).sum::<f64>()).collect::<Vec<_>>());
        let ((mx, sx), (my, sy)) = (sums(x), sums(y));
        let se = (sx * sx + sy * sy).sqrt();
        let z = if se > 0.0 { (mx - my) / se } else if mx == my { 0.0 } else { f64::INFINITY };
        pooled.push((name, in_tier.len(), mx, sx, my, sy, z));
    }
    let (mut worst_z, mut over_5, mut judged) = (0.0f64, 0usize, 0usize);
    let (mut worst_dense, mut worst_middle) = (0.0f64, 0.0f64);
    for b in 0..nb {
        let ((mx, sx), (my, sy)) = (per_block(x, b), per_block(y, b));
        let se = (sx * sx + sy * sy).sqrt();
        if se > 0.0 {
            judged += 1;
            let z = ((mx - my) / se).abs();
            worst_z = worst_z.max(z);
            over_5 += (z > 5.0) as usize;
            if z > 5.0 {
                let rel = (mx - my).abs() / mx.max(my);
                match tier(b) {
                    "dense" => worst_dense = worst_dense.max(rel),
                    "middle" => worst_middle = worst_middle.max(rel),
                    _ => {}
                }
            }
        }
    }
    Compared { pooled, worst_z, over_5, judged, worst_dense, worst_middle }
}

pub(crate) fn print_compared(what: &str, c: &Compared) {
    println!(
        "  {what}: blocks judged {}, worst |z| {:.1}, over 5: {}; of those, the largest difference {:.3} dense, {:.3} middle",
        c.judged, c.worst_z, c.over_5, c.worst_dense, c.worst_middle
    );
    for (name, n, mx, sx, my, sy, z) in &c.pooled {
        if *n > 0 {
            println!("    {name:<6} {n:>3} blocks: {mx:.4e} +- {sx:.1e}  against  {my:.4e} +- {sy:.1e}   ratio {:.4}  z {z:+.1}", mx / my.max(f64::MIN_POSITIVE));
        }
    }
}

/// **How a render is made**, for `render_blocks`.
pub(crate) struct Run<'a> {
    /// A plan to draw, or none: the plain chaos game.
    pub plan: Option<&'a Cylinders>,
    pub ipt: u32,
    /// Dispatches a replicate.
    pub frames: usize,
    pub reps: usize,
    /// Keep each thread's orbit between dispatches
    /// (docs/projects/persistent-orbits.md).
    pub persistent: bool,
    /// Workgroups of each dispatch, cycled through; empty is 256 each.
    pub workgroups: &'a [u32],
    /// A refresh period in place of the one the flame needs
    /// (persistent-orbits.md §3.7); none keeps the flame's.
    pub refresh: Option<u32>,
}

impl Default for Run<'_> {
    fn default() -> Self {
        Self { plan: None, ipt: 1024, frames: 4, reps: 8, persistent: false, workgroups: &[], refresh: None }
    }
}

/// A render in replicates: each replicate's 16x16 block sums of density
/// and of each colour channel's light (colour times density), all per
/// equivalent iteration, and the last replicate tone-mapped.
pub(crate) struct Blocks {
    pub density: Vec<Vec<f64>>,
    pub light: [Vec<Vec<f64>>; 3],
    pub image: Vec<u8>,
}

/// Render `cfg` at `n` x `n` as `run` says, in replicates.
pub(crate) fn render_blocks(device: &wgpu::Device, queue: &wgpu::Queue, cfg: &FractalConfig, n: u32, run: &Run) -> Blocks {
    let mut cfg = cfg.clone();
    cfg.deterministic_rng = false;
    cfg.cylinder_targeting = run.plan.is_some();
    let mut r = crate::renderer::FlameRenderer::with_palette_size(device, queue, wgpu::TextureFormat::Rgba8Unorm, n, n, &cfg.flame, cfg.palette_size);
    r.set_persistent_orbits(run.persistent);
    if let Some(p) = run.plan {
        let _ = r.show_plan(device, queue, &cfg, p.clone());
    }
    let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("load") });
    r.load_config(device, &mut enc, queue, &cfg, &cfg.palette, 1, 0);
    queue.submit(Some(enc.finish()));
    if let Some(period) = run.refresh {
        r.set_orbit_refresh_period(period);
    }
    let side = (n / 16) as usize;
    let mut out = Blocks { density: Vec::new(), light: [Vec::new(), Vec::new(), Vec::new()], image: Vec::new() };
    for rep in 0..run.reps {
        // Independent replicates: a replicate that resumed the last one's
        // orbits would be a continuation of it, and their spread would
        // understate the noise.
        r.restart_orbits();
        let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("reset") });
        r.reset(&mut enc, queue, run.ipt, cfg.zoom, cfg.pan_x as f32, cfg.pan_y as f32, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, cfg.speed_factor);
        queue.submit(Some(enc.finish()));
        for f in 0..run.frames.max(1) {
            let groups = if run.workgroups.is_empty() { 256 } else { run.workgroups[f % run.workgroups.len()] };
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
            let k = r.compute_pass(
                &mut enc, queue, device, groups, run.ipt, 20, cfg.zoom, cfg.pan_x as f32, cfg.pan_y as f32, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 0.0, cfg.speed_factor, true, false,
            );
            r.accumulate_pass(&mut enc, queue, device, k);
            queue.submit(Some(enc.finish()));
            let _ = device.poll(wgpu::PollType::wait_indefinitely());
        }
        let acc = r.read_accumulator_blocking(device, queue);
        let mut d = vec![0.0f64; side * side];
        let mut l = [vec![0.0f64; side * side], vec![0.0f64; side * side], vec![0.0f64; side * side]];
        for (i, px) in acc.iter().enumerate() {
            let (x, y) = (i % n as usize / 16, i / n as usize / 16);
            let b = y * side + x;
            d[b] += px[3];
            for c in 0..3 {
                l[c][b] += px[c] * px[3];
            }
        }
        out.density.push(d);
        for c in 0..3 {
            out.light[c].push(std::mem::take(&mut l[c]));
        }
        if rep + 1 == run.reps {
            let mut enc = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("tonemap") });
            r.tonemap_pass(queue, &mut enc);
            queue.submit(Some(enc.finish()));
            out.image = pollster::block_on(r.read_fractal_pixels(device, queue, false, [0.0, 0.0, 0.0])).expect("pixels").2;
        }
    }
    out
}
