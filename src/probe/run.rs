//! Driving the probe on a real device.
//!
//! One flame per batch, one dispatch per dimension, results decoded
//! straight out of the histogram buffer. The heavy lifting is elsewhere:
//! the shader comes from the ordinary builder, the bind group from a
//! live [`FlameRenderer`], and the classification from
//! [`super::classify`].

use super::batch::{builtin_targets, plan_batches, Batch};
use super::classify::{class_mask, summarise, Sample};
use super::inputs::probe_inputs;
use super::report::{self, Entry, Meta, Report, Timings, SCHEMA};
use super::shader::{self, ENTRY_POINT};
use crate::config::FractalConfig;
use crate::renderer::compute_kernel::FlameRenderer;
use crate::scene::transforms::RenderMode;

/// Render target size for the probe's renderer.
///
/// Nothing is rendered — the size only has to make the histogram buffer
/// big enough for the probe's I/O, which is about 43 KB at the current
/// grid. 128² gives 256 KB, several times the need, and costs nothing
/// worth measuring.
const PROBE_DIM: u32 = 128;

pub struct Outcome {
    pub report: Report,
    /// One entry per (variation, parameter, dimension), named
    /// `variation.parameter`. A separate report because it is four times
    /// the size of the default-parameter one and answers a different
    /// question — reusing `Entry` means `compare` works on it unchanged.
    pub sweep: Report,
    pub timings: Timings,
}

