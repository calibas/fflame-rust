# Flame Scripting: Random Generation Scripts, Python, Web, Decompositions, L-systems

**Status:** Planned — no code yet. This is the implementation plan.

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
| `random_palette()` / `palette(name)` | `PaletteLibrary` |
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

**Feature gating** (needed for the Python wheel, harmless before then):
`gui` feature (default) gates `app/`, `ui/`, winit/egui/wgpu-surface
deps; `script` core (config + scene + variations registry + formats +
script engine) must build with `--no-default-features`. The variations
registry is required headlessly anyway (name/param validation). This
factoring lands in Phase 5 with the wheel; Phases 1–4 don't need it.

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

**Phase 2 — In-app panel.** Random Generator panel section: script
list (shipped + user dir), editor, param sliders from declarations,
seed + Reroll, Run/Apply as one undo point; Modifier entry point on the
current flame. Ship starter scripts in `assets/scripts/`. Stretch:
port/mirror `randomize.rs` presets as example generator scripts
(dogfooding — proves API coverage; the Rust randomizer stays).

**Phase 3 — WASM enablement.** Panel on in the web build; embedded
starter scripts (mirroring the preset embedding); verify budgets on the
browser main thread (chunked/async run if a long script janks).

**Phase 4 — Built-ins.** `kleinian_generators`, Apollonian/sphere
packing, `lsystem`/`turtle`; the decomposition Modifier scripts and 2–3
L-system Generator scripts shipped as content. Math validated against
the packed variations they decompose (render comparison).

**Phase 5 — `pyfflame`.** `gui` feature-gating/factoring, PyO3
bindings, maturin CI, format IO, `run_script`, CLI render wrapper,
PyPI-ready docs.

**Phase 6 — Animation-track generation (deferred by design).** The
object model grows an `anim` handle (tracks/keyframes targeting the
same parameter names), scripts can emit `.anim`; Python gets it via the
same bindings. Deliberately last: the API is designed so this is an
extension, not a rework.

---

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
