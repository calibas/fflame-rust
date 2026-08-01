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
  "user_id": "…",                    // owner
  "display_name": "…",               // owner's, for stem namespacing
  "authors": ["…"],                  // CREDIT, not ownership; [] is normal
  "description": "…markdown…",       // from the header comment — derived
                                     // no description_plain: client strips
  "source": "…rhai…",                // authoritative
  "version": 3,
  "visibility": "private",           // private | unlisted | public
  "flags": ["norng"]                 // derived
}
```

#### 5.4.1 Settled

**Reserved names — rejected at create, regardless of visibility.**
Shipped stems are reserved client-side: `merge_sources` refuses a user
script that takes one rather than shadowing it (8.0, which was a live
bug — one Save hijacked `basic_random`'s `run_script("random_palette")`
call). So accepting such a name stores a script *nobody* can run, its
author included: the local refusal does not care where the script came
from or who owns it.

The list ships **in the generated contract** (`scripts.builtin_scripts`
in [`docs/generated/engine-contract.json`](../generated/engine-contract.json)),
read from the embedded starters. Transcribing ten stems into the API
would be the name-gating trap in a third guise — the same implicit
cross-repo dependency as the helper libraries and the blend marker, and
it would go stale in exactly the same silence. A test asserts the
contract reserves precisely what `is_builtin_stem` refuses, so the
generator reading the wrong source fails here rather than in the field.

**`name` is per-user unique; `id` is identity.** `users.display_name` is
case-insensitively unique server-side (migration 37), so
`display_name/stem` is already a globally unique, stable, human-readable
key — the client's import-rename rule uses it instead of inventing a
disambiguator or showing UUID fragments. Both script payloads carry
`user_id` and `display_name`, as `FlameResponse` already does.

**Cross-calls: downloaded scripts may call only shipped stems.**
`run_script(id)` resolves against the *whole* local library, the user's
own scripts included, so an unrestricted downloaded script would bind to
whatever that machine happens to have under a name — different results
per machine, and a stranger's script reaching the user's. Restricting to
shipped stems is the only option that renders identically everywhere.

**Enforcement is the client's — DONE — and a `dependencies` field is
deliberately NOT reserved.** The server cannot enforce this:
`run_script(some_variable)` is not statically resolvable, so a
server-side check would catch literal call sites and miss dynamic ones —
a guard that looks like enforcement and is not. Unlike the blend marker,
where the marker/call correlation is exact.

Built in `script::host` before anything can be fetched, on the same
reasoning §5.2 used: doing it after would mean a migration. An
untrusted script may call shipped stems only, and the check runs
*before* the id is resolved so the refusal cannot report whether the
user has a script by that name.

The rule is "any untrusted frame on the stack", not "the immediate
caller" — otherwise `downloaded -> shipped -> user` would launder the
restriction through one hop. That divergence is unreachable from real
scripts (it needs a shipped script that calls a user one, and shipped
scripts are compiled in), which is exactly why the rule is a free
function tested on data: an engine-driven test would pass under either
reading while appearing to pin the stricter one.

And the restriction is what makes reserving unnecessary. If downloaded
scripts may only call shipped stems, no published script can *have* a
non-builtin dependency, so when dependencies land the backfill is
provably empty — every existing row correctly means `[]`, with no old
sources to parse. Adding the column later is one additive migration;
adding it now is an unconsumed field on a published payload, and this
session established that those are the ones that can never be removed,
because you cannot prove nobody reads them.

**`authors` is credit; `user_id` is ownership. They are additive.**
Renamed from `author` for consistency with variations and effects, but
it does not mean the same thing here and the two must not be conflated.
On variations and effects `authors` is the *only* attribution —
free-form "Name (year)", explicitly not a foreign key, because those
people mostly are not platform accounts. A script has both: `user_id`
owns it (permissions, namespacing, `display_name/stem`), while `authors`
credits whoever wrote it, possibly someone who has never used the app —
a ported script, a technique from a paper.

So a script written from scratch by its uploader has `authors: []` and
the UI falls back to `display_name`. A ported one credits the original
while still being owned by its uploader. Treating `authors` as "the
author field" would duplicate `display_name` on every original script,
which is the version of this that goes wrong.

**`description_plain` is client-side for scripts, and only for
scripts.** This is a deliberate exception to
[VARIATIONS_WIRE_FORMAT](VARIATIONS_WIRE_FORMAT.md) §7's "the same pair
applies to effects and scripts". A variation's prose is authored
metadata with no client-side source to re-derive from, so the stripped
copy must travel and both consumers agree on one result. A script's
description is *derived from its source*, by `parse_doc`, and the source
is authoritative and always present — so a stored plain copy would be a
derivation of a derivation, a third representation of the same bytes
with its own way to go stale against a source the client re-reads on
every load anyway.

`script::strip_markdown` does it client-side. Inline syntax only; block
structure (`# Heading`, indented tables, list markers) is left for the
panel, which already renders it structurally.

