# Architecture review (2026-11-04)

> **Archived 2026-11-04** -- finished: items 1-6 of section 9's order built the same day (branch `architecture-pass`, PR #150), with the user's choices -- one gallery renderer, the engine features removed, each surface's own defaults kept. Item 7 is the Rhai plans, in [rhai-plugins.md](rhai-plugins.md). What is left: the copied code in [pyfflame.md](pyfflame.md) (section 4); the flame dispatch governor's move in [doc-review-findings.md](doc-review-findings.md) (*Still open*); web video and modules on demand in [wasm-modules.md](../../experimental/wasm-modules.md); the assets' cache and origin in [online-assets.md](../../experimental/online-assets.md); simulations online in [v0.6.0-release.md](../../projects/v0.6.0-release.md). Read [ARCHITECTURE.md](../../ARCHITECTURE.md) for how it fits together now; this is the record of what was found and decided.

Asked by the user after pyfflame merged: does everything fit together now
that an Apophysis clone has grown three engines, eight builds and a
scripting language? Are we building parallel systems we do not need? Does
the division into WASM modules make sense? Can the CLI, the in-app
exports and Python reach the animation and scripting systems? Is the
state management the editor's alone, and does it reach the new
features? The rendering pipeline, the governor -- and do the plans
(everything in Rhai, the online API, a resource panel for browsing
effects and other kinds) line up with the code?

Method: the docs first, then the code where a doc made a claim, plus two
agents checking `docs/main/` against the code. An architecture review,
not an audit of every file. Nothing here is decided; section 9 is a
proposed order for the user to pick from.

**Status** (branch `architecture-pass`, 2026-11-04): the docs pass (1)
is done. One applier (2.1) is done: `config::apply` is the setter, the
video export's arms are gone, and scripts' `config.set` and Python's
`Config.set` go through it by name (`config::by_name`). The engine
facade (2.2) has begun: the script calls have one home,
`script::headless`, for `wasm/script` and `wasm/python`, and the frame
source is split out of the video export (`animation::frames`), Python's
frames taken from it. One `ensure` (2.4): `FractalConfig::ensure_sources()`
at the whole-config entry points. Beside the
order, the mode switch was reworked at the user's request: each engine
owns its settings, the switch is a whole-config undo step, and an empty
flame is seeded where one is drawn (`config::mode`).

The user's decisions of 2026-11-04, built the same day:
- **Gallery (3): option (a)**, "0.4 MB is a small price to avoid
  separate modules". `wasm/flame`, `wasm/escape` and `wasm/sim` are
  retired and `wasm/render` serves every config.
- **Engine features**: removed. With no build dropping an engine,
  `engine-flame`, `engine-escape`, `engine-sim` and `lang` (which the
  escape engine needs) are gone, with their cfgs and the fallbacks
  behind them (`RenderError::EngineMissing`, the `linear`-only catalog);
  `web-app` and `terrain` remain, and the gallery check is their one
  build without either.
- **Headless defaults (2.2)**: the gallery's iterations per thread are
  1024, the CLI's and Python's; the other defaults stay per surface
  (sticky on or off, unknown-name refusal, the gallery's renderer
  reuse).
- **Origins (2.5)**: one model, `provenance::Origin` -- Builtin, Local,
  Api -- for registry entries, library files, scripts and textures.
  `ScriptOrigin` had four values because it answered where a script's
  file is kept, not who wrote it; it is `ScriptHome` now.
- **The tail (2.3)**: one sequence, `renderer::tail::Tail`, for the
  viewport and every headless render (`render::finish`);
  `HighResExporter` keeps its own tone map and calls the tail's density
  and colour stages around it.

## Summary

**The core holds.** One `FractalConfig` carries all three engines' state,
so one file format, one undo history, one animation addressing and one
online blob serve them all. Three generators share one tail. Every
non-interactive render goes through one API (`render` / `render_with`).
Every kind of definition follows one registry pattern. And the long-term
direction, Rhai sources carried in the config, is already how four of the
six kinds work. Nothing here calls for a rewrite.

