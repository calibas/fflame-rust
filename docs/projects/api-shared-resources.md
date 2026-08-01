# API: shared resources — variations, effects, scripts, palettes

**Status:** Plan. Nothing here is implemented yet.

Pairs with a server-side counterpart (`api-shared-resources-server.md`,
to be written in the API repo). That doc owns the schema, storage,
curation and endpoints; this one owns what the client needs, what it
does with it, and the wire formats it can consume.

Scope: the four resource types the app can *share*, beyond the flame
CRUD that already works. Flames and animations are done and are not
discussed here except where they touch these.

| resource | today | wanted |
| --- | --- | --- |
| **variations** | download works; wire format is behind the engine | extend the schema, manifest-driven catalog, fix three bugs |
| **effects** | nothing (a dead stub type on both sides) | downloadable effects **carrying their own shader code** |
| **scripts** | nothing; web has no local storage at all | full CRUD |
| **palettes** | personal library; no standalone create | standalone save — *deferred, low priority* |

Explicitly **not** wanted: sharing parameter presets over the built-in
effects. Scripts cover that use case, and better.

---

## 0. Decisions

These were open; they are now settled and the plan below assumes them.

1. **Variations and effects are server-curated only.** No user upload
   path in the API. Users who want custom ones build them as
   **local-only plugins** (§2) and submit through other channels for
   curation. This resolves the WGSL trust question (§1) by keeping the
   distribution channel closed.
2. **Catalogs are manifest-driven.** The Variations and Effects panels
   list *everything available*, including entries not yet downloaded,
   from a manifest fetched from the server.
3. **No shadowing of built-ins, for any resource type.** Built-ins are
   not editable. Editing one produces a renamed copy and switches to
   it. Variations already behave this way; **scripts do not and must
   change** (§4.2).
4. **Everything carries `author` and a markdown `description`.** These
   feed the separate browsing app as well as the in-app panels, and
   apply to variations, effects and scripts alike.
5. **`GET /api/variations/{name}` is intended and stays.** It is what
   the client calls and what the browsing app uses for detail views.
   The spec is simply stale (§0.1).

### 0.1 Correct the stale spec first

`docs/main/openapi.json` was last touched 2026-02-28. `src/api/`
changed 2026-06-29 and `api-v2-server.md` is from the same June date,
after the v2 blob migration. **Two endpoints the client uses every day
are absent from that spec:**

- `GET /api/variations/{name}` — the live variation download path, now
  confirmed intended. The spec documents only `GET /api/variations`.
- The whole `/api/users/me/palettes` family (list, nickname, remove).
  The spec instead describes a different `/api/palettes` CRUD model.

Anyone working from `openapi.json` will build against a contract four
months stale. The authoritative description of what the client speaks
today is `src/api/mod.rs` plus `api-v2-server.md`. Regenerating the
spec from the live server should probably precede this work.

---

## 1. The WGSL trust boundary — resolved by curation

Downloading a flame, palette or animation ships *data*. Downloading a
variation or effect ships **code that runs on the GPU**. Worth stating
plainly what that does and does not expose, because it justifies the
decision and bounds what still needs doing:

- **Cannot** read across bindings or escape the GPU sandbox. naga and
  the browser validate the module and bounds-check array access; there
  is no memory-disclosure path to other buffers or host memory.
- **Cannot** see anything secret through an effect — its inputs are the
  rendered image and its own parameters.
- **Can** hang the GPU. A validated, well-formed shader may still
  contain an unbounded `loop {}`, giving a device-lost/TDR reset or a
  killed browser tab. There is no GPU equivalent of the operation
  budget the script sandbox uses.

So the exposure is **availability, not confidentiality** — the same
conclusion the script-sandbox review reached.

**Server curation is the mitigation.** Because nothing reaches a client
that a curator did not publish, the client does not need a WGSL
screening pass, and this plan does not propose one.

Two things are still worth doing, and they apply to **variations
today**, not only to future effects. The client currently compiles
downloaded WGSL with no checks at all:

- **A compile-failure path that degrades, not crashes.** A resource
  whose shader fails to compile must disable itself with a visible
  message. Effects already skip *unknown* types; a *failed compile* on
  a resource the user is waiting for needs the same treatment plus a
  report.
- **Provenance in the UI.** A user should be able to see that a flame
  is about to run third-party code — built-in vs downloaded vs local.
  The variations panel shows "API v#" today; effects show nothing.

A source-length sanity cap is cheap and worth adding, but under
curation it is hygiene rather than a security control.

---

## 2. Local-only plugins (new)

The counterpart to curation: users who write their own variations and
effects need somewhere to put them that never touches the API.

**Design intent:** a local plugin is the *same object* as a downloaded
one, from a different source. That keeps one registration path, one
compile path and one set of ceilings rather than a parallel system.

- **Format:** the same JSON as the download payload
  (`VariationDownload` / `EffectDownload`). A curator submission is
  then literally the file the user already has, which makes the
  "submit through other channels" route frictionless.
- **Location:** desktop, a plugins folder beside the user scripts dir
  (`<app_data>/plugins/variations/`, `.../effects/`). WASM,
  localStorage under a distinct key prefix — deliberately *not* the
  download cache, so clearing the cache never destroys the user's own
  work.
- **Registration:** extend the existing runtime path. `register_from_api`
  becomes source-tagged (`Builtin | Api | Local`) rather than gaining a
  parallel `register_from_local`. `VariationInfo.is_core` grows into
  that enum; the effects registry gains the same field when it becomes
  mutable (§3.2).
- **Never uploaded.** No code path should send a local plugin to the
  API.
- **Name collisions are rejected, not shadowed** (§0 decision 3). A
  local plugin whose name matches a built-in or a catalog entry fails
  to load with a message naming the conflict. This is what variations
  already do for API registrations.

**One consequence to design for:** a flame using a local-only plugin
will not render for anyone else, and will not render for the same user
on another device. Saving or uploading such a flame should warn, and
the missing-resource path (§3.2 item 6) should distinguish "this needs
downloading" from "this needs a plugin you do not have."

---

## 3. Variations

The download path is **functional** and survived the variation
refactor: it was written against the runtime `VariationInfo` (owned
`String` WGSL), not the static `VariationDef`, and the global registry
is `Lazy<RwLock<…>>` with a `global_registry_mut()`. Fetch → disk cache
→ `register_from_api()` → shader rebuild all work, triggered
automatically when a loaded flame references an unknown variation.

### 3.1 The wire format is behind the engine

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
To be precise about what that means: nothing breaks today, because
shipped variations are built-in and never downloaded. The risk is a
*curated server-side* variation needing one of them — it compiles and
then **mis-renders with no warning**. `from_download` hardcodes
`state_count: 0`, and the code already flags this as deferred.

### 3.2 The schema lives in VARIATIONS_WIRE_FORMAT.md

**[VARIATIONS_WIRE_FORMAT.md](VARIATIONS_WIRE_FORMAT.md) is the
canonical client↔API variation contract** — endpoints, payloads,
parameter types, the WGSL function contract, versioning, aliases. It
predates this plan and stays authoritative; do not restate its schema
here.

What this review contributed back into it:

- **§6.1** — the gap is wider than the three fields §6 listed. The full
  13-feature measurement above, the 38% figure, the proposed `features`
  array, and the "ignore unknown features with a warning" rule.
- **§7** — `description_plain` alongside the markdown `description`,
  per decision 4, with the reasoning for stripping server-side.

Its §9 already anticipated effects: *"When effects move to the API, the
wire format should mirror the variation contract one-for-one... Decide
once, apply twice."* §4.4 below follows that instruction.

### 3.3 Manifest-driven catalog

`GET /api/variations` already exists and `list_variations()` is already
written — it is currently dead code with no callers. Decision 2 gives
it a purpose: it becomes the manifest behind the Variations panel.

