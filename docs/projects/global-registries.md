# Global Registries Refactoring Plan

**Status: ✅ COMPLETED (2025-01-02)**

## Overview

This document outlines the plan to refactor `VariationRegistry`, `PresetLibrary`, and `PaletteLibrary` into properly managed global singletons to eliminate redundant loading and improve startup performance.

## Current State (After Refactoring)

### VariationRegistry ✅ (Already Complete)
- **Location**: `src/variations/mod.rs`
- **Singleton**: Yes, via `global_registry() -> &'static VariationRegistry`
- **Pattern**: `once_cell::sync::Lazy`
- **Mutability**: Immutable after initialization
- **Status**: No changes needed

### PresetLibrary ✅ (Refactored)
- **Location**: `src/scene/presets.rs`
- **Singleton**: Yes, via `global_preset_library() -> &'static PresetLibrary`
- **Pattern**: `once_cell::sync::Lazy`
- **Mutability**: Immutable after initialization
- **Status**: Complete - all call sites updated

### PaletteLibrary ✅ (Refactored)
- **Location**: `src/scene/palette.rs`
- **Singleton**: Yes, via `global_palette_library() -> &'static RwLock<PaletteLibrary>`
- **Pattern**: `once_cell::sync::Lazy` with `RwLock` for mutability
- **Mutability**: Mutable via RwLock (add custom palettes, enable/disable packs)
- **Status**: Complete - all call sites updated

## Rendering Pipelines Analysis

### Does Export Need Libraries?

**Key Insight**: `FractalConfig.palette` is `Option<Palette>`. When the palette is embedded in the config, no library lookup is needed.

| Pipeline | PresetLibrary Needed? | PaletteLibrary Needed? | Notes |
|----------|----------------------|------------------------|-------|
| **GUI (Desktop)** | Yes (preset browser) | Yes (palette editor, picker) | Full access needed |
| **CLI Export** | No | **Fallback only** | Config loaded from file with embedded palette |
| **Headless Render** (`renderer/render.rs`) | No | **Fallback only** | Uses `config.palette.or_else(\|\| library.get(...))` |
| **Animation Export** | No | **Fallback only** | Uses same fallback pattern |
| **WASM Export** | No | **Fallback only** | Uses `self.palette_library` from App |
| **WASM `load_preset()`** | Yes | Indirectly (via presets) | Needs preset lookup |

### Fallback Pattern in Exports

All export paths use this pattern:
```rust
let palette = config.palette
    .as_ref()
    .or_else(|| palette_library.get(config.palette_index))
    .ok_or(RenderError::NoPaletteFound)?;
```

**Implication**: If `config.palette` is `Some(...)`, the library is never accessed. Well-formed `.fflame` files should always have embedded palettes.

## Proposed Architecture

### 1. VariationRegistry (No Change)
```rust
// Already exists in src/variations/mod.rs
pub fn global_registry() -> &'static VariationRegistry {
    static REGISTRY: Lazy<VariationRegistry> = Lazy::new(VariationRegistry::new);
    &REGISTRY
}
```

### 2. PresetLibrary (Add Singleton)
```rust
// In src/scene/presets.rs
use once_cell::sync::Lazy;

/// Global preset library singleton (immutable)
pub fn global_preset_library() -> &'static PresetLibrary {
    static LIBRARY: Lazy<PresetLibrary> = Lazy::new(PresetLibrary::new);
    &LIBRARY
}
```

**Changes Required**:
- Add `global_preset_library()` function
- Remove `PaletteLibrary::new()` from `PresetLibrary::new()`
- Presets don't need embedded palettes (they use `palette_index` or custom embedded palette)
- Update all `PresetLibrary::new()` calls to use `global_preset_library()`

### 3. PaletteLibrary (Add Singleton with RwLock)

Since PaletteLibrary needs to be mutable (add custom palettes, enable/disable packs):

```rust
// In src/scene/palette.rs
use once_cell::sync::Lazy;
use std::sync::RwLock;

/// Global palette library singleton (mutable via RwLock)
pub fn global_palette_library() -> &'static RwLock<PaletteLibrary> {
    static LIBRARY: Lazy<RwLock<PaletteLibrary>> = Lazy::new(|| {
        RwLock::new(PaletteLibrary::new())
    });
    &LIBRARY
}
```

