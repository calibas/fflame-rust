//! Every variation evaluated as a **screen-space camera lens**.
//!
//! # What a lens is
//!
//! Every escape kernel turns a pixel into a sample point the same way:
//! a normalised screen offset, scaled by the view span, rotated, plus
//! the centre. A lens is a map applied to that offset before the scale
//! — so the fractal is untouched and only *which point each pixel
//! samples* changes.
//!
//! That makes a lens the INVERSE direction of a flame's final
//! transform, which maps attractor points to the screen. Two
//! consequences, both useful: a variation need not be invertible to be
//! a lens (many-to-one just means a region appears more than once, and
//! every pixel still gets exactly one deterministic sample), and the
//! picture a lens gives is not the picture the same variation gives as
//! a final.
//!
//! # Why this exists
//!
//! There are 600-odd variations and no way to guess which ones read as
//! camera effects. This evaluates all of them over a screen grid and
//! dumps the maps, so the survey is a resample of one rendered image
//! rather than 600 renders. That resample is not an approximation of
//! the real thing: with `L` the lens and `A` the affine a kernel
//! already applies, a lensed render is `fractal(A(L(p)))`, which is
//! exactly the un-lensed image sampled at `L(p)`. Only two things
//! differ — resolution where the lens magnifies, and points whose
//! image leaves the rendered frame.
//!
//! # How it differs from the numeric probe
//!
//! Same harness, two deliberate changes. The grid is a screen rather
//! than [`super::inputs`]'s adversarial points, and the flame's affine
//! is the IDENTITY rather than [`super::flame`]'s deliberately skewed
//! one — a lens is the variation applied to the screen offset, and an
//! extra affine would be measuring something else.

use super::batch::{builtin_targets, plan_batches, Batch};
use super::flame::CARRIER;
use super::shader::{self, ENTRY_POINT};
use crate::config::FractalConfig;
use crate::renderer::compute_kernel::FlameRenderer;
use crate::scene::transforms::{Flame, RenderMode, Transform};
use std::io::Write;
use std::path::Path;

/// File magic. Bump the digits if the layout changes.
const MAGIC: &[u8; 8] = b"FFLENS01";

/// A lens map: where each grid point lands.
pub struct LensMap {
    pub name: String,
    /// `grid * grid` pairs, row-major from the top-left.
    pub points: Vec<[f32; 2]>,
}

/// The screen grid, in the convention the lens would see: half-height
/// one, y up, row-major from the TOP-left so the dump indexes like an
/// image.
///
/// Half-height one rather than the raw `[0,1]` uv because that is the
/// scale variations are written for — a unit disc is where `fisheye`,
/// `spherical` and the rest do something recognisable, and a lens fed
/// `[0,1]` offsets would be surveying the wrong part of every formula.
pub fn screen_grid(grid: u32) -> Vec<[f32; 2]> {
    let n = grid as usize;
    let mut out = Vec::with_capacity(n * n);
    for j in 0..n {
        for i in 0..n {
            let u = (i as f32 + 0.5) / grid as f32 * 2.0 - 1.0;
            let v = (j as f32 + 0.5) / grid as f32 * 2.0 - 1.0;
            out.push([u, -v]);
        }
    }
    out
}

/// One transform per target, identity affine, weight one — so the
/// output of transform *i* is exactly `targets[i]` applied to the
/// point.
///
/// The carrier rides along for phase variations exactly as the numeric
/// probe does it: a Pre target runs before `linear` passes the point
/// through, a Post target runs after, and either way the result is the
/// target's own map.
fn lens_flame(batch: &Batch) -> Flame {
    let mut flame = Flame::new();
    flame.name = "lens survey".to_string();
    flame.transforms.clear();
    for target in &batch.targets {
        let mut xf = Transform::new();
        // Identity: a lens is the variation on the screen offset.
        //
        // In this engine's convention that is a = d = 1 with e and f
        // the translation (`apply_affine`, shaders/core/affine.wgsl),
        // NOT a = e = 1. The probe block evaluates the variation
        // without the affine, so the wrong value here changed no
        // measurement -- `linear` still read as the identity to 6e-8 --
        // but it is written correctly rather than harmlessly.
        xf.a = 1.0;
        xf.b = 0.0;
        xf.e = 0.0;
        xf.c = 0.0;
        xf.d = 1.0;
        xf.f = 0.0;
        xf.g = 0.0;
        xf.weight = 1.0;
        if target.needs_carrier() {
            xf.set_variation(CARRIER, 1.0);
        }
        xf.set_variation(&target.name, 1.0);
        flame.transforms.push(xf);
    }
    flame
}

