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
    aliases: Vec<String>,               // serde(default); foreign-app names
                                        //   that resolve to this variation
                                        //   on import (e.g. `linear3D` for
                                        //   `linear`). See §10.
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

### 3.1 `shader_2d` stays required — mark the variation instead

`shader_2d` is `Option<String>` server-side and a required `String`
here, and `subflame_wf` violates it today: its shaders are
deliberately NULL, so `GET /api/variations/subflame_wf` would fail
this deserializer outright. Latent only because the client has it
built in and never fetches it.

**Recommendation: keep `shader_2d` required, and flag the variation on
the LIST as not downloadable.**

```jsonc
// VariationListItem
"downloadable": true   // false => built-in only, do not fetch detail
```

Why not the alternatives:

- **`Option<String>` on the client** deserializes cleanly and then
  fails *worse*. `VariationInfo.wgsl_source` is already `Option`, and
  the builder's response to `None` is `continue` — the variation is
  skipped and contributes nothing, silently. That is the same
  degradation class as the missing-`shader_3d` bug, which is on the
  fix list precisely because it is invisible. Trading a loud
  deserialization error for a silent no-op is the wrong direction.
- **Excluding shaderless variations from the catalog** makes it
  silently incomplete. `subflame_wf` is a variation users *can* use —
  the browsing app should document it and the panel should list it. It
  simply is not something to download.
- **Serving `""`** was correctly rejected upstream: it satisfies the
  type and moves the failure to shader compilation.

The flag is honest about what is actually true — *this variation is
built into the client; there is nothing to fetch* — and it matches the
rule `has_shader_3d` established: a list field earns its place when a
browse-or-select decision can be made from it. "Do not attempt to
download this" is exactly such a decision.

It also keeps the download contract clean: **if a variation is
downloadable, it has a shader.** No consumer has to handle a null one.

Note the property generalises beyond `subflame_wf` — see §4.1. Any
variation whose WGSL depends on engine infrastructure (subflame
buffers, name-gated helper libraries) is built-in-only for the same
reason, so the flag will not stay a one-row special case.

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
- **No 3D auto-wrapper.** This section previously claimed the client
  generates a 2D-pass-through wrapper when `shader_3d` is `None`. It
  does not — it **skips the variation entirely** in 3D flames, so the
  variation silently contributes nothing. The fallback was removed
  deliberately: it masked shader-validation crashes where a
  vec2-returning function got called from a vec3 accumulator. A
  variation meant to work in 3D must ship `shader_3d`. `has_shader_3d`
  on the list exists so this is visible before download rather than as
  a variation that renders nothing.

Reference: signature builder in
[src/shader_builder_v2.rs](../../src/shader_builder_v2.rs) (search
`pre_variations`, `normal_variations`, `post_variations` codegen).

### 4.1 A downloaded shader must be self-contained

The builder splices several helper libraries into the assembled shader,
but **only for variations it recognises by name**:

| helper | trigger |
| ------ | ------- |
| `noise.wgsl` | name is `dc_perlin`, `crackle`, `dc_crackle_wf` |
| `voronoi.wgsl` | name is `crackle`, `dc_crackle_wf` |
| `fractwf.wgsl` | name matches `fract_*_wf` |
| `subflame.wgsl` | the flame uses a subflame |
| `complex.wgsl` | always present |
| Möbius SL(2,C) lib | **`Feature::NeedsMobiusLib`** |

Consequence for curation: **a server-hosted variation cannot reach the
name-gated helpers.** Its WGSL must define everything it calls, apart
from the shader-wide primitives (`get_param`, `get_state`, `rng_*`,
`complex.wgsl`) and any library it can request declaratively. Today
`NeedsMobiusLib` is the only declarative one — which is a further
reason the `features` array of §6.1 matters: it is the sole mechanism
by which a downloaded variation can pull in infrastructure.

A shader referring to, say, `simplex_noise_3d` without being named
`crackle` compiles to an undefined-function error at pipeline build.
Worth validating at publish time if cheap; otherwise it is a curation
checklist item.

**This is why some variations cannot be served at all.** `subflame_wf`
has a perfectly real `wgsl_2d`, but it calls `subflame_iterate` and
reads the subflame storage buffers — engine infrastructure that cannot
travel over the wire. It is *built-in-only by nature*, not a variation
that happens to lack a shader. See §3.1 for how the wire format should
say so.

---

## 5. Categories

The client maps the `category` string via
`VariationCategory::from_api_str()`
([src/variations/mod.rs](../../src/variations/mod.rs)). Unknown strings
fall through to `Plugin`.

**Category is functional, not cosmetic.** `only_3d` is dropped from 2D
shaders (`ShaderBuilder::active_with_local_indices`) — it names
variations with no meaningful 2D reading at all. Every other category
is UI grouping. Getting `only_3d` wrong means a variation compiled into
a shader where it cannot work; getting the others wrong only misfiles
it in the panel.

### The canonical vocabulary

