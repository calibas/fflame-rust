# WASM Profiling Guide

## Overview

This guide covers performance profiling for the fractal flame renderer running in WebAssembly/browser environments.

## Profiling Methods

### 1. CPU Timing (Recommended for WASM) ✅

**Best for:** Real-time performance monitoring, synchronous profiling

The `PerformanceMetrics` struct uses `web_time::Instant` which works identically on desktop and WASM:

```rust
use fractal_flame_wgpu::util::PerformanceMetrics;

let mut metrics = PerformanceMetrics::new();

// ... render frames ...
metrics.update();

// Export to browser console
metrics.export_to_console();

// Or log to console
metrics.log_snapshot();
```

**Browser Console Output:**
```
Performance Snapshot:
{
  "version": "0.1.0 (build #7)",
  "build_number": 7,
  "git_hash": "dba27e8",
  "fps": 60.5,
  "frame_time_ms": 16.52,
  "frame_count": 1000,
  "compute_time_ms": 12.3,
  "accumulate_time_ms": 1.5,
  "tonemap_time_ms": 0.4,
  "ui_time_ms": 1.2,
  "timestamp": "2025-10-21T23:45:00Z"
}
```

### 2. Browser DevTools Performance Tab ✅

**Best for:** Detailed flame graphs, GPU timeline, memory profiling

#### Chrome/Edge DevTools

1. Open DevTools (F12)
2. Go to **Performance** tab
3. Click **Record** 🔴
4. Interact with the fractal flame renderer
5. Click **Stop**
6. Analyze timeline:
   - **Main Thread**: JavaScript execution, WASM calls
   - **GPU**: WebGPU command submission
   - **Frame Timeline**: 60fps target line

**What to look for:**
- Long frames (>16.67ms for 60fps)
- GPU stalls (red bars in GPU timeline)
- JavaScript overhead
- Memory allocation spikes

#### Firefox DevTools

1. Open DevTools (F12)
2. Go to **Performance** tab
3. Click **Start Recording**
4. Render some frames
5. Click **Stop Recording**

**Features:**
- Flame graph visualization
- Call tree analysis
- GPU profiling (requires WebGPU support)

### 3. JavaScript Performance API ✅

**Best for:** Custom timing instrumentation

Add manual timing marks in JavaScript:

```javascript
// In your index.html or main.js
performance.mark('fractal-start');

// ... WASM rendering ...

performance.mark('fractal-end');
performance.measure('fractal-render', 'fractal-start', 'fractal-end');

// Get measurements
const measures = performance.getEntriesByType('measure');
console.log('Render time:', measures[0].duration, 'ms');
```

### 4. GPU Profiling (Async) ⚠️

**Best for:** Detailed GPU pass timing (requires async handling)

GPU timestamp queries work on WASM but require asynchronous context:

```rust
use fractal_flame_wgpu::profiler::GpuProfiler;

let profiler = GpuProfiler::new(&device);

// In render loop
profiler.begin_scope(&mut encoder, 0);
// ... GPU work ...
profiler.end_scope(&mut encoder, 0);
profiler.resolve(&mut encoder);

// MUST be in async context for WASM
async fn read_gpu_timings(profiler: &GpuProfiler, queue: &Queue) {
    if let Some(timestamps) = profiler.read_timestamps(queue).await {
        let period = queue.get_timestamp_period();
        let duration_ms = GpuProfiler::calculate_duration(&timestamps, 0, period);
        log::info!("GPU pass: {:.2}ms", duration_ms);
    }
}
```

**Limitations:**
- Requires async/.await (not easy from sync render loop)
- Not all browsers support TIMESTAMP_QUERY
- Use CPU timing instead for simplicity

---

## Practical WASM Profiling Workflow

### Quick Performance Check

Add to your WASM app initialization:

```rust
// Log version on startup
let version = fractal_flame_wgpu::version::get_version_info();
log::info!("Fractal Flame Renderer v{}", version.full_version());
log::info!("Platform: {} {}", version.platform(), version.architecture());

// Periodic performance logging
let mut frame_count = 0;
let mut metrics = PerformanceMetrics::new();

// In render loop
metrics.update();
frame_count += 1;

if frame_count % 60 == 0 {
    metrics.export_to_console();
}
```

### Export Performance Data