**`version` on PUT is optimistic — and means something different here
than on variations and effects.** Desktop app and browser tab share no
store, which is exactly the case last-write-wins loses data in, silently
and unnoticeably.

The collision to name explicitly: on variations and effects `version` is
a *cache-invalidation key*, and §7 says prose edits deliberately do not
bump it so clients keep cached copies. An optimistic token must bump on
every write, unconditionally. Same field name, opposite rules.

One field serves both for scripts because **the source is the content**.
There is no prose-vs-payload split like a variation's
description-vs-WGSL, so there is no edit that should leave a cached copy
valid — bump-on-every-write is simultaneously correct for both purposes.
That reasoning is specific to scripts and does not transfer back; nobody
should later "harmonise" the three.

`UPDATE … WHERE id = ? AND user_id = ? AND version = ?` returns zero
rows for three different reasons, and collapsing them makes conflict
handling unwritable. Existence and ownership are checked first (404 /
403), then version (409), with the current version and `updated_at` in
the 409 body so the client can choose between refetch-and-merge and
warn. The version travels in the request body rather than an `If-Match`
header: the API is JSON end to end with no ETag machinery, so a header
would be the only one of its kind — **agreed, no preference for the
HTTP-idiomatic form here**, since there is no intermediary cache to
benefit from it.

**`flags` stays an open vocabulary — and the client now actually
behaves the way I said it did.**

I reported "the client warns on an unknown flag and drops it". It did
not: `declare_script` propagated the error, so a script declaring a flag
this build did not know **failed to run**. I had described variation
*features* — which genuinely warn and drop — and carried the claim into
the generated contract, where the API repo read it and built on it. The
one artifact whose whole purpose is to stop hand-maintained cross-repo
claims from going stale, carrying a hand-maintained claim that was never
true.

The behaviour is now what was documented, and on its own merits rather
than to make the sentence true. Both flags are UI affordances — `norng`
hides the seed controls, `palette` offers the script in the Palette
Editor — so neither touches the rendered flame and a dropped one costs
an affordance, never a wrong result. With public browsing live, hard
erroring means an older build refuses a newer script that would have run
correctly. **A flag that affected output could not be degraded this way,
and adding one would be a breaking change.**

The original design's concern was right and is preserved: a silently
ignored switch looks like a broken feature, so the warning is carried on
`ScriptMeta` and shown by the **collect** pass — a typo'd `norgn`
surfaces while editing, as it did when it was an error, rather than
waiting for Run. A malformed flag (a number where a string belongs) is
still a hard error: author mistake, not version skew.

**No server-side normalisation, but not for the reason given.** Flag
names are matched case- and whitespace-insensitively client-side, so
`NoRng` already works and lowercasing upstream would repair nothing.
Storing verbatim is still right — rewriting what an author wrote is not
the server's job — and `MAX_SCRIPT_FLAG_LEN` is the correct shape for
the real gap, since it bounds payload without touching the vocabulary.

#### 5.4.2 Open