```jsonc
// GET /api/variations
{
  "version": "…",        // etag or catalog version, for cheap revalidation
  "variations": [
    { "id": "…", "name": "…", "display_name": "…", "category": "…",
      "version": 3, "author": "…", "description": "…markdown…" }
  ]
}
```

Client behaviour:

- Fetch on startup or on panel open; cache with the version/etag so
  revalidation is a conditional request.
- The panel lists **every** entry, with a state per row: built-in,
  local plugin, downloaded (with version), or available-not-downloaded.
- Downloading is triggered by use or by an explicit control; the
  existing on-demand fetch (a flame referencing an unknown variation)
  stays as the automatic path.
- The manifest must work **offline** — a failed fetch shows built-ins,
  local plugins and previously downloaded entries, not an error page.
- A catalog entry whose `version` exceeds the cached copy's should be
  offered as an update. (Today `register_from_api` replaces silently.)

### 3.4 Three bugs to fix client-side

1. **The WASM variation cache is write-only.** `list_cached()` is a
   stub returning empty, so `load_all()` finds nothing: web sessions
   re-download everything every time, and "Clear Cache" reports 0 and
   does nothing. Needs a localStorage key index.
2. **Timed-out fetches leak into the next batch.**
   `trigger_variation_fetches` never clears `variation_fetch_results`,
   so a late arrival from an abandoned batch is drained by the next one
   and decrements its counter, finalizing it early. `finalize` also
   ignores its `had_failures` argument, so a timeout tells the user
   nothing.
3. **A missing `shader_3d` silently contributes nothing in 3D.** The
   builder skips the variation rather than falling back to 2D. The skip
   is deliberate (it prevented vec2/vec3 crashes) but invisible — with
   a manifest listing 2D-only entries, this should be visible up front.

---

## 4. Effects — downloadable, carrying their own shader code

The substantial new build. **Effects have none of the plumbing
variations have**, and the variations subsystem is the blueprint for
all of it.

### 4.1 What an effect is today

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

### 4.2 Client work required

Roughly in dependency order:

1. **Make the registry mutable.** `Lazy<EffectRegistry>` →
   `Lazy<RwLock<EffectRegistry>>`, plus `global_effect_registry_mut()`,
   mirroring variations. 17 call sites across 5 files
   (`effects/mod.rs`, `renderer/effect_chain.rs`, `script/api.rs`,
   `ui/effects_panel.rs`, `ui/target_selector.rs`) — mechanical, since
   they become guard derefs.
2. **`EffectInfo` holds source or path**, plus a source tag
   (`Builtin | Api | Local`, §2) and `version: u32`.
3. **Source-accepting compilation.** `load_effect_shader` takes a path
   only. It needs a source variant, and both must keep the
   `// INCLUDE_BLEND_MODES` splice of
   `shaders/effects/common/blend_modes.wgsl`.
4. **Compile-failure recovery** (§1) — disable and report, do not take
   down the render.
5. **Cache**, mirroring `storage/variation_cache.rs` — with the WASM
   key index done properly rather than inheriting §3.4's bug.
6. **Fetch trigger**, mirroring `app/variation_fetch.rs`: a config
   referencing an unknown `effect_type` pauses, fetches, registers,
   rebuilds. The hook exists — `effect_chain.rs` already detects
   unknown types, it just logs instead of fetching. It must
   distinguish a downloadable entry from a missing local plugin (§2).
7. **Manifest + panel**, matching §3.3: `GET /api/effects` returns the
   catalog, the panel lists everything with per-row state and
   provenance.

### 4.3 Engine ceilings a downloaded effect must respect

Hard limits. The server should validate against them at publish time
so a bad effect never reaches a client:

- **`MAX_EFFECT_PARAMS = 48`** — the per-effect uniform is a fixed
  `[[f32; 4]; 12]`. An effect declaring more cannot be represented;
  the extras are silently dropped.