/// Run the whole probe.
///
/// `on_batch` is called before each batch starts, so a caller can flush
/// progress to disk. That matters more than it looks: a GPU hang is a
/// device loss that takes the process with it, and cannot be caught
/// in-process. If the run dies, the last thing reported is the batch
/// that was executing — which is the difference between "something
/// hung" and "batch 4, 3D hung".
pub async fn run(run_sweep: bool, mut on_batch: impl FnMut(&str)) -> Result<Outcome, String> {
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::all(),
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        })
        .await
        .map_err(|e| format!("no GPU adapter: {e:?}"))?;

    let info = adapter.get_info();
    let adapter_limits = adapter.limits();
    let mut limits = wgpu::Limits::default();
    limits.max_storage_buffer_binding_size = adapter_limits.max_storage_buffer_binding_size;
    limits.max_buffer_size = adapter_limits.max_buffer_size;
    limits.max_storage_buffers_per_shader_stage = adapter_limits.max_storage_buffers_per_shader_stage;

    let mut features = wgpu::Features::CLEAR_TEXTURE;
    if adapter.features().contains(wgpu::Features::FLOAT32_FILTERABLE) {
        features |= wgpu::Features::FLOAT32_FILTERABLE;
    }

    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("variation probe"),
            required_features: features,
            required_limits: limits,
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        })
        .await
        .map_err(|e| format!("device creation failed: {e:?}"))?;

    let version = crate::version::get_version_info();
    let points = probe_inputs();
    let batches = plan_batches(&builtin_targets());

    let mut report = Report {
        schema: SCHEMA,
        kind: report::Kind::Defaults,
        meta: Meta {
            app_version: version.short_version().to_string(),
            git_hash: version.build_id(),
            adapter: info.name.clone(),
            backend: format!("{:?}", info.backend),
            driver: if info.driver_info.is_empty() {
                info.driver.clone()
            } else {
                format!("{} {}", info.driver, info.driver_info)
            },
            os: format!("{} {}", version.platform(), version.architecture()),
            input_labels: points.iter().map(|p| p.label.to_string()).collect(),
            notes: vec![
                "Each line is one variation at its DEFAULT parameters. `classes` is".into(),
                "one glyph per output component, in input order, so a difference".into(),
                "localises to the input that moved. The parameter sweep lives in the".into(),
                "-sweep report beside this one.".into(),
            ],
        },
        entries: Vec::new(),
        skipped: Vec::new(),
    };
    let mut sweep = Report {
        schema: SCHEMA,
        kind: report::Kind::Sweep,
        meta: Meta {
            // The sweep records one glyph per (value, component), not
            // per input, so the input labels would mis-attribute a
            // divergence. Left empty; `compare` then falls back to
            // naming the glyph position, which here IS the step.
            input_labels: Vec::new(),
            notes: vec![
                "THIS IS THE PARAMETER SWEEP. Each line is one variation's one".into(),
                "parameter, swept one value at a time with every other parameter at".into(),
                "its default. `classes` is a fixed-width presence mask per swept".into(),
                "value: the class's glyph where that class occurred across the 27".into(),
                "inputs and all components, `.` where it did not. So a mask says".into(),
                "WHICH kinds a value produced, not at which input or in which".into(),
                "component — rerun the probe on the variation alone for that.".into(),
                "A change confined to one input, or a sign flip, shows only in the".into(),
                "digest, which covers every raw sample.".into(),
            ],
            ..report.meta.clone()
        },
        entries: Vec::new(),
        skipped: Vec::new(),
    };
    let mut timings = Timings::default();

    for (i, batch) in batches.iter().enumerate() {
        for render_3d in [false, true] {
            let dim = if render_3d { "3d" } else { "2d" };
            let what = format!("batch{i}-{dim}");
            on_batch(&what);

            let started = web_time::Instant::now();
            let mut phases: Vec<(String, f64)> = Vec::new();
            let result = run_batch(&device, &queue, batch, render_3d, &mut |name, ms| {
                phases.push((name.to_string(), ms));
            });
            match result {
                Ok(entries) => report.entries.extend(entries),
                Err(e) => {
                    // One batch failing must not lose the other six. The
                    // whole batch is recorded as skipped, so the report
                    // says what it does not cover rather than quietly
                    // omitting 99 variations.
                    log::error!("probe {what} failed: {e}");
                    for target in &batch.targets {
                        report.skipped.push((
                            format!("{} ({dim})", target.name),
                            format!("batch {i} failed: {e}"),
                        ));
                    }
                }
            }
            for (name, ms) in phases {
                timings.push(format!("{what}.{name}"), ms);
            }
            timings.push(format!("{what}.total"), started.elapsed().as_secs_f64() * 1000.0);

            if run_sweep {
                on_batch(&format!("{what} sweep"));
                let started = web_time::Instant::now();
                let mut rounds = 0usize;
                match sweep_batch(&device, &queue, batch, render_3d, &mut rounds) {
                    Ok(entries) => sweep.entries.extend(entries),
                    Err(e) => {
                        log::error!("probe {what} sweep failed: {e}");
                        sweep
                            .skipped
                            .push((format!("batch {i} ({dim})"), format!("sweep failed: {e}")));
                    }
                }
                // Per round, not total. Batches do wildly different
                // amounts of sweep work — one variation carries 157
                // parameters — so comparing totals would flag the batch
                // that legitimately does the most and say nothing about
                // whether any single dispatch is slow.
                let total_ms = started.elapsed().as_secs_f64() * 1000.0;
                timings.push(
                    format!("{what}[{rounds}r].sweep"),
                    total_ms / rounds.max(1) as f64,
                );
            }
        }
    }

    // Sort so the report's order is the registry's, not the order two
    // dimensions happened to interleave in.
    report.entries.sort_by(|a, b| a.name.cmp(&b.name).then(a.dim.cmp(b.dim)));
    sweep.entries.sort_by(|a, b| a.name.cmp(&b.name).then(a.dim.cmp(b.dim)));

    Ok(Outcome {
        report,
        sweep,
        timings,
    })
}

