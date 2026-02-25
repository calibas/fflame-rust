# PR: API Integration (feature/api-integration → main)

## Summary

Full client-side integration with the Fractal Flame REST API, enabling users to save, load, browse, search, and delete flames and palettes online. Works on both desktop (ureq) and WASM (browser Fetch API).

## Key Features

### Authentication
- **Login & Registration** — Email/password auth via dedicated Login dialog panel
- **JWT persistence** — Token stored in SystemSettings (desktop: filesystem, WASM: localStorage)
- **Auto-reconnect** — Periodic health checks (GET /api/users/me every 30s) detect connectivity state
- **Stale connection resilience** — Health check retries once on 401 to handle spurious failures after sleep/wake
- **Sign Out** — Clears token from all storage locations

### Save & Load
- **Save Online** — Creates flame + palette on the API, links them together
- **Save dialog** — Name input with "Upload thumbnail" (on by default) and "Make public" (off by default) checkboxes
- **Thumbnail upload** — Renders 512x512 thumbnail via existing GPU pipeline, uploads as PNG via PUT /api/flames/{id}/thumbnail
- **Update existing** — Re-saves to the same flame ID
- **Load from API** — Downloads flame + palette and reconstructs FractalConfig

### Browse & Search
- **Online tab** in Fractal Browser — Lists user's flames with load/delete actions
- **Palette Library** — Online section for browsing API palettes
- **Search** — Filter by render mode, variations, transform count, name

### Connectivity
- **Three-state tracking** — Unknown → Online → Unreachable (orthogonal to auth status)
- **Visual indicators** — Amber "Offline" label in menu bar, greyed-out Save Online buttons when unreachable
- **Network errors preserve JWT** — Only genuine 401 from /api/users/me clears the token

### Optimization
- **Compact palette format** — Indexed 256-color palettes use color_data (packed u32 array, ~3KB) instead of 256 stop objects (~15KB)
- **HTTP connection pooling** — Shared ureq Agent for TCP/TLS keep-alive on desktop

## Architecture

### New Modules
- `src/api/` — API integration layer
  - `mod.rs` — ApiState coordinator, health check, all high-level operations
  - `client.rs` — Cross-platform HTTP client (WASM fetch + ureq), JSON and binary PUT support
  - `types.rs` — Request/response types matching OpenAPI schema (~550 lines)
  - `sync.rs` — FractalConfig ↔ API format bidirectional conversion (~635 lines)
  - `auth.rs` — AuthState with token/user management
- `src/ui/login_dialog.rs` — Login/register panel with email+password forms
- `src/ui/save_online_dialog.rs` — Save Online panel with name, thumbnail, visibility options
- `src/wasm_api.rs` — JS-callable auth wrapper for WASM builds

### Modified Modules
- `src/app/mod.rs` — Health check fields, connectivity polling in render loop
- `src/app/ui_handlers.rs` — Save/update/delete handlers, thumbnail rendering, health check processing
- `src/ui/mod.rs` — EguiLayer: api_connectivity, cloud state management, auth flow
- `src/ui/menu_bar.rs` — Online indicator, Save Online/Update buttons with connectivity gating
- `src/ui/menu_context.rs` — MenuState carries api_connectivity
- `src/ui/fractal_browser.rs` — Online tab with flame list, load/delete actions
- `src/ui/palette_library.rs` — Online palette browsing section
- `src/ui/panel_viewer.rs` — Login and Save Online dock panels
- `src/storage/settings.rs` — API fields: base_url, auth_token, auth_email, online_mode
- `src/resources/error.rs` — Added Unauthorized, Forbidden, NotFound variants to FetchError
- `locales/en.yml` — 67 new i18n strings for all API UI

### Cross-Platform Pattern
All async operations follow the same pattern:
- **Desktop**: `std::thread::spawn` + `pollster::block_on` + `Arc<Mutex<Option<Result>>>` polling
- **WASM**: `wasm_bindgen_futures::spawn_local` + same mutex polling
- Results polled each frame in the render loop

## Commits (18)

1. `c5f780e` Add API integration project doc
2. `3e82e54` Add API integration module (Phase 1: WASM foundation)
3. `3ffc166` Add JS auth wrapper and fix FlameListItem serialization
4. `f401f2a` Add Save Online menu item under File menu (feature-gated)
5. `3e1233f` Add Online tab to Fractal Browser for browsing API flames
6. `d624158` Fix save order: create flame before palette to satisfy API validation
7. `de130e8` Add search/palette client methods and Phase 2 plan doc
8. `9860913` Complete Phase 1 API UI: save/update flow, notifications, delete
9. `57f4909` Replace compile-time API feature flag with runtime Online Mode
10. `d9492d1` Fix Save Online dialog: convert from egui::Window to docked panel
11. `dd273bc` Make all API operations work on desktop (not just WASM)
12. `b156e21` Add HTTP connection pooling via shared ureq Agent
13. `9fa8ecc` Redesign Account panel, add registration, fix 401 handling and Save Online bugs
14. `852eb8d` Add API connectivity tracking with periodic health checks
15. `f60a4c0` Update palette API types: replace scope with visibility, remove flame_id
16. `7190b6a` Add thumbnail upload and public visibility to Save Online flow
17. `01419f6` Use compact color_data format for indexed palettes in API uploads
18. `f44e04f` Fix spurious logout after sleep by retrying 401 in health check

## Stats

- **30 files changed**, 5,545 insertions, 56 deletions
- New files: 8 (api module: 5, UI panels: 2, wasm_api: 1)

## Dependencies

- `ureq = "2"` — Added for desktop HTTP (was already in Cargo.toml for native builds)
- No new WASM dependencies (uses existing web-sys Fetch API features)

## Testing

- Desktop: Manual testing of full save/load/update/delete cycle
- Connectivity: Verified health check handles sleep/wake, network loss, token expiry
- Thumbnail: Verified 512x512 PNG generation and upload
- Palette format: Indexed palettes use compact color_data, gradients use stops

## API Endpoints Used

| Method | Endpoint | Purpose |
|--------|----------|---------|
| POST | /api/auth/login | Email/password login |
| POST | /api/auth/register | New account registration |
| GET | /api/users/me | Token validation + health check |
| GET | /api/flames | List user's flames |
| POST | /api/flames | Create new flame |
| PUT | /api/flames/{id} | Update existing flame |
| DELETE | /api/flames/{id} | Delete flame |
| PUT | /api/flames/{id}/thumbnail | Upload PNG thumbnail |
| GET | /api/palettes | List palettes |
| POST | /api/palettes | Create palette |
| PUT | /api/palettes/{id} | Update palette |
| DELETE | /api/palettes/{id} | Delete palette |
| GET | /api/search/flames | Search with filters |
