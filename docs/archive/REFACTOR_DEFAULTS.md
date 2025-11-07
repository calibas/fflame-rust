# Refactor: Consolidate Default Values

## Problem
Default values for parameters like `histogram_color_scale` are duplicated across 8+ locations in the codebase. This leads to:
- Easy to forget updating all locations when changing defaults
- Code bloat and maintenance burden
- Risk of inconsistency

## Current State

### histogram_color_scale = 100.0 (8 occurrences)
1. `src/config/fractal_config.rs:96` - `default_histogram_color_scale()` ✅ **SOURCE**
2. `src/app/mod.rs:113` - `App::new()`
3. `src/app/mod.rs:163` - `App::new_headless()`
4. `src/gpu/buffers.rs:290` - `GpuParams::new()`
5. `src/gpu/buffers.rs:313` - `AccumulateParams::new()`
6. `src/renderer/compute_kernel.rs:80` - `FlameRenderer::new()`
7. `src/scene/presets.rs:368` - `serpinski_3d_cone()` preset
8. `src/scene/presets.rs:425` - `julia_3d()` preset

### Similar issues exist for:
- `low_density_smoothing = 0.5` (multiple locations)
- `blend_factor = 0.1` (multiple locations)
- `density_compression_strength = 0.0` (multiple locations)
- `target_iterations_per_pixel = 0` (multiple locations)
- `iterations_per_thread = 256` (multiple locations)
- `speed_multiplier = 1` (multiple locations)
- `exposure = 1.0` (multiple locations)
- `gamma = 2.2` (multiple locations)

## Proposed Solution

### Option 1: Use FractalConfig::default() everywhere
```rust
// Instead of:
App {
    histogram_color_scale: 100.0,
    exposure: 1.0,
    gamma: 2.2,
    // ... 20 more fields
}

// Do:
let config = FractalConfig::default();
App {
    histogram_color_scale: config.histogram_color_scale,
    exposure: config.exposure,
    gamma: config.gamma,
    // ... or use From/Into trait
}
```

**Pros:** Single source of truth
**Cons:** Verbose if only using a few fields

### Option 2: Create a DefaultValues constants module
```rust
// src/config/defaults.rs
pub const DEFAULT_HISTOGRAM_COLOR_SCALE: f32 = 100.0;
pub const DEFAULT_LOW_DENSITY_SMOOTHING: f32 = 0.5;
pub const DEFAULT_BLEND_FACTOR: f32 = 0.1;
// ...

// Usage:
use crate::config::defaults::*;
App {
    histogram_color_scale: DEFAULT_HISTOGRAM_COLOR_SCALE,
    // ...
}
```

**Pros:** Clear, discoverable, easy to import
**Cons:** Still need to reference in multiple places, adds a file

### Option 3: Make default functions public and use them
```rust
// In fractal_config.rs, make functions pub:
pub fn default_histogram_color_scale() -> f32 { 100.0 }
pub fn default_low_density_smoothing() -> f32 { 0.5 }
// ...

// Usage:
use crate::config::fractal_config::*;
App {
    histogram_color_scale: default_histogram_color_scale(),
    // ...
}
```

**Pros:** Functions can have logic/comments, centralized
**Cons:** More verbose to call, overkill for simple constants

### Option 4: Hybrid - Derive from FractalConfig with builder
```rust
impl App {
    fn new(/* ... */) -> Self {
        let config = FractalConfig::default();

        Self {
            // Copy all defaults from config
            histogram_color_scale: config.histogram_color_scale,
            exposure: config.exposure,
            gamma: config.gamma,
            low_density_smoothing: config.low_density_smoothing,
            // ... etc

            // Override app-specific fields
            gpu: GpuResources::new(/* ... */),
            flame_renderer: None,
            // ...
        }
    }
}
```

**Pros:** Clear where defaults come from, easy to override
**Cons:** Manual field copying, but only in one place per struct

## Recommended Approach

**Use Option 2 (Constants Module) for frequently duplicated values:**

### Step 1: Create constants module
```rust
// src/config/defaults.rs
//! Default values for configuration parameters
//!
//! These are used across the codebase for initialization.
//! Changing a value here updates all usages.

// Histogram & Color
pub const DEFAULT_HISTOGRAM_COLOR_SCALE: f32 = 100.0;  // Max precision: 100 color levels
pub const DEFAULT_EXPOSURE: f32 = 1.0;
pub const DEFAULT_GAMMA: f32 = 2.2;

// Accumulation
pub const DEFAULT_LOW_DENSITY_SMOOTHING: f32 = 0.5;  // Moderate smoothing
pub const DEFAULT_DENSITY_COMPRESSION: f32 = 0.0;    // No compression
pub const DEFAULT_BLEND_FACTOR: f32 = 0.1;           // 10% blend rate
pub const DEFAULT_USE_DYNAMIC_BLEND: bool = true;    // Exponential convergence

// Performance
pub const DEFAULT_ITERATIONS_PER_THREAD: u32 = 256;
pub const DEFAULT_SPEED_MULTIPLIER: u32 = 1;
pub const DEFAULT_TARGET_ITERATIONS_PER_PIXEL: u64 = 0;  // Disabled

// Other
pub const DEFAULT_MAX_ITERATIONS: u64 = 1_000_000_000;
```

### Step 2: Use constants in FractalConfig::default()
```rust
fn default_histogram_color_scale() -> f32 {
    crate::config::defaults::DEFAULT_HISTOGRAM_COLOR_SCALE
}
```

### Step 3: Replace all hardcoded values with constants
```rust
use crate::config::defaults::*;

App {
    histogram_color_scale: DEFAULT_HISTOGRAM_COLOR_SCALE,
    exposure: DEFAULT_EXPOSURE,
    gamma: DEFAULT_GAMMA,
    // ...
}
```

### Benefits
- ✅ Single source of truth
- ✅ Clear, discoverable naming
- ✅ Easy to change defaults
- ✅ Self-documenting code
- ✅ Compile-time safety
- ✅ No runtime overhead

## Implementation Priority

### High Priority (Duplicated 5+ times)
- ✅ histogram_color_scale (8 duplicates)
- ✅ exposure (6+ duplicates)
- ✅ gamma (6+ duplicates)
- ✅ low_density_smoothing (5+ duplicates)
- ✅ blend_factor (5+ duplicates)

### Medium Priority (Duplicated 3-4 times)
- iterations_per_thread
- speed_multiplier
- density_compression_strength
- target_iterations_per_pixel

### Low Priority (Duplicated 2 times or specific to one context)
- Preset-specific values (can stay hardcoded)
- One-off initializations

## Notes

### Presets
Presets should probably keep hardcoded values because they're defining specific artistic configurations, not using "defaults". The duplication there is intentional and fine.

### GPU Buffers
GPU buffer initialization should pull from App state, not define defaults:
```rust
// Instead of:
histogram_color_scale: 100.0,

// Do:
histogram_color_scale: /* pass from renderer init */,
```

The renderer should get values from FractalConfig/App, not hardcode them.
