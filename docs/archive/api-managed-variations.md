# API-Managed Variations

## Overview

Allow flame variations to be loaded from the API at runtime, in addition to the ~26 hard-coded built-in variations. This enables shipping new variations without app updates and supports a long-term goal of 500+ variations.

All API-served variations are vetted server-side. The trust model assumes the API is the source of truth for variation correctness and safety.

## Goals

- Load WGSL shader code for variations from the API on demand
- Persistent local cache so subsequent loads are instant
- Versioning so server-side updates can invalidate stale caches
- Render flames that reference unknown variations by fetching the variations first
- Graceful fallback: if a fetch fails, render anyway with unknown variations skipped
- A Variations panel in the UI showing the full registry (built-in + cached) with a "Clear Cache" action

## Non-Goals

- User-submitted variations (vetting happens server-side; users only consume)
- Sandboxing untrusted shader code (trust model assumes API is trusted)
- Caching compiled shader artifacts (only the WGSL source)

## Existing System (relevant context)

The variation system is already designed to handle per-variation WGSL source:

- Each variation is a `VariationDef` (in `src/variations/defs/`) bundling name, category, parameters, and `wgsl_2d` / `wgsl_3d` source strings
- Registered in `VariationRegistry` as `VariationInfo` with `wgsl_source` and `wgsl_source_3d` fields
- `ShaderBuilder` assembles the final shader by collecting WGSL from active variations
- The current registry has an `is_core: bool` field that can distinguish built-in from API-loaded

This means adding API-loaded variations doesn't require changes to the shader builder — they become `VariationInfo` entries with `is_core: false` and WGSL strings populated from the API response. The plumbing for per-variation shader code already exists.

## Architecture

### Data flow

```
Flame loaded
    │
    ▼
Check variations referenced by transforms
    │
    ├── All in registry? ──► Build shader, render
    │
    └── Some unknown ──► Block render
                        Show "Loading variations..." notification
                        Fetch missing variations from API (parallel)
                        │
                        ├── All succeed ──► Cache to disk, register, build shader, render
                        │
                        └── Some fail ────► Show warning notification
                                            Build shader without missing variations
                                            Render (those weights effectively zero)
```

### Components

```
VariationRegistry (existing, no structural change needed)
├── variations:    HashMap<String, VariationInfo>  (built-in + API-loaded)
└── ordered_names: Vec<String>                     (stable insertion order)

VariationInfo (existing, with new field)
├── name, display_name, category, phase, needs_rng, parameters
├── wgsl_source, wgsl_source_3d  (already present)
├── is_core: bool                (already present — true for built-in, false for API)
└── version: u32                 (NEW — 0 for built-in, server version for API)

VariationCache (new trait)
├── DesktopCache: filesystem at <user_data>/variations/<name>.json
└── WasmCache:    IndexedDB (NOT localStorage — size limits)

VariationFetcher (new)
└── fetch_by_name(name) → Future<VariationDownload>
```

### `version` on `VariationInfo`

Add `version: u32` to `VariationInfo` for all variations. Built-ins use `0`; API-loaded variations use the server's version number. Uniform handling lets the Variations panel display "built-in" or "API v1" without special cases, and the cache layer can compare versions without checking `is_core` first.

### API response shape

```json
{
  "id": "51a7723d-1c62-438a-b40b-4d979641da7a",
  "name": "zscale",
  "display_name": "Z-Scale",
  "description": "Z-scale 3D transformation",
  "category": "depth_3d",
  "version": 1,
  "phase": "normal",
  "needs_rng": false,
  "parameters": [
    {
      "name": "scale",
      "display_name": "Scale",
      "param_type": "float",
      "default_value": 1.0,
      "min_value": 0.0,
      "max_value": 10.0
    }
  ],
  "shader_2d": "fn variation_zscale(p: vec2<f32>) -> vec2<f32> { return p; }",
  "shader_3d": "fn variation_zscale(p: vec3<f32>) -> vec3<f32> { return vec3(p.xy, p.z * get_param(...)); }"
}
```

Field notes:
- `id` — server's UUID; used for the GET endpoint, not stored on `VariationInfo`
- `name` — unique identifier (snake_case), used as the registry key and cache filename
- `display_name` — human-readable label for the UI
- `description` — optional, shown as a tooltip in the Variations panel
- `category` — matches the existing `VariationCategory` enum (`basic_2d`, `advanced_2d`, `depth_3d`, `full_3d`, `pre_phase`, `post_phase`, `rotation_3d`, `blur`, etc.)
- `version` — integer; bumping it server-side invalidates cached copies with lower versions
- `phase` — `"pre"`, `"normal"`, or `"post"` (matches `VariationPhase`)
- `needs_rng` — whether the shader function takes an RNG state parameter (affects function signature)
- `parameters` — array; each parameter has the same fields as the existing `VariationParameter` struct
- `shader_2d` — WGSL function source for 2D rendering
- `shader_3d` — optional WGSL function source for 3D rendering (omit if 2D-only)