- Nothing. See §8.3 for the client work that remains.
- **Client-asserted metadata.** The server has no Rhai engine, so
  `kind` / `flags` / `display_name` are whatever the uploader sent —
  a public listing filtered by `kind=generator` filters on data that
  could be wrong. Cosmetic, and "source is authoritative" already
  covers correctness; noted so it is a known property rather than a
  surprise.

### 5.5 Client work

- **Local cache** so scripts survive offline and load fast. On WASM
  this cache *is* the local store (localStorage), which closes the
  "web cannot save scripts" gap **even before the API lands** — worth
  doing first and independently.
- Scripts panel: sign-in-aware list (mine / built-in / public),
  save-to-cloud, conflict handling on update, fork-on-edit (§5.2).
- **Security note.** Unlike variations and effects, scripts are *user
  content* — public browsing means running strangers' code. The
  sandbox that makes that survivable is bounded on four axes:
  the Rhai operation budget, input caps on the L-system builtins,
  transform and xaos ceilings, and — closing the recorded gap that the
  operation budget cannot see native work — a **step** budget inside
  the native walks (§8.1). Not a wall-clock deadline: see that item for
  why a host-speed-dependent abort would have traded a DoS bound for
  the determinism guarantee this whole effort exists to protect.

  Coverage is now complete rather than partial. Every one of the four
  walks that expands a rule body per symbol — the quadratic shape, the
  only one that can outrun `LSYSTEM_MAX_LEN` — charges the budget. The
  3D walk in `lsystem_pieces3` allocated a budget and never spent from
  it until `2bf9865e`; a compiler warning about the unused binding is
  what surfaced it.

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

- [x] **M** — ~~Local cache / local store~~ **DONE.** `script::store`
  routes save / load / delete / list through `storage::backend`, so the
  user's own scripts work on the web as well as on desktop. This closes
  "a web user cannot save a script at all" — every one of those four
  operations was `#[cfg(not(target_arch = "wasm32"))]`, so a browser
  user could edit a script in the panel and the text died with the tab.
  Desktop paths are byte-identical to before (`<app data>/scripts/`), so
  nothing needs migrating.

  On the web this store is **not** a cache of something the server
  holds; it is the only copy. Hence a per-script 256 KB cap checked
  before the write: localStorage is a browser-wide quota shared with
  system settings and the palette, variation and thumbnail caches, and
  one runaway paste should not be able to evict all of it.

  The change that made this tractable was to key scripts by **stem**
  rather than path. `ScriptOrigin` splits `File(PathBuf)` into
  `Shipped(PathBuf)` / `User` / `External`, so "may this be deleted" is
  answered by the variant instead of by canonicalizing a path and
  comparing it against the user folder — a check that could not exist in
  a browser. `delete` takes a stem and builds the key itself, so there
  is no path for a caller to aim outside the store.

  `save` returns the stem it **actually** used, which is not always the
  name it was given: sanitizing turns "My Script" into `My_Script`, and
  a caller that assumes otherwise reports the wrong location and selects
  the wrong entry after a fork. That was a live bug in the version this
  replaced, and it is silent — the write succeeds either way.
- [x] **S** — ~~Fork-on-edit for built-ins~~ **DONE**, on the reading
  that the fork happens on **Save** rather than on the first keystroke:
  forking per keystroke would litter the store with copies made by
  accidental typing, and §5.2's requirement — "the user keeps editing
  without interruption and the built-in is untouched" — holds either
  way. Editing a shipped script and saving writes `<name>-copy` and
  switches the editor to it. Paired with 8.0's shadowing removal, which
  is what makes the fork necessary rather than optional.

  The web half only started working now: Save was desktop-only, so a
  browser user editing a built-in got no fork and no copy.
- [x] **M** — ~~CRUD client calls~~ **DONE.** Six calls against §5.4,
  built against `openapi.json` rather than against the shape I had
  assumed — which was wrong in two ways the compiler could not catch:
  `display_name` is the SCRIPT's name (the owner's is
  `owner_display_name`), and `display_name`/`kind` are required on
  create rather than optional.