**Method 1: Browser Console**
```rust
// Exports JSON to browser console
metrics.export_to_console();
```

Then in browser console:
```javascript
// Copy the JSON output, or:
copy(/* paste JSON here */);
```

**Method 2: Download as File**

Add to your HTML:
```html
<button onclick="downloadPerfData()">Download Performance Data</button>

<script>
function downloadPerfData() {
    // Get from WASM (you'd expose this via wasm-bindgen)
    const perfData = window.getPerfSnapshot();
    const blob = new Blob([JSON.stringify(perfData, null, 2)],
                          { type: 'application/json' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = 'fractal-perf-' + Date.now() + '.json';
    a.click();
}
</script>
```

**Method 3: Local Storage**
```javascript
// Save to localStorage
localStorage.setItem('fractal-perf', JSON.stringify(perfData));

// Retrieve later
const savedPerf = JSON.parse(localStorage.getItem('fractal-perf'));
console.table(savedPerf);
```

---

## Performance Targets (WASM)

### Expected Performance

| Browser | GPU | Resolution | Target FPS | Typical FPS |
|---------|-----|------------|------------|-------------|
| Chrome 113+ | Modern | 1080p | 60 | 50-60 |
| Chrome 113+ | Modern | 720p | 60 | 60+ |
| Firefox 121+ | Modern | 1080p | 60 | 40-60 |
| Safari 18+ | M1/M2 | 1080p | 60 | 60+ |

**"Modern GPU"** = RTX 2060+, RX 5700+, Intel Xe, Apple M1+

### Frame Budget Breakdown (60fps = 16.67ms)

```
WASM Frame (target 16.67ms):
├─ JavaScript overhead: ~0.5-1ms (3-6%)
├─ WASM compute shader: ~10-12ms (60-70%)
├─ Accumulate pass: ~1-2ms (6-12%)
├─ Tonemap pass: ~0.3-0.5ms (2-3%)
├─ UI (egui): ~1-2ms (6-12%)
└─ Browser compositing: ~1-2ms (6-12%)
```

**Compared to Desktop:**
- WASM adds ~10-20% overhead (JavaScript interop, bounds checking)
- Still hits 60fps on modern hardware
- Mobile WebGPU: Expect 30-45fps on high-end phones

---

## Debugging Performance Issues

### Low FPS (<30fps)

**Check 1: Browser Support**
```javascript
// Check WebGPU availability
if (!navigator.gpu) {
    console.error('WebGPU not supported!');
}
```

**Check 2: Hardware Acceleration**
```
chrome://gpu
```
Ensure WebGPU is enabled and using hardware acceleration.

**Check 3: Resolution**
WASM canvas might be rendering at wrong resolution:
```javascript
const dpr = window.devicePixelRatio;
console.log('Device Pixel Ratio:', dpr);
console.log('Canvas size:', canvas.width, 'x', canvas.height);
```

**Check 4: Iteration Count**
Too many iterations per frame:
```rust
// Reduce for WASM
#[cfg(target_arch = "wasm32")]
const DEFAULT_ITERATIONS: u32 = 128;

#[cfg(not(target_arch = "wasm32"))]
const DEFAULT_ITERATIONS: u32 = 256;
```

### Memory Leaks

**Symptom:** FPS degrades over time

**Check 1: Browser Memory Tab**
DevTools → Memory → Take heap snapshot

**Check 2: Look for GPU resource leaks**
```javascript
// In browser console
performance.memory.usedJSHeapSize / 1024 / 1024 + ' MB'
```

**Check 3: WASM Linear Memory**
```javascript
// Check WASM memory growth
const wasmMemory = Module.memory; // Adjust based on your WASM export
console.log('WASM pages:', wasmMemory.buffer.byteLength / 65536);
```

**Fix:** Ensure textures and buffers are properly destroyed on resize/reset.

### Stuttering / Jank

**Cause:** Browser event loop blocking

**Solution:** Reduce workload per frame
```rust
// Split work across multiple frames
let iterations_per_frame = 64; // Lower for WASM
```

**Check for long JavaScript tasks:**
DevTools → Performance → Look for tasks >50ms

---

## Comparing WASM vs Desktop

### Benchmark Setup

**Desktop:**
```bash
cargo run --release --bin simple_benchmark
```

