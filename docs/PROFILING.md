# Performance Profiling Guide

This document describes the profiling and benchmarking tools available for the fractal flame renderer.

## Table of Contents

1. [Quick Start](#quick-start)
2. [CLI Benchmark Tool](#cli-benchmark-tool)
3. [GPU Profiling](#gpu-profiling)
4. [CPU Benchmarks](#cpu-benchmarks)
5. [Visual Regression Testing](#visual-regression-testing)
6. [Interpreting Results](#interpreting-results)
7. [Optimization Tips](#optimization-tips)

---

## Quick Start

### Basic Performance Test
```bash
# Run benchmark with default settings (Complex preset, 100 frames, 1080p)
cargo run --release --bin benchmark

# Quick test (10 frames)
cargo run --release --bin benchmark -- --frames 10

# 4K test
cargo run --release --bin benchmark -- --width 3840 --height 2160 --frames 50
```

### CPU Microbenchmarks
```bash
# Run Criterion benchmarks
cargo bench

# Run specific benchmark
cargo bench cpu_iteration
cargo bench variations
```

### Unit Tests
```bash
# Run all tests
cargo test

# Run regression tests only
cargo test --test regression

# Run with output
cargo test -- --nocapture
```

---

## CLI Benchmark Tool

The `benchmark` binary provides headless GPU rendering benchmarks.

### Basic Usage

```bash
cargo run --release --bin benchmark [OPTIONS]
```

### Options

| Flag | Description | Default |
|------|-------------|---------|
| `--preset <NAME>` | Preset to benchmark | complex |
| `--frames <N>` | Number of frames to render | 100 |
| `--width <N>` | Output width in pixels | 1920 |
| `--height <N>` | Output height in pixels | 1080 |
| `--iterations <N>` | Iterations per thread | 256 |
| `--workgroups <N>` | Number of workgroups | 128 |
| `--config <FILE>` | Load config from .flame file | - |
| `--output <FILE>` | Export final image (not yet implemented) | - |
| `--gpu-profile` | Enable GPU timestamp queries | false |
| `--validate` | Run CPU reference validation | false |

### Available Presets

- `simple` - Simple 2-transform flame
- `complex` - Complex 4-transform flame
- `spherical` - Spherical variation showcase
- `spiral` - Spiral pattern
- `julia` - Julia set variation
- `3d_spiral` - 3D mode spiral tower

### Examples

**Quick performance test:**
```bash
cargo run --release --bin benchmark -- --frames 10
```

**4K rendering benchmark:**
```bash
cargo run --release --bin benchmark -- \
  --width 3840 --height 2160 \
  --frames 50 \
  --preset complex
```

**GPU profiling (requires TIMESTAMP_QUERY support):**
```bash
cargo run --release --bin benchmark -- \
  --gpu-profile \
  --frames 100
```

**Validate CPU reference implementation:**
```bash
cargo run --release --bin benchmark -- \
  --validate \
  --preset simple \
  --frames 10
```

**Custom config file:**
```bash
cargo run --release --bin benchmark -- \
  --config my_flame.flame \
  --frames 100
```

### Output Explanation

```
=== Results ===
Total time: 2.45s          # Wall clock time for entire benchmark
Avg frame time: 24.52ms    # Average time per frame (CPU+GPU)
Avg FPS: 40.8              # Frames per second

Percentiles:               # Frame time distribution
  P50 (median): 24.12ms    # Typical frame time
  P95: 26.87ms             # 95% of frames faster than this
  P99: 28.34ms             # 99% of frames faster than this

GPU Breakdown:             # (only with --gpu-profile)
  Compute: 18.23ms         # Trajectory shader time
  Accumulate: 2.15ms       # Accumulation pass time
  Total GPU: 20.38ms       # Sum of GPU passes

Throughput:
  Total iterations: 3.35B  # Total flame iterations computed
  Iterations/sec: 1.37B    # Throughput (higher is better)
```

---

## GPU Profiling

The renderer includes GPU timestamp query profiling for measuring individual pass durations.

### Enabling GPU Profiling

**In benchmark tool:**
```bash
cargo run --release --bin benchmark -- --gpu-profile
```

**In code:**
```rust
use fractal_flame_wgpu::profiler::GpuProfiler;

// Create profiler (checks for TIMESTAMP_QUERY feature)
let profiler = GpuProfiler::new(&device);

if profiler.is_enabled() {
    // Use profiling
    profiler.begin_scope(&mut encoder, 0); // Start scope 0
    // ... render pass ...
    profiler.end_scope(&mut encoder, 0);   // End scope 0

    // Resolve queries
    profiler.resolve(&mut encoder);

    // Read timestamps asynchronously
    if let Some(timestamps) = profiler.read_timestamps(&queue).await {
        let period = queue.get_timestamp_period();
        let duration_ms = GpuProfiler::calculate_duration(&timestamps, 0, period);
        println!("Pass 0: {:.2}ms", duration_ms);
    }
}
```

### Query Indices

The profiler allocates 10 query slots (5 passes × 2 timestamps each):

| Index | Pass |
|-------|------|
| 0 | Compute (trajectory) |
| 1 | Accumulate |
| 2 | Tonemap |
| 3 | UI |
| 4 | Present |

### Limitations

- Requires `TIMESTAMP_QUERY` GPU feature
- Not supported on all platforms (especially mobile)
- Some drivers may have precision limits
- Async readback adds CPU overhead

### Platform Support

| Platform | Support |
|----------|---------|
| Desktop NVIDIA | ✅ Full |
| Desktop AMD | ✅ Full |
| Desktop Intel | ✅ Full |
| Apple Silicon | ✅ Full |
| WebGPU | ⚠️ Limited |
| Mobile | ❌ Rare |

---

## CPU Benchmarks

Criterion-based microbenchmarks measure CPU-side performance.

### Running Benchmarks

```bash
# Run all benchmarks
cargo bench

# Specific benchmark group
cargo bench cpu_iteration
cargo bench variations
cargo bench affine_transform
cargo bench point_calculations

# Generate detailed report
cargo bench -- --verbose

# Save baseline for comparison
cargo bench -- --save-baseline main

# Compare against baseline
cargo bench -- --baseline main
```

### Benchmark Groups

#### 1. CPU Iteration
Measures flame iteration performance on CPU.

```bash
cargo bench cpu_iteration
```

Tests:
- Single iteration (all presets)
- 100 iterations (all presets)

#### 2. Variations
Measures individual variation function performance.

```bash
cargo bench variations
```

Tests all 24 variation functions:
- 2D variations (0-15)
- 3D variations (16-23)

#### 3. Affine Transform
Measures transform application performance.

```bash
cargo bench affine_transform
```

Tests:
- Affine matrix multiplication
- Variation blending

#### 4. Point Calculations
Measures polar coordinate calculations.

```bash
cargo bench point_calculations
```

Tests:
- `r()` - radius
- `r_squared()` - radius²
- `theta()` - angle
- `phi()` - 3D angle

### Interpreting Criterion Output

```
cpu_iteration/single_iter/Complex
                        time:   [1.234 µs 1.245 µs 1.256 µs]
                        change: [-2.34% -1.23% +0.12%] (p = 0.23 > 0.05)
                        No change in performance detected.
```

- **time**: [min, estimate, max] in microseconds
- **change**: Performance change vs previous run
- **p-value**: Statistical significance (< 0.05 = significant change)

### Baseline Comparison

```bash
# Save baseline before optimization
cargo bench -- --save-baseline before

# Make code changes...

# Compare after changes
cargo bench -- --baseline before
```

---

## Visual Regression Testing

Visual regression tests ensure rendering output doesn't change unexpectedly.

### Generating Reference Images

```bash
cargo run --release --bin generate_references
```

This creates reference images in `tests/visual_references/`:
- PNG images (512×512, 50 frames)
- Checksum files for metadata

### Running Visual Tests

```bash
# Not yet implemented
cargo test --test visual_regression
```

### Manual Visual Inspection

Export images and compare visually:

```bash
# Export reference
cargo run --release --bin benchmark -- \
  --preset complex \
  --frames 100 \
  --output reference.png

# After changes, export again
cargo run --release --bin benchmark -- \
  --preset complex \
  --frames 100 \
  --output modified.png

# Compare with image diff tool
compare reference.png modified.png diff.png  # ImageMagick
```

---

## Interpreting Results

### Frame Time Breakdown

Typical frame time distribution:

```
Total Frame: 16.67ms (60 FPS target)
├─ GPU Compute: 12.00ms (72%)  ← Trajectory shader (bottleneck)
├─ GPU Accumulate: 1.50ms (9%) ← Blending pass
├─ GPU Tonemap: 0.50ms (3%)    ← Display rendering
├─ CPU UI: 1.00ms (6%)         ← egui rendering
└─ CPU Submit: 1.67ms (10%)    ← Command submission
```

### Performance Targets

| Resolution | Target FPS | Frame Budget |
|------------|------------|--------------|
| 720p | 60 | 16.67ms |
| 1080p | 60 | 16.67ms |
| 1440p | 60 | 16.67ms |
| 4K | 30 | 33.33ms |

### Throughput Metrics

**Good performance** (RTX 3060 / M1 Pro):
- 1080p: 200-400 FPS (5-2.5ms/frame)
- 4K: 50-100 FPS (20-10ms/frame)
- Throughput: 1-2 billion iterations/sec

**Typical bottlenecks:**
1. **GPU Compute** (70-80% of time)
   - Increase workgroups
   - Reduce iterations per thread
   - Optimize variation functions

2. **Memory Bandwidth** (10-20%)
   - Reduce accumulation buffer size
   - Use lower precision (Rgba16Float)

3. **CPU Submit** (5-10%)
   - Batch more work per frame
   - Reduce command buffer overhead

---

## Optimization Tips

### 1. Adjust Workload Parameters

```rust
// More workgroups = better GPU utilization
workgroups: 256  // Default: 128

// Fewer iterations = lower latency (but noisier)
iterations_per_thread: 128  // Default: 256
```

### 2. Profile Before Optimizing

Always measure before and after:

```bash
# Before
cargo bench -- --save-baseline before

# Make changes

# After
cargo bench -- --baseline before
```

### 3. GPU vs CPU Workload

The renderer is GPU-bound (70-80% time in compute shader).

**To optimize GPU:**
- Simplify variation functions
- Reduce transform count
- Lower resolution
- Reduce iterations per frame

**To optimize CPU:**
- Minimize UI updates
- Reduce command buffer overhead
- Use release builds (`--release`)

### 4. Resolution vs Frame Rate

Frame time scales roughly with pixel count:

| Resolution | Pixels | Expected Time |
|------------|--------|---------------|
| 720p | 0.9M | 1.0× (baseline) |
| 1080p | 2.1M | 2.3× |
| 1440p | 3.7M | 4.1× |
| 4K | 8.3M | 9.2× |

### 5. Quality vs Performance

Progressive refinement allows trading quality for speed:

- **Low quality (interactive)**: 10-50 frames
- **Medium quality (preview)**: 100-500 frames
- **High quality (export)**: 1000-10000 frames

---

## Common Issues

### GPU Profiling Not Available

```
Warning: GPU profiling requested but TIMESTAMP_QUERY not supported
```

**Solution**: Not all GPUs support timestamp queries. Use CPU timing instead.

### Low FPS on High-End GPU

**Check:**
1. Running in release mode? (`--release`)
2. VSync enabled? (limits to monitor refresh rate)
3. GPU actually being used? (check task manager)

### Benchmark Results Inconsistent

**Tips:**
1. Close other GPU applications
2. Use performance power profile
3. Run multiple times and average
4. Disable GPU boost/throttling

### Out of Memory

```
Error: Failed to create buffer
```

**Solution**: Reduce resolution or close other applications.

---

## References

- [wgpu Profiling Guide](https://github.com/gfx-rs/wgpu/wiki/Profiling)
- [Criterion.rs User Guide](https://bheisler.github.io/criterion.rs/book/)
- [GPU Timestamp Queries](https://www.khronos.org/opengl/wiki/Query_Object#Timestamp_queries)

---

**Last Updated:** 2025-10-21
**Project:** fflame-rust