- [x] **S** — ~~Derive and send `kind` / `flags` / `description`~~
  **DONE**, in `sync::script_to_create_request`. Publishing is refused
  for a script that does not COMPILE — there is nothing to derive, and
  values its source cannot reproduce would be invisible to the uploader
  and visible to everyone browsing. A script that compiles but never
  calls `script(name, kind)` IS published: `collect` defaults it to a
  generator with a warning, and because that default is part of the
  derivation, every client re-deriving computes the same answer.
  Deterministic, not invented.
- [x] **M** — ~~Panel: sign-in-aware, save-to-cloud, conflict
  handling~~ **DONE.** The online section is drawn only when signed in;
  offering Publish to someone who cannot use it is worse than not
  showing it, since the local store is the entire feature for a
  signed-out user and it works.

  A 409 is a **decision**, not an error message, so it renders as a
  banner that stays until made: *Overwrite with mine* (retry against
  their version — the "I looked and I still want mine" path, and it has
  to be a click rather than an automatic retry) or *Load theirs*.
- [x] **M** — ~~Public browse UI~~ **DONE.** Search, results with owner
  and credit shown separately, and one sentence stating what opening
  one means — said once where the action is, rather than left to be
  inferred from a download button.

**The piece that made both of the last two possible: `ScriptLink`.**
A sidecar (`scripts/_links.json`) carrying each stored script's cloud id,
last-seen version, and whether somebody else wrote it. All three are
load-bearing:

- Without the id and version, an update after a restart is impossible —
  optimistic concurrency needs the version you read, and that does not
  survive a process that forgot which server script this is.
- **`from_others` must be persistent, not a UI flag.** Adopting a
  downloaded script makes it `ScriptOrigin::User`; if trust were read
  from the origin, pressing Save would launder away the cross-call
  restriction. The user chose to keep the script — they did not read it.
- `delete` clears the link, so a reused stem cannot inherit the previous
  script's cloud identity *or* its provenance. Writing your own script
  under a freed name must not leave it marked as somebody else's.

Two defects found while building, both silent:

- **Adopt and Refetch are not the same operation.** Resolving a conflict
  on your own script by adopting would save a second copy under a freed
  stem *and* mark your own work as somebody else's, so it would run
  under the cross-call restriction from then on. They look
  interchangeable — both fetch a script and write it locally — and
  reusing one for the other is the natural mistake.
- **An adoption writes to the store from a background task**, so the
  panel never learned the script existed. A generation counter fixes it;
  the panel holds only `&ScriptCloudState` and so cannot clear a flag,
  but it can remember the last value it acted on.

### 8.4 Local-only plugins (§2)

- [x] **S** — ~~Source tag~~ **DONE.** `Provenance` replaced
  `is_core: bool` **and** a separate `version: u32` on `VariationInfo`;
  effects got it in §8.5. The duplicate version was the more dangerous
  half — it is what a `merge_state` would have compared, so a local
  plugin's file version would have been read as a server counter.
  Three call sites needed genuinely different answers rather than a
  substitution, because the questions come apart:

  | | shipped | downloaded | local plugin |
  |---|---|---|---|
  | third-party code? | no | **yes** | **yes** |
  | in the download cache? | no | **yes** | no |
  | updatable? | no | **yes** | no |

  `clear_api` filtered on `!is_core`, which would have swept the user's
  own plugins into Clear Cache the moment they existed — deleting their
  files under a label that says "cache".
- [x] **M** — ~~Plugin load path~~ **DONE.** `storage::plugins`, under
  `plugins/variations/` and `plugins/effects/` — a prefix neither cache
  enumerates, so "Clear Cache never destroys the user's work" is
  structural rather than a rule to remember.

  A plugin file is **the same JSON as the download payload**, so it
  takes the same registration path with `Provenance::Local` rather than
  a parallel `register_from_local` that would need every future refusal
  copied into it. Installing validates through that same code, so a
  file that would be refused later is refused while the user is looking
  at it.

  The **file name is the identity**, not the `name` inside it —
  otherwise two files could claim one name and which won would depend
  on directory order.
