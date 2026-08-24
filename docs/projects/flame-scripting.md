# Flame Scripting: Random Generation Scripts, Python, Web, Decompositions, L-systems

**Status:** Implemented (Phase 1 shipped; see §7b field notes). This is
the design record — the reasoning, the rejected alternatives, and what
each decision cost. For **how to write a script**, see
[docs/main/SCRIPTING.md](../main/SCRIPTING.md), which is the user-facing
reference and is kept current by a staleness test.

One system ties five requests together: user-writable random flame
generation scripts, a Python library, a web/in-app script platform,
variation decompositions (Kleinian, sphere packing, Möbius), and
L-system support. The tying point is a single sandboxed script engine
plus a scripting object model over `FractalConfig`, implemented once in
the main crate and exposed through three doors: the desktop app, the
existing WASM build, and a PyO3 Python wheel.

---

## 1. Decision record

### Script language: Rhai

Requirements: simple enough for non-programmers to edit, shareable
without security risk (explicitly: no raw Rust), runs identically on
desktop, browser (WASM), and Python.

**Chosen: [Rhai](https://rhai.rs)** — a pure-Rust embedded scripting
language.

- Sandboxed by construction: scripts can only call functions we
  register. No file/network/process access exists unless added.
- Built-in execution budgets (max operations, call depth, array/string
  sizes) — a hostile or buggy `loop {}` in a *shared* script terminates
  cleanly instead of hanging the app.
- Pure Rust ⇒ compiles into the existing WASM build with zero friction,
  and into a PyO3 wheel the same way. One engine, one behavior, three
  platforms.
- Rust-flavored syntax (`let`, `for`, `{}`) — accepted.

**Rejected:**
- *Lua (mlua)* — wraps C Lua: WASM build is painful, and Python would
  need a second runtime (`lupa`) that can drift. Pure-Rust Luas are
  incomplete.
- *Custom DSL* — we'd own a parser/interpreter/docs forever, and simple
  DSLs grow warts the moment someone needs a helper function.
- *Python as the script language* — cannot be sandboxed safely in-app;
  sharable Python is exactly the arbitrary-code risk this design exists
  to avoid.
- *Declarative JSON recipes* — cannot express the target examples
  (loops, conditionals, rng) without becoming an ad-hoc language.

### Python integration: PyO3 native wheel, NOT the WASM binary

Running our `.wasm` inside Python (wasmtime-py) was considered and
rejected: identical source-level reuse but a far worse seam — every
call crosses as serialized bytes (WASM has no object/string ABI and
wasm-bindgen's glue is JS-only), no numpy interop, and our WASM build
assumes a browser (WebGPU/web-sys) so a separate WASI build would be
needed anyway. Instead: `pyfflame` = PyO3 + maturin wheel over the same
crate. Real classes, real exceptions, native speed, one CI job for all
platforms. Reuse happens at the crate level, where it's total.

### Integration point: structural load, NOT ConfigManager deltas

Scripts operate on a plain `FractalConfig` (via the object model below)
and the app applies the result **as one atomic action** through the
same path presets use (`FlameRenderer::load_config` + one undo point).
This matches the existing rule that structural actions (preset load,
import, transform add/delete) live outside the delta system. Scripts
never see `ConfigPath`; the sandbox API is decoupled from ConfigManager
internals. Undo/redo integration comes for free.

### Determinism: fixed-algorithm seeded RNG

`rand()` in scripts draws from a PRNG with a **pinned algorithm**
(`rand_pcg::Pcg64Mcg` — not `StdRng`, whose algorithm may change
between rand versions). Script + seed ⇒ byte-identical flame on
desktop, web, and Python. This gives: precise sharing ("my script, seed
8842"), a trivial Reroll button (seed + 1), and reproducible bug
reports. `randomize.rs` already has the right shape
(`generate_random_flame_with_rng<R: Rng>`) — the script host follows
it.

Caveat to document for users: floating-point math in the *script* is
f64 and deterministic; tiny cross-platform differences can only enter
through the renderer, not the generated config.

**Update (rand 0.9 upgrade): the DRAWS are pinned too, not just the
generator.** Pinning `Pcg64Mcg` turned out to be half the job. The raw
PCG stream is a stable published spec, but the mapping *from* that
stream to a float or a bounded integer is a library implementation
detail — and rand 0.9 changed the integer one (it accepts a word 0.8
rejected, shifting every subsequent draw). The `wasm/script`
CLI-parity fixtures caught it: same seed, different flame.

So `script::host::draw` now owns all four draws the script API makes
(`unit`, `range_f64`, `below`, `range_i64`), reproducing rand 0.8.5's
`Standard`/`sample_single` exactly — verified against rand 0.8 across
tens of thousands of samples over many seeds and ranges before the
upgrade landed. Only `next_u64` still comes from the dependency, which
is the one piece that is the generator's own defined output. Guarded
by `draw_tests::pinned_draws_match_their_golden_values` (unit level)
and `random_stream_is_pinned` (end to end).

Related trap, already handled at the call sites: `usize` is 64-bit on
desktop and 32-bit on wasm32, and rand dispatched to a *different*
integer implementation for each — so a `gen_range(0..len)` forked the
stream between platforms. Every script draw takes a fixed-width `u64`.

---

## 2. Script model

### Two kinds: Generators and Modifiers

Declared explicitly by the script; drives which UI surface offers it.

- **Generator** — starts from `FractalConfig::default()`, builds a
  flame from nothing. Offered in the Random Generator panel.
- **Modifier** — receives a *copy* of the current config, transforms it
  ("add plasma if rng > 0.5", "jitter all colors", "decompose this
  kleinian"). Offered on the current flame (panel section and/or
  Fractal menu). Apply is still atomic + one undo point; Cancel
  discards the copy.

Both kinds are seeded. File extension: **`.rhai`** (free editor
support). Shipped examples live in `assets/scripts/generators/` and
`assets/scripts/modifiers/` (desktop auto-load, embedded on WASM like
presets).

### Metadata and declared parameters

Two-phase execution. The host first runs the script in **collect
mode** (scratch config, defaults, seed 0) to gather metadata and
parameter declarations; the UI renders them; the **real run** then
receives UI values. Collect mode is cached per script-text hash.

```rhai
script("Kleinian Duo", "generator");            // name, kind — required, first
let ta   = param("trace_a", 2.0, 1.8, 2.2);      // float slider
let n    = param_int("copies", 3, 2, 8);         // int slider
let wild = param_bool("wild_mode", false);       // checkbox
let kind = param_choice("style", ["Loxodromic", "Parabolic"], 0); // dropdown
```

Rules (documented, host-enforced where possible): `script(...)` must be
the first statement; `param*` calls must be top-level and unconditional
(a param gated behind `if` won't appear reliably in the UI — collect
mode uses defaults, so conditionals *around* params are the one
footgun; the host warns when a real run touches a param collect mode
didn't see). This is the same convention JWildfire script params use.

Param values are shown as the standard slider set (reusing
`VkbSlider`/param-UI conventions) above the Run button, with the seed
field and Reroll.

### Script descriptions

The panel shows each script's own header comment as its description —
the summary always visible, the rest behind a disclosure.

Read from the SOURCE rather than from a `description(...)` call. That
costs authors nothing (all eight shipped scripts already open with such
a block), needs no new syntax, and still shows for a script that fails
to compile — exactly when a reader most wants to know what it was meant
to do. It follows the convention the variation definitions already use:
a doc block above the thing it describes. (Worth noting the variation
blocks are Rust doc comments read only in the source; `VariationDef` has
no description field, so there was no extraction machinery to reuse —
only the convention.)

Three details came from looking at what the shipped scripts actually
contain, not from guessing:

* They open with a **title line** — the script's name. Taken as a title
  and dropped, because showing it would just repeat the picker's label.
  The first version used it as the summary, which made the feature
  useless: every description read "Turntable", "L-System Curve", …
  while the real prose stayed hidden. The test only asserted the summary
  was non-empty, so it passed.
* Sections are **ALL-CAPS lines** (`HOW IT WORKS`, `SOME TO TRY`), not
  `# Heading`. Both are accepted, and a heading may carry a lower-case
  parenthetical aside.
* Indented lines are **tables** — the L-system symbol list — so they
  render monospace and unjoined, while prose lines are joined into
  paragraphs and left for egui to wrap at the panel's real width rather
  than frozen at the source's 72 columns.

### Script flags

`script(...)` takes an optional third argument, a list of switches:

```rhai
script("Turntable", "modifier", ["norng"]);
```

Two arities of one registered function, so the list stays optional and
every script written before flags existed is untouched.

`norng` says the script ignores the seed. The panel then hides the seed
field, Reroll **and** Batch — all three are ways of asking for a
different random result, and a control that changes nothing is worse
than an absent one because it implies the result varies. Five of the
eight shipped scripts are fully deterministic and declare it.

Flags are a small set of named booleans rather than a free-form bag: a
flag exists to change what the UI does, so the panel has to understand
each one anyway. Adding one is a field on `ScriptFlags`, a match arm,
and its use. A flag this build doesn't know is an **error** naming the
ones it does — a silently ignored switch looks like the feature is
broken.

`norng` is a claim about behaviour, not just a UI hint, so a test runs
every shipped script that declares it at two very different seeds and
asserts the output is identical.

### The object model (what scripts can touch)

Write access: **everything in `FractalConfig`** — flame structure,
variations + params, colors/palette, camera (all 4 angles + position),
render mode, tone mapping, depth effects, post-effect chain,
`max_iterations`. It's all sandboxed state; the apply step validates.

Sketch of the API surface (final names settled in Phase 1 review):

```rhai
script("Demo", "generator");

let n = rand_int(2, 4);
for i in 0..n {
    let t = flame.add_transform();
    t.add_variation("linear", 1.0);
    t.set_variation_param("mobius.re_a", 1.0);   // "name.param" keys, as in the app
    t.weight = rand(0.5, 2.0);
    t.color = rand(0.0, 1.0);
    t.translate(rand(-1.0, 1.0), rand(-1.0, 1.0));
    t.scale(rand(0.5, 1.5));                      // 50–150%
    t.rotate(rand(0.0, 360.0));
}

if rand() > 0.5 {
    flame.add_effect("plasma");
}

flame.set_palette(random_palette());              // draws from the loaded library
config.render_mode = "3d";
config.camera.pitch = 35.0;
config.tonemap.gamma = 2.4;
```

Host-side building blocks:

| script call | backed by |
| --- | --- |
| `flame.add_transform()` / `.final_transform()` / xaos setters | `scene::transforms` |
| `t.add_variation(name, w)` | registry lookup — unknown names are a script error *with the line number*, not a silent no-op |
| `t.set_variation_param("name.param", v)` | validated against the variation's param defs |
| `flame.set_palette(name)` / `flame.random_palette()` / `palette_names()` | `PaletteLibrary`, seeded pick |
| `flame.add_effect(name)` | effect chain |
| `rand()` / `rand(a,b)` / `rand_int(a,b)` / `pick(array)` / `shuffle(array)` | seeded Pcg64Mcg |
| `variation_names(category)` | registry — lets scripts do "pick a random Blur variation" |

Apply-time validation (script-error on violation, not clamp-and-hope):
`MAX_TRANSFORMS` (128), `MAX_VARIATIONS_PER_FLAME` (100),
`MAX_VARIATION_PARAM_SLOTS` (1600), finite floats everywhere.

### Sandbox budgets

`Engine::set_max_operations` (~5M ops — generation workloads are
thousands), call depth 64, max array 100k, max string 1 MB, no modules,
no `eval`. Heavy math does not run in-script — see built-ins (§4).

**Limits on what a script can ask the NATIVE side for** (added after a
security review; each closed a confirmed abort or hang):

| limit | value | why |
| --- | --- | --- |
| transforms per pool | 128 (`MAX_TRANSFORMS`) | was uncapped until render time — a script built 200,000, which also armed the O(n²) `set_xaos` table |
| xaos row length | 128 | `vec![1.0f32; count]` took an unbounded `i64`; `exclude_xaos_row(0, i64::MAX)` aborted the process on one line |
| L-system rule | 4096 chars | the piece walks are `rule × body`; a 200k rule ran ~10¹¹ native steps and never returned |
| L-system axiom | 4096 chars | same walk |
| rules per system | 64 | bounds the partner search, which is O(rules × rule length) |

The structural point behind all of them: **the operation budget cannot
see native work.** `on_progress` fires between interpreter operations,
so a single built-in call runs to completion however long it takes —
and the Scripts panel runs on the UI thread, so there is nothing to
interrupt it. Bounding the *input* is what keeps the walk bounded.
A wall-clock deadline inside the walks is still worth adding; it is the
general fix, of which these caps are the specific one.

For scale: the longest rule in any shipped script is the Peano curve's
21 characters, so these ceilings are orders of magnitude above real use.

---

## 3. Architecture and factoring

New module **`src/script/`** in the main crate (no workspace split yet;
the crate already builds as rlib + cdylib):

- `host.rs` — engine setup, budgets, collect/real two-phase runner,
  seed handling, error mapping (Rhai positions → user-facing messages).
- `api.rs` — the object model registration (`flame`, `t`, `config`,
  rng, palette, registry queries).
- `builtins.rs` — heavy-math helpers (Phase 4): Kleinian/Apollonian
  recipes, L-system expansion.
- `mod.rs` — `ScriptKind`, `ScriptMeta`, `ScriptParamDecl`, results.

**Feature gating — planned, then found unnecessary.** The intent was a
`gui` feature (default) gating `app/`, `ui/` and the winit/egui/wgpu
deps, so the core would build with `--no-default-features` for the
wheel. Phase 5 showed that isn't needed: the wheel depends on the crate
as it stands, with default features, and the linker drops everything it
never calls (2.6 MB, no GPU or window code in it). The refactor would
have bought nothing but risk to the editor. See Phase 5 below.

A user script can be **deleted** from the picker row, behind a
confirmation (there is no undo for a removed file), reusing the shape
the Palette Editor already uses. The guard matters more than the button:
`discover` hands `ScriptOrigin::File` to the shipped `assets/scripts/`
files *and* to the user's own copies, so the origin alone does not say
who owns a script — `is_user_script` checks the canonical path lies
inside the user folder, and `delete_user_script` refuses anything else.
Without that, Delete would remove the starters that ship with the app.
Deleting a user copy re-reveals the shipped script it was shadowing,
which is how you reset an edited starter.

The panel's editor has **Open…** and **Save As…** alongside Save and
Revert, taking the same file-dialog route the Animation panel uses for
`.anim`: `rfd` on desktop, the browser picker and a download on the web.
An opened file joins the picker as an entry so the combo keeps naming
what is actually in the editor. (The browser picker stashes its text in
egui's temp store, which the panel collects itself — no app-level
plumbing needed.)

UI: script panel added to the existing **Random Generator panel**
(`PanelType` addition), plus a Modifiers entry point on the current
flame. Editing: `egui` multiline code editor is fine for v1 (no
highlighting); load/save/reload-from-disk buttons so people can use a
real editor alongside.

Errors must be first-class: Rhai gives position info — the panel shows
`line 7: unknown variation "linnear"` next to the editor. For the
target audience this is the single most important UX feature.

---

## 4. Built-ins for the heavy math (decompositions & L-systems)

Rhai is an interpreter — orchestration is fine, tight numeric loops are
not. Anything algorithmic ships as a **Rust built-in registered in the
engine**, so scripts stay short and Python gets the same function from
the same crate.

### Decompositions

A decomposition is a Modifier script (or a Generator from parameters):
where a packed variation iterates its group *inside* one variation,
the decomposed form emits N transforms, each carrying one generator as
an explicit `mobius` (or affine) variation with visible parameters.
Payoff: the group structure becomes flame structure — xaos weighting,
per-generator coloring, per-transform animation, triangle-editor
manipulation.

Built-ins:
- `kleinian_generators(ta, tb)` → the four Möbius coefficient sets via
  Grandma's recipe (complex trace solve — the Markov identity quadratic
  lives in Rust, not script).
- `apollonian_circles(...)` / sphere-packing inversions via
  Descartes/inversive coordinates.
- `mobius_from(...)` helpers mapping coefficient sets onto our `mobius`
  variation's params.

Example shape:

```rhai
script("Decompose Kleinian", "modifier");
let g = kleinian_generators(param("trace_a", 2.0, 1.8, 2.2),
                            param("trace_b", 2.0, 1.8, 2.2));
flame.clear_transforms();
for m in g {                      // a, b, A, B
    let t = flame.add_transform();
    t.add_variation("mobius", 1.0);
    t.set_mobius(m);              // writes re_a/im_a … re_d/im_d
    t.color = rand(0.0, 1.0);
}
```

Targets, in order: Kleinian (recent work, freshest), sphere packing,
plain Möbius/Littlewood-style linear sets. Decompositions can also run
Python-side (same functions through the wheel) — but in-app-first since
that's where regenerate-and-view is fast.

### L-systems

Built-in `lsystem(axiom, rules_map, depth)` → expanded string, and
`turtle(expanded, angle, step)` → array of segment poses
(position, heading, depth, branch index). The *script* owns the
interesting choice — how poses become transforms (affine along each
segment, color by branch depth, weight by generation) — which is
exactly the part a fixed variation can't leave open. Expansion depth is
budgeted (string cap) like everything else. Precedent: mondrianomies
already does CPU-side L-system work; this generalizes it to flame
structure.

---

## 5. Python library (`pyfflame`)

Separate deliverable, same crate. PyO3 + maturin wheel exposing:

- `FractalConfig` load/save: `.fflame` (JSON) — serde code, zero drift.
- `.flame` XML import/export — `flame_xml.rs`, including all the unit
  conversion quirks, for free.
- `.anim` load/save — animation model.
- The full object model (same semantics as the script API — mutate
  transforms/variations/params/palette/camera/etc.).
- `run_script(text, seed, params)` — the *same* Rhai engine, so a
  script authored in-app runs identically in a Python pipeline.
- The built-ins (`kleinian_generators`, `lsystem`, …) as plain Python
  functions.
- `render(config, out_png, width, height, ...)` — subprocess wrapper
  around the existing CLI export mode (`fractal_flame_wgpu export`),
  with the binary path configurable. No GPU bindings in the wheel.

Repo layout: `python/` (pyproject.toml + maturin config + pure-Python
conveniences + tests). Wheel builds via CI matrix.

---

## 6. Web

The engine rides the existing WASM build (pure Rust — no new deps
class). The script panel works in the web app exactly as on desktop,
which covers the stated preference: load the script *into the app* for
fast generate/regenerate/view. A standalone "script playground" page
(script + params + thumbnail via `wasm_api`) is a thin later add, not a
platform.

---

## 7. Phases

Each phase ships independently.

**Phase 1 — Engine core + CLI (the load-bearing phase).**
`src/script/` host + object model + seeded RNG + budgets + validation;
two-phase param collection; `ScriptKind`. CLI:
`fractal_flame_wgpu generate --script x.rhai --seed 42 [--set name=val …] -o out.fflame`
— fully testable headless. Tests: determinism (same seed ⇒ identical
JSON), budget kill, unknown-name errors with positions, param
collection, generator-vs-modifier semantics.
*Exit criterion: the user's example script from this plan runs and
produces a valid .fflame.*

**Phase 2 — In-app panel.** ✅ Implemented as a **dedicated `Scripts`
panel** rather than a section of the Random Generator, which the plan
originally suggested. Two reasons: modifiers are not "random generation"
and would sit oddly under that heading, and the editor plus parameter
sliders need more room than a section affords. One panel holds both
kinds, switching its verb (Generate / Apply) on the script's declared
kind. Contains: script picker (embedded starters + `assets/scripts/` +
user folder, later sources shadowing earlier by file name), parameter
widgets built from the collect pass, seed + Reroll (seed + 1), Batch
across consecutive seeds into the existing Fractal Browser, a code
editor with Save (always to the user folder — shipped starters are never
overwritten) and Revert, and an error line that leads with the line
number. Apply goes through `load_config_with_undo`, so a run is one undo
step, same as a preset load.

Generators start from a default config carrying the **current palette**
over, since the script API cannot pick palettes yet — without that every
generated flame would arrive in the default Fire palette. A
`set_palette`/`random_palette` API would supersede this.

Stretch (not done): port/mirror `randomize.rs` presets as example
generator scripts (dogfooding — proves API coverage; the Rust randomizer
stays).

**Phase 3 — WASM enablement.** Panel on in the web build; embedded
starter scripts (mirroring the preset embedding); verify budgets on the
browser main thread (chunked/async run if a long script janks).

**Phase 4 — Built-ins.** Done. All four packed groups decompose through
one `decompose_group.rhai`, and `lsystem.rhai` builds curves from
rewriting rules.

Validated by render comparison against the packed original (mean
absolute difference over the frame):

| | 2D | 3D |
| --- | --- | --- |
| `schottky_group` | 0.001 | 0.032 |
| `apollonian_gasket` | 0.009 | 0.180 |
| `klein_group` | 0.009 | 0.008 |
| `sphere_packing` | 1.07 | 2.39 |

The packing residual is not error: its decomposition lights a strict
SUBSET (zero pixels lit only in the decomposition). The packed variation
also *seeds* points onto the configuration spheres so their outlines
render crisply, which a pure IFS has no equivalent for. Restoring it
needs a circle-boundary sampler; `blur_circle` fills a disc.

Three findings worth keeping:

- **Every group has its own "don't backtrack" rule, and they differ in
  distribution, not just in which index is blocked.** Schottky and
  Apollonian redraw into the NEXT generator, so it gets double share
  (`avoid_xaos_row`). A packing blocks repeating the same mirror, since
  an inversion is its own inverse (`repeat_xaos_row`). `klein_group`
  excludes the inverse and draws uniformly from the remaining three
  (`exclude_xaos_row`). Index order differs too: `[a, b, a', b']` for the
  Schottky family, `[a, a', b, b']` for Klein.
- **Z has to be carried deliberately.** With `preserve_z` off the
  renderer flattens z each iteration and only `AlwaysZ` variations
  survive; `apollonian_gasket`/`schottky_group` are `AlwaysZ` but
  decompose into `mobius`, which is not, so a 3D flame collapsed to the
  flat 2D group *while still looking correct*. The script now sets
  `preserve_z` exactly when the source is `AlwaysZ` and the target is
  not, read from the registry via `variation_always_z()`. Blanket-setting
  it breaks `klein_group` just as badly the other way.
- **An inversion needs no special variation.**
  `x -> c + r²(x-c)/|x-c|²` is translate → `spherical3D_wf` (weight r²) →
  translate back, so packing mirrors land as ordinary transforms the
  triangle editor can grab.

L-systems cover the full gamut across two scripts:

* **L-System Curve** — edge rewriting (Koch, dragon: drawn depth-1
  segments are the pieces; `contractiveness()` independently confirms
  ln(1/3), ln(1/√2), ln(1/2)) and node rewriting (Hilbert, Peano: one
  map per variable occurrence, spans measured on a deep expansion with a
  Richardson step and snapped to the rational grid). Output as the
  infinite-depth attractor, or as the finite-depth path via the
  `lsystem_path` variation — no baked geometry, vertices are digit-wise
  map compositions, `iterations` is a live parameter, and node curves
  draw the CENTRE chain (cell spans lie on the boundary lattice and
  overlap; centres are the classic self-avoiding drawing — found when an
  in-app render survived exact maps unchanged).
* **L-System Plant** — bracketed rules by the Barnsley-fern
  construction: recursion sites become branch maps, drawn stems become
  SQUASHED maps laying a flattened copy of the whole plant along the
  stem (the rachis trick), bracket nesting becomes colour, size becomes
  weight. Handles variable recursion (`X=F-[[X]+X]+F[+FX]-X`, `F=FF`)
  and drawing recursion (`F=FF-[-F+F+F]+[+F-F-F]`, no separate stems).
  Linked transforms were considered for ordering and declined: a linked
  chain changes the walk's order, never the attractor set.
  **Tree mode** (the 2D default) draws the FINITE depth of the book
  figures via the `lsystem_tree` variation — each sample picks a level
  weighted by the drawn length there (ρ^l, ρ = Σ branch scales),
  composes that many scale-weighted branch maps, and lands on a random
  stem segment; depth is a live parameter. Needed because the attractor
  and the book picture are different objects once the branch system's
  similarity dimension reaches 2 (`F=F[+F]F[-F][F]` is 5 copies at
  0.49 → dim 2.27): every finite depth is a clean bush, the limit is a
  filled plane — the same modality lesson as the curve script's Path
  mode, rediscovered when the ABOP presets came out "bushy"/"clouds".
  A stem run spanning the whole displacement (the 3D Bush's trunk —
  every recursion site bracketed) also exposed that stem maps must be
  contractions of the plant's measured EXTENT onto their segment, or
  the trunk drops out of the attractor entirely.

Both scripts speak **3D** (ABOP's `&`/`^` pitch and `\`/`/` roll,
auto-detected — no mode switch): pieces come back as full 3D matrices
carried by the new `matrix3D` variation (a raw-matrix container; the
existing `affine3D` is JWF's rotate/scale/shear parameterization, the
wrong shape for exact measured maps), with `preserve_z`, 3D render mode
and a tilted camera set automatically. Path mode uses `lsystem_path_3D`
(12 coefficients per map). Findings: a 2D segment cannot carry roll, so
3D pieces store the turtle's frame; the global frame must be rotated so
the displacement runs along +x or path anchors point the wrong way
(found as a shattered render); the shader builder only carries top-level
`fn`/`const` items, so variation WGSL cannot declare structs. And an
honest limitation, verified against an independent simulation: many 3D
edge-rewriting CURVE rules have no stable self-similar limit — their
rotations compound so the displacement direction never settles — which
is why 3D L-system art is nearly always plants. **4D was considered and
deferred**: no established notation exists to paste from, and the flame
model has no 4D affines; the one viable door, noted for later, is a
path-variation map bank (its maps are dimension-agnostic parameters).

**Graph-directed systems are SUPPORTED after all** — the earlier
"cannot express" claim was wrong, and the user's instinct (xaos) was the
mechanism. A multi-variable system is a graph-directed IFS, and the
chaos game for a GIFS is exactly a flame with xaos: one transform per
occurrence, allowed to follow another only when it consumes the type the
other produced (`occ(next) == owner(prev)`), with per-transform OPACITY
hiding every type except the axiom's — the scaffold curves drive the
dynamics invisibly. This removes the mirror-pair-only limitation in both
2D and 3D. Routing note: a graph-directed verdict must gate EDGE mode
off, or it builds a curve out of the connective F segments (the Peano
trap again). And the visible 3D Hilbert exists by a second route:
single-type self-similar 3D Hilbert curves exist (Haverkort), and
`hilbert3d_maps()` CONSTRUCTS one — a deterministic search over the
cube's 48 symmetries, octant by octant in Gray-code order, picking the
first that keeps entry/exit corners chaining. Verified by invariants
(tiling, 1e-12 chaining, determinism) and shipped as `hilbert3d.rhai`:
path mode draws the genuine cube-filling maze through `lsystem_path_3D`
with a live depth parameter.

**Maintenance risk:** `builtins.rs` hand-mirrors four shader sources
(`complex.wgsl`, `init_schottky_group`, `apollonian_gen`/`su_conjugator`,
`sp_conf2/3`+`sp_mirror2/3`) and `init_klein_group`. If any changes, the
decompositions silently drift from their originals. Each site says so,
but a shared definition would be better.

**Phase 5 — `pyfflame`.** Done, and smaller than planned.

`python/` is a **standalone crate with its own `[workspace]`**, depending
on the main crate by path. The app is not touched at all: no features
toggled, no modules gated, so its build and codegen are byte-for-byte
what they were. That was the user's constraint (zero performance impact,
no rewrites of the app) and it turned out to cost nothing — see below.

Ships: `Config` (flame model, camera, colour, render settings),
`.fflame` and `.flame` read/write as both files and strings,
index-addressed transform/variation/parameter editing with registry
validation, `run_script(source, seed, params, base)`, and registry
queries. No rendering: PNGs come from the existing CLI exporter, called
as a subprocess.

**The feature-gating refactor was never needed.** The plan assumed the
wheel required severing the GUI. Measurement first said the core was
nearly free-standing anyway — 211 variation files with zero GUI/GPU
references, and exactly ONE code edge from the core into GPU-land
(`MAX_BLUR_BUFFERS`, a `const u32`; the other two hits are doc
comments). Then the spike showed even that didn't matter: the crate
builds as a path dependency unmodified, and the linker drops the unused
GPU and window code. **The wheel is 2.6 MB.** A refactor touching the
whole crate would have bought nothing and risked the editor.

**Verified against the app, not against itself.** `run_script` output is
byte-identical to `fractal_flame_wgpu generate` for the same script and
seed, so the two entry points cannot silently diverge. Ten Python tests
cover both formats round-tripping, seed determinism, and error quality.

**Parameter coercion follows the script's DECLARATION**, mirroring
`generate --set`, rather than guessing from the Python type. Caught in
testing: passing a choice as a plain int reads as an index, so
`output=0` selected "Attractor" while the caller meant "Path" — the
script looks broken when the argument was merely misread. Choices now
take an option's name or its index, and a bad one lists what's allowed.

Everything still outstanding on the Python side — ergonomics, the rest
of the model surface, wheel CI, and the nice-to-haves — is specced in
[pyfflame.md](pyfflame.md).

**Phase 6 — Animation-track generation.** Done.

A script may optionally describe how its flame MOVES, not just what it
is. Entirely opt-in: a script that never mentions `anim` produces no
animation, and every script written before this is byte-identically
unaffected (pinned by a test over the whole shipped set).

```rhai
anim.name = "Spin";
anim.duration = 8;                      // optional — defaults to the last key
anim.key("rotation", 0.0, 0.0);         // the same names config.set takes
anim.key("rotation", 8.0, 360.0);
anim.key("zoom", 0.0, 1.0, "ease_in_out");
anim.interpolation("zoom", "smooth");   // per-track curve

t.key("weight", 0.0, 0.2);              // per-transform, on the handle
t.key("julian.power", 0.0, 2.0);        // variation params too
```

**Targets are built as `ConfigPath` values and asked for their string
key, never hand-formatted.** Tracks address parameters by
`to_string_key()` strings that the loader parses back with
`from_string_key`; emitting `"Transform.0.Weight"` by hand would be a
second spelling of the same thing, free to drift from the parser. Going
through the enum makes every target one the loader accepts by
construction.

**Two spellings, both accepted.** `config.set` addresses fields by their
serde name (`camera_rotation_x`) while tracks use `ConfigPath` keys
(`CameraRotationX`). Requiring authors to know both would be a trap, and
the two differ only by case and separators — so a snake_case name is
converted and retried. A name that resolves to neither is an error, not
a dropped track: a track that silently does nothing is invisible in a
rendered animation.

The `.anim` carries the flame as its `base_config`, so it stands alone.
`generate` writes one beside the `.fflame` when the script defined one;
the Scripts panel loads it into the timeline (binding after the config,
so tracks resolve against the flame they were written for); Python gets
`result.animation` with `save`/`load`.

**Angles are radians.** `rotation` and the `camera_*` angles are stored
in radians; the View panel converts for display, so the number on the
slider is not the number in the file. Three shipped scripts had been
setting `camera_rotation_x` to `35.0` as though it were degrees — that
is 35 radians, about 2005 degrees, landing on an arbitrary angle after
wrapping. Fixed to `35.0 * PI() / 180.0`, and called out here because
nothing in the type system distinguishes the two.

Ships `turntable.rhai`, a modifier that adds a seamless looping spin to
whatever flame is open — reading `render_mode` to orbit the camera in 3D
or rotate the image in 2D, and starting from the current angle so the
first frame is the picture you were already looking at.

Verified through the app's own `AnimationController`: the tests load
what a script produced and assert the values it evaluates at t=0, 4 and
8, so a script's animation cannot be well-formed-but-unplayable.

Not done: signal-driven tracks from scripts (`TrackSource::Signal`), and
generators. Both are additive — the object model already has room.

**Phase 7 — Palette generation, in scripts.**

Today a script picks an *existing* palette (`flame.set_palette(name)`,
`flame.random_palette()`, both seeded) or leaves the current one alone.
It cannot BUILD one — and that, not the colour picker or the calling
machinery, is the load-bearing gap.

**Decision: palette generation is a script, not a shared Rust
generator.** The earlier plan put a generator in `scene::palette` with
the Palette UI and the script API as two callers. Scripts win instead:
they are shareable, editable by the person who wants a different scheme,
and they get batch-and-choose for free — a palette modifier plus the
existing Batch button already yields a grid of options in the Fractal
Browser. What that costs is a generate button in the Palette Editor,
bought back by letting panels RUN scripts (below).

Four pieces, in the order they unblock each other:

*1. Build a palette.* `flame.set_palette_colors(name, [c, ...])` for
evenly spaced stops and `flame.set_palette_stops(name, [[pos, c], ...])`
for explicit ones. `Palette` is just a name and a list of
`(position, rgb)`, so this is small — but nothing else in the phase
works without it.

*2. A colour type and a colour parameter.* `param_color("base",
"#ff8800")` renders through `color_edit_button_rgb`, which three panels
already use. Rhai gets a `Color` with `.r/.g/.b`, `.h/.s/.v`, and
`rotate_hue` / `with_saturation` / `mix` / `hex`.

HSV earns its place: the relationships colour theory NAMES are literally
coordinates in it — complementary is `h + 180`, triadic `h ± 120`,
analogous `h ± 30`, monochromatic is "hold h, vary s and v". None of
those is simple arithmetic in RGB, and interpolating between two hues in
RGB passes through desaturated mud. The same goes for the "mixed-up
stops with noise" end of the dial: jittering H/S/V independently is
meaningful, jittering R/G/B independently is not.

Known limit: HSV is not perceptually uniform, so equal hue steps do not
look equally different — yellows and greens crowd. If generated palettes
come out uneven, OKLCH is a drop-in second constructor and accessor pair
(~30 lines) with the same `h/s/v`-shaped API.

*3. Stable script ids.* Needed so one script can name another, and
already a latent bug: the picker keys on the DECLARED name and `reload`
restores the selection by it, so two scripts declaring the same name
already misbehave.

The **file stem is the id** (`random_palette`). It is already the
de-facto unique key — `discover` builds its map keyed on file name, with
later sources shadowing earlier — so `run_script("random_palette")`
means "whichever `random_palette.rhai` won", and a user copy shadowing a
shipped script keeps working without a special case. Surfaced in the
picker.

Accepted consequence: shipped script FILENAMES become a public API.
Renaming one breaks its callers, the same class of rule as append-only
variation registration.

*4. `run_script(id, params)`.* The callee works on the same config, so a
palette script called from a generator simply sets that flame's palette.
No return value: the shared config covers every case we have, and it
keeps both the model and the sandbox simple.

  - **Seed**: the callee CONTINUES the caller's RNG stream rather than
    re-using its seed. Still exactly reproducible from (script, seed),
    but two calls give two palettes instead of the same one twice.
  - **Params**: `run_script("random_palette", #{ scheme: "complementary" })`
    feeds the existing `provided` mechanism. The callee's parameters do
    NOT surface in the caller's panel — the caller owns them.
  - **Collect mode**: a no-op, so metadata collection stays fast and
    free of side effects.
  - **Errors and print output** are attributed to the script they came
    from. An unattributed line number in an unknown file is a dead end
    for this audience.

**Runaway protection is not optional here**, because scripts are things
people share. Calling a generator from a generator is allowed, so the
guards have to be structural rather than a rule about what to call:

  - A **shared operation budget** across every nested run. Rhai enforces
    `max_operations` per evaluation, so a fresh sub-run would otherwise
    get a fresh 5,000,000 — and `for i in 0..1000 { run_script("x") }`
    would buy a billion. The counter accumulates through `on_progress`
    into the script state.
  - **Cycle detection** on the call stack of ids: A → B → A is an error
    naming the cycle, not a hang.
  - A **nesting depth cap**, for long chains that never repeat an id.

**Panels can run scripts.** The Palette Editor has a *Generate from
script* section listing everything that declares the `palette` flag,
with the script's own parameters, a seed and Reroll. One implementation,
two surfaces, no shared Rust generator — and the same hook serves any
panel that later grows generators of its own.

It applies through `ConfigPath::Palette`, the route every other edit in
that panel takes, so undo and the GPU update come for free. It takes
**only the palette** from the result: a script can touch anything in the
config, and a button called Generate in the Palette Editor rewriting the
flame is not what anyone would expect.

The parameter controls are shared with the Scripts panel
(`ui/script_params.rs`) rather than copied, since the Palette Editor is
unlikely to be the last panel that wants them.

**Palettes are built slot by slot.** `flame.palette_to_fixed()` converts
to the 256 evenly spaced slots the Palette Editor's Fixed switch
produces, `flame.palette_colors()` reads them out and
`flame.set_palette_fixed(name, colors)` writes them back (resampling to
256 rather than rejecting a shorter list). Even slots are what make the
two effects on top simple: slicing the palette up is array work, and
per-slot noise lands evenly instead of bunching wherever the gradient
stops happened to sit.

`random_palette.rhai` uses all three — scheme, then a slice-and-reorder
shuffle, then H/S/V noise with a separate amount per axis. Separate axes
are the point: nudging hue keeps the brightness structure and scatters
the colour, nudging value keeps the colours and roughens the light, and
the same jitter in RGB would just make mud.

`iq_palette.rhai` builds Inigo Quilez's procedural palettes,
`a + b*cos(2*PI*(c*t + d))`, with his seven published sets plus Custom.
Three cosine waves slightly out of step sweep through hues no single hue
ramp produces — and the classic rainbow's `d = (0, 0.33, 0.67)` is a
third of a cycle apart, which is the same triadic 120° the Random
Palette script rotates by, arrived at from the other direction. Verified
against an independent evaluation of the formula rather than against
itself.

**Status: done.** All five steps are in. What is not done, and is
additive: `run_script` returning a value, forwarding a callee's
parameters into the caller's panel, and OKLCH beside HSV.

---

## 7b. Phase 1 field notes (implemented)

Things learned building the engine that the later phases and the docs
need to account for:

- **Rhai can't assign through a call result.** `flame.transform(i).color
  = 0.25` is a syntax error ("expression cannot be assigned to"); you
  must bind first: `let t = flame.transform(i); t.color = 0.25;`. Users
  will hit this — the shipped examples model the correct form, and it
  belongs in the user docs.
- **Globals are variables, not constants.** Rhai refuses property and
  indexer assignment on a constant, so `config["gamma"] = 2.2` and
  `flame.name = "x"` fail if `flame`/`config` are pushed with
  `push_constant`.
- **Seeds must be scrambled, not OR'd.** PCG64-MCG needs an odd state;
  forcing the low bit maps seeds 8842 and 8843 onto the *same* stream,
  silently breaking "reroll = seed + 1". `expand_seed` runs SplitMix64
  over both 64-bit halves. Implemented in-repo rather than via
  `SeedableRng::seed_from_u64` so the mapping can never shift under a
  dependency bump — script + seed is a shareable artifact.
- **`.fflame` text is not byte-stable** (pre-existing, not caused by
  scripting). `variations`, `variation_params` and effect `params` are
  `HashMap`s, which serialize in per-instance hash order, so two saves of
  an identical flame differ textually. Determinism tests must compare
  `serde_json::Value` (BTreeMap-backed, sorted), not strings. Worth
  considering separately: sorted serialization would also stop committed
  presets and test configs producing spurious diffs on re-save.
- **rhai pulls in smartstring**, which adds a second `Add` impl for
  `String`. That made an existing line in `shader_builder_v2.rs` stop
  compiling — with two candidate impls, `&String` no longer deref-coerces
  to `&str` in operator position. Fixed with an explicit `.as_str()`.
- **`config.set` ambiguity is resolved by key presence.** ~30 of 82
  config fields are skip-if-default, so absence from the JSON doesn't
  prove a typo. The rule: a key already present is known (no warning even
  if the write is a no-op); only absent-and-no-change warns.
- **Convergence is a whole-flame property, not a per-transform one.** An
  individual transform scaling above 1 is legitimate and often where the
  structure comes from; what decides whether the chaos game settles is
  the probability-weighted mean log scale, `Σ pᵢ·0.5·ln|det Aᵢ|`, where
  `pᵢ` is the transform's selection probability. Exposed to scripts as
  `flame.contractiveness()` / `flame.set_contractiveness(target)` (one
  shared factor, so each transform keeps its character) and
  `t.area_scale()`. Verified: at a fixed seed, target −0.25 renders a
  clean flame while +0.3 scatters the same flame into near-blackness.

- **Mutation is a modifier run in batch.** Apophysis-style "mutate" needs
  no new mechanism: a *modifier* script re-run across consecutive seeds
  from the same starting flame IS a batch of mutations. The panel names
  the button after the intent (`Mutate` for modifiers, `Batch` for
  generators) and leads the batch with the untouched original, so picking
  "none of these" is a click rather than an undo. Shipped as
  `modifiers/mutate.rhai` with strength, a mode (Everything / Shape /
  Variations / Weights / Colours) and a balance-preserving option.

- **Variation weights scale the transform's output, and the balance
  metric has to count them.** The dispatcher computes
  `result = Σ_v w_v · f_v(A·p)`, so a lone `linear` at weight 1.18 is an
  18% expansion regardless of the affine. The first version of
  `contractiveness()` read only `det A`, so `set_contractiveness`
  rebalanced the affines while the true scale drifted — a mutated flame
  rendered to haze even though the metric said balance was preserved, and
  the unit test passed because it compared the same wrong metric against
  itself. **Only the render caught it.** The term is now
  `0.5·ln|det A| + ln(Σ w_v)`, over variations that actually join the
  weighted sum: `Normal` always, `Any` unless this transform's
  `fx_priority` moved it (<0 pre, >0 post), `Pre`/`Post` never — matching
  the shader builder's own rule. Two traps found along the way: `linear`
  is phase `Any`, not `Normal`, so a naive `== Normal` filter summed to
  zero; and a transform with nothing in the weighted sum drags the mean
  to ~-27, which asked for a 1e12 rescale before the per-transform clamp.

- **Rhai does not coerce `1` to `1.0`.** Every numeric entry point takes
  `Dynamic` and goes through `num()`, because `t.weight = 1` otherwise
  fails with "function not found" — a wall for exactly the audience this
  is built for. Property setters use `register_get`/`register_set`
  separately, since `register_get_set` ties the setter's type to the
  getter's.

- **Determinism needs fixed-width arithmetic, and the platforms disagree
  about `usize`.** Seeded scripts produced different flames in the browser
  than on desktop. `pick()`, `shuffle()` and `random_palette()` drew via
  `gen_range` over `usize` — 64-bit on desktop, **32-bit on wasm32** — and
  rand dispatches to a different integer implementation per width,
  consuming the stream differently. All draws are over `u64` now. Second
  cause: `mean_log_scale` summed variation weights in `HashMap` order, and
  float addition isn't associative, so the last bits tracked the hasher
  seed (which varies per build and platform) and fed
  `set_contractiveness`. It sums in canonical variation order now.
  `random_stream_is_pinned` locks the stream with golden values —
  **changing it invalidates every script anyone has shared**, so treat a
  failure there as a breaking change rather than a number to update.
  Relevant to Phase 5 too: a Python wheel is 64-bit and would have matched
  desktop, hiding this.

- **The web build had a pre-existing startup crash** (fixed in
  `2e5748c4`, unrelated to scripting): egui rasterizes its font atlas per
  `pixels_per_point`, browsers change the scale factor during startup, and
  egui-wgpu panics if a partial texture update arrives for an image it
  never received in full. Worth knowing for Phase 3 that the WASM target
  had gone untested long enough for this to sit undiscovered since the
  egui 0.34 bump.

- **Measuring nonlinear variations is unsolved and needs the GPU.** The
  metric above covers the affine part only, which is exact for
  affine-only flames and meaningless once a bounding variation like
  `spherical` is in play. Analytic extension would need each variation's
  Jacobian; we have 500+ variations as WGSL source with no derivatives
  and **no CPU evaluator at all**, so that route is closed. The workable
  answer is empirical, and needs only forward evaluation:
    1. *Numerical Lyapunov estimate* — iterate two nearby points through
       the same transform sequence, accumulate `log(δₙ/δ₀)` with
       renormalisation. Standard, handles arbitrary nonlinearity.
    2. *Divergence rate* — count how often the shader's existing
       bad-value respawn fires. Cheaper (one atomic), and a more direct
       answer to the practical question "will this render as mud?".
    3. *Bounding box* of the attractor, which the random generator could
       also use for auto-framing.
  All three want a short **GPU probe dispatch** (small workgroup count,
  a few thousand iterations) returning statistics. Worth building for the
  existing random generator regardless of scripting; scoped for a later
  phase, not Phase 1.

## 8. Open questions (non-blocking, settle during Phase 1/2)

1. Exact API naming pass (`t.translate` vs `t.move_by`, `config.*`
   nesting) — settle when registering, with the shipped examples as the
   style guide.
2. Modifier UX: panel-only, or also a Fractal-menu "Apply Script…"?
3. User script directory location (`assets/scripts/` is shipped
   content; user scripts likely belong next to SystemSettings storage).
4. Script-declared param *animation*: should declared params be
   targetable by the animation system later (a generator re-run per
   frame)? Powerful but expensive — decide at Phase 6, not before.
5. Reroll semantics: seed+1 vs random seed (probably a UI toggle).

## 9. Decisions already settled with the user

- Rhai, Rust-flavored syntax accepted.
- Script-declared parameters: yes (two-phase collect design above).
- Generator/Modifier as two explicit categories.
- Write access: everything in `FractalConfig`.
- Python via native PyO3 wheel, not the WASM binary.
- Structural-load integration; ConfigManager deltas not exposed to
  scripts.