**What does not fit is the same job done several times in parallel, each
copy drifting.** Five places (section 2):
1. **Config writes**: one setter, four sets of rules, three ways to
   address a field.
2. **The headless engine facade**, written five times. The code copied
   for pyfflame is the latest instance.
3. **The render tail**, sequenced in five places.
4. **Carried sources**, registered by four `ensure` functions from about
   30 call sites.
5. **Origins**: three models of where a thing came from.

Each is the kind of duplication that has already shipped a bug, and the
evidence is cited with each.

**The single-engine gallery modules no longer match the dependency
graph.** The engines now use each other, and the escape and simulation
modules silently draw configs wrong (section 3).

**The docs**: ARCHITECTURE.md's structure is right, but its registry and
scripting sections predate Rhai becoming the definitions, and about a
dozen specifics are stale. The two agents found 19 contradictions in the
reference docs (section 1).

---

## 1. Do the docs match the code?

### ARCHITECTURE.md

The map, the engines and tail diagram, the render API, the export and
the frame sections are right in substance. Stale:

| Where | Says | Code |
|---|---|---|
| §2, after the verification table | `src/undo.rs` is in the tree, uncompiled | gone |
| §2 Application and UI | `src/ui/` has 51 files and 30 panels (§3 says 32) | 56 files, 32 panels |
| §2 Outside `src/`, §7 | 374 visual configs | 383 |
| §2 module map | (no row) | `src/lang/` (the GPU dialect's compiler, feature `lang`) and `src/scripted.rs` (UUID and version keys) are not listed |
| §2 `provenance.rs` row | `Builtin`, `Api`, `Local` | `Builtin`, `Api { version }`, `Custom` |
| §3 Registries | static WGSL defs; Rhai variations from configs and the Formula Editor | the built-in effects are their Rhai sources; simulation and escape built-ins have Rhai twins; Rhai plug-ins load at start (`library::load_plugins`); four `ensure`s |
| §3 Registries | `load_installed_resources` is "called by every entry point that renders" | the app, CLI `export` and `export-animation` only: not `generate`, the gallery modules or Python (no store) |
| §3 The unified render API, Users | `app/export.rs`, thumbnails, video, `wasm/render` | also the web app's custom-size export (now `render()`), `wasm/python`, texture sync |
| §3 Scripting | one paragraph on CPU scripts | the two Rhai worlds -- the GPU dialect that is now how kinds are defined, and the CPU scripting API -- are the program's central cross-cutting system and are not described |
| §5, §7 | `release.py check` builds each single engine and runs the gallery tests | `check` compiles the gallery crates (the feature matrix) and the WASI module; the tests run under `release.py gallery`, with pyfflame's |
| §6 Add an effect | WGSL in `shaders/effects/`, `register_builtin_effects` | a built-in is its Rhai source (`assets/effects/*.rhai`, `BUILTIN_TWINS`); the WGSL is the reference |
| §6 Add a simulation model | `models.rs` | still true, with a Rhai twin beside it (`assets/sim/`) held to it |

### The reference docs (two agents, 2026-11-04)

| Doc | Contradictions | The ones that matter |
|---|---|---|
| CONFIG.md | 5 | the render mode's writer is `render_mode::choose`, not `switch_render_mode` / `apply_with_look`; the PNG's `Config` chunk is the versioned form; `SystemExportWidth/Height` do not exist; counts (`ConfigPath` 413, not 372; presets 84, not twenty) |
| ONLINE.md | 6 | the `Local` provenance; `MissingReason::ProbablyAPlugin` is `NotInCatalog`; the contract's categories and param types are generated now; search URLs are encoded now; the gallery table's features (`lang` in flame and sim) and `wasm/python` |
| ANIMATION.md | 2 | the escape centre IS animatable (since 2026-10-31); the picker offers zoom log10, not log2 |
| RENDERER.md | 2 | the web custom-size export calls `render()`, not its own loop; the constructor table names `FlameRenderer::new` (no caller) and not `for_job` (what `render` and video use) |
| VARIATIONS.md | 1 | names `local_plugin_dependencies`, removed with the WGSL plug-ins |
| EFFECTS.md | 1 | the panel draws parameters with `ui::params`, not `variation_params` (the doc says both) |
| ESCAPE.md | 1 | mode D has 6 distance functions, not 3 |
| WASM.md | 1 | `release.py check` does not run the gallery tests |
| SCRIPTING.md, SIMULATION.md, wasm/README.md | 0 | accurate |

Outside the docs: `wasm/render/Cargo.toml:29` says the render API speaks
`egui_wgpu::wgpu` (no longer: the engine names `wgpu`), and the four
renderer gallery crates still depend on `egui-wgpu`, which none of them
now needs.

### The plans

- **rhai-plugins.md** opens "Nothing under *The plan* is built yet"; P0
  to P4 are built. Its *What is left* lists `params-one-system`, done.
- **wasm-modules.md** §1: the Rhai variations row is fixed (the flame and
  simulation modules build `lang`); every other row still holds
  (section 3).
- **v0.6.0-release.md**: current.

---

## 2. Parallel systems

### 2.1 Config writes: one setter, four sets of rules, three addressings

`ConfigManager::set_value` (`config/manager.rs`, 1,922 lines, 413 paths)
is the program's setter: every clamp and side effect for every
`ConfigPath`. Beside it:

- **The video export's applier** (`animation/export.rs`,
  `apply_config_value` + `apply_flame_value`: 566 lines, 162 paths) runs
  its own arms first and falls back to the manager's setter
  (`ConfigManager::apply_value_detached`, a throwaway manager around the
  config) for the rest. The fallback was added on 2026-10-08 because the
  arms had fallen behind: Palette Squeeze, Xaos and Solo animated in the
  preview and froze in the video, and so had camera X/Y and `Sim.Steps`,
  each found as a shipped bug (the comments and
  `a_sim_steps_ramp_advances_frame_by_frame` record them). Where an arm
  and the setter still differ, the preview (which uses the setter,
  `update_param_silent_on`) and the video differ.