- [x] **S** — ~~Reject name collisions~~ **DONE**, in both directions,
  which is the part worth stating. A plugin may not take a built-in's
  name (shadowing `linear` would change what every shared flame
  renders). And a **download may not displace a plugin** — the worse
  direction, since it replaces the user's own work with somebody
  else's and they never asked for it.

  Plugins load LAST at startup, so a collision is detected against
  everything already present and the user's file is the one refused:
  refusing is recoverable (rename it), displacing a curated resource is
  not.

  Refusals are **reported**, not just logged. A plugin the user
  installed that then does not appear is exactly the case where a
  console line is not a report.
- [x] **S** — ~~Never upload a local plugin~~ **DONE**, asserted at the
  boundary. A flame travels as variation **names** plus its own
  parameters, never as definitions — so there is no payload a plugin's
  WGSL could ride out on, and a test fails if one appears. The client
  also exposes no endpoint that uploads a variation or effect at all;
  those are read-only by §0 decision 1.
- [x] **M** — ~~Warn on save or upload~~ **DONE**, on both the
  save-to-file and upload paths. A notice rather than a refusal: saving
  a flame you can still open yourself is legitimate, and the plugin may
  be submitted for curation later. This is a fact the user needs, not a
  mistake to prevent.
- [x] **S** — ~~Tested end to end with real plugin files~~ **DONE.**
  `assets/plugins-example/` ships one variation and one effect. Both are
  deliberately **no-ops at their defaults** — the variation equals
  `linear`, the effect's `amount = 0` changes nothing — so a byte-identical
  render proves the plugin's own shader compiled and ran, and a changed
  render proves its parameters reached the GPU. Both held.

  Testing found two bugs that no unit test would have.

  **Headless never loaded plugins or cached downloads.**
  `load_cached_api_variations` and `plugins::load_all` were called from
  `App::new` only, so a CLI export dropped both and said nothing — a
  missing variation is a weight contributing zero, not an error. That
  was live for downloads since §8.2. Now one
  `storage::load_installed_resources` shared by every entry point, with
  refusals on stderr for headless runs.

  **The include marker was a substring, not a directive.**
  `process_shader_includes` replaced *every* occurrence, so a shader
  whose comment merely QUOTES `// INCLUDE_BLEND_MODES` got two hundred
  lines of library spliced into the middle of a sentence, then failed to
  compile pointing at a line its author never wrote. No built-in quotes
  the marker; the first plugin written to document it hit this
  immediately. Now matched only when alone on its line, with a repeat
  dropped rather than duplicated.
- [x] **S** — ~~Missing-resource path distinguishes the two~~ **DONE.**
  From the config they look identical — both are a name the registry
  does not know — but they need opposite responses, and telling
  somebody to wait for a fetch that cannot succeed is worse than
  telling them nothing.

  Four cases, not two. `Unknown` is separate from `ProbablyAPlugin`
  because **being offline is not evidence**: with no catalog fetched,
  the honest move is to try the fetch and let it fail rather than
  accuse the flame of needing a plugin.

### 8.5 Effects — the large build (§4)

- [x] **M** — ~~`Lazy<RwLock<EffectRegistry>>` + `_mut()`~~ **DONE.**
  19 call sites, 6 files. Two had to be restructured rather than
  substituted: a guard cannot be dropped at the end of the statement
  that borrows from it.
- [x] **S** — ~~`EffectInfo` holds source~~ **DONE**, as
  `EffectSource::Builtin { embedded, path } | Owned(String)` — embedded
  always, with the on-disk copy preferred on desktop, exactly the
  arrangement `assets/scripts/` has.
