# API: shared resources — variations, effects, scripts, palettes

**Status:** Plan. Nothing here is implemented yet.

Pairs with a server-side counterpart (`api-shared-resources-server.md`,
to be written in the API repo). That doc owns the schema, storage,
moderation and endpoints; this one owns what the client needs, what it
does with it, and the wire formats it can consume.

Scope: the four resource types the app can *share*, beyond the flame
CRUD that already works. Flames and animations are done and are not
discussed here except where they touch these.

| resource | today | wanted |
| --- | --- | --- |
| **variations** | download works; wire format is behind the engine | extend the schema, fix three bugs |
| **effects** | nothing (a dead stub type on both sides) | downloadable effects **carrying their own shader code** |
| **scripts** | nothing; web has no local storage at all | full CRUD |
| **palettes** | personal library; no standalone create | standalone save — *deferred, low priority* |

Explicitly **not** wanted: sharing parameter presets over the 15
built-in effects. Scripts cover that use case, and better.

Read-only by policy: **variations and effects are not user-editable.**
The API exposes list and fetch for them; no create, update or delete
from the client. Scripts are the opposite — full CRUD, owned by the
user.

---

## 0. Correct the stale spec first

`docs/main/openapi.json` was last touched 2026-02-28. `src/api/`
changed 2026-06-29 and `api-v2-server.md` is from the same June date,
after the v2 blob migration. **Two endpoints the client uses every day
are absent from that spec:**

- `GET /api/variations/{name}` — the live variation download path. The
  spec documents only `GET /api/variations`.
- The whole `/api/users/me/palettes` family (list, nickname, remove).
  The spec instead describes a different `/api/palettes` CRUD model.

Anyone working from `openapi.json` will build against a contract that
is four months stale. The authoritative description of what the client
speaks today is `src/api/mod.rs` plus `api-v2-server.md`. Regenerating
the spec from the live server should probably precede this work.

---

## 1. Cross-cutting: the WGSL trust boundary

**This is the decision that matters most, and it needs to be made
before either variations or effects proceed.**

Downloading a flame, a palette or an animation ships *data*. Downloading
a variation or an effect ships **code that runs on the GPU**. Those are
different risk classes, and the second one is already live: the client
fetches `shader_2d` / `shader_3d` from `/api/variations/{name}` and
compiles it with **no validation of any kind** — no length cap, no
structural check, nothing. Adding downloadable effects doubles down on
a boundary that was never designed.

What hostile WGSL can and cannot do, so the decision is made on facts:

- **Cannot** read across bindings or escape the GPU sandbox. naga and
  the browser validate the module and bounds-check array access; there
  is no memory-disclosure path to other buffers or to host memory.
- **Cannot** see anything secret through an effect: an effect's inputs
  are the rendered image and its own parameters.
- **Can** hang the GPU. A validated, well-formed shader may still
  contain an unbounded `loop {}`. The result is a device-lost / TDR
  driver reset, or a killed browser tab. This is exactly the class of
  failure the script-sandbox review just closed on the CPU side, and
  the GPU has no equivalent of an operation budget we can impose.
- **Can** hit driver bugs. Less predictable, not mitigable from here.

So the exposure is **availability, not confidentiality** — the same
conclusion the script review reached, and the same reason it still
deserves a policy rather than a shrug.

Three viable models. This plan assumes **(a)** unless the API repo says
otherwise:

- **(a) Server-curated.** Variations and effects are published only by
  reviewed/trusted authors; the API is a distribution channel, not a
  user upload surface. Matches the "not user-editable" rule already
  chosen, and costs the client nothing. **Recommended.**
- **(b) Open upload with client-side screening.** Requires a WGSL
  screening pass in the client (cap source length, reject unbounded
  loops, cap loop nesting). Partial by nature — a bounded loop of
  10⁹ iterations passes any cheap check.
- **(c) Open upload, unscreened.** Any user can hang any other user's
  GPU. Not defensible.

Whichever is chosen, the client should still apply cheap hygiene, and
this applies to **variations today**, not just future effects:

- a source-length cap on downloaded WGSL;
- a compile-failure path that reports and disables the resource rather
  than taking down the render (effects already skip unknown types; a
  *failed compile* needs the same treatment);
