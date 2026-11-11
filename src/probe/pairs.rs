//! A built-in variation beside its Rhai twin, input for input
//! (docs/archive/projects/rhai-plugins.md, P7 step 3, V1).
//!
//! The probe's own harness, two transforms a pair: the built-in in one,
//! its twin in the next, the same parameter values in both, evaluated at
//! the same points through the real `apply_variations`. Paired
//! (`ShaderConstants::probe_paired`), the random numbers are seeded from
//! the point alone, so a `NeedsRng` built-in and its twin draw the same
//! sequence, and the colour a `WritesColor` variation drives comes back
//! beside the point -- each output `[x, y, z, colour]`.
//!
//! What a twin is held to, the caller says: it gets the raw outputs.

use super::shader::{self, ENTRY_POINT};
use crate::config::escape::CustomSource;
use crate::config::FractalConfig;
use crate::renderer::compute_kernel::FlameRenderer;
use crate::scene::transforms::{Flame, RenderMode, Transform};

/// A built-in and its twin: the twin's registry key, and the parameter
/// values the two take, by the built-in's parameter names.
#[derive(Debug, Clone)]
pub struct Pair {
    pub builtin: String,
    pub twin: String,
    pub params: Vec<(String, f32)>,
}

/// A pair's outputs at each point, `[x, y, z, colour]` (z 0 in 2D, the
/// colour 0 for a variation that drives none).
#[derive(Debug, Clone)]
pub struct Outputs {
    pub builtin: Vec<[f32; 4]>,
    pub twin: Vec<[f32; 4]>,
}

/// The transform each half of a pair sits in: the probe's affine, and
/// `linear` beside a pre or post variation so the transform shows it.
fn transform(name: &str, params: &[(String, f32)], carrier: bool) -> Transform {
    let mut xf = super::flame::probe_transform();
    if carrier {
        xf.set_variation(super::flame::CARRIER, 1.0);
    }
    xf.set_variation(name, 1.0);
    for (k, v) in params {
        xf.set_variation_param(name, k, *v);
    }
    xf
}

/// Evaluate every pair at every point, in 2D or 3D: the twins' sources
/// carried in the flame (`carried`) and registered as a flame's are.
pub fn run_pairs(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    pairs: &[Pair],
    carried: &[CustomSource],
    points: &[[f32; 3]],
    render_3d: bool,
) -> Result<Vec<Outputs>, String> {
    let mut flame = Flame::new();
    flame.name = "variation pairs".to_string();
    flame.transforms.clear();
    flame.custom_variations = carried.to_vec();
    crate::variations::custom::ensure(&flame);
    for pair in pairs {
        let carrier = crate::variations::global_registry()
            .get(&pair.builtin)
            .is_some_and(|v| matches!(v.phase, crate::variations::VariationPhase::Pre | crate::variations::VariationPhase::Post));
        flame.transforms.push(transform(&pair.builtin, &pair.params, carrier));
        flame.transforms.push(transform(&pair.twin, &pair.params, carrier));
    }

    let mut config = FractalConfig::default();
    config.flame = flame.clone();
    config.render_mode = if render_3d { RenderMode::ThreeD } else { RenderMode::TwoD };
    let xform_count = flame.transforms.len();
    let words = shader::buffer_words(points.len(), xform_count);
    // The histogram carries the I/O: a side whose square holds it.
    let dim = ((words as f64 / 4.0).sqrt().ceil() as u32).max(128);
    let mut renderer = FlameRenderer::with_palette_size(device, queue, wgpu::TextureFormat::Rgba8Unorm, dim, dim, &config.flame, config.palette_size);
    // As the probe: the flame as it is, not the sticky superset.
    renderer.set_sticky_enabled(false);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("pairs config") });
    renderer.load_config(device, &mut encoder, queue, &config, &config.palette, 1, 0);
    queue.submit(Some(encoder.finish()));
    renderer.run_init_pass(device, queue);

    let source = shader::build_flame(&flame, render_3d, true);
    let mut input = vec![0u32; shader::output_base(points.len())];
    input[0] = points.len() as u32;
    input[1] = xform_count as u32;
    for (i, p) in points.iter().enumerate() {
        let base = shader::HEADER_WORDS + i * shader::WORDS_PER_SLOT;
        input[base] = p[0].to_bits();
        input[base + 1] = p[1].to_bits();
        input[base + 2] = p[2].to_bits();
    }
    let out = renderer.dispatch_readback(device, queue, &source, ENTRY_POINT, &input, words, (points.len() * xform_count) as u32, &mut |_, _| {})?;
    let base = shader::output_base(points.len());
    let read = |xform: usize| -> Vec<[f32; 4]> {
        (0..points.len())
            .map(|i| {
                let at = base + (xform * points.len() + i) * shader::WORDS_PER_SLOT;
                [0, 1, 2, 3].map(|c| f32::from_bits(out[at + c]))
            })
            .collect()
    };
    Ok((0..pairs.len()).map(|k| Outputs { builtin: read(2 * k), twin: read(2 * k + 1) }).collect())
}
