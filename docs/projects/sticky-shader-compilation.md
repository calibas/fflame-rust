# Sticky shader compilation

Avoid recompiling the flame shader when the flame changes — for the
gallery's random walks, in-app batch generation, thumbnails, and the
editor's own variation add/remove churn. This is the working plan; it is
updated as phases land, and the measurements section carries real
numbers, not estimates.

## The problem, measured

The renderer generates WGSL specialized to each flame's variation set,
and the `ShaderCache` holds **exactly one** current pipeline — any change
of variation set, baked constant, or render mode rebuilds and replaces
it. Even alternating A→B→A recompiles every time.

Per-render fixed cost in the gallery = WGSL generation (fast, Rust) +
`createShaderModule` + `createComputePipeline` in the browser (Tint +
driver, tens to hundreds of ms) + the init pipeline. Since the gallery
began reusing its device and renderer, shader compilation is the *only*
remaining per-seed fixed cost — which is why it became visible. In-app
batches and thumbnails funnel through the same `ensure_shaders_current`
path with the same behavior.

Random flames defeat any naive cache: `basic_random` picks a different
variation subset per seed, so consecutive seeds are cache misses by
construction.

## Design

Two composing layers plus an optional third. They compose because they
attack different axes of the cache key: the sticky superset collapses
the huge axis (which variations are compiled in), and the pipeline LRU
absorbs the residual small-cardinality axes that still fork the shader
(see "What still forks the shader" below).

### Layer A — pipeline LRU cache *(landed)*

`ShaderCache` keeps an LRU of 8 compiled pipeline sets. The key is the
**generated WGSL itself** (main + init source), not a structured key:
codegen is a pure function of (flame, flags, constants), so identical
text is an identical pipeline, and every change detector — including
ones added later — is subsumed automatically. A u64 hash prefilters;
candidates must match the full sources, so a collision degrades to a
miss rather than binding the wrong pipeline.

The init shader must be part of the key: two flames with identical main
WGSL can bake different (xform, variation) init pairs when an
init-bearing variation sits on a different transform, and serving the
wrong init pipeline zeroes that transform's derived params (the
collapse-to-line class). A test constructs exactly this pair and asserts
they fork the key.

Measured honestly: **the LRU shows zero hits in random-batch benches**,
by design — batch seeds never revisit a key, and repeats of the current
flame take the pre-existing early-out. Layer A serves *revisit*
patterns: undo/redo across a variation change, A/B toggling, returning
to the startup flame. Tests cover revisit-hit, the init-placement fork,
and eviction at the cap. Batches are Layer B's job.

### Layer B — sticky variation superset (the core)

The renderer keeps a `sticky` set of variation names — **renderer
state, never serialized into a config**. The compiled shader's active
set is `flame's variations ∪ sticky`. Variations present in the shader
but absent from the flame are dead at runtime: the dispatcher already
gates every variation on `if (w != 0.0)`, and the probe demonstrates
this at scale — its batch shaders carry 99 unrelated variations per
flame, compile fine, and gate correctly.

Convergence does the work: a random hall picks from a fixed pool (the
default generator pool is 15 variations), so after a handful of seeds
the sticky set *is* the pool and recompiles stop. No declaration API is
required — which is also why the same mechanism serves the in-app batch
case and the editor, where no declaration step exists.

**No clear triggers — eviction is the clearing mechanism.** When
`|flame ∪ sticky|` exceeds the cap, evict the least-recently-rendered
sticky entries not in the current flame. Loading a preset whose
variations are already in the superset costs nothing; one that brings
new variations costs one recompile with the union, and the old entries
age out through LRU. A stale sticky set is dead weight bounded by the
cap, not a correctness hazard, so there is nothing an explicit clear
would fix that eviction does not.

**The map is the whole implementation.** Every consumer — shader
builder, `Transform.variations[idx]` packing, `get_param` offsets, the
init shader, per-thread state layout — already derives from one local
index map. Today each derives it internally from the flame; the refactor
makes the map an explicit parameter computed once by the renderer. That
single choke point is why the probe's superset flames work unmodified.

