//! Phase 0 of the GPU reference-orbit plan: measure, then decide.
//!
//! [`docs/projects/escape-ntt-reference.md`] gates that project on a
//! number rather than an intuition — "GO if ≥3x at 197 limbs;
//! otherwise park the project and record the numbers". This module is
//! that measurement.
//!
//! WHAT IS BEING MEASURED. A reference orbit is `z ← z² + c` in
//! fixed-point, and it is strictly SEQUENTIAL: iteration n+1 needs
//! iteration n. So the only parallelism available is *inside one
//! multiply*, which means one workgroup — a single SM — no matter how
//! large the GPU. That is the whole question: can one workgroup of
//! 16-bit digit products beat one CPU core's 64-bit ones?
//!
//! The GPU is at a structural disadvantage that has nothing to do with
//! how fast it is: WGSL has no 64-bit integer multiply and no
//! multiply-high, so each u64×u64 product the CPU issues as a single
//! instruction becomes SIXTEEN 16×16→32 products here. The GPU has to
//! win that 16x back out of parallelism alone.
//!
//! HONESTY OF THE MEASUREMENT. A throughput probe that skips work
//! measures nothing, and a big-integer kernel is an easy place to skip
//! work by accident. So the kernel is verified BIT-EXACTLY against a
//! Rust reference implementing the identical digit algorithm before
//! any timing is reported: same truncation window, same carry
//! propagation, same digit count. If the two disagree the probe fails
//! rather than prints a number.
//!
//! The CPU baseline is the calibrated cost model from `reference.rs`
//! (`predicted_orbit_seconds`), which is measured on this project's
//! real f3 build rather than re-derived here.

#![cfg(test)]

/// u16 digits per u64 limb of the CPU representation.
const DIGITS_PER_LIMB: usize = 4;

/// Guard digits kept below the truncation window, mirroring
/// `mul_trunc`'s two guard limbs.
const GUARD_DIGITS: usize = 4;

/// One truncated squaring in the digit representation the GPU kernel
/// uses: `out = (a·a) >> (16·(D − GUARD))`, digits little-endian.
///
/// This is the oracle. It is deliberately the most literal
/// transcription of the algorithm the shader runs — same window, same
/// accumulation order, same carry sweep — because its only job is to
/// prove the shader did the work.
fn cpu_square_digits(a: &[u32], out: &mut [u32]) {
    let d = a.len();
    let k0 = d - GUARD_DIGITS;
    // Output digits k0..2d-1, accumulated as 64-bit sums.
    let span = 2 * d - k0;
    let mut acc = vec![0u64; span];
    for i in 0..d {
        for j in 0..d {
            let k = i + j;
            if k < k0 {
                continue;
            }
            acc[k - k0] += (a[i] as u64) * (a[j] as u64);
        }
    }
    // Carry sweep, low to high, 16 bits per digit.
    let mut carry = 0u64;
    for slot in acc.iter_mut() {
        let v = *slot + carry;
        *slot = v & 0xFFFF;
        carry = v >> 16;
    }
    // The kept window starts GUARD digits up: that is the >> above.
    for (n, slot) in out.iter_mut().enumerate() {
        *slot = acc.get(n + GUARD_DIGITS).copied().unwrap_or(0) as u32;
    }
}

/// The shader. `{D}` (digits) and `{WG}` (workgroup size) are
/// substituted; everything else is fixed.
///
/// One workgroup owns the whole multiply. Each thread owns a set of
/// OUTPUT digits (strided), so no atomics are needed for the
/// accumulation: a thread reads all of `a` and writes only its own
/// accumulator slots. The carry sweep is inherently sequential and
/// runs on one thread — cheap next to the D²/2 products, and a real
/// implementation would face the same sweep.
const SQUARE_WGSL: &str = r#"
const D: u32 = {D}u;
const K0: u32 = D - 4u;          // truncation window start
const SPAN: u32 = 2u * D - K0;   // accumulator slots

@group(0) @binding(0) var<storage, read_write> state: array<u32>;

var<workgroup> a: array<u32, D>;
var<workgroup> acc_lo: array<u32, SPAN>;
var<workgroup> acc_hi: array<u32, SPAN>;