- **The scripting API** (`script/api.rs`): `config.set("key", v)` patches
  the config's serde JSON by field name, with no clamps; about 200
  registered functions write fields directly, each with clamps of its
  own (`sim.dt` clamps against the ceiling; the
  2026-11-04 `max_dt` crash went through three such sites).
- **Python's `Config`** writes the JSON; the engine's parse and the
  renderer's own clamps are its only rules.

**Three ways to address one field**: `ConfigPath` (the UI, undo, history
labels, `UpdateType`); its string keys (`"Escape.ZoomLog10"`: tracks,
`.anim`, `anim.key`); serde field names (`"escape.zoom_log10"`: the
file, `config.set`, Python). `anim.key` already translates between the
last two.

**Is state management the editor's alone?** No. `ConfigManager` is in
`src/config`, compiled into every build, and the camera, fly mode and
the video export reach its rules through `apply_value_detached`. History,
preview and `UpdateType` are rightly the editor's. What is wrong is that
the *rules* -- what a value may be and what changing it implies -- are
reachable only through a manager, so the script API and Python grew
their own.

**Proposal.** Lift the rules out as a pure function:
`config::apply(&mut FractalConfig, EditingTarget, &ConfigPath,
ConfigValue)`. `apply_value_detached` already proves it can be separated,
since it works by building a manager to throw away. Then:
- `ConfigManager` wraps it with history, preview and `UpdateType`.
- The video export calls only it: its arms go, except
  `holds_exact_centre`.
- `config.set` in scripts and a `Config.set(key, value)` in Python take
  `ConfigPath` string keys through it, with the serde names as aliases.

The serde names stay the file's. This is also P5's prerequisite: the
plan says a script track's values must "reach both appliers", and with
one there is nothing to keep in step.

### 2.2 The headless engine facade, five times

Every way to drive the engine without the editor has written its own
front end:

| Surface | Where | Its own choices |
|---|---|---|
| CLI | `lib.rs` (`export_mode`, `export_animation_mode`), `script/cli.rs` | ipt 1024; sticky off; refuses unknown names; loads installed resources (export, video; not `generate`) |
| Web app API | `wasm_api.rs` (`WasmApi`) | `render()` on its own device |
| Gallery renderer | `wasm/render/src/lib.rs` (and its three siblings) | device and renderer reuse; ipt 256 (`RenderJob::new`); sticky on; no unknown-name check |
| Gallery scripts | `wasm/script/src/lib.rs` | the script host with both libraries |
| Python | `wasm/python/src/{render,scripts}.rs` | ipt 1024; sticky off; no unknown-name check; a copy of the script calls and of the video frame loop |

The same config can therefore render differently on two surfaces
(iterations per thread, the sticky superset), and a config naming
something a build lacks fails loudly on one and draws wrong on three.

**Proposal**: one facade in the library, with one set of defaults. It
would hold:
- the calls: render a still or a sequence, an animation session, the
  scripts (list, params, run stages), presets, `.flame` in and out, the
  PNG encode with metadata, unknown names, installed resources;
- `export_animation` split into a **frame source** (tracks, signals,
  moments, engines, frames) and the FFmpeg sink, so the CLI, the app,
  Python and later a WebCodecs export in the browser share the frames.

Each module then becomes a thin wrapper: wasm-bindgen, the C ABI or CLI
arguments. **This is where the copied code awaiting review goes away**
(pyfflame.md §4). The script calls and the frame loop have one home;
wasm/script and wasm/python call it.

### 2.3 The render tail, sequenced five times

The tail's stages are shared functions, but the sequence that calls them
is written out per path:
- `render_with`'s flame path, `render_sim` and `escape_tail` in
  `renderer/render.rs`;
- the viewport's `App::render`;
- `HighResExporter`, which tone maps in strips and skips density effects
  above a budget.

ARCHITECTURE.md §1 says so ("a new tail stage must be added to each"),
and the two video loops that existed until 2026-09-09 disagreed about
density effects and depth of field for this reason.

**Proposal**: a `Tail` that every generator hands its `Rgba32Float` image
to (density effects, tone map, colour effects, then an output: present,
readback, or strips for the large export). It does not merge the engines.
The viewport keeps its own loop for interaction and the governor, and
`HighResExporter` stays a separate engine for size. Only the stage
sequence becomes one. This is worth doing when the next tail stage
arrives, or before.

### 2.4 Carried sources: four `ensure`s, about 30 call sites

A config carries its Rhai sources; before it renders, each kind's
`ensure` registers them:
- `variations::custom::ensure`
- `escape::custom::ensure`
- `sim::custom::ensure`
- `effects::custom::ensure`

They are called from about 30 places in 14 files: `render_with`, the sim
renderer's entry points, reference orbits, the panels, `unknown_names`
and others. A new entry point that misses one draws that kind's
built-in fallback (a Mandelbrot for a formula). rhai-plugins P1 planned
"one `ensure` from a config's carried sources", and it was not done.