- surfacing "this flame uses a downloaded effect/variation" in the UI,
  so a user knows third-party code is about to run.

---

## 2. Variations — extend the schema, fix three bugs

The download path is **functional** and survived the variation
refactor: it was written against the runtime `VariationInfo` (owned
`String` WGSL), not the static `VariationDef`, and the global registry
is `Lazy<RwLock<…>>` with a `global_registry_mut()`. Fetch → disk cache
→ `register_from_api()` → shader rebuild all work, triggered
automatically when a loaded flame references an unknown variation.

### 2.1 The wire format is behind the engine

`VariationDownload` carries 3 of the engine's 13 `Feature` flags and
has no state-slot fields. Measured across the 646 shipped variations:

| missing from the wire format | variations using it |
| --- | --- |
| `AlwaysZ` | 173 |
| `Replace` | 47 |
| `CanHide` | 39 |
| `WritesRgb` | 20 |
| `NeverZ` | 16 |
| `NeedsW` | 12 |
| `NeedsAccum` | 11 |
| `NeedsMobiusLib` | 9 |
| `VolumeSideEmit`, `AnalyticBlur` | 2 each |
| `state_count > 0` | 27 |

**245 of 646 (38%)** use at least one thing the schema cannot express.
Nothing breaks today, because shipped variations are `is_core` and are
never downloaded — but a *server-authored* variation needing any of
them compiles and then **mis-renders with no warning**. `from_download`
hardcodes `state_count: 0` and the code already flags this as deferred.

### 2.2 Proposed schema extension (additive, back-compatible)

Add a `features` array that supersedes the three legacy bools when
present, so old payloads keep working:

```jsonc
{
  // ... existing fields unchanged ...

  // Authoritative when present. Falls back to needs_rng /
  // needs_transform / writes_color when absent.
  "features": ["needs_rng", "always_z", "replace"],

  // Per-thread state slots (get_state / set_state in the shader).
  "state_count": 0,
  "shader_state_init": null,

  // Foreign names for .flame import matching.
  "aliases": []
}
```

Feature names, all thirteen: `needs_rng`, `needs_transform`,
`writes_color`, `writes_rgb`, `needs_accum`, `always_z`, `never_z`,
`replace`, `can_hide`, `volume_side_emit`, `analytic_blur`, `needs_w`,
`needs_mobius_lib`. Their meanings live in
`src/variations/definition.rs` — the server needs only to carry them.

An unknown feature string should be **ignored with a warning**, not an
error, so a newer server can serve an older client.

### 2.3 Three bugs to fix client-side

1. **The WASM variation cache is write-only.** `list_cached()` is a
   stub returning empty, so `load_all()` finds nothing: web sessions
   re-download every variation every time, and the "Clear Cache" button
   reports 0 and does nothing. Needs a localStorage key index.
2. **Timed-out fetches leak into the next batch.**
   `trigger_variation_fetches` never clears `variation_fetch_results`,
   so a late arrival from an abandoned batch is drained by the next one
   and decrements its counter, finalizing it early. Also `finalize`
   ignores its `had_failures` argument, so a timeout shows the user
   nothing.
3. **A missing `shader_3d` silently contributes nothing in 3D.** The
   builder skips the variation rather than falling back to 2D. The skip
   is deliberate (it prevented vec2/vec3 crashes) but is invisible —
   it should surface.

Also: `list_variations()` is dead code with no callers. Either wire it
to a browse UI or delete it.

---

## 3. Effects — downloadable, carrying their own shader code

This is the substantial new build. **Effects have none of the plumbing
variations have**, and the variations subsystem is the working
blueprint for all of it.

### 3.1 What an effect is today

An effect in a config is a registry key plus a float map:

```json
{"effect_type": "vignette", "enabled": true, "params": {"intensity": 0.5}}
```

resolved against **15 compiled-in shaders** (12 color, 3 density),
loaded by `include_str!` on WASM or from the shipped `shaders/`
directory on desktop. `EffectInfo` holds a `shader_path: String` — a
path, never source. The registry is `Lazy<EffectRegistry>` with **no
`RwLock`**: unlike variations, there is no runtime-insertion door.

