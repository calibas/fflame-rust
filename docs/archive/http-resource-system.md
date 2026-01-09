# HTTP Resource Fetching System

**Branch:** `feature/http-resources`
**Created:** 2025-01-07
**Status:** Complete (Phases 1-2)

## Overview

A unified system for fetching resources (palettes, fractals, animations) from HTTP sources, working consistently across desktop and WASM platforms. This lays the groundwork for an optional online API for sharing user-created content.

## Goals

1. **Platform Consistency**: Same fetch mechanism for desktop and WASM
2. **Generic Design**: Reusable for palettes, fractals, animations, and future resource types
3. **Lazy Loading**: Resources fetched on-demand with loading state UI
4. **Offline Fallback**: Embedded essential resources for instant startup
5. **Future-Ready**: Architecture supports remote API integration

## What Was Implemented

### Phase 1: Core Fetch Infrastructure ✅

- Created `src/resources/mod.rs` with core types (`LoadState`, `PackMetadata`, `ResourceManifest`)
- Implemented `src/resources/fetch.rs` with platform-specific fetch:
  - Desktop: Filesystem read via `std::fs`
  - WASM: Browser `fetch()` API via `web_sys`
- Created `src/resources/error.rs` with `FetchError` type
- Created `src/resources/palettes.rs` for palette-specific loading

### Phase 2: Palette Pack System ✅

- Created `assets/palettes/packs/manifest.json` with 6 packs
- Embedded `builtin.json` and `manifest.json` at compile time for WASM
- Refactored `PaletteLibrary` to use `PalettePackInfo` with load states
- UI shows loading spinners, error states with retry button
- Auto-fetch enabled packs on startup (both platforms)
- WASM: Async loading with global/local state synchronization
- Build scripts copy palette assets to `pkg/` for WASM

**Palette Packs:**
- Built-in (5 embedded palettes) - always available
- Starter Pack (12 palettes) - enabled by default
- Apophysis Classic 1-4 (701 palettes total) - load on demand

### Deferred

- **Phase 3**: Preset pack migration (presets work fine as-is)
- **Phase 4**: Animation support (future feature)
- **Phase 5**: Remote API preparation (future feature)

## Architecture

### Files Created/Modified

**New Files:**
- `src/resources/mod.rs` - Core types and re-exports
- `src/resources/fetch.rs` - Platform-specific HTTP/filesystem fetch
- `src/resources/error.rs` - FetchError enum
- `src/resources/palettes.rs` - Palette pack loading functions
- `assets/palettes/packs/manifest.json` - Pack discovery manifest
- `assets/palettes/packs/builtin.json` - Embedded fallback pack

**Modified Files:**
- `src/scene/palette.rs` - PaletteLibrary now uses PalettePackInfo
- `src/ui/palette_library.rs` - Loading states, WASM async handling
- `build-wasm.bat` / `build-wasm.sh` - Copy palette assets to pkg/

### Key Types

```rust
/// Load state for any fetchable resource
pub enum LoadState {
    NotLoaded,           // Metadata known, content not fetched
    Loading,             // Fetch in progress
    Loaded,              // Ready to use
    Failed(String),      // Error message
}

/// Metadata for a resource pack (from manifest)
pub struct PackMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub file: String,
    pub item_count: usize,
    pub enabled_by_default: bool,
}

/// Pack info with load state tracking
pub struct PalettePackInfo {
    pub metadata: Option<PackMetadata>,
    pub pack: Option<PalettePack>,
    pub load_state: LoadState,
    pub enabled: bool,
    pub is_builtin: bool,
}
```

### WASM Considerations

- Manifest embedded at compile time for instant pack discovery
- Async fetch via `wasm_bindgen_futures::spawn_local`
- Global singleton pattern for state updates from async context
- `sync_from_global()` method syncs async results to UI instance
- `ctx.request_repaint()` after async completion

## Usage

**Desktop:** Packs load synchronously when enabled. Enabled packs auto-load on startup.

**WASM:** Packs load asynchronously. UI shows spinner during fetch. Enabled packs auto-load when Palette Library panel first renders.

## Future Extensions

The infrastructure supports:
- Remote API endpoints (change base URL)
- Additional resource types (presets, animations)
- User content sharing
- Caching strategies