| wire string | client variant | shipped variations |
| ----------- | -------------- | -----------------: |
| `basic_2d` | `Basic2D` | 7 |
| `advanced_2d` | `Advanced2D` | 415 |
| `depth_3d` | `Depth3D` | 17 |
| `rotation_3d` | `Rotation3D` | 6 |
| `full_3d` | `Full3D` | 114 |
| `only_3d` | `Only3D` | 0 (defined, unused) |
| `plugin` | `Plugin` (default) | 87 |

Counts measured across the 646 shipped variations on 2026-07-31.
`from_api_str` also accepts the no-underscore spellings (`basic2d`, …).
`to_api_str` is the inverse; a round-trip test asserts every variant
survives both directions.

### Divergence found 2026-07-31, and how it resolves

The server was serving a different vocabulary — `basic` (53 rows),
`parametric` (30), `3d`, plus `pre` / `post` / `blur` — of which only
`advanced_2d` and `plugin` (1 row each) matched. Everything else fell
through to `Plugin`, so the panel had no working grouping. Invisible
only because `list_variations()` is still dead code; the manifest work
is exactly what would have exposed it.

**Resolution: the client enum is canonical, and the bulk import carries
the categories already assigned in `defs/`.** This is not two
vocabularies to reconcile — the server's ~118 rows are the set the
import replaces with 646, so its spellings are superseded rather than
negotiated. No hand reclassification is needed, and the lossy `3d`
collapse dissolves on its own: those rows get real `depth_3d` /
`rotation_3d` / `full_3d` values from the definitions.

Two of the old values are not categories at all and should not become
any:

* `pre` / `post` — that is **phase**, which is already its own field
  (`ApiVariationPhase`, §4). A variation has both.
* `blur` — an effect-kind notion with no category slot; those rows take
  whatever their definition assigns.

Client-side bugs this surfaced, both now fixed:

* This section previously listed `parameterized` as recognised. It
  never was — there is no such arm and no such enum variant, so those
  30 server rows were becoming `Plugin` exactly like the rest. The doc
  was wrong, not the code.
* `Only3D` had **no wire spelling at all**, which made the one
  functionally significant category unexpressable: a downloaded
  variation in it arrived as `Plugin` and was compiled into 2D shaders.
  Latent only because nothing ships in that category yet.

The legacy `"3d"` spelling still parses, to `Depth3D`, so old payloads
keep working — but it is wrong for the 114 `Full3D` variations and
should not be sent after the import.

### Nullability

`category` is a required `String` on `VariationListItem` and
`VariationDownload`, with no `serde(default)`. **A single NULL row
would fail deserialization of the entire list response**, not just that
row. The column must be `NOT NULL` server-side. (If it ever needs to be
optional, the client field has to gain a default in the same release —
not after.)

If the API wants a new category to be a first-class UI bucket, this
enum must learn it too — coordinate before shipping.

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

### 6.1 The gap is wider than three fields

Measured across the 646 shipped variations (2026-07-31). The wire
format carries **3 of the engine's 13 `Feature` flags** — `needs_rng`,
`needs_transform`, `writes_color`. Everything else defaults to absent:

| Feature | On the wire? | Shipped variations using it |
| ------- | ------------ | --------------------------- |
| `NeedsRng`, `NeedsTransform`, `WritesColor` | yes | 297 / 123 / 77 |
| `AlwaysZ` | **no** | 173 |
| `Replace` | **no** | 47 |
| `CanHide` | **no** | 39 |
| `WritesRgb` | **no** | 20 |
| `NeverZ` | **no** | 16 |
| `NeedsW` | **no** | 12 |
| `NeedsAccum` | **no** (§6) | 11 |
| `NeedsMobiusLib` | **no** | 9 |
| `VolumeSideEmit` | **no** | 2 |
| `AnalyticBlur` | **no** | 2 |
| `state_count > 0` | **no** (§6) | 27 |

**245 of 646 (38%)** use at least one thing the wire format cannot
express. To be precise about the risk: nothing breaks today, because
shipped variations are built-in and never travel over the wire. The
failure mode is a **server-hosted variation** needing one of these — it
deserializes, compiles, and then **renders incorrectly with no
warning**, because the missing flag silently changes the generated
function signature or the plot behaviour.

`AlwaysZ` is the one to note: 173 variations, and its absence means a
variation's z contribution is zeroed whenever `preserve_z` is false.

**Proposed shape** — a single array supersedes the three legacy bools
when present, so old payloads keep working and future flags need no
further schema changes:

```jsonc
"features": ["needs_rng", "always_z", "replace"]
```

