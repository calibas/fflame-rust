# Escape-Time, Field & Orbit-Trap Fractals

**Status:** SHIPPED and live on the `escape-time` branch — 25
formulas, 14 colorings, 2 field formulas, deep zoom by perturbation
with big-float references, relief shading, auto contrast, and the
whole shared tail (palette, tone mapping, effects, export, animation,
scripting, online sync). This began as a plan and is now the design
record: the dated sections below are the history of how each piece was
built and what was measured, and they are the reason a change here can
be judged against what was already tried.

The remaining work is listed in **What is left** immediately below.
The per-item completion tracker, the new-families research and the
seed document for the third fractal family are finished and archived
under [docs/archive/escape-time/](../archive/escape-time/).

## What is left

Everything else in this document is built. These are not:

- **Perturbation tiers for the transcendental formulas** —
  `tetration`, `exponential`, `trig`, `collatz`. The deltas exist
  (`exp(Z+d) = exp(Z)·exp(d)`); each needs its own error analysis.
  Ducks' trig variants 1-3 (Secant among them) sit here too: they
  want big-float `sin`/`cos`/`exp` on the reference side, which is
  comparable groundwork to the `ln`/`atan2` already done.
- **Chebyshev over the two non-unity Newton functions** DECLINES the
  tier, by measurement rather than omission: its `r` is
  asymptotically constant, so the quotient rule loses the delta, and
  the symbolic cancellation that fixes it exists only for `z^p - 1`.
  Recorded so it is not re-attempted blind.
- **Series approximation** — absent, and still unmeasured against
  BLA. Worth a Phase-0 measurement before any building.
- **Kaliset's zoom-24 floor** — it perturbs correctly above zoom 24
  and declines below it, because the inversion amplifies the delta
  faster than the rung holds. The panel's depth hint does not yet
  express that nuance (it reads "unlimited" from zoom 14).
- **Row banding of the perturbed path** — would let a frame exceed a
  device's buffer limit by sizing the per-pixel state to a band, as
  the direct path already does via `tile_y0`. Only needed where a
  device's real `max_buffer_size` is under the frame's state (398 MB
  at 4K); accumulation covers the antialiasing case. Not built.
- **Temporal antialiasing / spectral rendering** — motion blur and
  wavelength-correlated fringing on a zoom. Planned, never scheduled;
  it is an export-loop feature, not a shader one, and wants its own
  plan. Full costing in the archived new-families research (§7).
- **GPU reference-orbit computation (the NTT direction)** —
  measurement-gated go/no-go, never run. See
  [escape-ntt-reference.md](../experimental/escape-ntt-reference.md).
- **`timeline_of_a_cached_revisit`** fails (148 vs 197 limbs) and has
  since before 2026-09-02; verified pre-existing against a stashed
  tree. Unrelated to the tiers.

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

### A golden spiral orbit trap (2026-08-29)

`orbit_trap` gains shape 3, a logarithmic spiral, golden by default —
the first trap here that is not a point, a cross or a circle. It
composes with all 23 formulas, which is why it was worth doing before
adding a 24th (see
[escape-new-families.md](../archive/escape-time/escape-new-families.md)).

The form is Nylander's:

```
r = log|z| / (4 log g)  -  arg(z) / 2pi        d = |r - round(r)|
```

`g` per QUARTER turn is what makes a logarithmic spiral golden when
`g = phi`, hence the 4; `r` is "how many turns out along the spiral
this point sits", so the distance to the nearest arm is how far `r`
sits from a whole number. `growth` exposes `g`, so the golden spiral
is a default rather than a hardcoded constant. Unlike the other three
shapes the result is in TURNS, not z-units, doubled to span 0..1.

Both hazards were known in advance and are guarded at the origin,
where `atan2` is the Metal fast-math trap (pi/4 for same-sign zeros,
NaN for mixed) and `log|z|` is minus infinity. The escape engine's
idiom for this is an explicit `dot(z,z) < 1e-30` branch rather than
the flame side's `ff_atan2`, matching `esc_clog` and
`stripe_average`; the branch returns a huge distance, which leaves the
running minimum untouched — "this sample says nothing".

**How it was verified, which is the reusable part.** At
`max_iter = 1` the orbit is exactly {0, c} and the origin sample is
skipped, so THE IMAGE IS THE TRAP DISTANCE FIELD: a closed form,
checkable with no iteration in the way. The test bins pixels by the
f64 distance and measures colour spread WITHIN a bin, which ignores
the palette and asks only whether the render is a function of the true
distance. It reads 0.40/255. Changing the quarter-turn 4 to a
half-turn 2 sends it to 68.39/255 — and that version still looks like
a perfectly good spiral, which is exactly the failure a screenshot
cannot catch.

### Normal-map shading, and a wrap that only the eye caught (2026-08-29)

`normal_map` lights the set like a relief. The normal comes from the
DERIVATIVE rather than from neighbouring pixels: Chéritat's
construction pulls the radial direction back through `dz/dc`, giving
`u = z/dz`, and the reference C (Wikimedia Commons, behind Wikibooks'
bump-mapping article) is ported unchanged:

```c
u = Z / dC;  u = u / cabs(u);
reflection = cdot(u, v) + h2;          // v = exp(2 pi i angle)
reflection = reflection / (1.0 + h2);
if (reflection < 0.0) reflection = 0.0;
```

`angle` and `height` are exposed; the defaults are that snippet's own
45 degrees and 1.5. Nothing here calls `atan2` — the light direction
comes from an angle in turns — so the Metal zero-pair hazard cannot
arise at all.

**Where it does NOT work, said out loud.** The perturbed rungs never
iterate a derivative (`dz` is a constant seed there), and 12 of the 23
formulas define none. In both cases `z/dz` would be `z`, and the
shading would be a smooth function of `arg(z)`: convincing relief that
encodes nothing about the surface. A new `HAS_DERIVATIVE` constant
tells the coloring which case it is in, and it returns FLAT light
instead — visibly unshaded beats plausibly wrong. `distance_estimate`
had the same exposure and was left silent at the time; it has since
been guarded too (see below).

**The bug worth remembering.** The first render had a thin black seam
through the bright quadrant. The template wraps every coloring's value
with `fract` so unbounded ones (escape count, smooth) cycle through
the palette as they grow — but a bounded value of exactly 1.0 wraps to
0.0, so the points whose normal aims straight at the light rendered as
the palette's darkest colour. A new `ColoringFeature::Bounded` clamps
instead of wrapping.

It is worth noticing what caught it. The numerical check — bin pixels
by the reference reflection, measure colour spread within a bin — read
1.74/255 WITH the bug and 1.38 without: it could not see a defect at
one value out of a continuum. A person looking at the picture saw it
immediately. The test now asserts both: the shading matches its
reference, AND no lit pixel is near-black.

### `lambda_sine` — the Cantor bouquet family (2026-08-29)

`z <- lambda*sin(z)`. NOT `sin(z) + c`, which is the separate `trig`
formula: the parameter MULTIPLIES, and that is the whole difference
between a nice fractal and the named one. The literature is specific
(Pardo-Simón, arXiv:2209.03284, after Devaney–Tangerman): *the Julia
set of any λ·sin(z) with λ in (0,1) is a Cantor bouquet* — a Cantor
set of disjoint HAIRS, each an arc to infinity, escaping except at
the endpoints. Julia mode with a real λ in (0,1) is where they live.

**The parameter plane seeds at π/2, not zero,** and that is
load-bearing rather than stylistic: `sin 0 = 0`, so zero is a fixed
point for EVERY λ, and a zero-seeded plane renders one flat colour
with nothing anywhere to say why. π/2 is where `cos z = 0`, so it is
the critical point, and its critical value is λ. `LAMBDA` (the
logistic map) seeds at 1/2 for exactly the same reason.

Escape is `|Im z| > bailout` RAW, shared with the rest of the trig
family: sin grows like sinh in the imaginary direction, so orbits
leave through ±i∞ and the bailout wants to be ~50, not 4.

Verified against an f64 orbit at λ = 0.5: **0.00% of pixels differ**,
and making the parameter additive — the exact mistake the name warns
about — sends that to 14.45%. The render is also 2π-periodic in Re z
to 99.5% (pixel rounding) and mirror-symmetric about the real axis to
100%, both properties of the map rather than of the code. A separate
CPU test iterates the map from both seeds so the π/2 choice fails
loudly, with its reason attached, if anyone ever "normalises" it.

### `origami` — McCabe's Butterfly Origami, with two corrections (2026-08-29)

Fold the plane along a sequence of lines, average over the image
points. algorithmic-worlds records the algorithm: *"choose a number of
random lines cutting the square and order them. For each point of the
square, compute its images under the sequence of mirror symmetries
about the sequence of random lines. Then color the result by
performing an average over all the image points."* Structurally this
is Ducks — iterate, and let an averaging coloring do the work — which
is why that page says the results resemble the Duck-like algorithms.

**The reflections must be CONDITIONAL, and that is a correction to the
wording made on evidence.** An unconditional reflection is an isometry,
so a composition of them is affine and the average of |z| over the
sequence is smooth. Prototyped in numpy before any Rust was written,
that reading renders as plain concentric rings with no structure at
all; folding — reflecting only the points on one side, which is what
"origami" means and what the widely repeated paraphrase ("fold a piece
of paper, project an image onto it, unfold") describes — produces the
creases and wing lobes. Both prototypes are in
[escape-new-families.md](../archive/escape-time/escape-new-families.md).

A second thing the prototypes exposed: the creases are DERIVATIVE
discontinuities, so a smooth greyscale ramp hides them almost
completely. They only became visible under a cycling palette, which is
how the engine maps values anyway. A first look at the greyscale
render nearly sent this to the "does not reproduce the reference"
pile.

**One fold per iteration**, along line `i mod lines`, so `max_iter` IS
the length of McCabe's sequence and the accumulator sees every image
point — which is exactly what "average over all the image points"
asks for. That needed a new `FormulaFeature::NeedsIndex`, which
appends the loop counter to the step's signature the way `NeedsPrevZ`
appends the previous iterate. Folding K lines inside one step instead
would show the coloring only every Kth point, and the orbit freezes
the moment it reaches the region every fold leaves alone (measured:
with 6 random lines, 0% of pixels were still moving after the first
pass, and the image was identical at 4, 40 and 200 iterations).

**The line hash is INTEGER on purpose.** The usual
`fract(sin(x) * 43758.5)` idiom multiplies a sine by a large constant,
which amplifies rounding enough that f32 and f64 disagree — and so can
two GPUs. That would have made the line arrangement, and therefore the
whole image, device-dependent, which nothing else in this project
accepts. Integer ops are exact and immune to fast-math, and the 24
bits taken from the hash are exactly what an f32 mantissa holds.

Verified against an independent reimplementation of the whole
algorithm — hash, lines, folds, running mean — at **0.39/255**, and
the test separately asserts the creases EXIST, because "matches the
oracle" would pass if both were smooth washes. Making the reflection
unconditional sends the agreement to 73.32/255.

**A third correction, from a user looking at the render: colour comes
from the average POSITION, not the average magnitude.** McCabe colours
each point by "a weighted average of that list of positions" — a 2-D
vector — and the report was that our image "seem[ed] kinda like a
kaleidoscope", not like the published work. It was: `magnitude_average`
collapses the orbit to a mean |z| first, and concentric contour rings
are what that produces. Reprototyped both in numpy side by side
(`output/origami/c-magnitude.png` vs `c-angle.png`): the ANGLE of the
mean position carries the creased, layered-paper seams, and the
magnitude carries the rings. Shipped `position_average` — accumulates
`state + z`, divides by the iteration count, and maps either the angle
(default) or the length. The shipped config moved to it.

Two things about it are worth writing down. The average is
UNWEIGHTED, where McCabe weights each fold; a per-step weight needs the
iteration index inside the coloring accumulator, and only the formula
side has that today (`FormulaFeature::NeedsIndex`). And the source's
full mapping drives hue AND brightness from the one vector, which a
1-D palette cannot do — reaching it needs a coloring that writes RGB
directly, which the escape template has no path for.

Its `scale` matters more than for other colorings: the mean position
sweeps only a narrow arc across a typical view — measured at **0.09 of
a turn** on the shipped config — so at `scale = 1.0` the whole image
lands in one slice of the palette and reads as flat. The shipped
config uses 11.6, which is 1/0.086.

The GPU test asserts BOTH directions, because either alone is weak: the
render must track an f64 average-position oracle, and must NOT be
explained by the mean magnitude (0.37 vs 58.66/255 on the geometry of
the day; 0.44 vs 44.23/255 after the wad correction below).
Reintroducing the bug — accumulating `length(z)` again — flattens the
image so completely that the first assertion passes on a constant
picture; it is the ratio that catches it.

**Zooming was measured dead past zoom ~5, and the section that stood
here blamed "at most O(F) affine pieces". Both the diagnosis and the
shipped geometry were wrong — see the fourth-correction section below,
which replaces this one.** What was real in the measurement: with the
fold lines FIXED in the plane, later folds miss the shrinking wad
entirely (0% of pixels moving), the crease count stays O(F) in
practice, and crease density fell 0.80 at zoom −1 to 0.000 by zoom 5
even with 256 folds. That is a symptom of the wrong line placement,
not a property of folding.

### `origami`, corrected again: fold the wad, not the sheet (2026-08-29)

The user compared the render against McCabe's published work
("Origami Butterfly 8": dense ornamental chains, rosettes, lace,
folds-on-folds) and it clearly wasn't it — different character, and
no depth. The prose descriptions of the algorithm were exhausted, so
the fix came from code archaeology: the only public implementation
found is **Kyle McDonald's Processing port** (OpenProcessing sketch
1185; the modern page hides source behind a membership, but the
[2012 Wayback
snapshot](https://web.archive.org/web/20120209174422/http://www.openprocessing.org/visuals/?visualID=1185)
embeds it in full). It contains the one detail
every prose description omits:

```java
folds[i] = { randomPosition(i), randomPosition(i) };
float[] randomPosition(int level) {
    return foldPoint(random(w), random(h), level);  // folded through
}                                                   // the previous folds!
```

**Each fold line's endpoints are themselves folded through all
previous folds**, so every new crease is guaranteed to land on the
current wad — real paper folding: you fold the wad you are holding,
not the original flat sheet. With that construction every fold stays
active (measured 8%–92% of pixels reflected on each of 32 folds,
against 0% for late fixed lines) and the crease count compounds
toward 2^F. The folds-on-folds look appears immediately; 32 folds is
McCabe's number, and MORE is smoother, not deeper (96 folds measured
visibly softer — the wad keeps shrinking under the endpoint domain).

