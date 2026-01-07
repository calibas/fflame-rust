# HTTP Resource Fetching System

**Branch:** `feature/http-resources`
**Created:** 2025-01-07
**Status:** Planning

## Overview

A unified system for fetching resources (palettes, fractals, animations) from HTTP sources, working consistently across desktop and WASM platforms. This lays the groundwork for an optional online API for sharing user-created content.

## Goals

1. **Platform Consistency**: Same fetch mechanism for desktop and WASM
2. **Generic Design**: Reusable for palettes, fractals, animations, and future resource types
3. **Lazy Loading**: Resources fetched on-demand with loading state UI
4. **Offline Fallback**: Embedded essential resources for instant startup
5. **Future-Ready**: Architecture supports remote API integration

## Resource Types

| Type | Current System | New System |
|------|---------------|------------|
| Palette Packs | Desktop: filesystem, WASM: embedded | HTTP fetch with manifest |
| Fractal Presets | Desktop: filesystem, WASM: embedded | HTTP fetch with manifest |
| Animations | Not implemented | HTTP fetch with manifest |

## Architecture

### Core Abstractions

```rust
// src/resources/mod.rs

/// Load state for any fetchable resource
#[derive(Debug, Clone, PartialEq)]
pub enum LoadState {
    NotLoaded,           // Metadata known, content not fetched
    Loading,             // Fetch in progress
    Loaded,              // Ready to use
    Failed(String),      // Error message
}

/// Metadata for a resource pack (from manifest)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackMetadata {
    pub id: String,
    pub name: String,
    pub description: String,
    pub file: String,              // Relative path/URL to JSON file
    pub item_count: usize,         // Number of items in pack
    pub enabled_by_default: bool,
}

/// Manifest listing available packs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ResourceManifest {
    pub version: u32,
    pub resource_type: String,     // "palettes", "presets", "animations"
    pub packs: Vec<PackMetadata>,
}

/// Generic pack info with load state
pub struct PackInfo<T> {
    pub metadata: PackMetadata,
    pub load_state: LoadState,
    pub content: Option<T>,        // None until loaded
}
```

### Platform-Specific Fetch

```rust
// src/resources/fetch.rs

/// Fetch text content from a URL (relative or absolute)
/// - Desktop: Reads from filesystem OR HTTP
/// - WASM: Uses browser fetch() API
pub async fn fetch_text(url: &str) -> Result<String, FetchError>;

/// Fetch and parse JSON
pub async fn fetch_json<T: DeserializeOwned>(url: &str) -> Result<T, FetchError>;
```

### Resource Manager

```rust
// src/resources/manager.rs

/// Manages async loading of resource packs
pub struct ResourceManager {
    base_url: String,
    pending_fetches: Vec<PendingFetch>,
}

impl ResourceManager {
    /// Start fetching a resource, returns immediately
    pub fn start_fetch(&mut self, url: &str) -> FetchHandle;

    /// Poll for completed fetches (call each frame)
    pub fn poll_completions(&mut self) -> Vec<FetchResult>;
}
```

## Manifest Format

### Palette Manifest (`assets/palettes/packs/manifest.json`)
```json
{
  "version": 1,
  "resource_type": "palettes",
  "packs": [
    {
      "id": "starter",
      "name": "Starter Pack",
      "description": "Essential palettes for getting started",
      "file": "starter_pack.json",
      "item_count": 12,
      "enabled_by_default": true
    },
    {
      "id": "apophysis1",
      "name": "Apophysis Classic 1",
      "description": "Classic Apophysis palettes (1-175)",
      "file": "apophysis1.json",
      "item_count": 175,
      "enabled_by_default": false
    }
  ]
}
```

### Preset Manifest (`assets/presets/manifest.json`)
```json
{
  "version": 1,
  "resource_type": "presets",
  "packs": [
    {
      "id": "examples",
      "name": "Example Fractals",
      "description": "Built-in example configurations",
      "file": "examples.json",
      "item_count": 8,
      "enabled_by_default": true
    }
  ]
}
```

## Phases

### Phase 1: Core Fetch Infrastructure
**Goal:** Create platform-agnostic fetch system