The function signature in the WGSL must match what the shader builder expects, derived from `phase`, `needs_rng`, and whether `parameters` is empty:
- Basic: `fn variation_NAME(p: vec2<f32>) -> vec2<f32>`
- With RNG: `fn variation_NAME(p: vec2<f32>, rng: ptr<function, RngState>) -> vec2<f32>`
- With params: `fn variation_NAME(p: vec2<f32>, xform_id: u32, variation_id: u32) -> vec2<f32>`
- Both: `fn variation_NAME(p: vec2<f32>, xform_id: u32, variation_id: u32, rng: ptr<function, RngState>) -> vec2<f32>`

The 3D variants use `vec3<f32>` instead of `vec2<f32>`.

### Cache invalidation behavior

- On flame load: if the variation is in cache, use it as-is (no API call)
- If not in cache: fetch from API, store with the response's `version`
- Server-side version bumps don't proactively invalidate caches — they take effect the next time we'd otherwise fetch
- Future enhancement (not v1): a "Refresh from API" button in the Variations panel that re-fetches all cached variations and updates if `server_version > cached_version`

### Cache format on disk

`<user_data>/variations/<name>.json`:
```json
{
  "name": "twintrian",
  "version": 1,
  "category": "advanced_2d",
  "parameters": [...],
  "shader_2d": "...",
  "shader_3d": "...",
  "cached_at": "2026-04-18T12:34:56Z"
}
```

Cache uses the same JSON shape as the API response plus a `cached_at` timestamp for diagnostics.

### Versioning behavior

- On flame load: if the variation is in cache, use the cached version (no API call)
- If the flame's transforms reference variations the user hasn't loaded before, fetch fresh
- Server-side version bumps don't immediately invalidate caches — but next time the variation is referenced and a fetch happens (e.g., via "refresh" or cache clear), the new version is fetched

If we need stricter versioning later, we could add an "if-modified-since" check or a global registry version endpoint.

## Implementation Plan

### Phase 1: Registry extensions
- Add `version: u32` field to `VariationInfo` (built-ins use `0`)
- Add a `register_from_api()` method on `VariationRegistry` that accepts an API response, builds a `VariationInfo` with `is_core: false`, and inserts it (mirroring `register_from_def` but without requiring a static `VariationDef`)
- The existing `wgsl_source` / `wgsl_source_3d` fields are already used by the shader builder, so no shader builder changes are needed

### Phase 2: Cache + fetcher
- Add `VariationDownload` API type matching the JSON shape
- Add `VariationCache` trait with desktop (filesystem) and WASM (IndexedDB) impls
- Add `VariationFetcher` that calls the API endpoint
- Load cached variations into the registry at app startup via `register_from_api`

### Phase 3: Fetch on demand
- When loading a flame: scan referenced variations, find any not in the registry
- If any are missing, trigger parallel fetches via `VariationFetcher`
- Block render until all fetches complete (or fail), reusing the loading notification pattern from URL deep-link loads
- 30s timeout per fetch (consistent with URL load timeout)
- On success: write to cache, register
- On failure: show warning notification, render with the missing variations skipped (the IFS will simply not apply those weights)

### Phase 4: Variations panel UI
- New panel listing all variations in the registry, grouped by category
- For each variation: name, category, source label (built-in / API v1)
- Show parameter list per variation
- "Clear Variation Cache" button: deletes all cached files, removes API-loaded variations from registry (built-ins remain), shows confirmation dialog
- "Refresh from API" button (optional, future): re-fetch all cached variations

### Phase 5: Settings integration
- Cache directory location surfaced in Settings (read-only display)
- Total cache size shown in MB

## Storage Locations

### Desktop
`<user_data>/variations/` where `<user_data>` is:
- Windows: `%APPDATA%/FractalFlame/variations/`
- macOS: `~/Library/Application Support/FractalFlame/variations/`
- Linux: `~/.config/FractalFlame/variations/`

### WASM
IndexedDB database `fflame_variations`, object store `variations`, keyed by name.
Reasoning: localStorage has a 5–10MB limit per origin and is synchronous; IndexedDB has effectively no limit and is async-native, which fits our fetch flow.

## Failure Modes

| Scenario | Behavior |
|---|---|
| API unreachable, all variations cached | Works normally with cached + built-in |
| API unreachable, some variations missing | Render flame with those variations skipped, show warning |
| Fetch timeout (>30s) | Treat as failure, show warning, render anyway |
| Cached variation fails to compile | Log error, remove from registry, show warning, treat as missing |
| Cache directory unwritable | Falls back to in-memory only (no persistence) |
| WASM IndexedDB unavailable | Falls back to in-memory only, log warning |
| Variation references variation IDs we don't have | Show warning, render with available ones |

## UI / UX

### Loading notification
"Loading variations: twintrian, secant2..."

### Success notification (silent — variations just appear)

### Failure notification
"Couldn't load variations: twintrian. Fractal may render incorrectly."

### Variations panel
```
Variations (153)
├── Linear (Basic 2D) — built-in
├── Sinusoidal (Basic 2D) — built-in
├── Twintrian (Advanced 2D) — API v1
├── Secant2 (Advanced 2D) — API v1
└── ...

[Clear Cache (12 cached)]
```

## Open Questions

- Should built-in variations be listed in the Variations panel, or only API-loaded ones?
- Auto-prune: useful, or just a "Clear Cache" button is enough?
- Should we add a "Variation Library" panel for browsing all available API variations (not just ones in use)? Useful for discovery, but means a list endpoint + thumbnails.
- Trust signing: do we want HMAC or signature verification on cached files? Probably not for v1.