McDonald's colouring is also position-based but not an average: the
final folded position looks up a SOURCE IMAGE ("project an image onto
the paper, like tie-dye, then unfold"), normalized to the wad's
bounding box. A per-pixel shader cannot do the global normalization,
but the source is periodic so an affine remap is only a
frequency/phase change: `position_map` projects a two-sine plasma at
the orbit's final position (`freq_x`, `freq_y`). That coloring
reproduced the bead-chain-and-rosette ornament of the reference on
the first render.

**The true zoom limit, replacing the O(F) story.** A fold map is
CONTINUOUS — creases are derivative kinks, not value jumps — and
every fold is an isometry, so the final-position field is
Lipschitz-1: inside a window of size w, landing positions vary by at
most w. Any fixed smooth colour source therefore washes to a constant
as the window shrinks, regardless of how good the fold geometry is;
measured on the corrected geometry, smooth-source contrast still died
by zoom ~5. The piece count is irrelevant to this; it is the
smoothness of the colour source that caps the depth.

What survives zooming is the DISCRETE channel: which folds reflected
the point. That is piecewise-constant with a jump at every crease, so
its contrast does not decay with window size — only crease density
does, and with 2^F facets it thins gradually instead of vanishing.
`position_map.address_mix` blends it in: the coloring accumulator
builds a binary branch address (0.5 per moved step, halved each
iteration; `moved = any(z != z_prev)`, reliable because an unmoved
point returns bit-exact — and it is z against z_prev, not a
fast-math-hazard self-compare). Measured, following crease
intersections: address-edge density 0.75 at zoom 0, still 0.01 at
**zoom 22**, against nothing past zoom 5 for the smooth channel. GPU
renders confirm crisp structure at zoom 6 and clean facet boundaries
at zoom 10. The f32 accumulator keeps the last ~24 branch bits — the
fine ones; the coarse folds are already visible in the smooth part.

`lines` (the cycling line count) is gone from the params — the fold
count is `max_iter` (capped at 64), which is what it always was in
the source algorithm. The lines are cached per invocation in a
`var<private>` array built incrementally (line j folds its endpoints
through lines 0..j-1, O(F²) once per pixel per dispatch; WGSL
zero-initializes private variables, so resumed chunks rebuild
correctly). The GPU oracle tests both reimplement the wad-relative
construction in f64: the shipped-geometry agreement is 0.51/255, and
deleting the endpoint folding from the shader alone sends it to
71.30/255, so the construction is what the test pins, not just the
fold formula.

### Relief shading — a layer, not a coloring (2026-08-30)

Asked for as "fake depth shading ... something that applies on top of
other palettes and/or coloring modes", with colour, strength and blend
mode for shadows and highlights separately, and with the observation
that it ought to work with `position_map` because the published
Origami images clearly have it.

**Why it could not be a `ColoringDef`.** A coloring returns one scalar
which the template maps through the palette into RGB, so colorings
replace each other by construction; there is no way for one to
decorate another. That is not a limitation to work around, it is why
`normal_map` — the analytic-normal coloring — takes over the image
instead of shading it. Relief therefore runs AFTER the palette lookup,
on the finished RGB, which is what makes "on top of any coloring" true
by construction rather than by effort.

**Where the surface comes from.** Every escape template now writes the
coloring's scalar value to an R32Float target alongside the colour,
and the relief pass takes central differences of it. That choice is
what makes the feature universal: no derivative is involved, so it
works on the perturbed rungs (where `dz` is a constant seed) and on
the 13 of 25 formulas that define no derivative at all — Origami
included, which is the case that prompted it.

The height target is bound as a 1×1 dummy while shading is off and
the templates store to it unconditionally. WGSL discards
out-of-bounds `textureStore`, so the cost of always writing is one
dead store per pixel, and there is ONE shader variant rather than two
kept in step by a flag. Off, the field costs 16 bytes; on, 4 bytes per
render pixel (81 MB at 4500²).

**The pass is fused into the supersample resolve.** Shading needs a
destination distinct from the colour it reads, and a third
full-resolution RGBA32Float target would have cost another 324 MB at
4500² on top of the colour's own 324 MB — the kind of allocation that
produced the TDR device-loss earlier in this branch. Folding it into
the pass that was going to downsample anyway costs nothing extra, and
it puts the shading BEFORE the box average, which is the correct
order: the slope is measured at render resolution and antialiased
along with everything else, rather than computed from already-blurred
pixels. With shading off and no supersampling the pass does not run at
all, and all 215 existing visual baselines stayed byte-identical,
including the supersampled ones.

**Two normalizations that are not cosmetic.**

*Flat must mean untouched.* The response is the signed tilt toward the
light — the slope along the light direction, divided by
`sqrt(1 + |grad|²)` to make it `sin(tilt)` — so it is exactly zero on
flat ground and both terms start there. The GPU test asserts it: the
interior of the set has no value field at all, and must come through
the light unchanged.

*Shadows that could not get dark, and why two fixes were needed.*
Reported from the app: at relief height 50, black shadows at strength
1.0 were barely visible while white highlights at strength 0.03 looked
about as strong. The response was already symmetric (the mirror test
pins that), so the asymmetry had to be downstream, and it turned out
to be two separate things.

The first is a real bug. The blends ran on LINEAR light -- the escape
pass raises the palette to 2.2 on lookup -- and in linear light a dark
base gives `multiply` almost nothing to take away while `screen` has
the whole range to add into. Measured on a mid-tone relief config, a
full-strength black shadow moved 22.45/255 against a white
highlight's 52.53: 2.3x, from a pair of controls that read as
identical. Blending in a perceptual space instead brings that to 1.4x,
and fixes the layer COLOUR as a side effect -- it arrives from the
picker in sRGB and was being composited against linear light, so any
shadow tint that was not pure black was landing in the wrong place.

The second is not a bug and cannot be fixed by any choice of blend
space: DYNAMIC RANGE. A pixel sitting near black is about 0.18
perceptual units from black and 0.82 from white, so the same `amt`
moves it far further up than down. On a deliberately dark palette the
gap is still 9.3x after the perceptual fix. What was missing there is
headroom, so both strengths now range to 4 rather than 1 -- past 1 the
layer saturates sooner rather than travelling further, which is
exactly what a shallow slope on a dark image needs. `amt` is clamped
before the mix, since an unclamped lerp past 1 would extrapolate
through the layer colour into negative light.

Measured on that dark palette (unshaded mean 20.1/255): a black shadow
takes it to 12.3 at strength 1 and 10.6 at strength 4. The test
asserts that rather than an equal bite, because an equal bite is
something a dark image cannot deliver and asserting it would have
meant either a false failure or a meaningless threshold.

*Softer edges are a third control, and the first attempt at it was
wrong in a way worth recording.* It widened the difference stencil to
a ring of radius r. That is not a blur: every tap stays a single
sharp sample, so estimating the slope at p from the height at p±r
DISPLACES the structure and prints ghost copies either side of every
edge — reported from the app as the relief "mirroring into 3 equally
sharp parts". It shipped because the tests then in place measured
only that high-frequency content went down (it does, for the wrong
reason) and that the colour was untouched (it was).

Softness is now a Gaussian sigma in display pixels, applied as a
separable two-pass low-pass of the HEIGHT FIELD, after which the
shade pass takes its plain ±1 difference. Separable because the
radii are large — softness 8 at 3× supersampling is 24 render pixels,
a 49×49 window — so two passes of 2r+1 taps replace one of (2r+1)².
It is also continuous now rather than an integer stencil radius,
which made the old control coarse as well as wrong.

The composite could not settle whether a given implementation ghosts:
the shading term extracted from it is nonlinear (perceptual blend,
then gamma), and two composite-level metrics — correlation against a
shifted copy, and a commutation check — both ranked the ring stencil
BETTER than the fix. So the test checks the mechanism where it is
exactly checkable: the blurred height field must equal a Gaussian
blur of the raw one computed independently on the CPU. That found a
second bug immediately — both blur passes shared one uniform buffer,
and `Queue::write_buffer` is ordered against SUBMISSIONS rather than
against passes inside one, so the second write won and both passes
blurred vertically. It now reads rms 0.00000, worst 0.0000003 of the
field's range.

*Surface texture* is a micro-relief lit by the same light: Grain (one
octave of isotropic value noise) or Paper (octaves stretched along
different axes, so it reads as fibre laid in a felt rather than as
speckle). It is added to the surface TILT rather than to the height,
which is what keeps it independent of both the coloring's value scale
and the relief depth — added to the height it would be multiplied by
`height` along with everything else, and a strength that read well at
height 50 would be a sandstorm at 1000.

*And the two sides must be symmetric.* The first version used a
Lambert dot product with each side normalized by its own achievable
span, and that shipped a bug the user caught immediately: black
shadows at full strength over a white palette came out **mid-grey**
while highlights were fine. The normal's z component is always
positive, so `dot(n, l)` can never fall below `-|l.xy|` = −0.707,
while the highlight side was normalized over a span of just
`1 - l.z` = 0.293. Measured across tilt angles: at 45° the highlight
was already saturated at 1.000 while the shadow had reached 0.414 —
and 0.414 of black over white IS mid-grey. Worse, the Lambert term is
non-monotonic in tilt: a vertical wall facing the light got **no**
highlight at all, because the dot product peaks at 45° and falls back
to zero by 90°.

The signed-tilt response has none of that: it runs −1..+1, is
monotonic, and is symmetric by construction. Measured on the same
scene, the darkest full-strength black shadow went from 120/255 to
12/255. The test pins it without a magic threshold — flipping the
light 180° negates the tilt, so a shadow-only render at angle A must
be BIT-IDENTICAL to a highlight-only render at A+180 with the same
colour, strength and `mix` blend. Restoring the Lambert version fails
that equality.

*Relief is per DISPLAY pixel, not per render pixel.* The difference is
taken on the supersampled grid, where a given slope spans `factor`
times as many samples and would read `factor` times shallower — so
turning on 3× antialiasing would quietly flatten the relief. The
uploaded height is scaled by the factor to cancel it.

**The `height` control is honestly coloring-dependent.** The slope is
in palette turns per pixel, which IS a normalized unit — every
coloring's value is in turns, since that is what the palette cycles
on — but colorings differ by orders of magnitude in how many turns
they spend across a view. `smooth` on the Mandelbrot at scale 0.03
runs about 0.04 turns/px; a bounded coloring runs far less. So the
slider is logarithmic over 0.01–1000 and the default is 10, which puts
both of those into visible relief. `normalize()` on the surface normal
doubles as the saturation, so an over-large value tips the normal flat
against the surface rather than blowing the lighting out.

**Two source fields, because they are two different pictures.**
`Smooth` slopes the coloring's raw value; `Banded` slopes the wrapped
palette coordinate, so every band edge becomes a step — the engraved,
contour-map look. Worth knowing what `Smooth` does on an escape count:
the field is a global ramp toward the set, so its relief is a lit
dome, with a straight shadow terminator across the image. That is
correct terrain relief of the escape-time surface and it looks like
what it is; the fractal detail rides on top as a small perturbation.

**The analytic normal was dropped, on measurement.** The plan said to
prefer `dz` where it exists and fall back to differences. Carrying it
would mean RGBA32Float instead of R32Float for the height target — 16
bytes per render pixel against 4, so 324 MB against 81 MB at 4500² —
and a rendered comparison on the Mandelbrot (`output/origami/
m-compare.png`: plain, relief at two heights, `normal_map`) shows the
field-gradient relief reading as the same surface. The analytic look
remains available as the `normal_map` coloring for anyone who wants it
exactly. Revisit if a case appears where the difference is visible.

### `distance_estimate` stops pretending (2026-08-30)

The deferred half of the `normal_map` work, finished. `d = |z|·ln|z| /
|dz|` needs a derivative orbit; without one `dz` is the constant seed
of 1 and the formula collapses to `|z|·ln|z|` — a smooth function of
the escape radius alone.

**That failure was invisible because the wrong answer is beautiful.**
Rendered side by side (`output/origami/de-deep-compare.png`), a
Mandelbrot dive to zoom 30 gave a fully detailed, entirely convincing
deep-zoom picture with 516 distinct colours in the exterior — and not
one pixel of it was a distance estimate. Nothing about the image
invites suspicion, which is exactly the class of bug this project
keeps deciding is worse than a broken-looking one.

It now returns a flat 0.5 when `HAS_DERIVATIVE` is false, matching
`normal_map`'s flat light. 0.5 rather than 0.0 on purpose: pixels that
do not escape are painted by the template rather than by the coloring,
so returning the palette's bottom would blend the exterior into the
interior and read as "everything is in the set" instead of "this
coloring is unavailable".

**Two cases reach it**, and the second is the one that surprises: the
13 of 25 formulas that define no derivative, and EVERY perturbed
render — the deep rungs do not iterate a derivative orbit, so a
Mandelbrot dive past `PERTURB_MIN_ZOOM` loses its derivative even
though the formula has one. Both existing visual configs
(`mandelbrot-de`, `lambda-de-julia`) are shallow views of
derivative-bearing formulas, so no baseline moved.

**Flat is honest but silent, so the panel now says why.**
`EscapeRenderer::derivative_gap` answers "formula", "deep path" or
"none", and the escape panel shows the matching sentence under the
coloring selector whenever the active coloring declares
`NeedsDerivative`. It is pinned by assembling the real shader for
every formula and reading `HAS_DERIVATIVE` back out of the source —
comparing against a restatement of the rule would pass with both
sides wrong together. A second test walks a Mandelbrot from zoom 4
(no gap) to zoom 30 (gap: the deep path) and then adds damping, which
takes the view back off the perturbed path and restores the
derivative — so the hint follows the settings rather than the zoom.

A finite-difference distance estimate would cover both cases; the
relief-shading pass already differences the value field for exactly
this reason, so the machinery exists. Not attempted here: it is a
different estimator, and "distance estimate" should keep meaning the
analytic one until something deliberately adds the other.

### The Lambda perturbation tier (2026-08-30)

`z' = c*z*(1-z)` now perturbs on both rungs, taking the logistic
family from the direct path's ~2^14 ceiling to the same depth the
Mandelbrot tiers reach.

**It is the first tier whose parameter MULTIPLIES.** Every earlier one
has c entering additively, so the delta step never needed to know the
reference's c at all. Expanding `(C+dc)(Z+d)(1-Z-d) - C*Z(1-Z)` and
collecting gives

    dP = d*(1 - 2Z - d)
    d' = C*dP + dc*(Z(1-Z) + dP)

so the step carries a factor of C, and the parameter-plane term acts
on the reference's own z-product `Z(1-Z)` instead of being a bare
`+ dc`. C rides in the perturb uniform as plain f32, in what used to
be two words of padding: it is a MULTIPLIER, so only its relative
error matters, and 2^-24 is what the rung already accepts for the
reference itself.

**The seed is the other half.** Lambda's parameter plane starts at the
CRITICAL POINT 1/2, because zero is a fixed point of this map for
every c — a zero-seeded lambda plane is one flat colour. The reference
generator needed a matching branch (`MAP_LAMBDA`, seeded 1/2, with the
`1 - z` subtraction done in fixed point and the imaginary part negated
rather than subtracted, since the limbs are sign-magnitude and a naive
`sub` would leave a non-canonical negative zero). The delta still
starts at zero, because both orbits share that seed.

BLA is declined for now. It is derivable — the map is holomorphic with
`A = C(1-2Z)` and `B = Z(1-Z)` — but the table builder is written
around the power tier's `A = p*Z^(p-1)` and has no per-tier hook yet,
so Lambda iterates per step.

**An f64 oracle is NOT ground truth for this family, and finding that
out cost most of the work.** The verification pattern that settled
Phoenix and Manowar — render, then compare escaped-or-not against an
f64 orbit — reported 19% disagreement here, which looks exactly like a
broken delta step. It was not. Lambda's critical orbit LINGERS: near
the boundary pixels take hundreds of iterations to escape, where the
Mandelbrot and Phoenix views finish in tens, and over that many steps
an independent f64 orbit amplifies its own rounding past the escape
decision.

Three measurements separated the instrument from the code:

- the delta recurrence reproduces the exact difference to f64 rounding
  (~1e-16 against deltas of 1e-7), traced step by step;
- disagreement is flat in zoom — 18.6% at zoom 16, 19.6% at 20, 18.5%
  at 24 — which is not what a precision bug looks like;
- against a **60-digit** ground truth on 120 sampled pixels, the f64
  direct orbit was wrong on **10.0%** and the perturbed recurrence on
  **7.5%**. The perturbed path is the more accurate of the two, which
  is the entire point of carrying an exact reference.

The instrument was then fixed rather than the code: with the same view
and the same shader, disagreement runs 0.23% at `max_iter` 600 and
9.40% at 3000. The shipped test uses 600 — long enough that plenty of
pixels have escaped, short enough that f64 is still an authority — and
says so in a comment, with the numbers, so the next person does not
re-run the same investigation.

A degeneracy guard came out of the same episode and is worth keeping
generally: the first center tried was entirely interior, and an
all-interior view agrees with ANY oracle. The test now refuses to draw
a conclusion unless between 10% and 90% of pixels escaped.

### Fixed-point division, and Feather's unfinished tier (2026-08-30)

The completion plan listed Feather, McMullen and Magnet as tractable
with the note "division needs care near poles". The actual blocker was
one level down: **the fixed-point layer had no division at all.** Its
own header says so — "the core never divides (only small-scalar
division for decimal I/O)" — which was true while every reference map
was a polynomial. Every rational family needs it to build a reference
orbit, so none of them could start.

`FixedPoint::recip` now provides it: Newton's `x <- x*(2 - a*x)`,
seeded from f64 and run to the full limb width, with
`FixedComplex::div` on top (via `conj` over the squared magnitude, so
the only reciprocal taken is of a real). Tested three ways —
`a*(1/a)` returns 1 to within the bottom limb at 4, 12 and 40 limbs;
complex division matches an f64 oracle; and out-of-range input is
REFUSED rather than saturated.

**That refusal is the load-bearing part.** With `INT_BITS = 8` the
representable range is about ±128, so `1/a` for `|a| < 1` does not
fit, and near a pole it does not fit by a wide margin. Saturating
there would write a quietly wrong reference orbit into the on-disk
cache — the failure mode this engine keeps refusing — so `recip`
returns `None` and the caller decides. It is also what decides which
rational family could ship first: Feather's denominator is
`1 + x^2 - i*y^2`, whose real part is at least 1, so `|D| >= 1` for
every z and the reciprocal is always in range. Magnet and McMullen
have genuine poles and need a range strategy before they can be wired
up at all.