fn run_batch(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    batch: &Batch,
    render_3d: bool,
    phase: &mut dyn FnMut(&str, f64),
) -> Result<Vec<Entry>, String> {
    let points = probe_inputs();
    let flame = super::flame::build_probe_flame(batch);

    let mut config = FractalConfig::default();
    config.flame = flame.clone();
    config.render_mode = if render_3d {
        RenderMode::ThreeD
    } else {
        RenderMode::TwoD
    };

    let mut renderer = FlameRenderer::with_palette_size(
        device,
        queue,
        wgpu::TextureFormat::Rgba8Unorm,
        PROBE_DIM,
        PROBE_DIM,
        &config.flame,
        config.palette_size,
    );
    // The probe compiles its own shader from the RAW flame and packs
    // params through the renderer: the sticky superset's canonical map
    // would misalign those get_param offsets. Measure the flame as it
    // is, always.
    renderer.set_sticky_enabled(false);

    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("probe config"),
    });
    renderer.load_config(device, &mut encoder, queue, &config, &config.palette, 1, 0);
    queue.submit(Some(encoder.finish()));

    // `load_config` packs the variation parameter buffer but only marks
    // the init-derived slots dirty — the pass that fills them normally
    // rides inside `render()`, which the probe never calls. Without this
    // the 134 variations with init-derived slots read zeros and compute
    // stable, reproducible, identical-on-every-platform nonsense.
    // `ripple` returning NaN at every input is what exposed it.
    renderer.run_init_pass(device, queue);

    let source = shader::build(batch, render_3d);
    let xform_count = batch.targets.len();

    let mut input = vec![0u32; shader::output_base(points.len())];
    input[0] = points.len() as u32;
    input[1] = xform_count as u32;
    for (i, point) in points.iter().enumerate() {
        let base = shader::HEADER_WORDS + i * shader::WORDS_PER_SLOT;
        input[base] = point.x.to_bits();
        input[base + 1] = point.y.to_bits();
        input[base + 2] = point.z.to_bits();
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
        phase,
    )?;

    let out_base = shader::output_base(points.len());
    let components = if render_3d { 3 } else { 2 };
    let dim = if render_3d { "3d" } else { "2d" };

    let mut entries = Vec::with_capacity(xform_count);
    for (xform_idx, target) in batch.targets.iter().enumerate() {
        let mut samples = Vec::with_capacity(points.len() * components);
        for point_idx in 0..points.len() {
            let slot = xform_idx * points.len() + point_idx;
            let base = out_base + slot * shader::WORDS_PER_SLOT;
            for c in 0..components {
                samples.push(Sample::of(f32::from_bits(words[base + c])));
            }
        }
        let (classes, digest) = summarise(&samples);
        entries.push(Entry {
            name: target.name.clone(),
            dim,
            classes,
            digest,
        });
    }
    Ok(entries)
}