Two chains, ordered independently: `density_effects` run before
tonemap, `color_effects` after. Both are flat lists on `FractalConfig`
and already sync to the cloud inside a flame's opaque config blob —
what is missing is effects as *standalone shareable objects*.

### 3.2 Client work required

Roughly in dependency order:

1. **Make the registry mutable.** `Lazy<EffectRegistry>` →
   `Lazy<RwLock<EffectRegistry>>`, with `global_effect_registry()` and
   a new `global_effect_registry_mut()`, mirroring variations. 17 call
   sites across 5 files (`effects/mod.rs`, `renderer/effect_chain.rs`,
   `script/api.rs`, `ui/effects_panel.rs`, `ui/target_selector.rs`) —
   mechanical, since they become guard derefs.
2. **`EffectInfo` holds source or path.** Built-ins keep the path;
   downloaded ones carry owned WGSL. Add `is_core: bool` and
   `version: u32`, as `VariationInfo` has.
3. **Source-accepting compilation.** `load_effect_shader` currently
   takes a path only. Needs a source variant, and both must keep the
   `// INCLUDE_BLEND_MODES` splice of
   `shaders/effects/common/blend_modes.wgsl`.
4. **Compile-failure recovery.** A downloaded effect that fails to
   compile must disable itself with a visible message. Today an
   uncompiled effect logs a warning and is skipped — fine for an
   unknown type, not fine for a *broken download* the user is waiting
   on.
5. **Cache.** Mirror `storage/variation_cache.rs` — and implement the
   WASM key index properly this time, rather than inheriting §2.3's
   write-only bug.
6. **Fetch trigger.** Mirror `app/variation_fetch.rs`: on loading a
   config that references an unknown `effect_type`, pause, fetch,
   register, rebuild. The hook point exists — `effect_chain.rs` already
   detects unknown types, it just logs instead of fetching.
7. **Provenance in the UI.** The effects panel should mark downloaded
   effects the way the variations panel marks "API v#", and offer a
   cache clear.

### 3.3 Engine ceilings a downloaded effect must respect

Hard limits, and the server should validate against them at publish
time so a bad effect never reaches a client:

- **`MAX_EFFECT_PARAMS = 16`** — the per-effect uniform is a fixed
  `[[f32; 4]; 4]`. An effect declaring more cannot be represented.
- **`MAX_EFFECT_SLOTS = 32`** — effects per frame, across both chains.
- Parameters are **`f32` only** (`params: HashMap<String, f32>`).
  Booleans and enums are encoded as floats, as the built-ins do.
- An effect belongs to exactly one category, `density` or `color`,
  which decides where in the pipeline it runs.

### 3.4 Proposed wire format

Mirrors `VariationDownload`, which is the shape the client already
knows how to consume:

```jsonc
{
  "id": "…",
  "name": "warp_ripple",              // registry key, matches effect_type
  "display_name": "Warp Ripple",
  "description": "…",
  "category": "color",                 // "color" | "density"
  "version": 1,

  "parameters": [                      // max 16
    {
      "name": "intensity",
      "display_name": "Intensity",
      "param_type": "float",           // float | int | bool | enum
      "default_value": 0.5,
      "min_value": 0.0,
      "max_value": 1.0,
      "description": "…",
      "choices": null                  // enum only
    }
  ],

  "shader": "…WGSL…",
  "requires_blend_modes": true         // splice the common include
}
```

Endpoints (read-only, per the policy above):

```
GET /api/effects            → [{id, name, display_name, category, version, description}]
GET /api/effects/{name}     → the object above
```

Note the existing `GET /api/effects` returns a name catalog only — no
parameters, no shader — so it needs extending regardless, and a
by-name endpoint needs adding. The dead `Effect` struct in
`src/api/types.rs` should be replaced by `EffectDownload`, not
extended.

---

## 4. Scripts — full CRUD

The one resource the user owns and edits, so the only one with real
CRUD. Nothing exists on either side today.

### 4.1 Why this is more urgent than it looks

**On web there is no script persistence at all.** `user_script_dir`,
`save_user_script` and `delete_user_script` are all
`#[cfg(not(target_arch = "wasm32"))]`, and `discover()` on WASM returns
only the ten embedded scripts. A web user can write a script in the
panel and has **nowhere to put it** — reloading the page loses it.