/// Split the numeric probe's batches down to what one dispatch can
/// carry.
///
/// The probe's own packing is built for its 25-point grid; a screen
/// grid is thousands of points, so the I/O buffer rather than the slot
/// budget is what binds. Splitting a batch is safe because
/// [`lens_flame`] builds each transform independently — a smaller
/// batch is just a smaller flame.
fn split_to_fit(batches: Vec<Batch>, points: usize, max_words: usize) -> Vec<Batch> {
    // buffer_words = output_base(points) + points * n * WORDS_PER_SLOT
    let per_target = points * shader::WORDS_PER_SLOT;
    let room = max_words.saturating_sub(shader::output_base(points));
    let by_buffer = (room / per_target.max(1)).max(1);
    // The dispatch is one thread per (point, variation) at a workgroup
    // size of 64, and a dispatch may not exceed 65535 groups in a
    // dimension. At a 160 grid the buffer binds first and this never
    // fires; at 256 it is the only thing that does, so bound by both
    // rather than discovering it as a validation error.
    let by_dispatch = ((65535usize * 64) / points.max(1)).max(1);
    let cap = by_buffer.min(by_dispatch);
    let mut out = Vec::new();
    for batch in batches {
        for chunk in batch.targets.chunks(cap) {
            out.push(Batch {
                targets: chunk.to_vec(),
                slots: chunk.iter().map(|t| t.slots).sum(),
            });
        }
    }
    out
}

/// Blocking wrapper, for the gates that are not async.
pub(crate) fn open_device_for_bounds_blocking() -> (wgpu::Device, wgpu::Queue) {
    pollster::block_on(open_device_for_bounds()).expect("gpu")
}

/// A device for the forward-bound gate, which needs the probe's
/// limits (the flame compute bind group exceeds WebGPU's floor of
/// eight storage buffers) without running a survey.
pub(crate) async fn open_device_for_bounds(
) -> Result<(wgpu::Device, wgpu::Queue), String> {
    super::run::open_device("forward bounds").await
}

/// Evaluate every shipped variation over the screen grid.
pub async fn run(grid: u32, mut on_batch: impl FnMut(usize, usize)) -> Result<Vec<LensMap>, String> {
    let (device, queue) = super::run::open_device("lens survey").await?;

    let points = screen_grid(grid);

    // The histogram buffer is the I/O channel, at four `u32` per pixel
    // ([r, g, b, density]). Size the renderer from what the dispatch
    // needs rather than picking a dimension and hoping.
    const MAX_PER_BATCH: usize = 64;
    let want = shader::buffer_words(points.len(), MAX_PER_BATCH);
    let dim = (((want as f64 / 4.0).sqrt().ceil() as u32) / 64 + 1) * 64;
    let max_words = (dim as usize) * (dim as usize) * 4;

    let batches = split_to_fit(plan_batches(&builtin_targets()), points.len(), max_words);
    let total = batches.len();

    let mut maps = Vec::new();
    for (i, batch) in batches.iter().enumerate() {
        on_batch(i + 1, total);
        maps.extend(run_batch(&device, &queue, batch, &lens_flame(batch), &points, dim)?);
    }
    Ok(maps)
}

/// Evaluate `batch`'s variations at `points`, through the flame the
/// caller supplies.
///
/// The flame is an argument rather than [`lens_flame`]'s own because
/// the survey is not the only caller: the forward-bound gate
/// (`variations::bound`) needs the same evaluation at specific
/// variation PARAMETERS, and building its own flame is the only way
/// to set them. Everything else — the probe shader, the init pass,
/// the readback — is identical, and sharing it is what makes that
/// gate a test of the shipped WGSL rather than of a second opinion
/// about it.
pub(crate) fn run_batch(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    batch: &Batch,
    flame: &Flame,
    points: &[[f32; 2]],
    dim: u32,
) -> Result<Vec<LensMap>, String> {
    let mut config = FractalConfig::default();
    config.flame = flame.clone();
    config.render_mode = RenderMode::TwoD;

    let mut renderer = FlameRenderer::with_palette_size(
        device,
        queue,
        wgpu::TextureFormat::Rgba8Unorm,
        dim,
        dim,
        &config.flame,
        config.palette_size,
    );
    // As the numeric probe: measure the flame as it is, and fill the
    // init-derived slots the render pass would normally fill.
    renderer.set_sticky_enabled(false);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("lens config"),
    });
    renderer.load_config(device, &mut encoder, queue, &config, &config.palette, 1, 0);
    queue.submit(Some(encoder.finish()));
    renderer.run_init_pass(device, queue);

    let source = shader::build(batch, false);
    let xform_count = batch.targets.len();

    let mut input = vec![0u32; shader::output_base(points.len())];
    input[0] = points.len() as u32;
    input[1] = xform_count as u32;
    for (i, p) in points.iter().enumerate() {
        let base = shader::HEADER_WORDS + i * shader::WORDS_PER_SLOT;
        input[base] = p[0].to_bits();
        input[base + 1] = p[1].to_bits();
        input[base + 2] = 0f32.to_bits();
    }

    let total_slots = points.len() * xform_count;
    let words = renderer.dispatch_readback(
        device,
        queue,
        &source,
        ENTRY_POINT,
        &input,
        shader::buffer_words(points.len(), xform_count),
        total_slots as u32,
        &mut |_, _| {},
    )?;

    let out_base = shader::output_base(points.len());
    let mut maps = Vec::with_capacity(xform_count);
    for (xform_idx, target) in batch.targets.iter().enumerate() {
        let mut out = Vec::with_capacity(points.len());
        for point_idx in 0..points.len() {
            let slot = xform_idx * points.len() + point_idx;
            let base = out_base + slot * shader::WORDS_PER_SLOT;
            out.push([f32::from_bits(words[base]), f32::from_bits(words[base + 1])]);
        }
        maps.push(LensMap {
            name: target.name.clone(),
            points: out,
        });
    }
    Ok(maps)
}

