# API updates without breaking older apps

Status: **step 1 merged** (PR #160); **step 2 built 2026-11-11** in ffapi, branch `reader-vocabulary` (migration 100); steps 3-5 to come. Sources: a
survey of the client (`src/api/`), the config, and the server repository
(`c:\projects\ffapi`, axum + Postgres, latest migration 099), checked
against the code where it mattered. This plan absorbs the API half of
[v0.6.0-release.md](v0.6.0-release.md) and P6 of
[rhai-plugins.md](../archive/projects/rhai-plugins.md).

The goal, in the user's words: "do all the API updates in a way that
won't break older versions of the app. There's numerous API updates
needed, not just Rhai."

---

## 1. Where things stand

### What the app talks to

- One base URL, `https://fractalsforall.com` (`api/mod.rs`), every route
  under `/api`, no version prefix. Desktop: ureq with a Bearer token and
  no User-Agent of its own. Web: fetch with the session cookie.
- **No client says what it is.** No version or contract header is sent,
  and the server's CORS allows only Content-Type, Authorization, Cookie.
  The server cannot tell an old client from a new one.
- Resources the client uses: auth; `users/me`; flames (list, CRUD,
  thumbnail, search, a flame's animations); palettes by hash; animations
  (CRUD); scripts (CRUD, search; optimistic `version`, 409 on conflict);
  read-only catalogs of effects and variations.
- **Flames**: `config` (the `.fflame` JSON with its `version`, palette and
  root transforms lifted out) and `transforms[]` are stored verbatim; the
  server reads only `/render_mode` (a Postgres enum: 2d, 3d, escape).
  Limits: config 256 KB, a transform 16 KB, 128 transforms.
- **Animations**: `tracks` and `generators` stored as opaque JSON, 32 KB
  each; `base_config` is sent and dropped (the server has no field);
  `motion_blur` is not sent.
- **The engine contract** (`src/contract.rs`): vocabularies, limits,
  helper gating. Its `shape` fingerprints key paths only, so new
  vocabulary values never move it. The server learns of engine changes by
  a hand copy (RELEASE.md §3).

### Who the old clients are

- **Desktop 0.5** (tag `0.5`, 2026-08-24): config v3, render modes 2d/3d,
  phases pre/normal/post, no script tracks. Zips live forever, and
  nothing tells their users to update.
- **The web app** updates on reload (a tab can stay stale). **The gallery
  modules** were built 2026-09-29: a stale one refuses v7 configs too.
- Every client from now on is an "old client" to the next.

### What breaks an old client (verified)

1. **The config version gate.** A config with `version >
   CURRENT_CONFIG_VERSION` is refused (`fractal_config.rs`, also in 0.5),
   and every save stamps the current version whatever it uses. So **every
   flame a current client saves online is unloadable in 0.5** (v7 > v3),
   and every future bump does the same to 0.6. RELEASE.md §4b's claim
   that an older build "degrades rather than breaks" is wrong.
2. **Strict wire enums** (no `#[serde(other)]` anywhere but one field
   key): a new value fails the whole response.
   - `FlameListItem.render_mode` -- in 0.5 an `escape` flame on a page
     **fails the whole list or search page** (live wherever migration 099
     is deployed); `simulation` will do it to 0.6.
   - `ApiVariationPhase {pre, normal, post}` -- the server stores phase
     `any` (migration 071; 546 variations re-imported in 097), so the
     missing-variation download, exactly what an old client does for a
     variation newer than itself, **fails to decode**. The wire doc's
     "old clients fall back to normal" is not what happens.
   - `ApiParamType`, `ApiLoopMode`, `ApiVisibility` likewise.
3. **Animations lose data silently.** `tracks` are parsed as one
   `Vec<Track>` with `.ok().unwrap_or_default()`; `TrackSource` has no
   catch-all. One `{"type": "Script"}` track (PR #158) gives a pre-#158
   client **zero tracks**, no error -- and PUT is a full replace, so its
   Save erases them. Script sources can also exceed the 32 KB track cap.
4. **Config enums** (RenderMode, ColorMode, ToneMapMode, HighlightMode,
   ~35 in escape and simulation config): one new value makes an otherwise
   readable config unreadable.
5. **Unknown fields are dropped on re-save**: an older, version-
   compatible client loses a newer client's fields when it saves back.

What is already safe: names carried as **strings** degrade to "missing"
-- escape formula and colouring, sim model and colouring, effect types,
variation names, script `kind`, effect `category`. Unknown fields are
ignored on read (no `deny_unknown_fields` in either repository).

---

## 2. The rules

1. **Additive only on existing endpoints.** No field removed, renamed or
   retyped; no new value an old client cannot read sent to an old client.
   A change that cannot be additive gets a new endpoint, the old one kept.
2. **A reader says what it reads, in the URL: `?reader=N`**, N being
   the highest config version it can read (the app's
   `CURRENT_CONFIG_VERSION`). It goes on every request, GETs included. No
   header, no User-Agent sniffing: the same rule for the app and for any
   other client. A request without `reader` gets **the original
   vocabulary**: what 0.5 reads (reader 3).
3. **The server holds the vocabulary per reader.** Each value that came
   after the original vocabulary is tagged with the first reader version
   that knows it, and a request is answered in its reader's vocabulary:
   - a flame whose `min_reader` is above the reader, or whose render mode
     the reader does not know, is left out of lists and searches (not
     errored);
   - phase `any` is sent as `normal` to a reader that does not know it;
   - a script of a kind the reader does not know is left out.

   A request with a higher `reader` gets the newer values, so new content
   is opt-in for every client and no client receives a value it has never
   seen.
4. **From 0.6 on, clients degrade instead of failing**: an unknown enum
   value reads as a fallback and is kept for the round trip; an unknown
   track is kept as opaque JSON; a newer config loads read-only-safe
   (rule 5); unknown fields survive a save.
5. **A config says the lowest version that can read it**, apart from the
   version that wrote it: `version` (writer, migrations) and
   `min_reader`. A reader older than `min_reader` refuses; one at least
   `min_reader` but older than `version` loads, warns, and keeps what it
   does not understand (a value it cannot read is set aside, its field
   defaulted, and put back on save unless the user changed that field;
   fields it does not know are kept).
   - **A default change** bumps `version`, and from then on the writer
     always writes that field out, even at its default: configs omit
     default fields, and an older reader would fill an omitted one with
     its own old default. `min_reader` stays.
   - **A change an older reader would get wrong** (a renamed key, a new
     meaning) bumps both.
   - **An older app re-saving a newer config** stamps its own `version`,
     because its omitted fields follow its own defaults, and keeps the
     config's `min_reader`. The newer app's migration steps then restore
     those defaults. So a migration step must never overwrite a field
     that is already present.
   - **On the server**, a reader that knows `min_reader` (0.6 on) sees a
     config when `min_reader <= reader`; an older one (0.5, and anything
     before `min_reader`) only compares `version`, so it sees a config
     when `version <= reader`.
6. **0.5 cannot be fixed** -- its gate is compiled in. It sends no
   `reader`, so it gets the original vocabulary (rule 2): lists without
   what it cannot open, nothing it cannot parse.
7. **An update notice**: `GET /api/app/latest` gives the newest app
   version; 0.6 asks at startup and shows a notice when it is behind
   (RELEASE.md §4b's in-app update check, not built). Any client can ask.

---

## 3. The changes, in order

### Step 1 -- harden the client (app 0.6, no server change)

**Built 2026-11-11 on branch `api-client-hardening`** (PR.md there): every
item below. The update check asks `GET /api/app/latest`, which the server
does not have yet (step 2). The "save refuses to PUT what it dropped"
guard is not needed: nothing is dropped any more.

Ships first, so that everything after can be added safely.

- **Catch-alls on every wire enum**: `ApiRenderMode`, `ApiVariationPhase`
  (and accept `any`), `ApiParamType`, `ApiLoopMode`, `ApiVisibility` get
  an `Unknown(String)` (or `#[serde(other)]`) arm; list pages skip an item
  they cannot use instead of failing.
- **Tracks parsed one at a time**; an unparseable track kept as raw JSON
  (`TrackSource::Unknown(Value)`), shown as "a track this version cannot
  play", and written back unchanged. A `.anim` with one loads the rest.
  Save refuses to PUT an animation it read with tracks it dropped (belt
  and braces).
- **Lenient config enums**: a deserialize helper that reads an unknown
  value as the type's default and records it, for the ~40 config enums;
  the value written back as read (kept beside the field).
- **Unknown fields preserved**: `#[serde(flatten)] extra` on
  `FractalConfig`, `EscapeConfig`, `SimConfig`, `Flame`, `Transform`,
  `Animation`, so a save keeps what a newer client wrote.
- **`min_reader`** (rule 5) written on save; the gate changed to refuse
  only below it. Old files have none: read as `version`.
- **`reader=N`** (rule 2) on every request; a `User-Agent` of its own
  (`FractalArtEditor/<version>`), for the server's logs only; the update
  notice (rule 7) asked for and shown.
- Tests: a corpus of "from the future" payloads -- a flame with
  `render_mode: "hologram"`, a phase `"sideways"`, a track `{"type":
  "Teleport"}`, a config `version: 99, min_reader: 7` with unknown fields
  -- each loads, degrades as specified, and round-trips byte-stable where
  untouched.

### Step 2 -- the server answers each reader in its vocabulary (server, additive)

**Built 2026-11-11** (ffapi branch `reader-vocabulary`, PR.md there):
`src/reader.rs`, migration 100 (`flame_readable_by`,
`animation_readable_by`), every flame and animation list, phase `any`, a
409 for an animation PUT a reader below 7 would wreck, `GET
/api/app/latest` (`APP_LATEST_VERSION` / `APP_LATEST_URL`). Not filtered:
scripts (their kinds are strings a client degrades on, and no new kind
exists yet: step 4 filters the new ones), and a flame's inline animation
refs (each is guarded at its PUT). Two tests on ffapi's main fail until
step 3 seeds what the corpus sync brought: the effect `distance_field`
(decision 4) and the built-in scripts `escape_deep` and `sim_sweep`.

- An extractor reads `?reader=` on every route (absent = 3, the original
  vocabulary) into the request's `Reader`.
- Shaping (rule 3): phase `any` as `normal` below the reader that knows
  it; flame lists and searches filtered by `min_reader` and render mode;
  script lists filtered by kind; `GET /api/app/latest`.
- **Fix the conformance test** that reads `render_modes` as an array (the
  contract publishes an object), so it has never run.

### Step 3 -- catch up the server (additive, v0.6.0's list)

- **The catalog's facets** ([api-catalog-spec.md](api-catalog-spec.md)):
  each saved config's engine, scene (2D, 3D, solid, terrain), versions and
  components (formula, model, variations: a twin unedited counts as its
  built-in), so configs can be browsed by fractal type and searched by
  formula or model.

- **Re-pin the contract**; the moved limits (`variation_slots_per_flame`
  262,144; `largest_shipped_variation_slots` 2410 against
  `policy_cap` 512 -- decision 2).
- **Script kind `library`** (the client sends it; the server's enum
  refuses it with 422).
- **Config v7**: the server must not validate variation names against
  its catalog (Rhai variations are `u<uuid>_<hex>`).
- **Render mode `simulation`** (migration `ADD VALUE`, no transaction),
  legacy-filtered (step 2) -- decision 3.
- **Corpora**: variations (subflame_wf, pre_subflame_wf, mondrianomies,
  polychoron/menger inits; renamed params `w.super`/`z.super` ->
  `*_shape`, `hexnix3D.3side` -> `triangle_scale`, old names tolerated in
  stored configs), effects (`distance_field`; `textureSampleLevel`;
  decision 4), scripts (`escape_deep`, `sim_sweep`, six changed).
- **Animations**: store `motion_blur`; `base_config` stays unstored (the
  flame is referenced by `flame_id`; decision 5); raise the
  32 KB `tracks` cap to hold script sources (or store a track's source
  apart).
- Clean up the abandoned `client-sync-2026-10` branch and the uncommitted
  corpora in ffapi's working tree; refresh `openapi.json` in both repos.

### Step 4 -- P6: one scripts resource for every Rhai kind

- `script_kind` gains `variation`, `effect`, `formula`, `colouring`,
  `solid`, `field`, `field_colouring`, `sim_model`, `sim_colouring`,
  `track` -- every new value legacy-filtered (step 2), so 0.5 and 0.6 see
  generators and modifiers as today.
- Publish, update, unpublish, search and versions for each, by `kind`;
  adopting saves into the library, flagged as a plug-in if wanted (P3).
- The client's ids are kept (design rule 7: a UUID in the source, a
  SHA-256 version); the server accepts the client's id on create.
- Settles the effect corpus question: the API stores the Rhai source.
- The WGSL variation/effect catalogs stay beside it for old clients (D2).

### Step 5 -- the config, made easier to evolve (app, after step 1)

From the config review (§4). Each needs `min_reader` in place first.

---

## 4. The config, reviewed

- **Flat and large**: `FractalConfig` has 78 top-level fields (view,
  camera, DoF, fog, depth, solid, filter, colour, palette ×7, tone ×~20,
  effects, Focused Rendering ×6, auto exposure, `engine_settings`), plus
  `escape` (43 fields; terrain, path tracer, shading, layers, textures),
  `sim` (25), `solid_shading`, `importance`, `flame`. Typical files are
  2.4-3.5 KB, solids ~6.5 KB, the largest fixture 41 KB.
- **Skip-if-default is done two ways**: ~41 fields by
  `skip_serializing_if`, ~37 by a hand-maintained list
  (`remove_default_fields`) that duplicates the field list; raw
  `serde_json::to_value` (the engine stash) skips the hand list. One way:
  `skip_serializing_if` everywhere, the list deleted.
- **Versions bump for default changes** (v3->4 levels gamma, v4->5
  shadow, v5->6 lights follow camera), and each bump locks every older
  reader out (§1, hazard 1). With `min_reader`, a default change no longer
  does.
- **`engine_settings`** (the stash of the 32 engine-owned fields for the
  engines not shown) is **not walked by the migrations**: a default change
  must be applied inside it too, and an unreadable stash silently resets
  that engine. It also duplicates whole palettes and Rhai effect sources
  inside the 256 KB blob, outside the server's content-hashed palettes.
  Migrations should walk it; palettes in it could be stored by hash.
- **Rhai sources ride in the blob** (`flame.custom_variations`,
  `custom_effects`, the escape and simulation custom slots) under the
  256 KB cap; after P6 a config could name a published script by id and
  version and carry the source only when unpublished.
- **Oddly placed**: `solid_shading` is top-level though engine-owned and
  escape-specific; `assets/presets.fflame` carries dead keys
  (`histogram_color_scale`, `low_density_smoothing`) and versions 1, 3 and
  6 -- re-save the presets once.
- Unchecked: the 16 KB per-transform cap against large Rhai array
  parameters.

---

## 5. Decisions for the user

1. **0.5's floor** (rule 6): **decided 2026-11-11: serve it filtered,
   with an update notice.**
2. **The variation slot policy cap: decided 2026-11-11, raised** (to
   ~4096; polychoron ships at 2410 > 512).
3. **`simulation` in `render_modes`**: **decided 2026-11-11, a hard
   requirement**: "Any config we currently support, flames, simulations
   or escape-time, needs to be able to be saved online." Simulations are
   stored (legacy-filtered), and every config the app can make must fit
   the server's limits (256 KB config, 16 KB a transform, 32 KB tracks).
   Check the largest real configs against them.
4. **Which effect WGSL the API holds** until P6 stores Rhai: **decided
   2026-11-11, the Rhai twins' generated WGSL.**
5. **An animation's `base_config`**: **decided 2026-11-11: not stored.**
   The flame is saved as its own flame and the animation references it by
   `flame_id`; on load the client makes the server's copy of that flame
   the `base_config` (`api::mod`). The copy sent on save is redundant: the
   server dropping it loses nothing. The client may stop sending it.
6. **How a client says what it reads: decided 2026-11-11, a URL
   parameter, `?reader=N`** (rule 2). A payload describes itself in its
   body (config `version` / `min_reader`), so writes need no negotiation.
   Reads have no body, hence the parameter, which the user prefers to a
   header. A request without it gets the original vocabulary.
   - **User-Agent detection was considered and dropped.** 0.5 sends
     ureq's default, `ureq/2.12.1`, but that is the last ureq 2 release,
     so any Rust client on ureq 2 would have been filtered too.
   - Escape-time is not in production yet, so no existing integration
     loses anything it sees today.
   - URL versioning (`/api/v2`) is kept for a change that cannot be
     additive.
7. **How far back to test: decided 2026-11-11, API integration is part
   of testing.** In the user's words: "It's easy to spin up a server
   whenever we need locally." So a local ffapi (with a scratch database)
   gets a test run that drives the client's API calls against it: save
   and load a flame, an escape config and a simulation, an animation with
   a script track, at reader 7 and with no reader.

## 6. Order

Step 1 first, in the app's next release, before any server change that
introduces new values. Then step 2 (the server shapes for legacy
clients), then step 3 (catch up), then step 4 (P6). Step 5 when step 1's
`min_reader` is in. The server work happens in ffapi, told about each
change as RELEASE.md §3 says.
