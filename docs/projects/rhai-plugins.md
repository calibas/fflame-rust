# Everything scriptable in Rhai: the plan

Recorded 2026-10-18 as the direction, from the user's statement of where
the program is going. Reviewed against the code and turned into a plan
on 2026-11-01; the user's decisions the same day are at the end and are
folded into the steps. **As of 2026-11-04, P0 to P4 are built, and P7
for the simulations; P5 and P6 and the rest of P7 are not** (*What is
left, as branches*, below).

## The goal, in the user's words

> Using them as plugins is perfect. That's actually my plan, to switch
> all plugins over to Rhai scripts. Everything scriptable: flame
> variations, escape-time formulas, simulation formulas, colorings,
> they can be used as plugin, saved within .fflame files, and
> downloaded from an API.
>
> They can declare parameters, the parameters can be targeted for
> animations and the undo/redo history.
>
> One more goal for Rhai scripts, they should be usable as animation
> tracks. The script would get the current animation time (delta?) and
> the config state of the fractal, and then be able to modify the state
> however you want.

2026-11-01:

> The overall goal is to have pretty much everything that can be edited
> as Rhai scripts. So Variations, Effects, Escape/orbit formulas,
> Escape/orbit coloring, Simulations, and custom animations. Plus
> generic scripts to modify or create new fractals, like the random ones
> we already have.
>
> They can be used as plugins, they can be saved/loaded as local files,
> and they can be downloaded from the API.

## Where each piece stands (2026-11-01)

| Kind | In Rhai | Local library | Carried in the `.fflame` | Plug-in (loaded at start) | From the API |
|---|---|---|---|---|---|
| Flame variations, 2D and 3D | yes (`fn variation`, `fn variation3`) | `formulas/`, beside the formulas | yes, a list (`Flame::custom_variations`) | WGSL JSON only (`plugins/variations/`) | WGSL JSON, a read-only catalog |
| Escape formulas, colourings, solids, hybrids | yes | `formulas/` (also UF `.ufm/.ucl/.frm`) | one formula and one colouring (`EscapeConfig::custom_formula`, `custom_coloring`) | no | no (only inside an uploaded flame) |
| Effects | no | -- | by name only | WGSL JSON (`plugins/effects/`) | WGSL JSON (sign-in required) |
| Simulation models and colourings | no (31 models, 13 colourings, inline WGSL) | -- | by name only | -- | -- |
| Animation tracks | no (`TrackSource` is `Keyframes` or `Signal`; a generator can emit a keyframe animation once, through `anim`) | -- | -- | -- | -- |
| Generators and modifiers | yes | `scripts/` plus 18 built in | -- | -- (library entries) | yes: publish, update, unpublish, search, adopt; Rhai source |

## Two Rhai worlds

The word covers two engines, and the design keeps them apart:
- **The GPU dialect** (`crate::lang`, its own feature since 2026-11-02): a typed subset of Rhai lowered
  to the formula language's IR and compiled to WGSL, run per sample or
  per pixel. Rhai only parses it (`Engine::new_raw()`); it never runs.
  Variations, formulas, colourings and solids today; effects and
  simulation rules next. It has no maps of the config, no I/O, no loop
  the GPU cannot finish; what it cannot express it refuses at check time
  with a reason.
- **The scripting API** (`src/script/`): full Rhai on the CPU, building
  configs, with a pinned PCG stream for `script + seed`. Generator and
  modifier scripts today; **animation tracks** belong here, since a
  track runs once a frame, not per sample.

They share the `rhai` crate and nothing else: two parameter
declarations (`const PARAMS` vs `param*` calls), two stores, two origin
models. The engines should stay two. The packaging around them should
become one (P3).

## Design rules

1. **One compiler, one parameter model.** Every GPU-side kind is a Rhai
   source with `PARAMS` and `INFO`, checked and compiled by
   `crate::lang`. A new kind (a simulation rule, an effect) is a new
   entry kind of the same language, not a new language.