/// Dump the maps for the resampler: magic, grid, count, then per
/// variation a length-prefixed name and `grid²` xy pairs.
pub fn write_dump(path: &Path, grid: u32, maps: &[LensMap]) -> std::io::Result<()> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let file = std::fs::File::create(path)?;
    let mut w = std::io::BufWriter::new(file);
    w.write_all(MAGIC)?;
    w.write_all(&grid.to_le_bytes())?;
    w.write_all(&(maps.len() as u32).to_le_bytes())?;
    for m in maps {
        w.write_all(&(m.name.len() as u32).to_le_bytes())?;
        w.write_all(m.name.as_bytes())?;
        for p in &m.points {
            w.write_all(&p[0].to_le_bytes())?;
            w.write_all(&p[1].to_le_bytes())?;
        }
    }
    w.flush()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_grid_is_a_screen_with_y_up() {
        let g = screen_grid(4);
        assert_eq!(g.len(), 16);
        // Row-major from the top-left, so the FIRST row is the top of
        // the image and its y is positive.
        assert!(g[0][0] < 0.0 && g[0][1] > 0.0, "top-left: {:?}", g[0]);
        assert!(g[15][0] > 0.0 && g[15][1] < 0.0, "bottom-right: {:?}", g[15]);
        // Half-height one: the extreme samples sit inside the unit
        // square by half a pixel, not at some uv-scaled fraction.
        assert!((g[15][0] - 0.75).abs() < 1e-6, "{:?}", g[15]);
    }

    /// The affine must be the identity, or the survey measures the
    /// variation composed with a skew — which is what the numeric
    /// probe deliberately does and a lens must not.
    #[test]
    fn the_lens_flame_applies_no_affine_of_its_own() {
        let batch = Batch {
            targets: vec![super::super::batch::Target {
                name: "linear".to_string(),
                slots: 0,
                needs_init: false,
                phase: crate::variations::VariationPhase::Normal,
            }],
            slots: 0,
        };
        let flame = lens_flame(&batch);
        let t = &flame.transforms[0];
        assert_eq!([t.a, t.b, t.c, t.d, t.e, t.f], [1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    }

    /// The 256 grid overflowed `dispatch_workgroups` while fitting the
    /// buffer comfortably. Pin the dimension that actually binds.
    #[test]
    fn splitting_respects_the_dispatch_limit() {
        let mk = |n: usize| Batch {
            targets: (0..n)
                .map(|i| super::super::batch::Target {
                    name: format!("v{i}"),
                    slots: 0,
                    needs_init: false,
                    phase: crate::variations::VariationPhase::Normal,
                })
                .collect(),
            slots: 0,
        };
        let points = 256 * 256;
        // A buffer with room to spare, so only the dispatch cap can bind.
        let out = split_to_fit(vec![mk(64)], points, usize::MAX / 2);
        for b in &out {
            let threads = points * b.targets.len();
            assert!(
                threads.div_ceil(64) <= 65535,
                "{} variations x {points} points is {} workgroups",
                b.targets.len(),
                threads.div_ceil(64)
            );
        }
        assert_eq!(out.iter().map(|b| b.targets.len()).sum::<usize>(), 64);
    }

    #[test]
    fn splitting_respects_the_buffer() {
        let mk = |n: usize| Batch {
            targets: (0..n)
                .map(|i| super::super::batch::Target {
                    name: format!("v{i}"),
                    slots: 0,
                    needs_init: false,
                    phase: crate::variations::VariationPhase::Normal,
                })
                .collect(),
            slots: 0,
        };
        let points = 100;
        let max_words = shader::buffer_words(points, 3);
        let out = split_to_fit(vec![mk(10)], points, max_words);
        assert!(out.iter().all(|b| b.targets.len() <= 3), "a batch overflows");
        assert_eq!(out.iter().map(|b| b.targets.len()).sum::<usize>(), 10);
    }
}
