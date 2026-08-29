# Escape-Time, Field & Orbit-Trap Fractals

**Status:** Planning — agreed architecture, no code yet. Scheduled for
a post-0.5 branch. This is the working plan.

A family of per-pixel fragment rendering modes (Mandelbrot, Burning
Ship, tetration, Kali, Lyapunov, Weierstrass fields, escape-time IFS,
…) sharing the app's palette, tone mapping, effects, export and
animation machinery. Deep zoom via perturbation theory is a later
phase. Source conversations with the math worked out:

- [docs/experimental/pertubation-theory.md](../experimental/pertubation-theory.md)
  — perturbation, rebasing, BLA, floatexp.
- Claude Heiland-Allen's deep-zoom corpus, reviewed 2026-08 and
  referenced as *[mathr]*:
  [theory & practice](https://mathr.co.uk/blog/2021-05-14_deep_zoom_theory_and_practice.html)
  (glitches, rescaling, abs-formula case analysis),
  [again](https://mathr.co.uk/blog/2022-02-21_deep_zoom_theory_and_practice_again.html)
  (rebasing, BLA), the
  [reference page](https://mathr.co.uk/web/deep-zoom.html)
  (per-formula BLA details, hybrids), and the fixed-point numerics
  posts ([2019](https://mathr.co.uk/blog/2019-09-30_fixed-point_numerics.html),
  [2022](https://mathr.co.uk/blog/2022-11-24_fixed-point_numerics_revisited.html)).
  Upstream: K.I. Martin's
  [sft_maths.pdf](http://www.science.eclipse.co.uk/sft_maths.pdf) and
  Zhuoran's "Another solution to perturbation glitches"
  (fractalforums.org).
- [docs/experimental/escape-time-ifs.md](../experimental/escape-time-ifs.md)
  — the missed-fractal sweep, the tetration name-cluster resolution,
  field-mode fractals, the Hepting–Hart escape buffer, and the
  chaos-game bridges. Referenced throughout as *[ETI]*.

Sibling plan: [flame-deep-zoom.md](flame-deep-zoom.md) — deep zoom
for the *chaos game* via importance sampling. Independent feature,
but §7 there names the two pieces built shared with this plan (the
Lipschitz extension, the deep camera type).

---

## 1. The shape of the feature: one pipeline, three fragment modes

Everything below is "replace the chaos game at the generator stage;
share everything downstream" — but the generator slot has three
distinct shapes, unified by camera, palette, tone mapping and panel:

- **Mode A — iterate & classify** (single-pass): per-pixel orbit of a
  formula; terminate on escape / convergence / detected period; color
  by classification + orbit statistics. The bulk of the catalog.
- **Mode B — field evaluation** (single-pass): no iteration state at
  all — evaluate a finite analytic sum or a finite-orbit statistic
  per pixel. Weierstrass / Besicovitch–Ursell surfaces, Lyapunov
  exponent maps, FTLE. Cheapest mode; f32 trivially sufficient.
- **Mode C — escape-time IFS** (multi-pass): the Hepting–Hart escape
  buffer — an image-space fixed-point iteration over ping-pong
  textures that renders *IFS attractors* (including flame-style ones,
  within limits) as solid, DE-shadeable sets instead of density
  clouds. See §6.

```
[chaos game + accumulate]  OR  [fragment mode A/B/C]   ← the mode switch
              ↓ (same Rgba32Float HDR target)
     tonemap (Linear default for fragment modes; log stays available)
              ↓
     color-effects chain          ← kaleidoscope on a Mandelbrot: free
              ↓
     display / PNG export / thumbnails
```

Zero impact on flame rendering is structural: the only new code on the
flame path is the top-level mode check.

---

## 2. What exists already (and what happens to it)

| thing | what it is | fate |
|---|---|---|
| `julia` color effect | Mandelbrot/Julia overlay as a post-effect (own zoom/pan, HSV, blend modes) | Stays as the "garnish over a flame" tool; HSV coloring superseded by palette coloring in the new mode. Deprecation decided later. |
| `fract_*_wf` variations (6) | Escape-time *inside the chaos game* — seed, iterate, plot by escape count | Untouched; different artistic object. |
| `mandelbrot` variation | Random-walk Buddhabrot in the chaos game | Untouched; see bridge §7.2. |
| `littlewood` variation | Dynamical-space attractor A(λ) of the ±1-series IFS | Untouched. Its parameter-space twin is catalog §5.11; the two cross-check (§8). |
| `julia`/`julian`/`juliascope` variations | Random inverse-branch maps | Already the bridge §7.1 in miniature — IIM without knowing it. Untouched. |
| Effects system | Ping-pong chain, params→UI autogen, `EffectSource::Owned` | Reused downstream and imitated (registry/param-UI shape). Not the host — §3. |
| `docs/projects/new-shaders.md` | WGSL technique cookbook | Coloring reference (IQ palettes, domain warp). |

---

## 3. Decision record

### The fragment renderer is its own pipeline stage, not an effect

Effects are image *transforms*: they consume an input texture, and
their ABI is a 48-float uniform block with no storage buffers. A
fragment fractal renderer *generates*; phase 4 needs a
reference-orbit storage buffer and an arbitrary-precision center that
cannot ride in 48 floats; mode C needs its own ping-pong pair and a
multi-pass loop. The `julia` effect only fits the effect mold by
ignoring its input.

**Rejected: hosting in an effect slot** (ABI mismatch, above).
**Rejected: copying the effects system** (~1400 lines duplicated in
order to diverge immediately). What is actually reused: the palette
and tonemap stages wholesale, the effects chain *downstream*, and the
params→UI/registry *pattern*.

### Layers: not in v1

Escape-over-flame garnish = the existing `julia` effect;
escape-as-subject = the new mode; true layer compositing stays the
separate future feature CLAUDE.md already lists. **Rejected for v1:**
building a compositor as a prerequisite is exactly the bloat this
plan avoids.

### Camera: the mode owns its own; deep zoom forces the shape

Flame view state is f32 — physically incapable of holding a deep-zoom
center. The fragment camera:

- `center_re`, `center_im`: **decimal strings** (parsed by the
  fixed-point module in phase 4; by f64 until then),
- `zoom_log2: f64` — a plain float ConfigPath, so undo, animation
  tracks and scripting work unmodified. A deep-zoom dive is "animate
  the exponent, center fixed" — the orbit cache's cheap case, and the
  exponential track interpolation has exactly the right feel,
- `rotation: f32`.

This representation is deliberately **shared** with the flame
deep-zoom plan ([flame-deep-zoom.md](flame-deep-zoom.md) §7) — a
chaos-game deep-zoom camera hits the identical f32-center wall.
Design the type once, not mode-private.

Viewport input (drag, wheel-to-cursor, pinch) routes by render mode.
**Not animatable in v1:** the center strings (see open questions).

### Formula × iteration-scheme × classification × coloring are orthogonal axes

Ultra Fractal's split, extended per *[ETI]*: the tetration cluster and
the published Mann/Ishikawa literature both collapse into axis
combinations rather than formula lists.

- **FormulaDef registry** mirroring `VariationDef`: static defs,
  inline WGSL step body, param defs (auto-generating panel UI the way
  variation params do), feature flags (`NeedsPrevZ`,
  `NeedsComplexExpLog`, `Convergent`, `NonEscaping`, `AbsFold`,
  `SignChoice`, `NeedsDerivative`) gating what the assembler splices.
- **Iteration-scheme modifiers**, applicable to (nearly) every
  formula, implemented once each:
  - *Damped/Mann iteration*: `z ← (1−α)z + α·f(z)` — two float
    params; the published Mann/Ishikawa/Picard-variant fractal
    families fall out of this one knob *[ETI]*.
  - *Root-finder schemes* over a fixed-point equation: Newton,
    Halley, Schröder, Householder-3, secant (two-register), Chebyshev,
    König — plus a complex relaxation parameter (the generalized
    Newton a-plane, where the good Nova-like galleries live) *[ETI]*.
- **Classification options**: escape (|z|², or Re/|Im| for
  exponential-family), convergence (`|z − z_prev| < ε`), **period
  detection** (short-ring compare / Brent, colored by cycle length),
  and the **biomorph toggle** — Pickover's classify-on-|Re| or |Im|
  individually — as a switch on *every* formula, not a formula
  *[ETI]*.
- **ColoringDef registry** consuming the per-pixel orbit summary
  `(z_final, n, class ∈ {escaped, converged, periodic(k), root(i)},
  trap accumulator, average accumulators, |dz/dc| when enabled)`.
- **Julia mode is a toggle**, not a formula-list entry: c fixed,
  seed = pixel. One flag halves the catalog.

Shaders assemble per (formula, scheme, classification, coloring) on
demand — small WGSL, fast compiles — and the existing **pipeline LRU**
absorbs recompiles when the user flips combinations.

### Arbitrary precision: our own fixed-point, no new dependencies

The reference orbit lives in a bounded box (|z| ≤ 2 until escape), so
the CPU side needs **fixed-point**, not arbitrary-precision floating
point: `[u64; K]` limbs, a few integer bits of headroom (intermediates
like x²+y² reach 8 before subtraction), implied binary point. That
deletes MPFR's hard parts — no exponents, no normalization, no
rounding modes. Surface: add, sub, schoolbook mul (u64×u64→u128;
Karatsuba past ~30 limbs), shifts, decimal round-trip, f64
conversion, floatexp export, complex wrapper. ~600–1000 lines with
tests.

**On "everyone uses floating point for this"** — the deep-zoom
literature computes references in arbitrary-precision *floats* because
it builds on existing libraries (MPFR, custom floatexp), not because
the math wants an exponent: for polynomial formulas the exponent never
moves, and every mantissa operation inside an MPFR-style float IS
integer limb arithmetic plus the normalization/rounding we delete.
*[mathr]*'s own numerics posts say it directly — escape-time z values
sit near magnitude 1, "a good fit for high-precision fixed-point"
without "the more-complex algorithms of floating-point numerics" — and
Fractal eXtreme, the speed benchmark of the pre-perturbation era, was
fixed-point bignum. Floats genuinely belong in exactly four places,
all outside the core: the per-pixel delta iteration (hardware
f32/f64 — the whole point of perturbation), the floatexp export
boundary, the derivative orbit, and the Newton wrapper (below).

Design points from review (second-opinion pass + *[mathr]*):

- **The bounded box is a per-formula property, not a global
  assumption.** It holds for the polynomial families (Mandelbrot,
  Multibrot, Tricorn, Ship). It does NOT hold for the **derivative
  orbit** dZ/dc — distance estimation's companion sequence grows
  without bound and must be floatexp, computed alongside from the
  stored orbit, never inside the fixed-point core. Transcendental/
  tetration formulas have unbounded Z itself (already "perturbation:
  none" in the catalog); Magnet is bounded-ish but divides, so it is
  excluded from deep zoom initially. `FormulaDef` carries a
  reference-number-system property.
- **One guard limb** below target precision. Truncating multiplies
  drift over a 10⁶–10⁸-iteration orbit; an extra u64 costs ~13% at 8
  limbs and makes bit-identity mean identical *and correct* trailing
  bits. It also absorbs the error of **truncated multiplication**
  (compute only the high half: n(n+1)/2 limb-muls vs n² — *[mathr]*
  2022), which is the cheap 2× at schoolbook sizes; revisit at
  Karatsuba sizes.
- **Complex squaring is two big muls, not three**: Re = (x+y)(x−y),
  Im = 2xy (the 2 is a shift). The big-mul is the entire runtime, so
  this is a free 33%. The two muls are independent — and that is the
  only parallelism there is: orbits are inherently sequential, so
  parallelize within an iteration or across separate references,
  never across iterations.
- **Write the limb routines exponent-agnostic** (operate on slices,
  shift amounts as parameters). Newton nucleus-finding (phase 5)
  needs dynamic range a fixed binary point cannot hold — the step
  divides by the unbounded derivative — so it gets a thin big-float
  wrapper: limb array + one i64 exponent + normalize-on-demand,
  reusing the same cores, ~200 lines, still no rounding modes. The
  core itself still never divides.
- **The floatexp export is where "no normalization" could quietly
  cost correctness**: fixed→floatexp is a leading-zero count + shift,
  and near-zero orbit values are precisely where rebasing and glitch
  behavior live — 2Zₙ at tiny |Zₙ| must convert exactly. lzcnt-based
  export is a first-class, tested operation, not an afterthought.

Throughput per 1M-iteration orbit: ~30 ms at 1e-30, ~0.3 s at 1e-100,
~2–3 s at 1e-300, ~10 s at 1e-600 (Karatsuba). MPFR's FFT wins only
past several thousand bits (zoom 1e-1000+), out of scope for a
browser-first app; Toom-3 is an internal upgrade if ever needed.

**wasm asterisk on those timings**: u64×u64→u128 lowers to
compiler-rt calls over 32-bit halves on wasm32 — deterministic (the
bit-identity invariant survives) but ~3–5× slower, so 1e-300 is ~10 s
in-browser. Acceptable with the planned progressive upload; the
natural upgrade is **server-side reference computation** — references
are small (period-length × 16 B as floatexp), cacheable by location
hash, exactly what a gallery backend should precompute. **Rejected: a
32-bit-limb wasm build** — faster, but truncation positions move and
bit-identity between targets dies, failing this plan's own premise.

Why owning beats the libraries:

- **Cross-platform bit-identity is structural** — integer limbs are
  identical on x86/ARM/wasm; a saved deep-zoom location must
  reproduce exactly on desktop and web. Same reasoning that pinned
  the script RNG.
- **The GPU half is hand-rolled regardless** (WGSL has no f64); one
  numeric story with matched semantics at the upload boundary.
- **Rejected: `rug`/MPFR** — C build chain, wasm-hostile, LGPL care,
  ~1% surface use. **Rejected: `astro-float`/`dashu`** — a general
  float tower imported as audit surface, 2–5× slower than MPFR
  anyway.

### Config, serialization, contract

- `EscapeConfig` in `FractalConfig`, skip-if-default — existing
  `.fflame`s stay byte-stable.
- `render_mode` gains a variant (wire form `"escape"`). **Verified at
  implementation: the engine contract does NOT carry render modes, so
  the shape fingerprint does not move and nothing forces the API to
  notice.** That makes the coordination item fully manual — the
  server's Postgres `render_mode` enum and `openapi.json` must be
  updated by hand, and until then the client-side Save Online guard is
  the only thing standing between an escape config and a server 500.
- `.flame` XML: not written, not read (no Apo/JWF equivalent);
  `.fflame`-only, same policy as depth-density compensation.
- Scripting: fragment params are ConfigPaths → `config.set` works day
  one; a richer script API later.

### UI

One new dockable panel, **Escape Fractal** (working name — see open
questions): formula picker, Julia toggle + c picker, iteration-scheme
and classification controls, per-formula params (auto-generated),
coloring picker + params, trap editor, iteration budget, camera
readout. Compact mode: in the Window submenu. Colors/Tone
Mapping/Palette panels apply to both engines as-is; the View panel
stays flame-only.

Progressive rendering v1: whole-frame re-render per change (direct
f32 is fast). The iteration governor's median-filtered shape is
available if a heavyweight combination ever needs adaptive `max_iter`
— wire only on demonstrated need.

### Integration map — the wiring, point by point

Reviewed against the code before branching (2026-08). Each reuse
target with its actual mechanism and what this feature must add.
The requirements this satisfies: parallel to flames, no flame-UI
complication, no flame-performance cost, undo + coalescing,
animations, scripts, effects, palettes.

**Undo / ConfigManager / coalescing.** All escape parameters flow
through `config_manager.update_param(path, value)` like everything
else, which makes undo, redo, 500 ms/3 s coalescing and the history
panel automatic. What must be added, in `src/config/delta.rs`:

- `ConfigPath` variants: the scalar set (`EscapeZoomLog2`,
  `EscapeRotation`, `EscapeMaxIter`, `EscapeBailout`, `EscapeJulia*`,
  scheme/classification/coloring selectors) plus **two keyed
  variants** mirroring `DensityEffectParam { index, param }`:
  `EscapeFormulaParam { param: String }` and
  `EscapeColoringParam { param: String }` — per-formula and
  per-coloring params stay open-ended without new enum arms per
  formula.
- The center strings are `ConfigValue::String` (already exists) —
  undoable day one, not animatable (by design, see open questions).
- `from_string_key`/`to_string_key` arms — this single addition is
  what makes **animation tracks, scripting (`config.set`) and
  signal-driven audio reactivity all work unmodified**: anim targets
  resolve through `ConfigPath::from_string_key`, and signals drive
  anim tracks, never parameters directly.
- `describe()` arms + `locales/*.yml` keys, so history entries read
  as words.

**GPU routing.** `update_type()` today speaks flame
(`IterationReset` clears the accumulator). Escape needs one new
answer: `UpdateType::EscapeRerender` → a `rerender_escape` flag on
`UpdateAction` (`src/config/manager.rs`), drained where the app
already drains the others (`src/app/gpu_updates.rs::
apply_pending_updates`). Camera-only changes can re-render without
pipeline rebuilds; formula/scheme/classification changes go through
the pipeline LRU. Escape paths return `EscapeRerender`, flame paths
are untouched — **a flame session never sees a new code path**.

**Parallel renderer, zero flame cost.** `App` gains
`escape_renderer: Option<EscapeRenderer>` beside `flame_renderer`,
built lazily on the first escape-mode frame and destroyed on mode
exit (WebGPU discipline: explicit `destroy()`, the lesson the
gallery module keeps re-learning). The frame loop branches once, at
the same point that currently calls `renderer.compute_pass(...)`
(`src/app/mod.rs`); the governor, batching and overwrite-mode logic
stay inside the flame branch untouched.

**Everything downstream inherits through the unified render path.**
`render_thumbnail_async` → `render()` → `render_with`
(`src/renderer/render.rs`), and the CLI `export` command, the visual
suite, animation/video export and the gallery's `wasm/render` module
all go through the same two functions. The escape dispatch therefore
lives **inside `render_with`** (branch on `config.flame.render_mode`
before the chaos-game loop), not in the app: thumbnails, headless
export at any resolution, video export, browser-card previews and
gallery tiles all get escape rendering with no further wiring. The
tail of `render_with` (tonemap → density/color effect chains → pixel
readback) is shared verbatim — which is the effects and palette
story too: escape writes the same `Rgba32Float` accumulation-shaped
target the tonemap consumes, and colorings sample the same palette
texture (reuse the existing palette→texture upload, don't re-derive
it).

**Save Online / API.** `sync.rs` converts `RenderMode` with
*exhaustive* `From` impls — adding the variant is a compile error at
both conversion sites, which is the forcing function working. But
the server casts the wire string into a **Postgres enum**, so until
the API adds the value, uploading an escape config would 500.
Phase 1 therefore ships a client-side guard: Save Online for an
escape config is disabled with a "not yet supported online" tooltip
until the API's enum + `openapi.json` are updated (that coordination
is already a §9 item; the guard makes the interim graceful instead
of a server error).

**Scripts.** `config.set`/`config.get` reach every ConfigPath via
the string keys above, so escape scripting works the day the paths
exist (`script("…", "generator")` + `config.set("escape.formula",
"mandelbrot")` — exact key spelling decided with the paths). The
richer typed API (`escape.` handle in Rhai) stays deferred as
planned. Gallery consequence, for free: `wasm/script` produces
configs and `wasm/render` renders them through `render_with`, so
escape tiles in the Endless Gallery need only the reserved-stem and
corpus updates any new script needs.

**What stays flame-only, deliberately**: the Transforms/Triangle/
Xaos/Variations panels (never shown in escape mode — the new panel
is the whole editing surface), the View panel, fly mode, the
governor, sticky shaders, the probe/census tooling, and `.flame`
XML. The mode check hides flame-only panels rather than teaching
them a second vocabulary.

---

## 4. Phases

**Phase 1 — mode A core (ships standalone value).**
Pipeline stage + mode switch; EscapeConfig + serialization; the
integration map's wiring (ConfigPaths + string keys + describe/i18n,
`UpdateType::EscapeRerender` + `UpdateAction` flag, dispatch inside
`render_with`, lazy `escape_renderer` in App, Save Online guard);
camera + input routing; panel; FormulaDef/ColoringDef registries +
assembler; formulas: Mandelbrot/Multibrot, Tricorn/Multicorn, Burning
Ship family, McMullen, Kaliset (+ Julia toggle everywhere); biomorph
toggle; colorings: escape count, smooth iteration, basic orbit traps,
orbit average (Kaliset needs it); palette mapping; Linear tonemap
default; PNG export (thumbnails/CLI/video inherit via `render_with`);
visual corpus; formula × coloring compile probe.

**Phase 1 status (2026-08-25, branch `escape-time`).** Implemented
and CLI-verified: EscapeConfig + skip-if-default serialization;
ConfigPaths/string keys/describe/i18n; `UpdateType::EscapeRerender` →
`UpdateAction::rerender_escape`; `RenderMode::Escape` + the Save
Online client-side refusal; FormulaDef/ColoringDef registries with
feature flags (`NonEscaping`, `SeedFromPixel`, `NeedsOrbitAccum`,
`ColorsInterior`); marker-splicing assembler (bailout compiles out for
NonEscaping, accumulators compile in per coloring) + naga
parse/validate over every formula × coloring (the compile half of the
probe) + the fast-math lints extended over the assembled WGSL;
EscapeRenderer (one dispatch, Rgba32Float in accumulator format,
pipeline cache, explicit destroy) feeding the shared
tonemap/effects/readback tail via `tonemap_pass_with_input`; dispatch
inside `render_with` (CLI export, thumbnails, video, gallery inherit);
lazy `escape_renderer` in the App frame loop, event-driven (no
progressive refinement); Escape Fractal panel (registry-generated
param sliders) + Window/compact menus + View-row mode switch; Linear
tonemap defaulted on mode entry; flame-only panels hidden in escape
mode; viewport input (drag pan, wheel zoom-to-cursor, pinch) in the
center-strings/zoom_log2 vocabulary; formulas Mandelbrot, Multibrot,
Tricorn/Multicorn, Burning Ship family (6 variants), McMullen,
Kaliset; colorings escape count, smooth, orbit trap (point/axes/
circle), orbit average; biomorph toggle (runtime flag); Julia toggle;
12-config visual corpus (`tests/visual/configs/escape/`). Scripting
works via `.fflame` field names (`config.set("escape.zoom_log2", …)`).

Closed since: keyboard arrows/+/- routed (drag semantics through the
shared pan entry); viewport-size transparent export re-tonemaps from
the escape output; the WASM in-app custom-size export grew an escape
branch; escape configs are kept off the flame-only HighResExporter
(the `long_render` heuristic reads flame iteration counts); the GPU
render half of the probe runs every formula × coloring on a device
(`app_repro_test.rs`, ignored). Entering escape mode also resets
exposure/gamma to config defaults alongside the Linear tonemap —
flame presets carry Logarithmic-calibrated values (exposure 0.016)
that render Linear output at ~1e-5 brightness (the all-black-viewport
bug).

Still open within phase 1: alt-drag look routing (writes flame camera
paths, invisible in escape mode); escape-native high-res tiling —
custom-size exports whose histogram exceeds one storage binding are
refused with a toast (the size limit is the FLAME renderer's
histogram; escape itself needs no histogram, so a lean tiled path is
straightforward when wanted); per-mode tonemap state (the mode switch
currently adjusts shared exposure/gamma, one undo point);
orbit-average look tuning (Kaliset renders correct structure but
wants weighted-falloff glow — phase-2 coloring depth); Burning Ship
variant conventions pinned only by our corpus, not yet against
external reference images.

**Phase 2 — catalog breadth and coloring depth.**
Phoenix, Lambda, Newton/Nova + the root-finder scheme axis, Magnet,
exponential family, the tetration family (§5.8), Feather, Collatz,
Novaretti, Fractint legacy set, Littlewood parameter space, Ducks; damped/Mann
iteration modifier; stripe-average, triangle-inequality, distance
estimation (derivative orbit), interior/period coloring.

**Phase 2 status (2026-08-25, branch `escape-time`): COMPLETE.**
Everything on the list shipped and is corpus-pinned: Phoenix, Lambda,
Newton/Nova with the scheme axis (Newton/Halley/Chebyshev + complex
relaxation), Magnet I/II, exponential family (e^z, sin, cos — with
the per-formula escape-metric slot: Re for exp/tetration, |Im| for
trig/Collatz), tetration, Feather (MandelBrowser code-form
convention), Collatz, Novaretti, the Fractint slice (Spider via
MutatesC, Manowar via per-formula prev-init, Barnsley M1–M3, Cactus),
Littlewood (greedy SignChoice over three digit sets), Ducks;
damped/Mann complex-α modifier (compile-gated); stripe-average,
triangle-inequality, distance estimation (derivative orbit for
Mandelbrot/Multibrot/Lambda), interior/period (Brent detection,
compiled per coloring). Colorings read one `OrbitSummary` struct.

Deferred with notes at their sites: Fractint Frothy Basin /
Volterra–Lotka / Unity (need source consultation);
Schröder/Householder-3/König/secant schemes; Novaretti's second
critical orbit; the Littlewood-variation cross-check test; Ducks and
orbit-average falloff look-tuning; derivative bodies for abs-fold and
anti-holomorphic families.

**Phase 3 — mode B, field evaluation.**
Weierstrass / Besicovitch–Ursell / Bagula lacunary-sum fields (one
FormulaDef, generator + sequence presets; analytic gradient →
hillshade normals for free); Markus–Lyapunov (unparked per *[ETI]* —
"cheaper than Magnet"; signed scalar → diverging palette mapping);
FTLE / standard-map stability as the generalization. Mode B may land
interleaved with phase 2 — it is the cheapest of the three modes.

**Phase 3 status (2026-08-26, branch `escape-time`): SHIPPED.**
`src/escape/fields.rs`: a `FieldDef` registry parallel to the
formula registry (append-only, inline WGSL, name-disjointness
test-pinned) with its own template (`FIELD_TEMPLATE` — no
classification, no escape test; a fixed-count loop over eight floats
of per-pixel state accumulating value + analytic gradient;
`max_iter` is the term count). Three fields: `weierstrass` — the 2D
Besicovitch–Ursell product form Σ aⁿ·g(bⁿx+φ)·g(bⁿy+φ) with
cos/sin/triangle generators (Bagula's published double-x is a 1D
field in disguise; we implement the evident intent) and the
term-by-term analytic gradient; `markus_lyapunov` — logistic λ over
the (r_A, r_B) plane, bit-pattern forcing sequence, warmup
transient, in-loop normalization; `standard_map_ftle` — Chirikov
tangent-map log growth with per-step renormalization. Three field
colorings: value bands, diverging (atan squash, zero at
mid-palette — the signed-scalar convention), analytic-gradient
hillshade (azimuth/elevation/relief). Routing is by which registry
resolves `EscapeConfig::formula` — the renderer's pipeline/params
paths branch there, the panel shows the field group in the same
dropdowns, fields never enter the perturbation gate (test-pinned),
and a stored mode-A coloring name falls back to the field's declared
default rather than mis-resolving. Whole matrix naga-validated +
fast-math-linted; corpus: weierstrass-hillshade, markus-lyapunov-ab,
standard-map-ftle.

**Phase 4 — perturbation (deep zoom, mode A).**
The `fixedpoint` module; CPU reference orbit on a worker with
progressive upload; orbit cache keyed on (center strings, precision,
maxiter), append-on-deepen; scaled-f32 deltas; floatexp WGSL type;
Zhuoran rebasing; ladder direct → scaled-f32 → floatexp. Per-formula
tiers as cataloged. Math: pertubation-theory.md; mechanics *[mathr]*:

- **Rebasing, exactly**: when |Z+z| < |z|, set z ← Z+z and reset the
  reference index to 0 (generally: jump to whichever orbit minimizes
  |(Z−Z_o)+z| among the current reference and each critical orbit at
  iteration 0). This *replaces* glitch detection — Pauldelbrot's
  criterion |Z+z|² < G·|Z|² needs a G ∈ [1e-8, 1e-2] nobody knows how
  to choose, and rebasing makes the choice moot: "avoided rather than
  detected", efficiency and correctness at once.
- **Rescaling, concretely**: iterate w with z = S·w:
  `w ← 2Zw + Sw² + d`, re-deriving S every few hundred iterations.
  Hoist the underflow tests to rescale time (skip Sw² when S
  underflowed and Z is not small; skip +d when it underflowed) rather
  than paying them per iteration. When |Z| itself underflows the
  scaled form breaks: do one full-range floatexp iteration, then
  rescale — the reference passing near zero is the case, and it is
  also exactly where the lzcnt export path (§3) must be exact.
- **BLA is deferred, and that is a decision, not an omission.**
  Perturbation + rebasing alone give *correct* images at O(iterations)
  per pixel; BLA is the iteration-skipping accelerator on top, worth
  it when max_iter runs to millions. When it lands (phase 5): the
  O(2M) binary-doubling table (M single-step BLAs merged pairwise;
  start at iteration 1 — iteration 0 has radius 0; merge:
  `A ← A_y·A_x, B ← A_y·B_x + B_y,
  r ← min(r_x, max(0, (r_y − |B_x|·max|c|)/|A_x|))`), validity
  `|z| < ε|Z| − max|c|/(|2Z|+1)`-shaped per formula, real 2×2
  matrices for the nonconformal family (§5.2). *[mathr]* notes
  Zhuoran considers this construction suboptimal — check the forum
  thread before building. Series approximation stays rejected
  (fold-sensitive, poorly-understood stopping, abs formulas barely
  skip).
- **Hybrids/multiple critical points** (phase 5 with the hybrid
  loops): one reference orbit and BLA table per phase of the loop and
  per critical point — a (M,BS,M,M) loop carries four — with rebasing
  selecting the nearest orbit.

**Phase 4 status (2026-08-25, branch `escape-time`): core shipped.**
The `fixedpoint` module is exactly the planned design — sign-magnitude
u64 limbs, 8 integer bits of headroom, exponent-agnostic slice cores,
truncated high-window multiply with the guard limb, two-big-mul
complex squaring, exact-decimal parse (the config's center strings go
straight to full precision), and the lzcnt floatexp export tested
exact at near-zero. Reference orbits iterate in fixed-point with
append-on-deepen (bit-identical to fresh compute) behind the
(center, precision, julia-c)-keyed single-slot cache. The GPU delta
pipeline iterates scaled-f32 deltas (w = δ/S, S = pixel spacing) with
Zhuoran rebasing generalized to `δ ← z_full − Z₀` (zero-start
references reduce to the textbook form; Julia references start at
Z₀ = center and NEED the subtraction — found by the agreement test).
Every coloring works under perturbation unchanged: the loop
reconstructs z = Zₘ + δ for the rebase test, which is exactly the
OrbitSummary colorings consume. Gate: Mandelbrot (parameter and
Julia planes), undamped, no biomorph, zoom_log2 > 18; the fixed-scale
f32 ladder holds to ~zoom 54 (w² overflow) and the UI clamp is 45.
Verified: seamless direct↔perturbed threshold crossings on both
planes, block-mean agreement tests (0/768 param, 3/768 julia), and
crisp structure at zoom 42 (~4·10¹²) on a 38-digit seahorse center.

Closed since: the switch moved to zoom 14 (field-observed direct
pixelation by 16 — the center's f32 ulp binds before pixel spacing
does); the floatexp rung shipped (shared-exponent complex floatexp —
vec2 f32 mantissa + one i32 exponent via frexp — past zoom 48, pixel
spacing delivered symbolically so nothing ever underflows; UI clamp
raised to 300); Multibrot joined via assemble-time binomial codegen
(p = 2 emits the hand-written step byte-for-byte; integer powers
2–12, fractional powers honestly direct); reference orbits moved to
a worker thread with chunked append-only publication and
newest-request-wins preemption — deep frames render with whatever
prefix has landed (rebasing makes short orbits an early wrap, so
partial frames refine rather than being wrong), CLI/export keeps the
blocking deterministic path; and the plain Burning Ship variant
joined via diffabs (exact three-branch case analysis, homogeneity
carrying it into S units). Agreement tests cover param/Julia ×
scaled/floatexp × Mandelbrot/Multibrot-3/Ship: 0/768 structural
blocks everywhere (3/768 on one Julia filigree case).

Newton nucleus-finding SHIPPED (the phase-5 backlog item, pulled
forward): ball-method period detection (center orbit in fixed-point,
radius in extended-range floats) + Newton on f_c^p(0) = 0 in the
big-float wrapper (limbs + one saturating i64 exponent,
division-by-reciprocal only — verified against the period-2 nucleus
at exactly −1 and the period-3 antenna constant to 19 digits).
Parameter-plane Mandelbrot references relocate to the governing
nucleus on both the blocking and worker paths: Z_period = 0 makes
the wrap-rebase exact (the fix for wrap-laundering banding), the
orbit array is period-length, and the pipeline carries the
(view − nucleus) offset as a pixel-unit uniform. Search failures
fall back to the plain reference. The panel gains "Center on
minibrot" — Newton navigation writing exact-precision center digits
as one undo point.

Chunked perturbed dispatches (field-driven): a single unbounded
dispatch at high max_iter tripped Windows' TDR watchdog (driver
reset, process abort) — perturbed renders now run bounded
iteration windows with 48 B/px resume state, adaptive to pixel
count and rung cost, refining on screen chunk by chunk; verified by
forcing 64-iteration chunks through every agreement scenario.

The whole Ship family now rides both perturbation rungs: each of the
six fold variants gets its own delta algebra (re plain or
diffabs-folded, im from the variant's sign/abs arrangement of the
cross term), the fixed-point reference builds every variant from the
plain square's parts by sign-magnitude flips (verified per-variant
against an f64 loop), and the floatexp rung runs the same algebra on
extended-range scalars — a per-component SFe type whose sfe_diffabs
does the exact three-branch analysis against the f32 reference
components, closing the deferred "floatexp diffabs" item (the
deep-needle full-range concern dissolves: every quantity in that
step is full-range by construction). Agreement sweep: ship v0–v5
scaled + ship floatexp, all within tolerance (worst 4/768 boundary
blocks). Multibrot nucleus math shipped with batch N1 (ball-method
majorant generalized to z^p + c, Newton with the p-power derivative
chain), and the minibrot button became an async background search.

BLA iteration skipping SHIPPED (Zhuoran's improved construction as
described by Claude Heiland-Allen — mathr.co.uk/web/deep-zoom.html
and the 2022-02-21 deep-zoom-theory post): the CPU builds an O(2M)
binary-merge table from the reference orbit in extended-range floats
(single steps linearize z → z^p + c; each level merges adjacent
pairs, radii composed so an entry is valid exactly when both halves
would have been, |δc| bounded by the viewport half-diagonal plus the
nucleus offset), truncated at the reference's own escape so a skip
can never overshoot a pixel's escape iteration past the covered
prefix. Both perturbed rungs walk the level table per iteration
(binding 7; a zeroed dummy disables it with no pipeline
permutation): from an aligned reference index the longest valid
2^ℓ-step run collapses to one affine δ' = A·δ + B·δc — extended
range on the floatexp rung, clamped-exponent collapse on the scaled
rung. Holomorphic tiers only (the Ship family's abs-folds are not
linear in δ across a fold-sign change), and colorings that
accumulate per iteration keep the per-step path. Agreement: BLA
on-vs-off at zoom 40 (scaled), zoom 60 (floatexp) and Julia all
0/768 blocks, and the shallow direct-vs-perturbed checks now run
their perturbed arm with skips active.

Orbit persistence SHIPPED (desktop): a disk store under the app data
dir memoizes reference orbits exactly — files are content-addressed
on the full request identity (center strings, limb count, plane,
power, Ship variant) and carry the orbit PLUS the live fixed-point
state, so a reloaded orbit deepens with extend() bit-identically to
the original (pinned by a resume-vs-fresh-compute test). Cost-gated
(len·limbs² threshold; deep nucleus orbits always qualify), rewrite
only when deeper than the stored file (12-byte staleness probe),
rename-over writes, newest-first eviction to 24 files / 256 MB.
Nucleus-relocated orbits record the (zoom, height) their pixel-unit
offset was measured at and only serve an exactly matching view — the
bookmark case; offset-free orbits serve any view at their precision.
Hooked into both the blocking cache and the worker (load before
compute, save on completion).

The JFA distance-field bridge (§7.3) SHIPPED as the
`distance_field` color effect: the rendered attractor (any flame —
no invertibility required) seeds an rg32float coordinate field,
~log2(max dim) jump-flood passes propagate nearest-seed coordinates,
and a composite pass shades the distance — glow (nearest-seed color,
exponential falloff), contour bands, or nearest-color fill. It
registers as an ordinary color effect (config, UI, animation,
undo/redo all free); the chain runner special-cases its execution
into the multi-pass pipeline, with one params slot per pass and all
field reads via textureLoad (rg32float is unfilterable; also the
WASM-safe idiom).

In-app polish (field-reported, 2026-08-26): mouse pan and
zoom-to-cursor now accumulate the center in FIXED-POINT
(`FixedPoint::decimal_add_f64`) — the old f64 round-trip capped the
step at the center's own ulp, so past ~zoom 45 horizontal pans
"skipped" while a small-imaginary axis still moved (the reported
symptom, pinned by a unit test whose f64 control drops the step
entirely). Supersampling shipped as `EscapeConfig::supersample`
(1–3×; renders at N× per axis into the internal texture and
box-downsamples pre-tonemap — config-level, so files reproduce
identically in-app, CLI and thumbnails; the whole perturbation
pipeline inherits consistency because the render resolution IS the
internal resolution). Density Levels is hard-off in escape mode
(it remaps the chaos game's measured density statistic — stale
flame data here, and escape density is a constant 1/px), with the
tonemap panel saying so. The animation system's target picker
gained an Escape category (zoom, rotation, iterations, bailout,
Julia seed, damping, and the ACTIVE formula's/coloring's parameters
— mode B fields included); the apply path worked all along via the
Escape.* string keys, and selectors/center strings stay
deliberately non-animatable.

Second field round (2026-08-26): nucleus relocation offsets now
carry their PROVENANCE view (`off_zoom_log2`/`off_height_px` on
ReferenceOrbit and OrbitProgress) and every consumer rescales to its
own view via `rescale_offset` (pixel units scale with
S = 2^(2−zoom)/height). The missing rescale was one root with two
faces: dragging the zoom VALUE panned the render toward the
governing nucleus (same center string → cached orbit reused → stale
pixel units; wheel zoom masked it by rewriting the center every
notch), and toggling supersampling shifted the view (internal
height changed under the same offset). Relocations a view can't
rescale to (far zoom-out) retire the orbit — blocking cache and
worker reuse both guard. Also two AA-exposed capacity hazards:
the chunk floor dropped 256 → 16 iterations (256 × tens of
supersampled megapixels was a multi-second dispatch — the TDR
crash class), and resize clamps the render pixel count to 32 Mpx
(48 B/px iteration state; unbounded supersample × display was a
device OOM abort).

Zoom-limit audit + accuracy verification (2026-08-26, prompted by a
user session at zoom_log2 = 50000 on the Misiurewicz point c = i):
the design ceiling is the floatexp rung's i32 exponent arithmetic
(≈2^31 — zoom_log2 ~10^9); the interaction paths now reach 1e8 (the
old wheel/keyboard clamp of 300 was the phase-1 travel ceiling and
COLLAPSED deep sessions; pan/anchor deltas are symbolic
mantissa·2^exponent — plain f64 dies at ~1060). The practical walls
arrive first: reference-orbit CPU time (limbs ≈ zoom/64; ~783 limbs
and ~4 min for a 500k-iteration orbit at 50000) and max_iter, which
grows ~linearly with zoom at boundary points (escape from a
2^-zoom-sized δ₀ takes ~zoom doublings). Accuracy verified by Tan
Lei self-similarity at c = i (multiplier λ = 4(1+i), |λ| = 2^2.5,
arg 45°): the zoom-50000 render matches the zoom-50002.5 render
under the predicted 45° rotation — near-identical visually, and
edge-map correlation ranks the λ-rotated frame ~3× above both a
half-period control and the wrong-sign rotation. Precision noise
cannot reproduce the correct similarity with the correct rotation
direction; the imagery at these depths is real. (Coloring
ergonomics at depth — smooth values crowding one palette segment
until `palette_squeeze` ≳ 3 spreads them — noted as a later
usability item, per the field report.)

WASM escape SHIPPED (2026-08-26) — without the deferred
SharedArrayBuffer worker, which turned out unnecessary for
correctness: the single-threaded browser build TIME-SLICES the
fixed-point reference per frame (`OrbitCache::get_budgeted` /
`ensure_orbit_with` — budget shrinks with limb count to keep a slice
in tens of milliseconds), and rebasing renders partial-orbit frames
correctly (early wrap), so deep zooms refine progressively exactly
like the desktop worker, minus parallelism. The budgeted path skips
the nucleus Newton search (a blocking run has no place on a UI
thread; plain references + rebasing serve). Supersampling's
render-pixel cap now also respects the DEVICE's storage-binding and
texture limits (browsers grant far less than desktop adapters).
Verified in a real Chrome WebGPU session driving the wasm bundle:
direct (mandelbrot), perturbed (zoom 30), mode B (weierstrass
hillshade) and a zoom-5000 sliced-orbit render all produce correct
imagery; the WASM visual-regression harness now includes the escape
corpus category. Still wasm-absent, deliberately: orbit persistence
(no filesystem; IndexedDB is async-only) and the worker thread
(now a pure parallelism upgrade, still gated on COOP/COEP hosting).

Deep-location hardening (2026-08-26, field report at zoom ~484 on a
3757-digit curated center): pan now preserves the DEEPER of (what
the zoom needs, what the center already carries) — the
zoom-proportional reformat would have truncated a curated location
to ~170 digits on the first pan, silently capping its depth (test:
a 400-digit center pans at zoom 20 with reformat error below its
last digit). BLA now builds at ANY depth: the |δc| bound rides log
space into an extended-range MagFe (the old f64 bound underflowed
past ~zoom 1000 and disabled BLA exactly where multi-million-
iteration renders need skips most); the GPU agreement harness gained
BLA on/off cases at zoom 484 and zoom 1000 on the field location —
both 0/768. Iteration ceiling raised to 100M (u32 carries 4.29G;
chunked dispatches and BLA make it practical). The reported
"glitchy coloring" at ~zoom 484 was investigated to ground truth on
the field location: iteration-ceiling-independent, ~3× smoothed by
3× supersampling, and POSITION-LOCKED under a half-pixel view
jitter (speckle-residual correlation 0.94 — sensitivity-amplified
precision noise would decorrelate). Verdict: real, deterministic
sub-pixel band dust around the target minibrot — correctly
rendered unresolved structure, not an artifact. Remedies are
supersampling (shipped) and coloring-scale/squeeze; the "glitchy
panning" half was the digit-truncation footgun fixed above.

Deep-dive reference anchoring (2026-08-26, field report: an
f3-imported location collapsing to a uniform frame past zoom ~684):
reference precision now honors the CENTER'S OWN DIGITS
(`limbs_for_view` — cache, budgeted path, worker request and
nucleus fallback all anchor there). Truncating a curated deep
center to zoom-proportional limbs made the reference a shallow dust
point whose orbit escapes early; pixels whose deltas cannot outgrow
d0 before that escape get stolen by the reference's own escape —
past a razor cliff (z680→z685 on the field location) the entire
frame rides the reference and renders one uniform value. Diagnosed
by exact fixed-point ground truth plus a bit-faithful CPU replica
of the shader's CFe arithmetic; the replica confirms the fix
(deltas grow to O(1) and escape individually once the reference is
the 197-limb center). Also: relocation offsets are capped at 2^15
px (the f32 d0 sum quantizes pixels past that — defensive; not the
collapse's cause), and `ESCAPE_DISABLE_NUCLEUS` /
`ESCAPE_DISABLE_BLA` env hooks ship as diagnostics.

HONEST LIMITS, measured on the field location: the fix restores
real structure through ~zoom 700; deeper (z900+) the f32-mantissa
delta model itself stalls at THIS location class (near-nucleus
dives whose dynamics repeatedly crush δ back to d0 scale — each
crush truncates pixel history to f32/26-octave resolution, so
represented orbits drift from truth; visible as reference-dependent
fine structure at z543 and interior-stall at z900+). The two known
remedies, in order: import f3's `reference.period` hint and build
the PERIODIC deep-nucleus reference (orbit = one period, crushes
aligned with the reference — the reason fraktaler-3 ships the
field); and double-f32 ("DF") mantissas for the CFe rung (~2^-48
relative). Both are scoped follow-ups, not band-aids.

Periodic references + PROGRESSIVE period detection (2026-08-26/27):
three layers. (1) `EscapeConfig::reference_period` — f3's
`reference.period` as a verified hint (the center's orbit must close
at that period below the view's pixel scale, or it falls back with a
warning). (2) A panel Detect button running ball-method detection at
the center's intrinsic depth on a background thread
(`detect_center_period`). (3) The automation: ball-method period
candidates are exactly the indices where the center orbit's |Z|
reaches a new minimum — and the reference worker is ALREADY
iterating that orbit, so `extend()` tracks the running minimum as an
OCTAVE (extended-range: f64 magnitudes underflow at 2^-537, and
intermediate cascade passes go far deeper) and the orbit becomes its
own periodic reference the moment |Z_p| drops 16+ octaves below the
view's pixel scale — truncate to one period, stop extending, zero
extra compute. Closure validity is ZOOM-RELATIVE
(`closure_limit_for_zoom`, `periodic_serves`): zooming deeper
tightens the limit, shallow closures retire, and detection
rediscovers the deeper period — measured live on the f3 location:
period 142,232 (|Z| ~ 2^-921) discovered in 7 s serving z700 (which
now renders full structure), retired at z1100 and re-detected at the
cascade's doubling, period 284,432 (|Z| ~ 2^-1379), in 14 s. Orbit
files carry the closure octave (format FFORBIT2). The hint layer's
remaining role is pre-seeding the FULL deep orbit ahead of a dive;
at any given zoom the auto-detected (cheaper, shallower-but-valid)
closure may supersede it — correct per-zoom behavior. The panel now shows the
period actually IN USE ("In use: period N", with a Use button that
adopts it into the hint field) - published by both orbit paths
through a small atomic slot, since the auto-detected period changes
under the user mid-dive and was otherwise invisible. Deferred:
hints on the wasm budgeted path. (The z900+ interior wall once blamed
here on "f32-mantissa crush" was the reference-exponent bug
described below — z900 and z1100 render full structure as soon as
near-nucleus iterates keep their magnitude.)

DF ("double-f32") tier shipped (2026-08-26/27): error-free
transforms with BITMASK Dekker splits (integer ops — immune to
Metal fast-math folding) give the floatexp rung ~2^-48 relative
precision end to end — CFe2 delta mantissas (hi+lo pairs; the
26-octave add cutoff widens to 49; d0 carries the pixel+offset sum
in DF; the rebase rebuilds deltas in DF), and DF REFERENCE STORAGE
(a parallel orbit_lo array, binding 8, format FFORBIT3) so the 2Z
multiplier reads ~2^-48 reference values. IterState still fits 48
B/px; the FE chunk budget drops to 6e8 for the extra per-iteration
cost. The hint path no longer gets preempted by shallower
auto-closures during its own verification (a period-142,232 closure
was stealing the 1,137,764 hint), so f3-style full-period deep
references now actually build (measured: 53 s once at 197 limbs,
then persisted). All gates and the 18-scenario agreement suite
hold.

MEASURED HONESTLY at the time: none of it moved the z700 wall. Two
independent exact implementations (fixed-point and python-Decimal)
agreed the window's true escape band is [23,638..23,928] with only
the exact center interior — while the rendered band sat in (41k,
50k], a coherent ~2× lag IDENTICAL across f32/DF deltas, f32/DF
references, shallow/deep periodic references, and BLA on/off. (The
earlier "center escapes at 40,285" ground truth was itself an
artifact of a probe's 15-limb truncation; the true center is
interior ≥60k.)

REFERENCE EXPONENTS — the z700 wall, resolved (2026-08-26).
It was never a precision limit. Reference iterates were stored as
f32 hi + f32 lo: 48 bits of MANTISSA, but only f32's EXPONENT
RANGE. A reference that passes close to a nucleus goes far below
f32's smallest normal (2^-126) and read back as exactly zero —
deleting the 2·Z·δ term from that step of the delta recurrence.
With the dominant term gone the delta drops to δ², and it never
recovers the octaves it lost.

Found by a paired trace: the shader's DF model and an EXACT
fixed-point delta, side by side against the same reference. They
agreed to one octave for 8,896 iterations, then the model lost 154
octaves in a single step — at i=8,897, where the reference dips to
|Z| ~ 2^-183 — and another 156 at i=17,769 (the next such dip).
Between drops the growth rates match exactly; the entire ~2×
deficit is those two steps. Dips to 2^-13..2^-37 recur every ~70
iterations and are harmless; only the sub-2^-126 ones bite, which is
why the lag looked like a smooth model deviation rather than an
arithmetic bug.

The fix is a per-entry binary exponent (`ReferenceOrbit::orbit_e`,
binding 9, orbit format FFORBIT4): the stored iterate is
(hi + lo)·2^e, with **e = 0 for every iterate above 2^-90**. The
overwhelming majority of entries are therefore byte-identical to
the old format, and every consumer that wants a plain value (BLA
radii, the rebase test, `z_full`, Ship) keeps its old semantics
through `entry_value` / `ref_z` — which flush to zero exactly as
f32 did. Only near-nucleus iterates carry a nonzero exponent, and
the deep rung's 2Z multiplier (`cfe2_mul_zdfe`, and `k·z_ref_e` on
the multibrot terms) is what reads it.

Verified: the CPU replica of the shader model now escapes at 23,649
— exactly ground truth — with the model-vs-exact gap flat at one
octave across all 24k iterations, both dips included. On the GPU
the same window renders 100% interior at max_iter 23,000 and 99%
escaped at 24,000, bracketing the true band [23,638..23,928]; the
pre-fix build was 100% interior at 30,000 AND at 41,000. The
curated z680 location and z900/z1100 on it — the "interior wall"
previously blamed on f32-mantissa crush — now render full
structure.

It also explains "panning changes the STRUCTURE of the fractal,"
reported at z543 and never explained: a pan moves the reference
center, which moves where that reference's sub-2^-126 dips fall,
which changes how much delta growth gets deleted — so the picture
reorganized instead of sliding. Measured on the user's z543 pan
pair (200k iterations, 640×384): pre-fix the two frames have no
alignment peak at all (best mean|Δ| 44.9 at dy=−2 vs 46.8
unshifted — uncorrelated images). Post-fix they align at dx=0,
dy=−3, mean|Δ| 44.3 → 23.6: a pure vertical pan, which is what the
config pair actually encodes. The pre-fix and post-fix renders of
the SAME config are entirely different images.

Cost: 4 bytes per orbit entry (16 → 20 B) and one extra buffer read
per iteration. Measured on identical-output workloads (1280×800,
max_iter 400k, best of 2, ~0.4 s of that is process + GPU startup):
floatexp rung 1067 → 1138 ms, scaled rung 551 → 574 ms — +4 to +7%
wall clock, and the two builds' PNGs are byte-identical where no
dip occurs, which is what makes it a fair comparison. BLA is
unchanged and still declines to skip across a deep dip (a
zero-radius entry): conservative, not wrong.

Depth reached after the fix, on the curated z680 location (640×384,
reference_period 1,137,764, ~1–2 s per frame once the orbit is
persisted): z680, z900, z1100, z1500, z2000 and z3000 all render
full structure. BLA-on vs BLA-off agree on the interior/escaped
classification to 0.00–0.08% of pixels across z700–z3000.
CAUTION when eyeballing that comparison: at `scale` 0.1 a palette
cycle is ~10 iterations, so a ±1-iteration difference repaints the
frame and the two renders "differ" on 26–73% of pixels. Widen the
palette (`scale` 1e-5, a 100k-iteration cycle) and the same pair
differs on 0.03–0.15% — boundary dust, the z484 class. Judge
agreement on the escape MASK or a wide palette, never on a
compressed one.

What is NOT claimed: only the z700 window has exact ground truth
(two independent implementations). Deeper frames are verified for
self-consistency (BLA on/off) and for producing structure, not
against an exact escape band.

THE HINT RECOMPUTE LOOP (fixed 2026-08-27). Pushing the same
location to the f3 file's own depth (zoom 9316.69) exposed a
separate pathology: nothing rendered, and one CPU core stayed busy
forever. The log said why once it was read at info level — "periodic
reference from hint: period 1137764" every 54 seconds, endlessly.
`try_periodic_from_hint` returned a periodic orbit whose closure was
NOT deep enough for the view; the cache's `periodic_serves` check
then rejected it on the very next frame, and the next frame rebuilt
it. Every frame paid a full reference computation and none of them
was ever usable.

The fix keeps the work rather than throwing it away: when the hinted
period closes but not below the view's pixel scale, the orbit is
already the plain prefix 0..=p with a live fixed-point state that
continues it, so the periodicity is dropped, the ordinary closure
limit is restored (auto-detection can still close it deeper later),
and it extends to max_iter as a plain reference. One build, then the
frame renders. The warning names the numbers.

Measured on the f3 location at zoom 9317: |Z_p| ~ 2^-6263 against a
pixel-scale limit of 2^-9332. So THAT center string, with that
period, supports a periodic-wrap reference down to about z6250 —
past that the wrap is not exact and the render needs a full-length
reference (or a center refined further toward the nucleus, which is
the Newton work still open for large periods). This is a property of
the coordinates, not a bug: |Z_p| is the center's distance from the
true nucleus amplified by the multiplier.

THE f3 TARGET RENDERS (2026-08-27). 001.f3.toml's own location and
depth - zoom_log2 9316.69, 10,100,100 iterations, the coordinates
verified char-for-char against the .fflame - now produces a clean
deep-zoom frame: a double-scepter valley with spiral filigree, no
glitch dust, no interior collapse. 640x384 took ~8 minutes cold
(dominated by the 10.1M-iteration reference at 197 limbs) and
**26.5 s** warm at 2x supersampling, reference reloaded from the
orbit store. Two practical notes fell out of it:

- Drop `reference_period` from a config whose zoom the hint cannot
  serve. It costs a 54-second build that is then discarded, and -
  because the disk-load filter matches on the hint - it also blocks
  the cached plain reference from loading. Without the hint the
  same frame reuses the stored orbit and renders in seconds.
- The first pass looked like noise at `scale` 0.02 with no AA. It
  was not: 2x supersampling and a wide palette (2e-4) resolve it
  into clean structure. Same lesson as the BLA comparison - a
  compressed palette at depth turns per-pixel iteration differences
  into speckle, and judging accuracy through one is a mistake.

Two costs measured while chasing this, both worth knowing before
anyone "optimizes" the deep path:

- A reference iteration at 197 limbs is 48.9 us, and 44.6 us of that
  (91%) is the two `mul_trunc` calls. Stripping essentially every
  heap allocation out of `z.sqr().add(&c)` won 1.03x - the allocator
  reuses same-size blocks and is not the problem. Karatsuba does not
  pay at this limb count either: the truncated high-window product
  already costs ~n^2/2 MACs, about what Karatsuba on the FULL product
  would cost at n = 197. So a 10.1M-iteration reference is ~8 minutes
  of arithmetic, one-time and persisted - the honest floor for the
  f3-class target, not an inefficiency to hunt.
- Known cost not yet addressed: the disk-load filter requires
  `o.periodic == reference_period`, so a fallback (non-periodic)
  orbit built for a hinted location is rejected next session and
  rebuilt. In-session reuse works - the in-memory slot has no such
  check - so this is 8 minutes once per session, not per frame.
  Fixing it properly means persisting "this hint was tried and found
  too shallow", i.e. another format bump; deliberately deferred
  rather than churning the format twice in a day.

### Deep-zoom performance queue (agreed 2026-08-27)

Ordered. Each entry says what is MEASURED and what is inferred, so
nobody re-derives it.

1. **Adaptive chunk sizing.** DONE 2026-08-27 — and it exposed a
   correctness bug worth more than the speed. Original diagnosis:
   the in-app path runs
   exactly one chunk per redraw and sizes it as
   `6e8 / (W·H·ss²)` iterations — at 3× supersampling on a
   1280×720 viewport that is ~72 iterations per frame, so a
   10.1M-iteration render needs ~140,000 frames and each of those
   frames also re-runs downsample + tonemap over 8.3M pixels. The
   GPU is idle most of every frame: low CPU, low GPU, slow render.
   The headless path has no such pacing (`while !settled`), which is
   why the same frame is 26.5 s from the CLI and minutes in-app.
   Measured effective rate on this GPU at z9316/2× AA:
   3.7e11 pixel-iterations/s, i.e. the 6e8 budget is a **1.6 ms**
   chunk.

   Shipped: the static `budget / pixels` rule is now only a SEED, and
   a feedback loop resizes the chunk against a wall-clock target
   measured between calls. The target is derived from a running
   minimum of that time rather than being a fixed millisecond figure
   — under vsync the inter-call time is pinned at the refresh period,
   so a fixed 12 ms target would read 16.7 ms as "over budget" and
   shrink to the floor forever. Headless sets an explicit 200 ms.
   `ESCAPE_CHUNK_MS` forces a target, for the invariance test below.
   Measured on the f3 frame, 640×384: 26.5 s → **6.6 s** at 2× AA,
   and 3× AA (2.25× the pixels) costs only 7.3 s — both are now
   dominated by loading the 197 MB reference, not by compute.

   THE BUG IT EXPOSED: the same config rendered 36% of its pixels
   differently at the new chunk size. Chunk size was an INPUT to the
   image. BLA skip selection was bounded by `perturb.iter_end - i`,
   the CHUNK's end, so small chunks permitted only short skips and
   the amount of linearization depended on where the boundaries
   fell — i.e. on window size, supersampling and frame pacing. Fixed
   by bounding the skip with `params.max_iter - i` (the render's end)
   and carrying the true iteration index across chunks in
   `IterState.i_at`, so a skip may cross a boundary and the next
   chunk resumes where the pixel actually is. The escaped flag moved
   into the high bit of `n_done` to pay for the word, keeping the
   struct at 48 B/px. Verified: a 1 ms target and a 500 ms target now
   produce **bit-identical** frames (0 pixels differ, was 36%).

2. **Persist the rejected hint.** DONE 2026-08-27 (format FFORBIT5).
   The orbit
   filename hashes (center, limbs, julia, power, ship, variant) —
   NOT the period — while the disk-load filter demands
   `stored.periodic == reference_period`. So setting a period on a
   location whose plain reference is already cached rejects that
   file and rebuilds it (8 minutes at 10.1M/197 limbs), and then
   `save_to` refuses to rewrite because the rebuilt orbit is not
   longer — no new file, old timestamp, all work discarded.
   Observed by the user verbatim.

   Shipped: the orbit records the hint it TRIED and the |Z_p| octave
   that hint actually closed at, both in the format's fixed prefix so
   the store's staleness probe stays a 16-byte header read.
   `answers_hint` then lets a request carrying that period recognise
   the plain orbit as its answer — and it is zoom-aware, so the same
   hint can be too shallow here and exactly right two hundred octaves
   out. `save_to` also had to learn that a hint-only difference is
   worth a rewrite: the fallback orbit is exactly as long as the plain
   one already stored, so a length-only staleness test silently
   dropped the fact. Measured end to end on the f3 config with the
   period field set: run 1 **495 s** (one hint build, then the
   fallback), run 2 **6.2 s** (zero hint builds, loaded from store).

   Related, FIXED 2026-08-27 after it bit in practice: the store's
   byte cap was a 256 MB constant while one 10.1M-iteration reference
   is 202 MB, so a second deep location evicted the first. Observed
   live - a session wrote a 45 MB orbit, the f3 reference went out
   with it, and the next visit spent eight minutes rebuilding a file
   that had been on disk. The cap is now a SystemSetting
   (`orbit_cache_mb`, Settings -> Advanced, 64-8192 MB) applied to a
   runtime static, default raised to 1 GB, with current usage shown
   and a Clear button.

**The zoom-OUT crash (fixed 2026-08-27).** Reported as
STATUS_STACK_BUFFER_OVERRUN (0xc0000409) when zooming out of an
f3-depth location, and again from an animation whose zoom track
crossed the same line. Reproduced headlessly: `Parent device is
lost` -> wgpu panics -> the process aborts, which is what that exit
code is.

The cause is not the live-update pacing it looked like. Below
`PERTURB_MIN_ZOOM` the DIRECT template renders, and it has no
per-pixel resume state, so a whole frame is ONE dispatch costing
pixels x max_iter. That is harmless at the iteration counts a
shallow view normally carries and fatal when a deep-zoom config
keeps its 10.1M max_iter on the way out: tens of seconds in a single
submission, and Windows resets the driver at two.

Fixed by splitting the direct and field templates into ROW BANDS.
Each band is a complete render of its own rows - no resume state
needed, the output texture accumulates them - so the dispatch is
bounded without touching the math. Verified pixel-identical to the
old single-dispatch render at two sizes. The band offset reuses the
uniform's spare pad word, so the layout is unchanged and the
perturbed templates (which chunk by ITERATION) simply leave it zero.

The band size is deliberately FIXED and conservative rather than
adapted. The wall-clock feedback that paces iteration chunks does
not transfer here: a small band is latency-bound rather than
throughput-bound, so several doublings all come back under target
and the next one is the whole frame again - measured, blind doubling
lost the device in 2.6 s. At 4x the chosen budget a supersampled
10M-iteration view still lost the device while the budget completed
it, so that is where it sits. The cost is real and worth stating:
small bands under-fill the GPU, and this view renders in 20 s
against ~3 s for one unbounded dispatch. That is the price of not
gambling the process on a state the user reaches with one zoom-out.

**Orbit logistics, not GPU math, dominated a deep interactive
session (fixed 2026-08-28).** Reported as the app running "unusually
slow, like it's still doing something in the background" after
returning from an f3-depth location toward z15-20, ending in the
0xc0000409 abort -- and reducing max_iter did not help, because the
store loads the full stored orbit regardless, so every cost below
scales with the STORED length (10.1M), not the request. Four
mechanisms, found by code review of this scenario:

1. Request-epoch churn. `OrbitRequest` equality includes zoom and
   max_iter, so every wheel notch bumped the epoch; the worker then
   republished the ENTIRE orbit into the shared progress (hundreds
   of MB of memcpy at f3 sizes) even when it REUSED the orbit, and
   the renderer -- keying its upload on the epoch -- re-uploaded all
   ~200 MB of orbit buffers per notch. `OrbitProgress` now carries a
   content GENERATION, bumped only when the orbit is actually
   replaced (fresh compute, truncation republish): reuse skips both
   the clone and the upload; appends stream under an unchanged
   generation. The same key fixes a latent staleness bug on the
   blocking path: a pan at fixed max_iter swaps in a different orbit
   of the SAME length (both non-escaping orbits are max_iter+1
   long), which the length-only upload test left stale on the GPU.

2. BLA rebuild on every zoom-OUT tick. The table guard accepted the
   cache only while the view's |dc| bound sat below the built one,
   and that bound grows monotonically zooming out -- so every notch
   paid a full f64 orbit copy (taken under the worker mutex, stalling
   the worker too) + an O(n) merge tree + a ~half-GB buffer rewrite,
   on the main thread. A rebuild forced by dc growth now builds with
   4 octaves of |dc| HEADROOM (further rebuilds at most once per 16x
   widening; the cost is slightly smaller skip radii via the dc term
   in the merge -- conservative, never wrong), while a FRESH build
   keeps the exact bound, so one-shot headless renders are
   bit-identical to before (verified over the 51-config escape suite;
   uniform padding moved up to 13% of pixels on the deep
   parameter-plane frames, so the pad is confined to the interactive
   case that actually churned). The table is also keyed on the orbit
   generation, and never (re)built while the reference is still
   growing: a shorter-prefix table of the same orbit keeps serving
   (valid -- its spans are a prefix of the appended orbit), otherwise
   the dummy is bound and per-step iteration carries the frame.

   Two follow-ups after a crash survived the first round (2026-08-28):
   the build is CAPPED at max_iter + 1 -- entries past the view's
   iteration budget are unreachable (a pixel's reference index never
   exceeds its iteration count, and the shader bounds every skip by
   `max_iter - i`), so a retained 10.1M-entry orbit serving a
   shallow view with a few thousand max_iter now builds a
   thousand-entry table instead of paying a ~2 GB transient and a
   seconds-long main-thread stall per rebuild. And consecutive
   dc-growth rebuilds DOUBLE the headroom (4 -> 64 octaves): the
   f3-to-threshold zoom-out crosses ~2300 octaves, which is ~580
   rebuilds at a fixed 4-octave pad and ~a dozen with backoff.

3. Chunk-state restarts during reference growth. The chunk key
   included orbit_len, so every worker publication restarted the
   perturbed render from iteration 0 -- nothing progressed until the
   reference finished, and the BLA length check re-triggered (2) per
   frame on top. The key now carries (generation, done): an append
   leaves per-pixel state valid (rebasing wraps at the published
   length, and the GPU mirror is append-only under one generation),
   and the done flip restarts ONCE, so the settled image is computed
   entirely against the finished reference.

4. Device loss aborted the process with no line of ours in the
   message: `on_uncaptured_error` never sees device loss.
   `set_device_lost_callback` now logs the reason -- observability
   only (recovery needs a full GPU-context rebuild), but it is the
   difference between "crashed" and knowing whether it was a TDR or
   a VRAM overcommit.

5. Found via the crash.log breadcrumb that (4) enabled, one session
   later: the app's full-GPU-reinit recovery dropped flame_renderer,
   effect_chain and the egui layer before rebuilding the device --
   but NOT escape_renderer. It survived reinit holding buffers from
   the dead device, and the next escape frame's write_buffer tripped
   wgpu-core's dead-buffer assert; the unwind then hit wgpu-hal's
   swapchain-semaphore Drop assert, and the double panic aborted as
   0xc0000409. Fixed: the reinit path drops it (lazy recreation
   against the new device; the orbit worker exits with its channel,
   the reference reloads from the disk store) and re-marks escape
   dirty. So the crash sequence was: device loss (logged now),
   successful reinit, then the forgotten renderer -- not a new
   escape-pipeline fault.

6. With recovery working, the underlying loss showed itself: a TDR
   loop, reproduced as "high iterations at low zoom after a deep
   zoom" (10.1M max_iter at zoom ~1 -- the direct path, as the
   reporter correctly noted). The row-band budget is a px-iteration
   bound, but a px-iteration's COST is config-dependent, and the
   bands over the set INTERIOR run every pixel to full max_iter: one
   of those exceeded the ~2 s TDR window, and recovery restarted the
   render from the top into the same fatal band, forever. Fixed with
   a shrink-only session breaker (`DIRECT_BUDGET_SHIFT`): a device
   loss during a banded direct render halves the budget (the
   post-recovery retry cannot repeat the dispatch that killed the
   device), and a surviving band measured over 700 ms halves it too.
   It never grows back within a session -- growth is the feedback
   the band-size ledger above measured losing the device -- and it
   is free for ordinary renders, which stay one full-height dispatch
   even at the floor. GPU events (device lost, first 20 uncaptured
   errors) now also append to crash.log: the GUI build is a
   windows-subsystem binary, so without CLI arguments there is no
   console and stderr logging goes nowhere -- which is why "set
   RUST_LOG=info" showed nothing.

7. Cold and warm renders of the SAME config diverged -- caught by a
   user comparing a freshly regenerated f3 frame against the next
   session's store-loaded one ("similar structure, like one is the
   other rotated"). The configs were byte-identical and so was the
   orbit (the cold session saved the very file the warm session
   loaded): the difference was the UPLOAD path. The progressive
   append streamed the growing orbit to the GPU, and on a capacity
   crossing (1.5x headroom) the recreated buffer was refilled with
   only the newest tail -- everything before it stayed garbage, and
   the settled render was a structurally wrong self-similar sibling.
   A complete orbit (store hit) uploads whole in one pass, hence
   correct-warm/wrong-cold. Predates the generation rework (the old
   epoch compare had the same hole). Fixed: a recreated buffer
   always refills from scratch; pinned by an app-repro test that
   renders the same perturbed view progressive and blocking and
   demands byte-identical output. Two adjacent worker fixes landed
   with it, both observed in the same session: in-memory reuse now
   accepts an orbit at HIGHER precision than the request (equality
   rebuilt a full-depth orbit at every 64-octave limb crossing of a
   zoom-out -- four 202 MB duplicates of one (-1.5, 0) orbit), and
   the worker's disk-load filter gained the blocking path's
   answers_hint + periodic_serves acceptance test, which it had
   silently lacked.

These four are also most of the desktop-vs-wasm gap the user
measured at z14+: the wasm path structurally skips the worker
republish, the nucleus/hint machinery, and the disk-store inheritance
of 10M-entry orbits, so it never paid (1)-(3).

**Waiting for a slow reference instead of rendering against it.**
A partial reference does not render as progressive refinement at
depth: every pixel wraps almost immediately, so the frame is flat
colour that changes wholesale as the prefix grows - the user's
report was "flashing, solid colors". Where the reference is quick
that flicker is invisible and the early frames are genuinely useful,
so the rule has to distinguish the two cases rather than pick one.

`predicted_orbit_seconds(iters, limbs)` does it from the measured
step cost - a reference iteration is two truncated big multiplies
and nothing else that matters, so cost is iterations x limbs^2 x a
constant calibrated against the f3 build (10,100,100 iterations at
197 limbs = 495 s). Above 0.75 s predicted, the in-app path holds
the last good frame and publishes progress instead of dispatching;
below it, nothing changes. The viewport draws "Computing reference
orbit... N%" over the held frame. The headless path is untouched (it
was never progressive) - verified bit-identical output.

3. **Make Detect zoom-aware.** DONE 2026-08-27.
   `detect_center_period` returns the
   SMALLEST closing period; deep views need the largest period whose
   closure still sits below the view's pixel scale. Measured on the
   f3 location: period 71,100 closes at |Z_p| ~ 2^-613 (serves to
   ~z597) and period 1,137,764 at 2^-6263 (serves to ~z6250) — a
   cascade ladder, ×16 in period and ~×10 in closure octaves per
   rung. Detect handed back 71,100 for a z9316 view, which cannot
   serve and silently costs a rebuild. Note the corollary: a period
   deep enough for z9316 would exceed max_iter 10.1M, so for that
   target NO hint is the right answer and the field should say so.

   Shipped as `detect_period_for_zoom`, applying the same test
   `extend` uses to accept a closure — the first NEW |Z| minimum below
   the view's limit — so anything returned provably wraps. The panel
   reports the conclusion ("Period P — wrap exact to 2^-N, i.e. zoom
   ~Z") and says plainly when nothing within the cap wraps at this
   depth, which is the f3 target's actual answer.

4. **BLA disagrees with exact iteration at depth.** LARGELY FIXED
   2026-08-27; a residual remains, and its cause is NOT any of the
   three things one would try first. Measured on the f3 frame
   (z9316, 10.1M iterations, 640×384): against a BLA-off render
   (exact perturbed iteration, 177 s vs 6.3 s — BLA buys **28×** at
   this depth, against only 1.4× at z1500), the BLA render differs on
   **66.5%** of pixels, mean |Δ| 25.6/255. Tightening `BLA_EPS` from
   2^-24 to 2^-40 — 65,536× — moved that by 0.01 and cost nothing in
   time; the two tolerances agree with EACH OTHER on 99.5% of
   pixels. So the divergence is systematic, not accumulated
   linearization error.

   That hypothesis was right. The table was built from the f32
   reference `hi` alone while the deltas ran in DF at ~2^-48, so
   every skip injected ~2^-24 of coefficient error — invisible to
   `BLA_EPS`, which bounds only the dropped nonlinear term. Feeding
   `build_with_dc` the resolved `hi + lo` scaled by the per-entry
   exponent, in f64, moved the disagreement with exact iteration from
   **66.5% of pixels (mean 25.6/255) to 12.4% (mean 2.6)** — 10x on
   the mean — and made the render slightly FASTER (6.3 s → 5.7 s),
   because a deep dip now carries a real radius instead of a zero one
   and those steps became skippable. That also subsumes the dip
   mechanism described below, which was the same range problem seen
   from the other side.

   THE RESIDUAL (12.4%, mean 2.6/255 ≈ 1%) is NOT explained by the
   obvious candidates, all measured:
   - Not the tolerance. `BLA_EPS` from 2^-24 to 2^-40 — 65,536x —
     moves it by 0.01 and costs nothing in time.
   - Not per-skip coefficient error accumulating. Capping the table
     at 8 levels (spans ≤ 512, so many more, much shorter skips)
     gives 12.58%/2.63 — indistinguishable, and 42% slower.
   - Not skip length in either direction.
   What remains, unproven: the GPU `BlaEntry` still carries `a_m` as
   a **vec2<f32> mantissa**, so the multiplier a skip applies is
   f32-precise even though the table is now computed in f64. Making
   it a DF pair costs ~25% on table memory. The skip-length evidence
   argues AGAINST per-skip error dominating, so this is a real
   precision gap but probably not the explanation — do not start
   there without a cheaper discriminating experiment. Chaotic
   amplification of any path difference at 10M iterations is the
   other candidate and would be irreducible.

   Practical state: BLA now buys **31x** (177 s → 5.7 s) at ~1% mean
   colour deviation from exact iteration on the hardest frame we
   have. `ESCAPE_DISABLE_BLA=1` still renders the exact path.

   The original dip mechanism, now subsumed by the f64 feed:
   BLA is fed exponent-flushed reference values (`entry_value`), so
   an iterate below 2^-126 reads as zero, its radius
   `eps·|Z|·2/(p−1)` is zero, and `bla_mag_lt` treats that as "never
   valid" — no skip across that step, and every merged entry
   spanning it inherits radius zero, capping useful skip spans near
   the dip. With exponent-aware input the radius at a 2^-183 dip
   would be ~2^-207, and the traced pixel's δ there was ~2^-350 —
   comfortably inside, so those skips would become legal. Dips
   recur roughly every 9,000 iterations in the z700 orbit. Unknown
   until instrumented: the actual skip rate, so the win is unsized.
   Same bug family as the reference-exponent fix, one layer up.



5. **Headless chunk overhead.** Partly subsumed by (1): the
   per-chunk downsample pass is redundant for an export where only
   the final image matters.

**Orbit-store compression (FFORBIT6, shipped 2026-08-28).** A
double-double shadow of the reference runs inside `extend()`; the
stored file keeps only the shadow's CORRECTIONS (full-precision
resets, emitted at every index where the shadow's decomposition
would differ from the true fixed-point one) plus an RLE exponent
stream, and load replays the shadow to regenerate the arrays
BYTE-FOR-BYTE -- exactness by construction, not tolerance, so
renders, cold==warm identity, and the store roundtrip tests are all
untouched. Measured on the f3 orbit before committing to the design:
an f64 shadow is useless (corrections every ~1.4 iterations -- the
48-bit stored triples are not self-consistent under f64 iteration),
the DD shadow spaces corrections ~216 iterations apart, and the
retroactive lossy alternative (rebuilding corrections from stored
48-bit values) measurably moved renders (mean 2.4/255 over 83% of
pixels on the f3 frame) and was REJECTED for the in-pipeline shadow,
which resets from the live fixed-point state instead. In-memory and
GPU formats unchanged; old FFORBIT5 files read as misses (standard
policy). The array section is dual-mode: an orbit whose corrections
would OUTWEIGH the raw arrays (dip-dense near-nucleus orbits record
a 52 B correction almost every entry -- measured 2.6x worse than raw
on the period-3 antenna) is stored raw instead, tagged by a mode
byte; whichever encoding is smaller wins, and both are byte-exact. Deep dips below f64 range poison the shadow and degrade
gracefully to per-entry corrections through the dip.

### Formula accuracy audit (2026-08-28)

Every registered formula checked against an INDEPENDENT oracle --
numpy transcriptions written from the published definitions, not from
our WGSL (transcribing our own code proves only that it equals
itself, which is how Ducks shipped for months with `c` outside the
log). Reproduce with `python scripts/audit_escape_formulas.py`.

Two observables, because one does not fit every formula:

- **Set membership** for escaping maps: an all-white palette on black
  makes "escaped" exactly "not background", with no dependence on
  palette shape or tone mapping. A wrong map moves that boundary
  everywhere.
- **Equal-area field thresholds** where the set is uniform (Manowar
  escapes everywhere; convergent maps converge everywhere; the
  non-escaping trio never escapes at all). Both fields are monotone
  in the same quantity, so thresholding each at the same AREA
  fraction must select the same region -- a comparison gamma,
  exposure and palette cannot affect.

A vacuum guard reports any formula whose mask is >98% one class as
UNINFORMATIVE rather than counting it as agreement, so the audit
cannot pass by comparing two blank images.

**21 of 23 verified.** Disagreement is boundary-pixel noise
(f32 render vs f64 oracle), worst 0.24% on tetration:

| result | formulas |
|---|---|
| set matches (<0.25%) | mandelbrot, multibrot, tricorn, burning_ship, mcmullen*, phoenix, lambda, spider, barnsley, cactus, exponential, trig, tetration, collatz, feather, newton†, nova†, magnet†, littlewood |
| escape-time field matches | manowar (its set is uniform -- everything escapes) |
| mean-\|z\| field matches | ducks |
| structure verified, field UNEXPLAINED | kaliset, novaretti |

\* McMullen is audited in JULIA mode: its parameter plane escapes
everywhere, because the critical point IS the pole -- which the
registry doc already says, and which the shipped carpet config
already does. The audit confirms the caveat rather than contradicting
it.
† Convergent maps need a low iteration cap (10-14) to be informative:
uncapped they converge everywhere and the mask is uniform by
construction. Capped, the mask is "converged fast", whose boundary is
the basin structure.

**The bug it found.** Novaretti's double-pole guard returned the
sentinel `1e10`, documented as "feeds infinity, which maps to 0 next
step". True in exact arithmetic; false in f32, where the next step
computes `(2*(1e10)^3)^2 = 4e60` and overflows the 3.4e38 ceiling, so
num and den both become inf and the step returns inf/inf. On Vulkan
that is NaN, which poisons the pixel for every remaining iteration;
on Metal, whose fast-math folds inf/inf to 1.0, it silently returns a
plausible finite number instead -- the same formula rendering
differently on two platforms. The sentinel is now 1e6, which keeps
den at 4e36 and reaches ~1.5e-12 next step, as intended. McMullen's
identical-looking guard is safe because that formula ESCAPES: its
sentinel trips the bailout immediately instead of having to survive
another step.

**Novaretti closed 2026-08-28, against a third-party implementation.**
A user supplied an independent Shadertoy of the same map (component-
unrolled real arithmetic, dynamical plane, its own bailouts). Two
results:

- The MAP is confirmed. Transcribed literally, its field disagrees
  with ours by 6.5% under its own bailout policy and by **0.51%**
  once that policy is matched to ours -- so the difference was never
  the formula, only the conventions: it breaks on |z|^2 > 1e4 or
  < 1e-4 and freezes its orbit trap there, while ours is
  NonEscaping and keeps iterating (justified: for large |z| the map
  gives z' ~ -1.5/z^2, so infinity maps back toward 0 and orbits do
  not truly escape). Its pole guard is also looser, 1e-7 against our
  1e-24, and it breaks where we substitute and continue. Those are
  the bright blobs visible in a side-by-side: pixels whose trap it
  froze early and ours refined further.
- The SEED is confirmed, and it was the one thing the audit could not
  check independently (the oracle copied it from our own WGSL).
  Solving f'(z) = 0 numerically for several c gives critical points
  satisfying z^3/c in {-0.0729490168, -3.4270509832} -- the two roots
  of t^2 + 3.5t + 0.25 -- and our seed constant is the first of them
  to ten digits. The parameter plane is therefore seeded at a genuine
  critical point, as Mandelbrot's z0 = 0 is.

`novaretti-julia.fflame` pins the verified view.

**OPEN: kaliset and novaretti fine-grained fields.** Side-by-side
renders show the SAME structure as the oracle (novaretti's star
field, kaliset's arc system, in the same places), and the algebra is
verified by inspection and by a match at one iteration. But the
mean-|z| fields disagree 16% / 23% under equal-area thresholds, and
the cause is none of the candidates, all measured and excluded:

- not formula transcription (inspection; novaretti matches at
  iteration 1, before any amplification);
- not f32-vs-f64 precision -- an f32 ORACLE disagrees with our render
  identically (14.0% vs 14.0%), which is the discriminating test;
- not overflow (max |z^3| reaches 6.9e17, inside f32) and not
  cancellation in `2z^3 - c` (0% of pixels lose even 3 digits);
- not the pole sentinel above (fixing it left the numbers
  bit-identical, so that guard is not hit in these views);
- not 8-bit quantization alone, and not `fract()` wrap on the tail
  (tested by excluding clipped pixels, by true-range scaling, and by
  excluding the wrapping tail -- all three leave it).

Both are chaotic, heavy-tailed maps whose fields span five orders of
magnitude, which an 8-bit linear render carries badly in either
scaling; that is the leading remaining suspect for the MEASUREMENT
rather than the map. Recorded here so the next person starts from the
excluded list rather than re-deriving it.

### Tricorn joins the perturbation tiers (2026-08-28)

The first formula added to the deep-zoom machinery since the Ship
family, and the one that shows what "extending it" actually costs.

The MAP is anti-holomorphic (`conj(z)^p + c`), but its delta
expansion is not new mathematics: `conj(Z+d)^p - conj(Z)^p` expands
by the same binomial as the plain power, in conj(Z) and conj(d),
because conjugation is an involution that distributes over products.
So both rungs' codegen was PARAMETERISED on its operand names rather
than copied, and the tier is a four-line wrapper binding conjugated
operands -- on the deep rung too, since conjugating a DF pair is also
just a sign flip. Powers 2..12, both rungs, verified against the
direct render at 0/768 differing blocks.

What does NOT come along, and why:

- **BLA.** The skip table models a step as `A*delta + B*delta_c`,
  and conjugation is antilinear, so no such A exists. Tricorn renders
  per-step; the deep-zoom speedups BLA buys elsewhere are unavailable
  until someone derives an antilinear analogue.
- **Nucleus relocation and period detection.** Both are derived for
  `f_c^p(0)` -- Newton on the polynomial, the ball method, the
  closure test. Tricorn takes the plain view-centre reference.

The orbit's map identity is carried in the existing `ship_variant`
field, which is unused when `ship` is false: 0 is the plain power, 1
is the conjugate family (`MAP_PLAIN` / `MAP_CONJ`). That field
already threads through every orbit signature, the disk key and
`serves()`, so a parallel `conj` flag would have put the same fact in
two places and allowed a cache key to disagree with itself. When a
third non-fold family lands, the honest refactor is a small `MapId`
struct carrying (power, ship, variant) -- recorded here rather than
done now.

Verified at depth, not just where the direct path can argue: against
an f64 oracle at the same centre, our perturbed tricorn disagrees on
0.33% of pixels at zoom 20 and 0.40% at zoom 28 -- the same quality
as the known-good mandelbrot control measured the same way (0.00 to
0.04%). `tricorn-deep.fflame` pins the zoom-28 view. Note this needed
a control: the first measurement said 9-13%, which looked like a
depth-only defect until the mandelbrot control came back clean and
the cause turned out to be the MEASUREMENT -- the view had been
walked to the slowest-escaping pixel, so it was packed with orbits
finishing within 10% of max_iter, where a one-iteration difference
flips set membership. With iteration headroom the disagreement
collapsed to 0.4%. A deep view centred on a near-max_iter pixel
cannot be compared by set membership at that max_iter.

One trap worth remembering for the next tier: the nucleus-aware
route had to exclude the new family MECHANICALLY, not just
mathematically. A nucleus orbit is built tagged variant 0, which a
variant-1 request can never be served by, so leaving it in place made
the cache rebuild the reference every frame and the render never
settle -- which is what "chunk loop failed to settle" meant when the
agreement test first ran.

### Phoenix joins the perturbation tiers (2026-08-28)

The second family added, and the one that broke the mould the first
two fit. `z' = z^2 + c + p*z_prev` is a TWO-TERM recurrence with a
CONTINUOUS parameter, so it needed three things Tricorn did not:

- **A second delta.** The step is `w' = 2Zw + Sw^2 + p*w_prev + d0`,
  with the history advancing to the delta just left. It rides the
  scaled rung's `w_lo` field, which is the deep rung's and dead here
  -- so IterState does not grow and no perturbed render pays 8 B/px
  for a family it does not use. That is also why Phoenix is
  SCALED-RUNG ONLY: on the deep rung `w_lo` is live, so there is no
  free slot, and the renderer pins Phoenix below the floatexp
  threshold (twice: in `wants_perturbation`, and again where the rung
  is chosen, because the test hook can force it).
- **A rebase to index ONE.** The pixel's history must be measured
  against a reference iterate too, and there is no Z_-1: the earliest
  state to rebase onto is the PAIR (Z_1, Z_0), with both deltas moved
  together. Rebasing only the current delta would leave the history
  against a different index -- a wrong orbit, not a noisy one.
- **The parameter in the orbit IDENTITY.** A different `p` is a
  different reference, so it reaches the cache key, `serves`, the
  worker's dedup and the on-disk format (FFORBIT7). Unlike Tricorn's
  fold selector it cannot ride a variant enum, which is what forced
  `MapId`.

BLA is off for the same shape of reason as Tricorn's, but a different
mechanism: a skip advances the reference index without running the
intervening steps, and Phoenix's step also advances its HISTORY, so
`w_prev` would end up measured against the wrong iterate. A two-term
BLA wants 2x2 coefficients.

Verified: 0/768 blocks differ from the direct render for two
parameter values, and against an f64 oracle at depth, 0.00% of pixels
at zoom 28 and at zoom 20. `phoenix-deep.fflame` pins the zoom-28
view, `phoenix-short-orbit.fflame` the rebase-heavy one below.

**The rebase target is index 0, and getting that wrong is invisible
to every cheap test.** Index 1 looks like the natural choice for a
two-term recurrence -- it is the earliest index whose PREDECESSOR
exists in the array -- but `Z_1 = c`, so `z_full - Z_1` is a
difference of two O(1) f32 numbers. The delta survives only to
ulp(c), which at zoom 22 is about EIGHTY PIXELS across, and the image
comes apart into displaced rectangular blocks (twice as wide as tall
when the centre's two components straddle a binade -- that ratio is
how the cause was identified). Index 0's state is the pair
(Z_0, Z_-1) = (Z_0, 0): the reference began with no history and so
did the pixel, so on the parameter plane the rebase subtracts zero,
exactly like every one-term tier. Both deltas move together and the
gate weighs the PAIR's norm -- shrinking z while leaving z_prev far
from the reference would blow up the `p*w_prev` term on the next
step.

**And it shipped broken anyway, which is the useful part.** In the
app the first deep Phoenix render was visibly a DIFFERENT FRACTAL the
moment perturbation engaged. The reference orbit's `p` was resolved
by `map_params_for` with `unwrap_or(0.0)` while the shader's uniform
was resolved by `pack_params` with `unwrap_or(def.default)`: an
absent key meant "the registry default" to one and "zero" to the
other, so a FRESH config iterated deltas for p = -0.5 against a
reference built for the plain quadratic. A config that had been
edited in the UI carried the keys explicitly and worked.

Two lessons, both now enforced by tests:

- **Two code paths resolving the same value is the bug**, not the
  arithmetic. `reference_parameters_match_the_shader_uniform`
  (renderer.rs) asserts the two resolvers agree, with and without the
  keys present, and fails in under a second.
- **Depth alone is not the regime that matters; REBASING is.** The
  zoom-20 oracle test ran with 4000 iterations of headroom, so its
  pixels escaped long before the reference ran out and almost none
  ever rebased -- it read 1.23% through the index-1 rebase bug and
  its threshold was slack enough to pass. The view that broke had 256
  iterations, where the orbit's end forces a rebase on nearly every
  pixel. `perturbed_phoenix_matches_an_exact_smooth_field_on_a_short_orbit`
  covers that regime with a palette-agnostic metric: bin pixels by the
  f64 smooth value, measure the colour spread WITHIN a bin (3.05/255
  correct, 41.34/255 broken). The zoom-20 threshold is now 0.5%, which
  the same bug would also trip.
- **The direct-vs-perturbed agreement test cannot catch this class.**
  It runs shallow, where rebasing fires almost every iteration and
  the reference contributes almost nothing -- it read 0/768 with the
  bug in place. A comparison that exercises the reference has to run
  at DEPTH, against an exact orbit:
  `perturbed_phoenix_matches_an_exact_orbit_at_depth` renders zoom 20
  and compares escaped-or-not against an f64 orbit computed in the
  test (1.23% agreed, 35.9% with the bug reintroduced). It renders
  through a WHITE palette on purpose -- under the default grayscale
  the low end is black, so an escaped pixel with a small smooth value
  is indistinguishable from the interior and the comparison reads 63%
  disagreement on a correct render.

Two cache defects rode along and are fixed with it: `key_for` did not
hash `map_params`, so two Phoenix `p` values shared one orbit file;
and `load_from` passed `orbit.map_params` to `serves()`, comparing
the file against itself so the guard could never fire.

### Phoenix gets a deep rung (2026-08-29)

Phoenix shipped scaled-rung-only, and `wants_perturbation` refused the
perturbed path above `PERTURB_FLOATEXP_ZOOM`. The view then fell
through to the DIRECT path, whose f32 pixel mapping resolves nothing
past zoom 14: 5035 distinct colours at zoom 48.000, exactly **1** at
48.001. "Falls back to the direct path's honest mush" reads fine in a
comment and is a solid block of colour on screen.

The deep rung carries the second delta in real struct fields rather
than the scaled rung's spare `w_lo`, so `IterState` genuinely grows,
48 B/px to 72 B/px, for Phoenix renders only:

- `iter_state_bytes(tier, floatexp)` is the single place the Rust
  buffer and the WGSL struct agree, and `iter_state_stride_matches_
  the_shader` measures the ASSEMBLED struct with naga's own layout
  rules rather than re-deriving the arithmetic. A tier that grows its
  state and forgets the constant would read and write past its slot —
  silent corruption of a neighbouring pixel's history, which no image
  comparison attributes to the right cause.
- The mantissa keeps its LOW half. The history feeds `p * w_prev`
  straight into the next delta, so truncating it to a single f32
  would inject exactly the 2^-24 reseed error the DF rung exists to
  avoid.
- The render-pixel cap at resize now divides by the WIDEST tier's
  stride, because resize does not know which formula will be rendered
  into the surface afterwards.
- The step DELEGATES to the canonical p = 2 floatexp generator and
  appends the history term. A hand-copied version drifted from the
  reference's variable names within the hour.

Verified: 0/768 blocks differ from the direct render on the deep rung
for all three parameter values, 0.00% from an exact orbit at zoom 20
and 3.05/255 on the short-orbit smooth field — the same numbers the
scaled rung posts, because both are compared against the same ground
truth. `crossing_the_floatexp_threshold_keeps_the_phoenix_image`
renders 47.999 and 48.001 and compares them: 0/108 blocks differ.

What that test deliberately does NOT assert is that some deeper zoom
still shows detail. Whether a centre has structure at zoom 60 is a
property of the FRACTAL: checked at 80 digits with mpmath, the
reported centre's neighbourhood is uniform by zoom 55 (every pixel
escaping at iteration 171), and a boundary-hugging centre found by
descent turned out to be uniform too — all of it escaping within one
iteration of the cap, which is the same max_iter cliff that made
Tricorn look 9-13% wrong. A "still has detail at zoom 60" assertion
would be a claim about Phoenix, not about this renderer.

### Manowar joins the two-term tier (2026-08-29)

`z^2 + z_prev + c` is Phoenix's recurrence with p = 1, so it reuses
the second delta, the step and the pair rebase. It differs in two
places, and both matter:

- **The seed.** Manowar starts z_0 = z_-1 = c, so BOTH deltas start
  at d0 rather than at zero -- a parameter-plane map with a Julia
  plane's initial delta. The history's initialiser is a separate
  splice from the current delta's because the template declares them
  in that order; emitting the seed only for `w` left `w_prev` at zero
  and moved 19% of the pixels.
- **The rebase target.** Its history rebases against Z_-1 = Z_0 = c,
  not against zero.

**It is pinned to the DEEP rung at every depth**, which is the
opposite of the Phoenix story and the interesting part. Manowar's
history term carries the delta forward with coefficient 1, so where a
one-term map's delta decays near the reference, Manowar's persists
and f32 mantissa error accumulates over hundreds of iterations.
Measured against an exact orbit at a centre whose own orbit stays
bounded: **18.4%** of pixels wrong at zoom 20 and **27.0%** at zoom
26 on the scaled rung, against **1.6%** and **2.1%** on the deep one
-- the latter matching the direct path's own boundary noise (1.2% at
zoom 12), with the escaped fraction exact to a tenth of a percent.
`perturbed_manowar_matches_an_exact_orbit_at_depth` asserts it and
reads 18.37% if the pin is removed.

Two measurement notes worth keeping, since both cost time here:

- `ship_variant` names a map family ONLY when `ship` is false. With
  `ship` true it is the fold variant, and fold variant 3 collides
  with `MAP_MANOWAR` -- a seeding guard that forgot `!ship` reseeded
  Burning Ship v3 at c and moved 160/768 blocks in the agreement
  test.
- A centre found by maximising local variance sits where the
  reference NEARLY escapes (1704 iterations against a 1500 budget),
  which is a hostile test of the pixel rather than of the renderer.
  The comparison centre is chosen for a bounded reference instead.

**Depth, per formula.** Not one number: `mandelbrot`, `multibrot`
(integer powers), the `burning_ship` variants, `tricorn`/multicorn,
`phoenix` and `manowar` perturb and reach z9316+ (Manowar on the deep
rung only, by measurement -- see above); the rest render
through the DIRECT path and stop where its f32 pixel mapping does, at
about zoom 14 -- which is why `PERTURB_MIN_ZOOM` sits there. Extending that set is item 2 of the
completion plan.

**What remains before this branch is done** is scoped in
[escape-time-completion.md](escape-time-completion.md): per-formula
accuracy against references (only 3 of 23 formulas can perturb --
the other 20 stop at the direct path's ~2^14 ceiling), extending the
Mandelbrot deep-zoom machinery to more formulas (only 3 define a
derivative, which is what BLA, distance estimation and nucleus
finding all need), a UI/UX pass, splitting the WASM builds by
engine, online-API support, and scripting (the script API mentions
escape zero times today).

Two plans split out of this queue on 2026-08-28 (both "later",
sequenced after the orbit-store compression lands):
[escape-tdr-safety.md](../archive/escape-tdr-safety.md) — the perturbed-path
TDR breaker, trust-bounded chunk growth, GPU timestamp pacing,
interior detection, escape-consistent animation playback, and
retiring Overwrite/live-preview in escape mode; and
[escape-ntt-reference.md](../experimental/escape-ntt-reference.md) — GPU reference
computation with a measurement-gated go/no-go.

Still open within phase 4/5: nucleus math for the Ship tier (needs a
2×2 real-Jacobian Newton — abs-folds break holomorphy); a wasm
WORKER as a performance upgrade only (COOP/COEP hosting decision);
coloring-scale ergonomics at extreme depth (auto-ranging the smooth
value); reference-orbit COMPRESSION for the store and the in-memory /
GPU mirrors (the Imagina/Zhuoran scheme FractalShark ships, its
FractalShark.pdf S6: store correction anchors where cheap f64
re-iteration of the reference drifts past tolerance, decompress per
pixel -- our raw hi/lo/exp layout is 20 B/iteration, 202 MB for the
f3 orbit, and lives in ~5 copies).

**Phase 5 — mode C, escape-time IFS + the bridges.**
Status (2026-08-25): the JFA distance-field bridge (§7.3) SHIPPED as
the `distance_field` color effect; BLA skipping and Newton
nucleus-finding both shipped inside phase 4 (see above). What
remains is mode C itself and the deferred tails:

- **Mode C (§6) is a project, not a tail item** — sequenced last,
  deliberately. Its prerequisites are its own scope: an
  invertible-variation registry (closed-form inverses on the defs),
  the largest-singular-value contractiveness extension (shared with
  flame-deep-zoom §7 — build it as common analysis, it has two
  customers), the bounding-disk fit, the ping-pong buffer pipeline
  with per-map layers for RIFS/xaos, and index-map coloring shipped
  WITH the mode. None of it blocks anything else in this plan.
- **Hybrid formula loops**: deferred — per-phase BLA validity is the
  open design question, and BLA just landed; revisit once the
  single-formula skip tables have soaked.
- **wasm worker**: deferred on infrastructure, not code — a
  SharedArrayBuffer worker needs COOP/COEP headers (a hosting
  decision) or a separate worker bundle; browser builds keep the
  synchronous reference path meanwhile.

---

## 5. Mode A catalog — iterate & classify

Per entry: iteration, test, coloring pairings, params beyond the
shared set (`max_iter`, `bailout`, Julia toggle + `c`, biomorph
toggle, scheme modifiers), and perturbation tier for phase 4
(**clean** / **diffabs** / **hard** / **none**).

### 5.1 Mandelbrot / Multibrot — `z ← zᵖ + c`
Integer `p ≥ 2`. Escape |z|² > bailout. All colorings. Perturbation:
**clean** (binomial; precompute `Zₙᵖ⁻ᵏ`). Param: `power`.

### 5.2 Tricorn / Multicorn — `z ← z̄ᵖ + c`
Conjugation is ℝ-linear. `power` exposed (multicorns, per *[ETI]*).
Perturbation: **clean**, but BLA coefficients are real 2×2 matrices —
adopt matrix BLAs from day one so the abs family shares the path.

### 5.3 Burning Ship family — abs-fold variants of 5.1
One formula, `variant` enum: Burning Ship, Perpendicular
Mandelbrot/Ship/Celtic/Buffalo — each a choice of component folds and
conjugation placement. Perturbation: **diffabs** (case analysis; under
rescaling it becomes `diffabs(XY/s, Xy + xY + sxy)`). *[mathr]*'s
deep-needle warning: the scaled form needs a full-range iteration
whenever *either* component of Z is small — near the needle that is
constantly, so floatexp-throughout often beats rescaled-with-branches
there; pick per region, don't force the ladder. BLA validity uses
min(|X|, |Y|) in place of |Z| (Fraktaler 3 halves it as a fudge) and
shrinks near the fold axes — single-step fallback.

### 5.4 McMullen family — `z ← zⁿ + c/zᵐ`
*The biggest prior omission* *[ETI]*. Rational maps with
Sierpiński-carpet Julia sets; heavily studied, visually distinctive,
trivial in a shader. Guard the pole (|z| ~ 0 → treat as escaped or
perturb seed). Perturbation: **clean-ish** (rational expansion);
phase 4 stretch. Params: `n`, `m`.

### 5.5 Lambda / logistic plane — `z ← λz(1−z)`
Conformally conjugate to Mandelbrot but the λ-plane layout is its own
classic. Pixel = λ. Perturbation: **clean** via conjugacy. Phase 2.

### 5.6 Phoenix — `z ← z² + c + p·z_prev`
`NeedsPrevZ`. Classic at Julia-mode `c = 0.5667, p = −0.5`.
Perturbation: **clean-ish** (2×2 block over (δₙ, δₙ₋₁)); stretch.
Params: `p_re`, `p_im`.

### 5.7 Newton / Nova / root-finder plane
**Convergent** (`|z − z_prev| < ε`, or per-root proximity). Colored
by root index + convergence speed. Nova adds `+ c` (Mandelbrot-like
parameter plane over the convergent core) and relaxation `R`. The
**scheme axis** (§3) generalizes this entry: Newton, Halley,
Schröder, Householder-3, secant, Chebyshev, König over `zᵖ − 1` (or a
small polynomial picker), with complex relaxation — the a-plane
galleries. "Root-finder Alloy" (Geisler) = alternating schemes, a
phase-5 hybrid-loop citizen. Perturbation: **hard** (convergent
rebase criterion — Imagina has prior art); last in line.

### 5.8 The tetration family — `w ← cʷ = e^(w·log c)`
The research finding that reshaped this entry *[ETI]*: Daniel
Geisler's names — Tower Julia, Tetration Star, Schröder's Basin,
Halley's Comet, Biomorph Tower, Root-finder Alloy, Oscillating Tower,
**Deep Tetration Web** — are gallery/feature names on ONE formula
family, not distinct algorithms. The axes:

- pixel = c (parameter space: the tetration fractal; "Tetration
  Star" and "Deep Tetration Web" are regions/zooms of it) or
  pixel = w₀, c fixed ("Tower Julia") — the standard Julia toggle;
- three-way classification: converge / escape (**test Re(w), not
  |w|**) / **period-k oscillation** — Geisler's signature coloring
  is by detected cycle length ("Oscillating Tower"); this entry is
  why the orbit summary carries a periodic(k) channel;
- scheme axis: direct iteration vs root-finders on `w = cʷ`
  ("Halley's Comet", "Schröder's Basin");
- biomorph toggle ("Biomorph Tower").

Implementation notes: `log c` once per pixel; **clamp Re(w·log c)
before `exp`** (overflow guard). f32 is fine — nobody deep-zooms
these; perturbation: **none**, by design. The former "Deep Tetration
Web open question" is hereby resolved: it is a preset (region +
high-`max_iter` + band-compressed coloring via palette log-strength)
over this family, shipped as a named preset, not a formula.

### 5.9 Exponential family — `z ← eᶻ + c`, `sin z + c`, `cos z + c`
`NeedsComplexExpLog`; escape on Re z (exp) / |Im z| (trig) — the
per-formula escape-test slot exists for these. Cantor bouquets;
average colorings shine. Perturbation: **hard**; direct-only until
proven wanted.

### 5.10 Magnet I / II
Rational maps; escape AND converge-to-1 both terminate. Perturbation:
**hard**. Phase 2, direct-only.

### 5.11 Littlewood parameter space — the root cloud
Pixel λ: bounded orbit of the greedy sign-choice map `w ← λ·w ± 1`
(`SignChoice`) ⟺ a ±1 power series vanishes near λ. The
parameter-space twin of the chaos-game `littlewood` variation, whose
module doc explicitly points here ("root-finding, not a chaos
game"). **Cross-check test** at landmark λ (twin dragon 1+i, golden
ratio): membership must agree with the variation's attractor
containing 0. Perturbation: **none** (bounded λ-annulus; f32
forever). Params: digit set ({±1}, {0,±1}, {±1,±i}), mirroring the
variation's `coeffs`.

### 5.12 Kaliset — `z ← |z|/⟨z,z⟩ ± c` (component abs)
`NonEscaping`: no bailout; colored **only** by orbit averages (the
classic glow is min-distance-to-axes with falloff) — the entry that
makes average accumulators phase-1 core. ~50–200 iterations.
Perturbation: **none** (self-similar at every depth). Convention
(sign, abs placement) pinned against reference images during
implementation.

### 5.13 Ducks / Kali-log (Monnier)

**Variants added 2026-08-28**: Softology's four variations (the
2011-04-06 post), as a `variant` parameter on the formula (0 classic;
1 log(sin(f+c)); 2 log(f+c - sec(f+c)); 3 log(sin(f)+c) -- the
reordered one; 4 log((f+c)^2)), all on the canonical upper fold.
Each variant's mean-|z| field sits at a different level, so the
coloring offset must be re-derived per variant (the sec variant at
the showcase c: field 1.86..2.05). `ducks-sec.fflame` pins variant 2.

**Animation export fixed 2026-08-28**: the video exporter ran the
flame chaos game regardless of render_mode -- an escape animation
previewed correctly in-app and exported the FLAME (field report).
The frame loop now branches: a persistent EscapeRenderer (orbit
cache and BLA table carry across frames) settles each frame through
bounded chunked dispatches exactly like the headless single-frame
path, and the tonemap reads the escape output; load_config already
kept the shared tail in sync per frame.

**The WASM in-app custom-size export had the sibling gap** (fixed
2026-08-28): one render() call sharing the tonemap encoder, i.e. a
single chunk, so a high-iteration export encoded whatever the first
dispatch reached -- and on wasm, where the reference orbit is sliced
per call rather than built on a worker, a deep zoom got one slice of
its reference too. It also never applied `escape.supersample`. It
now settles (chunk until final, each its own submission) and
resizes. Browsers have no blocking device poll, so the loop submits
without waiting, which is precisely where the adaptive chunk sizer's
wall-clock proxy LIES: with the queue never draining it measures CPU
encode time, reads every chunk as free, and grows into a
watchdog-length dispatch -- the browser's version of a TDR.
`EscapeRenderer::set_fixed_chunk` therefore pins the size to the
TDR-calibrated seed instead of feeding the feedback loop a bad
measurement; the render is chunk-invariant, and a GPU test asserts
adaptive and fixed are byte-identical on a supersampled view. The
WasmApi/gallery export and the desktop custom-size export both route
through the unified render API and already settled correctly (which
is why the wasm suite's escape category passed throughout).

**And the escape PARAMETERS were never applied to exported frames**
(fixed 2026-08-28, reported as "Escape.JuliaIm animates in-app but
the exported video doesn't morph"). `apply_config_value` -- the
export's own animation-apply path, separate from ConfigManager --
had no Escape arms at all, so every Escape.* track fell through to
its per-flame catch-all and was silently dropped. Palette and other
FractalConfig-level tracks kept working, which disguised it as a
partial failure. All animatable escape paths now have arms (Julia
toggle and seed, zoom, rotation, bailout, damping, max_iter,
supersample, formula/coloring params -- exactly what
`json_to_config_value` accepts), with ConfigManager's clamps
mirrored so an exported frame equals the in-app frame at the same
time. Both exporters share the function, so the RenderJob-based one
is fixed too. Pinned by a CPU-only test driving
`apply_animation_values` end to end: removing a single arm fails it.
Same bug family as the CameraX/CameraY and depth-effect arms whose
comments already sit in that function -- worth a sweep if another
config surface grows animatable paths.

**Corrected 2026-08-28 against the references** (Monnier's post
2011-02-27; Softology's variations post 2011-04-06), after a user
compared: c belongs INSIDE the log -- `z = log(Iabs(z) + c)`, not
`log(Iabs(z)) + c` as first shipped -- and the parameter plane seeds
z0 = 0, not the pixel. Monnier folds the lower half-plane UP
(Im <- |Im|); Softology's pseudocode folds down, the mirror image --
the author's fold is canonical. The reference coloring is the MEAN
OF |z| over the orbit (50-100 iterations), which no existing
coloring computed: `magnitude_average` (append-registered) is that
statistic, with an `offset` parameter because a Ducks julia field
can span ~0.2 around a mean of ~1.7 -- offset to the floor, scale
up, exactly the contrast normalization the reference images use.
Verified against an f64 numpy ground-truth probe of the reference
algorithm: our renders match its field structure (including the
genuinely-chaotic speckle zones -- not an f32 artifact), and the
showcase julia at c = (0.10, -0.62) reproduces the classic beaded
scaly look (`ducks-julia.fflame` pins it; `ducks-param.fflame` keeps
the home-view spiral/feather composition). — `z ← log(Re z + i·|Im z|) + c`
Half-fold then complex log (`NeedsComplexExpLog`, `AbsFold`).
Non-escaping; average colorings; spectacular with stripe-average.
Perturbation: **none**. Phase 2.

### 5.14 Feather — `z ← z³/(1 + z̄²·…) + c` and kin
Fractal Forums-era community favorites alongside Burning Ship
*[ETI]*. Pin exact convention against the originating threads during
phase 2. Perturbation: unassessed; direct-only.

### 5.15 Collatz — iterate the standard interpolation `¼(2 + 7z − (2+5z)cos πz)`
Obscure-famous conversation piece *[ETI]*. `NeedsComplexExpLog`
(trig). Escape on |Im z|. Perturbation: **none**. Phase 2.

### 5.16 Fractint legacy set
Spider (`z ← z²+c; c ← c/2+z`), Manowar, Barnsley M1–M3 (conditional
affine — escape-time renderings of IFS-like maps, ancestors of mode
C), Frothy Basin, Volterra–Lotka, Unity, Cactus *[ETI]*. Each is a
few lines of step body once the trait exists; big nostalgia coverage.
Phase 2, batched. Perturbation: **none**.

### 5.17 Novaretti — `z ← −6z(z³ + c) / (2z³ − c)²`
Community formula credited to Elena Novaretti (ZoneXplorer author);
circulated via a Reddit-era post and surviving reimplementations.
Degree-6 rational map, worked dynamics:

- **Nothing escapes**: numerator degree 4 < denominator degree 6, so
  ∞ ↦ 0 — `NonEscaping`; classify by convergence/**period
  detection**, never bailout.
- z = 0 is a fixed point with multiplier −6/c: attracting iff
  |c| > 6; other attracting cycles carry the |c| < 6 territory.
- f(ωz) = ω·f(z) for ω³ = 1 → **3-fold symmetric Julia sets**.
- **Closed-form critical points** for the parameter plane:
  z³ = c·(−7 ± 3√5)/4 — two essentially distinct critical orbits
  (the three cube roots are symmetry-equivalent); iterate both,
  classify by cycle. The poles 2z³ = c feed ∞ ↦ 0.

Colorings: orbit average (the circulating implementation colors by
accumulated Σ log|z|² — already in the coloring catalog), period,
traps. Guard the double poles like McMullen's. Perturbation: **none**
(convergent rational; direct f32). Phase 2. Convention pinned against
the reference images during implementation (Feather policy).

### Explicitly not in the catalog

- **Fragment Buddhabrot** — a density technique; see bridge §7.2.
- **Mandelbulb / Mandelbox / 3D DE** — ray-marched, a genuinely
  different pipeline (camera rays); separate plan; nothing here
  blocks it. The KIFS-DE tradition (§6, forward pointer) is its 2D
  doorstep.
- **Lyapunov** — *moved to mode B* (§4 phase 3), where it naturally
  lives; formerly parked, now unparked per *[ETI]*.

---

## 6. Mode C — escape-time IFS (the Hepting–Hart escape buffer)

*[ETI]* analyzed Hepting–Hart (GI '95) and the conclusion is adopted
wholesale: the escape buffer **is** a texture-feedback fragment
pipeline described a decade before GPUs could run it.

The algorithm: seed a float buffer with a continuous residual inside
a bounding annulus, then iterate the image-space fixed point

```
E(x) ← max(E(x), maxᵢ E(Sᵢ⁻¹(x)) + 1)
```

as fullscreen ping-pong passes (Jacobi form). Pass count is
`⌈log(p/R)/log λ⌉` for precision p and max Lipschitz λ — ~12–13
passes at 4K for λ = 0.5; milliseconds total. Output: a continuous
escape-time field over the attractor's exterior → every mode-A
coloring (bands, DE-style shading, traps) applies to **IFS
attractors**, rendered solid and antialiased instead of as density
clouds.

Why this algorithm and not per-pixel inverse-tree traversal:

1. **Robust to open-set violation** — overlapping transforms are the
   flame aesthetic norm, and overlap breaks the tree methods but not
   the buffer.
2. **Zero divergent control flow** — N coherent texture fetches per
   pixel per pass; the tree is a warp-divergence nightmare.
3. **RIFS = xaos, structurally**: the recurrent form keeps one buffer
   layer per map and gathers along graph edges — a texture array with
   per-layer masks. Xaos-linked flames drop straight in.

Scope and caveats, recorded honestly:

- **Invertible maps only**: affine + variations with closed-form
  inverses (spherical, swirl, Möbius, polar qualify; sinusoidal and
  most folds don't). This mode covers the invertible subset of flame
  IFSs, not arbitrary flames — still large and interesting.
- **λ → 1 blows up pass count** (near-isometries need hundreds).
  Viability check per flame: needs the **largest singular value**
  (Lipschitz) per transform — the existing contractiveness machinery
  is determinant-flavored, so this is a small extension of it, and
  the number gates whether the mode is offered for a given flame.
  **Second customer:** the flame deep-zoom plan
  ([flame-deep-zoom.md](flame-deep-zoom.md) §7) consumes the same
  extension three ways — build it as shared analysis, not
  mode-C-private.
- **Bounding disk** `T(D_R) ⊂ D_R` — conservative bound needed with
  nonlinear variations; same machinery.
- **wgpu wrinkle**: bilinear sampling of the buffer wants
  `float32-filterable`, an optional feature. Fallback: manual 4-tap,
  or `r16float` (values are small integers + [0,1) residual — fp16
  suffices). Decide at implementation.
- **Index maps** (which transform won at each level) give
  symbolic-address coloring — the escape-time analogue of xform
  color, and what makes these renders read as structured. Ship it
  with the mode, not later.

Forward pointer, deliberately not lost: the paper's open wish — a
distance version — was answered by the **KIFS/fold-space DE**
tradition (knighty et al.), which is the modern single-pass form and
the eventual successor if mode C earns love. And by **jump
flooding**, which is bridge §7.3.

---

## 7. Bridges between the two engines

The deep statement *[ETI]*: contractive IFS and expanding dynamics
are inverse descriptions of the same objects; each engine samples a
different measure on them. Concrete, buildable bridges:

1. **Inverse iteration (IIM/MIIM) — escape-time objects as flame
   content.** A Julia set is the attractor of the inverse IFS
   {±√(z−c)}; the `julia`/`julian`/`juliascope` variations already
   ARE random-branch inverse maps. Generalizing: any rational map's
   inverse branches as a variation family, with MIIM-style
   derivative-based branch weighting to fix the balanced-measure
   starvation. A novel-ish flame feature; backlog, unphased.
2. **Escape-time → density: generalized Buddhabrot.** Render mode-A
   *orbits* (not classifications) into the existing flame
   accumulation histogram — Buddhabrot for every formula in the
   catalog, reusing the histogram machinery rather than a per-pixel
   scatter. Phase 5+, and the honest home of the old "fragment
   Buddhabrot" idea.
3. **Flame → distance field via JFA.** Chaos-game-render the
   attractor as a seed mask; jump-flood it into an exterior distance
   field in O(log n) fullscreen passes; then every DE coloring
   (shading, contours, traps) applies to **arbitrary flames** —
   invertibility not required. Nearly free, independent of every
   other piece of this plan, and could ship in any branch as a
   standalone win.
4. **Post-warps**: fragment output as a texture warped by flame
   variations in image space; fragment fields as image traps in
   mode-A coloring. Cheap experiments once both engines exist.

---

## 8. Coloring & trap catalog

All consume the orbit summary; all map to **palette position** and
inherit rotation/squeeze/reverse/log-strength free.

| coloring | consumes | notes |
|---|---|---|
| Escape count (banded) | n | log-compress via palette log-strength |
| Smooth iteration | n + fraction | default for escaping formulas |
| Orbit trap point/circle/cross/line/shape | min (or avg) trap distance | composable with any formula; small SDF enum |
| Orbit average (Kali glow) | running mean of trap fn | REQUIRED for NonEscaping; optional everywhere |
| Stripe average | mean sin(k·arg z) | phase 2 |
| Triangle inequality | per-step TIA term | phase 2 |
| Root basin | root index + speed | Convergent formulas |
| Period / interior | cycle length k | tetration, Mandelbrot interior |
| Distance estimation | |z|, |dz/dc| | phase 2; derivative orbit perturbs identically |
| Field value / gradient | mode B | value bands; analytic-gradient hillshade |
| Lyapunov exponent | mode B | signed scalar → diverging palette convention |
| Index map / symbolic address | mode C | per-level winning transform |

Two-channel emission (position + intensity → rgb·a HDR) so tone
mapping has something real to do.

## 9. Testing

- **Visual corpus**: one config per formula at landmark coordinates;
  no RNG anywhere in modes A/B → baselines are exact. Mode C adds a
  pass-count-vs-λ case near the viability boundary.
- **Compile probe**: every formula × scheme × classification ×
  coloring combination assembles and parses (the crackle_fast
  lesson: name-gated helper injection is where this breaks).
- **Fixed-point (phase 4)**: differential vs f64 at 1–2 limbs;
  published deep-zoom orbits; ring axioms; decimal round-trip;
  CPU↔GPU floatexp boundary agreement; **lzcnt export exactness for
  near-zero values** (close reference passes are where rebasing
  lives); truncation drift over a long orbit stays inside the guard
  limb.
- **Littlewood cross-check** (§5.11) against the chaos-game
  variation.
- **Tetration**: Shell–Thron boundary landmarks; period coloring at
  known cycle regions.
- **Contract**: `render_mode` vocabulary change coordinated with the
  API repo before release.

## 10. Bloat ledger

Genuinely new: the fragment stage + bind groups (one single-pass, one
ping-pong for mode C), EscapeConfig, three registries + assembler
(patterned on the variation builder), one panel, input routing, the
Lipschitz extension to contractiveness, and in phase 4 the fixedpoint
module + orbit cache. Reused wholesale: palette, tonemap, Levels,
curves, effects chain, export, thumbnails, undo/ConfigPath, animation
tracks, scripting, docking UI, pipeline LRU, visual harness. **Zero
new dependencies.**

## 11. Open questions

1. **Mode naming** in the UI ("Escape Fractals"? "Deep Fractals"?
   "Fragment Fractals"?) — user's call.
2. **Center-path animation** (Misiurewicz walks) — needs waypoint or
   string-interpolation machinery tracks can't express; parked.
3. **Kaliset / Feather formula conventions** — pin against reference
   images during implementation (§5.12, §5.14).
4. **Bagula "Mandelbrot Cartoon" bug-or-intent** — the published
   notebook passes `x` twice, making its surfaces 1D fields in
   disguise *[ETI]*; decide deliberately whether the field-mode
   preset reproduces the published images or the evident intent
   (probably: both, as two presets).
5. **`float32-filterable` fallback** for mode C (manual bilinear vs
   r16float) — measure, then decide.
6. **Whether the `julia` effect deprecates** once mode A ships.
7. **Iteration-governor reuse** for heavyweight combinations — wire
   only on demonstrated need.

~~Deep Tetration Web reference convention~~ — resolved: a preset
region/zoom of the tetration parameter space (§5.8), per Geisler's
atlas naming; not a distinct algorithm.
