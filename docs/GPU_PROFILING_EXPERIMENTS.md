# GPU Profiling Experiments

## Complete List of GPU Operations Per Frame

**When IDLE (rendering_complete=true):**
1. **get_current_texture()** - Acquire swapchain image from surface
2. **create_view()** - Create texture view for rendering target
3. **create_command_encoder()** - Create UI encoder
4. **egui_render** - UI rendering pass ✅ PROFILED (~120µs)
5. **resolve_queries()** - Resolve UI timestamp queries
6. **queue.submit()** - Submit UI commands to GPU
7. **create_command_encoder()** - Create fractal encoder
8. **fractal_tonemap** - Tonemap rendering pass ✅ PROFILED (~96µs)
9. **resolve_queries()** - Resolve fractal timestamp queries
10. **queue.submit()** - Submit fractal commands to GPU
11. **end_frame()** - End profiler frame (bookkeeping)
12. **frame.present()** - Present to screen ❌ NOT PROFILED

**Total Measured GPU Time: ~0.22ms (216µs)**
**Total Frame Time: ~16.67ms (60 FPS)**
**Measured GPU %: 1.3% of frame time**

**When RENDERING (rendering_complete=false):**
- All of the above PLUS:
- **fractal_compute** - Compute shader (128 workgroups × iterations)
- **fractal_accumulate** - Accumulation pass (ping-pong blending)
- **update_encoder** - Config change GPU uploads (when needed)

## What's NOT Measured by wgpu-profiler

1. **frame.present()** - This is a CPU→driver call that triggers:
   - GPU composition (blending layers, applying effects)
   - VSync timing and synchronization
   - Display engine work
   - Desktop Window Manager (DWM) on Windows
   - Driver overhead for presentation

2. **Encoder creation** - Small CPU overhead, happens 2-4x per frame

3. **Texture acquisition** - get_current_texture() waits for available image

4. **Driver overhead** - Command buffer processing, state tracking

## Hypothesis: frame.present() is the Culprit

**Theory:**
`frame.present()` calls into the graphics driver which:
1. Waits for VSync (up to 16.67ms)
2. Performs composition and color conversion
3. Interacts with Windows DWM compositor
4. May keep GPU in active state even though our rendering is done

This work is invisible to wgpu-profiler because it happens in the driver/compositor,
not in our GPU command buffers.

## Experiments to Run

### Experiment 1: Skip frame.present()
```bash
# Set environment variable and run
SKIP_PRESENT=1 cargo run --release
```
**Expected Result:** If frame.present() causes high GPU usage, skipping it should show lower GPU usage (but blank screen).

### Experiment 2: Reduce Frame Rate When Idle
Modify frame timing to only present at 30 FPS when idle instead of 60 FPS.

**Expected Result:** If 60 Hz wake-ups cause high usage, 30 Hz should reduce it by ~50%.

### Experiment 3: Skip All Rendering When Idle
Skip both egui and tonemap when idle - only present previous frame.

**Expected Result:** Should show minimal difference since those passes only take 0.22ms.

### Experiment 4: Check macOS vs Windows
Test on macOS to see if Windows-specific (DWM compositor) is the issue.

## wgpu Architecture and OS Integration

**How wgpu interacts with OS:**
1. wgpu is a high-level API that translates to native graphics APIs:
   - Windows: DirectX 12 (primary) or Vulkan
   - macOS: Metal
   - Linux: Vulkan

2. **Present Mode: Fifo (VSync)**
   - We use `PresentMode::Fifo` on macOS and Windows (line 165-169 of device.rs)
   - This forces GPU to sync with display refresh (60 Hz)
   - GPU must wake up 60x/second even for minimal work
   - Each wake-up has overhead (power state transitions)

3. **Windows-Specific Overhead:**
   - Desktop Window Manager (DWM) composites all windows
   - Even our simple present() goes through DWM composition
   - DWM may do additional GPU work (effects, blending, scaling)
   - This is OUTSIDE our wgpu command buffers

4. **Double/Triple Buffering:**
   - Swapchain has multiple buffers (typ. 2-3)
   - `get_current_texture()` may wait for available buffer
   - This waiting time may show as "GPU usage"

## Potential wgpu-Specific Issues

1. **Command Buffer Overhead:**
   - We create 2 encoders per frame minimum
   - Each submit() has driver overhead
   - Could potentially batch into single encoder

