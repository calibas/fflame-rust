# Animation Export Race Conditions - Fix

**Branch:** `fix/animation-export-race-conditions`
**Created:** 2025-12-31
**Status:** Implementation Complete (Ready for Testing)

## Problem Statement

Animation export randomly freezes with "Render error: Other" messages flooding the console. The freeze is timing-dependent and happens during video export operations. Investigation revealed multiple race conditions in the export pipeline.

## Root Causes

### 1. Primary Issue: Tight Polling Loop (Spin-Loop Anti-Pattern)

**Location:** `src/animation/export.rs:1304-1313`

The buffer mapping code uses a tight spin-loop that consumes 100% CPU while waiting for GPU:

```rust
// Poll until mapped
loop {
    let _ = device.poll(PollType::Poll);  // ⚠️ TIGHT LOOP - NO SLEEP/YIELD
    match rx.try_recv() {
        Ok(Some(Ok(()))) => break,
        Ok(Some(Err(e))) => {
            return Err(AnimationExportError::GpuError(format!("Buffer map error: {:?}", e)));
        }
        _ => {}
    }
}
```

**Problems:**
- CPU starvation (100% of one core)
- GPU driver contention from rapid polling
- No yield to other threads (FFmpeg writer gets starved)
- Platform-dependent behavior (different drivers react differently)

**Comparison:** Every other location in the codebase (9+ places) uses proper blocking:
```rust
let _ = device.poll(PollType::Wait { submission_index: None, timeout: None });
rx.await  // Proper async await
```

### 2. Secondary Issue: Small Channel Buffer

**Location:** `src/animation/export.rs:1150`

```rust
let (frame_tx, frame_rx) = mpsc::sync_channel::<Vec<u8>>(4);  // Only 4 frames buffered
```

**Potential Deadlock Scenario:**
1. Export thread renders frame 5
2. Tries to send to channel (blocks if 4 frames queued)
3. Export thread enters tight polling loop for frame 6
4. FFmpeg writer thread gets CPU-starved by spin-loop
5. Writer can't drain channel → export thread blocks forever

### 3. GPU Contention Between Export and Main App

**Location:** `src/app/mod.rs:1832-1840`

Even though export uses a separate GPU device, the main app continues rendering at 60 FPS:

```rust
// Skip GPU work during video export to avoid GPU contention (separate device in background thread)
let is_video_exporting = self.animation_export_progress.lock()
    .map(|p| p.is_exporting)
    .unwrap_or(false);
```

Main app skips *fractal* GPU work but still renders UI, which can cause:
- Surface acquisition failures (`SurfaceError::Other`)
- GPU driver overwhelm from combined load
- System-wide GPU stress

### 4. No Timeout Detection

If GPU hangs or becomes unresponsive, the tight loop runs forever with no escape hatch.

## Proposed Solutions

### Fix 1: Replace Spin-Loop with Proper Async/Await ✅

**File:** `src/animation/export.rs:1296-1314`

**Before:**
```rust
// Poll until mapped
loop {
    let _ = device.poll(PollType::Poll);
    match rx.try_recv() {
        Ok(Some(Ok(()))) => break,
        Ok(Some(Err(e))) => {
            return Err(AnimationExportError::GpuError(format!("Buffer map error: {:?}", e)));
        }
        _ => {}
    }
}
```

**After:**
```rust
// Wait for mapping to complete (proper blocking)
let _ = device.poll(PollType::Wait {
    submission_index: None,
    timeout: Some(std::time::Duration::from_secs(30))  // 30s timeout
});

rx.await
    .map_err(|_| AnimationExportError::GpuError("Failed to receive map result".to_string()))?
    .map_err(|e| AnimationExportError::GpuError(format!("Buffer map error: {:?}", e)))?;
```

**Benefits:**
- CPU yields to other threads (no spin-loop)
- GPU driver can process commands normally
- FFmpeg writer thread gets CPU time
- 30-second timeout prevents infinite hangs

### Fix 2: Increase Channel Buffer Size ✅

**File:** `src/animation/export.rs:1150`

**Before:**
```rust
let (frame_tx, frame_rx) = mpsc::sync_channel::<Vec<u8>>(4);
```

**After:**
```rust
let (frame_tx, frame_rx) = mpsc::sync_channel::<Vec<u8>>(16);  // Increased from 4 to 16
```

**Rationale:**
- Each frame is ~800×600×4 = 1.92 MB (typical size)
- 16 frames = ~31 MB memory (acceptable overhead)
- Reduces blocking when FFmpeg writer experiences temporary slowdown
- Still bounded (not unbounded channel) to prevent memory explosion

### Fix 3: Pause Main App Rendering During Export ✅

**File:** `src/app/mod.rs:454-503` (render function)

Add early return when video export is active:

