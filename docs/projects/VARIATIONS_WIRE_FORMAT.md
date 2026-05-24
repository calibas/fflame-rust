# Variations Wire Format — Client ↔ API

Reference for the variation wire contract between this client and the
fractalsforall API. Use this when changing either side; the client's
expectations are listed here so the API repo can stay aligned.

Variations are **read-only on the client**: the server is the source of
truth, the client fetches and caches. There is no create / update /
delete from this side.

---

## 1. Endpoints in use

| Method | Path | Auth | Returns | Client call site |
| ------ | ---- | ---- | ------- | ---------------- |
| `GET`  | `/api/variations` | none | `Vec<VariationListItem>` | `ApiState::list_variations` ([src/api/mod.rs](../../src/api/mod.rs)) |
| `GET`  | `/api/variations/{name}` | none | `VariationDownload` | `ApiState::fetch_variation` ([src/api/mod.rs](../../src/api/mod.rs)) |

`{name}` is the canonical lowercase identifier (e.g. `julia`,
`pre_rotate_x`), matching the `name` field on the payload. The client
keys its cache and shader by this name.

Both endpoints are public (no token). The client uses the empty-string
token path through its `api_get_unauth` helper.

---

## 2. List payload — `VariationListItem`

Summary form, no WGSL. Used to populate the registry pane and prefetch
queues.

```rust
struct VariationListItem {
    id: String,
    name: String,          // canonical, lowercase
    display_name: String,
    category: String,      // snake_case (see §5)
    version: u32,
    description: Option<String>,

    // PLANNED — pending API rollout. See §7.
    authors: Vec<String>,  // serde(default); order-preserving; one entry
                           //   per author, free-form "Name (year)" form
}
```

Defined at [src/api/types.rs](../../src/api/types.rs) (search
`VariationListItem`).

---

## 3. Download payload — `VariationDownload`

Full record. Carries the WGSL source. Cached to disk on desktop at
`<app_data>/variations/<name>.json` as the raw JSON.

```rust
struct VariationDownload {
    // Identity
    id: String,
    name: String,                       // function suffix; the WGSL fn
                                        //   must be `variation_<name>`
    display_name: String,
    description: Option<String>,        // serde(default)
    category: String,                   // see §5
    version: u32,                       // bumps invalidate client cache

    // Execution
    phase: ApiVariationPhase,           // "pre" | "normal" | "post"

    // Signature-shaping flags (drive WGSL fn signature; see §4)
    needs_rng: bool,                    // serde(default)
    needs_transform: bool,              // serde(default, alias = "needs_affine")
    writes_color: bool,                 // serde(default)

    // Parameters (user-tweakable + init-derived)
    parameters: Vec<ApiVariationParameter>,   // serde(default)
    init_param_count: usize,                  // serde(default)

    // Shader sources
    shader_2d: String,                  // required
    shader_3d: Option<String>,          // optional; client auto-generates
                                        //   a 2D-pass-through 3D wrapper
                                        //   when None
    shader_init: Option<String>,        // optional; required when
                                        //   init_param_count > 0

    // PLANNED — pending API rollout. See §7.
    authors: Vec<String>,               // serde(default); see §2
}
```

`needs_transform` has a serde alias for `needs_affine` for backward
compatibility with older API payloads — the client will read either,
but new payloads should use `needs_transform`.

### `ApiVariationParameter`

```rust
struct ApiVariationParameter {
    name: String,                       // lowercase identifier
    display_name: String,               // for UI labels
    param_type: ApiParamType,           // see below
    default_value: f32,
    min_value: Option<f32>,             // serde(default)
    max_value: Option<f32>,             // serde(default)

    // PLANNED — pending API rollout. See §7.
    description: Option<String>,        // serde(default); free-form
                                        //   per-param help / tooltip prose
}
```

### `ApiParamType`

Wire form: serde `snake_case`, externally tagged for `enum`.

```jsonc
"float"               // continuous, clamped to [min, max]
"unlimited_float"     // slider [min, max] but typed entry unbounded
"integer"             // f32 wire, cast to i32 for UI
"unlimited_integer"   // ditto, unbounded typed entry
"boolean"             // 0.0 / non-zero; UI renders a checkbox
"angle"               // degrees 0..360
{ "enum": { "choices": ["choiceA", "choiceB", "..."] } }
```

All param values are stored as `f32` on the wire and in the GPU
buffer regardless of type — `boolean` is `0.0` / `1.0`, `enum` is the
index of the chosen variant, `integer` is a whole-number `f32`. The
`param_type` drives UI rendering and validation only, never storage
layout.

**Adoption status (client-side, drives the bulk-import migration in
§7):**

- `boolean` is fully wired but underused. The client corpus currently
  has ~38 parameters declared as `integer` with range `[0.0, 1.0]`
  that are semantically booleans (`filled`, `invert`, `inverse`,
  `xaxis`, `use_cos_x`, etc.). These migrate to `boolean` during the
  bulk import.