2. **Query Pool Management:**
   - wgpu-profiler creates query pools
   - Resolving queries has overhead
   - May keep GPU busy longer than necessary

3. **State Tracking:**
   - wgpu tracks pipeline state, bindings, etc.
   - On each frame, state is validated and applied
   - Driver may do background compilation/optimization

## Recommendations for Further Investigation

1. **Try alternative present modes** (if supported):
   - `PresentMode::Immediate` - No VSync, lower latency
   - `PresentMode::Mailbox` - VSync but no blocking

2. **Batch encoders:**
   - Combine UI and fractal into single encoder
   - Reduce submit() calls from 2 to 1

3. **Conditional profiling:**
   - Only enable profiler when debugging
   - Check if profiler overhead contributes to usage

4. **External GPU profilers:**
   - Use vendor tools (Nsight for NVIDIA, PIX for AMD/Intel)
   - Can see driver and compositor work

5. **Accept it as normal:**
   - 60 FPS UI apps typically show some GPU usage
   - Modern OSes do continuous composition
   - As long as actual work is minimal (~0.22ms), it's fine

## Comprehensive Profiling Added (2025-11-22)

**CPU-side timing now measured for ALL operations:**
- `get_current_texture()` - Swapchain acquisition (may block waiting for VSync)
- `create_view()` - Texture view creation
- `create_command_encoder()` - Encoder allocation
- UI render (CPU time)
- `queue.submit()` - Command buffer submission
- `frame.present()` - **KEY SUSPECT** - triggers VSync, composition, DWM

**What to look for when testing:**
1. **present() time** - If this is high when idle, it's blocking/waiting
2. **get_current_texture() time** - May block if swapchain is busy
3. **Total CPU time vs GPU time** - Where is the time going?

**Example output when idle:**
```
=== GPU Profiling (IDLE) ===
GPU timings:
  GPU[0] egui_render: 0.120ms (120.4µs)
  GPU[1] fractal_tonemap: 0.096ms (95.8µs)
  GPU TOTAL: 0.216ms

CPU timings:
  get_current_texture: ???ms  ← May reveal VSync blocking
  create_view: ???ms
  create_encoder: ???ms
  ui_render (CPU): ???ms
  submit (UI): ???ms
  present: ???ms  ← KEY METRIC - suspected culprit
  TOTAL frame: ???ms
```

**Hypothesis:**
If `present()` time is high (e.g., 10-15ms when idle), that's where the mystery "GPU usage" comes from - it's not actual rendering work, but VSync/composition blocking.

## Actual Test Results (2025-11-22)

**Measured when idle with UI active (60 FPS mode):**
```
GPU work: 0.141ms (egui_render + fractal_tonemap)
CPU work: ~1.8ms total (all operations)
Frame interval: 17.25ms (~58 FPS)
MYSTERY GAP: 15.5ms unaccounted for!
```

**Analysis:**
- `present()` time: 0.142ms (NOT blocking - hypothesis disproven!)
- `get_current_texture()`: 0.029ms (NOT blocking)
- Total measured work: 1.8ms
- **87% of frame time is unexplained** (15.5ms / 17.25ms)

**New Theory:**
The 15.5ms gap is the GPU/driver **waiting for VSync** between frames. Even though `present()` returns quickly (0.142ms), the GPU is kept awake in a "ready" state for the full 16.67ms frame period. This "time awake" shows as "GPU usage" even though no work is happening.

**Windows is using PresentMode::Mailbox** (non-blocking VSync), but the GPU still waits for display refresh.

## New Experiment: Test Without VSync

Added `PRESENT_MODE` environment variable to test different modes:

```bash
# Test without VSync (should eliminate the 15.5ms gap)
PRESENT_MODE=immediate cargo run --release

# Test with blocking VSync
PRESENT_MODE=fifo cargo run --release

# Default (non-blocking VSync)
PRESENT_MODE=mailbox cargo run --release
```

**Expected Result:**
With `PRESENT_MODE=immediate`, frame interval should drop to ~2ms (actual work time), and GPU usage should be minimal.

## Conclusion

The wgpu-profiler confirms our rendering is extremely efficient (~0.22ms per frame, 1.3% of frame time).
The high GPU "usage" percentage is likely from:
- VSync synchronization overhead
- Windows DWM composition
- Driver/OS-level work invisible to profiler
- How GPU utilization % is calculated

**Next step:** Run with profiling and check if `present()` time reveals the mystery.