- **`MAX_EFFECT_SLOTS = 32`** — effects per frame, across both chains.
- Parameters are **`f32` only** (`params: HashMap<String, f32>`).
  Booleans and enums encode as floats, as the built-ins do.
- Exactly one category, `density` or `color`, deciding pipeline
  position.

**The effect cap is the opposite kind of number to the variation one,
and confusing them would be expensive.** A variation's parameters go
into a dynamically packed buffer, so its old 16 was obsolete and the
cap is a policy choice (512, well under the 1600 the engine can
actually hold). An effect's parameters go into a *fixed-size uniform*,
so 48 is physical capacity. **The API's check must equal it, never
exceed it** — an API that accepts more produces effects that upload
cleanly and then render wrong, which is the worst failure shape
available here.

Raised from 16 to 48 on 2026-07-31, ahead of effects that need it. It
was free: the slot stride is 256 B (set by the uniform offset
alignment, not by the struct), so anything up to 240 B of parameters
costs nothing and the buffer stays 32 × 256 B = 8 KiB. Beyond ~60 it
stops being free *and* stops being possible — 512 parameters would
need ~72 KiB, over the 64 KiB `max_uniform_buffer_binding_size` of a
default device and 4.5× the 16 KiB the WASM build requests. That would
mean moving effect parameters to a storage buffer: a real change, not
a constant bump. A `const` assertion in `effect_chain.rs` fails the
build if the struct ever outgrows the stride.

### 4.4 Proposed wire format

Mirrors `VariationDownload`, the shape the client already consumes:

```jsonc
{
  "id": "…",
  "name": "warp_ripple",              // registry key, matches effect_type
  "display_name": "Warp Ripple",
  "author": "…",
  "description": "…markdown…",
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

Endpoints (read-only, per decision 1):

```
GET /api/effects            → manifest (no shader source)
GET /api/effects/{name}     → the object above
```

The existing `GET /api/effects` returns a name catalog only — no
parameters, no shader — so it needs extending regardless, and a
by-name endpoint needs adding. The dead `Effect` struct in
`src/api/types.rs` should be **replaced** by `EffectDownload`, not
extended.

---

## 5. Scripts — full CRUD

The one resource the user owns and edits, so the only one with real
CRUD. Nothing exists on either side today.

### 5.1 Why this is more urgent than it looks

**On web there is no script persistence at all.** `user_script_dir`,
`save_user_script` and `delete_user_script` are all
`#[cfg(not(target_arch = "wasm32"))]`, and `discover()` on WASM returns
only the ten embedded scripts. A web user can write a script in the
panel and has **nowhere to put it** — reloading the page loses it.

For desktop the API is convenience and sharing. For web it would be
the only storage that exists. That argues for scripts before effects,
despite effects being the more interesting build.

### 5.2 Removing built-in shadowing (behaviour change)

Per decision 3. Scripts are the one place shadowing is still active:
`discover()` builds its map with "later wins on a name clash", so a
user copy of `basic_random.rhai` replaces the shipped one and inherits
its id. That is currently *documented as intended*, and the doc comment
on `ScriptEntry::id` should be updated along with the behaviour.

Target behaviour, matching what variations already do:

- Built-in scripts are **read-only**. Editing one is not blocked at the
  keystroke; instead the first change **forks it** — a copy under a new
  name (`basic_random-copy`, or a user-supplied name), and the editor
  switches to the copy. The user keeps editing without interruption and
  the built-in is untouched.
- A user script whose stem collides with a built-in **fails to load**
  with a message naming the conflict, rather than silently overriding.
- Same rule for API-sourced scripts and local plugins.

**Do this now, before anyone depends on it.** Scripting has not
shipped to users, so there are no shadowing copies in the wild and no
migration path is needed — just the behaviour change and the doc
comment. Deferring it is what would create a migration.

Note the cost this removes: today "shipped script FILENAMES are a
public API" because `run_script(id)` resolves by stem and a user copy
can capture that name. Without shadowing, a built-in id always means
the built-in — which is what `run_script` cross-calls actually want.

### 5.3 What a script is

