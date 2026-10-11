# Online Service and Shared Resources

**Overview:** Everything that talks to the online service at
`fractalsforall.com`: signing in, saving and loading flames and
animations, the cloud palette library and the script library, and the
**shared resources** — variations and effects the app downloads,
caches and registers at run time, next to the user's own local
plugins. Also what this repository publishes **for** the service: the
generated engine contract, the corpora, and the OpenAPI spec both
repositories work to. The app renders fully offline; every online
feature fails to a status line, not to a broken panel.

**See also:**
- [VARIATIONS.md](VARIATIONS.md) — the registry rules downloads and
  plugins register under
- [EFFECTS.md](EFFECTS.md) — the effect registry and its download checks
- [ANIMATION.md](ANIMATION.md) — what an animation carries
- [FRACTAL-BROWSER.md](FRACTAL-BROWSER.md) — the Fractal Browser's Online
  tab
- [PALETTE_LIBRARY.md](PALETTE_LIBRARY.md) — Cloud Palettes
- [SCRIPTING.md](SCRIPTING.md) — the scripts the online library holds
- [RELEASE.md](../RELEASE.md) — §3 *Syncing with the API*, §8 *Adding
  things that affect the API*
- [VARIATIONS_WIRE_FORMAT.md](../projects/VARIATIONS_WIRE_FORMAT.md) —
  the variation wire contract, prose half
- [WASM.md](../WASM.md) — the web app and the gallery modules

**Code locations:**
- [src/api/mod.rs](../../src/api/mod.rs) — `API_BASE_URL`, `ApiState`
  (one method per endpoint), the health check
- [src/api/client.rs](../../src/api/client.rs) — the HTTP transport
  (ureq on desktop, `fetch` on the web)
- [src/api/types.rs](../../src/api/types.rs) — wire types,
  `ApiRenderMode`
- [src/api/sync.rs](../../src/api/sync.rs) — `FractalConfig`,
  `Animation` and script source ↔ wire format
- [src/api/health.rs](../../src/api/health.rs) — `HealthPolicy`
- [src/api/auth.rs](../../src/api/auth.rs) — `AuthState`
- [src/storage/credentials.rs](../../src/storage/credentials.rs) —
  remembered sign-in
- [src/storage/catalog.rs](../../src/storage/catalog.rs),
  [resource_store.rs](../../src/storage/resource_store.rs),
  [variation_catalog.rs](../../src/storage/variation_catalog.rs),
  [effect_catalog.rs](../../src/storage/effect_catalog.rs),
  [variation_cache.rs](../../src/storage/variation_cache.rs),
  [effect_cache.rs](../../src/storage/effect_cache.rs) — catalogs and
  download caches
- [src/storage/plugins.rs](../../src/storage/plugins.rs),
  [src/provenance.rs](../../src/provenance.rs) — local plugins, and
  where a resource came from
- [src/app/variation_fetch.rs](../../src/app/variation_fetch.rs),
  [effect_fetch.rs](../../src/app/effect_fetch.rs),
  [script_cloud.rs](../../src/app/script_cloud.rs) — the app's
  background requests
- [src/ui/login_dialog.rs](../../src/ui/login_dialog.rs),
  [save_online_dialog.rs](../../src/ui/save_online_dialog.rs) — the
  Account panel and the Save Online dialog
- [src/contract.rs](../../src/contract.rs),
  [docs/generated/engine-contract.json](../generated/engine-contract.json)
  — the engine contract
- [src/bin/export_variations_json.rs](../../src/bin/export_variations_json.rs),
  [export_scripts_json.rs](../../src/bin/export_scripts_json.rs),
  [export_effects_json.rs](../../src/bin/export_effects_json.rs) — the
  corpus exporters
- [docs/main/openapi.json](openapi.json) — the API's OpenAPI spec

---

## The service and the transport

**One server.** `api::API_BASE_URL` is `https://fractalsforall.com`,
compiled in. `SystemSettings::api_base_url` is a deprecated field that
is still read from old settings files and never written.

**The spec.** [openapi.json](openapi.json) is the API's own OpenAPI
document (82 paths), committed here as the wire authority for both
repositories. The client implements a subset of it.

**Transport** ([client.rs](../../src/api/client.rs)). Every call is an
`async fn` with the same signature on both platforms:

