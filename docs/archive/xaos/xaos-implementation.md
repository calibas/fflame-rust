# Xaos (Weighted Transform Transitions) Implementation

## Overview

Xaos allows asymmetric weighting of transitions between transforms in the chaos game iteration. Instead of selecting the next transform based solely on its density weight, Xaos adds per-source-transform modifiers that control how likely each destination transform is to be selected.

**Key Formula:** `probability[src→dst] = transform[dst].weight × xaos[src][dst]`

When all xaos weights are 1.0 (default), the renderer behaves identically to classic chaos game with no performance overhead.

## Design Goals

1. **Zero overhead when disabled**: Flames without xaos should have identical performance to current implementation
2. **Minimal memory impact**: Only allocate xaos data when actively used
3. **Conditional shader compilation**: Generate xaos-aware shaders only when needed
4. **Full Apophysis compatibility**: Import/export chaos attributes from .flame XML

## Architecture

### Data Storage

#### Option A: Per-Flame Xaos Matrix (Recommended)

Store xaos as a separate N×N matrix at the Flame level:

```rust
// In src/scene/transforms.rs
pub struct Flame {
    pub transforms: Vec<Transform>,
    pub final_transform: Option<Transform>,
    // ... existing fields ...

    /// Xaos transition weights: xaos[src][dst] = modifier for src→dst transition
    /// None when all weights are 1.0 (default behavior, no memory allocated)
    /// When Some, outer Vec has len = transforms.len(), inner Vec has len = transforms.len()
    pub xaos: Option<Vec<Vec<f32>>>,
}
```

**Pros:**
- Clear separation of concerns (xaos is a flame property, not transform property)
- Easy to detect "is xaos active?" → `flame.xaos.is_some()`
- Memory only allocated when needed
- Matches conceptual model (N×N matrix)

**Cons:**
- Must keep in sync when transforms are added/deleted

#### Option B: Per-Transform Xaos Array

Store xaos weights within each Transform (like Apophysis):

```rust
// In src/scene/transforms.rs
pub struct Transform {
    // ... existing fields ...

    /// Xaos weights for transitions FROM this transform TO each destination
    /// None when all weights are 1.0 (default)
    pub xaos: Option<Vec<f32>>,
}
```

**Pros:**
- Matches Apophysis storage model exactly
- Transform carries all its own data

**Cons:**
- Harder to detect globally if any xaos is active
- Each transform needs separate Option check

### Recommendation: Option A (Flame-level matrix)

The flame-level storage is cleaner for our use case because:
1. Shader builder needs to know globally if xaos is active
2. GPU upload is simpler (one contiguous buffer)
3. Easier validation when transform count changes

### GPU Buffer Layout

```rust
// In src/gpu/buffers.rs

/// Xaos transition weights buffer (optional, only when xaos is active)
/// Layout: N × N matrix where N = num_transforms
/// Index: xaos_weights[src * num_transforms + dst]
pub struct XaosBuffer {
    buffer: Buffer,
    num_transforms: u32,
}

impl XaosBuffer {
    /// Create buffer for xaos weights
    /// Size: N × N × sizeof(f32) = up to 32 × 32 × 4 = 4KB
    pub fn new(device: &Device, num_transforms: u32) -> Self;

    /// Update xaos weights from flame
    pub fn update(&self, queue: &Queue, xaos: &[Vec<f32>]);
}
```

**Memory footprint:**
- 3 transforms: 36 bytes
- 6 transforms: 144 bytes
- 16 transforms: 1KB
- 32 transforms: 4KB

Negligible compared to existing buffers.

### Shader Implementation

#### Conditional Compilation Approach

Add a new condition to `TemplateProcessor`:

```rust
// In shader_builder_v2.rs
processor.set("XAOS_ENABLED", flame.has_xaos());
```

#### Modified Transform Selection (utilities.wgsl)