**Map ordering (the correctness kernel).** The dispatcher emits each
phase in map order, and pre/post variations *chain* — each transforms
the previous output — so reordering two same-phase chained variations on
one transform is a **different image**, not float noise. Rule:

    map = [flame's variations, in flame order] ++ [sticky extras, canonical order]

Live variations keep their relative order, so output is **bit-identical**
to the specialized shader (dead variations are branch-skipped, never
summed — `+0.0` never happens, the term simply doesn't exist).
Verified by test with `deterministic_rng` + byte-compare.

The cost of flame-order-first maps: same union, different add order ⇒
different map ⇒ different pipeline. Mitigation where it matters: the
random generator emits each transform's variations in canonical
(registry) order — it is our generator, one sort, and then every
generated flame with the same union shares one map by construction.
Arbitrary imported flames still benefit from stickiness, with at most
one order-variant pipeline per distinct ordering, which Layer A absorbs.

**Monotone flags.** `HAS_DC` / `HAS_RGB` become properties of the
superset — once a color-writing variation enters, the flag stays on
until its variation is evicted. Behavior-neutral for flames that don't
use it (nothing writes `vc`, the blend reduces to identity); verified by
byte-compare.

**Hard ceilings** are `MAX_VARIATIONS_PER_FLAME = 100` and 1600 param
slots, checked against `flame ∪ sticky` the same way the probe's batch
packer checks them. The soft cap comes from the dead-cost measurement
below.

### Layer C — gallery pre-warm (optional, last)

`warm(configs[])` on `fflame-render` seeding the sticky set, so tile 1
pays one compile for the whole hall instead of the first few tiles
paying convergence. Only optimizes cold start; the sticky set converges
without it.

## What still forks the shader under a superset

Baked into the WGSL and therefore part of any cache key: `NUM_TRANSFORMS`
(loop unrolling; the generator emits 2–5), `COLOR_MODE`, RENDER_3D,
xaos, path tracking, post-affine, attachments, post-symmetry, analytic
blur, solid, `attachment_cap`, plot-emit cap, preserve-z. The superset
cannot collapse these; their joint cardinality in a random-batch context
is small (~4 transform counts × occasional flags), which is Layer A's
job.

## Scoping — where sticky is NOT wanted

CLI export and the visual regression suite keep the specialized +
inlined-constants path: they want dead-code elimination, compile once
per image, and their reproducibility contract is against the specialized
shader. The 148 baselines are untouched by construction. The probe and
census keep their own harnesses. Sticky serves the interactive app,
batches, thumbnails, and the gallery.

## Decisions (made)

- **Default-on in the app.** It also removes the recompile when a user
  toggles a variation off and back on while designing.
- **Cap**: 32, from the dead-cost curve (free to ~15 dead, −8% at 30,
  −40% at 60). Configurable; the measurement is in Phase 0 below.
- **No clear triggers** — LRU eviction under the cap self-manages (see
  above).
- This document is the working plan and lives in the repo.

## Phase 0 measurements

Measured 2026-08-08, GTX 1660 SUPER / Vulkan, 512², 20 seeds x 10M
iterations, `cargo run --release --bin shader_bench`:

| configuration | total | median/seed | rebuilds | compile share |
|---|---|---|---|---|
| baseline, natural flames | 1632 ms | 84.5 ms | 20 | **91%** |
| baseline, pinned transform count | 1345 ms | 89.2 ms | 20 | 78% |
| superset proxy (pinned + pool at w=0) | 467 ms | **12.0 ms** | **2** | 24% |

The diagnosis is confirmed brutally: **91% of a random batch's wall time
is shader compilation.** The superset proxy — Layer B emulated with
weight-0 augmentation and zero new machinery — cuts the median per-seed
time **7x** and collapses 20 rebuilds to 2 (renderer creation + the
first superset build; every later seed is a hit). The natural-vs-pinned
delta shows `NUM_TRANSFORMS` churn alone is worth ~450 ms of compile
across 20 seeds, which is Layer A's residual to absorb.

**Dead-variation cost curve** (50M iterations, second render of each,
4-transform flame with 4 live variations; dead entries drawn from
small-parameter registry variations):

| dead compiled in | throughput | compile |
|---|---|---|
| 0 | 1951 Miter/s | 11 ms |
| 15 | 2100 Miter/s (noise; free) | 116 ms |
| 30 | 1798 Miter/s (−8%) | 142 ms |
| 60 | 1175 Miter/s (**−40%**) | 279 ms |
| 95 | 740 Miter/s (**−62%**) | 425 ms |

**Dead variations are NOT free at scale.** The cost is flat to ~15,
mild at 30, and steep past that — consistent with the dispatcher being
inlined at its three call sites, so code size hits instruction cache and
occupancy even when every dead branch is skipped. Two consequences:

- **The cap defaults to 32** (compiled set = flame ∪ sticky). That holds
  the converged default pool (15) with room, keeps worst-case throughput
  cost under ~10%, and stays far out of the cliff. In range of the
  expected 15–100, at the bottom of it, and the measurement is the
  reason.
- **Eviction is a throughput guardrail, not just hygiene.** A sticky set
  allowed to grow unbounded would silently halve render speed.

Curve caveat: the dead entries are small-bodied variations; heavyweight
ones inflate code size faster per entry, so real-world cost per dead
variation is likely somewhat higher — an argument for the conservative
cap, revisitable with a finer curve.

**Warm-cache note.** The table above is a cold NVIDIA driver cache — the
honest cold-start story. A warm re-run (after Layer A landed) shows the
same structure at lower absolutes: baseline 426 ms total, 21.9 ms/seed,
compile share 54%; superset proxy 225 ms, 10.3 ms/seed, 2 rebuilds.
Even fully warm, half of a random batch is still compilation, and the
browser's shader cache behavior is its own (Tint + driver, different
eviction), so the cold numbers are the right planning basis for the
gallery.

## Order of work

1. **Phase 0**: shader-cache rebuild counters + timing; the benchmark;
   the three measurements above. *(done — results above)*
2. **Layer A**: pipeline LRU. *(done — keyed by generated WGSL,
   init source included; revisit/init-fork/eviction under test)*
3. Generator canonical-order emission (tiny, independent).
4. **Layer B**: the map-parameter refactor + sticky policy + tests
   (byte-identity, convergence, caps).
5. Flip default-on in-app once identity + throughput evidence is in.
6. **Layer C** if the gallery wants faster cold start.

## Verification

- Byte-identity: same config, same `deterministic_rng`, specialized vs
  sticky ⇒ identical pixels. Promised only for the preserved-order case,
  which is every generated flame and every flame without multi-pre/post
  transforms.
- Convergence: a 20-seed run asserts compile count stops growing.
- Throughput: the dead-cost curve; `run_benchmarks.py --quick` unchanged
  (export path untouched).
- Visual suite 148/148 (it renders through the export path).