2. **The source is the artefact.** A config carries the sources it
   uses and registers them on load (`escape::custom::ensure`,
   `variations::custom::ensure`), so a `.fflame` renders anywhere. A
   plug-in file and an API download are other places the *same text*
   comes from; compiled WGSL is at most a cache, never the thing
   stored. (Today's WGSL JSON plug-ins and downloads are the old form.)
3. **Parameters are `ConfigPath`s.** Whatever a script declares in
   `PARAMS` is addressed through `ConfigPath`, so it is animatable,
   undoable and scriptable by construction -- never a side channel the
   history or the timeline cannot see.
4. **Names and provenance.** A registered script has a name of its own
   and a provenance (built-in, download, plug-in, the user's own); a
   name is never shadowed, and re-registering the same provenance's
   source replaces it and bumps the registry's version.
5. **Determinism.** A script's output depends on its source, its
   parameters, the config and its seed alone -- the same on desktop,
   web and CLI, in the viewport and in an export -- so a still or a
   video is reproducible from the file.
6. **Hooks are the built-ins' features.** A Rhai variation reaches the
   flame shader's machinery through the same `Feature`s a built-in
   declares (random numbers, the accumulator, per-thread state, colour
   writes), so a script can do what a ported variation does and the
   shader builder has one code path.
7. **Identity: a UUID for the thing, a hash for the version** (decided
   2026-11-01; the API already works this way -- scripts and flames have
   ids and versions, palettes are content hashes).
   - **Built-ins of every kind keep their string ids** (`julian`,
     `vignette`, `mandelbrot`, a sim model's name): every config names
     them, scripts call them by name, and we keep them unique.
   - **Everything else -- the user's scripts and plug-ins, downloads --
     is a UUID**, minted on New, Save As, Duplicate, or importing a file
     that has none, and **written into the script's own text** (`INFO`'s
     `id` for the GPU kinds, the `script(...)` declaration for the CPU
     ones), so a copied `.rhai` keeps its identity with no sidecar.
   - **A version is the SHA-256 of the source.** A config refers to a
     script by its UUID and carries the source. On load the same UUID and
     hash use what is registered; the same UUID with another hash
     registers the carried version beside it, so the config renders as
     saved, and the UI offers the installed one.
   - **The namespace is the origin, not part of the key**: built in,
     mine, downloaded, carried by a config. Lists show it, and the trust
     rules use it (what a downloaded script may import or run). A plug-in
     of mine and the same plug-in downloaded back are one thing.
   - Names are free: two scripts may share one, and nothing is looked up
     by it.
   - Publishing keeps the UUID: the API should accept the client's id
     on create (server work), or the sidecar's link maps the two.
8. **Everything a person reads follows the language**: titles,
   descriptions, parameter captions and hints, choice labels. A field
   is a string (every language) or a map of locale to string
   (`#{ en: "Spiral", ja: "..." }`), resolved current locale, then
   `en`, then the first entry, and kept in the script so a plug-in is
   self-contained. Parameter *keys* stay plain identifiers -- they are
   what configs, undo and tracks address. Built-ins' labels, English by
   policy today, move to the same rule; translating them is its own
   work.

## Review (2026-11-01): what is disjointed

### Three ways in

1. **WGSL JSON** (variations, effects). Plug-in files
   `plugins/{variations,effects}/<name>.json` load at every start
   (`storage::load_installed_resources`); API downloads are cached under
   `variations/` and `effects/`; `Provenance` Builtin / Api / Local.
   Nothing in the UI installs or removes a plug-in -- `plugins::install`
   and `remove` have no callers outside their tests -- and on the web
   there is no way to add one at all.
2. **GPU Rhai** (`formulas/`, `storage::formula_store`, the Formula
   Editor). Nothing loads at start: an entry registers when the editor
   Uses it or a config's `ensure` finds it.
3. **CPU Rhai** (`scripts/`, `script::store`, 18 embedded). Its own
   origin model (`ScriptOrigin` plus `ScriptLink` in `_links.json`),
   deliberately outside `Provenance`; the only kind with an online
   library both ways. (One origin model since 2026-11-04:
   `provenance::Origin`, `Builtin` / `Local` / `Api`, for every kind;
   `ScriptOrigin` is `ScriptHome`, where the file is kept.)

### Inside the GPU dialect: one compiler, two of everything below it

One front end (`lang/rhai.rs`), checker, IR and WGSL back end.
Below that:
- **Three registries**: `escape::custom::Registry` (content-hash names,
  `custom-<fnv>`), `ifs::custom_solids`, and the variation registry with
  `Provenance::Local` (was `Custom`) plus `variations::custom`'s own source map.
- **Two ensure paths** with different failure memory, and **two
  outputs**: leaked `FormulaDef` / `ColoringDef` / `IfsDef`, or a
  synthesized `VariationDownload` pushed through `register_from_api` --
  Rhai variations compile *into the legacy wire type*.
- **Two parameter namings**: a complex parameter's parts are `name.re`
  / `name.im` in a formula (`escape/custom.rs`) and `name_re` / `name_im`
  in a variation (`variations/custom.rs`); errors are `Vec<Diag>` on one
  side and one string on the other.
- **Two naming schemes**: hashes for escape entries, the user's
  `INFO.name` for variations.
- **The cost of a kind**: the solid kind touched about a dozen files,
  and `EntryKind` has ~55 exhaustive matches across 9 files; Rhai
  variations touched 32 files.

### Five declarations of a parameter

`ParamType` / `VariationParamDef`, `EffectParameter` (the same
`ParamType`), `EscapeParamDef` (f32 plus `choices`), `SimParamDef` (f32
plus `choices`), and the CPU scripts' `ParamDecl` (Float, Int, Bool,
Choice, Text, Color). A GPU script's `PARAMS` is lowered into whichever
its kind uses.

### Carrying

Variations carry a list; escape carries one formula and one colouring;
effects, simulations and scripts carry nothing, so a config using an
effect that is not built in renders only where that effect is installed.

### Inconsistencies between the kinds

- ~~An effect plug-in's install checks only `check_download`, not the
  name~~ -- gone with the WGSL plug-ins (D3).
- ~~Effect downloads need a token and are gated by online mode only at the
  caller; variation downloads need neither~~ -- since 2026-11-04 neither
  needs a token or the account: a guest fetches both (ONLINE.md, *The
  online account, and guests*).
- ~~Updates and Clear Cache exist for variations only~~ -- fixed in P3
  step 3: the Effects panel updates a download and clears the cache
  (`EffectRegistry::clear_api`; revisions and `remove_by_name` came
  with P2).
- An effect download's WGSL is never parsed before it is registered: a
  broken one is a pipeline error at first use. (The built-ins are now
  validated by `every_built_in_effect_validates`.)
- ~~The Variations panel's "downloaded" banner also counts Local and
  Custom entries~~ -- fixed in P3 step 3: downloads only.
- ~~Adopting an online script whose stem matches one of the user's own
  overwrites it~~ -- fixed in P3 step 3 (`store::adopt_stem`: free of
  theirs too; the same online script replaces its copy).
- `ApiVariation` and `ApiEffectStage` have no users.

### The CPU sandbox is open (found 2026-11-01)

`script::host::build_engine` uses `Engine::new()` and disables only
`eval`. In rhai 1.25.1 (read from the crate source, not yet run):
- `Engine::new()` installs a `FileModuleResolver` on desktop, so
  `import "path"` loads and runs any `.rhai` file on disk.
- The core package registers a blocking `sleep()`, which the operation
  budget does not see: a script can hang the thread it runs on.
- The standard package's time functions include `timestamp()`, which
  breaks rule 5.

`sandbox_has_no_io` imports a module named `"std"` that does not exist,
so it fails on an open engine too and proves nothing. SCRIPTING.md and
the guide say there is no `import` and no clock. Scripts adopted from
other people run in this engine; P0 closes it.

### Docs that disagree with the code

- VARIATIONS_WIRE_FORMAT.md §8: the web client does enumerate cached
  variations now, and no stale version is refetched when a flame names
  it (only the panel's Update buttons fetch).
- escape-formula-language.md (arrays "refused") and ESCAPE.md ("arrays
  sized by a parameter" not done): both exist.
- SIMULATION.md lists 9 model features (the code has 10; `ExactDiscs`
  is missing); the colouring signature in `sim/mod.rs`'s doc comment is
  the old `(s, grad, p)`.

## What a new GPU kind needs from the dialect

The dialect cannot read a texture or another pixel (nothing in
`src/lang` does), has no `vec2` or `vec4`, and its outputs are a
point, a palette index or a distance -- never a colour. Effects and
simulations need exactly what is missing:

| Needed | Effects | Simulation colourings | Simulation models |
|---|---|---|---|
| Read the input at a position or pixel | 10 of the 16 built-ins | -- | -- |
| Read a cell at an offset | -- | through `SimSample` | every model (`sim_read(p + o)`) |
| `vec4` in and out | RGBA | rgb and coverage | the cell's state |
| Values from the uniform | size, `density_mean`, `pixel_scale` | the palette, `SimSample`'s fields | `dt`, step index, random numbers |
| More than one pass | `distance_field` only | -- | 8 of the 31 |
| Beyond that | the blend library | -- | kernel tables, layers and couplings, memory, pyramid, agents |

Effects are the smallest step. 15 of the 16 are one pass: 6 are pure
functions of the pixel's own texel, and 9 more read other pixels at
computed positions or over bounded kernels (at most 21x21 = 441 taps,
under `LOOP_CAP` 4096). `distance_field` -- seed, `ceil(log2(size))`
floods, composite, special-cased by name in `effect_chain.rs` -- is the
only one that needs passes.

## The plan

### P0. Close the CPU sandbox, keeping `import` (small; first)

**Done 2026-11-01** (branch `rhai-plugins`): `host::LibraryResolver`,
`check_library`, `ScriptKind::Library`, `sleep` / `timestamp` refused,
`untrusted_code_running` for `run_script` and `import` alike; the panel,
the listing and the Palette Editor read scripts through the whole
library (the Palette Editor also now marks downloaded scripts, which it
never did). **For the API repository**: the contract's `scripts.kinds`
gained `"library"`, which a publish now sends -- the server must accept
it on create.

The user wants scripts to import other scripts. Half of that is already
safe: `run_script("id", #{...})` resolves the id through the script
library, never the filesystem, catches cycles, caps nesting at 8,
charges one shared operation budget, and lets a downloaded script call
only shipped ones (`host::run_sub_script`). But it runs a whole script
against the shared config; it does not give the caller functions.
Rhai's own `import`, as `Engine::new()` sets it up, is the unsafe half:
a path on disk, desktop only.

- **`import` through the library.** Replace the `FileModuleResolver`
  with a resolver over the same library `run_script` reads, with its
  rules: the downloaded-script restriction (checked before the name is
  resolved, as there), cycle and depth limits, the shared budget. No
  filesystem, so it works on the web and in the CLI alike (rule 5).
- **A library kind.** Rhai runs a module's top-level statements when it
  is imported, so importing a generator would run its body against the
  config. A third `ScriptKind`, a library of functions and constants,
  is what `import` accepts; a generator or modifier is refused with a
  pointer to `run_script`. Libraries sit in the Scripts panel like the
  other kinds (shipped ones in `assets/scripts/`, the user's in
  `scripts/`) and publish like them.
- Remove `sleep` and `timestamp` -- or build from `Engine::new_raw()`
  with only the packages the API needs -- and audit what else the
  standard package brings in.
- Rewrite `sandbox_has_no_io` so each attempt would work on an open
  engine: import a real `.rhai` file by path, `sleep(0)`, `timestamp()`.
  New tests: a library imports and its functions run; a downloaded
  script importing a user library is refused; an import cycle is named.
- Make SCRIPTING.md and the guide say what is true.

The GPU dialect's equivalent -- helper functions shared between
formulas -- is not part of P0; `vary("name", z)` already pulls a
library variation in by name.

### P1. One machinery for every GPU kind, in namespaces

Before a fourth registry is written for effects:
- **Retire the WGSL plug-in system first (D3).** Stop loading
  `plugins/{variations,effects}/*.json`; remove `storage::plugins` and
  its `PluginKind`. For one release, a start that finds files there
  logs that they are no longer loaded, naming them. The WGSL API
  catalogs and their caches stay (D2). **Done 2026-11-01**: the module,
  `Provenance::Local`, `local_plugin_dependencies` and its save and
  upload notices, and `assets/plugins-example/` are gone;
  `storage::retired_plugin_files` names leftovers (a notice in the app,
  stderr headless); `MissingReason::ProbablyAPlugin` is `NotInCatalog`.
- **Identity and origin (D6, rule 7)**: built-ins keep their string
  ids, so every existing config and `.flame` reads unchanged;
  everything else is keyed by the UUID in its source, with the source's
  SHA-256 as its version, and carries an origin (built in, mine,
  downloaded, carried by a config) that every list shows. (Built
  2026-11-04 as three, at the user's choice: `provenance::Origin`'s
  `Builtin`, `Local` -- mine, and a source a config carries -- and
  `Api`.) The WGSL
  variation and effect catalogs are server copies of the program's own
  built-ins, so what they fetch is a built-in this build lacks, under
  its string id.
- **Labels follow the language (rule 8)** in the shared parameter and
  `INFO` lowering: a string or a locale map wherever a person reads it.
- One registry for compiled Rhai entries, keyed by kind and id, with
  one `ensure` from a config's carried sources, one failure memory, one
  error type (`Vec<Diag>`), and a provenance on every entry. (The one
  `ensure` is `FractalConfig::ensure_sources()`, 2026-11-04, over the
  four kinds' own; the registry, failure memory and provenance are
  still per kind.)
- One parameter lowering from `PARAMS`, from which each kind's def is
  built; one complex-part naming (`.re`), reading `_re` as an alias so
  configs saved with Rhai variations keep their values.
- Rhai variations register as a variation def directly, not through a
  synthesized `VariationDownload`.
- References: a transform names a Rhai variation, and an effect list a
  Rhai effect, by its UUID; parameters are `<uuid>.<key>`. The escape
  slots, which hold one carried source each, move from their FNV names
  to the same UUID and hash. **Done 2026-11-02** (below, *Order taken*).
- The kind as one value in the lowering rather than four bools, so the
  next kind costs a table entry, not fifty matches.

Exit: every test, shader dump and visual config unchanged. No new
feature.

**Order taken (2026-11-01).** Built first, as the core every kind
shares: `crate::scripted` (UUIDs, versions, the `Ledger`, `Label` and
`Labels`), labels by locale in the dialect, `with_info_id`. Effects (P2)
were then built on it directly, with UUID identity from the start, so
they needed no migration and tested the core.

**Rhai variations and the escape slots moved onto it, 2026-11-02** (the
user's order: "the id@version keys for effects, then the variation and
escape-slot migration on top of them"). Versions live side by side, as
effects' do: every version is its own registry entry, and a config
renders the one it carries.
- **One key format** for both, `scripted::key(id, source)` =
  `u<32 hex of the UUID>_<first 8 hex of the source's SHA-256>`: an
  identifier, because a variation's key is its WGSL function's name and
  its parameters' prefix. `scripted::key_id` reads the id back. (Effects
  were `<id>@<version>` in one commit on this branch, a3d5c947; a config
  saved by that build still renders, as `ensure` registers under the
  entry a config names, but Use does not move it to the new form.)
- **Variations.** The id is `INFO.id`; a source with only `INFO.name`
  derives one from the name (`scripted::derived_id`, a v8 UUID), so it
  keys the same everywhere without being rewritten. `INFO.name` stays
  what `vary("name", z)` calls, so a formula's text needs no ids.
  Config **v7** renames every carried variation from its name to its key
  wherever the config names it. `ensure` registers a source under the
  entry the config names, so an unmigrated config (an `.anim`'s base,
  which derives `Deserialize`) still renders. Use mints a UUID only for
  a source with neither an id nor a name, or a hand-written id; a new
  version re-keys the config (`custom::rekey`, one `apply_config_edit`
  step); Live follows the id; pickers offer one version of each id
  (`VariationRegistry::superseded`); the sticky shader set drops older
  versions of a live id.
- **Escape slots.** A Rhai entry with an id is `custom-<key>`, its
  version over the source and the variations it carries; any other
  keeps its FNV name, and a config keeps the name it was saved with, so
  no migration: no Rhai formula had an id before. Use mints one into the
  file. A variant and the perturbation tier take the slot's 64-bit hash
  through `custom::slot_hash`, which reads an FNV name's own hex and
  hashes a key. Live and a first Use's view follow the id.
- **Known gaps.** An animation track that names a variation by a key a
  later Use replaced still names the old one (the base config moves, the
  track does not); a subflame's transforms are not re-keyed by Use (the
  flame's own are, and the subflame renders the version it carries).

### P2. Effects in Rhai

**Status 2026-11-01** (branch `rhai-plugins`): steps 1-5 built, on the
shared core (rule 7's UUIDs, rule 8's labels) rather than after the rest
of P1 -- see *Order taken* below. **Step 6 done 2026-11-02**: every
one-pass built-in has a twin in `assets/effects/*.rhai` -- the twelve
colour effects match their WGSL within a step of 8-bit rounding at four
settings each (`the_rhai_twins_match_their_wgsl`), the three density
ones to a part in a thousand (`the_density_twins_match_their_wgsl`),
each with the built-in's parameters, types, order and category. Two
additions made that possible (user's answers, 2026-11-02):
- **`hash2(p)`** emits the noise effects' shared hash as their shaders
  write it. The dialect's `sin` is range-reduced (`uf_reduce`, for
  Metal's error), so a hash written in Rhai was a different noise; the
  built-in gives the same pixels. With it, `fract`.
- **`type: "angle"`** in a `PARAMS` entry, and `param_angle` in the CPU
  API: a float in degrees, drawn with the angle control. Scripts read
  degrees, as stored and shown.

**The compiler's own feature, done 2026-11-02** (the user: "compile
Rhai to WGSL" in every WASM module, the Rhai scripting module an
option): `src/escape/lang/` is `src/lang/`, the `lang` feature, which
`engine-escape` brings; the deep-zoom passes (`bigeval`, `periodic`,
the interpreter's steppers) stay with `engine-escape`; Rhai variations
and effects need `lang` alone, and the flame and simulation gallery
modules turn it on. **Its price**: the flame module from 1.21 MB to 1.55
MB gzipped (5.2 to 6.3 MB raw), measured before `wasm-opt`. (The `lang`
feature went with the engine features on 2026-11-04, when the
single-engine modules retired: every build has the compiler.) The CPU
scripting module (`src/script`) is compiled into every build and linked
only where something calls it (wasm/script, the app), so it is already
an option by use; a `scripting` feature would make it one by name.
Step 7 can now go ahead.

1. **Dialect**: `vec4` (split into scalars like `vec3`); an `effect`
   entry kind returning a `vec4`; the pixel's position, the image size,
   `density_mean` and `pixel_scale` from the context; `input(uv)`
   (filtered, `textureSampleLevel` at level 0, which is legal in
   non-uniform control flow, so the WASM rule holds) and `input_at(x, y)`
   (`textureLoad`, clamped to the image); `blend(base, fx, mode, amount)`
   over the blend library, with `BLEND_MODE_NAMES` as a `choices`
   shorthand. Exact names are settled when it is built.
2. **Assembler**: the effect template around the compiled function (the
   `EffectParams` uniform, `get_param`, `vs_main` / `fs_main`, the blend
   library spliced only when called); at most 48 parameters
   (`MAX_EFFECT_PARAMS`).
3. **Registry**: a Rhai effect becomes an `EffectInfo` with
   `EffectSource::Owned` WGSL and its carried source, `Provenance::Local`,
   colour or density from `INFO`.
4. **Carrying**: `FractalConfig::custom_effects: Vec<CustomSource>`
   (skip-if-empty), ensured in `render_with` so the CLI, video and
   thumbnails have it.
5. **Editor**: the Formula Editor learns effect entries (Use as effect);
   the Effects panel gets Edit as Rhai on a built-in.
6. **Twins**: the 15 one-pass built-ins written in Rhai
   (`assets/effects/*.rhai`), each held to its WGSL by a GPU pixel
   compare (skipped without an adapter), under the same names with the
   same parameters in the same order -- so no config can tell which it
   got.
7. **Replace the built-ins (D1)**, each once nothing is lost: its twin
   matches the WGSL's pixels; it is no slower (timed on the user's
   GTX 1660 SUPER at 1080p and 4K); it runs on the web (WebGPU); a
   config that names it reads the same values; the online effects
   corpus still exports (`export_effects_json` writes the compiled
   WGSL while the WGSL catalog stays, D2); and the desktop's edit
   without a rebuild survives (the on-disk copy becomes the `.rhai`).
   One that fails a check keeps its WGSL until it passes.
   `distance_field` keeps its WGSL (three passes) unless passes are
   added to the dialect, which is only done if wanted.

   **Built 2026-11-02**: all fifteen are their Rhai sources
   (`effects::custom::BUILTIN_TWINS`; [EFFECTS.md](../main/EFFECTS.md#effects-written-in-rhai)).
   The WGSL is kept as the reference and as what a build without `lang`
   registers (`effects::wgsl_builtins`; only the test matrix's
   single-engine builds lack it -- every shipped module compiles Rhai,
   the user's answer of 2026-11-02). The checks, as tests:
   - pixels -- the two compares, now against `wgsl_builtins`;
   - the values a config reads -- names, types, defaults, ranges, **labels
     and hints** (`reference_beside_twin`). This found six differences
     the earlier shape check missed (`density_blur`'s "Max Radius" for
     "Radius", "Threshold" for "Density Threshold" and two missing hints,
     `kaleidoscope`'s "Rotation" for "Source Angle", `julia`'s "(real)"
     case) and two titles ("Hue Cycle", "Julia"); the twins were fixed;
   - the web -- statically (`the_builtin_twins_are_web_safe`: explicit
     sampling, Tint's operator rule, Metal's self-ops, subnormals); **a
     run in a browser is the user's to do**;
   - the corpus -- `export_effects_json` writes all sixteen, the twins'
     compiled WGSL with the blend marker;
   - the desktop's edit without a rebuild -- `assets/effects/<name>.rhai`
     beside the app, when it differs and compiles; the WGSL files carry
     a note that an edit there reaches nothing
     (`scripts/mark_effect_wgsl_references.py`);
   - speed -- `the_twins_are_no_slower` (ignored; 1080p and 4K, nine
     rounds of a ~20 ms batch, the median of the rounds' twin/WGSL
     ratios; more than 5% slower fails), run on the user's GTX 1660
     SUPER with the app closed. The first run had nine failures, every
     one from the compiler's code, fixed there rather than the twins
     dropped (the user: "I want Rhai to be first-class and compile to
     equivalent WGSL"): trig range reduction in effects (plasma 1.54x),
     the loop cap's second counter (julia 1.44x, `density_blur`), and
     `density_blur`'s twin skipped the taps its shader skips. Then
     kaleidoscope (1.09-1.13x) and julia (1.09x), found with
     `time_effect_variants` -- WGSL files timed against the built-in, the
     twin moved toward the WGSL one change at a time and the WGSL toward
     the twin: julia's loop test `!(a && b)` (6%; now one `break` per
     part, everywhere), kaleidoscope's int parameters read through i32
     per pixel (6%; now the float rounded, in effects), `min`/`max`/
     `clamp` as tests (2%; now WGSL's own, in effects), the `atan2`
     guard's branch and `floor`'s int round trip. Kaleidoscope's twin
     with the WGSL's own body inside the template ran at 1.001x, so the
     template costs nothing. Now all fifteen are within 3% or faster
     (julia 4% faster, `density_blur` 5%).
   Startup: compiling fifteen twins takes about 25 ms (native release; a
   shipped twin skips naga's validation, which the tests do). Step 5's
   remainder came with it: **Edit as Rhai** on a built-in's instance in
   the Effects panel makes it the user's copy, in use in the editor.

Exit: an effect written in the editor is saved in a `.fflame` and
renders on another machine with no plug-in installed; every built-in
that passed is its Rhai source.

### P3. One library

- One library over the GPU store (`formulas/`) and the CPU store
  (`scripts/`): the same metadata (name, kind, description, author,
  version, provenance, online link), the same sidecar (`_links.json`
  extended to `formulas/`), the same file actions, one scan. The two
  folders stay -- UF files live in `formulas/` -- so no user file moves.
- **A plug-in is a library entry flagged to load at start**: any kind,
  registered at start in the user's namespace, so a Rhai variation or
  effect is in the lists without a config carrying it. The plug-in UI
  -- enable, disable, import a file, remove -- works on the web too.
  This replaces the WGSL plug-in system retired in P1.
- Fix on the way the inconsistencies this touches: the adopt overwrite,
  the banner, the effect install's name check, updates and Clear Cache
  for effects.

**Status 2026-11-02: steps 1-4 built** (branch `rhai-plugins`). What P3
leaves for P6: publishing and adopting a GPU Rhai entry online (the
formula store's link record holds a cloud id already, unused); a
library panel of both stores together, if one is wanted -- the scan
(`storage::library::scan`) is ready for it, and nothing draws it yet.

**The steps** (written 2026-11-02, from the code as it stands):
1. **One sidecar, one metadata** (built 2026-11-02). `storage::links`: the record
   `scripts/_links.json` keeps (`ScriptLink`: cloud id, version, owner,
   `from_others`), plus `plugin`, for both stores --
   `formulas/_links.json` keyed by file name, as a formula file's
   extension is part of its name. Deleting a formula forgets its link,
   as deleting a script does. `INFO.description` and `INFO.author` in
   every GPU kind (a description by locale, as a title is). A scan,
   `storage::library::scan()`, gives one entry per file of either store
   and per shipped one (the script starters, the formula examples, the
   effects): its store and file, kind, title, description, author, id,
   version, origin (shipped, yours, downloaded), link and plug-in flag.
2. **Plug-ins** (built 2026-11-02, as written here). A Rhai variation, effect, formula, colouring or solid
   in `formulas/` with `plugin` set is registered at start, where every
   entry point registers the downloads (`load_installed_resources`,
   desktop, web and CLI), under its key in the user's namespace. It is
   then in the lists without a config carrying it: the variation
   pickers (already), the Effects panel's Add list, and a group of the
   user's own in the escape panel's formula and colouring pickers --
   each choosing it carries its source into the config, as Use does.
   The Formula Editor's file list marks plug-ins and gives the open file
   a "load at start" switch; Import and Delete are its own, on the web
   too.
3. **The fixes** (built 2026-11-02; `store::adopt_stem`, `downloaded_variations_in_use` on `is_cached_download`, `EffectRegistry::clear_api` and the Effects panel's Update and Clear buttons): adopting an online script never overwrites one of the
   user's (a stem free of theirs too, or the same script by its cloud
   id); the Variations panel's "downloaded" banner counts downloads
   only; effect downloads get updates and Clear Cache as variations
   have.
4. **One presentation** (built 2026-11-02; `storage::library::marks`): the Formula Editor's list and the Scripts
   panel show the same marks for an entry's origin and plug-in flag,
   from the scan; the shipped effects are in the editor's list beside
   the formula examples.

### P4. Simulations in Rhai

1. **Colourings first**: a colour function over `SimSample`'s fields
   returning rgb and coverage, with `palette(t)`; one pass and no reads
   beyond the sample. **Built 2026-11-02**
   ([SIMULATION.md](../main/SIMULATION.md#colourings-written-in-rhai)):
   `fn cell_color(cell, p, ctx)`, `EntryKind::SimColoring`; the sample
   through one built-in (`__cell`, by a literal index) rather than a
   predefined name per field, and the effect's code generation hosted by
   the colour template; the features from what it reads; carried on
   `SimConfig`, ensured where a render starts; Use from the Formula Editor
   and a plug-in group in the panel. The twin is Channel's, equal to 1e-5.
   A colour stack's entries name built-ins only: carrying one source per
   entry is a later step if wanted.
2. **Models, the single-layer class**: a step function with neighbour
   reads at offsets (the boundary stays the framework's), `vec4` state,
   `dt`, the step index, random numbers, a seed function; up to four
   passes. Gray-Scott is the first twin. Kernel tables, couplings,
   memory, the pyramid and agents stay built-in, refused with a reason.
   **Built 2026-11-02**
   ([SIMULATION.md](../main/SIMULATION.md#models-written-in-rhai)):
   `fn cell_step(cell, p, ctx)` to `cell_step4`, `fn cell_seed(inside,
   noise)`, `EntryKind::SimModel`; `read(dx, dy)` through `__nbr_*`
   built-ins and `sim_read`; the working values (`dt`, `max_dt`, `steps`,
   `diffusion`, `state`) from `INFO`, and the built-in-only keys refused
   with a reason. Gray-Scott's twin steps the built-in's field to 1e-5
   over 600 steps, at its speed (median ratio 0.98). Each component of a
   neighbour is its own `sim_read` in the WGSL; the driver merges them
   (measured on NVIDIA), and a browser's or Metal's compiler is expected
   to, unmeasured. Presets stay the built-ins' (step 6); layers are
   step 4.
3. **Carried**: custom model and colouring sources on `SimConfig`,
   ensured like the escape slots. **Built** with steps 1 and 2:
   `custom_coloring`, `custom_model`, `sim::custom::ensure` at every
   `SimRenderer` entry point that takes a config.

**Full support (agreed 2026-11-02).** A survey of the 31 built-in models
and 13 colourings against steps 1-2 found 9 models outside the
single-layer class by design (kernel tables: Lenia, SmoothLife,
Lattice4, Turing -- which also takes a drive and publishes a signal;
the pyramid, memory, a measure pass and min/max: McCabe; agents:
Physarum, DLA; a repeated relax pass and min/max: DBM, Fingering). The
other 22 models and every colouring fit, but faithful twins need the
steps below. Ising and Cyclic CA were compiled as trial ports
(`output/sim-ports/`): integers, bit operations, parameter-bounded
nested loops and neighbour reads at loop offsets already work.

4. **Layers**: a Rhai model in any layer and a Rhai colouring in any
   colour layer. The source is carried beside the name
   (`SimLayer::custom_model`, `SimColorLayer::custom_coloring`; per
   entry, so adding, removing, promoting and demoting a layer carry it
   with no pruning, and undo restores both together), ensured for every
   entry. Plug-ins in every slot's picker; the Formula Editor's Use
   picks the layer; Live updates every slot that carries a version.
   A new version of a layer's model keeps its parameters, as `SimModel`
   does. GPU side unchanged: a layer's model is its own step module.
   **Built 2026-11-02**: as written, plus choosing a built-in drops the
   slot's source; a two-layer coupled run with Gray-Scott's twin in
   layer 1 and Channel's in colour layer 1 matches the built-ins to 1e-5
   (`rhai_models_and_colourings_run_in_layers`).
5. **The language, for twins**:
   - `rand(k)`: a draw on stream `k` (`sim_rand(p, k)`), in steps and
     seeds; seeds may draw (10 built-in seeds do). A bare `rand()`
     keeps its call-site stream.
   - `INFO.time_step: false` (`NoTimeStep`, the 12 automata) and
     `INFO.settles: false` (`NeverStills`).
   - `init_at(x, y)` in a seed: the init shape at another cell
     (Wolfram's middle row).
   - `break` and `continue` in loops; an early `return` in the
     effect-like functions (effects, colourings, models).
   - Colourings: `ctx.step` (Age), `read(dx, dy)` (`ReadsCell`, LIC),
     `cell.memory` (`ReadsMemory`, Scale Memory), `pcg(n)` and
     `ctx.seed_lo` / `ctx.seed_hi` (Label, LIC's noise).
   **Built 2026-11-02**, as written; the memory is `cell.m0` and
   `cell.m1`, as the template's sample names it, and the seed words
   and `pcg` are a model's too. A Rhai `break` outside a loop is
   Rhai's own parse error; a helper or an `if` value is sealed from its
   caller's loops and return (`seal`). Held by
   `the_language_of_the_twins_compiles`; the twins (step 7) hold the
   semantics on the GPU.
6. **Model working values**: `fn max_dt(p)`, the stable time step as a
   function of the parameters, evaluated on the CPU (`dt_bound`; six
   models); `INFO.presets`, a model's presets as the built-ins' are.
   **Built 2026-11-02**: `max_dt` is plain Rhai on the CPU, the source
   kept in the registry by name and parsed once per thread (rhai is
   built without `sync`, so an AST is not shared); `dt_bound` is one
   function for every script, `script_dt_bound`, which finds the script
   by the model's name. Presets go through JSON into the config's own
   serde types (`lang::rhai::info_json`). Held by
   `a_model_has_its_own_time_step_and_presets`.
7. **Twins**, in batches, each held to its built-in (a field compare,
   bit-exact where the arithmetic is the same, to a tolerance where a
   chaotic rule amplifies the last bit) and timed against it: the 21
   remaining in-class models and the 12 remaining colourings.
   Built-ins stay (D2); the twins ship as Rhai sources.
   **Models built 2026-11-02**: all 22 in-class models
   ([SIMULATION.md](../main/SIMULATION.md#models-written-in-rhai)),
   every preset's field bit-identical to the built-in's; equal speed but
   Brusselator2 (2-3%, unexplained, under the gate). The port found two
   more language needs, built with it: `ctx.step` an int (a float loses
   whole steps past 2^24, and Ising's parity with them) and `ctx.layer`
   (Ballistic Deposition draws at another cell, as `sim_rand` there,
   from `pcg` and the seed words). And two compiler fixes from the speed
   check: one read per neighbour, an int offset kept an int.
   **Colourings built 2026-11-02**: all 13, every picture of the compare
   bit-identical; at speed but LIC (3-4% slower) and Gradient (6%
   faster), under the gate.
8. **Out of the class**, unchanged: the 9 models above stay built-in,
   refused with a reason. *Superseded by step 9 (the user,
   2026-11-02: "I'd like to be able to cover everything").*
9. **The framework's stages, for the other 9** (agreed 2026-11-02, not
   started). Nothing in them is beyond the compiler: each is a stage the
   framework runs outside the step, which a script asks for in `INFO`
   and reads (or writes) through builtins -- the pattern `max_dt(p)`
   and `init_at` set. In order, cheapest first, each with its twins'
   field compare and timing as step 7's:
   - **9a. The field's min/max and a repeated pass** -> DBM, Fingering.
     `INFO.min_max: true` (`NeedsMinMax`) and `ctx.field_min` /
     `ctx.field_max` (the ring slot the next step reads, one step of
     lag as the built-ins'); `INFO.repeat: #{ pass: 2, param: "relax" }`
     (`ModelDef::repeat`), the framework running the pass that many
     times a step. **Built 2026-11-02**: the range a host value (no
     `INFO` key: reading it is `NeedsMinMax`, as drawing is
     `NeedsRng`); DBM's and Fingering's twins bit-identical at every
     preset and at speed (0.995, 0.996).
   - **9b. Kernel tables** -> Lenia, Lattice4; then exact discs ->
     SmoothLife; then the drive and the published signal -> Turing.
     `fn kernel(p)` run on the CPU as `max_dt` is, giving the table (its
     radius and taps, one or two blocks), uploaded as the built-ins'
     `kernel` closure's is; `kernel_radius()` / `kernel_tap(i)` in the
     step. `INFO.exact_discs` (`ExactDiscs`) and a read of the spectral
     stage's disc averages. `ctx.drive` (`TakesDrive`) and
     `INFO.publishes_signal` (the pass that writes the signal channel).
     A built-in's table is computed in f32 Rust and a script's in f64
     Rhai: these twins match to a tolerance, and a chaotic rule (Lenia)
     drifts over a long run, unless the table is made in f32.
     **Built 2026-11-03**: as written, the CPU functions one runner
     (`with_script`, `cached`: a table is thousands of taps of Rhai, so
     cached on the parameters' values); `tap(i)` and `disc(i)`
     builtins; `drive()` a vector; reading each implies its feature.
     Rhai's int division joined the language (`IDiv`: the seeds' block
     indices). The twins of Lenia, Lattice4, SmoothLife and Turing: the
     tables within 1e-5 of their largest weight, the steps bit-identical
     on the built-ins' own tables at every preset (`swap_kernel`), and
     four coupled Turing twins bit-identical to four built-ins
     (`a_rhai_turing_couples_as_the_built_in`); on their own tables
     Turing and most of Lattice4 drift under 4e-6 over 300 steps,
     Lattice4's cells presets 5e-5, Lenia's age channel by whole steps.
     All four at speed (1.001, 1.004, 1.024, 0.993; the timing's rounds
     now about 200 ms each, so a wide kernel's run stays minutes).
     Lattice4 and Turing declared no `NeedsRng` though their steps draw:
     corrected (a comment in the step shader; the texture engine
     re-pinned, no bump).
   - **9c. The pyramid, memory and a measure pass** -> McCabe. Reads of
     the pyramid's averages at a radius (`pyr_sample`, `pyr_exact`,
     `pyr_diff`), and the step writing more than its `vec4`: the
     memory's slices (`sim_mem_write`) and the measure's scratch. New
     to the language: a function's outputs beyond its result -- a
     result map (`#{ state, memory }`), as a variation's `#{ z, color }`
     is, rather than side-effecting calls the IR has none of.
     **Built 2026-11-03, as calls, not a result map**: the IR gained
     n-ary host calls (`IExprKind::Host`, `IStmt::Host`), the writes
     statements -- `write_scratch(k, v)`, `write_memory(k, v)` -- which
     are the template's own calls one for one; a result map would have
     needed the scratch's slices and the memory's in one shape, and
     McCabe writes them from different passes. The reads `pyr_sample`,
     `pyr_level`, `pyr_diff`, `scratch(dx, dy, k)`, `memory(k)`,
     `outside`, `param(k)` (a table's rows by slot); `ctx.scale_warped`,
     `ctx.scratch_on`, `ctx.memory_count`; `INFO.measure` and
     `INFO.scale_warps` (the per-scale warps, a registry beside
     `SCALE_WARPS`); `NeedsPyramid` and `Memory` from what the step
     reads. The checker's cap on a model's parameters is the layer's
     block, 124 floats (McCabe has 82). Helpers take `p` and `ctx`
     whole. McCabe's twin bit-identical at every preset and on every
     other path (`a_rhai_mccabe_runs_every_path_as_the_built_in`: the
     shifted grid, exact discs, a weighted leaning table, symmetry, the
     variation radius, warps, the memory's slices) and at speed (1.024;
     1.027 with the variation radius, from 1.104 before a compiler fix:
     a fresh array is its `let`'s local, not copied into one; the
     pyramid's caches filled only where a path reads it, as the
     built-in's are, measured no difference). The per-scale table's grid
     was the built-in's alone (`PARAM_TABLES`) until the parameter
     layout (P7, below).
   - **9d. Agents** -> Physarum, DLA. A second entry point, per agent
     rather than per cell (`fn agent_step(a, p, ctx)`): its record
     (`vec4`s), its own draws (`agent_rand`), reads of the field around
     it, and deposits into the accumulation buffer (atomics, as
     `agent_deposit`); `INFO.agents` sizing the population. The largest
     piece: a new template for the compiler to fill.
     **Built 2026-11-03**: `fn agent_seed(p, ctx)`, `agent_step(a, p,
     ctx)`, `agent_step2`, each lowered as a step is (`SimFn::AgentSeed`,
     `Agent`) into the agent template's functions, the record a vector of
     four (x, y, heading, state); the population `fn agent_count(p, w,
     h)` on the CPU rather than an `INFO` key, since it is a function of
     the grid. `claim`, `claim_check`, `deposit`, `read_at`,
     `take_deposit`, `ctx.agent`, `ctx.periodic`; `rand(k)` in an agent is
     its stream. Physarum's and DLA's twins bit-identical at every preset,
     every agent's record included, Physarum against walls too
     (`the_rhai_agent_models_move_as_the_built_ins`), and at speed (0.999,
     1.013). DLA's needed its launch sum's terms in the built-in's order
     for the shader compiler to fuse them alike. **Found**: the built-in
     DLA read its launch gap from `crowding`'s slot and its crowding from
     `launch_gap`'s (`mparam(2u)`, `mparam(3u)`), swapped since it was
     written. **Fixed the same day** (the user: DLA is unreleased, swap
     the saved configs): each read from its own slot, built-in and twin,
     and the defaults, the preset and the saved setups swapped to the
     values in effect, so no picture moved (SIMULATION-CATALOG.md, DLA).
     **All 31 models now have twins.** A script marks its slow
     parameters, `INFO.slow` (the built-ins' `SLOW_PARAMS`), so the four
     twins with one commit them when a drag ends.

### P5. Animation tracks as scripts

`TrackSource::Script`: CPU Rhai, compiled once per track (the AST kept,
not a new `Engine` each frame), run each frame after the keyframe
tracks.
- **A pure function of time (D4)**: the same time gives the same
  values, whatever came before, so export, scrubbing and motion-blur
  moments agree; anything that accumulates is recomputed from the time.
  No state survives from one frame to the next.
- In: the time, the duration and frame rate, the frame's config (read
  only, after the keyframes), the track's own parameters. Out: values
  for the targets the script declares -- declared so the timeline can
  show them and a clash with a keyframe track on the same target is
  visible.
- The same pairs reach both appliers: the live one
  (`update_param_silent_on`) and the export's (`apply_animation_values`).
  A per-frame operation budget, smaller than a generator's.
- Carried in the animation as source; P0's libraries can be imported.

### P6. The API: one resource for every kind

Server work: tell the API repository (RELEASE.md §3; the contract's
shape fingerprint will not move by itself).
- The scripts resource (publish, update, unpublish, search, versions,
  public and private, Rhai source) carries every kind by a `kind`
  field: a variation, effect, formula, colouring, solid, simulation
  rule, track or generator is published, found and adopted the same
  way.
- Adopting saves into the library, flagged as a plug-in if wanted. A
  config that carries its sources needs nothing from the API.
- The WGSL catalogs stay beside it (D2).

### P7. Every built-in editable as Rhai

> The eventual goal is to port everything over to Rhai so the user can
> edit any formula or coloring. (The user, 2026-11-02.)

For each kind, two things: a **twin** of every built-in, held to it
(its settings, its picture and its speed, as P4 step 7 holds the
simulation twins), and an **Edit as Rhai** where the built-in is chosen
that puts the twin in its place and opens it in the Formula Editor as
the user's own -- what the Escape panel's Edit as Formula and the
Effects panel's Edit as Rhai already do. Whether a twin then replaces
its built-in (as D1 did for the effects, and as the escape plan's step
9 retires native code family by family) or sits beside it is decided
kind by kind (D7).

Where each kind stands (2026-11-02; the counts below are from a census
of each built-in's features and WGSL, not from ports -- P4's ports found
needs the census did not, the int step and `ctx.layer`):

| Kind | Built-ins | Twins | Edit as Rhai |
|---|---|---|---|
| Effects | 15 | all 15, the definitions (D1) | yes, the Effects panel |
| Escape formulas | 26 | all 26 (`escape::twins`) | yes, Edit as Formula |
| Escape colourings | 28 | 28 (six batches, branch `rhai-escape-colourings`; plan in step 2) | yes, the Escape panel's colouring (2026-11-05) |
| Mode D solids | 6 | none held; examples for Mandelbox, the KIFS Menger, a quaternion | no |
| Mode B fields | 3 | none | no |
| Simulation models | 31 | all 31 (P4 steps 7 and 9, done 2026-11-03) | yes, the simulation panel (2026-11-03) |
| Simulation colourings | 13 | all 13 | yes, the simulation panel (2026-11-03) |
| Flame variations | 647 | 444 (`variations::twins`, branch `rhai-variation-hooks`; plan in step 3), besides the 10 test fixtures | yes, a transform's variation (2026-11-05) |

Steps, in the order that unlocks the most first (the user picks):

1. **The interpreter's flow statements.** P4 step 5 built `break`,
   `continue` and an early `return` through the lowering and the WGSL,
   for the GPU-only kinds. Formulas, escape colourings and variations
   also run in the CPU interpreter (deep zoom's references, `vary`, the
   lens, the tests), so it learns the three statements, and the
   derivation passes refuse them where they cannot follow (as they
   refuse a solid's `Map`). Then every kind has them: built-in
   variations use early returns in 86 places and `break` / `continue` in
   9. **`break` and `continue` built 2026-11-03** (the user away, queued):
   the evaluators run all three statements (`ir::Flow`), the lowering
   takes `break` and `continue` in every kind's loops, the perturbation
   emitters write them; a formula using them runs in the interpreter as
   the same formula without them, bit for bit, and the deep-zoom paths
   decline it without panicking. **An early `return` built 2026-11-03**,
   by scope (the user's idea, over a flag on every statement after it or
   a refusal): a body that returns early -- a helper, in every kind; a
   varied variation; a variation's own; a formula's `step` -- is a scope
   its `return` leaves, the result set first; the WGSL a `loop` it
   breaks, with a flag only from inside the body's own loops. A varied
   variation's return ends the variation and the step goes on: bit for
   bit in the interpreter against the formula written without it, and
   on the GPU but 6 pixels of 12,288 on an escape's edge (the scope's
   loop moves a multiply-add's fusion). Deep zoom declines a formula with
   a scope. Open: a colouring's own `finish` and the escape colourings'
   sections returning early (P7 step 2 will want them).
2. **Escape colourings** (28), branch `rhai-escape-colourings` (begun
   2026-11-05; **all six batches built 2026-11-05**: every colouring
   twinned, to merge). A census of the 28 against the
   dialect (2026-11-05) corrected the estimate: 7 could be twins then,
   not about 20. The rest needed values the loop has and a Rhai
   colouring could not read.

   **What a Rhai colouring had then.** `start(z, p)`, `step(s, z, p)`
   and `finish(s, z, p)`, each with an optional `ctx`. `finish` returns
   a palette position or `#{ index, solid }`. `ctx` has `c`, `julia`,
   `angle`, `maxiter`, `center`, `magn`, the pixel's place, `random`,
   and `numiter` (in `finish` only). `INFO` takes `region`.
   `compile_coloring` sets only `NeedsOrbitAccum` and `ColorsInterior`.

   **A bug found.** On the GPU, `start`'s `z` is the literal 0, while
   the interpreter gives it the formula's starting z. No test catches
   it, because the test's formula starts at 0. Fixed in batch 2: each
   template hands `start` its starting z (the perturbed path the
   reference's first point plus the pixel's offset). It changes the Point
   Trap example on a formula that does not start at 0.

   **The order of work**, each batch ending with the twins it unlocks:
   - **Batch 1: the seven writable today**, with the machinery every
     batch uses. **Built 2026-11-05**: every twin matches its built-in on
     every pixel, at every setting that takes another path, on five views
     (`builtin_colouring_twins_render_as_their_builtins`). They are
     within a few percent of their built-ins' speed, but for Itinerary,
     25% slower through the language's zero-pair-exact `atan2`
     (ESCAPE.md). `round_even` came forward from batch 2's numerics: the
     dialect's `floor` and `round` give an int, which overflowed on
     Gaussian Integer's iterates at a large bailout, and a helper
     branching to even cost it 45%.
     - The twins: Escape Count, Orbit Trap, Sphere Average,
       Decomposition, Basic, Gaussian Integer, Itinerary.
     - A registry beside the formulas' (`escape::twins`,
       `COLORING_TWINS`), with sources in
       `assets/formulas/twins/colorings/`.
     - A CPU test that each twin compiles with its built-in's
       parameters.
     - A GPU picture compare against the built-in, on more than one
       formula.
     - A printed timing, as the formula twins have.
     - The Escape panel's **Edit as Rhai** for the colouring; the
       Formula Editor's `open_in_use` takes the entry's kind.
   - **Batch 2: what the loop knows.** **Built 2026-11-05**: all twelve
     match their built-ins on every pixel on the five views and Newton's
     basins, but Stripe Average, under 0.25% of pixels by an ulp (its
     range-reduced `sin` at up to 32 pi; allowed 1%). Speed within a few
     percent, but Curvature Average 14% slower (`atan2`, as Itinerary)
     and Stripe Average 9% (`sin`). Two rules found for matching to the
     bit, both the GPU's fused multiply-add: a blend as `prev + (mean -
     prev) * f` (WGSL's `mix`), and a term the built-in adds in one
     expression added in one (Root Basin's shading by time).
     - `ctx.escaped`, `ctx.converged`, and `ctx.period`, which brings
       `NeedsPeriod`.
     - `ctx.degree` and `ctx.bailout` in every function.
     - The escape fraction the debanding uses, as the template computes
       it (`esc_escape_fraction`, its never-escaping case included).
     - `c` and the previous `z` in `step`, and the formula's starting z
       in `start` (the bug above).
     - `log2`, so twins round as their built-ins. (`inverseSqrt` is not
       needed: Sphere Average's `1 / sqrt` matched to the pixel.)
     - The twins it unlocks: Smooth, the five deband averages (Orbit,
       Stripe, Magnitude, Position, Threads), Triangle Inequality,
       Curvature Average, Velocity, Exponential Smoothing, Root Basin,
       Period.
   - **Batch 3: `INFO` keys.** **Built 2026-11-05**: External Rays
     matches its built-in on every pixel; Position Map differs by an ulp
     on under 1.7% of pixels on Origami, its `sin` range-reduced as
     Stripe Average's (allowed 2%). The batch 1 and 2 twins gained their
     built-ins' `bailout` and `pick`, which the compile test now checks.
     A Rhai colouring picked from the panel's plug-ins, or first Used in
     the Formula Editor, applies them as a built-in's pick does.
     - `bounded`: clamp rather than wrap, the `Bounded` flag.
     - The recommended bailout, and the pick parameters a fresh pick
       takes (the averages' deband).
     - The twins it unlocks: Position Map, External Rays. With it, a
       twin also picks as its built-in does.
   - **Batch 4: the derivative.** **Built 2026-11-05**: `ctx.dz`,
     `ctx.has_derivative`, `ctx.render_px`, `ctx.supersample`. Distance
     Estimate matches its built-in on every pixel at every mapping and
     unit, on the five views and over the Mandelbrot's Rhai twin (its
     derivative derived); Normal Map too, but by an ulp on under 0.06%
     with its light past a half turn (the range-reduced `cos`/`sin`;
     allowed 0.2%). Speed within 6%.
     - `dz` in `finish`, which brings `NeedsDerivative`, with the
       template's has-a-derivative flag.
     - The supersample factor and the render pixel's size.
     - The twins it unlocks: Distance Estimate, Normal Map.
   - **Batch 5: a colour as the result.** **Built 2026-11-05**: all
     four match their built-ins on every pixel at every setting tried, on
     the five views; Direct Orbit Traps' twin re-iterates on a palette
     edit. Speed within 2% (Rainbow Fringe, Infinite Waves) or faster
     (Visions of Chaos Traps 7%, Direct Orbit Traps 20%). As built, it
     differs from the plan below in two places:
     - The colour is a function of its own, `fn color(s, z, p[, ctx])`
       ending with a `vec3`, not a value `finish` returns beside the
       index: the colour reads the value after the transfer
       (`ctx.value`), which exists only once `finish` has given the
       index. It is the program's transform section, through the same
       result handling as effects and simulation colourings
       (`put_result`).
     - The palette is four reads -- `palette`, `palette_srgb`,
       `palette_table`, `palette_curve` -- host calls, each read once
       into a slot for its parts. Three reads a part cost Visions of
       Chaos Traps' twin 40%. A read in `start` or `step` brings
       `PaletteInLoop`.
     The measure was fixed on the way: it made a renderer for each run,
     so it timed the pipelines' building too, which read as up to 60% on
     the short renders; with one renderer the earlier batches' figures
     hold (Itinerary 28%, Curvature Average 12%, Stripe Average 8-12%).
     - `finish` returning an `rgb` beside the index, through the result
       handling effects and simulation colourings share (`put_result`).
     - The palette in `finish`, and in `step` with `PaletteInLoop`, so a
       palette edit re-iterates.
     - The value after the transfer.
     - The view's rotation as cosine and sine.
     - The twins it unlocks: Rainbow Fringe, Infinite Waves, Visions of
       Chaos Traps, Direct Orbit Traps.
   - **Batch 6: Image Trap** (corrected 2026-11-05: the census had it
     reading a user image, but it reads the config's simulation texture,
     `EscapeConfig::texture`). **Built 2026-11-05**, all 28 colourings now
     twinned: the twin matches the built-in on every pixel, on every
     merge mode, both orders, both proportions and a moved trap, direct,
     perturbed and on a Julia set (`the_image_trap_twin_renders_as_its_builtin`),
     and recolours on a palette edit (`a_texture_reading_twin_recolours`).
     18% slower, its trap's rotation and skew through the reduced trig
     each iteration as the built-in's are. The texture read is the
     language's, for every kind (below). The built-in now reads the
     texture as the language does -- bilinear by hand in f32 from
     `textureLoad`, not through the sampler, whose filtering weights each
     vendor rounds its own way -- so the two agree and both agree across
     platforms (`the_image_trap_reads_the_texture_as_the_language_does`;
     the visual config moved by one 8-bit level on 535 of 307,200
     pixels, the Ultra Fractal trace's agreement unchanged). What it
     needed:
     - `texture(uv)`, an `rgba`, and `texture_size()`, in `start` and
       `step` alone (the colouring's iterate passes bind slot 14; the
       recolour pass does not), each read once per call site as the
       palette's are; reading it brings `TextureInLoop`.
     - `color` ending with an `rgba`, its `a` the pixel's coverage
       (`DirectAlpha`), as a simulation colouring's does. The template's
       `coloring_alpha(sum, state)` gains `v`, so it can be written from
       the same body; Image Trap's own changes to match.
     - An `INFO` key for `SkipsEscapingIterate` (UF's loop section: the
       step sees only the iterates that did not bail out).
     - The compare with a texture on the renderer, as
       `the_image_trap_draws_what_ultra_fractals_source_says` sets one:
       every merge mode, both orders, both proportions, the trap's
       transform.

   **Textures in other kinds** (the user's ask, 2026-11-05: one way to
   read a texture, usable by variations too). The language side is
   kind-free: `texture(uv)`, `texel(x, y)` and `texture_size()` lower to
   host calls (`Host::texture`), each read once per call site into a
   slot, and every kind's WGSL reads through the one portable
   `lang::wgsl::texture_wgsl(decl, var)` -- the bilinear JWildfire's
   texture variations do on the CPU, so a port can match them, and
   `texel` for one that interpolates its own way. A kind opts in with
   four things:
   1. the checker allows its kind and sections (`check::host_args`);
   2. its generator emits `texture_wgsl` with its own binding and the
      slots' declarations (as `wgsl::coloring` does for slot 14);
   3. a feature, from `wgsl::reads_texture`, that makes its passes bind
      the texture and makes the texture part of what it renders;
   4. a texture to read.
   For **flame variations** (`colormap_wf`, `displacemap_wf`,
   `post_bumpmap_wf` ... in variation-port-blockers.md, blocker 2) the
   first three are small: the Variation context in the generator, a
   `Feature` (append-only; the API repository is told), and a binding
   the compute layout holds only for a flame that uses it
   (`used_bindings`). The fourth is where a flame's texture comes from:
   the user plans several (2026-11-05), so the reads already name theirs
   -- `texture(uv, k)`, `texel(x, y, k)`, `texture_size(k)`, k 0 by
   default and 0 alone yet, a literal. When more come, the index may
   become any expression (a variation's parameter picking its texture per
   instance, as JWildfire's each have one), the config holds a list
   (escape's one texture its first), and the shader binds the list --
   separate bindings under a switch, or a `texture_2d_array` if the
   textures share a size (WebGPU's default limit is 16 sampled textures a
   stage). Every path that renders (export, video, thumbnails, the
   gallery modules, pyfflame) bakes the list first, and the UI picks into
   it.

   **Numerics to write by hand in a twin.** WGSL's `round` rounds half
   to even, the dialect's half away from zero. A real `sin`/`cos` is
   range-reduced past pi. A number `min`/`max` is an `if`, so NaN
   differs. Where a twin cannot match bit for bit, its compare names
   the share it allows, as the formula twins' table does.

   **Stays beside the built-ins.** Whether the twins then replace them
   is D7's, decided later, as for the formulas.

3. **Flame variations' hooks.** 339 of the 647 use nothing a Rhai
   variation cannot say today; the other 308 need at least one of:
   - **derived parameters** (134 built-ins, `wgsl_init`): a variation's
     `fn prepare(p)`, compiled into the once-per-transform setup in the
     built-ins' own precision, so a twin stays exact (the escape
     dialect's `prepare` and P4's CPU-side `max_dt` are the precedents);
   - **the transform's coefficients** (123, `NeedsTransform`): a `ctx`
     read beside `ctx.weight`;
   - `INFO` keys: the phase (pre or post, 45), `replace` (assign rather
     than add, 48), `never_z` (16);
   - **outputs beyond the point**: the result map (today `#{ z, color }`)
     with `rgb` (`WritesRgb`, 20, the vector type built), `hide`
     (`CanHide`, 39) and `w` (`NeedsW`, 12, with its read) -- the same
     design P4 step 9c needs for McCabe's memory;
   - `pcg` and the host ints for every kind (23 hash integers; P4 step
     5 opened them to the simulations alone);
   - the Möbius helpers (9 built-ins share `NeedsMobiusLib`) as a Rhai
     library, imported (P0's `import`).
   With the hooks but derived parameters, 141 stay blocked; with
   derived parameters, 7: the renderer's own hooks (`PlotEmits` 3,
   `AnalyticBlur` 2, `VolumeSideEmit` 2), each a design of its own,
   open.

   **The census again, 2026-11-05** (branch `rhai-variation-hooks`; the
   corpus `export_variations_json`, read against the dialect as built),
   correcting the one above:
   - **337 need no new hook**: 230 advanced 2D, 60 full 3D, 29 plugin,
     13 depth 3D, 5 basic.
   - `NeedsTransform` (123) is mostly the variation's own weight: 117
     read `transforms[xform_id].variations[variation_id]`, JWildfire's
     weight inside the formula, which `ctx.weight` already gives. Six
     read the affine's coefficients (`fan`, `popcorn`, `waves`,
     `dc_carpet`, `dc_triangle`, `dc_ztransl`): the coefficients hook is
     theirs alone.
   - The rest, by how many need each: derived parameters (`prepare`)
     134; replace 47; the phase 45; hide 39; rgb 20; `never_z` 16; `w`
     12; the Möbius library 9; the renderer's hooks 7 (out). 61 have a
     3D body that writes z without `AlwaysZ`, where the dialect's 3D
     body declares it: they need the z rule as an `INFO` key too. 74 use
     bit operations, which the dialect has on ints; the 23 hash
     integers (`pcg` and the host ints) are to check.
   - **The WGSL is regular**: `let`, `var`, `if`, `for` (68), `switch`
     (40), `select` (242), helpers (92), vectors and swizzles,
     `get_param`, `rng_nextf`; 11 arrays, 2 matrices; three helpers take
     a pointer beyond the rng (`dc_hexes_wf`, `glsl_circuits`,
     `mondrianomies`).
   - **Exactness:** a variation's Rhai trig is range-reduced (only an
     effect's is the hardware's), where the built-ins call the
     hardware's: a twin differs by ulps wherever trig is, and that is
     the portable side (the user's rule, 2026-11-05: whatever makes
     WebGPU, Metal and Vulkan agree). So a twin is held per point within
     an ulp tolerance -- exact where no trig is -- and, in a flame, by
     density, as the fixtures are.

   **The order of work** (the user, 2026-11-05: "let's get started on
   variation hook & twins"):
   - **V1. The machinery, on a handful written by hand.** A registry of
     twins beside the escape ones (`variations::twins`, sources in
     `assets/variations/twins/`), each with its built-in's parameters
     exactly (a compile test, as the colourings'); **a pair compare on
     the probe**: the built-in and its twin as two transforms of one
     probe flame, every probe input through both, 2D and 3D, at the
     defaults and at swept parameters, held to a tolerance; a density
     compare in a flame for a sample of each batch (`render_blocks`); a
     timing; and **Edit as Rhai** for a transform's variation, the twin
     in the built-in's place with its weight and parameters, the source
     carried.
     **V1 built 2026-11-05**: the registry, `probe::pairs` (the probe
     paired: the RNG seeded from the point, the colour written out),
     the compile, pair and density tests, Edit as Rhai in a variation's
     settings menu, and seven twins by hand. Not yet: a timing.
   - **V2. A WGSL-to-Rhai translator** for the regular subset (naga's
     parse of a def's WGSL to the dialect: a vector `p` a complex in 2D
     and a `vec3` in 3D, helpers kept, `PARAMS` from the def), so the
     337 come in batches by family, each translated twin held by V1's
     compare and read by eye; what it cannot say is listed, and written
     by hand.
     **Built 2026-11-05**, as a text-level translator in Python
     (`scripts/translate_variation_twins.py`: the author's names,
     structure and comments kept, where naga's IR loses them): 291
     twins, 298 with the seven. It found what the dialect and the compare
     needed: `free` and `exponential: false` on a parameter; the float
     `floor` everywhere; WGSL's `log` is Rhai's `ln`; a parameter WGSL
     reads as an f32 is `to_float`; the compare's f64 judge, and density
     for the twins that draw. Refused, 39 of the 337: a colour written
     (11), a 3D body writing z without `AlwaysZ` (8), bit operations
     (5) and hex literals (3), the coefficients (3), arrays (3), a
     helper of its own kind (crackle's, synth's, ho's), state, a
     non-range `for`. Read by eye: a sample, not all 291.
     **Nothing lost, built 2026-11-05**: each twin opens with its def's
     doc block verbatim and gives its category and phase in `INFO`; an
     unedited twin keeps the deep-zoom planner's data and exports in a
     `.flame` as its built-in (VARIATIONS.md, *Twins*). Still the
     def's: its aliases.
   - **V3. The hooks, each followed by the twins it unlocks**:
     derived parameters (`fn prepare(p)` lowered to the def's init
     setup, 134); the z rule, the phase and replace as `INFO` keys (61,
     45, 47, 16); outputs beyond the point, `hide`, `rgb`, `w` (39, 20,
     12); the coefficients (6); the hash ints; the Möbius library as an
     imported Rhai library (9). The 7 on the renderer's own hooks stay
     out.
     **Built 2026-11-05**, in order: **the phase** (`INFO.phase` pre or
     post: the translator's refusal lifted, 27 twins); **the z rule**
     (`INFO.z`: `always`, `preserve_z`, `never`, as `AlwaysZ` and
     `NeverZ`; 45 twins `preserve_z`); **derived parameters** (`fn
     prepare(p)`, compiled to the def's `init_<name>` for the init pass,
     87 twins; a transformation's parameters now take its transform's
     block, so `quaternion`'s 92 fit); **a colour read** (`ctx.color`,
     the colour coordinate as it arrives; the interpreter starts from
     the probe's 0.5; 23 twins, judged by their colour per point and per
     channel in a flame); **replace** (`INFO.replace`, the feature the
     shader combines by; 16 twins, the cut family waiting for `hide`).
     **hide** (`#{ z, hide }` and `ctx.hide`, `CanHide`), with the
     translator's `while` for a `for` not over a range and `smoothstep`
     written out (34 twins, 23 of them the cut family). 517 twins, 7842
     pairs, 0 apart. On the way: a held temporary inside a vector part's
     assignment came out a bare name (now refused if it ever recurs); a
     vector and a number are added part by part; `length(u - d)` came out
     `u - d.abs()`, the receiver unparenthesised -- found by density in
     two cut twins that draw, and compiling because a complex compares
     with a number by its real part (UF's rule, which the Rhai dialect
     keeps); the judge perturbs by a non-round amount (1e20 ± 1e14 is
     still a multiple of 5). **State**, the language's already (`fn
     init(p)`, `this.name`), the translator's now: each slot a field named
     as the body reads it, the def's state init as `init` (2 twins:
     curliecue2, macmillan; the other stateful built-ins wait on `u32`,
     the Möbius library, `w`, or an init that draws). 519 twins, 7870
     pairs, 0 apart. **A complex has no order** in Rhai (the user's
     call). **`u32`** (the user's call: an unsigned type, not bit
     operations on the int alone): `Ty::UInt` through the checker, the
     interpreter, deep zoom's reference and the WGSL -- WGSL's
     semantics, an int joining to it by its bits and back only by
     `to_int`; a shift its left side's type; `INFO.slow` for a
     variation's parameters. 23 twins (the hashes and counts), 542 in
     all, 8504 pairs, 0 apart. **A direct colour** (`#{ z, rgb }`,
     `ctx.rgb`; `WritesRgb`), judged by its flame's light, channel by
     channel; the translator writes a vector's part-by-part functions out
     part by part, and names no temporary as the WGSL names a local (a
     shadowing that compiles). 18 twins, 12 of them the `glsl_*` family:
     560 in all, 8980 pairs, 0 apart. **The transform's coefficients**
     (`ctx.a` .. `ctx.f`; the judge's truth given the probe transform's):
     fan, popcorn, rings, waves and the three dc_* that read them, 567 in
     all, 9098 pairs, 0 apart. **The 4th coordinate** (`ctx.w`, `#{ z, w
     }`, a 3D body's; `NeedsW`), the 4D twins' flames rendered in 3D:
     jubiq4d, minkowski, minkowski_camera, mobiq4d, 571 in all, 9366
     pairs, 0 apart. **A vec4** as the language's four-part kind, in the
     translator: 7 twins (the mandelbox and kaleidocomplex glsl,
     cymatics3d, the hyperbolic and quaternion cameras, the quaternion
     Julia set), 578 in all, 9672 pairs, 0 apart; sphere_packing's
     `vec2<u32>` is refused, the language having no integer vectors.
     **Arrays** of numbers and complex numbers in the translator (the
     Jacobi functions, waves2b, cut_kaleido); **a drawing state init**
     (`rand()` in `init`, the thread's stream, as the built-ins'
     `wgsl_state_init`); the shared complex helpers translated in;
     `loop` and `exp2`: 11 twins, 589 in all, 9990 pairs, 0 apart.
     **The raw draw**, `rand_u32()` (the shader's `rng_next`, a `u32`
     from the same stream as `rand()`): the juliaq family, cpow2,
     log_db, 594 in all, 10052 pairs, 0 apart. **Helpers**: a helper
     reading the parameters takes `p` (synth), a helper after the
     variation's own function (ho), the init's helpers, and the shared
     helpers of noise.wgsl and its neighbours as complex.wgsl's
     (dc_perlin): 597 in all, 10266 pairs, 0 apart.
     On the way: the translator writes a literal first in `min` and
     `max` (Rhai's keeps a NaN first operand, WGSL's does not: `arcsinh`
     at 1e30); the judge moves the parameters as it moves the point, and
     caps how often the built-in may be "off its formula" before the
     twin is doubted -- a hole a twin of another formula would have
     passed through. Left, 50, by count (`translate_variation_twins.py
     --report`): a struct (16: the Möbius library's, with its table, 9;
     the fract family's, 6; complex.wgsl's `CMat2`, klein_group), the
     renderer's hooks (6: the analytic blurs, the volume emitters, the
     subflames), arrays as arguments or of vectors (7: crackle's three,
     dc_hexes_wf, iconattractor_js, the honeycombs), an integer vector
     (4: sphere_packing, and polyhedra.wgsl's and polychora.wgsl's face
     ranges for polyhedron, polychoron, menger), a parameter by a
     computed slot (3, the lsystems), plot emits (3), a palette read in a
     variation (3, the quaternion family's `.rgb`), a pointer argument
     (2: glsl_circuits, mondrianomies's hash state), a parameter named
     `super`, which Rhai reserves (`w`, `z`), and one each: a matrix
     (glsl_apollonian), a parameter named `3side` (hexnix3D), state by a
     computed slot (farblur), a helper hiding the point (rhodonea).
4. **The twins, in batches**: the variations by family -- a skeleton
   generator from the defs, as P4's `write_twin_skeletons`, and each
   held by a density compare and timed -- the escape colourings, the
   solids (Mandelbox, Mandelbulb, the KIFS, the quaternion Julia; the
   two Flame Attractor solids are the flame itself, nothing to twin),
   the fields (a never-escaping formula and an averaging colouring,
   Weierstrass's analytic gradient for the hillshade the part to check;
   decided in the KIFS plan to stay in mode B, so twins beside them).
   **A parameter layout, built 2026-11-03** (the user: "a grid as a
   generic param type ... and param display dependent on other param
   selection"): `show_if` on a parameter, a grid, a grid's column; a
   `type: "grid"` `PARAMS` entry expanding into its cells; a grid's fill
   on the edit that switches it on and its row warnings, CPU functions;
   `p.grid[i].column` on the GPU and the CPU (`param_layout.rs`,
   `lang::rhai::param_layout`). McCabe's built-in table moved onto it
   (`ParamTable` gone), its twin declares the same grids -- table and
   warps -- and reads them by row instead of by slot: still bit for bit
   on every path, at speed (1.030; 1.032 Table, 1.024 Table with the
   variation radius, 1.024 warped). Open: only the simulation panel
   draws a layout; the Escape, Effects and variation panels list a
   grid's cells one by one and ignore `show_if`; a computed row reads a
   grid in a simulation model alone (elsewhere a constant row).
5. **Edit as Rhai everywhere**: the simulation panel (the model and the
   colouring, per layer and colour layer), the Escape panel's colouring,
   the solids and fields, a transform's variations -- each putting the
   twin in place and opening it. **The simulation panel's built
   2026-11-03**: a **Rhai** button beside a built-in model (each layer)
   and colouring (each colour layer), `edit_sim_as_rhai` -- the twin in
   the slot under a UUID of its own, the slot's settings kept, one undo
   step, the editor holding it in use for Live.

Not convertible: JWildfire's 8 variations that run user-typed Java
(`custom_wf` and the rest; refused for security,
[variation-port-blockers.md](variation-port-blockers.md)). A Rhai
variation is the safe stand-in for what they were used for, but a
`.flame` that carries Java code cannot be translated.

## What is left, as branches (proposed 2026-11-03)

`rhai-plugins` merged on 2026-11-03 with P0-P4 done and P7 done for
the simulations (step 1, the simulation part of step 5, and the
parameter layout). What remains, one branch each, every one starting
from `main`. The order is the user's to pick: D5 put P5 tracks and then
P6 the API next, and D7 came after it. The dependencies are listed
under each branch.

1. **`params-one-system`** -- **merged 2026-11-03** (PR #148). One
   parameter system for every kind
   ([one-parameter-system.md](../archive/projects/one-parameter-system.md); the user,
   2026-11-03).
   - One definition, one renderer and one compiler converter for the
     variations, effects, escape formulas and colourings, simulations
     and CPU scripts, which today have five definition types and four
     widget sets.
   - Replaces `rhai-layouts-everywhere`: grids and `show_if` in every
     panel, and a cell at a computed row in every kind, come with it.
   - Depends on nothing. Comes first, so the twin branches after it add
     parameters to one system, not to five.
   - **Done on the branch (2026-11-03)**, steps 1-11, merged (PR #148);
     the record is [one-parameter-system.md](../archive/projects/one-parameter-system.md).
     Left from it:
     - **A grid's fill and warning outside a simulation.** They are Rhai
       functions the CPU runs, and only the simulation registry holds a
       CPU script for a source (`sim::custom::with_script`, `params_map`,
       the warning's `ctx` with the grid's size and pyramid reach). Every
       other kind refuses them (`LayoutSpec::without_cpu`) rather than
       dropping them; moving that CPU machinery out of `sim::custom`
       would give them to every kind.
     - **A CPU script's layout.** A script declares its parameters by
       calls (`param`, `param_choice`, ...), not a `PARAMS` map, so it
       has no conditions or grids to declare; a `param_show_if`-like
       call would give it them.
2. **`rhai-escape-colourings`** -- P7 step 2: twins of the 28 escape
   colourings, and Edit as Rhai in the Escape panel's colouring.
   - About 20 accumulate over the orbit, which `start`, `step` and
     `finish` already do.
   - The rest need a colour as the result (`finish` ending with a
     `vec3` or `rgba`, through `put_result`), the palette inside the
     loop (P4's `palette(t)`), a read of the formula's derivative
     outside the perturbation sections, `INFO` flags for `Bounded`,
     `ColorsInterior`, `DirectAlpha` and `SkipsEscapingIterate`, and an
     early `return` in `finish` and the sections (a scope, as P7 step 1
     built).
   - Image Trap stays out with user image files (decided 2026-10-02).
   - Each twin held by a picture compare and timed, as the simulation
     twins are.
3. **`rhai-variation-hooks`** -- P7 step 3, the language a variation
   twin needs.
   - `fn prepare(p)`, derived parameters compiled into the
     once-per-transform setup (134 built-ins).
   - The transform's coefficients as a `ctx` read (123).
   - `INFO` keys: the phase, pre or post (45); `replace` (48);
     `never_z` (16).
   - Outputs beyond the point: `rgb` (20), `hide` (39), `w` (12).
   - `pcg` and the host ints in every kind (23).
   - The Möbius helpers as an imported Rhai library (9).
   - With these, 7 built-ins stay blocked, each on a renderer hook of
     its own (`PlotEmits`, `AnalyticBlur`, `VolumeSideEmit`).
   - The KIFS plan's variation leftovers belong here too: a Rhai
     variation's state inside a formula, state arrays, `rand()` in
     `init`, `WritesRgb`.
4. **`rhai-variation-twins`** -- P7 step 4 for the variations, in batches
   by family. Depends on 3.
   - A skeleton generator from the defs, as `write_twin_skeletons` is
     for the models.
   - Each batch held by a density compare and timed.
   - The largest piece by count: 647, of which 339 need no new hook.
   - Probably several branches, one family each; whether the twins then
     replace the built-ins is D7's per-kind decision (D2 keeps the WGSL
     until they are done).
5. **`rhai-solids-fields`** -- P7 steps 4 and 5 for mode D and mode B.
   - Twins of Mandelbox, Mandelbulb, the KIFS and the quaternion Julia.
     The two Flame Attractor solids are the flame itself: nothing to
     twin.
   - Twins of the three fields beside them (decided in the KIFS plan):
     a never-escaping formula and an averaging colouring, Weierstrass's
     analytic gradient for the hillshade the part to check.
   - Edit as Rhai for solids and fields.
   - Depends on 2 for the fields' colourings.
6. **`rhai-tracks`** -- P5, animation tracks as scripts.
   - A track is a pure function of time (D4): CPU Rhai compiled once
     per track, run each frame after the keyframes.
   - Its targets declared, so the timeline shows them and a clash with
     a keyframe track is visible.
   - The same values reach the live applier and the export's.
   - Depends on nothing.
7. **`rhai-api`** -- P6, one scripts resource for every kind. Work in
   the API repository first (RELEASE.md §3).
   - Publish, update, unpublish, search and versions for every kind by a
     `kind` field.
   - Adopting saves into the library, flagged as a plug-in if wanted.
   - Settles the effect corpus question in
     [v0.6.0-release.md](v0.6.0-release.md): the generated WGSL or the
     hand-written reference, until the API stores the Rhai source.
   - Depends on nothing here; best after 2 and 3, so the kinds it
     publishes have settled.

Not given a branch, for the user to place:
- Rhai's overloads by arity, refused since P7 step 1: support them, or
  keep the refusal.
- The KIFS plan's other leftovers (below): C3, a built-in variation as
  a step in a formula; a formula's Live re-apply following an edit to a
  variation it calls; a hybrid member carrying its variation.
- The Brusselator2 (2-3%) and LIC (3-4%) twins' unexplained slowdowns,
  under the gate.

## Decisions (the user, 2026-11-01)

- **D1. The built-in effects: replaced by their Rhai twins**, "so long
  as we don't lose anything" -- the checks are P2 step 7.
- **D2. The WGSL download catalogs stay** for the moment: converting
  them all is a much bigger project. The 647 built-in variations stay
  WGSL.
- **D3. The old plug-in system is deprecated**: "it's not really being
  used, and I'm not sure it even works correctly still." Retired at the
  start of P1; Rhai plug-ins replace it in P3.
- **D4. Script tracks are a function of time** (P5).
- **D5. Simulations before tracks**: the order is P0, P1, P2, P3, P4
  simulations, P5 tracks, P6 the API.
- **D6. Namespaces**: "the program's default scripts, user-created
  plugins and the API won't interfere with each other. The UI can show
  where things came from." Then, the same day: UUIDs for everything but
  the built-ins, with hashes for versions, names free, and every field a
  person reads following the language -- design rules 7 and 8.
- **P0**: the user wants a way to import Rhai scripts -- through the
  library, as P0 now says.
- **D7. Everything editable as Rhai** (2026-11-02): "The eventual goal
  is to port everything over to Rhai so the user can edit any formula
  or coloring." Every built-in gets a twin and an Edit as Rhai (P7);
  D2's "stay WGSL" holds until a kind's twins are done, and whether
  they then replace the built-ins is decided kind by kind, as D1 did
  for the effects.

## Left from the KIFS and variations plan

From [escape-kifs-and-variations.md](../archive/projects/escape-kifs-and-variations.md),
archived 2026-10-21 (built: A1-A5, B, C1, C1e, C2, C4):
- **C3**: a built-in WGSL variation as a step in a formula (`vary`
  takes the name as a string so it could), as the user wants it; and the
  maps [uf-transforms-review.md](../archive/projects/uf-transforms-review.md) (archived)
  found missing, written as Rhai variations -- the user's pick.
- A formula's Live re-apply does not follow an edit to a variation it
  calls (Use the formula again); a hybrid's member that calls a variation
  does not carry it.
- In a Rhai variation: its state inside a formula (refused), state
  arrays, `rand()` in `init`, writing RGB (`WritesRgb`).
- Transformations (C4): none in mode B; in mode D the Orbit stage only
  (a solid's colouring, 3d-look-and-tools P5 -- Iteration for every
  solid is in escape-sim-followups.md); clipping (UF's `#solid` from a
  transformation); a colouring's own start reading the Orbit map.
- A function parameter named as a built-in's takes that built-in's value
  when a config switches between them.
