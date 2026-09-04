//! Simulation mode, phase 0: what a stencil step actually costs.
//!
//! Test-only (`#[cfg(test)]` at the module declaration) — this is not
//! product code and nothing links it. It exists to replace the
//! estimates in `docs/projects/simulation-pipeline.md` §5 and §10 with
//! measurements *before* the driver is designed around them, which is
//! the phase-0 gate in the master plan.
//!
//! What it measures is the shape every Tier-1 model shares: two
//! `rgba32float` textures ping-ponged by a compute pass that gathers a
//! 3×3 neighbourhood and writes one texel. The arithmetic is
//! Gray–Scott's, including the clamp the NumPy prototype showed is
//! mandatory, so the cost is a real model's cost and not an empty
//! kernel the compiler could fold away.
//!
//! Two things the pipeline document needs and could only estimate:
//!
//! - **ms per step against grid size**, which decides how many steps a
//!   frame can afford and therefore whether the interactive budget in
//!   §5 is reachable at all.
//! - **how much of that is submission overhead**, measured by running
//!   the same step count as one submission of N and as N submissions
//!   of one. The driver batches steps per submit; this says what the
//!   batching is worth and what a hard cap costs.
//!
//! Run it:
//!
//! ```text
//! cargo test --release --lib sim_microbench -- --ignored --nocapture
//! ```

use std::time::Instant;

const SHADER: &str = r#"
@group(0) @binding(0) var src: texture_2d<f32>;
@group(0) @binding(1) var dst: texture_storage_2d<rgba32float, write>;

// Gray-Scott, Sims' scheme: D_A = 1, D_B = 0.5, dt = 1, 3x3 weights
// -1 / 0.2 / 0.05, periodic, clamped to [0, 1]. The clamp is not
// decoration -- without it the NumPy prototype reached NaN within a
// few thousand steps at some feed/kill pairs.
@compute @workgroup_size(8, 8, 1)
fn main(@builtin(global_invocation_id) gid: vec3<u32>) {
    let dims = textureDimensions(src);
    if (gid.x >= dims.x || gid.y >= dims.y) {
        return;
    }
    let w = i32(dims.x);
    let h = i32(dims.y);
    let x = i32(gid.x);
    let y = i32(gid.y);

    let c = textureLoad(src, vec2<i32>(x, y), 0);
    var lap = vec4<f32>(0.0, 0.0, 0.0, 0.0);
    for (var dy = -1; dy <= 1; dy = dy + 1) {
        for (var dx = -1; dx <= 1; dx = dx + 1) {
            if (dx == 0 && dy == 0) {
                continue;
            }
            let sx = (x + dx + w) % w;
            let sy = (y + dy + h) % h;
            let wgt = select(0.05, 0.2, dx == 0 || dy == 0);
            lap = lap + wgt * textureLoad(src, vec2<i32>(sx, sy), 0);
        }
    }
    lap = lap - c;

    let a = c.x;
    let b = c.y;
    let abb = a * b * b;
    let f = 0.0545;
    let k = 0.062;
    let na = clamp(a + (lap.x - abb + f * (1.0 - a)), 0.0, 1.0);
    let nb = clamp(b + (0.5 * lap.y + abb - (f + k) * b), 0.0, 1.0);
    textureStore(dst, vec2<i32>(x, y), vec4<f32>(na, nb, 0.0, 1.0));
}
"#;

struct Bench {
    device: wgpu::Device,
    queue: wgpu::Queue,
    pipeline: wgpu::ComputePipeline,
}