- `enum` is fully wired but **unused** — no variation declares one
  today. ~6 parameters declared as `integer` with small ranges and
  distinct per-value semantics (`mode`, `type`, `shape`) migrate to
  `enum` with proper `choices` labels during the bulk import.

### `ApiVariationPhase`

Wire form: serde `lowercase`.

```jsonc
"pre"      // direct modification of pre-affine point; not weighted
"normal"   // weighted contribution to running result accumulator
"post"     // direct modification of post-accum result; not weighted
```

---

## 4. WGSL function contract

The client's shader builder emits the call site; the API's `shader_2d`
(and `shader_3d`) body must declare a matching signature. The
arguments are appended in a **fixed order** based on the flags. Skip
arguments whose flag is false.

```
fn variation_<name>(
    p: vec2<f32>,                              // or vec3<f32> in 3D
    [accum: vec2<f32>,]                        // if needs_accum (see §6 — not yet on the wire)
    [xform_id: u32, variation_id: u32,]        // if parameters.len() > 0  OR  needs_transform
    [rng: ptr<function, RngState>,]            // if needs_rng
    [vc: ptr<function, f32>,]                  // if writes_color
) -> vec2<f32>                                 // or vec3<f32> in 3D
```

Notes the API team needs to know:

- **Function name** is always `variation_<name>`. The client builds the
  call as `variation_{name}({args})` and the call must match.
- **`(xform_id, variation_id)`** is passed together when *either*
  parameters or `needs_transform` is set. `variation_id` is the slot
  index in `xform.variations[]`, used by `get_param(xform_id,
  variation_id, slot)` to read parameters and by transform reads to
  identify which weight to use.
- **`needs_transform`** lets a variation read the per-transform storage
  buffer entry (`transforms[xform_id]`) — affine coefficients, weight,
  color, opacity, `direct_color`. Set this true whenever the WGSL body
  references `transforms[...]` for anything other than its own
  `xform.variations[idx]`.
- **`writes_color`** is set for Apophysis direct-color (DC) variations
  that write the iteration-local color register through `*vc = ...`. The
  client uses this flag to detect whether any DC variation is active in
  the flame and emit the Step-3 lerp into the iteration's color update.
- **`get_param(xform_id, variation_id, slot)`** indexes into the packed
  parameter buffer. User parameters live at slots
  `[0, parameters.len())`, init-derived parameters live at
  `[parameters.len(), parameters.len() + init_param_count)`.
- **3D auto-wrapper**: if `shader_3d` is `None`, the client generates a
  2D-pass-through wrapper. So a pure-2D variation needs only
  `shader_2d`; a true 3D variation should provide both.

Reference: signature builder in
[src/shader_builder_v2.rs](../../src/shader_builder_v2.rs) (search
`pre_variations`, `normal_variations`, `post_variations` codegen).

---

## 5. Categories

The client maps the `category` string via
`VariationCategory::from_api_str()`
([src/variations/mod.rs](../../src/variations/mod.rs)). Unknown strings
fall through to `Plugin`. Current recognised values (snake_case):

- `basic_2d`
- `advanced_2d`
- `depth_3d`
- `rotation_3d`
- `full_3d`
- `parameterized`
- `plugin` (default for anything unrecognised)

If the API wants a new category to show up as a first-class UI bucket,
this enum must learn about it on the client side too — coordinate
before shipping.

---

## 6. Open gap — state / accum metadata

The internal `VariationInfo` carries three additional fields that
**are not yet on the API wire format**:

| Field | Purpose | Default on API load |
| ----- | ------- | ------------------- |
| `state_count: usize` | Per-(thread, xform, variation) f32 state slots persisted across the inner iteration loop | `0` |
| `wgsl_source_state_init: Option<String>` | Optional WGSL fragment to seed state beyond the default zero-fill | `None` |
| `needs_accum: bool` | When true, fn signature gains an `accum` arg after `p` (the running result of prior variations in the iteration — cpp's `FPx/FPy/FPz`) | `false` |

The client conversion (`VariationInfo::from_download` in
[src/variations/mod.rs](../../src/variations/mod.rs)) explicitly
hardcodes these defaults with a note that the API contract hasn't been
extended. Built-in variations (`VariationInfo::from_def`) wire them
through correctly.

Design background:
[docs/projects/intra-iteration-state-and-accum.md](intra-iteration-state-and-accum.md).

**Proposed API-side extension** (additive, no breakage):

```jsonc
// new fields on VariationDownload
{
  // ...existing fields...
  "state_count": 0,                       // integer, defaults 0
  "shader_state_init": null,              // optional WGSL string
  "needs_accum": false                    // bool, defaults false
}
```

All three should serde-`default` so older clients keep working
unchanged. Once shipped, the client conversion drops the hardcoded
defaults and reads the real values.

Until then: any variation requiring state or accum reads must remain a
**built-in** on the client (registered through `VariationDef` /
`defs/`). The API can host stateless / non-accum variations today
without limitation.

---

## 7. Description and author metadata (planned)

Both fields are **presentation-only** — the client does *not* load
them into the in-memory `VariationInfo`. They live on the wire and in
the on-disk JSON cache for the variations panel to consume on demand.
This keeps the in-memory footprint flat regardless of how much prose
ships per variation.

| Field | Lives on | Purpose |
| ----- | -------- | ------- |
| `description: Option<String>` | `VariationListItem` + `VariationDownload` | Free-form prose for the variation overall. Already on `VariationDownload`; planned addition to `VariationListItem` so the registry browser can show it without per-variation fetches. |
| `authors: Vec<String>` | `VariationListItem` + `VariationDownload` | Original designer(s). Order-preserving (multiple authors are common — ports + extensions). Free-form `"Name (year)"` style; **not** a foreign key to `users` — these are historical attribution, not platform accounts. |
| `description: Option<String>` on `ApiVariationParameter` | `VariationDownload.parameters[*]` | Per-parameter help / tooltip prose. Param names alone (`hypergon`, `super_n3`, `popcorn2_3D_c`) are extremely obtuse without explanation. |

**Versioning policy.** The whole initial corpus ships at `version =
1`. The app and the API are released in lockstep — there's no in-flight
drift to invalidate against, so the version-bump-vs-no-bump nuance
that applied to a long-lived independently-versioned variation
registry doesn't gate Day-1 work. For forward-looking guidance only:
prose / author edits *don't* warrant a bump (no client behavior
change); WGSL or signature or `param_type` corrections *do* (would
change rendering or recompilation). See §8.