**Usage**:
```rust
// Read access (most common)
let library = global_palette_library().read().unwrap();
let palette = library.get(index);

// Write access (rare - adding custom palette)
let mut library = global_palette_library().write().unwrap();
library.add(custom_palette);
```

## Files to Update

### PresetLibrary Changes
| File | Current | Change To |
|------|---------|-----------|
| `src/app/mod.rs:235` | `PresetLibrary::new()` | `global_preset_library()` (or remove, use directly) |
| `src/wasm_api.rs:126` | `PresetLibrary::new()` | `global_preset_library()` |
| `src/wasm_api.rs:191` | `PresetLibrary::new()` | `global_preset_library()` |
| `src/scene/presets.rs` | N/A | Add `global_preset_library()` function |
| `src/scene/presets.rs` | Creates `PaletteLibrary` internally | Remove, use default palette or pass in |

### PaletteLibrary Changes
| File | Current | Change To |
|------|---------|-----------|
| `src/app/mod.rs:236` | `PaletteLibrary::new()` | `global_palette_library().read().unwrap()` |
| `src/renderer/render.rs:173` | `PaletteLibrary::new()` | `global_palette_library().read().unwrap()` |
| `src/animation/export.rs:1121` | `PaletteLibrary::new()` | `global_palette_library().read().unwrap()` |
| `src/scene/presets.rs:365` | `PaletteLibrary::new()` | Remove (use default palette) |
| `src/scene/presets.rs:537` | `PaletteLibrary::new()` | Remove (dead code after refactor) |
| `src/scene/palette.rs` | N/A | Add `global_palette_library()` function |

## Preset Palette Strategy

**Problem**: Presets currently call `PaletteLibrary::new()` to get a default palette.

**Options**:
1. **Embed palette directly** - Each preset defines its own palette inline (like "Julian Disc Sea" already does)
2. **Use Palette::default()** - Fire palette as fallback
3. **Use palette_index only** - Let runtime resolve from global library

**Recommendation**: Option 2 - Use `Palette::fire()` or `Palette::default()` directly. This eliminates the PaletteLibrary dependency entirely from presets.

```rust
fn flame_to_config(flame: Flame) -> FractalConfig {
    FractalConfig {
        flame,
        palette: Some(Palette::fire()),  // Direct, no library needed
        palette_index: 1,  // Fallback index if palette is None
        ...
    }
}
```

## Export Pipeline Simplification

For export paths that currently create `PaletteLibrary::new()` just for fallback:

**Option A**: Keep global singleton for fallback
```rust
let palette = config.palette
    .as_ref()
    .or_else(|| global_palette_library().read().unwrap().get(config.palette_index))
    .ok_or(...)?;
```

**Option B**: Fail if no embedded palette (stricter)
```rust
let palette = config.palette
    .as_ref()
    .ok_or(RenderError::NoPaletteEmbedded)?;
```

**Recommendation**: Option A for backward compatibility, but log a warning when falling back to library.

## Implementation Order

1. **Phase 1: PaletteLibrary Singleton**
   - Add `global_palette_library()`
   - Update all `PaletteLibrary::new()` calls
   - Test GUI, exports still work

2. **Phase 2: PresetLibrary Singleton**
   - Remove PaletteLibrary dependency from PresetLibrary
   - Add `global_preset_library()`
   - Update WASM API calls
   - Test preset loading

3. **Phase 3: Cleanup**
   - Remove redundant code
   - Add deprecation warnings for direct `::new()` calls
   - Update documentation

## Expected Benefits

- **Startup Time**: Load palettes once instead of 8+ times
- **Memory**: Single instance of each library
- **Consistency**: All code paths share same data
- **Thread Safety**: RwLock for PaletteLibrary enables safe concurrent access

## Risks

- **RwLock Contention**: Unlikely issue since writes are rare (only when adding custom palettes)
- **Initialization Order**: `once_cell::Lazy` handles this correctly
- **WASM Compatibility**: `once_cell` works in WASM; `RwLock` works in single-threaded WASM

## Open Questions

1. Should exports fail hard if no embedded palette, or silently fall back to library?
2. Should we add a `Palette::default()` that returns Fire palette for consistency?
3. Should PresetLibrary also be wrapped in RwLock for future "save custom presets" feature?