All thirteen names: `needs_rng`, `needs_transform`, `writes_color`,
`writes_rgb`, `needs_accum`, `always_z`, `never_z`, `replace`,
`can_hide`, `volume_side_emit`, `analytic_blur`, `needs_w`,
`needs_mobius_lib`. Semantics live in
[src/variations/definition.rs](../../src/variations/definition.rs); the
server only carries them. **An unknown feature string should be ignored
with a warning, not rejected**, so a newer server can serve an older
client.

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
| `aliases: Vec<String>` | `VariationListItem` + `VariationDownload` | Foreign-app names that resolve to this variation on `.flame` XML import (e.g. `linear3D` for our `linear`). See §10. |
| `description_plain: Option<String>` | `VariationListItem` + `VariationDownload` | The same prose with markdown syntax stripped, for clients that do not render markdown. See below. |

**`description` is markdown; `description_plain` is what this app
shows.** The browsing app renders markdown properly. This app has no
markdown renderer and is not getting one — a dependency and a rendering
pass are not worth it for a paragraph in a side panel. So the server
carries **both**: the markdown source, and a plain-text version with
the syntax characters stripped.

Stripping server-side rather than client-side is deliberate: it happens
once, both consumers agree on the result, and neither client needs
markdown-parsing code. If `description_plain` is absent the client falls
back to showing `description` raw, which degrades readably — headings
and emphasis just appear as literal `#` and `*`.

The same pair applies to per-parameter descriptions and to effects and
scripts when they land (§9).

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
  parameter type corrections, the `aliases` field for foreign-app
  name compatibility (§10), and the bulk import of the `defs/`
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

  **One thing must NOT be applied twice: the parameter cap.** A
  variation's parameters are packed into a dynamically sized buffer, so
  its limit is a policy choice — 512 per variation, chosen so no single
  one monopolizes the 1600-slot budget the flame's whole active set
  shares (largest today: `su_custom` at 259). An effect's parameters
  live in a **fixed-size uniform**, so its limit is physical capacity:
  `MAX_EFFECT_PARAMS = 48`, raised from 16 on 2026-07-31. The API's
  effect check must equal 48 exactly. Accepting more would let effects
  upload cleanly and then render wrong, since parameters past the end
  of the uniform are silently dropped. See
  [api-shared-resources.md §4.3](api-shared-resources.md).

---

## 10. Aliases for foreign-app name compatibility

The `aliases: Vec<String>` field on `VariationDownload` (and
`VariationListItem`) is a list of foreign-app names that resolve to
*this* variation on `.flame` XML import. Used for the cases where
another fractal flame app (Apophysis 7X, JWildfire, Chaotica) ships
the same-shaped variation under a different name.

**Canonical example.** Apophysis 7X and JWildfire have a separate
`linear3D` variation; ours `linear` handles both 2D and 3D modes
from the same definition. Without an alias, an imported flame's
`linear3D="…"` attribute hits the registry's name lookup, misses,
and is silently dropped. With `aliases: ["linear3D"]` on the
`linear` payload, the lookup resolves through the alias index and
the attribute lands on the imported transform.

**When to use an alias vs. just renaming.** Aliases are for the
case where our variation is *deliberately shaped differently* from
the foreign-app version (different math, different scope — e.g.
our unified `linear` vs Apo/JWildfire's split `linear` / `linear3D`).
When our variation is the *same* shape as the foreign-app version
and we just spelled the name wrong (casing typos like our
`curl3d` vs upstream `curl3D`), the right answer is to rename ours
to match upstream — not to alias.

**Wire shape.**

```rust
struct VariationDownload {
    name: String,           // canonical name (we own this)
    aliases: Vec<String>,   // foreign-app names that resolve to `name`
    // …
}
```

`aliases` is **deserialized with `#[serde(default)]`** so older payloads
(or older cache files) without the field still parse — the missing
field becomes `Vec::new()` and the variation registers without any
aliases. New clients/servers always include the field. No `version`
bump needed; this is purely additive in both directions.

**Client-side handling.** The registry builds a parallel
`HashMap<String, String>` (alias → canonical name) at registration
time. `registry.get(name)` consults the alias map on a primary miss.
A `resolve_alias(name)` helper returns the canonical name (or the
input unchanged if no alias matches), useful at import time so the
in-memory `Transform` stores the canonical name and the rest of the
pipeline never sees the alias.

**Conflict rules.**

- An alias that matches an existing canonical variation name is
  rejected (logged, not fatal) — we'd be silently overriding a real
  variation otherwise.
- An alias that's already mapped to a different variation is
  rejected (first registration wins, duplicate is logged).

**Server-side.** The `variations` table needs an `aliases TEXT[]
NOT NULL DEFAULT '{}'` column. Backfill is empty for existing
rows; subsequent migrations populate where needed. See
[api-v2-server-side.md](api-v2-server-side.md) for the SQL plan.

---

## 11. When changing this doc

This file is the client's hand-rolled view of the contract. If you
change the wire shape on the client (`src/api/types.rs`) without
updating here, this doc will rot. Treat any edit to `VariationDownload`,
`VariationListItem`, `ApiVariationParameter`, `ApiParamType`,
`ApiVariationPhase`, or the shader-builder signature codegen as a
trigger to re-read this and update.
