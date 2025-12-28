# Path Buffer Optimization Plan

## Overview

The path buffer system (for PathMap coloring and path filters) should be optional to avoid memory and performance overhead when not in use.

## Current State (as of commit bc1c49f)

### GPU Resources Always Allocated
- **path_buffer**: `width × height × 32 bytes` - stores path data per pixel
  - At 1920×1080: ~66MB
  - At 4K: ~265MB
- **path_filter_buffer**: `64 × 16 bytes` = 1KB - stores filter patterns

### Shader Overhead
- Path tracking code always compiled into shader
- `var path = array<u32, 4>` declared every thread (register allocation)
- Conditional check every iteration: `(color_mode == 2u) || (num_path_filters > 0u)`
- Early exit when both false, but branch still exists

### When Path Features Are Used
- **PathMap color mode**: Stores path data to buffer, colors pixels by transform sequence
- **Path filters**: Blocks specific transform sequences (suffix or exact-depth matching)

## Optimization Plan

### Phase 1: Optional Buffers (Current Focus)

**Goal**: Zero memory overhead when path features disabled

#### Changes to FlameBuffers
```rust
// Before
path_buffer: Buffer,
path_filter_buffer: Buffer,

// After
path_buffer: Option<Buffer>,
path_filter_buffer: Option<Buffer>,
dummy_path_buffer: Buffer,      // 1 element, for binding when disabled
dummy_filter_buffer: Buffer,    // 1 element, for binding when disabled
```

#### Changes to FlameRenderer
```rust
// Track state
path_features_enabled: bool,

// Methods
fn set_path_features_enabled(&mut self, device: &Device, enabled: bool) {
    if enabled != self.path_features_enabled {
        self.path_features_enabled = enabled;
        if enabled {
            self.create_path_buffers(device);
        } else {
            self.drop_path_buffers();
        }
        self.rebuild_bind_group(device);
    }
}
```

#### Trigger Conditions
Path features enabled when:
- `color_mode == PathMap` OR
- `num_path_filters > 0`

#### Bind Group Handling
- WebGPU requires all declared bindings to be bound
- When disabled: bind dummy 1-element buffers
- When enabled: bind real buffers
- Rebuild bind group on toggle

#### Integration Points
- `FlameRenderer::set_color_mode()` - check if PathMap
- `FlameRenderer::set_path_filters()` - check if filters > 0
- Both should call internal method to update path_features_enabled

### Phase 2: Shader Variants (Deferred to Shader Builder Rewrite)

**Goal**: Zero shader overhead when path features disabled

#### Current Shader Structure
```
header.wgsl          - structs, bindings (includes path buffer bindings 8, 9)
rng.wgsl             - random number generation
variations_2d.wgsl   - variation functions
apply_variations     - generated per-flame
utilities.wgsl       - helper functions
path_filter.wgsl     - path filter checking functions
main_2d.wgsl         - main loop with path tracking
```

#### Proposed Shader Variants
Two main loop variants:
1. `main_2d.wgsl` - full version with path tracking
2. `main_2d_simple.wgsl` - no path tracking, no path array declaration

#### ShaderBuilder Changes
```rust
pub fn build_trajectory_2d(
    &self,
    active_variations: &HashMap<String, f32>,
    path_features_enabled: bool,  // NEW
) -> String {
    // ...
    if path_features_enabled {
        shader.push_str(include_str!("../shaders/core/path_filter.wgsl"));
        shader.push_str(include_str!("../shaders/core/main_2d.wgsl"));
    } else {
        shader.push_str(include_str!("../shaders/core/main_2d_simple.wgsl"));
    }
}
```

#### Benefits
- No register allocation for path array when disabled
- No branch in hot loop
- Cleaner separation of concerns

#### Considerations
- Shader rebuild on toggle (~50ms, acceptable)
- Already rebuild for 2D/3D mode switch
- Can cache both variants if needed

### Phase 3: Shader Builder Rewrite (Future)

#### Problems with Current ShaderBuilder
- Monolithic include-based assembly
- Hard to add conditional features
- Duplicate code between 2D/3D variants
- No caching of compiled shaders

#### Proposed Architecture
```rust
struct ShaderBuilder {
    // Feature flags
    render_mode: RenderMode,        // 2D or 3D
    path_features: bool,            // path tracking enabled
    // ... other flags

    // Cached compiled modules
    cache: HashMap<ShaderKey, ShaderModule>,
}

struct ShaderKey {
    render_mode: RenderMode,
    path_features: bool,
    active_variations: BTreeSet<String>,  // sorted for consistent hashing
}
```

#### Features to Support
- 2D vs 3D mode
- Path features on/off
- Active variation set (already dynamic)
- Future: final transform, symmetry, etc.

## Implementation Order

1. **Phase 1**: Optional buffers with dummy binding (this PR)
   - Saves memory immediately
   - No shader changes needed
   - Low risk

2. **Phase 2**: Shader variants (separate PR, during shader rewrite)
   - Create main_2d_simple.wgsl without path code
   - Add flag to ShaderBuilder
   - Rebuild shader on toggle

3. **Phase 3**: Full shader builder rewrite (separate project)
   - Modular architecture
   - Shader caching
   - Clean feature flag system

## Testing

### Phase 1 Verification
- [ ] Memory usage drops when PathMap mode disabled and no filters
- [ ] PathMap mode still works correctly
- [ ] Path filters still work correctly
- [ ] Toggle between modes works without crashes
- [ ] Resize works in both states

### Performance Metrics
- Baseline: Current memory/performance with path features unused
- After Phase 1: Memory should drop to ~0 for path buffers
- After Phase 2: Should see slight FPS improvement from reduced register pressure

## Files to Modify

### Phase 1
- `src/gpu/buffers.rs` - Optional path buffers, dummy buffers
- `src/gpu/pipelines.rs` - Bind group creation with optional buffers
- `src/renderer/compute_kernel.rs` - Track path_features_enabled, toggle logic
- `src/app/mod.rs` - Trigger toggle based on color_mode and filter count

### Phase 2 (Deferred)
- `shaders/core/main_2d_simple.wgsl` - New file, no path tracking
- `shaders/core/main_3d_simple.wgsl` - New file, no path tracking
- `src/shader_builder_v2.rs` - Add path_features flag