impl Bench {
    fn new() -> Option<Self> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .ok()?;
        let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("sim microbench"),
            required_features: wgpu::Features::empty(),
            required_limits: adapter.limits(),
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        }))
        .ok()?;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sim stencil"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("sim stencil"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        println!("adapter: {:?}", adapter.get_info());
        Some(Bench {
            device,
            queue,
            pipeline,
        })
    }

    fn textures(&self, w: u32, h: u32) -> [wgpu::Texture; 2] {
        let desc = wgpu::TextureDescriptor {
            label: Some("sim field"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba32Float,
            // Both usages on both textures: they swap roles every step,
            // which is the whole point of the ping-pong.
            usage: wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::STORAGE_BINDING
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        };
        [
            self.device.create_texture(&desc),
            self.device.create_texture(&desc),
        ]
    }

    /// Run `steps` steps, `per_submit` of them per command buffer.
    /// Returns wall-clock seconds for the whole run, measured after the
    /// queue has drained so nothing is still in flight.
    fn run(&self, tex: &[wgpu::Texture; 2], w: u32, h: u32, steps: u32, per_submit: u32) -> f64 {
        let views: Vec<wgpu::TextureView> = tex
            .iter()
            .map(|t| t.create_view(&wgpu::TextureViewDescriptor::default()))
            .collect();
        let layout = self.pipeline.get_bind_group_layout(0);
        // Two bind groups, one per ping-pong direction, built once.
        let groups: Vec<wgpu::BindGroup> = (0..2)
            .map(|i| {
                self.device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("sim step"),
                    layout: &layout,
                    entries: &[
                        wgpu::BindGroupEntry {
                            binding: 0,
                            resource: wgpu::BindingResource::TextureView(&views[i]),
                        },
                        wgpu::BindGroupEntry {
                            binding: 1,
                            resource: wgpu::BindingResource::TextureView(&views[1 - i]),
                        },
                    ],
                })
            })
            .collect();

        let gx = w.div_ceil(8);
        let gy = h.div_ceil(8);
        let start = Instant::now();
        let mut done = 0;
        while done < steps {
            let batch = per_submit.min(steps - done);
            let mut enc = self
                .device
                .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: None,
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.pipeline);
                for s in 0..batch {
                    pass.set_bind_group(0, &groups[((done + s) % 2) as usize], &[]);
                    pass.dispatch_workgroups(gx, gy, 1);
                }
            }
            self.queue.submit(std::iter::once(enc.finish()));
            done += batch;
        }
        let _ = self.device.poll(wgpu::PollType::Wait {
            submission_index: None,
            timeout: None,
        });
        start.elapsed().as_secs_f64()
    }
}

/// The phase-0 step-cost table: ms per step against grid size, and what
/// submission batching is worth.
#[test]
#[ignore = "manual: GPU microbenchmark, phase 0 of simulation mode"]
fn stencil_step_cost() {
    let Some(b) = Bench::new() else {
        println!("no GPU adapter; skipping");
        return;
    };

    // 256 and 512 are the catalogue's working grids; 1080p and 4K are
    // the viewport-bound cases the pipeline has to survive.
    let grids: [(u32, u32); 5] = [
        (256, 256),
        (512, 512),
        (1024, 1024),
        (1920, 1080),
        (3840, 2160),
    ];

    println!();
    println!(
        "{:>11}  {:>9}  {:>10}  {:>10}  {:>9}  {:>8}",
        "grid", "MiB", "ms/step", "steps/s", "per 16.7ms", "cells/ns"
    );
    for (w, h) in grids {
        let tex = b.textures(w, h);
        // Warm up: first dispatch pays pipeline setup and allocation.
        b.run(&tex, w, h, 16, 16);
        // Enough steps that the run is long against timer noise, but
        // capped so 4K does not sit under the watchdog.
        let steps = if w * h > 4_000_000 { 200 } else { 1000 };
        let secs = b.run(&tex, w, h, steps, 64);
        let ms = secs * 1000.0 / steps as f64;
        let mib = 2.0 * (w as f64) * (h as f64) * 16.0 / (1024.0 * 1024.0);
        println!(
            "{:>5}x{:<5}  {:>9.1}  {:>10.4}  {:>10.0}  {:>9.1}  {:>8.2}",
            w,
            h,
            mib,
            ms,
            1.0 / (ms / 1000.0),
            16.7 / ms,
            (w as f64 * h as f64) / (ms * 1.0e6)
        );
    }

    // What batching is worth: the same work as one submit of 256 and as
    // 256 submits of one. The driver batches; this prices it.
    println!();
    let (w, h) = (1920u32, 1080u32);
    let tex = b.textures(w, h);
    b.run(&tex, w, h, 16, 16);
    for per in [1u32, 8, 64, 256] {
        let secs = b.run(&tex, w, h, 256, per);
        println!(
            "1920x1080  {:>4} steps/submit -> {:>8.4} ms/step",
            per,
            secs * 1000.0 / 256.0
        );
    }
    println!();
}