**Feather's tier is built and gated off.** `MAP_FEATHER` (reference,
tracking f64 to 5.9e-8 at every limb count), a `Feather(p)` tier and
both delta rungs exist and assemble. They introduce the quotient delta
form the other rationals will reuse:

    dq = (dN - q*dD) / (D + dD)

with `q = N/D` the REFERENCE quotient. Written the obvious way
instead — `(dN*D - N*dD)/(D*(D+dD))` — it differences two full-size
products and loses the delta to cancellation. `dD` is component-wise,
because this denominator is not holomorphic, which also costs BLA.

It is verified at zoom 15 (0.00% against an exact orbit) and 20
(0.49%), and degrades past that: 26% at 25, and by zoom 30 the delta
stops iterating altogether — escape becomes a linear function of pixel
position, which renders as hard straight diagonals
(`output/origami/fe-zooms.png`). So `perturb_tier` returns `None` for
`feather`, and the panel's depth hint says 2^14 rather than promising
depth the engine will not deliver.

**The cause was found the next day, and the section below records
it** — the one line of that "faithful" simulation left in f64 was the
ESCAPE TEST, and that was the fault. Quantizing just that line to f32
reproduced the whole failure curve (0.00% / 0.33% / 30.7% / 50.3% at
zooms 15/20/25/30 against the GPU's 0.00% / 0.49% / 26.35% / dead).
Everything the paragraph above rules out stays ruled out; the delta
machinery shipped correct.

**A harness footgun found on the way out**, worth knowing before the
next tier: `force_perturbed` on a formula whose `perturb_tier` returns
`None` does not fail — it falls back to `PerturbTier::Power(2)` and
renders the MANDELBROT delta step against that formula's reference.
The result looks like a broken render of the formula under test rather
than what it is. That is how gating Feather off turned its passing
zoom-15 test into "6912/6912 pixels escaped".

### The delta-aware escape margin, and Feather un-gated (2026-08-30)

**The bug.** The perturbed templates reconstructed `z_full = Z_m + δ`
in f32 and tested `|z_full|² > bailout` in f32. The per-pixel
information is δ, whose effect on `|z_full|²` is ≈ `2·Z·δ`; one ulp of
the bailout (4.0) is 2.4e-7. At zoom 25, δ ~ 1e-8 makes that effect
~4e-8 — **below the ulp** — so the escape test could not distinguish
neighbouring pixels at all: every pixel inherited the reference's
rounded fate. Both rungs share that test, which is why the broken
Feather renders were bit-identical across two unrelated delta
implementations — the clue misread as "shared upstream data" the
first day.

**Why only Feather ever noticed.** A chaos-driven boundary
(Mandelbrot, Lambda, Phoenix) amplifies δ exponentially; by the time
an orbit crosses the bailout, δ is O(|Z|), far above ulp, and a pixel
whose δ is still sub-ulp at the crossing genuinely shares the
reference's fate to ±1 iteration. Feather grows |z| by only ~×1.2 per
step past the bailout (`z³/(1+x²−iy²)` is near-linear at large |z|),
and the test view made it maximal: |c|² = 3.9752, one step from the
bailout, so the local boundary is a smooth threshold arc DECIDED by
sub-ulp differences. The straight-edged renders were the quantized
decision regions — and, at this particular view, the *correctly
rendered* image is also a straight edge, because the true boundary
there is smooth. The oracle comparison is the arbiter, not the look.

**The fix.** The escape test is now a margin formed in parts that are
each exact or tiny:

    margin = (|Z|² − bailout) + (r2_lo + 2·Z·δ + |δ|²)

with |Z|² carried per reference entry as a CPU-computed DF pair
(`ref_r2`, binding 11 on both perturbed layouts). Near the threshold
`r2_hi − bailout` is EXACT (both f32, within a factor of two —
Sterbenz), and the remainder terms are all δ-scale. The channel is
computed at UPLOAD time in f64 from the hi+lo+exponent entries already
in hand, so there is no reference recompute and **no orbit-store
format change** — cache-loaded orbits get it for free, at the DF
shadow's own 2^-48 relative precision, which is the pipeline's native
reference precision anyway. In-shader compensated arithmetic was
rejected because Metal runs shaders with fast-math on (CLAUDE.md), and
error-free transformations do not survive it.

On the deep rung the cross term uses the plain f32 δ, which
underflows to zero exactly when it is too small to move the margin —
the correct answer, not an approximation. Residual honest limit: past
the scaled rung (zoom > 48), a slow-growth threshold boundary whose
deciding differences sit below 2^-48 relative still cannot be
resolved; that is the reference's own precision, not the test's.

**Verified four ways.** The zoom-30 Feather view that was 26% wrong
and then fully degenerate now reads **0.00% on both rungs** against
the f64 oracle; reverting the margin to the plain f32 test makes that
test fail; all 21 pre-existing GPU tests pass unchanged (the margin
is behaviourally invisible on chaos-driven boundaries); and all 217
visual baselines stayed within range, including the deep-threshold
configs. Feather's tier is re-enabled, the panel's depth hint says
deep again, and `escape-feather-deep.fflame` pins the view that found
the bug.

### McMullen perturbs, and what its parameter plane turned out to be (2026-08-30)

`z^n + c/z^m` now perturbs on both rungs — 0.22% and 0.19% against an
exact orbit at zoom 30 — making it the first tier with a genuine POLE.

**The pole is handled by normalizing, not by refusing.**
`FixedPoint::recip` could only invert `|a| >= 1`, which would have
blocked this family outright. `recip_scaled` finds the exact
normalizing shift with `to_floatexp` (leading-zero count, no
rounding), inverts a mantissa in [1,2), and returns the scale
separately — so a small denominator is fine and only an out-of-range
QUOTIENT is refused. For a map with a pole that refusal is not an
error: it is the orbit escaping, and the reference branch records it
exactly as the bailout would. The earlier "refuse `|D|^2 < 1`" rule
was a statement about the implementation rather than the requirement,
and it would have kept this family out for no reason.

**The delta form never divides small by small:**

    dA = (Z+d)^n - Z^n                  (binomial, exact)
    dM = (Z+d)^m - Z^m                  (binomial, exact)
    d' = dA - C*dM / [ (Z+d)^m * Z^m ]

The pole term's difference is written as
`1/(Z+d)^m - 1/Z^m = -dM/((Z+d)^m Z^m)` — small numerator over a
product of FULL values. Formed directly it would subtract two large
nearly-equal reciprocals and lose the delta entirely.

**JULIA ONLY, and that is a finding about our formula rather than a
limitation of the tier.** Our McMullen seeds its parameter plane at
`z_0 = c`. That is not a critical point of this map: `z = 0` is the
POLE, and the actual critical points sit at `z^(n+m) = (m/n)c`. The
consequence is measurable — **0 of 4000 sampled parameters have a
bounded orbit**, so the parameter plane has no interior at all and
perturbing it would be machinery for nothing. The formula's own doc
comment already flagged the seed as provisional ("the pixel-seeded
parameter plane is the exploratory map until the proper critical-orbit
seed lands"); this is the measurement behind that note. The classic
Sierpinski-carpet pictures are Julia sets, which is where the tier is
selected. Re-seeding the parameter plane is a separate, visible
formula change.

**The bug worth remembering, because it was the same one twice.** The
first perturbed render escaped on every pixel. The cause was not the
delta algebra — an f64 simulation matched the exact orbit at 0.0% for
zooms 10, 20 and 30 — but `map_params_for`, which had no McMullen
arm and so returned zero for the pole power. The reference was built
for `c/z^1` while the delta step used `c/z^3`: two different maps. That
is precisely the failure its own doc comment describes for Phoenix's
`p`, a year of code earlier.

So the guard was generalized rather than patched. The
`reference_parameters_match_the_shader_uniform` test now carries a
TABLE of which parameters ride `map_params` into the reference's
identity, checks each against `pack_params` for both default and
edited configs, and — the half that catches the next tier — asserts
that **every formula which can perturb has an entry**. An empty list
is a valid answer; being forced to say so is the point. Reverting the
McMullen arm now fails that test in milliseconds, on the CPU, instead
of costing a GPU debugging session.

### Magnet perturbs, and the perturbed path learns to converge (2026-08-30)

Both variants on both rungs, 0.00% against an exact orbit at zoom 30
in all four combinations — and the last of the four the completion
plan called tractable.

**The delta form is the quotient one, composed with a square:**

    dq = (dN - q*dD)/(D + dD)      q = N/D, the REFERENCE quotient
    df = 2*q*dq + dq^2             from f = q^2

What is new is that `c` appears in BOTH the numerator and the
denominator, so the parameter-plane term is not a bare `+dc`: it
enters `dN` and `dD` separately and then partially cancels inside
`dN - q*dD`. That cancellation is the map's own — it is what makes the
derivative small near the attractor — and it happens between two SMALL
quantities, so no significance is lost. Magnet II carries the same
shape with a cubic numerator and quadratic denominator.

**The perturbed templates could not converge, and Magnet is a
convergent family.** Both rungs hardcoded `converged = false`: fine
while every perturbing formula escaped, wrong the moment one settles.
Magnet's orbits go to the fixed point at z = 1, and every escape-count
and smooth coloring shades convergence SPEED for this family — so
without the settle test each converging pixel would run to `max_iter`
and report an iteration count two orders of magnitude too large. That
is a different picture, not a rounding difference.

Convergence detection is now spliced into both perturbed rungs with
the same `//__CONVERGE_TEST__` marker the direct template uses, gated
on `PerturbTier::is_convergent`. `assemble_perturbed` has no
FormulaDef in scope — on that path the TIER is the map's identity — so
the feature is restated there, and a test walks every formula asking
the renderer which tier it would use, checking the two answers agree.
For non-convergent tiers nothing is spliced and the WGSL is
byte-identical, which the 219 unchanged visual baselines confirm.

**Testing convergence took two attempts, and the first one lied.** The
binary lit/dark comparison this suite uses cannot see it: the template
sets `escaped` on convergence too (so the escape colorings shade
speed), which makes converged and escaped pixels both LIT. Worse, the
first views chosen — boundaries found by bisecting on "does the orbit
terminate" — turned out to contain **zero converging pixels**:
everything either escaped or landed on a higher-period cycle that a
period-1 settle test cannot catch. Disabling convergence support left
that test passing at 0.00%.

So there is a second test. It renders a genuine convergence/escape
boundary (measured 4069 converging pixels) with the escape-count
coloring, and bins rendered luminance against the f64 oracle's
TERMINATION ITERATION — palette-agnostic, asserting only that equal
counts render equally. It reads 0.58/255 with the settle test compiled
in and 10.82/255 without, and the threshold is calibrated between
them rather than guessed.

**Depth, per formula.** Not one number: `mandelbrot`, `multibrot`
(integer powers), the `burning_ship` variants, `tricorn`/multicorn,
`phoenix` and `manowar` perturb and reach z9316+ (Manowar on the deep
rung only, by measurement -- see above); the rest render
through the DIRECT path and stop where its f32 pixel mapping does, at
about zoom 14 -- which is why `PERTURB_MIN_ZOOM` sits there. Extending that set is item 2 of the
completion plan.

**What remains before this branch is done** is scoped in
[escape-time-completion.md](../archive/escape-time/escape-time-completion.md): per-formula
accuracy against references (only 3 of 23 formulas can perturb --
the other 20 stop at the direct path's ~2^14 ceiling), extending the
Mandelbrot deep-zoom machinery to more formulas (only 3 define a
derivative, which is what BLA, distance estimation and nucleus
finding all need), a UI/UX pass, splitting the WASM builds by
engine, online-API support, and scripting (the script API mentions
escape zero times today).

Two plans split out of this queue on 2026-08-28 (both "later",
sequenced after the orbit-store compression lands):
[escape-tdr-safety.md](../archive/escape-time/escape-tdr-safety.md) — the perturbed-path
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

### The big-float families perturb: Newton, Nova, Ducks, Kaliset (2026-09-02)

The four remaining families the completion plan called blocked. Each
needed the same thing first, and it is the reason they were blocked:
**a reference orbit that fixed point cannot hold**. Newton's step
divides by f', so a reference passing near a critical point makes an
excursion no fixed binary point survives (measured: |Z| reaches 3e9
from a 1e-5 miss, and 958 on an ordinary boundary view); Kaliset
inverts; Ducks takes a log. So these iterate a `BigComplex` -- the
limb-array-plus-exponent type that already existed for Newton
nucleus-finding -- and `bigfloat.rs` gained the transcendentals to do
it: `sqrt`, `ln` and a principal-value `atan2`, each Newton- or
series-based over the existing multiply, with `ln 2` and `pi` cached
per width. `map_is_big` names the family set; those orbits are never
written to the disk store (the file format has no slot for their live
state, and they are short).

**What each tier's delta form is**, beyond the shapes already in this
document:

- **Newton / Nova** — the quotient rule over Taylor differences: every
  `dF`, `dF'`, `dF''` is the cancellation-free binomial, and each
  scheme is a quotient of polynomials in those, so Feather's
  `dq = (dN - q dD)/(D + dD)` composes with the product rule. Newton
  is c-free, so the renderer requests its reference **Julia-style**
  whatever the toggle says (`PerturbTier::is_dynamical`): the delta
  starts at d0 and no dc term ever enters.
- **Kaliset** — the Ship tier's `diffabs` for the fold, over a REAL
  denominator: `dq = ((da, db) - q*dr2)/(r2 + dr2)`.
- **Ducks** — `log1p` of the fold's ratio, since a bare
  `log(T + dt) - log(T)` cancels to nothing.

Four things went wrong, and all four are worth recording because
three of them looked like the others.

**1. Chebyshev's `r` cannot be differenced by the quotient rule.**
`r = F F''/(2F'^2)` is asymptotically CONSTANT, so `dA - r dB`
cancels its own leading terms and leaves ~|Z|^-(3p-2) of the
operands -- at that 958 excursion, far below f32. Newton and Halley
never trigger it (their references peak at 10.5 and 2.7); Chebyshev
put 3.3% of pixels in the wrong basin. The fix is symbolic rather than
numeric: for `z^p - 1`, `r = k(1 - z^-p)` with `k = (p-1)/2p`, so
`dr = k dM/(Z^p (Z+d)^p)` -- McMullen's pole form, cancellation-free.
That closed form exists only for `z^p - 1`, so **Chebyshev over the
other two functions declines the tier** (`rootfinder_has_delta`)
rather than shipping a plausible wrong picture.

**2. Ducks' branch cut is not invisible.** `Log(T) + Log1p(u)` is the
principal value only MODULO 2*pi*i, and the fold does NOT undo the
difference (`|y|` and `|y - 2pi|` differ everywhere except at the cut
itself) -- while `|z|` is exactly what the Ducks colorings average. So
a missed turn is an O(1) error on a real fraction of pixels. Both
rungs now re-anchor the delta by a whole number of turns; a MULTIPLE,
because variant 4's reference is `Log(t^2)`, whose delta is
`2 Log1p(u)` and can start two turns out.

**3. The reference's f32 hi is not good enough for Ducks.**
`T = fold(Z) + C` is O(1) and built from the stored hi, so it is
wrong by ~2^-24 -- and that is not a relative error on the delta but
an INCONSISTENCY between the reference the delta is propagated
against and the one the coloring adds back. Measured at zoom 30:
8.0e-5 mean (worst 4.5e-3) against exact orbits, which is 1.6% of that
view's entire contrast. Feeding the low half back through one Neumann
term (`rf_tinv`) gives 1.1e-6.

**4. Kaliset is accurate only from about zoom 24.** Its inversion
gives the delta a 1/|Z|^2 amplification the other tiers have no
analogue for. Scored against exact orbits on an inverting view:
37% out at zoom 10 and 20% at 14 -- where the DIRECT path is 0.2% --
then 2.4e-2 at 18, 3.6e-4 at 24, 5.7e-6 at 30. So it carries a floor
of its own (`tier_min_zoom`) and renders direct below it, which is
what it did before this tier existed. Shipping without the floor would
have made the picture worse exactly at the threshold. A compensated
(exact-product) difference in its numerator was tried and measured to
change nothing, so it is not in the code.

**Testing this needed a better oracle than f64, and that cost the most
time.** At these boundary centres an f64 orbit has already lost the
trajectory by step 27-41, and Chebyshev's orbits run to 83 -- so an
f64 "truth" disagrees with the exact answer on ~3% of pixels by
itself, which reads exactly like a broken delta step. Worse, a
float32 CPU mirror of the delta algebra AGREED with that f64 oracle,
because both drifted together; it exonerated code that was in fact
wrong. The convergent oracles now read their outcome off a big-float
orbit per pixel, and the non-escaping ones off `exact_mean_magnitude`.
Two further traps in the same family: comparing the perturbed render
against the DIRECT one assumes direct is the truth, which on a chaotic
orbit average it is not (`perturbation_beats_direct...` scores both
against exact orbits instead); and Ducks variant 4 at 80 iterations is
chaos-dominated -- a 1e-3-PIXEL nudge moves the exact mean by 6.6e-3
on 97% of pixels -- so its test runs at 40, where the field is fully
determined and still carries 8.7e-3 of contrast.

**Two robustness fixes fell out.** The root-finder reference now takes
the shader's own `esc_cdiv` pole guard instead of ending the orbit at
a critical point: `z^p - 1` has `f'(0) = 0` and Newton's shipped
preset is centred at exactly z = 0, so an orbit that stopped there
handed the perturbed path a one-entry reference and rendered nothing
like the direct image (603 of 768 blocks). And `rf_cinv` gained an
upper guard, because Z^p at |Z| ~ 1e3 overflows f32 for the higher
powers and the unguarded form would divide inf by inf.

**The seam that came back, and what it actually was (2026-09-02).**
Reported from the app: past the threshold Ducks grew curved lines that
cut the image and slid as the zoom deepened, while the picture was
otherwise the one direct rendered. It was the Zhuoran REBASE, firing
on the branch wrap's own bookkeeping.

The wrap legitimately moves a delta by a whole turn, so `|delta|`
reaches ~2*pi on more than half of all steps -- measured, 285914 of
552960 at the reported view. The magnitude test reads that as "this
pixel has diverged from the reference" and re-anchors; but a turn is
not divergence, and every re-anchor pays `z_full - Z_0` in f32 -- two
O(1) iterates subtracted, so the rebuilt delta is accurate only to
ulp(1) ~ 6e-8 however small it truly was. At zoom 14 a pixel's delta
is ~1e-4, so that is three digits gone on most steps; and because the
re-anchor points fall on curves in the plane, the loss lands as curved
seams that slide with the zoom.

Bisected on a CPU mirror of the shipped step, mean relative error
against exact orbits at that view:

| | mean | pixels over 1e-3 |
|---|---|---|
| as shipped | 2.2e-3 | 4898 / 6912 |
| **rebase only at orbit end** | **1.8e-5** | **3 / 6912** |
| wrap removed instead | 1.0 | 6912 / 6912 |

So the wrap is load-bearing and the magnitude test is the fault.
Ducks now rebases ONLY when the reference runs out
(`rebase_only_at_orbit_end`), which is safe because its reference is
non-escaping and covers every iteration the render asks for. The GPU
reproduces the mirror exactly (1.830e-5, 3 pixels), and no existing
visual baseline moved -- the fix is surgical. The same policy was
tried on Kaliset and MEASURED not to help (identical at one centre,
slightly worse at the other), so Kaliset keeps the default rebase and
its floor: its problem is the inversion, not the rebase.

**Ducks goes flat at depth, and that is the COLORING, not the engine
(2026-09-02).** Reported next: past about 1e8 zoom, panning Ducks
rotates the image, and deeper still panning does nothing at all while
the coordinates change. Both are real and neither is a perturbation
bug -- the render matches exact orbits to 1.1e-7 (parameter plane) and
1.2e-6 (Julia) AFTER a 2560-pixel pan.

What is actually happening is that `magnitude_average` is a SMOOTH
function of c, and a smooth function restricted to a shrinking window
converges to its own first-order Taylor expansion. Measured: fit a
plane to the field and report the fraction of variance it explains --

| view | planarity | field std | gradient angle over five 512px pans |
|---|---|---|---|
| zoom 14, parameter | 1.0000 | 7.2e-5 | 87.5 -> 85.2 -> 82.7 -> 80.3 -> 77.7 |
| zoom 26.6, parameter | 1.0000 | 1.2e-8 | 87.5 -> 87.5 -> 87.5 -> 87.5 -> 87.5 |
| zoom 26.6, Julia | 0.8993 | 2.3e-4 | jumps around |

So the field IS a plane, exactly. A cyclic palette turns a plane into
parallel bands, the pan rotates the plane's gradient, and the bands
rotate with it -- that is the "rotation". Deeper still the gradient
direction freezes and its magnitude reaches 1e-8, so the bands stop
moving: "panning does nothing". Raising `max_iter` does NOT recover
the contrast (measured at zoom 26.6, parameter plane: 1.2e-8 at 80
iterations, 1.1e-7 at 2000, 4.0e-8 at 10000).

This is the general price of an orbit-STATISTIC coloring. An escaping
fractal keeps detail at any depth because the escape count is a
discontinuous integer; an orbit average is smooth, so it flattens. It
bounds how deep the non-escaping families are worth taking, and no
amount of reference precision changes it.

### Auto contrast: fitting the palette to the field (2026-09-02)

The answer to the flattening above. The field goes smooth under zoom
and the palette does not follow, so the fix belongs on the palette
side: measure what the coloring's values actually span and map THAT
onto the palette, rather than whatever `scale`/`offset` were set to
three zoom levels ago.

**Where it runs.** In the RECOLOR pass, which already exists to make
palette and coloring edits real-time over the cached per-pixel
records. Nothing about iteration changes, so the iterate templates are
untouched and their WGSL is byte-identical -- which the 236 unchanged
visual baselines confirm.

**How it measures.** A probe pass subsamples the coloring's own value
field (the R32Float height target every template already writes) onto
a 96x72 grid and reads back 28 KB, once per SETTLED view. A subsample
rather than a GPU reduction, deliberately: WebGPU has no float
atomics, and a CPU-side sample set buys true percentiles -- which
matters here, because Ducks' `log` guard emits -34.5 and a raw min/max
would let one singular pixel set the whole scale. Validity comes from
the colour target's alpha, since a pixel with no value stores height 0
and 0 is an ordinary value.

Two modes over the same fit. **Auto range** maps the measured range
onto the palette. **Flatten** first subtracts a least-squares PLANE
(three coefficients, solved by Cramer on the normal equations over the
live cells) and ranges the residual -- the plane being exactly what
deep zoom leaves behind. Where the field really is a plane, Flatten
correctly shows almost nothing, because there is nothing left.

**The ordering trap.** The fit is measured FROM the finished field, so
the pass that produces the field cannot apply it. A settled frame
therefore reports "not settled" once (`contrast_pending`); that frame
takes the recolor path, measures, applies, and settles for real.
Without it the app stops at the uncorrected image, because it renders
only while dirty. The height field also keeps the PRE-contrast value:
the probe reads that texture, so remapping it there would feed the fit
its own output and compound every frame.

**Measured**, on the reported view at zoom 26.6 where the field's
spread is 1.2e-8 -- tonemapped luminance spread across the image:

| contrast | spread |
|---|---|
| off | 0.062 |
| auto range | 71.6 |
| flatten | 69.3 |

A factor of 1150. Off is the default and writes nothing to the config,
so every existing file is byte-stable.

### The 4K video export hang (2026-09-02)

Reported as a video export stuck on one frame forever with the app
still responsive. It was a PANIC on the exporter's worker thread:

    In Device::create_buffer, label = 'Escape Iter State'
    Buffer size 398131200 is greater than the maximum buffer size
    (268435456)

3840 x 2160 x 48 bytes of per-pixel resume state for a 4K frame,
against that GPU's 256 MB limit. wgpu answers a validation error by
panicking; the worker died and the frame never arrived, so the dialog
waited on it forever.

**Ducks only exposed it.** Every perturbing formula was already
subject to this -- Ducks simply had never taken the perturbed path
before. The headless path has checked it since the tiers shipped
(`allocation_error`, which does both limits correctly); the video
exporter drives `EscapeRenderer` directly and never ran that check.
Nothing below supersample 1 shrinks the state, so
`affordable_supersample` could not save it either: it clamps the
FACTOR, and at 1 a 4K frame still wants 398 MB.

Fixed in two layers, because one of them has to hold for every caller:

- `perturb_state_fits` is consulted where the perturbed path is
  CHOSEN, so an unaffordable frame renders direct instead of asking
  for a buffer that panics. `ensure_iter_state` returns a bool as
  belt and braces rather than trusting that.
- The exporter refuses up front with the size that would work.
  Falling back to direct is right for a viewport frame, but past
  zoom 14 direct is mush, and a whole export coming out quietly wrong
  is worse than one that says why.

The regression test asserts the predicate AT ITS OWN BOUNDARY rather
than at a fixed 4K, so it means the same thing on a GPU with a 2 GB
buffer limit as on one with 256 MB, and costs no allocation.

**Corrected the same day: the 256 MB was not the GPU.** 268,435,456
is `wgpu::Limits::default().max_buffer_size` -- 256 MiB exactly. Both
video-export device creations started from the defaults and raised
only the storage-BINDING size from the adapter; `max_buffer_size` and
`max_texture_dimension_2d` stayed at 256 MiB and 8192. The exporter
asked wgpu for a buffer past the limit it had itself requested. The
still-image exporter (`app/export.rs`) has raised all three from the
adapter since it hit the same wall; the video exporter now matches
it, and the reported frame renders perturbed in 0.14 s under adapter
limits (`a_4k_deep_ducks_frame_renders_perturbed_under_adapter_limits`).
The precheck and the decision-point guard stay: they read the
device's limits, so they relax exactly as far as the device does.

### 4K video with antialiasing: what it costs and what was chosen (2026-09-02)

Asked for: 4K video at 8x antialiasing for every fractal, 2x at the
least. Reviewed against what exists rather than designed fresh.

**Three caps stood between the request and the render**, and only
one of them was hardware:

| cap | where | effect at 4K |
|---|---|---|
| `max_buffer_size` requested at wgpu's 256 MiB default | video exporter's device | panic on any perturbed frame (above) |
| render-pixel budget: 1.5 GiB / 132 B = 12.2 MP | `affordable_supersample` | 4K is 8.3 MP, so the GRID is clamped to 1x on every GPU |
| animated `Escape.Supersample` clamped to 3 | `apply_config_value` | a track set from the panel (which offers 8) exported softer than it previewed |

The second is the one that decides the design. A supersampled grid at
4K x 2 is 33 MP of per-pixel state; at x 8 it is 531 MP and 30720
pixels wide, past the 16384 texture-dimension limit every adapter
has. No budget tuning reaches 8x as a grid, at 4K or anywhere near.

**The flame tiler does not transfer.** `export/high_res.rs` exists
because the chaos game SCATTERS: a sample can land on any pixel, so a
tile needs a sample-emit pass and a scatter into its own histogram.
An escape render is independent per pixel -- a tile is just a
sub-rectangle -- so the scatter machinery solves a problem escape
does not have. What would transfer is the `TileLayout` row
arithmetic, and the direct path already has its own (`tile_y0`,
`direct_rows_per_dispatch`).

**What was chosen: accumulation, which was already the still
exporter's answer.** The same sample positions, taken as several
ordinary renders each displaced within a pixel and averaged
(`sample_grid`, `begin_accumulation`, `accumulate_sample`): identical
total iteration work, memory fixed at one frame, no size limit, and
the reference orbit shared by every sample. The video exporter
rendered exactly one grid sample per frame and had none of this;
it now runs the same loop the still path does, per frame, and the
tail reads the averaged image. One property carries it: a per-frame
`begin_accumulation` starts clean (pinned by
`per_frame_accumulation_starts_clean` -- frame B after frame A equals
frame B alone, worst channel delta 0/255). The track clamp is lifted
to the panel's maximum.

**Measured**, 4K, perturbed f32, on the development GPU (whose
throughput is 11.3 Gpx-iter/s on Ducks, a non-escaping map where
every pixel runs to `max_iter`; Mandelbrot escapes early and runs
faster per nominal iteration):

| frame | 1x | 2x (4 renders) | 8x (64 renders, extrapolated) |
|---|---|---|---|
| Mandelbrot, 2000 iterations | 0.43 s | 1.59 s | ~27 s |
| Ducks, 500 iterations | 0.27 s | 0.99 s | ~17 s |

Accumulation scales almost exactly linearly in samples (0.99 against
4 x 0.27), which is the reference orbit being reused. For a 30-second
video at 30 fps (900 frames) that is about 24 minutes at 2x and about
seven hours at 8x for the Mandelbrot row. The cost is iteration work,
not memory, and no rendering scheme changes it: 8x IS 64 renders'
worth of iterations however they are arranged. That is the honest
price of the request, and it is the user's to pay or not.

**Row banding was considered and not done.** It would let the
perturbed path hold frames larger than the device's buffer limit by
sizing the state to a band, the way the direct path already does.
With the exporter asking for adapter limits, that is now only needed
on a GPU whose real `max_buffer_size` is under the frame's state
(398 MB at 4K) -- integrated parts, mostly -- or for grids wider than
the texture limit, which accumulation sidesteps entirely. It is a
change to the chunk state machine, and nothing measured here asks
for it. If a device turns up that does, `tile_y0` is already in the
uniform.

### A Ducks reference that grazes the log singularity (2026-09-02)

Reported as: past about 1e12 the fractal changes completely, zooming
back out does not undo it, and two saved views render wrongly when
loaded directly though they look right if you zoom in to them. Read as
orbit caching. It was arithmetic, and it was not depth-dependent at
all -- measured against exact orbits, that centre was broken from zoom
19 upward, which is as soon as it perturbs. 1e12 was where it became
obvious, not where it began.

**What the centre does.** At iteration 65 the reference reaches
Z = (-0.1500, 0.6750) against c = (0.15, -0.675), so the Ducks step's
`fold(Z) + c` cancels two O(1) f32 numbers down to |T| = 2.08e-8 --
about a third of an ulp of the operands. The reference itself is fine
(it is big-float, and its own value is exact); it is the DELTA step's
f32 copy of T that is left holding nothing but rounding.

**Two failures in series.** `rf_tinv` expands 1/(T_hi + T_lo) to first
order in T_lo/T_hi, which is valid only while the hi half dominates.
Here the true T lives entirely in the low word, so the correction was
the size of the term it corrected. `rf_cinv` then returned its 1e20
floor sentinel, |u| passed 1e19, and `dot(u, u)` OVERFLOWED f32 inside
`rf_clog1p` -- +inf where log(1+u) is about 45, a value f32 holds
comfortably. 898 of 1728 pixels went infinite and took the
magnitude-average accumulator with them. Both rungs share `rf_tinv`,
so both were affected.

Fixed at both points. `rf_tinv` uses the f32 SUM of the halves when
they are comparable -- they share an exponent there, so nothing is
lost adding them, and t_hi is exact by Sterbenz -- and keeps the
expansion where the hi half dominates. `rf_clog1p` factors the
magnitude out for huge |u|: log|1+u| -> log(m) + log|u/m|, each term
in range.

| zoom | before | after |
|---|---|---|
| 19.27 | 1714/1728 non-finite | 0, mean err 2.1e-7 |
| 41.47 (reported) | 898/1728 non-finite | 0, mean err 1.7e-7 |
| 56 (deep rung) | -- | 0, mean err 1.7e-7 |

**Two things worth recording about finding it.** The first hypothesis
-- that the seam fix's removal of the magnitude rebase let the delta
grow unbounded -- was WRONG, and a one-line experiment restoring
`rebase_default` for Ducks produced byte-identical numbers, which
killed it in a minute. And a `max_iter` sweep on the real GPU (clean
through 64, broken at 66) localised the failure to a single iteration
before any shader was read, which is what made the near-singularity
findable at all.

Every existing Ducks test passed throughout: their centres never pass
close enough to the singularity for the expansion to break. The new
test pins nine depths spanning the rung switch against exact
big-float orbits, and `escape-ducks-singularity` renders the reported
view.

### A browser freeze on loading a deep config (2026-09-02)

Reported from WASM: loading an escape config with high iteration count
and zoom freezes, while zooming in manually to the same place does
not. The error is

    Uncaught RuntimeError: index out of bounds

from inside a requestAnimationFrame callback, with no Rust panic
message.

**That message is the diagnosis, and it says what it is NOT.** The
panic hook IS installed (`lib.rs`), so a Rust panic would have printed
a located message; a slice panic would read "index out of bounds: the
len is N but the index is M". A bare `RuntimeError` with neither is a
WebAssembly trap — a linear-memory access out of range — and safe Rust
cannot produce one. In the whole escape engine there is exactly one
place that can: the simd128 column multiply in `fixedpoint.rs`, which
stores through raw pointers.

Ruled out by measurement rather than reasoning, in this order:

- **The partial-orbit path.** WASM has no worker thread, so it slices
  the reference under a per-frame budget and renders against a partial
  orbit — a path with NO desktop coverage, which made it the first
  suspect. A test knob (`force_budgeted`) now takes it on the desktop;
  it settles cleanly at zoom 30/120/400/900 with up to 1M iterations.
- **32-bit `usize` overflow**, the classic desktop-works/wasm-fails
  divergence: every buffer size in the path is computed in `u64`.
- **`BigFloat::mul`**, which has its own bounds-checked schoolbook
  loop and never reaches the vector core.
- **The orbit store**, which is `std::fs` and desktop-only.

**What was wrong with the vector core.** Its index arithmetic is
correct — writes reach `2n+2` against an allocation of `2n+7`, and
reads are guarded by the loop condition. But every PRECONDITION that
makes that true (`a.len() == b.len() == n`, the allocation size, and
`n >= 2` so `2*(n-2)` does not underflow a `usize`) rested on
`debug_assert`s, which are compiled out of the shipped wasm. A
violation there is not a panic anyone can locate; it is a trap from
whatever callback happened to be running.

So the preconditions are now CHECKED, once per multiply — O(1)
against an O(n^2) body — and the scalar core, every index of which the
compiler bounds-checks, takes over if any fails. A violation now
surfaces as a located Rust panic instead of an unattributable trap.
The arithmetic itself is pinned by
`the_vector_column_core_stays_inside_its_allocation`, which reproduces
the index math for n = 2..64 on every platform, including the desktop
where the code it describes is not compiled.

**It recurred, at the SAME wasm offset** (3529510 in both reports),
which is itself evidence: an edit to `fixedpoint.rs` would move code
around, so the second report was almost certainly the same binary --
browsers cache `.wasm` aggressively, localhost included.

**The stack is the better hypothesis, and it was never configured.**
`wasm-ld` defaults the shadow stack to 1 MiB. This binary builds GPU
parameter blocks as plain arrays BY VALUE -- `GpuVariationParams` is
`[f32; 1600]` (6.4 KB) on its own, `GpuParams` carries per-transform
arrays over 128+ transforms -- and `dist` builds with fat LTO and one
codegen unit, which inlines deep chains and sums their frames. A wasm
shadow-stack overflow does not announce itself: the stack pointer
walks out of its region and the next access traps as
`RuntimeError: index out of bounds`, from whatever callback was
running, with no Rust panic and no location. That is every symptom,
including why safe Rust could produce it and why the desktop (8 MB
stack) never does.

Raised to 16 MiB in `.cargo/config.toml` and both build scripts, and
VERIFIED IN THE BINARY rather than assumed: the shadow-stack global
reads 16,777,216 where it read 1,048,576. Address space only, no
runtime cost.

**The experiment answered.** The rebuild cleared SOME of the freezes,
so stack depth was a real contributor, and Chrome named the trap class
Firefox could not: `memory access out of bounds` -- linear memory, not
a table or indirect-call error. One case survives: Mandelbrot loaded
at zoom 176.

**What the desktop cannot show.** That view was run natively at 1, 2,
4, 8 and 16 MiB of thread stack, in release AND in debug (where
`debug_assert`s and overflow checks are live), at 160x120 and at
1920x1080 with 3x supersampling, on the browser's own budgeted orbit
path via `force_budgeted`. Every one passes. The fault does not
reproduce off the browser, and the shipped `dist` profile answers a
trap with an address and nothing else: stripped symbols,
`panic = "abort"`, and wrapping arithmetic, so `$func868` cannot be
told from an integer wrap three frames away.

So there is now a `dist-debug` profile, reachable as
`./build-wasm.sh --debug` AND `build-wasm.bat --debug` (both scripts --
the Windows one is what gets used here). Same optimisation level and
the same simd/codegen shape, but symbols kept (verified: a name
section and 6052 symbols in the bindgen output), panics unwound
through the console hook, and -- the part that actually finds things
-- **debug assertions and overflow checks ON**, so a `debug_assert` or
a wrapping subtraction fails at its source instead of corrupting an
index and trapping elsewhere. About 250 MB, and not for shipping; the
default path of both scripts is unchanged.

### The browser had no way to say "I am working" (2026-09-02)

Reported after the zoom-176 fix landed: a 1e3591 zoom (about 11,900 in
log2, 189 limbs) at 10M iterations renders for a very long time, UI
"slightly responsive", no errors, and nothing on screen to explain it.
The desktop has a progress overlay; the browser showed none.

Two separate reasons, and the second was a real defect:

**The overlay was `#[cfg(not(target_arch = "wasm32"))]`, and progress
was only published from the worker branch.** The browser needs it
MORE, not less: it has no worker thread, so a long reference is built
in slices on the frame loop and the canvas just sits there. Both are
now shared, under the same "only when the wait is worth naming"
threshold the desktop uses (`ORBIT_WAIT_SECONDS`), so an ordinary zoom
does not flash an overlay for two slices.

**The slice budget contradicted the crate's own cost model.** It was
`1_000_000 / limbs`, while a reference iteration is two truncated big
multiplies and therefore costs `iterations x limbs^2` — which
`predicted_orbit_seconds` states outright and is calibrated against a
measured reference (10,100,100 iterations at 197 limbs, 495 s). The
missing factor is `limbs`, so the error GREW with depth, which is
exactly backwards: the deeper the view, the more it over-asked.

| limbs | old budget | old slice | new budget | new slice |
|---|---|---|---|---|
| 2 | 50,000 | 0.3 ms | 50,000 | 0.3 ms |
| 16 | 50,000 | 16.2 ms | 50,000 | 16.2 ms |
| 64 | 15,625 | 80.8 ms | 5,799 | 30.0 ms |
| **189 (reported)** | 5,291 | **238.7 ms** | 664 | **30.0 ms** |
| 400 | 2,500 | 505.2 ms | 148 | 29.9 ms |
| 1000 | 1,000 | 1263.0 ms | 64 | 80.8 ms (at the floor) |

Shallow views are byte-identical — the ceiling covers them — so this
changes behaviour only where it was wrong. Total work is unchanged;
it is the same reference cut more finely. `one_reference_slice_stays_near_a_frame`
pins it against the cost model and needs no GPU, because the thing
that was wrong was the arithmetic.

The overlay shows COUNTS rather than an ETA
(`1,234,567 / 10,000,000 iterations`): `predicted_orbit_seconds` is
calibrated natively and a browser runs some multiple of it, so a
wall-clock estimate would be confidently wrong, while the counts are
exact and carry the scale — which is the part that matters at ten
million iterations.

### The slice fix unleashed the GPU chunk: a browser TDR (2026-09-02)

Reported right after the progress-overlay commit: console spam
("Escape supersample clamped to 6x at 397x708"), an intermittent
`memory access out of bounds` after loading several files, and -- the
one that was a regression -- a `DXGI_ERROR_DEVICE_HUNG` mid-render.

**The spam** was a warning placed above `resize`'s no-change early
return; the viewport calls `resize` every frame with the config's
factor, and a factor the budget clamps is clamped identically every
frame. Moved below the early return: once, when the clamp takes
effect.

**The TDR was caused by the slice fix, by removing an accidental
throttle.** Without timestamp queries (WebGPU exposes none by
default) the chunk sizer grows the dispatch from CALL SPACING -- the
time between `render()` calls -- on the premise that a call waits for
the GPU. In a browser nothing does: WebGPU cannot block on
completion, so the spacing is the frame period plus CPU work, and the
chunk doubles every third on-time call regardless of GPU cost, up to
64x the seed. At 397x708 with 6x supersampling that ceiling is ~51k
iterations over 10 MP in one dispatch -- a guaranteed multi-second
submission. It had never fired because the CPU orbit slice took
240 ms, so every call was "late" and the chunk sat pinned at 16.
Cutting the slice to 30 ms removed the throttle, and the chunk
climbed to the TDR within a few seconds.

Fixed by giving the browser what the desktop has: a MEASURED cost.
The no-timestamp path now times each recorded batch from
`on_submitted_work_done` -- an upper bound on GPU time, which is the
conservative direction -- and drains it through the same attribution
the timestamp pacer uses (`apply_gpu_measurement`), so `next_chunk`
sizes from `target / ms_per_iter` with its existing bounded growth.
Until the first sample lands the browser chunk stays at the
calibrated seed; the blind call-spacing growth is desktop-only now,
where a call really does wait. `a_completion_time_sample_reaches_the_chunk_sizer`
injects a sample as the callback would and checks it lands.

**Two things that made the loss unrecoverable in the browser, also
fixed.** The circuit breaker halved the budget in-session but
`tuning::save()` was a no-op on wasm, so every reload started at the
full budget and lost the device again; the storage backend already
had a localStorage `read_file`/`write_file`, and the tuning file now
lives there. And a lost device is not rebuilt in the browser at all
(no window to recreate a surface on) -- every call after it fails
quietly, which is the "freeze" -- so the viewport now says so:
"Reload the page; the render budget has been lowered for next time."

**The `memory access out of bounds` is still open.** Chrome's
wording fixes the class (linear memory), and the 16 MiB stack cleared
some cases, but this one recurs after several loads at the same
function (`$func868`) and does not reproduce natively. The
`dist-debug` build exists for exactly this; its trace names the
function and its overflow checks fire at the source.

### The browser drew every frame of a reference it did not have yet (2026-09-02)

Reported as "it looks like it is trying to draw the flame while
calculating the reference orbit, which is ruining performance". It
was: not the flame (the chaos game is correctly skipped in escape
mode) but the ESCAPE dispatch, running full-frame on every slice of a
reference that was nowhere near complete.

The desktop worker path has held the frame since it was written --
`if !done && slow { set_orbit_progress(...); return false; }` -- for a
reason recorded there: rendering against a small fraction of a long
reference is not progressive refinement, it is noise, because every
pixel rebases almost immediately and the frame is flat colour that
changes wholesale as the prefix grows. The browser arm published
progress but still dispatched, so each frame paid a full perturbed
pass over every pixel AND the CPU slice that is the actual work, with
the two competing.

Measured on the reported depth (189 limbs, 66,400 iterations,
1280x720), same view and same settle loop, dispatching every frame
versus holding:

| | wall clock | frames |
|---|---|---|
| dispatch every frame | 8.85 s | 105 |
| **hold while building** | **3.17 s** | 114 |

**2.8x**, and the extra nine frames are the point rather than a cost:
the held run defers pixel iteration until the reference is complete
instead of re-running it against every prefix. A handful of frames at
the end against ~100 full dispatches thrown away.

**The two arms now share one function.** They had already drifted:
the desktop test knob -- the ONLY coverage of the browser's path --
still carried the old `1_000_000 / limbs` budget after the real path
moved to the cost model, so it was testing something the browser does
not do. `budgeted_orbit_step` is now called by both, and the knob
cannot diverge from what ships.

### Why the browser's reference is slow, and the part that was a bug (2026-09-02)

Asked: deep-zoom reference building is much slower in the browser than
on the desktop -- is something slowing it down that we are not
noticing? Yes, one thing, and it was quadratic.

**First, what is NOT the cause.** The obvious suspect is the multiply:
wasm has no hardware 64x64 -> 128 product, so the browser runs a u32
half-limb COLUMN form where the desktop runs a u128 row scan. Measured
natively, both compiled, at the depth of the report:

| limbs | row scan (desktop) | columns, scalar | with simd128 (est.) |
|---|---|---|---|
| 64 | 2.8 us | 6.4 us | ~3.4 us (1.21x) |
| 128 | 9.6 us | 19.3 us | ~10.2 us (1.05x) |
| **189** | **20.1 us** | 38.9 us | **~20.5 us (1.02x)** |
| 256 | 36.1 us | 66.4 us | ~35.0 us (0.97x) |

At 189 limbs simd128 buys back exactly what the u32 split costs: the
browser's multiply is at PARITY with the desktop's. Nor is it
parallelism -- `PAR_THRESHOLD_LIMBS` is 192, so a 189-limb reference
is serial on the desktop too. And Karatsuba is not the missing lever
either: `mul_trunc` is TRUNCATED and already computes only the ~n^2/2
products inside its window, which is the same factor of two Karatsuba
would buy at this size.

**The bug was the upload.** Every call to the non-progressive orbit
path re-sent all four GPU buffers from offset zero and recomputed
`r2_channel` over EVERY entry to do it. Harmless while only the CLI
came here -- one call, orbit already complete -- and the comment said
so outright ("append-only uploads are a later optimization; a full
orbit at max_iter 100k is 800 KB"). But the browser calls it once per
FRAME SLICE, so the cost went quadratic in the slice count: a
reference of N iterations built in N/budget slices re-derived O(N)
entries on each. Measured at 189 limbs, 640x480:

| max_iter | before | after |
|---|---|---|
| 66,400 | 2.92 s | 2.90 s |
| 199,200 (3x) | 10.62 s (3.6x) | 8.82 s (3.0x) |
| 398,400 (6x) | 22.33 s (7.6x) | 17.94 s (6.2x) |

Superlinear before, linear after. The 20% at 600 slices is not the
point -- the term grows as slices squared, and the reported view
(10M iterations, ~15,000 slices) was paying on the order of tens of
minutes of pure re-derivation. Fixed by appending only the tail, with
the same from-scratch guard the worker path carries for a
buffer-capacity crossing mid-growth -- that one is not hypothetical,
it rendered a structurally wrong frame a user caught by eye.
`a_sliced_orbit_upload_renders_what_a_whole_one_does` renders the same
view both ways and demands they agree exactly, because the failure
mode here is a wrong image rather than a crash.

**What is left is inherent**, and worth stating so it is not chased
again: wasm executes this integer code somewhat slower than native,
and the browser has no worker thread, so the reference is built in
frame slices instead of continuously on a spare core. Closing that
needs a Web Worker, which is infrastructure rather than a fix.

**A latent test flake, found on the way.** `the_new_tiers_take_the_perturbed_path_in_a_plain_render`
asserted on `diag::snapshot().path` -- a process-global, while cargo
runs tests in parallel. `EscapeRenderer::last_path` exists precisely
for this and says so in its own doc comment. It passed until the new
longer-running tests started interleaving with it; it now reads the
per-renderer field.

### The workspace follows the fractal that was loaded (2026-09-02)

An escape fractal opened from a file or the online browser arrived
into whatever layout was up -- usually Standard, which carries no
Escape panel at all, so nothing on screen could edit the thing that
had just been loaded.

The hook is `ConfigManager::load_generation`, a counter bumped only by
`load_config` -- preset, file import, and the browser/API path all go
through it, while animation playback (`load_config_silent`) and the
animation-exit undo snapshot (`load_config_with_explicit_before`)
deliberately do not, so playback cannot rearrange the workspace under
the user. The app compares it once per frame and switches after the
UI has drawn, for the same reason the panel-requested layout change
does: the workspace is borrowed for the whole frame, and switching
mid-draw would rebuild the dock tree the caller is still walking.

BOTH directions, because the Escape layout deliberately carries none
of the flame-only editors -- a flame loaded while it is up is the same
trap in reverse. Compact mode is single-panel, so it opens the panel
instead, mirroring what an animation opened from a URL already does.
An explicit layout choice survives everything except loading a
fractal of the other kind.

### The macOS visual divergence: one lead closed, one opened (2026-09-02)

Follow-up to the `powi` fix, which said plainly what it did not solve:
the M2 runs the visual suite at 217/238, and all 21 failures are
escape-time -- 14 deep-zoom (every one at `zoom_log2 >= 13.5`) and 7
transcendental-heavy formulas.

**The caveat it left is answered, and the answer is no.** That commit
predicted "the mechanism is platform-independent, so this almost
certainly fails on Windows too... it deserves a run on the main
machine". Run: `2f64.powi(e)` was compared against the exact bit
pattern for EVERY `e` in -1074..=0 with a `black_box` exponent, and it
is exact at all of them on this Windows x86-64 toolchain. `powi(-1060)`
returns 8.095e-320 here where the M2 returns literally 0. So the trap
is real but NOT platform-independent in practice -- it is a property
of that target's `__powidf2` lowering. The fix stays, because building
powers of two from the bit pattern is exact everywhere and cheaper
than a libcall; only the prediction was wrong.

**The atan2 lead is closed too, and closed clean.** Metal's `atan2` at
a zero pair is the documented hazard that cost `npolar` 73% of its
pixels, and `ff_atan2` -- the guard -- lives in the FLAME shader
library, which escape WGSL cannot see. Escape has ten raw `atan2`
sites. Every one of them turns out to be already guarded by an
explicit magnitude test, several with comments naming this exact
hazard: `esc_cpow` and `esc_clog` return early below `r2 < 1e-30`, the
stripe / basin / position colorings all test their accumulator first,
and the orbit-trap spiral returns 1e30. Not a lead; a thing already
done.

**The open lead is the deep rung's double-float arithmetic**, and it
fits the shape of the failures exactly. The floatexp rung carries its
delta as hi+lo f32 pairs, and 36 call sites route every complex
multiply and square through `df_add` / `df_mul`, both of which bottom
out in `df_two_sum` / `df_quick_sum`:

    fn df_quick_sum(a, b) { let s = a + b; return vec2(s, b - (s - a)); }

An error-free transform is only error-free under strict IEEE
evaluation. A compiler permitted to REASSOCIATE folds `s - a` to `b`,
so the error term becomes `b - b` = 0 and the double-float silently
degrades to plain f32 -- 2^-24 where the algorithm assumes 2^-48.
Metal runs shaders with fast-math on. That predicts divergence that is
depth-dependent (the deep rung only engages past the perturbation
threshold), structural rather than a tone offset, and perfectly
reproducible -- which is what the suite reports, down to the 13.5
floor.

Worth noting the near-miss: `df_split` WAS made immune with a bitmask,
and the comment above the block claims integer-op immunity for the
whole of it. The split is immune. The two sums are not.

`the_deep_rungs_error_free_transforms_are_not_folded_away` settles it
in one run. It lifts the shipped helpers out of a real assembled
deep-rung shader by brace matching -- not a copy, so it cannot drift
-- and feeds them `1e-8` added to `1.0`, which is below half an ulp,
so the sum rounds to exactly 1.0 and the whole addend IS the error
term. On Vulkan/Windows both return `1e-8` (measured; the test
passes). If they return 0 on Metal, the mechanism is identified rather
than suspected, and the fix is to rebuild those two transforms out of
operations a reassociating compiler cannot touch -- as `df_split`
already is.

### The double-float fold is not a Mac problem (2026-09-02)

The confirming run the previous commit asked for, on Windows/Vulkan.
It predicted the corrected probe "should still pass" here. It does
not, and that changes the conclusion.

    opaque inputs (what the deep rung runs):
      df_two_sum   lo 0        df_quick_sum lo 0
      df_split     lo 1.19e-7  df_two_prod  lo 1.42e-14

Same zeros as the M2. The earlier "no folding on Windows" reading was
itself the constant-folding artifact the previous commit identified --
it was measuring the compiler's constant folder on both platforms.

**The proof is an internal contradiction**, printed by the test rather
than argued. In ONE shader, on opaque storage-buffer inputs:

| expression | value |
|---|---|
| `s == a` | true |
| `s - a` | 1e-8 |
| `b` | 1e-8 |
| `b - (s - a)` | 0 |

`s == a` and `s - a != 0` cannot both hold under evaluation. They hold
because `(a + b) - a` was REWRITTEN to `b` -- not rounded differently,
rewritten. Bitcast laundering does not help: a `bitcast<u32>` round
trip is the identity, and the optimizer sees through it.

**It is the driver, not our toolchain.** naga -- the WGSL front end
wgpu uses -- emits one OpFAdd and two OpFSub for that expression,
exactly the source. Pinned by
`naga_emits_the_error_free_transform_faithfully`, which needs no GPU
and so runs in the default suite: if a future naga starts folding this
itself, the fix moves from "work around the driver" to "configure the
front end", and nothing else would notice.

**What this does and does not mean.**

- The deep rung's double-float is degraded on EVERY platform tested,
  not just Metal. `df_add` bottoms out in both zeroed transforms;
  `df_mul`'s `df_two_prod` survives (its split is the immune bitmask)
  but its result then passes through `df_quick_sum`, whose low half is
  zeroed. So the pair degrades to plain f32 after one operation --
  2^-24 where the algorithm assumes 2^-48 -- and the extra
  instructions buy nothing.
- It therefore CANNOT explain the macOS-versus-Windows divergence.
  A defect present identically on both is common-mode; it cannot
  produce a difference between them. The previous commit's
  "hypothesis CONFIRMED" is right about the mechanism and wrong about
  what it accounts for. The 14 deep-zoom failures still need a cause,
  and this is no longer a candidate for it -- though the two platforms
  folding in DIFFERENT places remains possible and unmeasured.
- The accuracy tests do not contradict this: the deep-rung Ducks
  checks pass at ~1.7e-7 mean error, which is f32-consistent. They
  were calibrated against what the rung actually produces, so they
  cannot distinguish 2^-24 from 2^-48.

**Not fixed here**, and the fix is not obvious. The immune primitive
in the file (`df_split`) is immune because it is integer work, and an
error-free SUM has no equally cheap integer form; the options are an
exponent-aligned integer construction, or accepting f32 on the deep
rung and deleting the machinery that is not paying for itself. Either
changes deep-zoom arithmetic on every platform and wants its own pass
over the visual suite.

### A driver rewrite fingerprint, and what it says about FMA (2026-09-02)

The df fold is common to Vulkan and Metal, so it cannot produce a
DIFFERENCE between them. The surviving possibility is the two drivers
rewriting in different PLACES, and
`driver_rewrite_fingerprint` measures that: identities true in real
arithmetic and false in floating point, on opaque inputs, verdict per
row. Run it on both platforms and diff the column.

Every row CHECKS ITS OWN DISCRIMINATION FIRST, in Rust f32, before the
GPU answer is interpreted. That guard earned itself twice
immediately: a first cut reported `(7*3)/3 == 7` as a rewrite (both
values are exact in f32, so IEEE agrees and the row said nothing), and
the assertion then caught `sqrt(3)^2 == 3` being exact too. A verdict
read off a row that does not separate the two forms is worse than no
verdict.

**Windows / Vulkan / NVIDIA:**

| identity | verdict |
|---|---|
| `(a+b) - a` | REWRITTEN to `b` |
| `(x+y)+z` vs `x+(y+z)` | evaluated -- NOT reassociated |
| `a*b + c` | **CONTRACTED to fma** |
| `(m*n)/n` vs `m` | REWRITTEN |
| `m/n` vs `m*(1/n)` | SUBSTITUTED |
| `sqrt(v)^2` vs `v` | REWRITTEN |
| `(-0) + 0` | REWRITTEN to `-0` (IEEE says `+0`) |

The second row is the interesting negative: this driver does NOT
reassociate in general, yet does cancel a common term in `(a+b) - a`.
The fold is a specific narrow rewrite, not blanket fast-math.

**On switching a CPU twin to `fma`:** measured, the GPU contracts
`a*b + c` here -- separate gives 0 for the chosen inputs, fused gives
1, and the GPU gives 1. So a twin using `mul_add` would match this
platform's arithmetic where a twin using separate ops does not. That
is a real basis for the change.

The caveat is the whole reason this file exists, though: contraction
is a DRIVER choice, and this table is one platform's. If Metal does
not contract, or contracts elsewhere, hard-coding `mul_add` in the
twin makes it right on Windows and wrong on macOS -- the same trap in
mirror image, and harder to see because the twin is what we would then
be trusting to judge the GPU. The fingerprint wants running on the M2
and diffing before that change lands.

**macOS / Metal / M2 (2026-09-03):**

| identity | Windows / Vulkan | macOS / Metal |
|---|---|---|
| `(a+b) - a` | REWRITTEN to `b` | REWRITTEN to `b` |
| `(x+y)+z` vs `x+(y+z)` | evaluated -- NOT reassociated | **REASSOCIATED** |
| `a*b + c` | CONTRACTED to fma | CONTRACTED to fma |
| `(m*n)/n` vs `m` | REWRITTEN | REWRITTEN |
| `m/n` vs `m*(1/n)` | SUBSTITUTED | SUBSTITUTED |
| `sqrt(v)^2` vs `v` | REWRITTEN | REWRITTEN |
| `(-0) + 0` | REWRITTEN to `-0` | REWRITTEN to `-0` |

**One row differs, and it is the one that was the useful negative on
Windows.** Metal reassociates general float sums; Vulkan/NVIDIA does
not. Six of seven rows agree, so this is not "Metal is loose and
Vulkan is strict" -- it is one specific, measured divergence in
rewriting, which is exactly the shape the previous entry predicted and
could not yet find.

That makes it the leading candidate for the 21 macOS failures, and it
fits both clusters rather than just one. Any expression with three or
more float terms may be summed in a different ORDER on Metal, so its
rounding differs -- depth-gated where the deep rung sums the most
terms per iteration, and equally reachable in the shallow
transcendental formulas (`weierstrass`'s series, `collatz`'s
`esc_ccos`, `lambda_sine`, `tetration`) whose failures the df fold
never explained. It is depth-dependent, structural rather than a tone
offset, and perfectly reproducible: the profile the suite reports.

Worth being clear about the scope: general reassociation is far wider
exposure than the df helpers. Every flame visual test passes on
macOS, so it is not breaking stochastic renders -- a chaos game
averages over sample order. It bites where arithmetic is
precision-critical and the result is a single deterministic value per
pixel, which is escape-time all over.

**The FMA question the previous entry left open is answered:** Metal
contracts `a*b + c` too, and its explicit `fma()` returns the fused
value as expected. So a CPU twin using `mul_add` matches BOTH
platforms' arithmetic, and the mirror-image trap the entry worried
about does not exist here. That change can land on this evidence.

**On regenerating the Windows baselines:** the full suite reads
**238/238 on Windows** with the current tree, so the baselines already
represent this platform's output and regenerating them changes no
verdict here. It only means something if code has changed --
and then it is not a refresh, it is a decision to move the reference,
which wants the diff read first. What a cross-platform comparison
actually needs already exists: a suite run leaves 238 full-resolution
renders in `tests/visual/current/` (untracked), against baselines that
are normalised thumbnails (160x120, metadata stripped) compared with a
tolerance rather than a hash -- so `current/` is the artifact to diff
between machines, not `baseline/`.

### The f3 location on macOS: the same fix, the same size (2026-09-03)

The Windows column measured the df fix at `output/f3-final.fflame`
(zoom_log2 9316.69, 197 limbs, 10,100,100 iterations, 3756-digit
centre). Here is the M2's, same config, same 640x384, reference served
from the orbit store so both builds iterate an IDENTICAL orbit and
only the DF arithmetic differs. Pre-fix build from a worktree at
`fef93691`, the commit before the hybrid.

| | Windows / Vulkan | macOS / Metal |
|---|---|---|
| pixels differing | 6.66% | **6.91%** |
| max channel delta | 203 | **192** |
| past 40 | 0.106% | **0.105%** |

The two platforms move almost identically, which is what the fold
being common-mode predicts: the same defect removed on both, so the
same pixels move. It is a consistency check on the fix rather than a
new finding, and it is the first time the two columns have agreed
this closely on anything.

Two things confirmed on the way. **macOS renders z9316 correctly** --
the double-scepter valley with spiral filigree, no glitch dust, no
interior collapse, `sd` 59.7 across the frame. And the cost really is
invisible at this depth: 3.15 s warm, dominated by loading the 14 MB
reference rather than the GPU pass, matching the Windows observation
that the ~24% DF penalty bites where the GPU dominates -- roughly zoom
48 to a few hundred -- and not at f3-class depths.

The differing pixels are ISOLATED points scattered through the
filigree, not a structural region: mean absolute delta 0.132 across
the frame against a max of 192. That is the signature of near-boundary
pixels whose escape iteration flips, which is what a change in delta
precision should do and where the direction question lives.

**Independently confirmed by eye:** the macOS frame matches both the
Windows render and an online reference image of this location. That
rules out the class of concern a pixel statistic cannot -- neither
build is grossly wrong at f3, and the fix introduces no visible
regression at the depth DF was built for, which is what "safe to
ship" needs. It cannot settle direction, and for the same reason it
is reassuring: a mean absolute delta of 0.132 is invisible by
construction, so no visual comparison can discriminate the two.

**Still not established: which render is correct.** The Windows entry
is right that a difference is not a direction, and the M2 column adds
no direction either -- two platforms agreeing tells us the fix is
consistent, not that it is right. What would settle it, and needs no
reference-independence (the thing that defeated both earlier attempts,
since orbit relocation collapses back onto the same orbit): take
pixels where the two builds disagree and compute their escape
iteration EXACTLY, with `ReferenceOrbit::compute` at the pixel's own
coordinates rather than the view centre's. That is the same
fixed-point machinery the z700 wall was settled with, it is ground
truth rather than a better approximation, and the per-pixel iteration
counts can be read directly through the `IterRecord` path instead of
being inferred from colour. Cost is roughly a reference build per
pixel, so a handful of pixels, not a frame.

### weierstrass-hillshade is trig range reduction, not reassociation (2026-09-03)

Reassociation was the standing candidate for the 21, so
`weierstrass-hillshade` was picked to test it at a real site: mean
2.32, **zero** pixels past the outlier threshold, no deep rung, and an
accumulation that is a serial 24-term dependency chain reassociation
cannot reorder. It is not reassociation. It is the generator.

The field is `sum a^n * cos(b^n x) * cos(b^n y)`, a=0.55, b=2, 24
terms, plane spanning +-2 -- so the last octaves evaluate `cos` near
`2^23 * 2` ~ 1.7e7, where one f32 ulp is about 2 radians, a third of a
period. `trig_accuracy_across_the_weierstrass_octaves` hands identical
f32 arguments to the GPU and compares against an f64 reference of the
same bits. On the M2:

| octave | max abs cos error |
|---|---|
| 0-11 | 1.4e-4 |
| 16 | 5.5e-3 |
| 20 | 1.1e-1 |
| 23 | **7.8e-1** |

`cos` is bounded by 1, so an error of 0.78 means the returned value is
unrelated to the cosine of that argument. Metal's argument reduction
gives up long before 1.7e7.

**The amplification is the whole story, and it is structural.** The
value series is weighted `a^n` (shrinking) and comes out fine; the
GRADIENT series is weighted `a^n b^n = (ab)^n` with ab = 1.1, which
GROWS with the octave -- so the hillshade is dominated by exactly the
terms where the trig is meaningless:

| series | weight | relative error |
|---|---|---|
| value | 0.55^n, shrinking | 3.3e-6 -- negligible |
| gradient | 1.1^n, growing | **14.1%** |

A ~14% error in the shading normal is a broad luminance shift with no
localised blowup, which is precisely the failure's signature: high
mean, zero outlier pixels.

The irony is worth recording, because it is the actual mathematics:
`ab >= 1` is the Weierstrass NOWHERE-DIFFERENTIABILITY condition. The
analytic gradient series does not converge -- that is the point of the
function -- so asking for it in f32 over 24 octaves is asking for a
number that does not exist. Both platforms compute garbage; they
compute DIFFERENT garbage, and the render diff is that difference.

**This is fixable, and it is not the accepted-divergence class.**
CLAUDE.md's third clause covers trig whose argument exceeds f32
resolution, where no reference class exists. That does not apply here:
b = 2, so `b^n * x` is exact in f32 (power-of-two scaling), the
argument is an exact value, and `cos` of it has a well-defined true
result both platforms could agree on. What is missing is accurate
range reduction. Reducing mod 2*pi in double-f32 before calling `cos`
-- the machinery the previous commit just repaired -- would give the
true value on both drivers and make them agree. Cost is a few ops per
octave, on a 24-term loop.

**Scope beyond this test.** The same shape covers most of the shallow
cluster the df fold never explained: `collatz` (`esc_ccos` of `pi*z`
with z growing), `lambda_sine`, `tetration`, `standard_map_ftle` all
feed unbounded arguments to trig. So the 21 now split into two
mechanisms rather than one -- range reduction for the shallow
transcendental failures, general reassociation still the candidate for
the deep-zoom cluster -- and neither is the df fold.

Caveat, in the file's own discipline: only the Metal column is
measured. The render diff already proves the platforms differ; the
Windows run of this probe would say whether NVIDIA reduces better,
worse, or merely differently, and the probe carries a self-check that
the low octaves are accurate so a broken buffer cannot be read as a
result.

### The df fold, fixed: a barrier the drivers cannot rewrite (2026-09-03)

`df_two_sum` / `df_quick_sum` / `df_two_prod` now launder every
rounding-sensitive intermediate through `df_l`, a bitcast-XOR against
an always-zero uniform field (`PerturbParams.df_zero`). Identity at
runtime, opaque to the compiler, and INTEGER work -- the same property
that made `df_split` immune all along. The probe passes on Metal for
the first time: `lo` comes back `1e-8` and `1.42e-14` instead of `0`.

**Placement is not negotiable, and the literal column is the canary.**
Laundering only the sum fails: `b - (s - a)` reassociates to
`(a + b) - s` and cancels there instead. A single barrier on the
cancelled operand passed the opaque path but read `0` on the LITERAL
path -- i.e. still reassociable, surviving on which shape the
optimizer happened to pick. Full laundering reads correct on both
columns, which is the bar.

**`df_two_prod` uses `fma(a, b, -df_l(p))`**, not a laundered Dekker
split. Both are exact; the fma form is one unrewritable operation
where the split leaves the sum that combines its four partial products
still reassociable. It is also ~4x cheaper (below).

**Cost, on the rung that pays it.** Only 5 configs in the visual
corpus run above `PERTURB_FLOATEXP_ZOOM = 48`:

| variant | DF-rung median | worst |
|---|---|---|
| laundered Dekker split | +108% | +347% |
| **fma (shipped)** | **+28%** | **+75%** |

**What it changes, and what it does not.** 5 of 87 escape renders move
-- and 4 of those 5 are the corpus's only DF-rung tests, so the hit
rate on the code that runs it is ~80%, not 6%. It does NOT close the
macOS/Windows gap: the fold was common-mode, present identically on
both platforms, so removing it cannot produce a difference that was
not there. The 21 macOS failures still point at general float-sum
reassociation on Metal, the one divergence the fingerprint found.
This is a correctness fix for the deep rung, and should be judged as
one.

**Baselines are unchanged deliberately.** They record the folded
arithmetic on both platforms; moving them is a decision to move the
reference, which wants the Windows diff read first. macOS still reads
66/87 on the escape category, the same 21, with 4 renders' metrics
shifted.

### The fma suggestion, measured at the site it would change (2026-09-02)

The M2 column answered the FMA question -- both drivers contract
`a*b + c` -- and that was read as clearing "switch the CPU twin to
fma". Measured at the actual site, on Windows, it does the opposite:
the mirror already matches and the change would break it.

The site is the complex multiply, which CPU and GPU write identically:

    CPU  bla.rs:68     re: self.re * o.re - self.im * o.im
    GPU  assembler.rs  a.m.x * b.x - a.m.y * b.y

`which_fma_form_matches_the_gpu_complex_multiply` computes it on the
GPU from opaque inputs and compares all three CPU candidates
BIT-EXACTLY:

| form | bits |
|---|---|
| GPU `x*y - z*w` | `c0800004` |
| GPU explicit `fma(x,y,-(z*w))` | `c0800005` |
| CPU separate | **`c0800004`** -- matches the GPU |
| CPU `mul_add` (either association) | `c0800005` |

**The driver does not contract this expression.** That is not a
contradiction of the fingerprint: the fingerprint's FMA row is
`a*b + c` with `c` a plain value, and the GPU fuses THAT (separate
gives 0 for its inputs, fused gives 1, the GPU gives 1). Product minus
PRODUCT is a different shape and this driver leaves it alone. So
"contracts multiply-add" is true and does not transfer to the site the
change was aimed at -- the shapes have to be measured where they
occur, not inferred from a neighbouring row.

On this evidence the `bla.rs` mirror should stay as it is on Windows.

**And the probe is worth running on the M2 for its own sake.** If
Metal fuses `x*y - z*w` where Vulkan does not, that is a SECOND
measured divergence, sitting directly in the CFe complex multiply that
the BLA table and the deep rung both depend on -- a candidate cause
for the deep-zoom cluster independent of general reassociation, and in
a much narrower place. If Metal also declines to fuse it, the row is
common-mode like the df fold and the mirror is simply correct
everywhere.

**M2 result (2026-09-03): Metal also declines.** Bit-identical to the
Windows column, same inputs, same three candidates:

| form | Windows / Vulkan | macOS / Metal |
|---|---|---|
| GPU `x*y - z*w` | `c0800004` | `c0800004` |
| GPU explicit `fma(x,y,-(z*w))` | `c0800005` | `c0800005` |
| CPU separate | **`c0800004`** matches | **`c0800004`** matches |
| CPU `mul_add` (either association) | `c0800005` | `c0800005` |

So this row is common-mode: the second divergence it was probing for
is NOT there, the CFe complex multiply is ruled out as a cause of the
deep-zoom cluster, and `bla.rs`'s separate-ops mirror is correct on
both platforms rather than only on Windows. Explicit `fma()` is fused
on both, which is the property the laundered `df_two_prod` relies on;
it is contraction of the WRITTEN form that neither driver performs
here.

The correction stands as recorded: the earlier "that change can land"
was inferred from the fingerprint's `a*b + c` row rather than measured
at the complex multiply, and inference across shapes is exactly what
this file keeps catching. Two drivers, seven fingerprint rows and one
site probe in, the tally is one divergence (general reassociation) and
everything else common-mode.

### The df-launder cost, measured on Windows (2026-09-02)

The hybrid landed with macOS numbers and no Windows column. Measured
here against the pre-hybrid commit built in a worktree, so this is the
actual before/after rather than a reconstruction. Wall clock, min of
9, GPU time isolated by subtracting a 64x48 run of the SAME config
from a 2400x1800 one -- process start and the reference-orbit build
are resolution-independent, so the difference is the dispatch.

| rung | config | pre | post | delta |
|---|---|---|---|---|
| DF | manowar-deep | 1.420 | 2.085 | **+46.8%** |
| DF | multibrot-dip-seam | 0.343 | 0.432 | +25.9% |
| DF | phoenix-past-floatexp | 0.209 | 0.259 | +24.2% |
| DF | fe-threshold-floatexp | 1.247 | 1.541 | +23.6% |
| DF | fe-zoom-60-edge | 0.769 | 0.843 | +9.7% |
| DF | ship-deep-floatexp | 0.647 | 0.651 | +0.5% |
| scaled | deep-zoom-30 | 0.376 | 0.376 | -0.1% |
| scaled | deep-zoom-42 | 0.472 | 0.466 | -1.3% |

**DF median +23.9%, worst +46.8%; non-DF unchanged.** The scaled-rung
controls at -0.1% and -1.3% are the attribution: `df_l` exists only in
the floatexp template, and paths that never reach it do not move. That
tight a noise floor is also what makes the DF column readable.

Against the macOS figures (+28% median, +75% worst) Windows pays
somewhat less, same order. No platform-specific regression.

**A correction to the config set, which affects both columns.** The
DF rung is entered by `zoom_log2 > 48` OR the Manowar tier, which
takes the deep rung AT EVERY DEPTH. `manowar-deep` sits at zoom 26, so
a zoom-only filter misses it -- and it is the WORST-affected config in
the corpus at +46.8%, being a two-term recurrence with 72 bytes of
per-pixel state and correspondingly more DF work per iteration. The
corpus has SIX DF-rung tests, not five.

**Two things this rules out.** An export of
`fe-threshold-floatexp` at 4200x2150 loses the device -- and the
PRE-hybrid binary loses it at the same size, so that TDR is inherent
to the config at an extreme size, not something the slowdown caused.
Both complete at ordinary export sizes. And the flame corpus is
untouched: the full suite reads median 51 Miter/s with every flame
test passing.

**Visual state on Windows: 236/238**, the two failures being
`escape-fe-threshold-floatexp` (mean 4.64) and `escape-manowar-deep`
(mean 4.81) -- both DF-rung, both renders the fix legitimately moves,
against baselines that still record the folded arithmetic. macOS saw
five change; Windows sees two exceed tolerance. The baselines remain
deliberately un-regenerated.

### Does the df fix matter? Measured at the f3 location (2026-09-03)

The hybrid commit said the fix "matters most where the corpus cannot
see: the deepest test here is z426, and DF was built for the z9000
class". The f3 file IS that class, so it was the place to look.

`output/f3-final.fflame`: zoom_log2 9316.69 -- 10^2804 -- 197 limbs,
10,100,100 iterations, a 3756-digit centre, imported from
`output/001.f3.toml` (fraktaler-3's own parameter file, zoom "4e2804",
period 1137764). Rendered 640x384 with the reference served from the
orbit store, so both builds iterate an IDENTICAL reference and only
the DF arithmetic differs:

| | pre-hybrid | post-hybrid |
|---|---|---|
| pixels differing | — | **6.66%** |
| max channel delta | — | 203 |
| pixels past 40 | — | 0.106% |

**So the fix is not inert where DF exists to work.** That is the
evidence the hybrid commit could not supply from the corpus, and it is
the strongest argument for keeping it: the deep rung's arithmetic
changes materially at the depth it was built for.

**And the cost is invisible there.** The same renders: 9.85 s
pre against 9.33 s post, both dominated by loading a 14 MB reference
rather than by the GPU pass. The measured +24% DF penalty bites in the
band where the GPU dominates -- roughly zoom 48 to a few hundred --
not at f3-class depths, where the reference build or load is the whole
cost.

The z700 / z900 / z1100 ladder in `output/` renders with full
structure (luminance sd ~47) on BOTH builds, diverging 6.5% / 3.9% /
2.0% of pixels as depth increases. The "interior stall at z900+"
recorded earlier in this file is gone -- fixed by the periodic
reference work, not by DF.

**What is NOT established: which render is correct.** A difference is
not a direction. Reference-independence would answer it (this file
already treats reference-dependent fine structure as the symptom of a
starved delta), but the two attempts here were inconclusive: a
sub-pixel centre perturbation was absorbed by orbit RELOCATION -- the
cached reference re-anchored rather than rebuilding, so both renders
used the same orbit and came out bit-identical -- and adding f3's
`reference.period` produced a 4-minute build after which both builds
still matched their plain-reference renders exactly, which is equally
consistent with the hint being verified and discarded. Settling it
wants a reference the pipeline cannot collapse back to the same orbit.

**Verified.** Against exact orbits at zoom 30, both rungs: Newton
schemes 0/1/2 over `z^p - 1` and the relaxed map at 0.00% outcome
mismatches; Nova 0.00%; Kaliset 8.5e-7 / 5.4e-8 mean relative error
on both sign branches; Ducks 1.1e-6 (variant 0) and its variant-4
view. Against the DIRECT path at shallow zoom, block-mean compared:
Newton every scheme and the relaxation, Nova, and Ducks both variants
and both planes, all on both rungs. Kaliset is deliberately absent
from that comparison -- forcing it below its floor would test it
outside the regime the engine ever uses it in.

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

**The function picker shipped 2026-09-01** (Newton only; Nova still
takes the roots of unity alone). Six f(z), Wikipedia's Newton-fractal
gallery: `z^p - 1`, `z^3 - 2z + 2` (Newton's classic FAILURE case --
the critical point falls into an attracting 2-cycle, so a whole region
converges to no root), `z^8 + 15z^4 - 16`, `sin z - 1`, `cosh z - 1`,
`z^p sin z - 1`. Appended as a `func` parameter, so every stored config
keeps its slots and its picture. Derivatives are held to central finite
differences of f in `newton_function_derivatives_are_consistent` -- the
check a hand-transcribed f' needs, and it caught a seeded coefficient
error when sabotaged.

Two things the picker forced into the open:

- **`root_basin`'s angle buckets are a basin index only for
  `z^p - 1`.** Roots elsewhere are not evenly spaced on a circle
  (`z^3 - 2z + 2` has one real and two conjugate; `sin z - 1`'s all sit
  ON the real axis), so a "General" key folds in log|z|. It is a
  discriminator, not an index, and it applies only to CONVERGED orbits
  -- keying on a non-converged final iterate painted the
  transcendentals' large wandering regions as a smooth rainbow that
  looked like structure and was not.
- **The escape bailout is wrong for a root-finder.** Newton's iterates
  wander far outside the unit disc before settling, and a function
  whose ROOTS lie past the bailout (`z^8 + 15z^4 - 16` has four at
  |z| = 2) has every one of them classified as an escape -- the basins
  vanish and the view renders flat. `EscapePreset` gained an optional
  `bailout` so the shipped presets carry 1e6. The principled fix is to
  make Newton `NonEscaping` the way Novaretti is (a root-finder has no
  escape criterion at all); that changes existing `z^p - 1` renders, so
  it wants sign-off rather than a silent flip.

Still open here: Schroder/Householder-3/Koenig (they want the third derivative), the
function picker for Nova, and Nova's critical-point seed -- its z0 is
hardcoded to 1, which is exact for `z^p - 1` at relaxation a = 1 and
wrong elsewhere (the critical point solves `z^p = a(p-1)/(p-a)`).

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

## 8b. Interactive latency: diagnostics and the recolor cache

Measured (the `interactive_latency_report` GPU test, 960x720,
mandelbrot, max_iter 2000): the perturbed compute pass costs ~6x the
direct pass for the IDENTICAL view (14.2 ms vs 2.3 ms per full
frame), and before the cache every edit — palette, coloring param,
relief slider — re-ran the whole iteration. That was the "noticeable
delay past zoom 14".

*The recolor cache splits iteration from coloring at the template
level.* Each iterate template (direct, perturbed f32, perturbed
floatexp — so every formula and all perturbation tiers inherit it)
writes a 32 B/px terminal record on its finishing pass: z, dz, n,
escaped/converged/period, and the coloring accumulator. A standalone
recolor pass — assembled per COLORING, a transcription of the iterate
templates' tail — re-runs `coloring_map` + palette lookup from the
records. When a frame's *iterate identity* (formula + params + view +
max_iter + bailout + damping + the derivative/interior compile flags,
plus the coloring's identity when it participates in iteration via an
accumulator or period test) matches the settled records, the frame is
one recolor dispatch: **coloring edits at zoom 25 went from ~70 ms to
~0.4 ms**, independent of iteration depth. The Escape panel's
Diagnostics section shows the live attribution (path = recolor on a
hit).

Correctness is pinned by `recolor_cache_is_invisible_and_invalidates_
correctly`: a cache-hit frame is BIT-IDENTICAL to a fresh render for
a map-only coloring (both paths), a derivative coloring (dz flows
through the records), and an accumulator coloring — whose param tick
must MISS, since the accumulator ran under the old params — and a
view change misses everywhere. Records live in one storage buffer
sized by the render grid; a pixel count past the device's binding
limit (giant CLI exports) gates the write off via params.flags bit 3
and renders uncached, exactly as before.

*Reference relocation closes the other half.* A pan or zoom-to-cursor
event used to recompute the reference orbit — the center strings were
part of its identity — even though perturbation never needed the
reference AT the view center (rebasing serves any nearby point).
`ReferenceOrbit::relocate_to` re-anchors the existing orbit under a
moved view: the new center is parsed at the orbit's own precision and
subtracted from the exact fixed-point reference `c`, so a drag of
hundreds of events accumulates NO error, and the offset rides the
same `ref_offset` machinery the nucleus path built. Wired into the
worker's reuse check and the blocking cache; parameter plane only (a
Julia orbit's reference is its seed, not retained in fixed point) but
every parameter-plane tier — power, ship, tricorn, phoenix, manowar,
lambda, feather, mcmullen, magnet — inherits it. Capped at 8192 px of
offset (the f32 d0 sum's precision budget; the nucleus path documents
the wall at 2^15): a drag reuses the reference for ~10
viewport-heights before one recompute re-centers it. Measured: a pan
at zoom 25 went from ~120 spin-frames of worker roundtrip to 2 frames
with `orbit=reused`; no GPU re-upload (the orbit generation is
unchanged) and no BLA rebuild (dc growth handles a widened offset on
its own). Pinned by `pan_reuses_the_reference_orbit`: the diag must
show a relocation and no rebuild, the relocated render must agree
with a FRESH reference at the new view to <3% of pixels (measured
1.27%, the reference-vs-reference noise band at zoom 30 — a sign
error in the offset reads 79.5%), and an out-of-range pan must fall
back to a rebuild. Relocation deliberately does not mark the orbit
store dirty, so panning a deep view no longer writes a store file
per gesture event.

*Continuous gestures need one more thing: the render must not wait
for the worker's acknowledgment.* Field report — a wheel zoom from 13
to 48 showed "settle 117-192 ms over 1 frame" per step, while dragging
the zoom FIELD to the same depth settled in 17 ms. One frame of chunk
work against 192 ms of wall clock is the signature of waiting, not
rendering: wheel smoothing changes the view every frame, so every
frame posts a new orbit request and reads an acknowledgment that is
always one epoch behind, and the image freezes for the whole gesture.

The stale-serve path renders the frame against the worker's LAST
publication instead. The published orbit carries its own request
(`OrbitProgress::served`), so the render thread can check the shape
itself and compose the offset: the published `ref_offset` rescaled to
this view, plus the EXACT fixed-point delta from the published anchor
to the new center (`center_delta_px` — the same arithmetic
`relocate_to` uses, so a glide accumulates no error). Same cap, same
parameter-plane-only restriction. The frame reports NOT settled, so
the acknowledged render still produces the authoritative image a
frame later. Measured on a paced 13→48 glide: 78 of 100 frames drew,
against 0 before.

Two structural guarantees, both tested. **Exports never preview**:
video export and the headless/CLI path leave `progressive` false,
which routes orbits through the blocking cache — the stale-serve path
lives in the progressive one and cannot run
(`continuous_gesture_keeps_drawing` asserts a non-progressive
renderer settles every call and never stale-serves, because a future
`progressive = true` on an export path would otherwise put a preview
frame in a finished file). **Animation playback is unaffected in
correctness**: it advances only on `!escape_dirty`, and a stale-served
frame is not settled, so no preview can be mistaken for a real
animation frame — playback merely stops freezing between them.

One diagnostics fix rode along: the settle clock now restarts on a
render-identity CHANGE rather than on any chunk restart, so the
reported settle time is the latency of the user's edit rather than
the whole gesture.

*And the chunked render must not flash black.* Field report, past
zoom 48: a black screen for the first frames of a zoom, permanently
black while panning, a single black flash when dragging the zoom
field, and "a black hole in the middle for a single frame" before
later chunks filled it — all at max_iter 605.

Two causes, both fixed. **The templates painted every pixel every
chunk**, so a pixel whose iterations had not finished yet wrote
black. It now skips the store unless it has escaped or the render has
reached max_iter, so an unfinished pixel keeps whatever the texture
held — the previous frame. The settled image is unchanged (the last
chunk writes everyone), which the test asserts against a fresh
render. **And the chunk pacer forgot its GPU measurement on every
restart**: `reset_chunk_pacing` zeroes `chunk_iters`, and the first
chunk was then bounded to 2x the cold-start seed. On the floatexp
rung that seed is ~13x smaller (PERTURB_CHUNK_BUDGET_FE), so a
gesture — which restarts every frame — never escaped it, and every
frame covered a fraction of max_iter. Side effect of fixing it: the
paced 13→48 glide went from 3463 ms to ~1800 ms.

*That fix's first form caused a device loss, and the reason is worth
keeping.* "Go straight to the measured size on restart" trusted
`gpu_ms_per_iter`, which is measured on whatever chunk last carried
timestamps — often a LATE chunk, where most pixels have escaped and
iterate for nearly free. A restart rebirths every pixel. At zoom 120
with ~100k iterations the survivor bias is enormous, so the restart's
first chunk was computed from the cheap tail, clamped only by the 1M
ceiling, and run with every pixel alive at floatexp cost:
`wgpu DEVICE LOST (Unknown)`. The measurement was honest about the
wrong population. The pacer now keeps a SECOND measurement taken only
on first chunks (`TimestampPacer::from_start` → `gpu_ms_per_iter_cold`)
— all pixels alive, the regime a restart re-enters — and a restart
sizes from that or stays seed-bounded as before; mid-render growth
keeps its 2x bound. `restart_chunks_never_trust_survivor_biased_
measurements` injects measurements and inspects the chosen chunk
directly, because a TDR cannot be a test: with the regression
restored it reads a 100,000-iteration restart chunk against an 868
seed.

*That was not the whole of it — the same view lost the device again
at 10k iterations, and the second look found two deeper holes.* A
measurement was kept across a change of COST REGIME: crossing zoom 48
into floatexp, a cheap scaled-rung cold measurement sized the
floatexp restart, and at max_iter 10k the resulting chunk covered the
whole render in ONE dispatch with every pixel alive. Measurements are
now tagged with the regime that produced them (rung, render width and
height, tier) and discarded outright on a mismatch — a measurement
from another regime is not a stale number, it is a number about
something else. And the growth CEILING was an absolute iteration
count (1M), blind to how many pixels pay it: identical at 320x240 and
at 1920x1080 with 3x supersampling, where it stands ~500x over the
budget the seed encodes. It is now a multiple of that seed
(`CHUNK_SEED_HEADROOM = 64`), which measures 499,968 against 2,048
for those two sizes. Both device losses in this area ended in a
dispatch the pixel-aware ceiling alone would have refused; the
circuit breaker still halves it, now through the seed. The test
covers all three failures — survivor bias, cross-rung reuse, and the
resolution-blind ceiling.

The test (`mid_render_frames_hold_content_instead_of_black`) forces
64-iteration chunks, settles, pans, and renders exactly ONE chunk
frame — what a drag shows. It guards against a vacuous pass (a view
that is itself nearly all black when settled cannot demonstrate
holding, and the first draft of this test picked exactly such a view
and passed while proving nothing), and reverting the shader guard
reads 100% black against a settled 30.9%.

## 8c. Coloring scale at depth, and device-loss recovery

*The scale floor was reachable in practice long before the slider
allowed it.* An iteration-scaled coloring (`smooth`, `escape_count`,
`period`, `distance_estimate`) multiplies an escape count by `scale`,
and a deep view's counts are orders of magnitude larger than a
shallow one's — so the useful values live far below the old 0.001
minimum. Three coordinated changes: the floor drops to 1e-6; a
slider whose range spans 1e5 or more decades becomes LOGARITHMIC
(otherwise the entire deep-zoom range sits in the leftmost thousandth
of the track, while every existing range keeps its linear feel); and
the one-shot suggestion button, which computed `8 / max_iter` and
then clamped it at 0.005, no longer clamps above the slider's own
minimum — at the 100k iterations a deep view needs it had been
returning a value 60x coarser than its own formula asked for.

*An automatic scale was tried twice and REMOVED — record it as a
failed experiment rather than a missing feature.* The idea was to
normalize an iteration-scaled coloring so a deep view did not show
proportionally more palette cycles. Three normalizers were tried and
each failed in its own way, which is the useful part:

- **By `max_iter`.** Wrong because it is a BUDGET: raising it does
  not change an already-escaped pixel's count. Measured, an 8x budget
  moved a shallow view's colours by 6/255 while normalizing by it
  moved them 74/255 — in the wrong direction. Its own test disproved
  it.
- **By the frame's MEAN escape count.** Correct as a depth measure,
  but it climbs as slower pixels escape in later chunks, so the
  colours climb with it: the image shifts under the viewer while it
  renders. No still image shows this; only rendering does.
- **By the frame's MINIMUM escape count.** Stable under progressive
  rendering by construction (later chunks only add larger counts —
  measured, it held at 644 across all 42 chunk frames) and monotone
  with depth where the median is not (1, 11, 22, 35, 75, 180, 647,
  960 over zooms 0..28). It still failed in use: any normalizer
  computed from frame CONTENT steps discontinuously as structure
  enters and leaves the view, so it trades the mean's slow drift for
  a sudden jump while zooming. That is inherent, not a tuning
  problem.

The conclusion from testing was that no normalization beats none:
the colour scale is already reasonably consistent across zoom and
iteration count, and where careful control is wanted, animation
tracks interpolate it deliberately and smoothly — which no automatic
rule can. So the option, its GPU reduction, the `IterationScaled`
coloring feature and the per-pixel record changes that supported it
were all removed. What remains from the episode is the part that was
actually needed: the reachable scale range above.

*Device-loss recovery existed but was never reachable.* The full GPU
rebuild — drop every renderer, `GpuContext::reinit`, recreate,
`request_full_resync` — was wired only to SURFACE errors from
`get_current_texture` (sleep/wake, display change). A lost DEVICE
arrives through wgpu's device-lost callback instead, so a TDR left
the app running against a dead device with every call failing, which
is the "never recovered" in the field report. The callback now
raises a flag the frame loop consumes (it is a plain `fn` with no
access to the app, so a static is the only place the two meet), and
the existing rebuild does the rest. `reinit` also retries device
creation across the reset window rather than panicking on the first
failure — a GPU that has just reset is briefly absent, and that path
cannot fail gracefully, since the old surface is already released.

## 8d. Making the panel approachable

Four changes, all aimed at the same report: the panel is confusing to
someone new, and much of what it offers does nothing for the fractal
in front of them.

*Render mode is a toggle, not a third kind of flame.* The View panel
is back to 2D/3D — all it ever meant, since it edits a flame — and
the Escape panel has one button: on, or off to 3D flame rendering.
No previous-mode state is kept; restoring something the user never
set is not a courtesy.

*The Escape workspace* puts the Escape panel in the left dock where
Standard puts Transforms, with Colors and History right. It is
deliberately NOT Standard with one panel swapped: Transforms, the
Triangle Editor and View all edit a flame and are inert here. Turning
escape mode on switches to it; the request rides `UiResponse` and is
applied by App, because the workspace is borrowed by the UI for the
whole frame.

*Controls that do nothing are hidden*, on two gates that are
properties of the assembled shader rather than of taste:

- `coloring_suits_formula`. A formula that never sets `escaped`
  leaves the templates' `escaped || COLORING_COLORS_INTERIOR` false
  everywhere, so an escape-time coloring over it renders BLACK. Note
  the subtlety that a first version got wrong: `NonEscaping` alone
  does not mean that, because a CONVERGENT formula has no bailout yet
  still sets `escaped` when its orbit settles — which is how
  Novaretti's shipped look shades convergence speed. The preset smoke
  test found it by declaring a shipped config impossible.
- `FormulaFeature::DynamicalOnly`. Origami, Newton, Collatz and
  Lattès ignore `c` and seed at the pixel, so both planes are the
  same image and the Julia toggle is inert. (Julia cannot be added to
  Origami without inventing a c-dependence the fold has no room for —
  that would be a different fractal, not a mode.)

Both tags are kept honest by tests that scan the WGSL rather than
trusting the flags, in both directions: a missing `DynamicalOnly`
offers an inert toggle, a wrong one hides a live control.

*The iteration controls are gated the same way*, because the same
question applies to them: `bailout` and the biomorph axis both live
INSIDE the escape test, which the assembler compiles in only for an
escaping formula, and Mann damping is spliced into the step of any
mode-A formula. A mode-B FIELD reads none of the three — it runs a
fixed-count accumulation with no escape test, no bailout and no step
to damp, so all three sat in its panel doing nothing. Rather than
trust a predicate, the test ASSEMBLES each formula's real shader and
asserts the control is offered exactly when the WGSL reads it. (One
thing that looked like a bug and is not: damping and biomorph also
take a config OFF the perturbed path, but they disable perturbation
rather than being silently ignored, so a damped deep zoom renders the
direct path's honest mush rather than wrong math — `usable_depth`
already tells the user which case they are in.)

*Presets* carry everything a formula needs to look like itself —
view, iteration budget, coloring, and both parameter sets — as one
undo step. The first preset of a formula is its default, applied on a
switch, which is what stops a centre and zoom chosen for the
Mandelbrot from following you into Origami. They are GENERATED from
the visual-regression configs (`scripts/gen_escape_presets.py`), so
each one is a view the suite already renders and hash-compares rather
than a plausible-looking invention; 46 of them across all 26
formulas, plus the three mode-B fields, whose natural view and TERM
COUNT are as particular as any formula's. A GPU smoke test renders
every preset and requires enough lit pixels to be an image and enough
distinct values to be a picture rather than a flat wash.

## 8e. Antialiasing: more of it, and how it combines

*The ceiling goes to 8x* (64 samples per display pixel). Whether a
view can actually have it is decided by the render-pixel budget in
`resize`, which reduces the factor rather than failing — and that
budget is now expressed in BYTES over the real per-pixel cost rather
than as a fixed pixel count. The pipeline had grown buffers since the
count was chosen (the recolor cache's 32 B/px of records, the
softening blur's two targets), so a fixed pixel ceiling quietly meant
a larger and larger allocation; raising the factor to 8x is what
makes that the difference between a clamp and an out-of-memory.
Measured at 160x128: aliasing (mean |second difference|) falls from
96.2 with no antialiasing to 26.1 at 8x.

*At export sizes the grid cannot deliver what is asked for, so
antialiasing also runs by ACCUMULATION.* 8x over a 4000x3000 export
is 768 megapixels of per-pixel state — and 32000 pixels a side, past
the 16384 texture-dimension limit every adapter has, so no budget
tuning reaches it. The factor was clamped away and nothing said so,
which reads as "antialiasing does nothing on export".

The same samples can be taken as several ordinary renders, each
displaced within a pixel, and averaged: identical total iteration
work, fixed memory, no size limit. No template needed changing,
because a sub-pixel shift of the sampling grid IS a shift of the
view — which the direct path already expresses through its centre
and the perturbed path through `ref_offset` (exact at any depth,
being a displacement in pixel spacings). The export path uses
whatever grid factor fits and makes up the rest this way, so 8x at
4K is a 1x grid times 8x accumulated: 64 renders, ~70 seconds,
against 0.7 for one. Measured on a detailed view at 4000x3000,
aliasing falls 3.75x.

Two things the test caught that review would not have. Adding the
offset silently did NOTHING at first: the recolor cache keys on the
config, the offset lives on the renderer, so every sample returned
the same cached image — the offset is now part of the iteration
identity. And the accumulation target cannot be one read-write
texture (`rgba32float` read-write storage is rejected outright) nor a
storage buffer (a 4K frame of `vec4<f32>` exceeds the 128 MB binding
limit); it is a write-only ping-pong pair. The test pins that one
accumulated sample at offset zero reproduces a plain render EXACTLY,
which is what separated "the arithmetic is wrong" from "the positions
differ", and that the accumulated image lands far closer to the grid
it replaces (3.08/255) than no antialiasing does (36.93/255). Not
bit-equality: the direct path carries its centre to the shader as
f32, so a sub-pixel shift lands a few ten-thousandths of a pixel off,
and at a chaotic boundary that flips pixels.

*An out-of-memory used to be served as a black image.* A 4000x3000
export at 8x produced an all-black PNG, and the crash log named it
exactly: `wgpu error: Out of Memory`, then `Buffer with 'Escape Iter
State' label is invalid` on every dispatch after. wgpu reports a
rejected allocation through the uncaptured-error handler, which stops
NOTHING — the buffer comes back invalid, each dispatch against it
quietly does nothing, and the export reports success over an empty
image. Three changes, because the failure had three causes:

- The export ran on the app's own device while the VIEWPORT's escape
  renderer was still holding its own gigabytes of per-pixel state at
  8x. The app now frees it before a synchronous export; the frame
  loop rebuilds it lazily and it has nothing to show meanwhile.
- The per-renderer budget was 3 GiB, which is too much when a device
  can carry two of these at once plus the flame renderer and the UI.
  Halved.
- The failure is now DETECTED rather than rendered past: an
  `OutOfMemory` error scope around the whole escape render (the
  accumulation passes included) turns it into a `RenderError`, and a
  precheck refuses sizes the device's DECLARED limits cannot hold
  before allocating anything at all. The precheck shares its
  supersample calculation with `resize` — checking the REQUESTED
  factor instead refused renders the renderer would have clamped and
  then made up by accumulation, which is a bug this doc records
  because the first version of it did exactly that.

*Downsample modes*, because "antialiasing washes colour out of fine
detail" is a fair report about a correct behaviour. A saturated
filament covering one sample in nine IS one ninth of that pixel's
light, and a linear average says so. Three combines:

- `Box`, the default and unchanged: a plain average in linear light,
  what a sensor does.
- `Perceptual`: average in gamma space, so mixed hues land between
  each other rather than at their linear sum. THIS IS THE ONE THAT
  ANSWERS THE REPORT — measured on a tight-banded view it retains
  10% more mean saturation than Box. It costs the other half of
  correctness: thin bright detail reads darker than it physically
  should.
- `Vivid`: keep the linear average's luminance, then restore the
  chroma the samples had. Worth knowing that the FIRST design of this
  — weighting each sample by its own saturation — did almost nothing
  (+1.2%), because what dilutes fine colour is mixing HUES rather
  than mixing colour with grey, and nine similarly-saturated samples
  produce nearly equal weights. Restoring the chroma directly gets
  +2.2%. Still the smaller effect of the two.

The test pins the property that separates them rather than any
pixel: Vivid must retain more saturation than Box, and all three must
agree to within 5% of pixels on WHERE the structure is, since they
differ in how samples combine and not in what was sampled. It also
needed a view with tight palette bands to say anything at all — on a
broad-banded image the three are nearly indistinguishable, which is
itself worth knowing.

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