Source text, plus metadata the client *derives* by running the collect
pass: declared name, kind (`generator` | `modifier`), flags (`norng`,
`palette`), parameter declarations, and a description read from the
file's header comment. The server should **store** that metadata for
search and listing but treat the **source as authoritative** — the
client re-derives on load, and a mismatch means the stored copy is
stale, not that the script is wrong.

### 5.4 Proposed endpoints and shape

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
  "kind": "generator",               // derived; stored for filtering
  "author": "…",
  "description": "…markdown…",       // from the header comment — derived
  "source": "…rhai…",                // authoritative
  "version": 3,
  "visibility": "private",           // private | unlisted | public
  "flags": ["norng"]                 // derived
}
```

### 5.5 Client work

- **Local cache** so scripts survive offline and load fast. On WASM
  this cache *is* the local store (localStorage), which closes the
  "web cannot save scripts" gap **even before the API lands** — worth
  doing first and independently.
- Scripts panel: sign-in-aware list (mine / built-in / public),
  save-to-cloud, conflict handling on update, fork-on-edit (§5.2).
- **Security note.** Unlike variations and effects, scripts are *user
  content* — public browsing means running strangers' code. The
  sandbox that makes that survivable was recently bounded (operation
  budget, input caps on the L-system builtins, transform and xaos
  ceilings). The recorded gap is that the budget cannot see native
  work, which a wall-clock deadline would close. Worth doing **before**
  public script browsing ships, not after.

---

## 6. Palettes — standalone save (deferred)

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

## 7. Cross-cutting client work

### 7.1 Descriptions — no markdown renderer, ever

**Decided:** this app is not getting a markdown renderer. The server
carries `description` (markdown, for the browsing app) *and*
`description_plain` (syntax stripped). The app shows the plain version;
if it is absent it shows the raw markdown, which degrades readably.

Stripping happens **server-side, once**, so both consumers agree and
neither client carries markdown-parsing code. Recorded in
[VARIATIONS_WIRE_FORMAT.md §7](VARIATIONS_WIRE_FORMAT.md); applies
identically to effects and scripts.

The client work is therefore just *display*: the variations panel shows
no description at all today, so the field needs a home in the UI
regardless of format.

### 7.2 A shared "downloadable resource" shape

Variations and effects will have near-identical machinery: manifest +
cache + on-demand fetch + runtime registration + provenance + local
plugins. Scripts share the cache and provenance halves. Building
effects by copying `variation_fetch.rs` and `variation_cache.rs`
wholesale would triple-maintain three copies of the same bugs — §3.4's
write-only WASM cache is exactly the sort of thing that would be
duplicated.

Worth factoring the common parts once, when effects are built, rather
than after. Not worth factoring speculatively before then.

---

## 8. Client-side task list

Everything this side needs, grouped by workstream. Sizes are rough:
**S** = a sitting, **M** = a day or two, **L** = a project.
Tick as they land; add freely.

### 8.0 Do first — cheap and unblocking

- [x] **S** — ~~Regenerate `docs/main/openapi.json`~~ **DONE.** All 14
  paths the client calls are declared with matching methods, verified
  by a bidirectional comparison. Landed with the live server
  (§0.1). Four months stale, missing two endpoints the client uses
  daily. Both sides are otherwise building against fiction.
- [x] **S** — ~~Remove built-in shadowing for scripts~~ **DONE**
  (`750bac4d`). Turned out to be a live bug, not tidiness:
  `random_palette.rhai` declares its own stem, so one Save hijacked
  `basic_random`'s `run_script("random_palette")` call.

### 8.1 Correctness, independent of any new feature

These are live defects, not future work.

- [x] **S** — ~~WASM variation cache is write-only~~ **DONE.** The root
  cause was that the storage backend had no enumeration primitive at
  all; localStorage does support it (`length`/`key`), it had just been
  stubbed with a TODO. Added `backend::list_entries`, which collapses
  the desktop/WASM split in `list_cached` into one implementation and
  is reusable for the effects cache and script store.
- [x] **S** — ~~Timed-out fetches leak into the next batch~~ **DONE.**
  Clearing the vector on trigger is NOT sufficient — a straggler thread
  pushes after the clear — so results now carry a batch epoch and
  mismatches are discarded. `finalize`'s ignored `had_failures` flag is
  gone: each call site shows its own message, and a timeout now names
  what did not arrive instead of resuming in silence.
- [x] **S** — ~~A missing `shader_3d` silently drops the variation in
  3D~~ **DONE.** The skip is correct (the 2D fallback masked vec2/vec3
  validation crashes); the silence was not. Two independent halves: the
  build logs when it actually happens, and the Variations panel marks
  2D-only entries from the registry — no plumbing out of the shader
  builder. The panel half is also what `has_shader_3d` will feed once
  the manifest lands (8.2).
- [x] **M** — ~~Compile-failure path~~ **DONE, in two halves.**
  `create_shader_module` handed bad WGSL straight to wgpu, whose
  default handler PANICS — one downloaded variation with broken shader
  code killed the app, pointing at a line of generated source the user
  has never seen. Now: `gpu::device` installs an uncaptured-error
  handler so a bad shader costs the render, not the session; and
  `ShaderCache::validate_wgsl` parses the assembled WGSL with naga
  first, naming the registered downloaded variations as likely culprits
  and suggesting Clear Cache.

  **Desktop only for the naming half** — the wasm build configures
  `wgpu::naga` out (the browser is the WGSL front end there), and
  bundling a parser purely to pre-check would tax the binary the
  Endless Gallery size budget protects. On web the handler still keeps
  the session alive; the message just arrives unattributed.

  Syntax only. A semantic error (calling a helper the flame did not
  splice in — see WIRE_FORMAT §4.1) still reaches the device; catching
  those needs a full `naga::valid::Validator` pass, worth adding if
  downloaded shaders start failing that way.
- [x] **S** — ~~Provenance in the UI~~ **DONE for variations.** The
  per-row `API v#` tag was already there and is the wrong instrument
  for §1's question — "is this flame about to run third-party code"
  was answerable only by scrolling 646 rows. The panel now leads with
  the downloaded variations *this flame* uses, extracted as a pure
  `downloaded_variations_in_use` so the rule is testable rather than
  buried in layout code. An unknown name is deliberately NOT reported:
  it is missing, not untrusted, and the fetch path owns it.

  **Effects deferred, on purpose.** Every effect is built-in today, so
  a provenance marker would have exactly one value — the same
  no-browse-or-select-decision argument that kept `state_count` off the
  list payload. It lands with 8.5, where the source tag becomes real.
