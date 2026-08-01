# API Integration — WASM Client Feature

## Overview

Feature-gated module (`--features api`) that enables the WASM app to authenticate
with the Fractal Flame API and save/load fractals and palettes to a remote server.

Desktop builds can also use this feature, but the primary target is WASM.

## Feature Gate

```toml
# Cargo.toml
[features]
default = []
api = []
```

All API integration code is gated behind `#[cfg(feature = "api")]`. When disabled,
the app behaves exactly as it does today — local-only operation with JSON
import/export via clipboard/download.

## Architecture

```
src/api/
  mod.rs          — Public API, feature gate re-exports
  client.rs       — HTTP client (wraps resources/fetch.rs)
  auth.rs         — Token storage, login state, JWT handling
  types.rs        — API request/response types
  sync.rs         — FractalConfig ↔ API format conversion
```

### Dependency on Existing Infrastructure

- **`src/resources/fetch.rs`** — Reuse the existing WASM fetch system. Extend with
  auth headers and POST/PUT/DELETE methods (currently only GET).
- **`src/storage/backend.rs`** — Store auth tokens in localStorage (WASM) or
  filesystem (desktop). Same path as SystemSettings persistence.
- **`src/ui/fractal_gallery.rs`** — Reuse the gallery widget for browsing remote
  flames. It already handles async thumbnail loading on WASM.
- **`LoadState` pattern** — Already handles Idle/Loading/Loaded/Failed for async
  resources. Reuse for API call state tracking.

## Architecture: Separation of Online Features

Online features (auth, save/load, gallery) are kept separate from the core app:

- **Core app** (`default` features): Local-only fractal editing, no network code
- **API module** (`api` feature): All network/auth code lives behind the feature gate
- **JavaScript wrapper** (WASM only): A thin JS layer on the host page coordinates
  auth flow and passes tokens into the WASM app. This wrapper can also trigger
  save/load actions and manage the browser-side auth state.
- **Desktop**: Opens the browser for login (same website), receives tokens back via
  the same mechanism (deep link, localhost callback, or manual paste — TBD).

The core Rust app never handles passwords or login forms directly. The website
handles all authentication UI, leveraging browser password managers and OAuth
providers natively.

## Authentication

### Flow

1. User clicks "Sign In" in the menu bar (or JS wrapper triggers it)
2. App opens the fflame.app login page in the default browser:
   - **WASM**: JS wrapper opens login page (same origin or popup)
   - **Desktop**: `webbrowser` crate (already a dependency) opens the URL
3. User authenticates on the website (email/password, OAuth, etc.)
   - Browser password managers work naturally since it's a real web page
4. Website completes auth and provides tokens back to the app:
   - **WASM**: Website writes tokens to localStorage or posts via `window.postMessage()`.
     The JS wrapper picks up the tokens and passes them to the WASM app.
   - **Desktop**: Website redirects to a callback URL. App receives tokens via
     deep link (`fflame://callback?token=...`) or localhost listener (fallback).
5. App stores tokens and uses them for all subsequent API calls
6. On 401, attempt token refresh; if refresh fails, prompt re-login

### Token Storage

```
WASM:    localStorage (managed by JS wrapper, passed to WASM app)
Desktop: {app_data_dir}/api_tokens.json
```

Stored as JSON: `{ "access_token": "...", "refresh_token": "...", "expires_at": 1234567890 }`

### JavaScript Wrapper (WASM)

The host page includes a thin JS wrapper that:
- Manages auth state in localStorage
- Opens the login page when the WASM app requests it
- Listens for auth callbacks (postMessage or localStorage changes)
- Passes tokens into the WASM app via exported Rust functions
- Can trigger save/load/gallery actions from outside the WASM app

```javascript
// Example: JS wrapper API surface
const fflame = {
    onAuthRequest() { /* open login page */ },
    onTokenReceived(tokens) { /* pass to WASM */ },
    getAuthState() { /* check localStorage */ },
    triggerSave() { /* tell WASM to save current flame */ },
};
```

The WASM app exposes functions the wrapper can call:
```rust
#[cfg(feature = "api")]
#[wasm_bindgen]
pub fn set_auth_tokens(access_token: &str, refresh_token: &str, expires_at: f64);

#[cfg(feature = "api")]
#[wasm_bindgen]
pub fn get_auth_state() -> String; // "signed_in" | "signed_out"
```

### UI

- **Signed out**: Menu bar shows "Sign In" button
- **Signed in**: Menu bar shows username + "Sign Out" button
- Auth state is a field on `App`, checked before any API calls
- Unauthenticated API calls return 401 → attempt refresh → prompt sign-in if refresh fails

### Implementation Notes

- The app never renders a login form — the website handles all auth UI
- Browser password managers, OAuth social login, MFA all work naturally
- Desktop uses `webbrowser` crate (already a dependency) to open the login URL
- Token refresh can happen in Rust (simple POST to token endpoint) or via JS wrapper
- The `jsonwebtoken` crate can optionally decode the JWT client-side to extract
  the username for display, but validation always happens server-side
- Desktop token callback mechanism TBD (deep links vs localhost listener vs manual paste)

## Saving Fractals

### Save Flow