| | desktop | web |
| --- | --- | --- |
| HTTP | a shared, pooled `ureq` agent (blocking) | the browser's `fetch`, CORS, `credentials: "include"` |
| auth | `Authorization: Bearer <token>` | the server's session cookie; the token argument is ignored |
| timeouts | 10 s to connect, 30 s overall | 30 s, 60 s for an upload, enforced with an `AbortController` |
| run on | a spawned thread under `pollster::block_on` | `wasm_bindgen_futures::spawn_local` |

Results come back through an `Arc<Mutex<…>>` slot that the frame loop
polls, so the UI never waits on the network. Status codes map to
`resources::FetchError`: 401 `Unauthorized`, 403 `Forbidden`, 404
`NotFound`, any other non-2xx `Http { status, message }` with the
response body as the message, transport failures `Network`. A 204 is
an empty success.

---

## Online mode

`SystemSettings::online_mode` (default `true`) gates the online
**surfaces**. No panel writes it; it changes only by editing the
settings file.

| gated on online mode | not gated |
| --- | --- |
| File → **Save Online** | the variation catalog, fetched once per session at startup |
| Window → **Account** | fetching a variation a loaded flame names but the app lacks |
| the Fractal Browser's **Online** tab | the effect catalog, fetched once per session (on desktop it needs sign-in: `list_effects` requires a token) |
| Palette Library → **Cloud Palettes** | fetching a missing effect (needs sign-in) |
| the menu bar's connection and account indicator | the Scripts panel's online library (needs sign-in) |
| the periodic health check | loading a web deep link |

---

## Signing in

The **Account** panel ([login_dialog.rs](../../src/ui/login_dialog.rs))
has Sign In and Register tabs, posting to `/api/auth/login` and
`/api/auth/register`; both answer with a token.

**Desktop.** The token is kept as `auth_token` in the settings file
(`settings.json` in the app data directory), with `auth_email` for
display; signed in means a token is present. **Remember me** stores the
email and password as `saved_credentials`, encrypted with AES-256-GCM
under a key compiled into the binary — the module's own words:
obfuscation, not security. At startup `try_auto_login` decrypts them and
signs in again. Signing out clears the token, the email and the saved
credentials.

**Web.** The server sets a session cookie; nothing secret is stored.
`auth_email` is held in memory only. Because a cookie may already be
valid, the web app always runs one health check at startup, and that is
how it discovers an existing session. Signing out also posts
`/api/auth/logout` to clear the cookie.

`api::auth::AuthState` is the token holder inside an `ApiState`; the
app builds a fresh `ApiState` per request and keeps its own sign-in
state in `SystemSettings`.

---

## Connectivity and session health

`check_api_health` calls `GET /api/users/me`. A 401 or a network error
is retried once, because the first request after an idle spell can land
on a pooled connection the server already closed.

`HealthPolicy` ([health.rs](../../src/api/health.rs)) decides what an
outcome means, free of clocks so it is tested as a transition table:

| outcome | connectivity | next check |
| --- | --- | --- |
| any HTTP answer — 200, 401, 5xx | `Online`; the failure count resets | 30 s |
| first network error | unchanged | 5 s |
| two network errors in a row | `Unreachable` | 15 s while unreachable |

Only Online → Unreachable and back raise a notification. While signed
in, the Save Online menu item is enabled only when connectivity is
`Online` (so also not before the first check answers). A 401
(`TokenExpired`) clears the sign-in, clears the cloud state in the UI,
and says the session expired.

The check runs only in online mode and while signed in; on the web the
startup check runs whether or not a sign-in is known.

---

## Flames online

### The wire format

`config_to_create_request` ([sync.rs](../../src/api/sync.rs)) builds a
`CreateFlameRequest` for `POST /api/flames` and `PUT /api/flames/{id}`:

| field | contents |
| --- | --- |
| `name` | the flame's name; the server's name column is authoritative on load |
| `visibility` | `private`, `unlisted` or `public`, or absent |
| `palette` | the palette, inline: packed `color_data` (R, G, B per entry, 0–255) when it is 256 evenly spaced stops, otherwise its `stops`. The server hashes the content. |
| `config` | the canonical `.fflame` JSON (versioned, defaults stripped) **minus** the palette and with the root flame's three transform pools emptied. Subflames keep their transforms inside it. |
| `transforms` | the root flame's transforms, flattened: `kind` (`normal`, `linked`, `final`), `sort_order`, `variation_names` (the non-zero ones, for search) and `data`, the transform's own JSON |