**Proposal**: `FractalConfig::ensure_sources()` (or the facade's job), one
call per entry point; the per-kind functions become its parts.

### 2.5 Three origin models

- `Provenance` (`Builtin`, `Api { version }`, `Custom`) on registry
  entries;
- `ScriptOrigin` (`Builtin`, `Shipped`, `User`, `External`) for CPU
  scripts;
- `storage::library::Origin` (`Shipped`, `Yours`, `Downloaded`) in the
  unified library scan.

rhai-plugins rule 7 wants one ("the namespace is the origin ... built
in, mine, downloaded, carried by a config"). A resource panel that lists
every kind together needs it.

### 2.6 How content arrives

There are five channels:
- sources carried in a config (rule 2: the artefact);
- the WGSL catalogs for variations and effects (kept by D2);
- the CPU-script resource (both ways);
- Rhai plug-ins from the local library, loaded at start;
- Poly Haven for skies and textures, planned, with the app talking to
  the provider directly.

These are deliberate decisions, and their costs are only:
- the surfaces without a store (the gallery, Python) cannot resolve a
  WGSL download and do not say so;
- the planned assets channel should share the download cache and the
  origin model rather than add a sixth way to keep files.

The WGSL catalogs are the one channel the direction retires (rule 2
calls them "the old form"). They go once the variation twins (P7) let
the catalog be Rhai sources.

---

## 3. The WASM modules

The library is built eight ways:

| Build | Engines | `lang` | Terrain | Purpose |
|---|---|---|---|---|
| desktop app + CLI | all | yes | yes | the editor, batch export |
| web app | all | yes | yes | the editor in a browser |
| `wasm/render` | all three | yes | no | gallery renderer, any mode |
| `wasm/flame`, `wasm/escape`, `wasm/sim` | one each | flame, sim: yes; escape: via its engine | no | smaller downloads |
| `wasm/script` | all (CPU only) | yes | -- | scripts, worker-safe |
| `wasm/python` (WASI) | all | yes | yes | pyfflame |

**The single-engine modules cut across real dependencies.** When they
were split, the engines were independent. Now:
- escape-time uses flame variations (the lens, transformations, the
  Flame Attractor in mode D, a solid's `vary`);
- escape textures render simulations;
- simulation layers are warped by the flame's transforms.

`wasm/escape` and `wasm/sim` build without `engine-flame`, so those
configs render with the variations missing: the Flame Attractor comes
out blank, a solid that `vary`s a built-in comes out as a Mandelbrot,
and a lens or warp is skipped. Nothing reports it, since no gallery
module calls `unknown_names` (wasm-modules.md §1, still true but for its
Rhai-variations row).

What the split saves: when measured, 0.80 MB gzip for `wasm/render`
against 0.73, 0.46 and 0.43 for flame, escape and sim (`wasm/README.md`;
render has since gained the simulation engine and has not been
re-measured).

**Options**, for the user:
- (a) one gallery renderer (`wasm/render`) and the three retired;
- (b) build escape and sim with `engine-flame`, wasm-modules.md's
  stopgap (+0.2-0.4 MB each);
- (c) variations as data, fetched per config.

(c) is the right end state, but **build it on Rhai sources, not on
`VariationDownload`'s WGSL**, or it becomes another consumer of the
format the direction retires. That puts it after the variation twins
(P7 step 4). Meanwhile (a) or (b), plus refusing unknown names as the
CLI does. Either way, the module boundary is not where the
duplication is: that is the facade (2.2).

The other two boundaries are right:
- `wasm/script` is separate because it needs no GPU and runs in a worker.
- `wasm/python` carries everything because a Python install has no
  download budget.

---

## 4. Surfaces against animation and scripting

| | Every engine | Run scripts | Script's animation | Play an animation | Video | Carried Rhai | API downloads | A name it lacks |
|---|---|---|---|---|---|---|---|---|
| App (desktop) | yes | yes | yes | yes | FFmpeg | yes | fetched | fetched |
| Web app | yes | yes | yes | yes | **no** | yes | fetched | fetched |
| CLI | yes | `generate` | `.anim` beside | `export-animation` | FFmpeg | yes | cached only | `export` refuses |
| Gallery | per module | `wasm/script` | `animation_json` | no | no | yes | **no** | **draws without it** |
| Python | yes | yes | yes | frames (a copy of the loop) | FFmpeg pipe | yes | **no** | **draws without it** |

Every surface reaches scripting. Animation is everywhere but the gallery,
and only Python reaches it through a copy. The gaps that matter:
- Python and the gallery should refuse or report a name they lack, as the
  CLI does (the facade, 2.2).
- The web app has no video. A WebCodecs encoder on the frame source
  (2.2) fits wasm-modules.md's "nearly everything in WASM, minus
  FFmpeg".
- Script tracks (P5) do not exist anywhere yet. Built on one applier
  (2.1), they reach every surface at once.
- **Simulations cannot be saved online** (`api/mod.rs:207`; open in
  v0.6.0-release.md). One engine of three is outside the API.

---

## 5. State management

Covered in 2.1: the store, history and `UpdateType` are sound, and
`ConfigPath` covers the new features (lighting, effects' parameters,
escape transformations, simulation layers and couplings, terrains, path
tracing, Rhai parameters under rule 3). The weak point is that the rules
are not callable without a manager. One more item, recorded in
rhai-plugins.md: a track that names a Rhai variation by a key a later
Use replaced still names the old one, because the base config is
re-keyed and the track is not.

## 6. The rendering pipeline

- **Sound**:
  - three generators, one image layout, one tail, one render API for
    every non-interactive path;
  - all three engines dispatched inside `render_with`;
  - `RenderEngines` for state that outlives a frame;
  - the viewport outside it on purpose.
- **Persistent orbits** are documented per surface. The sticky superset
  is on where batches need it and off where the CLI's picture is the
  contract, and its 4D leak is fixed (074c92ec).
- **To fix**: the tail's five sequencings (2.3), and the surfaces'
  differing defaults (2.2) -- ipt 256 against 1024 is a different flame
  sample set for the same config.

## 7. The governors

The governing is layered, and each layer answers a different question:

| Layer | Question | Where |
|---|---|---|
| Dispatch sizing | how much flame work fits this frame | `Batch`, `Knee`, `adjust_iter_scale`, in `app/mod.rs` |
| Quality rungs | what fidelity holds the target frame rate (escape, terrains, simulations, a flame's render scale and pacing cap) | `app/governor.rs` |
| Its fallback | the same without a GPU timer | `app/aa_governor.rs` |
| Device safety | how large a chunk can run without a driver timeout, learned and persisted | the escape and path-tracer renderers (`gpu_tuning.json`) |

Stills and exports are never governed, which is right. These are not
parallel systems. The one structural point is that the flame's dispatch
governor lives inside the 4,834-line `app/mod.rs`. Moving it beside
`governor.rs` would put the app's governing in one module; device
safety stays in the engines, where headless renders need it too.

---

## 8. The plans against the architecture

**In line.**
- Rhai as the definition with the source carried (rule 2): variations,
  escape, effects, simulations.
- UUID and hash identity (rule 7, `scripted.rs`).
- Parameters as `ConfigPath`s through one parameter system (rule 3).
- The library scan and plug-ins at start (P3).
- pyfflame carrying sources with no store.

**What the plans need first.**
- **P5, script tracks**: one applier (2.1). Without it a track's values
  go through two appliers kept in step by hand, as the keyframes'
  already were.
- **P6 and a resource panel for browsing effects and the rest**:
  - one origin model (2.5);
  - one `ensure` and, as P1 planned, one registry for compiled Rhai
    entries (2.4);
  - the library panel P3 left (`storage::library::scan` is ready and
    nothing draws it);
  - publishing and adopting GPU Rhai entries (P3's leftover);
  - the server's `kind` field;
  - simulations storable online.
- **Variations as data** (wasm-modules.md A): after the variation twins,
  in Rhai (section 3).
- **The facade** (2.2) is also where P6's downloads would reach Python
  and the gallery.

**The largest item, and the one that settles the others**: the 647
variation twins (P7 step 4). With them, the variation catalog is Rhai
sources, so:
- the WGSL catalogs and `VariationDownload` retire;
- the single-engine modules can fetch what a config needs;
- the online API stores one kind of thing.

---

## 9. A proposed order (for the user to pick)

1. **The docs** (section 1): one pass, cheap.
2. **Gallery correctness** (section 3): (a) or (b), and refuse unknown
   names. A live bug.
3. **One applier** (2.1), before P5.
4. **The engine facade** (2.2), with the frame source split out. This
   is the review of the copied code planned after the merge, so it
   folds in here.
5. **One `ensure`, one origin model** (2.4, 2.5), before P6 and the
   resource panel.
6. **The tail** (2.3), with the next tail stage or before it.
7. The plans: P5, then P7's steps, then P6 with the resource panel.