- [x] **S** — ~~Measure stored flames against `transforms_per_flame`~~
  **DONE on the API side** — their
  `no_stored_flame_exceeds_engine_transform_cap` reads the cap from the
  generated contract and passes on dev, so nothing stored blocks
  tightening. The prod query is recorded beside it. Original note:
  (128, shared across normals + linked + final). The API's schema is
  more permissive — 100 per pool, so 300 — and the engine PANICS on the
  total, not per pool. Tightening is free if nothing stored exceeds 128
  and a decision if anything does; either way the measurement comes
  first, since a tighter rule would reject flames that were valid when
  saved. `variations_per_flame` (100) and `variation_slots_per_flame`
  (1600) are unchecked server-side too.
- [x] **M** — ~~Wall-clock deadline inside the L-system builtin walks~~
  **DONE — as a STEP budget, not a clock.** Writing it down as
  wall-clock was a mistake: script + seed must name one exact flame on
  every machine, so an abort that depends on host speed makes the same
  script succeed on a desktop and fail on a phone. That would have
  traded a DoS bound for the determinism guarantee the rest of this
  work exists to protect. `MAX_TURTLE_STEPS = 20M`, charged per body
  expansion so the hot loop is untouched, tripping at the identical
  point everywhere.

  Honest note on the bound: the input caps had already taken the worst
  case from ~10^11 steps to ~1.6e9, and I could **not** reproduce a
  multi-second hang end to end afterwards — the depth-stepdown's
  interaction with `CHAR_BUDGET` keeps real walks small. The budget is
  a backstop against that interaction changing, not a fix for a
  reproduced hang.