**WASM:**
1. Build WASM: `./build-wasm.bat`
2. Serve: `python -m http.server 8080`
3. Open browser console
4. Run: `metrics.export_to_console()`

### Metrics to Compare

| Metric | Desktop | WASM | Ratio |
|--------|---------|------|-------|
| FPS (1080p) | 200-400 | 50-60 | 3-7x faster |
| Frame time | 2.5-5ms | 16-20ms | 3-4x slower |
| Compute pass | 2ms | 10-12ms | 5-6x slower |
| Accumulate | 0.5ms | 1-2ms | 2-4x slower |
| Tonemap | 0.2ms | 0.3-0.5ms | 1.5-2x slower |

**Why slower?**
- JavaScript interop overhead
- WASM bounds checking
- Browser sandbox restrictions
- Shared WebGPU queue

**Still acceptable** because we hit 60fps target!

---

## Advanced: Custom Performance Hooks

### Expose Metrics to JavaScript

In `src/lib.rs`:
```rust
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn get_performance_snapshot() -> JsValue {
    // Access your metrics somehow
    let snapshot = /* ... */;
    serde_wasm_bindgen::to_value(&snapshot).unwrap()
}
```

In JavaScript:
```javascript
import init, { get_performance_snapshot } from './pkg/fractal_flame_wgpu.js';

await init();

setInterval(() => {
    const perf = get_performance_snapshot();
    console.log('FPS:', perf.fps);
    updatePerfUI(perf);
}, 1000);
```

### Performance Overlay

Add live FPS counter to your HTML:
```html
<div id="perf-overlay" style="position: absolute; top: 10px; left: 10px;
     background: rgba(0,0,0,0.7); color: white; padding: 10px;
     font-family: monospace; font-size: 14px;">
    FPS: <span id="fps">--</span><br>
    Frame: <span id="frame-time">--</span>ms<br>
    Samples: <span id="samples">--</span>
</div>
```

Update from WASM:
```rust
#[cfg(target_arch = "wasm32")]
pub fn update_perf_overlay(metrics: &PerformanceMetrics) {
    use wasm_bindgen::JsCast;

    let window = web_sys::window().unwrap();
    let document = window.document().unwrap();

    if let Some(fps_el) = document.get_element_by_id("fps") {
        fps_el.set_inner_html(&format!("{:.1}", metrics.fps()));
    }
    if let Some(ft_el) = document.get_element_by_id("frame-time") {
        ft_el.set_inner_html(&format!("{:.2}", metrics.frame_time_ms()));
    }
}
```

---

## Tools & Resources

### Browser Extensions
- **Chrome WebGPU Inspector** - Inspect WebGPU state
- **Firefox WebGL Inspector** - Fallback for WebGL2

### Online Tools
- [WebGPU Samples](https://webgpu.github.io/webgpu-samples/) - Performance comparisons
- [GPU.rocks](https://gpu.rocks/) - WebGPU benchmarks

### Monitoring Services
- [Sentry](https://sentry.io/) - Real-user monitoring (RUM)
- [LogRocket](https://logrocket.com/) - Session replay with perf data

---

## Summary

### WASM Profiling Checklist

- ✅ Use `PerformanceMetrics` for CPU timing (synchronous, easy)
- ✅ Use Browser DevTools for deep analysis (flame graphs, GPU)
- ✅ Export snapshots to console with version info
- ✅ Monitor FPS/frame time in real-time
- ⚠️ Avoid GPU timestamp queries (async complexity)
- ✅ Compare WASM vs desktop performance
- ✅ Set realistic targets (60fps @ 720p, 30-60fps @ 1080p)

### Key Differences from Desktop

| Feature | Desktop | WASM |
|---------|---------|------|
| GPU Profiling | ✅ Easy (sync) | ⚠️ Complex (async) |
| CPU Timing | ✅ `std::time` | ✅ `web_time` |
| Export | File system | Console/LocalStorage |
| Overhead | Minimal | ~10-20% |
| Tools | RenderDoc, Nsight | DevTools |

**Recommendation:** Use CPU timing (`PerformanceMetrics`) for WASM. It's accurate enough and much simpler than async GPU profiling.

---

**Last Updated:** 2025-10-21
**Project:** fflame-rust
**WASM Build:** #7