**Bulk import.** The initial values come from the client's
[src/variations/defs/](../../src/variations/defs/) corpus (494
variations across 133 files). Implementation plan is in
[VARIATIONS_BULK_METADATA_IMPORT.md](VARIATIONS_BULK_METADATA_IMPORT.md):
structured doc-comments on each `pub static VariationDef`, parsed by a
script that emits a one-shot SQL migration.

**Parameter type corrections ride the same migration.** Also the
natural moment to convert:
- ~38 parameters declared as `integer` with `[0, 1]` range → `boolean`
  (`filled`, `invert`, `inverse`, `xaxis`, `use_cos_x`, etc. — see §3
  adoption status).
- 15–25+ parameters declared as `integer` with small enumerated
  semantics → `enum` with proper `choices` labels (`falloff2.type`,
  `subflame_wf.color_mode`, `spirograph3D.mode`, etc.).

Pure wire-format changes (`parameters[*].param_type` field). **No
WGSL change** — storage stays `f32`, bodies that compare `mode == 0`
keep working. UI flips to checkbox / dropdown rendering.

---

## 8. Versioning and cache invalidation

`version: u32` is the cache-invalidation key the client *would* use if
server-side variation updates ever drifted from app releases. In
practice the app and API ship in lockstep, so the entire initial
corpus is `version = 1` and the field rarely changes.

- Client persists the full `VariationDownload` JSON in
  `<app_data>/variations/<name>.json` on desktop.
- WASM has no enumeration story for cached variations yet (TODO in
  [src/storage/variation_cache.rs](../../src/storage/variation_cache.rs)).
- Built-in (core) variations carry `version = 0` and never collide with
  API loads — the registry rejects an API payload whose `name` matches a
  core variation (logged, not fatal).
- The client refetches when a flame references a variation whose
  cached version is older than the version returned by the list
  endpoint. Bumping `version` is the only signal the client uses.
- Forward-looking bump policy (not in play for v1): WGSL / signature /
  `param_type` corrections *would* bump; description / author / prose
  edits would not. The field exists for future server-side updates;
  not used to gate initial population.

---

## 9. Compatibility status

- After the palette API redesign (commit
  `feaeb5b Palette API redesign: inline, content-addressable,
  hash-keyed library`) the variation wire format is **unchanged and
  fully functional**. Palettes and variations are orthogonal — neither
  references the other.
- The `needs_affine` → `needs_transform` rename (commit `4d84536`) is
  carried via serde alias on the client; the API can serve either name
  during the transition window.
- **In flight**: the description + authors additions in §7, the
  per-parameter `description` field, the int→bool / int→enum
  parameter type corrections, and the bulk import of the `defs/`
  corpus. All additive on the wire (serde-default) — no break for
  older clients.
- **Effects parity (coordination note, no wire format yet)**: the
  built-in shader effects ([src/effects/mod.rs](../../src/effects/mod.rs))
  reuse the same `ParamType` enum as variations and have the same gaps
  — `EffectParameter` has no `description`, the corpus has at least
  one int-as-enum (`blend_mode` 0..12 across 13 modes), and effects
  are not yet API-hosted. When effects move to the API, the wire
  format should mirror the variation contract one-for-one: same
  `ApiParamType`, same per-param `description`, same
  `description + authors` at the effect level. Decide once, apply
  twice.

---

## 10. When changing this doc

This file is the client's hand-rolled view of the contract. If you
change the wire shape on the client (`src/api/types.rs`) without
updating here, this doc will rot. Treat any edit to `VariationDownload`,
`VariationListItem`, `ApiVariationParameter`, `ApiParamType`,
`ApiVariationPhase`, or the shader-builder signature codegen as a
trigger to re-read this and update.
