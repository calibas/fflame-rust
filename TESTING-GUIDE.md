# Complete Testing & Profiling Guide

Quick reference for running all types of tests, benchmarks, and profiling tools.

## Quick Command Reference

```bash
# Unit Tests (built into modules)
cargo test

# Regression Tests (integration tests)
cargo test --test regression

# CPU Benchmarks (Criterion)
cargo bench

# Simple CPU Benchmark
cargo run --release --bin simple_benchmark

# Show Version Info
cargo run --example show_version

# Run Main App
cargo run --release
```

---

## 1. Unit Tests

**What:** Tests embedded in source files (transforms.rs, palette.rs, etc.)

**Location:** Bottom of source files in `#[cfg(test)] mod tests { ... }`

**Run:**
```bash
# All unit tests
cargo test

# Specific module
cargo test --lib transforms
cargo test --lib palette
cargo test --lib version

# With output
cargo test -- --nocapture

# Release mode (faster)
cargo test --release
```

**Example Output:**
```
running 15 tests
test scene::transforms::tests::test_affine_identity ... ok
test scene::transforms::tests::test_point_calculations ... ok
test scene::palette::tests::test_palette_interpolation ... ok
test version::tests::test_version_info ... ok
...
test result: ok. 15 passed; 0 failed; 0 ignored
```

**What's Tested:**
- Transform math (affine, variations)
- Point calculations (r, θ, φ)
- Palette interpolation
- Version info capture
- Config serialization

---

## 2. Regression Tests

**What:** Integration tests to prevent breaking changes

**Location:** [tests/regression.rs](tests/regression.rs)

**Run:**
```bash
# Run regression test suite
cargo test --test regression

# With verbose output
cargo test --test regression -- --nocapture

# Release mode
cargo test --release --test regression
```

**Example Output:**
```
running 12 tests
test test_2d_variations ... ok
test test_affine_identity ... ok
test test_cpu_reference_deterministic ... ok
test test_point_calculations ... ok
test test_projection_types ... ok
test test_render_mode ... ok
test test_all_variations_no_panic ... ok
test test_color_blending ... ok
test test_variation_symmetries ... ok
test test_preset_configs_valid ... ok
test test_config_serialization ... ok
test test_transform_weights_valid ... ok

test result: ok. 12 passed; 0 failed; 0 ignored; 0 measured
```

**What's Tested:**
- CPU reference determinism
- All 24 variations work without panicking
- Transform weights are valid
- Affine identity transform
- All presets are well-formed
- Variation symmetries
- Color blending math
- Point calculations
- Config serialization round-trip

---

## 3. CPU Benchmarks (Criterion)

**What:** Precise microbenchmarks for CPU code

**Location:** [benches/flame_bench.rs](benches/flame_bench.rs)

**Run:**
```bash
# All benchmarks
cargo bench

# Specific benchmark group
cargo bench cpu_iteration
cargo bench variations
cargo bench affine_transform
cargo bench point_calculations

# Save baseline for comparison
cargo bench -- --save-baseline before-optimization

# Compare against baseline
cargo bench -- --baseline before-optimization
```

**Example Output:**
```
cpu_iteration/single_iter/Complex
                        time:   [1.234 µs 1.245 µs 1.256 µs]
                        change: [-2.34% -1.23% +0.12%] (p = 0.23 > 0.05)
                        No change in performance detected.

variations/apply/Linear time:   [26.4 ns 26.5 ns 26.6 ns]
variations/apply/Spherical
                        time:   [26.3 ns 26.4 ns 26.5 ns]
```

**What's Benchmarked:**
- **CPU iteration** - Full flame iteration (all presets)
- **Variations** - Each of 24 variation functions
- **Affine transform** - Matrix multiplication
- **Point calculations** - r, r², θ, φ

**Understanding Output:**
- `time: [min, estimate, max]` - Measurement range in µs/ns/ms
- `change: [min, estimate, max]` - Performance change vs previous run
- `p = 0.23` - Statistical significance (< 0.05 = significant change)

---

## 4. Simple CPU Benchmark

**What:** Quick, human-readable CPU performance test

**Location:** [src/bin/simple_benchmark.rs](src/bin/simple_benchmark.rs)

**Run:**
```bash
cargo run --release --bin simple_benchmark
```