/// Sweep every parameter of every variation in one batch.
///
/// Round *r* sets step *r* for every variation that still has one, then
/// dispatches once. Variations sit in separate transforms, so their
/// parameters are independent and the round covers all of them at once —
/// the batch costs `max steps of any one variation` dispatches rather
/// than the product of parameters and values.
///
/// The shader is built once. Parameters are read through `get_param`
/// from a storage buffer, so a step is a buffer write; only the base
/// pass pays for a compile.
fn sweep_batch(
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    batch: &Batch,
    render_3d: bool,
    rounds_out: &mut usize,
) -> Result<Vec<Entry>, String> {
    let points = probe_inputs();
    let dim = if render_3d { "3d" } else { "2d" };
    let components = if render_3d { 3 } else { 2 };

    // Flatten each variation's sweep once, up front.
    let schedule: Vec<Vec<super::sweep::Step>> = {
        let reg = crate::variations::global_registry();
        batch
            .targets
            .iter()
            .map(|t| reg.get(&t.name).map(super::sweep::steps_for).unwrap_or_default())
            .collect()
    };
    let rounds = schedule.iter().map(Vec::len).max().unwrap_or(0);
    *rounds_out = rounds;
    if rounds == 0 {
        return Ok(Vec::new());
    }

    let mut config = FractalConfig::default();
    config.flame = super::flame::build_probe_flame(batch);
    config.render_mode = if render_3d {
        RenderMode::ThreeD
    } else {
        RenderMode::TwoD
    };

    let mut renderer = FlameRenderer::with_palette_size(
        device,
        queue,
        wgpu::TextureFormat::Rgba8Unorm,
        PROBE_DIM,
        PROBE_DIM,
        &config.flame,
        config.palette_size,
    );
    // The probe compiles its own shader from the RAW flame and packs
    // params through the renderer: the sticky superset's canonical map
    // would misalign those get_param offsets. Measure the flame as it
    // is, always.
    renderer.set_sticky_enabled(false);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("probe sweep config"),
    });
    renderer.load_config(device, &mut encoder, queue, &config, &config.palette, 1, 0);
    queue.submit(Some(encoder.finish()));

    let source = shader::build(batch, render_3d);
    let xform_count = batch.targets.len();
    let total_slots = points.len() * xform_count;
    let out_base = shader::output_base(points.len());

    let mut input = vec![0u32; out_base];
    input[0] = points.len() as u32;
    input[1] = xform_count as u32;
    for (i, point) in points.iter().enumerate() {
        let base = shader::HEADER_WORDS + i * shader::WORDS_PER_SLOT;
        input[base] = point.x.to_bits();
        input[base + 1] = point.y.to_bits();
        input[base + 2] = point.z.to_bits();
    }

    // Accumulated per (variation index, parameter name), in the order
    // parameters were first seen so the report follows the definition.
    let mut collected: Vec<Vec<(String, Vec<Sample>, String)>> = vec![Vec::new(); xform_count];

    for round in 0..rounds {
        // Every variation gets exactly one non-default parameter, so a
        // result is attributable to that one parameter and nothing else.
        for (i, steps) in schedule.iter().enumerate() {
            let xf = &mut config.flame.transforms[i];
            xf.variation_params.clear();
            if let Some(step) = steps.get(round) {
                xf.set_variation_param(&batch.targets[i].name, &step.param, step.value);
            }
        }

        renderer.set_variation_params(queue, &config.flame);
        // The init-derived slots are computed from the user parameters,
        // so they are stale the moment one moves. Without this the sweep
        // would probe new parameters against the previous step's derived
        // values — a state no real flame is ever in.
        renderer.run_init_pass(device, queue);

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

        for (i, steps) in schedule.iter().enumerate() {
            let Some(step) = steps.get(round) else { continue };

            let mut samples = Vec::with_capacity(points.len() * components);
            for point_idx in 0..points.len() {
                let slot = i * points.len() + point_idx;
                let base = out_base + slot * shader::WORDS_PER_SLOT;
                for c in 0..components {
                    samples.push(Sample::of(f32::from_bits(words[base + c])));
                }
            }

            // A fixed-width presence mask for this value: which classes
            // occurred across every input and component. See
            // `classify::class_mask` for why a mask and not a single
            // representative class.
            let mask = class_mask(samples.iter().map(|s| s.class));

            let entry = match collected[i].iter_mut().find(|(p, _, _)| *p == step.param) {
                Some(e) => e,
                None => {
                    collected[i].push((step.param.clone(), Vec::new(), String::new()));
                    collected[i].last_mut().unwrap()
                }
            };
            entry.1.extend(samples);
            entry.2.push_str(&mask);
        }
    }

    let mut entries = Vec::new();
    for (i, per_param) in collected.into_iter().enumerate() {
        for (param, samples, mask) in per_param {
            // The digest covers every raw sample at every input, so a
            // difference the collapsed glyphs cannot show — a sign flip
            // at one input, say — still registers as a soft finding.
            let (_, digest) = summarise(&samples);
            entries.push(Entry {
                name: format!("{}.{param}", batch.targets[i].name),
                dim,
                classes: mask,
                digest,
            });
        }
    }
    Ok(entries)
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use crate::probe::batch::Target;
    use crate::probe::classify::Class;
    use crate::variations::VariationPhase;

    fn target(name: &str) -> Target {
        let reg = crate::variations::global_registry();
        let info = reg.get(name).expect("variation should exist");
        Target {
            name: name.to_string(),
            slots: info.slot_count(),
            needs_init: info.init_param_count > 0,
            phase: info.phase.clone(),
        }
    }

    /// End-to-end on whatever GPU is present: does the plumbing carry a
    /// known value through unchanged?
    ///
    /// `linear` at weight 1.0 with no affine applied is the identity, so
    /// every input must come back as itself. If the buffer layout, the
    /// slot indexing or the bitcasts are wrong, this is where it shows —
    /// and it is worth having a case whose right answer is known
    /// independently of any GPU, because every other result the probe
    /// produces is only ever compared against another run.
    #[test]
    fn linear_is_the_identity_end_to_end() {
        let batch = Batch {
            targets: vec![target("linear")],
            slots: 0,
        };

        let outcome = pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let Ok(adapter) = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
            else {
                return None;
            };
            let mut limits = wgpu::Limits::default();
            let al = adapter.limits();
            limits.max_storage_buffer_binding_size = al.max_storage_buffer_binding_size;
            limits.max_buffer_size = al.max_buffer_size;
            limits.max_storage_buffers_per_shader_stage = al.max_storage_buffers_per_shader_stage;
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("probe test"),
                    required_features: wgpu::Features::CLEAR_TEXTURE,
                    required_limits: limits,
                    memory_hints: wgpu::MemoryHints::Performance,
                    experimental_features: Default::default(),
                    trace: Default::default(),
                })
                .await
                .ok()?;
            Some(run_batch(&device, &queue, &batch, false, &mut |_, _| {}))
        });

        let Some(result) = outcome else {
            eprintln!("skipped: no GPU adapter available");
            return;
        };
        let entries = result.expect("probe dispatch should succeed");
        assert_eq!(entries.len(), 1);

        // The identity maps each input to itself — with one lawful
        // exception. GPUs may flush subnormals to zero, and the ones
        // tested do: the `subnormal` input (±1e-40) comes back as a
        // signed zero rather than the `NearZero` the CPU classifies it
        // as. That is hardware behaviour, not a plumbing fault, and it
        // is worth keeping in the grid precisely because *whether* a
        // platform flushes is the kind of thing the probe should
        // record. The sign survives the flush, which is why the two
        // components still come back distinguishable.
        let got: Vec<char> = entries[0].classes.chars().collect();
        let inputs = probe_inputs();
        assert_eq!(got.len(), inputs.len() * 2, "wrong number of components");

        for (i, point) in inputs.iter().enumerate() {
            for (c, value) in [point.x, point.y].into_iter().enumerate() {
                let actual = Class::from_glyph(got[i * 2 + c]).expect("valid glyph");
                let expected = Class::of(value);
                let flushed = matches!(expected, Class::NearZero)
                    && matches!(actual, Class::PosZero | Class::NegZero);
                assert!(
                    actual == expected || flushed,
                    "linear at `{}` component {c}: expected {expected:?}, got {actual:?} — \
                     the probe's plumbing is wrong, not the GPU's arithmetic",
                    point.label
                );
            }
        }
    }

    /// A pre-phase variation must produce something other than a column
    /// of zeros, which is what it would do without the carrier.
    #[test]
    fn a_pre_phase_variation_is_not_silently_zero() {
        let reg = crate::variations::global_registry();
        let pre = reg
            .names()
            .iter()
            .filter_map(|n| reg.get(n))
            .find(|i| i.phase == VariationPhase::Pre && i.provenance.is_builtin())
            .map(|i| i.name.clone());
        drop(reg);

        let Some(name) = pre else {
            eprintln!("skipped: no pre-phase variation in the registry");
            return;
        };
        let batch = Batch {
            targets: vec![target(&name)],
            slots: 0,
        };

        let outcome = pollster::block_on(async {
            let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
                backends: wgpu::Backends::all(),
                ..wgpu::InstanceDescriptor::new_without_display_handle()
            });
            let adapter = instance
                .request_adapter(&wgpu::RequestAdapterOptions::default())
                .await
                .ok()?;
            let mut limits = wgpu::Limits::default();
            let al = adapter.limits();
            limits.max_storage_buffer_binding_size = al.max_storage_buffer_binding_size;
            limits.max_buffer_size = al.max_buffer_size;
            limits.max_storage_buffers_per_shader_stage = al.max_storage_buffers_per_shader_stage;
            let (device, queue) = adapter
                .request_device(&wgpu::DeviceDescriptor {
                    label: Some("probe test"),
                    required_features: wgpu::Features::CLEAR_TEXTURE,
                    required_limits: limits,
                    memory_hints: wgpu::MemoryHints::Performance,
                    experimental_features: Default::default(),
                    trace: Default::default(),
                })
                .await
                .ok()?;
            Some(run_batch(&device, &queue, &batch, false, &mut |_, _| {}))
        });

        let Some(result) = outcome else {
            eprintln!("skipped: no GPU adapter available");
            return;
        };
        let entries = result.expect("probe dispatch should succeed");
        let all_zero = entries[0]
            .classes
            .chars()
            .all(|c| c == Class::PosZero.glyph() || c == Class::NegZero.glyph());
        assert!(
            !all_zero,
            "`{name}` returned zero everywhere — the carrier is missing, so the \
             pre phase's result is being discarded by an empty normal phase"
        );
    }
}
