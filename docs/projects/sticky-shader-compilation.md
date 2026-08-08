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

### Layer A — pipeline LRU cache

`ShaderCache` keeps a small LRU (~8 entries) of compiled pipelines keyed
by exactly the things `ensure_current` already compares: the local index
map, `ShaderConstants`, render mode, path/xaos flags, and the
specialization key. Transparent, bit-identical by definition, and it
already fixes the editor cases (undo/redo across a variation change,
A/B comparisons) on its own.

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
- **Cap**: set by measurement; expected in 15–100.
- **No clear triggers** — LRU eviction under the cap self-manages (see
  above).
- This document is the working plan and lives in the repo.

## Phase 0 measurements

*(pending — filled in by the Phase 0 commit)*

- 20-seed random batch, per-seed time and compile count, today's path.
- Same 20 seeds with every flame augmented to the pool union at weight 0
  and a fixed transform count — the superset proxy, measuring the payoff
  before any Layer B code exists (weight-0 variations enter the compiled
  set; the runtime gate keeps them dead).
- Dead-variation cost curve: one flame's sustained throughput with
  N ∈ {0, 15, 30, 60, 99} dead variations compiled in. Decides the cap.

## Order of work

1. **Phase 0**: shader-cache rebuild counters + timing; the benchmark;
   the three measurements above. *(this phase)*
2. **Layer A**: pipeline LRU.
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