**Tasks:**
- [ ] Create `src/resources/mod.rs` with core types
- [ ] Implement `src/resources/fetch.rs` with platform-specific code
  - Desktop: `reqwest` or filesystem fallback
  - WASM: `web_sys::fetch` via `wasm-bindgen-futures`
- [ ] Create `src/resources/manager.rs` for async coordination
- [ ] Add error types and logging

**Files:**
- `src/resources/mod.rs` - Core types (LoadState, PackMetadata, etc.)
- `src/resources/fetch.rs` - Platform fetch implementations
- `src/resources/manager.rs` - Async fetch coordination
- `src/resources/error.rs` - Error types

### Phase 2: Palette Pack Migration
**Goal:** Migrate palette loading to new system

**Tasks:**
- [ ] Create `assets/palettes/packs/manifest.json`
- [ ] Create embedded fallback pack (5 basic palettes, compiled in)
- [ ] Refactor `PaletteLibrary` to use `PackInfo<PalettePack>`
- [ ] Update UI to show loading states per pack
- [ ] Auto-fetch enabled packs on startup
- [ ] Remove old filesystem/include_str loading code

**UI Changes:**
```
▶ Built-in (5 palettes) ✓           [Always available]
▼ Starter Pack (12 palettes) ✓      [Loaded]
    [palette previews...]
▶ Apophysis 1 (175 palettes)        [Click to load]
▶ Apophysis 2 (180 palettes)        [Loading... 45%]
▶ Apophysis 3 (140 palettes)        [Failed: Network error] [Retry]
```

### Phase 3: Preset Pack Migration
**Goal:** Apply same pattern to fractal presets

**Tasks:**
- [ ] Create `assets/presets/manifest.json`
- [ ] Create embedded fallback presets
- [ ] Refactor `PresetLibrary` to use resource system
- [ ] Update Preset Library UI with loading states

### Phase 4: Animation Support (Scaffold)
**Goal:** Prepare infrastructure for animations

**Tasks:**
- [ ] Define animation pack format
- [ ] Create manifest structure
- [ ] Add placeholder in resource manager
- [ ] No UI implementation yet (future project)

### Phase 5: Remote API Preparation (Future)
**Goal:** Design for remote resource servers

**Considerations:**
- Base URL configuration (local vs remote)
- Authentication for user uploads
- Caching strategy (localStorage/filesystem)
- Rate limiting and error handling
- User's "My Library" syncing

## Desktop vs WASM Behavior

| Aspect | Desktop | WASM |
|--------|---------|------|
| Base URL | `assets/` (filesystem) | Relative to page URL |
| Fetch method | Filesystem read or HTTP | Browser fetch() |
| Caching | Filesystem | localStorage |
| Offline | Full filesystem access | Only embedded + cached |

## Embedded Fallback Resources

Resources compiled into the binary for instant startup:

**Palettes (Built-in Pack):**
- Grayscale
- Fire
- Cool
- Rainbow
- Purple/Pink

**Presets (Built-in Pack):**
- Sierpinski Triangle
- Barnsley Fern
- Default Flame

These are always available, even offline, before any fetches complete.

## Error Handling

```rust
pub enum FetchError {
    NetworkError(String),      // Connection failed
    NotFound(String),          // 404
    ParseError(String),        // Invalid JSON
    Timeout,                   // Request timed out
}
```

UI should:
1. Show error message on failed pack
2. Offer "Retry" button
3. Log details to console
4. Not block other packs from loading

## Testing Strategy

1. **Unit tests**: Mock fetch for parsing/state logic
2. **Integration tests**: Local file serving
3. **WASM tests**: Browser-based fetch verification
4. **Offline tests**: Verify embedded fallbacks work

## Dependencies

**New:**
- `reqwest` (desktop HTTP, optional feature)

**Existing:**
- `wasm-bindgen-futures` (WASM async)
- `web-sys` (WASM fetch API)
- `serde_json` (parsing)

## Migration Path

1. Implement new system alongside old
2. Feature flag to switch (`use_http_resources`)
3. Test thoroughly on both platforms
4. Remove old code once stable
5. Remove feature flag

## References

- Current palette loading: `src/scene/palette.rs`
- Current preset loading: `src/scene/presets.rs`
- Storage backend: `src/storage/backend.rs`
- WASM entry: `src/lib.rs`