The server stores the blob verbatim and reads only its **root**
`render_mode` (`/render_mode`, where config v3 puts it). A new config
field needs no API work.

**Loading** (`flame_response_to_config`) reverses it: the transforms
are bucketed by `kind` and `sort_order` back into the three pools, the
server's name and the inline palette are re-inserted, and the result
runs through the same versioned migration as a local `.fflame`. A blob
with no `flame` object — damage from an earlier server migration that
flattened the flame into the top level — is rebuilt around the
transforms; the flame-only state that was flattened (xaos, solo,
post-symmetry, subflames) falls back to defaults.

### Render modes

`ApiRenderMode` has four wire values: `2d`, `3d`, `escape`,
`simulation`. **Escape-time configs and simulations are stored as
flames**, on the same endpoints, told apart by `render_mode` alone. An
escape config's exact-decimal centre strings survive the round trip
(`an_escape_config_round_trips_with_its_centre_intact`).

A simulation maps to `"simulation"`, but the server's `render_mode`
enum does not know that value yet (the committed `openapi.json` lists
`RenderMode` as `2d`, `3d`, `escape` only; an upload is an HTTP 400
`render_mode_invalid`). So `save_flame` and `update_flame` refuse a
simulation client-side, before the token check and before anything is
sent (`refuse_unsupported_online`, returning `FetchError::Unsupported`
with a sentence the dialog shows) — the same seatbelt escape had until
the server added `escape`. Remove it when the server adds `simulation`.
(`simulation_configs_are_refused_by_save_and_update`)

### Names, never definitions

A flame travels as variation **names** plus its own parameters, never
as WGSL. `a_flame_payload_carries_names_not_definitions` asserts that a
local plugin's shader cannot leave the machine in a flame payload. A
flame that uses a local plugin therefore renders only where the plugin
is installed, and saving one online shows a notice naming the plugins
(`warn_if_flame_needs_local_plugins`) — a notice, not a refusal.

### Save Online

