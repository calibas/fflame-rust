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

**Phase 5 — mode C, escape-time IFS + the bridges.**
The Hepting–Hart escape buffer (§6) with RIFS/xaos layer support and
index-map coloring; the JFA distance-field bridge (§7.3) — which is
independent of everything above and can land whenever wanted; BLA
skipping and Newton nucleus-finding from phase 4's backlog; hybrid
formula loops.

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

### 5.13 Ducks / Kali-log (Monnier) — `z ← log(Re z + i·|Im z|) + c`
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