### 8.2 Variations

- [x] **S** — ~~Consume the `features` array~~ **DONE** (`95bddf67`)
  ([WIRE_FORMAT §6.1](VARIATIONS_WIRE_FORMAT.md)), superseding the
  three legacy bools; ignore unknown feature strings with a warning.
- [x] **S** — ~~Consume `state_count` / `shader_state_init`~~ **DONE**
  (`95bddf67`). Was a live defect: slots were allocated and read as
  zeros, so a stateful downloaded variation rendered wrong while
  looking like it worked. `aliases` and `plot_emits` landed too.
- [x] **S** — ~~Consume `description_plain` + `authors`~~ **DONE.** Both
  now render under the row they belong to. Worth noting *why* this had
  to wait for the catalog: built-in descriptions live in Rust doc
  comments, which do not exist at runtime, so the catalog is the only
  route by which prose reaches a variation row — including for the 646
  that ship with the app. Nothing local could have supplied it.
- [x] **M** — ~~Manifest-driven catalog (§3.3)~~ **DONE**, with one
  deliberate omission. `refresh_variation_catalog()` fetches `GET
  /api/variations` once per session in the background and caches it to
  `variations/_catalog.json`, giving `list_variations()` — dead code
  until now — its purpose. It does **not** pause the render or pop a
  notification: the catalog is panel metadata, not something a frame
  depends on.

  **Etag revalidation deferred.** `CachedCatalog` carries the `version`
  field for it, but the server sends no etag on this endpoint yet and a
  once-per-session full fetch of ~650 shaderless rows is not the cost
  worth optimising first.