For desktop the API is convenience and sharing. For web it would be
the only storage that exists. That argues for doing scripts before
effects, despite effects being the more interesting build.

### 4.2 What a script is

Source text, plus metadata the client *derives* by running the collect
pass: declared name, kind (`generator` | `modifier`), flags (`norng`,
`palette`), parameter declarations, and a description read from the
file's header comment. The server should **store** that metadata for
search and listing but must treat the **source as authoritative** —
the client re-derives on load, and a mismatch means the stored copy is
stale, not that the script is wrong.

Identity: locally a script's id is its file stem, and a user copy
shadows a shipped script of the same stem. The API needs its own
opaque id plus a `name`; the client maps between them. Worth deciding
with the API repo: whether a user's uploaded script can shadow a
built-in by name (locally it can, and that is the intended override
behaviour).

### 4.3 Proposed endpoints and shape

```
GET    /api/scripts              → the caller's scripts
POST   /api/scripts              → create
GET    /api/scripts/{id}         → fetch one (source included)
PUT    /api/scripts/{id}         → update
DELETE /api/scripts/{id}
GET    /api/search/scripts?q=…   → public browse
```

```jsonc
{
  "id": "…",
  "name": "grand_julian",            // stem-like key
  "display_name": "Grand Julian",    // from script("…") — derived
  "kind": "generator",               // derived; server stores for filtering
  "description": "…",                // from the header comment — derived
  "source": "…rhai…",                // authoritative
  "version": 3,
  "visibility": "private",           // private | unlisted | public
  "flags": ["norng"]                 // derived
}
```

### 4.4 Client work

- Local cache so scripts survive offline and load fast; on WASM this
  cache *is* the local store (localStorage), which also closes the
  "web can't save scripts" gap even before the API lands.
- Scripts panel: sign-in-aware list (mine / built-in / public),
  save-to-cloud, conflict handling on update.
- **Security note.** A downloaded script is *executable*, and the
  sandbox that runs it was just bounded (operation budget, input caps
  on the L-system builtins, transform and xaos ceilings). Those limits
  are what make running a stranger's script survivable, and they are
  CPU-side only — the deliberate gap recorded there is that the budget
  cannot see native work, which a wall-clock deadline would close.
  Worth doing before public script browsing ships.

---

## 5. Palettes — standalone save (deferred)

Today palettes are created *implicitly*: one rides inline on a flame
POST and the server hashes it by content. The client's own palette
endpoints are a **personal library** — list, nickname, remove — with no
standalone create. So "save this palette" without a flame has no path.

Wanted eventually, not a priority. When it happens it is small: the
server already has `POST /api/palettes` in the old spec and content
hashing in place; the client needs an upload call and a button in the
palette editor. Recorded here so it is not forgotten, and so the API
repo knows not to remove the endpoint.

---

## 6. Sequencing

Dependencies first, then value:

1. **Decide the WGSL trust model (§1).** Blocks effects, and changes
   what variations should be doing today. Costs nothing to decide.
2. **Regenerate the OpenAPI spec (§0).** Otherwise both sides build
   against a stale contract.
3. **Variation schema extension + the three bug fixes (§2).** Small,
   additive, and makes the existing feature honest.
4. **Scripts CRUD (§4).** Highest user-visible value, and the only way
   web users get script persistence at all. The local-cache half is
   worth doing even before the API exists.
5. **Downloadable effects (§3).** The largest build; benefits from
   scripts having exercised the pattern, and from the trust decision
   being settled.
6. **Standalone palette save (§5).** Whenever convenient.

## 7. Open questions for the API repo

1. **Trust model for shipped shader code** — curated, screened, or
   open? (§1) Everything else in effects follows from this.
2. Should the effect and variation catalogs be **versioned as a set**
   (a manifest with an etag) rather than fetched one at a time? The
   current one-at-a-time fetch is fine for the on-demand path but
   makes a browse UI chatty.
3. Can a user's uploaded **script shadow a built-in** by name, as a
   local user copy does?
4. Do downloaded resources need an **author/attribution** field the
   client should display? The variation panel shows only "API v#"
   today, and variation definitions in-tree carry `# Authors`.
5. Is `GET /api/variations/{name}` **intended** (it is what the client
   calls and works), or is the spec's list-only shape the real
   contract? (§0)