**Example Output:**
```
=== Fractal Flame CPU Benchmark ===

Preset: Complex (4 transforms)
  10000 iterations in 0.81ms
  12.34 M iter/sec
  Final point: (0.4989, -0.1983)

Preset: Spherical (2 transforms)
  10000 iterations in 0.44ms
  22.61 M iter/sec
  Final point: (0.5050, 0.5050)

=== Variation Performance Test ===
Linear: 37.87 M ops/sec (result: 0.7000, 0.3000)
Sinusoidal: 32.01 M ops/sec (result: 0.0017, 0.0017)
Spherical: 38.01 M ops/sec (result: 0.8908, 0.3816)
...

=== Affine Transform Test ===
Affine only: 259.67 M ops/sec
Affine + variations: 31.85 M ops/sec
```

**What's Benchmarked:**
- All presets (full iteration)
- All 24 variations individually
- Affine transform performance

**Use Cases:**
- Quick sanity check
- Before/after optimization comparison
- Platform comparison (Windows vs Linux vs macOS)

---

## 5. GPU Profiling (Desktop)

**What:** Detailed GPU pass timing using timestamp queries

**Requires:** GPU with TIMESTAMP_QUERY support (most modern GPUs)

**Status:** Infrastructure in place, not yet integrated into UI

**Code Location:** [src/profiler.rs](src/profiler.rs)

**How to Use (in code):**
```rust
use fractal_flame_wgpu::profiler::GpuProfiler;

let profiler = GpuProfiler::new(&device);

// In render loop
profiler.begin_scope(&mut encoder, 0); // Start timing
// ... GPU compute pass ...
profiler.end_scope(&mut encoder, 0);   // End timing

profiler.resolve(&mut encoder);
queue.submit(encoder.finish());

// Read results (async)
if let Some(timestamps) = profiler.read_timestamps(&queue).await {
    let period = queue.get_timestamp_period();
    let duration_ms = GpuProfiler::calculate_duration(&timestamps, 0, period);
    println!("GPU compute: {:.2}ms", duration_ms);
}
```

**Query Indices:**
- 0: Compute (trajectory shader)
- 1: Accumulate pass
- 2: Tonemap pass
- 3: UI pass
- 4: Present

**Note:** Full integration into the app is planned but not yet implemented. Currently requires manual code additions.

---

## 6. CPU Profiling (Desktop)

**What:** Real-time CPU timing displayed in UI

**Always Running:** Performance metrics tracked automatically

**View:**
- Run the app: `cargo run --release`
- Look at the **Performance** window
- Shows:
  - FPS
  - Frame time (min/max/avg)
  - Component timings (compute, accumulate, tonemap, UI)
  - Version and build number

**Export Snapshot:**
```rust
let metrics = PerformanceMetrics::new();
// ... run app ...
let snapshot = metrics.snapshot();
let json = serde_json::to_string_pretty(&snapshot)?;
println!("{}", json);
```

**Output:**
```json
{
  "version": "0.1.0 (build #9)",
  "build_number": 9,
  "git_hash": "dba27e8",
  "fps": 250.5,
  "frame_time_ms": 3.99,
  "compute_time_ms": 2.5,
  "accumulate_time_ms": 0.8,
  "tonemap_time_ms": 0.3,
  "ui_time_ms": 0.4,
  "timestamp": "2025-10-21T23:45:00Z"
}
```

---

## 7. WASM Profiling

**What:** Browser-based performance monitoring

**See:** [WASM-PROFILING.md](WASM-PROFILING.md) for complete guide

**Quick Start:**
```bash
# Build WASM
./build-wasm.bat  # Windows
./build-wasm.sh   # Linux/macOS

# Serve
python -m http.server 8080

# Open http://localhost:8080
# Open browser console (F12)
```

**Browser Console Methods:**
```javascript
// Automatic logging (already in code)
// Check console for performance snapshots

// Manual export (if exposed)
metrics.export_to_console();
```

**Browser DevTools:**
1. F12 → Performance tab
2. Click Record 🔴
3. Interact with renderer
4. Click Stop
5. Analyze flame graph

---

## 8. Visual Regression Testing

**Status:** Infrastructure in place, image generation not yet implemented

**Planned Usage:**
```bash
# Generate reference images
cargo run --release --bin generate_references

# Run visual tests
cargo test --test visual_regression
```