File → **Save Online** (or the Animation panel's button) opens the Save
Online dialog ([save_online_dialog.rs](../../src/ui/save_online_dialog.rs)).
Signed out, the menu item opens the Account panel instead.

- **Name**, **Upload thumbnail** (a 512×512 render of the config, JPEG
  quality 90, `PUT /api/flames/{id}/thumbnail`) and **Make public**.
- **Flame:** **Save** for a flame not yet online; for one that is,
  **Update** (only if you own it) and **Save as Copy**. A warning shows
  when animations depend on the flame.
- **Animation:** needs the flame saved online, owned by you, and an
  animation with tracks. **Save**, or **Update** and **Save as Copy**
  for one already online.

What **Make public** sends:

| action | checked | unchecked |
| --- | --- | --- |
| flame Save / Save as Copy | `public` | no visibility (the server's default) |
| flame Update | `public` | `private` if the flame was public; otherwise no visibility — `update_flame` then reads the flame's current visibility and sends it back, so a private or unlisted flame **keeps** what it had |
| animation Save / Update | `public` | `private` |

The dialog opens with the box checked for a public flame, so unchecking
it on an update makes the flame private (`flame_update_visibility`).

---

## Animations online

`animation_to_create_request` sends `POST /api/animations` or
`PUT /api/animations/{id}`: the name, the flame's id, `duration`,
`loop_mode` (`once`, `loop`, `ping_pong`), and `tracks`, `generators`
and `base_config` as opaque JSON. Online, an animation is tied to its
flame by `flame_id`; `base_config` is sent only if the animation already
carries one (the local `.anim` save embeds the config, the online save
does not add it).

**Loading** (`load_animation_full`, `GET /api/animations/{id}`) returns
the animation with the server's copy of its flame embedded. The
animation's own `base_config` is used if it has one; otherwise the
embedded flame becomes its `base_config`. The app loads that config
(undoable), loads and binds the animation, regenerates its signal
generators, and switches to the Animation layout.

**Where animations appear.** A flame loaded from the server carries a
summary of its animations; the Animation panel lists them with a Load
button each. On the web, `?flame=<id>` and `?animation=<id>` in the page
URL load either at startup, with a 30 s timeout.

---

## The Fractal Browser and cloud palettes

The Fractal Browser's **Online** tab lists your flames
(`GET /api/flames`, first 100) or a search (`GET /api/search/flames`),
loads one (`GET /api/flames/{id}`) and deletes one
(`DELETE /api/flames/{id}`); see
[FRACTAL-BROWSER.md](FRACTAL-BROWSER.md). Its error check treats
"Authentication required" (401) as an expired session and signs the
user out; "Access denied" (403) is shown as an error and leaves the
session alone (`only_a_401_signs_the_user_out`). A delete asks for
confirmation first.

**Cloud Palettes** in the Palette Library lists your palette library
(`GET /api/users/me/palettes`, first 100); a click sets the config's
palette (undoable), and **X** removes the library entry
(`DELETE /api/users/me/palettes/{hash}`) without deleting the palette.
See [PALETTE_LIBRARY.md](PALETTE_LIBRARY.md).

---

## The online script library

The Scripts panel raises a `ScriptCloudRequest`; the app performs it in
the background ([script_cloud.rs](../../src/app/script_cloud.rs)) and
folds the answer into `ScriptCloudState`. All of it needs sign-in, and
one request runs at a time.

| request | does |
| --- | --- |
| `ListMine` | your scripts (`GET /api/scripts`, first 100) |
| `Search(q)` | public search (`GET /api/search/scripts`, query percent-encoded, first 50) |
| `Publish` | create; records the returned id and version against the local script |
| `Update` | update with the version last read; a 409 is a **conflict** the user resolves (`ScriptConflict`) |
| `Adopt` | fetch someone's script and save it locally under a free name, marked as somebody else's, so it runs under the restrictions downloaded scripts get |
| `Refetch` | replace a local script's content with the server's in place, keeping its provenance |
| `Unpublish` | delete it from the server; the local copy stays |

A publish payload (`script_to_create_request`) is derived from the
source by the script's own declaration pass — display name, kind,
flags, and the header comment as the description — so a script that
does not compile cannot be published. It can still be saved locally.

---

## Shared resources: variations and effects

### Provenance

Every registered variation and effect has a `Provenance`
([provenance.rs](../../src/provenance.rs)):

| | `Builtin` | `Api { version }` | `Local` |
| --- | --- | --- | --- |
| shipped with the app | yes | no | no |
| third-party code | no | yes | yes |
| in the download cache (Clear Cache removes it) | no | yes | no |
| can be updated from the server | no | yes | no |

### Storage

[resource_store.rs](../../src/storage/resource_store.rs) holds the
storage both subsystems share. Paths are relative to the app data
directory on desktop and are localStorage keys on the web:

| path | holds |
| --- | --- |
| `variations/<name>.json` | a downloaded variation (`VariationDownload`) |
| `variations/_catalog.json` | the cached variation catalog |
| `effects/<name>.json`, `effects/_catalog.json` | the same for effects (`EffectDownload`) |
| `plugins/variations/<name>.json`, `plugins/effects/<name>.json` | local plugins |

A name starting with `_` is metadata, never a resource, so the catalog
file is not listed, counted or cleared as a cached entry. Plugins live
under their own prefix so a cache operation cannot reach them.

**At startup**, `storage::load_installed_resources` registers the
cached variations, then the cached effects, then the plugins. Every
entry point that renders calls it — the app, CLI `export` and
`export-animation` — because a flame using a missing variation renders
wrong silently (a missing variation contributes nothing). The app
reports refused plugins in a notification; the CLI prints them to
stderr.

### Catalogs

The app fetches `GET /api/variations` and `GET /api/effects` once per
session in the background, caches them, and starts from the cached copy,
so the panels work offline. A failed fetch is logged at info level and
nothing else. The catalog is the only route by which the server's
prose — `description_plain`, `authors` — reaches the app.

`catalog::merge_state` compares a catalog row with what is installed:

| installed | row | state |
| --- | --- | --- |
| built-in | any | `BuiltIn` |
| local plugin | any | `LocalOverride` — reported, never an update candidate |
| downloaded, older | any | `UpdateAvailable` |
| downloaded, same or newer | any | `Downloaded` |
| nothing | `downloadable` | `Available` |
| nothing | not downloadable | `BuiltInOnlyElsewhere` — counted, no fetch offered |

The Variations panel shows the catalog: update buttons for stale
downloads, the available list (fetched on demand, not downloadable from
the list), the not-fetchable count, and **Clear Variation Cache**, which
removes downloads and the cached catalog and refetches the catalog;
plugins are untouched. The Effects panel shows the effect catalog; there
is no control to clear the effect cache.

### Missing variations

When a config is loaded, `missing_variations_in` lists the variation
names with non-zero weight that the registry lacks, and
`classify_missing` checks each against the cached catalog:

| catalog says | `MissingReason` | the app |
| --- | --- | --- |
| listed, downloadable | `Downloadable` | fetches it |
| no catalog yet (or empty) | `Unknown` | fetches it — offline is not evidence of a plugin |
| listed, not downloadable | `KnownButNotFetchable` | notifies that a plugin is needed |
| not listed | `ProbablyAPlugin` | notifies that a plugin is needed |

Fetches (`GET /api/variations/{name}`, no authentication) run in
parallel and **pause rendering** until they all finish or 30 s pass.
Each download is cached, then registered as `Api { version }`; then the
renderer reloads the config so the shader includes them. Failures and
timeouts are notified. Updating from the Variations panel is the same
fetch again; names held by a built-in or a local plugin are filtered
out first.

### Missing effects

A missing effect is found where the effect chain compiles and has no
registry entry (`effects::take_missing_effects`). If signed in, the app
fetches it (`GET /api/effects/{name}`); each name is tried once per
session. A download is **registered first and cached second**, so a
payload this build refuses is never cached; a refusal is notified.
Rendering is not paused: the picture simply lacks the effect until it
arrives. The registration checks are in [EFFECTS.md](EFFECTS.md).

---

## Local plugins

A local plugin is a variation or effect installed by the user, in
**the same JSON as a download** (`VariationDownload` /
`EffectDownload`), so it takes the same registration path, compile path
and limits. [plugins.rs](../../src/storage/plugins.rs):

- **Where:** `<app data>/plugins/variations/` and
  `<app data>/plugins/effects/`; on the web, localStorage keys under
  the same prefix.
- **Identity:** the **file name**, not the `name` inside it.
- **Collisions are refused, never shadowed.** A plugin may not take a
  built-in's name or one already registered (built-ins, then downloads,
  load first), and a download may not displace a plugin.
- **Installing:** copy the file into the folder and restart. There is
  no install command in the UI; `plugins::install` and
  `plugins::remove` exist (install validates with the registration
  rules) but nothing calls them outside tests.

Two worked examples, with notes, are in
[assets/plugins-example/](../../assets/plugins-example/README.md): a
variation equal to `linear` at its defaults and a colour effect that is
a no-op at its defaults.

---

## The engine contract

[src/contract.rs](../../src/contract.rs) generates
[docs/generated/engine-contract.json](../generated/engine-contract.json):
the client's vocabularies and limits, emitted from the code, for the API
repository's conformance tests.

| key | contents |
| --- | --- |
| `categories`, `phases`, `features` | each wire name, how many shipped variations use it, and whether it is on the wire today |
| `param_types` | the parameter type names |
| `render_modes` | `known` (from `RenderMode::ALL`) and `wire_path` (`/render_mode`) |
| `limits` | variation slots per flame and the per-variation policy cap, variations and transforms per flame, effect params and slots, script source size |
| `helper_libraries` | the shader libraries spliced in by name, which a server-hosted variation cannot reach |
| `payload_features` | `plot_emits`, a feature with a payload |
| `shipped_without_shader_3d` | variations with no 3D body |
| `scripts` | the reserved built-in script stems, script kinds, known flags |
| `shipped_variations`, `shape` | the count, and the shape fingerprint |

**Regenerate** deliberately and read the diff:

```bash
UPDATE_CONTRACT=1 cargo test --lib contract_is_current
```

`contract_is_current` fails when the committed file differs from what
the code generates (it runs with the default engine features).

**The shape fingerprint** (`shape`) is a hash of the document's sorted
**key paths**, not its values; `$`-prefixed comment keys are excluded.
The API pins it, so a stale copy fails loudly. Adding a field moves it;
adding a variation changes only counts and does not. **A new `Feature`,
`VariationCategory` or `ParamType` does not move it either** — it is one
more array element with the same keys — so the API must be told
directly. [RELEASE.md §3](../RELEASE.md) has the table and the
procedure. Render modes are the exception: they are published as
`render_modes.known`, which the API's conformance test reads.

Where the vocabularies come from matters when adding one: `phases` and
`features` iterate `VariationPhase::ALL` and `Feature::ALL` (a test
holds `Feature::ALL` to the enum), and `render_modes` iterates
`RenderMode::ALL` (a test's exhaustive match holds it), but
`categories` and `param_types` are written out as lists in `generate()`,
so a new category or parameter type is absent from the contract until
it is added there by hand.

---

## The corpora

The API's catalogs are bulk-imported from JSON this repository
generates on demand. Each is large and written to the gitignored
`output/` unless a path is given:

| command | writes | contents |
| --- | --- | --- |
| `cargo run --release --bin export_variations_json [out.json]` | `output/variations-corpus.json` | every shipped variation: WGSL, parameters, features, and the description and authors recovered from `///` doc comments |
| `cargo run --release --bin export_scripts_json [out.json]` | `output/scripts-corpus.json` | every built-in script: declared name, kind and flags, description, and the source verbatim |
| `cargo run --release --bin export_effects_json [out.json]` | `output/effects-corpus.json` | every built-in effect: WGSL, parameters, category, `requires_blend_modes` |

Each stamps the current `contract_shape`. The variation and script
exporters refuse to write a corpus with a missing description (or, for
scripts, an undeclared kind), because the API has no other source for
it. Vocabularies come from the same `to_api_str` functions as the
contract.

---

## Working across the two repositories

The API is a separate repository; nothing syncs automatically. The
procedure — which artifact to regenerate for which change, what moves
the fingerprint, and when to update `openapi.json` — is
[RELEASE.md](../RELEASE.md) §3 and §8.
[VARIATIONS_WIRE_FORMAT.md](../projects/VARIATIONS_WIRE_FORMAT.md) is the
variation wire contract's prose half; its vocabularies defer to the
generated contract.

---

## Web gallery modules

The `wasm/` crates are standalone modules for web embedding (the
Endless Gallery). Each depends on this crate with
`default-features = false`, which drops `web-app` (the editor's
wasm-bindgen exports, and the `app` and `ui` modules), and picks
engines by feature:

| crate | package | engine features |
| --- | --- | --- |
| `wasm/script` | `fflame-script` | `engine-flame`, `engine-escape`, `engine-sim` — script + seed + params → config JSON |
| `wasm/render` | `fflame-render` | `engine-flame`, `engine-escape` |
| `wasm/flame` | `fflame-flame` | `engine-flame` |
| `wasm/escape` | `fflame-escape` | `engine-escape` |
| `wasm/sim` | `fflame-sim` | `engine-sim` |

The four renderers are one source (`wasm/render/src/lib.rs`) built with
different features. An escape or simulation config sent to a module
without that engine is refused with `RenderError::EngineMissing`
("this build has no simulation engine"); a flame config sent to a
module without `engine-flame` renders against a catalog holding only
`linear`. So `fflame-render` refuses simulations. The JavaScript API and its guarantees are in
[wasm/README.md](../../wasm/README.md); building and releasing them are
in [WASM.md](../WASM.md) and [RELEASE.md](../RELEASE.md) §5.

---

## Gotchas

- **Simulations cannot be saved online yet** — refused client-side
  until the server knows the mode; see *Render modes*.
- **Loading an animation from the Animation panel's list goes through
  the web deep-link loader**, which builds its `ApiState` with an empty
  token. On the web the cookie authenticates it; on desktop the request
  carries an empty bearer token.
- **The search URL is not encoded.** `SearchFlamesParams::to_query_string`
  interpolates the name raw, so `&` or `#` in a search corrupts the
  query (noted at `api::urlencode`, which the script search does use).
- **Several client methods have no caller:** `search_public_flames`,
  `get_palette`, `set_library_nickname`, `list_flame_animations`,
  `delete_animation`, `validate_token`.

---

## Design record

- [api-shared-resources.md](../archive/projects/api-shared-resources.md)
  — downloaded variations and effects, catalogs, plugins, provenance,
  the script library
- [endless-gallery.md](../archive/projects/endless-gallery.md) — the
  standalone WASM renderer and script evaluator
- [api-v1-v2/](../archive/api-v1-v2/README.md) — the first API
  integration and the v2 wire format: why the API looks as it does
- [api-managed-variations.md](../archive/api-managed-variations.md) —
  the first design for downloaded variations
- [VARIATIONS_BULK_METADATA_IMPORT.md](../archive/projects/VARIATIONS_BULK_METADATA_IMPORT.md)
  — the corpus exporter's plan