@compute @workgroup_size({WG})
fn main(@builtin(local_invocation_id) lid: vec3<u32>) {
    let tid = lid.x;
    let iters = state[D];   // host layout: D digits, then the count

    // Load the operand into workgroup memory once.
    for (var i = tid; i < D; i = i + {WG}u) {
        a[i] = state[i];
    }
    workgroupBarrier();

    for (var it = 0u; it < iters; it = it + 1u) {
        // Clear this thread's accumulator slots.
        for (var k = tid; k < SPAN; k = k + {WG}u) {
            acc_lo[k] = 0u;
            acc_hi[k] = 0u;
        }
        workgroupBarrier();

        // Products landing in the kept window. Thread `tid` owns
        // output digits k0 + tid, stepping by the workgroup size, so
        // each accumulator slot has exactly one writer.
        for (var k = K0 + tid; k < 2u * D - 1u; k = k + {WG}u) {
            var lo = 0u;
            var hi = 0u;
            // i from max(0, k-(D-1)) to min(D-1, k)
            var i = 0u;
            if (k >= D) { i = k - D + 1u; }
            let i_end = min(D - 1u, k);
            for (; i <= i_end; i = i + 1u) {
                let p = a[i] * a[k - i];   // 16x16 -> fits u32
                let s = lo + p;
                if (s < lo) { hi = hi + 1u; }
                lo = s;
            }
            acc_lo[k - K0] = lo;
            acc_hi[k - K0] = hi;
        }
        workgroupBarrier();

        // Sequential carry sweep + write-back of the kept window.
        if (tid == 0u) {
            var c_lo = 0u;
            var c_hi = 0u;
            for (var s = 0u; s < SPAN; s = s + 1u) {
                // v = acc[s] + carry, in 64 bits held as two u32.
                var v_lo = acc_lo[s] + c_lo;
                var v_hi = acc_hi[s] + c_hi;
                if (v_lo < c_lo) { v_hi = v_hi + 1u; }
                let digit = v_lo & 0xFFFFu;
                // carry = v >> 16
                c_lo = (v_lo >> 16u) | (v_hi << 16u);
                c_hi = v_hi >> 16u;
                acc_lo[s] = digit;
            }
            // The kept result starts one guard span up.
            for (var n = 0u; n < D; n = n + 1u) {
                let src = n + 4u;
                var digit = 0u;
                if (src < SPAN) { digit = acc_lo[src]; }
                a[n] = digit;
            }
        }
        workgroupBarrier();
    }

    // Publish.
    for (var i = tid; i < D; i = i + {WG}u) {
        state[i] = a[i];
    }
}
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use egui_wgpu::wgpu;

    fn device() -> Option<(wgpu::Device, wgpu::Queue)> {
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
        let mut limits = wgpu::Limits::default();
        limits.max_compute_workgroup_storage_size =
            adapter.limits().max_compute_workgroup_storage_size;
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("bignum probe"),
            required_features: wgpu::Features::empty(),
            required_limits: limits,
            memory_hints: wgpu::MemoryHints::Performance,
            experimental_features: Default::default(),
            trace: Default::default(),
        }))
        .ok()
    }

    /// Run `iters` squarings of `digits` on the GPU, returning the
    /// final digit vector and the wall time of the dispatch.
    fn gpu_square(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        digits: &[u32],
        iters: u32,
        wg: u32,
    ) -> Option<(Vec<u32>, std::time::Duration)> {
        let d = digits.len();
        let src = SQUARE_WGSL
            .replace("{D}", &d.to_string())
            .replace("{WG}", &wg.to_string());
        // Workgroup storage: a[D] + acc_lo[SPAN] + acc_hi[SPAN],
        // SPAN = D + 4. Over the device's limit this size simply
        // cannot be measured with a shared-memory layout -- report it
        // rather than trip a validation error.
        let need = 4 * (3 * d as u32 + 8);
        if need > device.limits().max_compute_workgroup_storage_size {
            return None;
        }
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("bignum square"),
            source: wgpu::ShaderSource::Wgsl(src.into()),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("bignum square"),
            layout: None,
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let mut host = digits.to_vec();
        host.push(iters);
        let bytes: &[u8] = bytemuck::cast_slice(&host);
        let storage = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bignum state"),
            size: bytes.len() as u64,
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_DST
                | wgpu::BufferUsages::COPY_SRC,
            mapped_at_creation: false,
        });
        queue.write_buffer(&storage, 0, bytes);
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("bignum readback"),
            size: bytes.len() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: storage.as_entire_binding(),
            }],
        });

        // Warm up (shader compile, first-touch) before timing.
        for round in 0..2 {
            if round == 1 {
                queue.write_buffer(&storage, 0, bytes);
                let _ = device.poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: None,
                });
            }
            let t0 = std::time::Instant::now();
            let mut enc =
                device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
            {
                let mut pass = enc.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: None,
                    timestamp_writes: None,
                });
                pass.set_pipeline(&pipeline);
                pass.set_bind_group(0, &bind, &[]);
                pass.dispatch_workgroups(1, 1, 1);
            }
            enc.copy_buffer_to_buffer(&storage, 0, &readback, 0, bytes.len() as u64);
            queue.submit(std::iter::once(enc.finish()));
            let _ = device.poll(wgpu::PollType::Wait {
                submission_index: None,
                timeout: None,
            });
            if round == 1 {
                let elapsed = t0.elapsed();
                let slice = readback.slice(..);
                let (tx, rx) = std::sync::mpsc::channel();
                slice.map_async(wgpu::MapMode::Read, move |r| {
                    let _ = tx.send(r);
                });
                let _ = device.poll(wgpu::PollType::Wait {
                    submission_index: None,
                    timeout: None,
                });
                rx.recv().ok()?.ok()?;
                let view = slice.get_mapped_range();
                let out: Vec<u32> = bytemuck::cast_slice::<u8, u32>(&view)[..d].to_vec();
                drop(view);
                readback.unmap();
                return Some((out, elapsed));
            }
        }
        None
    }

    /// Deterministic pseudo-random digits with the top digit set, so
    /// the value occupies its full precision.
    fn sample_digits(d: usize) -> Vec<u32> {
        let mut v = Vec::with_capacity(d);
        let mut x = 0x9E3779B9u32;
        for _ in 0..d {
            x = x.wrapping_mul(1664525).wrapping_add(1013904223);
            v.push((x >> 13) & 0xFFFF);
        }
        v[d - 1] |= 0x8000;
        v
    }

    /// Phase 0's gate. Prints the comparison and asserts only the
    /// things that would invalidate it (the kernel must be correct);
    /// the VERDICT is recorded in the plan doc, not enforced here --
    /// a slower GPU on someone else's machine is data, not a defect.
    #[test]
    #[ignore = "needs a GPU"]
    fn gpu_bignum_throughput_vs_cpu_model() {
        let Some((device, queue)) = device() else {
            println!("no GPU adapter; skipping");
            return;
        };

        println!(
            "\n=== Phase 0: GPU big-number throughput (one workgroup, sequential orbit) ===\n\
             limbs  digits  CPU model/iter   GPU/iter   ratio   verdict"
        );
        for &limbs in &[64usize, 197, 512] {
            let d = limbs * DIGITS_PER_LIMB;
            let digits = sample_digits(d);

            // Correctness first: one squaring, bit-exact against the
            // oracle. A fast wrong kernel measures nothing.
            let mut expect = vec![0u32; d];
            cpu_square_digits(&digits, &mut expect);
            let Some((got, _)) = gpu_square(&device, &queue, &digits, 1, 256) else {
                println!("{limbs:5}  {d:6}  (kernel would not build at this size)");
                continue;
            };
            assert_eq!(
                got, expect,
                "GPU squaring disagrees with the oracle at {limbs} limbs -- \
                 the timing below would be meaningless"
            );

            // Timing: enough iterations to swamp launch overhead.
            let iters = 200u32;
            let (_, elapsed) = gpu_square(&device, &queue, &digits, iters, 256)
                .expect("timing run after a successful correctness run");
            // Two big multiplies per complex squaring; the probe does
            // one, so an orbit iteration costs about twice this.
            let gpu_iter_s = elapsed.as_secs_f64() / iters as f64 * 2.0;
            let cpu_iter_s = crate::escape::reference::predicted_orbit_seconds(1, limbs);
            let ratio = cpu_iter_s / gpu_iter_s;
            let verdict = if ratio >= 3.0 { "GO" } else { "no" };
            println!(
                "{limbs:5}  {d:6}  {:>12.1} us  {:>8.1} us  {ratio:5.2}x   {verdict}",
                cpu_iter_s * 1e6,
                gpu_iter_s * 1e6,
            );
        }
        println!();
    }
}