- [x] **M** — ~~Source-accepting compilation~~ **DONE**, splice intact.

  **This fixed a bug live in every shipped desktop build.**
  `embedded_shaders` was already compiled into every target, but the
  desktop path read `shaders/` relative to cwd and *errored* when the
  file was missing rather than falling back to the copy it already had.
  Run the binary from anywhere else and every effect logged an error and
  rendered nothing. Proven by rendering one flame from two directories:
  old `be8669f0`/`e593f8ff` (differ), new `be8669f0`/`be8669f0` (match),
  and the new render is byte-identical to the old one where effects
  already worked.

  `load_blend_modes` was worse: a missing file logged an error and
  returned an **empty string**, so all twelve effects that splice it
  compiled against nothing and failed naming a function the reader never
  wrote.

  The WASM path is gone. It was a hardcoded match over the fifteen
  shipped paths with `_ => Err(...)`, which made a downloaded effect not
  unimplemented but **inexpressible** — the client could not have
  consumed the corpus format this repo already exports.
- [x] **M** — ~~Effect cache~~ **DONE**, and it registers *before* it
  saves, so a payload this build refuses is not stored to be refused
  again on every startup. Startup applies today's rules to what is
  already on disk, so a refusal tightened later reaches cached entries
  rather than being bypassed by them.
- [x] **M** — ~~On-demand fetch~~ **DONE.** Recorded where the answer is
  already known — `EffectChain` looks each name up while compiling and
  finds nothing — rather than by scanning the config, because unlike a
  missing variation, a missing effect depends on what the registry holds
  right now.

  Rendering is deliberately **not** paused, unlike a variation fetch: a
  flame missing a variation renders something wrong, a flame missing an
  effect renders the un-effected image.

  `compile_effects` runs every frame and never caches a failure, so
  nothing needs invalidating when an effect arrives — and a
  still-missing effect re-records its name every frame, which is why
  `effect_fetch_attempted` exists. Without it a server that does not
  have the effect would be asked sixty times a second, which is the
  ordinary case while `downloadable` is false.
- [x] **M** — ~~Manifest + panel~~ **DONE**, sharing 8.2's state machine
  rather than copying it: `storage::catalog` holds `CatalogState` /
  `merge_state` / `summarize` behind a three-method `CatalogItem` trait,
  and both subsystems implement it.

  The interim renders honestly. Every effect row is `downloadable:
  false` until the server seeds its shaders, so the whole catalog merges
  to `BuiltInOnlyElsewhere` — listed, counted, offered no fetch. Calling
  them Available would offer a download that returns a null shader and
  gets refused.
- [x] **S** — ~~Replace the dead `Effect` struct~~ **DONE** —
  `EffectDownload` + `EffectListItem`. Effect parameters reuse the
  variation parameter type outright rather than copying its shape: the
  API serves both verbatim from one format, and two structs that must
  stay identical are two structs that will not.

**Registration refuses rather than degrades**, for the reason variations
do — an effect that registers and renders nothing looks like a broken
feature with no way to find out why. Four refusals: no shader, unknown
category (the category *is* the pipeline position, so there is no safe
default), over the 48-parameter uniform capacity, and a shader that
calls the shared blend-mode library without including it.

That last guard reads its symbol list **out of** the library rather than
transcribing one, and it mattered: the library exports `luminance`,
`rgb_to_hsl` and `set_luminance` as well as `blend_*`, and a hand-written
list of blend functions — the obvious version — would have missed them.

**Two bugs found in my own earlier work while building this.**

`variation_cache::list_cached` listed `variations/*.json`, which
includes the catalog `_catalog.json` written in 8.2. So `load_all`
failed to parse it and warned on every startup, and `clear_all` counted
it — "Clear Variation Cache (N)" reported one more than there were
cached variations, and deleted the catalog as though it were one. Both
caches now skip `_`-prefixed entries as metadata.

Four script-store tests were **flaky, not failing**: `set_link` is
read-modify-write over one shared file, so parallel tests clobbered each
other even with distinct stems. They passed for two commits by luck of
scheduling.
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