- [x] **M** — ~~Variations panel lists everything~~ **DONE.** The
  registry listing (installed, by category) now sits under a catalog
  section carrying what the registry structurally cannot know: what
  exists elsewhere. `summarize()` is pure and lives in
  `storage::variation_catalog`, so the bucketing is tested without an
  egui context — which bucket a variation lands in decides whether the
  user is offered a download that cannot succeed.

  `BuiltInOnlyElsewhere` is the bucket that earns its keep: catalogued,
  real, and not fetchable, because it is part of the render engine
  rather than downloadable shader code (`subflame_wf`'s shape). Calling
  it "Available" would offer a fetch that fails; omitting it would make
  the catalog look short.

  **Local plugins are still absent** — there is no `Local` source to
  list until 8.4 introduces one.
- [x] **S** — ~~Offline behaviour~~ **DONE.** A failed fetch logs at
  *info* and changes nothing on screen: the cached catalog and the
  installed listing are both still true. An app that renders fractals
  perfectly well with no network should not grow an error panel because
  a metadata endpoint was unreachable, and a user who has never signed
  in should never see one at all — with no catalog, the section is
  simply not drawn.
- [x] **S** — ~~Offer updates on a version bump~~ **DONE.** A stale
  downloaded copy is marked in place and in the catalog section, with
  per-row **Update** and **Update all**. The action re-uses the install
  path exactly — `register_from_api` replaces a non-core entry and the
  cache write overwrites — so "update" is "install again" with no
  second code path to keep in step. A built-in is filtered out before
  the fetch: it can never be replaced by a download, so the request
  would spend 30 seconds discovering a no-op.

### 8.3 Scripts

- [ ] **M** — Local cache / local store. On WASM this *is* the only
  storage — closes "a web user cannot save a script at all", and is
  worth doing **before and independently of** the API.
- [ ] **S** — Fork-on-edit for built-ins (§5.2), paired with 8.0.
- [ ] **M** — CRUD client calls against §5.4.
- [ ] **M** — Panel: sign-in-aware list (mine / built-in / public),
  save-to-cloud, update-conflict handling.
- [ ] **S** — Derive and send `kind` / `flags` / `description` from the
  collect pass; treat source as authoritative on load.
- [ ] **M** — Public browse UI. Gated on the 8.1 deadline item, since
  this is the point where users run strangers' code.

### 8.4 Local-only plugins (§2)

- [ ] **S** — Source tag on registry entries
  (`Builtin | Api | Local`), replacing the `is_core` bool.
- [ ] **M** — Plugin load path: desktop folder, WASM localStorage under
  a key prefix distinct from the download cache.
- [ ] **S** — Reject name collisions with a message naming the
  conflict; never shadow.
- [ ] **S** — Never upload a local plugin — assert it at the API
  boundary, not just by convention.
- [ ] **M** — Warn when saving or uploading a flame that depends on a
  local plugin: it will not render for anyone else, or for the same
  user on another device.
- [ ] **S** — Missing-resource path distinguishes "downloadable" from
  "you do not have this plugin".

### 8.5 Effects — the large build (§4)

- [ ] **M** — `Lazy<EffectRegistry>` → `Lazy<RwLock<…>>` plus
  `global_effect_registry_mut()`. 17 call sites, 5 files, mechanical.
- [ ] **S** — `EffectInfo` holds source *or* path, plus source tag and
  `version`.
- [ ] **M** — Source-accepting compilation, preserving the
  `// INCLUDE_BLEND_MODES` splice.
- [ ] **M** — Effect cache, mirroring the variation cache — with the
  WASM key index done right rather than inheriting 8.1's bug.
- [ ] **M** — On-demand fetch trigger. The hook exists;
  `effect_chain.rs` detects unknown types and only logs.
- [ ] **M** — Manifest + panel listing everything, matching 8.2.
- [ ] **S** — Replace the dead `Effect` struct in `src/api/types.rs`
  with `EffectDownload`.
- [x] **S** — ~~Publish-time validation the server needs from us~~ —
  superseded: the cap is **48**, not 16 (raised 2026-07-31), and the
  numbers now ship in the generated contract rather than prose.
- [x] **S** — ~~Export the 15 built-in effects for the catalog~~
  **DONE** — `cargo run --bin export_effects_json`, 15 effects, 77 KB:
  WGSL, parameter schemas, display names, category and
  `requires_blend_modes` per effect. Shader source is RAW, with the
  `// INCLUDE_BLEND_MODES` marker intact, so the shared library is not
  baked into all 12 effects that splice it.

  Two gaps it surfaced, both places effects have not caught up with
  variations, and both emitted as nulls rather than papered over:
  `EffectParameter` has no `description` field (VariationParameter
  does, and 2836 `param!` macros populate it), and `EffectInfo` has no
  `display_name` — the curated labels live in `locales/en.yml` and the
  exporter reads them there. `sobel_edges` is "Edge Glow", so deriving
  a display name from the key would have been wrong.

### 8.6 Palettes — deferred

- [ ] **S** — Standalone palette upload + a button in the palette
  editor (§6). Low priority; recorded so the endpoint is not removed.

### 8.7 Shared machinery (§7.2)

- [ ] **L** — Factor manifest + cache + on-demand fetch + registration
  + provenance once, when effects are built. Three hand-copies of
  `variation_fetch.rs` / `variation_cache.rs` would triple-maintain the
  same bugs — 8.1's write-only WASM cache being exactly the kind that
  gets duplicated. Not worth factoring speculatively before then.

### Suggested order

8.0 → 8.1 (the three cheap defects) → 8.2 → 8.3 local store → 8.3 rest
→ 8.4 → 8.5 (factoring 8.7 as it goes) → 8.6 whenever.

Rationale: the cheap correctness items make the existing feature honest
before it gets a UI that exposes it; variations prove the manifest
pattern on the subsystem that already works; the script local store is
the single highest user-visible win and needs no server; effects are
last because they are the largest and benefit most from every pattern
above being settled.