```wgsl
{{#if XAOS_ENABLED}}
// Xaos buffer binding
@group(0) @binding(9) var<storage, read> xaos_weights: array<f32>;

// Select transform with xaos modifiers
fn select_transform_xaos(rand_val: f32, prev_xform: u32) -> u32 {
    var cumulative = 0.0;
    var total_weight = 0.0;

    let base_idx = prev_xform * params.num_transforms;

    // Calculate total modified weight
    for (var i = 0u; i < params.num_transforms; i++) {
        let base_weight = transforms[i].weight;
        let xaos_modifier = xaos_weights[base_idx + i];
        total_weight += base_weight * xaos_modifier;
    }

    let target = rand_val * total_weight;

    for (var i = 0u; i < params.num_transforms; i++) {
        let base_weight = transforms[i].weight;
        let xaos_modifier = xaos_weights[base_idx + i];
        cumulative += base_weight * xaos_modifier;
        if (target <= cumulative) {
            return i;
        }
    }

    return params.num_transforms - 1u;
}
{{/if}}
```

#### Modified Main Loop (main_template.wgsl)

```wgsl
{{#if XAOS_ENABLED}}
    var prev_xform_idx = 0u;  // Track previous transform for xaos lookup
{{/if}}

    for (var i = 0u; i < params.iterations_per_thread; i++) {
        let rand_val = rng_nextf(&rng);

{{#if XAOS_ENABLED}}
        let xform_idx = select_transform_xaos(rand_val, prev_xform_idx);
        prev_xform_idx = xform_idx;
{{else}}
        let xform_idx = select_transform_const(rand_val);
{{/if}}

        // ... rest of iteration unchanged ...
    }
```

### Performance Analysis

#### Without Xaos (Current Behavior)
- Transform selection: O(n) loop, ~n additions + comparisons
- No xaos buffer read
- Shader size: Unchanged

#### With Xaos Enabled
- Transform selection: O(n) loop, ~n additions + multiplications + buffer reads
- Additional buffer binding (9th binding)
- Shader size: ~50 lines additional

**Overhead estimate:**
- ~15-25% additional cost per iteration for transform selection
- Only affects flames that actually use xaos
- Flames with all 1.0 xaos weights compile to non-xaos shader (zero overhead)

### Alternative: PropTable Optimization

For maximum performance with xaos, Apophysis precomputes a 1024-entry lookup table per source transform. This trades memory for O(1) selection:

```rust
/// Precomputed probability lookup table (Apophysis approach)
/// 1024 slots per source transform, each slot contains destination index
pub struct PropTable {
    table: Vec<u32>,  // Size: num_transforms × 1024
}
```

**Memory footprint:**
- 3 transforms: 12KB
- 6 transforms: 24KB
- 16 transforms: 64KB
- 32 transforms: 128KB

**Not recommended for initial implementation** due to memory cost, but could be added later as an option for performance-critical use cases.

## Implementation Plan

### Phase 1: Core Data Structures

1. **Add xaos field to Flame struct** ([src/scene/transforms.rs](../../src/scene/transforms.rs))
   - Add `xaos: Option<Vec<Vec<f32>>>` field
   - Add helper methods: `has_xaos()`, `get_xaos()`, `set_xaos()`
   - Add `ensure_xaos_size()` for transform add/delete

2. **Add xaos to serialization** ([src/scene/transforms.rs](../../src/scene/transforms.rs))
   - Serialize xaos only when not all 1.0 (compact output)
   - Custom deserializer to handle missing field

3. **Add xaos to FractalConfig** ([src/config/fractal_config.rs](../../src/config/fractal_config.rs))
   - Mirror Flame.xaos in config
   - Add to ConfigPath enum for delta tracking

### Phase 2: GPU Support

4. **Create XaosBuffer** ([src/gpu/buffers.rs](../../src/gpu/buffers.rs))
   - Optional buffer (only created when xaos active)
   - Update method from Flame.xaos

5. **Add xaos buffer to FlameBuffers** ([src/gpu/buffers.rs](../../src/gpu/buffers.rs))
   - Add `xaos_buffer: Option<XaosBuffer>`
   - Dummy buffer for binding when disabled (like path_buffer)

6. **Update bind group creation** ([src/renderer/compute_kernel.rs](../../src/renderer/compute_kernel.rs))
   - Add binding 9 for xaos_weights
   - Use dummy buffer when xaos disabled

