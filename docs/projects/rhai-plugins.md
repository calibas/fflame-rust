# Everything scriptable in Rhai: the plan

Recorded 2026-10-18 as the direction, from the user's statement of where
the program is going. Reviewed against the code and turned into a plan
on 2026-11-01; the user's decisions the same day are at the end and are
folded into the steps. **Nothing under *The plan* is built yet.**

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
   library both ways.

### Inside the GPU dialect: one compiler, two of everything below it

One front end (`lang/rhai.rs`), checker, IR and WGSL back end.
Below that:
- **Three registries**: `escape::custom::Registry` (content-hash names,
  `custom-<fnv>`), `ifs::custom_solids`, and the variation registry with
  `Provenance::Custom` plus `variations::custom`'s own source map.
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
- Effect downloads need a token and are gated by online mode only at the
  caller; variation downloads need neither.
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
  downloaded, carried by a config) that every list shows. The WGSL
  variation and effect catalogs are server copies of the program's own
  built-ins, so what they fetch is a built-in this build lacks, under
  its string id.
- **Labels follow the language (rule 8)** in the shared parameter and
  `INFO` lowering: a string or a locale map wherever a person reads it.
- One registry for compiled Rhai entries, keyed by kind and id, with
  one `ensure` from a config's carried sources, one failure memory, one
  error type (`Vec<Diag>`), and a provenance on every entry.
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
MB gzipped (5.2 to 6.3 MB raw), measured before `wasm-opt`. The CPU
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
   `EffectSource::Owned` WGSL and its carried source, `Provenance::Custom`,
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
4. **One presentation**: the Formula Editor's list and the Scripts
   panel show the same marks for an entry's origin and plug-in flag,
   from the scan; the shipped effects are in the editor's list beside
   the formula examples.

### P4. Simulations in Rhai

1. **Colourings first**: a colour function over `SimSample`'s fields
   returning rgb and coverage, with `palette(t)`; one pass and no reads
   beyond the sample.
2. **Models, the single-layer class**: a step function with neighbour
   reads at offsets (the boundary stays the framework's), `vec4` state,
   `dt`, the step index, random numbers, a seed function; up to four
   passes. Gray-Scott is the first twin. Kernel tables, couplings,
   memory, the pyramid and agents stay built-in, refused with a reason.
3. **Carried**: custom model and colouring sources on `SimConfig`,
   ensured like the escape slots.

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