**Current Status:**
- Metadata generation works
- GPU readback for image capture not yet implemented
- Manual visual inspection required

---

## Typical Workflow

### 1. Before Making Changes

```bash
# Run all tests to establish baseline
cargo test
cargo test --test regression
cargo bench -- --save-baseline before

# Note current version
cargo run --example show_version
```

### 2. After Making Changes

```bash
# Quick validation
cargo test

# Regression check
cargo test --test regression

# Performance comparison
cargo bench -- --baseline before

# Manual testing
cargo run --release
```

### 3. Performance Investigation

```bash
# Quick check
cargo run --release --bin simple_benchmark

# Detailed benchmarks
cargo bench

# Profile specific variation
cargo bench variations/apply/YourVariation
```

### 4. Before Release

```bash
# Full test suite
cargo test --release
cargo test --release --test regression

# Benchmark all presets
cargo bench

# Manual testing
cargo run --release

# Check version
cargo run --example show_version

# Note build number for release notes
```

---

## Performance Targets

### Desktop (Release Build)

| Test | Target | Good | Excellent |
|------|--------|------|-----------|
| Unit tests | All pass | All pass | All pass |
| Regression tests | All pass | All pass | All pass |
| FPS (1080p) | 60+ | 100-200 | 200-400 |
| Frame time | <16.67ms | 5-10ms | 2.5-5ms |
| CPU iteration | >5 M/s | 10-20 M/s | 20-40 M/s |
| Variations | >15 M/s | 25-35 M/s | 35-60 M/s |

### WASM (Release Build)

| Test | Target | Good | Excellent |
|------|--------|------|-----------|
| FPS (1080p) | 30+ | 45-55 | 55-60 |
| FPS (720p) | 60 | 60 | 60+ |
| Frame time | <33ms | 16-20ms | <16.67ms |

---

## Continuous Integration

### Recommended CI Pipeline

```yaml
# .github/workflows/test.yml
name: Tests

on: [push, pull_request]

jobs:
  test:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v2
      - uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      - name: Run tests
        run: cargo test --release
      - name: Run regression tests
        run: cargo test --release --test regression
      - name: Run benchmarks (no comparison)
        run: cargo bench --no-run
```

---

## Troubleshooting

### Tests Fail

```bash
# Run with backtrace
RUST_BACKTRACE=1 cargo test

# Run specific failing test
cargo test test_name -- --nocapture

# Check test in isolation
cargo test --test regression test_affine_identity -- --exact
```

### Benchmarks Unstable

```bash
# Increase sample size
cargo bench -- --sample-size 1000

# Increase warmup
cargo bench -- --warm-up-time 5

# Close other applications
# Disable CPU frequency scaling
# Use performance power profile
```

### Can't Build Benchmarks

```bash
# Check Criterion is in dev-dependencies
grep criterion Cargo.toml

# Try clean build
cargo clean
cargo bench
```

---

## Summary: Testing Checklist

Before committing code:
- [ ] `cargo test` - Unit tests pass
- [ ] `cargo test --test regression` - Regression tests pass
- [ ] `cargo clippy` - No warnings
- [ ] `cargo run --release` - App works
- [ ] `cargo bench` - No major performance regression

Before releasing:
- [ ] All tests pass
- [ ] Benchmarks meet targets
- [ ] Manual testing on all platforms
- [ ] Version incremented
- [ ] `cargo run --example show_version` - Note build number
- [ ] WASM build tested

---

## Files Reference

| Type | Location | Command |
|------|----------|---------|
| Unit tests | `src/**/*.rs` (bottom of files) | `cargo test` |
| Regression | `tests/regression.rs` | `cargo test --test regression` |
| Benchmarks | `benches/flame_bench.rs` | `cargo bench` |
| Simple bench | `src/bin/simple_benchmark.rs` | `cargo run --bin simple_benchmark` |
| Profiler | `src/profiler.rs` | (used in code) |
| Metrics | `src/util.rs` | (automatic in app) |
| Version | `examples/show_version.rs` | `cargo run --example show_version` |

---

**Last Updated:** 2025-10-21
**Current Build:** #9
**Test Coverage:** Unit tests ✅, Regression ✅, Benchmarks ✅, Profiling ✅