### Phase 3: Shader Compilation

7. **Add XAOS_ENABLED condition** ([src/shader_builder_v2.rs](../../src/shader_builder_v2.rs))
   - Detect xaos from active_variations or flame parameter
   - Set condition in TemplateProcessor

8. **Add xaos shader code** ([shaders/core/utilities.wgsl](../../shaders/core/utilities.wgsl))
   - `select_transform_xaos()` function
   - Conditional binding declaration

9. **Update main_template.wgsl** ([shaders/core/main_template.wgsl](../../shaders/core/main_template.wgsl))
   - Track prev_xform_idx when xaos enabled
   - Call xaos-aware selection function

10. **Update header files** (all shader headers)
    - Add xaos binding declaration (conditional)

### Phase 4: Apophysis Compatibility

11. **Parse chaos attribute** ([src/apophysis_xml.rs](../../src/apophysis_xml.rs))
    - Parse `chaos="1 0.5 0 1 ..."` from xform elements
    - Build xaos matrix from per-transform arrays

12. **Export chaos attribute** ([src/apophysis_xml.rs](../../src/apophysis_xml.rs))
    - Write chaos attribute only for non-default weights
    - Match Apophysis format (space-separated values)

### Phase 5: UI

13. **Xaos editor panel** ([src/ui/](../../src/ui/))
    - Grid view showing N×N matrix
    - "View To" / "View From" toggle (like Apophysis)
    - Double-click to toggle 0/1
    - Drag to adjust values
    - Reset all to 1.0 button

14. **Add to ConfigManager** ([src/config/manager.rs](../../src/config/manager.rs))
    - ConfigPath::Xaos { src, dst }
    - Update tracking for undo/redo

### Phase 6: Animation Support

15. **Add xaos to animation targets** ([src/ui/target_selector.rs](../../src/ui/target_selector.rs))
    - Per-cell xaos animation
    - "All from transform N" shortcut

## Testing Strategy

1. **Unit tests**
   - Xaos matrix serialization round-trip
   - Transform add/delete with xaos resize
   - Probability calculation verification

2. **Visual regression tests**
   - Add test flames with xaos patterns
   - Verify render matches Apophysis output

3. **Performance benchmarks**
   - Compare with/without xaos enabled
   - Verify zero overhead when disabled

## File Changes Summary

| File | Changes |
|------|---------|
| `src/scene/transforms.rs` | Add xaos field and methods |
| `src/config/fractal_config.rs` | Add xaos to config |
| `src/config/delta.rs` | Add ConfigPath::Xaos |
| `src/gpu/buffers.rs` | Add XaosBuffer |
| `src/renderer/compute_kernel.rs` | Add xaos binding |
| `src/shader_builder_v2.rs` | Add XAOS_ENABLED condition |
| `shaders/core/header.wgsl` | Add xaos binding declaration |
| `shaders/core/utilities.wgsl` | Add select_transform_xaos |
| `shaders/core/main_template.wgsl` | Track prev_xform, use xaos selection |
| `src/apophysis_xml.rs` | Parse/export chaos attribute |
| `src/ui/xaos_editor.rs` | New file: Xaos editor panel |
| `src/ui/mod.rs` | Register xaos editor |
| `locales/en.yml` | Xaos UI strings |

## Open Questions

1. **Should xaos apply to final transform?**
   - Apophysis excludes final transform from xaos
   - Recommend: Follow Apophysis behavior (no xaos for final)

2. **PropTable optimization for high-transform flames?**
   - Could add as optional optimization later
   - Only beneficial for flames with many transforms (>10)

3. **Xaos presets/patterns?**
   - "One-way flow" (A→B→C→A cycle)
   - "Isolated transforms" (no cross-transitions)
   - Could add preset buttons to UI

## References

- [Xaos in Apophysis 7X](xaos-apophysis.md) - Detailed Apophysis implementation reference
- [Apophysis 7X XForm.pas](https://github.com/xyrus02/apophysis-7x) - Original source code