1. User clicks File → Save to Cloud (or Ctrl+Shift+S)
2. If not authenticated, prompt to sign in first
3. Convert current `FractalConfig` to API format:
   - FractalConfig + Flame fields → flat flame object
   - Transforms → array of transform objects
   - Palette → embedded or FK reference (server decides)
   - Xaos → flattened f32 array (or null)
   - Effects → array with sort_order
4. POST `/api/flames` (new) or PUT `/api/flames/{id}` (update)
5. Server returns flame ID → store locally for future updates
6. Show success notification in UI

### Save Format

The API accepts a JSON body that closely mirrors the `.fflame` format but with
server-specific additions (user_id is inferred from the JWT):

```json
{
  "name": "My Flame",
  "config": { /* FractalConfig JSON, same format as .fflame */ }
}
```

The server decomposes this into relational tables. The client doesn't need to know
the table structure — it sends/receives the same JSON format it already uses for
local save/load.

### Conflict Resolution

Simple last-write-wins. No collaborative editing in v1.

## Loading Fractals

### Browse Flow

1. User opens Cloud Library panel (or File → Browse Cloud)
2. Panel shows a gallery of the user's saved flames (reuses FractalConfigGallery)
3. API call: GET `/api/flames?page=1&per_page=20`
4. Server returns flame list with metadata (name, thumbnail_url, created_at)
5. Thumbnails loaded async from S3/CDN URLs
6. User clicks a flame → GET `/api/flames/{id}` → full FractalConfig JSON
7. Loaded via same path as local import: `FractalConfig::from_json()` → `load_config_with_undo()`

### Public Gallery

- GET `/api/flames/public?page=1&sort=popular` — browse all public flames
- No auth required for browsing public flames
- "Fork" button creates a copy in the user's library (requires auth)

### Search

- Query by name, variation names, color similarity
- Server handles search via indexed metadata columns
- Client sends query params: `?q=spiral&variations=julian,swirl`

## Palette Integration

### Save Custom Palettes

- Palettes saved as part of the flame (embedded in the config JSON)
- Standalone palette save: POST `/api/palettes` with palette JSON
- Palette scoping (custom/private/public) matches the API plan

### Load Palette Packs

- GET `/api/palettes/public?page=1` — browse public palette library
- Displayed in the existing Palette Library panel as an additional pack source
- Downloaded palettes cached in localStorage

## API Client

### HTTP Methods

Extend `src/resources/fetch.rs` to support:

```rust
// Current: GET only
pub async fn fetch_json<T: DeserializeOwned>(url: &str) -> FetchResult<T>

// New methods needed:
pub async fn post_json<T, B>(url: &str, body: &B, token: Option<&str>) -> FetchResult<T>
pub async fn put_json<T, B>(url: &str, body: &B, token: Option<&str>) -> FetchResult<T>
pub async fn delete(url: &str, token: Option<&str>) -> FetchResult<()>
```

### Base URL Configuration

```rust
// Compile-time or runtime configurable
const API_BASE_URL: &str = "https://api.fflame.app";

// Or configurable in SystemSettings for dev/staging
```

### Error Handling

Extend `FetchError` with auth-specific variants:

```rust
pub enum FetchError {
    // Existing...
    Unauthorized,        // 401 — token expired or invalid
    Forbidden,           // 403 — not your resource
}
```

## UI Changes

### Menu Bar

```
File → Save to Cloud         (Ctrl+Shift+S)  [api feature only]
File → My Flames              (cloud gallery) [api feature only]
File → Public Gallery          (browse public) [api feature only]
```

### Cloud Status Indicator

Small cloud icon in the menu bar when `api` feature is enabled:
- Grey cloud: not signed in
- Green cloud: signed in, synced
- Spinner: API call in progress

### New Panel: Cloud Library

Reuses `FractalConfigGallery` with an API-backed data source instead of
local presets. Tabs for "My Flames" and "Public Gallery".

## Config ID Tracking

When a flame is loaded from or saved to the API, store the server-side ID:

```rust
pub struct FractalConfig {
    // ... existing fields ...

    /// Server-side flame ID (set when loaded from / saved to API)
    #[cfg(feature = "api")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub api_flame_id: Option<String>,
}
```

This enables "Save" (update existing) vs "Save As" (create new) behavior.

## Implementation Phases

### Phase 1: Foundation
- Add `api` feature flag to Cargo.toml
- Create `src/api/` module structure
- Extend fetch.rs with POST/PUT/DELETE + auth headers
- Token storage in localStorage/filesystem
- JS wrapper scaffold (WASM): auth bridge functions
- Website login page integration (open browser, receive tokens)
- Desktop: `webbrowser` crate to open login URL, token callback mechanism

### Phase 2: Save/Load
- Save current flame to API (POST/PUT)
- Load flame from API (GET + gallery UI)
- Cloud Library panel (My Flames)
- Config ID tracking for updates

### Phase 3: Social
- Public gallery browsing
- Fork/copy public flames
- Palette library integration
- Search and filtering

## Open Questions

- Desktop token callback: deep links (`fflame://`) vs localhost listener vs manual paste?
- Auth provider choice: Clerk vs Auth0 vs custom (affects website login page)
- Thumbnail generation: client-side (WASM render) or server-side?
- Offline support: queue saves when offline, sync when reconnected?
- Rate limiting: how to handle on the client side (retry with backoff?)
- Versioning: should the API support flame version history?
- JS wrapper scope: how much control should the wrapper have over the app?