```rust
fn render(&mut self, window: &Window) -> Result<(), SurfaceError> {
    // Skip rendering if window is minimized
    if self.gpu.size.width == 0 || self.gpu.size.height == 0 {
        return Ok(());
    }

    // NEW: Skip ALL rendering during video export to reduce GPU contention
    let is_video_exporting = self.animation_export_progress.lock()
        .map(|p| p.is_exporting)
        .unwrap_or(false);
    if is_video_exporting {
        return Ok(());  // Skip frame entirely
    }

    // ... rest of render function
}
```

**Benefits:**
- Eliminates GPU contention from UI rendering
- Prevents `SurfaceError::Other` spam
- Export gets full GPU bandwidth
- UI still receives input events (can cancel export)

**Tradeoff:**
- UI freezes during export (expected behavior for long operations)
- Export progress still updates via shared `Arc<Mutex<ExportProgress>>`
- User can still cancel via UI (event handling continues)

### Fix 4: Add Comprehensive Error Context ✅

**File:** `src/animation/export.rs:1296-1320`

Improve error messages for debugging:

```rust
// Wait for mapping to complete
let _ = device.poll(PollType::Wait {
    submission_index: None,
    timeout: Some(std::time::Duration::from_secs(30))
});

rx.await
    .map_err(|_| AnimationExportError::GpuError(
        format!("GPU mapping timed out or channel closed (frame {})", frame)
    ))?
    .map_err(|e| AnimationExportError::GpuError(
        format!("Buffer map error on frame {}: {:?}", frame, e)
    ))?;
```

## Testing Plan

### Unit Tests
- ❌ Not applicable (async GPU operations, hard to mock)

### Manual Testing
1. **Basic export** - Single frame animation (verify no regression)
2. **Long export** - 300+ frames at 1080p (verify no freeze)
3. **High system load** - Export while running CPU/GPU intensive tasks
4. **Cancellation** - Cancel mid-export (verify clean shutdown)
5. **Multiple exports** - Back-to-back exports without restart
6. **UI interaction** - Verify UI remains responsive (or properly frozen)

### Success Criteria
- ✅ No "Render error: Other" messages during export
- ✅ Export completes without freezing
- ✅ CPU usage reasonable during export (not 100% spin-loop)
- ✅ FFmpeg writer thread gets CPU time
- ✅ Clean error messages if GPU hangs (timeout triggers)

## Implementation Checklist

- [x] Fix 1: Replace spin-loop with async/await + timeout
- [x] Fix 2: Increase channel buffer from 4 to 16
- [x] Fix 3: Pause main app rendering during export
- [x] Fix 4: Add frame-specific error context
- [ ] Manual testing: Basic export
- [ ] Manual testing: Long export (300+ frames)
- [ ] Manual testing: High system load
- [ ] Manual testing: Cancellation
- [ ] Manual testing: UI interaction
- [ ] Update docs if needed
- [ ] Commit and create PR

## Code Locations

### Files to Modify
1. `src/animation/export.rs` - Primary fixes (spin-loop, channel size, errors)
2. `src/app/mod.rs` - Pause main app rendering during export

### Reference Files (Working Examples)
- `src/export/high_res.rs:1153` - Proper `PollType::Wait` usage
- `src/export/renderer.rs:1129` - Proper `PollType::Wait` usage
- `src/renderer/compute_kernel.rs:1305` - Proper `PollType::Wait` usage
- `src/app/mod.rs:1424` - Proper `PollType::Wait` usage

## Notes

### Why This Wasn't Caught Earlier

The race condition is **timing-dependent**:
- **GPU load**: Faster GPUs complete work before spin-loop causes issues
- **System load**: Low-load systems have spare CPU for spin-loop
- **Driver differences**: Some GPU drivers tolerate rapid polling better
- **Frame complexity**: Simple flames render faster, less time in polling loop
- **OS scheduler**: Different schedulers handle spin-loops differently

The issue becomes **deterministic** under:
- High GPU load (complex fractals)
- High CPU load (other processes)
- Longer exports (300+ frames)
- Slower GPUs (integrated graphics)

### Future Improvements (Out of Scope)

1. **Async/await throughout export pipeline** - Full async refactor
2. **Progress streaming** - Update UI during tight loops
3. **GPU memory pooling** - Reuse staging buffers across frames
4. **Multi-threaded FFmpeg encoding** - Overlap encoding with rendering
5. **Export queue system** - Queue multiple exports

## References

- [wgpu PollType documentation](https://docs.rs/wgpu/latest/wgpu/enum.PollType.html)
- [Rust async book: CPU-bound futures](https://rust-lang.github.io/async-book/08_ecosystem/00_chapter.html)
- [mpsc::sync_channel docs](https://doc.rust-lang.org/std/sync/mpsc/fn.sync_channel.html)
