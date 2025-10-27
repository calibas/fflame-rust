# Convergence Diagnostic Tests - Implementation Guide

**Date:** 2025-10-27
**Purpose:** Verify hypothesis that accumulation-time compression fails due to convergence

## Hypothesis

Density compression has no visible effect because:
1. Progressive accumulation uses exponential moving average (blend_factor decreases over time)
2. By the time pixels are bright enough for compression to matter, blend_factor is already imperceptibly small
3. Compressing an imperceptible value (0.001) to an even smaller value (0.00001) produces no visible change

## Test Suite

### Test C: Non-Converging Accumulation (EASIEST TO IMPLEMENT)

**Goal:** Prove compression works when blend_factor is large

**Implementation:**
```wgsl
// In accumulate.wgsl, line ~67
// BEFORE:
let adjusted_blend = params.blend_factor * density_factor;

// AFTER (diagnostic):
let adjusted_blend = 1.0 * density_factor;  // Force 100% blend (no convergence)
```

**Expected Result:**
- At strength=0: Normal fractal
- At strength=50: Should see SIGNIFICANT darkening in bright areas
- At strength=100: Extreme effect, bright areas nearly frozen

**Why this proves the hypothesis:**
If compression is visible with blend=1.0 but invisible with normal convergence,
it confirms that convergence (tiny blend_factor) is the root cause.

**Rebuild:** `cargo build --release`

---

### Test D: Blend Factor Visualization (MOST DIAGNOSTIC)

**Goal:** Visualize how small adjusted_blend actually is in bright areas

**Implementation:**
```wgsl
// In accumulate.wgsl, replace the RGB blend line
// BEFORE (line ~68):
rgb_accumulated = prev.rgb * (1.0 - adjusted_blend) + new_color * adjusted_blend;

// AFTER (diagnostic):
let compression_factor = 1.0 / (1.0 + prev.a * prev.a * params.density_compression_strength);
let compressed_blend = adjusted_blend * compression_factor;

// Visualize: white=large blend, black=tiny blend
// Amplify by 1000x so we can see values in 0.001 range
rgb_accumulated = vec3<f32>(compressed_blend * 1000.0);
alpha_accumulated = prev.a;  // Keep density for tonemap
```

**Expected Result:**
- Entire image will be mostly BLACK (blend values < 0.001)
- Only sparse/new pixels will show any brightness
- Confirms: blend_factor is imperceptibly small in converged areas

**Rebuild:** `cargo build --release`

---

### Test A: Early-Frame Compression (REQUIRES FRAME COUNTER)

**Goal:** Show compression works when blend_factor is still significant

**Implementation:**
```wgsl
// In accumulate.wgsl, need to add frame counter to params struct first
struct AccumulateParams {
    // ... existing fields ...
    frame_counter: u32,  // ADD THIS
}

// Then modify blend calculation:
let compression_factor = 1.0 / (1.0 + prev.a * prev.a * params.density_compression_strength);

// Only apply compression in early frames (when blend is still large)
var final_compression = 1.0;
if (params.frame_counter < 20u) {
    final_compression = compression_factor;
}

let adjusted_blend = params.blend_factor * density_factor * final_compression;
```

**Expected Result:**
- At strength=100: First 20 frames show compression effect
- After frame 20: Effect disappears (confirming it only works when blend is large)

**Note:** Requires passing frame_counter from FlameRenderer to GPU

---

### Test E: Convergence Point Detection

**Goal:** Visualize when pixels cross the "converged" threshold

**Implementation:**
```wgsl
// Color pixels RED when they become imperceptibly small
let blend_threshold = 0.001;  // Converged when blend < 0.1%

if (adjusted_blend < blend_threshold) {
    rgb_accumulated = vec3<f32>(1.0, 0.0, 0.0);  // Red = converged
} else {
    rgb_accumulated = prev.rgb * (1.0 - adjusted_blend) + new_color * adjusted_blend;
}
```

**Expected Result:**
- Entire image turns red within 100-200 frames
- Shows how quickly pixels reach "imperceptible blend" state

---

## Recommended Test Order

1. **Start with Test C** (easiest, most dramatic proof)
   - If compression is visible with blend=1.0, hypothesis is confirmed

2. **Then Test D** (visual proof of tiny blend values)
   - Shows actual blend magnitudes

3. **Then Test A** (if curious about timing)
   - Shows compression works early, fails late

4. **Skip Test E** unless needed for documentation

---

## Success Criteria

If hypothesis is correct:
- ✅ Test C shows dramatic compression effect
- ✅ Test D shows blend values near zero in bright areas
- ✅ Test A shows compression works early, fails late

This would conclusively prove: **Compression fails because it's fighting convergence, not because of implementation bugs.**

---

## After Testing

1. Document results in DENSITY_AWARE_COLOR_POC_RESULTS.md
2. Revert all diagnostic changes
3. Remove non-functional `density_compression_strength` infrastructure
4. Update recommendations based on findings
