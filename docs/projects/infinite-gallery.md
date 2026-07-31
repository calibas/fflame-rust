# Infinite Gallery: standalone WASM renderer + script evaluator

**Status:** Delivered (all four phases; browser-verified). The
finished gallery will live in a separate repo; what this repo delivers
is the two production-grade WASM modules it will be built on, plus a
deliberately minimal proof-of-concept page. **The copyable usage guide
for consumers is [`wasm/README.md`](../../wasm/README.md)** — module
APIs, the envelope format, seed semantics, and testing commands live
there; this doc keeps the vision and the decision record.

---

## 1. The vision (context for the modules' design)

An "Infinite Gallery" web page: an endless scroll of randomly
generated fractals, all rendered on the fly in the visitor's browser.
Nothing is stored server-side — a position in the gallery IS a random
seed, so the whole state fits in a URL.

The spatial metaphor:

- **Wings** — 2D, 3D, Animation. Broad categories.
- **Hallways** — one per random *generator* script. Walking down a
  hallway is incrementing the seed: image *n* of the Grand Julia
  hallway is the Grand Julia generator run with seed *n*.
- **Rooms** — *modifier* scripts. Doors appear along a hallway; going
  through a room applies that modifier to everything in the hallway
  from then on. Go through the "Blue room" (a palette modifier
  favoring blue) and later the "Jitter room", and the hallway is now
  random Grand Julias with random blue palettes and random jitter.
  Going backwards through a room removes it.

A position is fully described by: generator, ordered list of applied
modifiers (with their door-crossing parameters), and the current seed.
That tuple serializes into a shareable URL:

```
#/2d/basic_random/rooms=iq_palette:hue=0.6,jitter:amount=0.3/seed=142
```

**Seed semantics (decided):** every stage of the pipeline — the
generator and each applied modifier — runs with the same hallway seed
*n*. This is the convention `run_script` already established (the
random-flame + random-palette pairing shares one seed), it keeps the
URL to a single seed, and it makes image *n* a pure function of
(generator, rooms, n). Rooms are *not* independently re-rollable; their
fixed choices are parameters in the URL, not seeds.

**Tile spec (decided):** 512×512 at 30M total iterations
(~115 samples/pixel) for the PoC. Both are user-editable settings in
the full version.

## 2. What this repo builds

```
wasm/render/          fflame-render.wasm  — FractalConfig JSON + dims → RGBA pixels
wasm/script/          fflame-script.wasm  — script + seed + params (+ base config) → FractalConfig JSON
web/gallery/          static PoC page (plain HTML/JS, no framework, no polish)
```

The two modules are the product; they must be solid. The page is a
demo harness and explicitly *not* the real gallery.

### Why two modules

The script evaluator is pure CPU (rhai + the scene model) and tiny;
the renderer carries wgpu and the shader corpus. Separating them keeps
the script module fast to instantiate, lets it run in a worker with no
GPU access, and — most importantly — forces the interface between them
to be the serialized `FractalConfig`. That contract is already the
`.fflame` format with skip-if-default stability, which is exactly what
a separate-repo future needs.

### The crate pattern

Both wasm crates copy the `python/` precedent exactly: standalone
crates with their own `[workspace]` (deliberately NOT members of the
main build), a path dependency on `fractal_flame_wgpu`, their own
lockfiles, gitignored artifacts. The app build is unaffected.

### The one main-crate change: the `web-app` feature

wasm-bindgen collects `#[wasm_bindgen]` exports from the **entire
dependency graph**. The main crate has `#[wasm_bindgen(start)]
wasm_main()` (boots the full app) and the `wasm_api` exports — any
thin wasm crate depending on the main crate would silently inherit the
whole application, egui and all.

Fix: a `web-app` cargo feature, **default on** (so every existing
build — desktop, wasm-pack app, pyfflame — is unchanged), gating the
wasm-bindgen export surface: `wasm_main`, `wasm_api`, and the web
clipboard/text-agent glue if it exports. The thin crates depend with
`default-features = false`.

Dependencies stay unconditional; unused code (egui, winit, audio,
i18n) is removed by wasm-ld's GC plus wasm-opt. That claim is
**verified by measurement** (`twiggy top` on the built module), not
assumed. If GC provably fails to drop something big, escalation is
optional dependencies behind the feature — a bigger refactor we do
only if the numbers demand it.

## 3. Module APIs

### `fflame-script.wasm`

```js
list_scripts()                          // embedded assets/scripts library: id, name, kind, description
collect_params(source)                  // param declarations → JSON (two-phase host, cached per source hash)
run(source, seed, params_json)          // generator: starts from FractalConfig::default()
run_on(source, seed, params_json, base_config_json)   // modifier: rooms
```

Room composition is JS-side folding — the module stays primitive:

```js
let config = run(generator_src, n, gen_params);
for (const room of rooms) {
    config = run_on(room.source, n, room.params, config);
}
```

Additions that emerged during the build: the run envelope carries
`animation_json` (a script-defined animation — the turntable modifier
was silently dropping its output; this is the Animation wing's door),
and listings carry `flags` (`norng` — the script ignores the seed, so
a gallery must not build a seed-varying hallway on it; `palette` — it
belongs in a palette picker).

Determinism is inherited: scripts draw from the pinned
`rand_pcg::Pcg64Mcg`, so script + seed ⇒ byte-identical JSON on
desktop, web, and Python. This is already tested cross-platform; the
new module gets a cross-check against the desktop `generate` CLI.

### `fflame-render.wasm`

```js
await probe()                           // fail early if WebGPU is unavailable
await render(config_json, w, h, iterations?) → { pixels, width, height, iterations, ms }
```

- Raw RGBA out, straight to `canvas.putImageData`. No `png`/`image`
  crates in the module — "save this image" is `canvas.toBlob` in JS.
- Internals: the existing unified headless API
  (`renderer::render::render(device, queue, RenderJob, progress)`),
  unchanged.
- **Device lifecycle (decided during build): created per render,
  `destroy()`ed after** — the pattern the app's own WASM export uses,
  because WebGPU defers dropped-buffer reclamation to the JS GC; a
  long tile scroll would otherwise accumulate GPU memory until renders
  fail black. Costs an adapter/device request and one shader compile
  per tile — milliseconds against a multi-hundred-ms render. Full
  version's optimization door: a persistent FlameRenderer + shader
  cache with explicit buffer destruction (consecutive seeds of one
  generator usually share a variation set, so the compile would
  amortize across the hallway).
- Deliberately excluded: UI, audio, i18n, `.flame` XML import,
  storage, PNG metadata, animation. The Animation wing needs a
  many-frame API — the door is left open, the PoC does not build it.

## 4. The PoC page

Minimal by instruction. One HTML file plus a small JS module:

- Infinite scroll via `IntersectionObserver`; tiles render as they
  approach the viewport, sequential seeds.
- Hallway/room UI: a generator picker, an "apply modifier" picker with
  its param sliders (from `collect_params`), an ordered applied-rooms
  list with remove.
- URL hash carries the full position (format above); load restores it.
- Click a tile → re-render larger. Nothing more.

## 5. Size budget

Target: **≤ ~2MB brotli** for the renderer module; the script module
should land well under that.

**Measured (release + wasm-opt -Oz, gzip -9; brotli lands lower):**

| module | raw | gzip |
| --- | --- | --- |
| fflame-render | 3.19 MB | 0.74 MB |
| fflame-script | 3.97 MB | 1.01 MB |

Both under budget with no lever pulled. String scans confirm zero
egui/winit/audio in either module — the `web-app` gate plus linker GC
did the job. The dominant shared payload is the 1292 variation WGSL
statics, as predicted (dead weight in the script module; the
`wgsl_body!` lever below remains unpulled).

Measured inputs: inline variation WGSL is 1.7MB of string data (the
dominant payload, and text — brotli takes roughly 4–5× off it);
`shaders/` templates are 520KB; wgpu's `webgpu` backend on wasm is
thin bindings (no wgpu-core, no naga on that path).

Levers, in order, only if measurement says we're over:

1. `wasm-opt -Oz`, `lto = "fat"`, `panic = "abort"`, `opt-level = "z"`
   (the `dist` profile already exists).
2. The script module carries the 1.7MB WGSL statics as dead weight
   (registry validation needs names/params, not shader bodies): a
   `wgsl_body!` macro compiling to `""` under a feature. Invasive
   across 100+ def files — a lever, not part of the PoC.
3. Curated variation subset — last resort; costs script compatibility.

## 6. Phases

**Phase 1 — `web-app` feature.** Gate the wasm export surface.
*Exit:* desktop build and full wasm app build behave identically with
defaults; a stub thin crate on wasm32 with `default-features = false`
compiles and contains no app exports.

**Phase 2 — `wasm/script`.** The four calls above; embedded script
library included.
*Exit:* for a sweep of seeds and scripts, module output is
byte-identical JSON to the desktop `generate` CLI (generator and
modifier paths both).

**Phase 3 — `wasm/render`.** init + render; a bare test page.
*Exit:* renders known configs correctly in-browser; size measured
against budget with `twiggy`/brotli numbers recorded here.

**Phase 4 — gallery PoC.** The page in §4 wiring both modules.
*Exit:* scroll a hallway, apply/remove rooms, share a URL, reload to
the same images.

## 7. Constraints and non-goals

- **WebGPU required, no fallback.** Compute shaders rule out WebGL —
  same constraint as the existing web build (Chrome/Edge 113+,
  Firefox 121+; Safari experimental).
- GPU renders are not bit-reproducible across backends/platforms.
  Config JSON is byte-exact; pixels are verified visually/by tolerance.
- The renderer runs one flame at a time on one device; the gallery
  queues tiles. Parallel devices/workers are a full-version concern.
- No server component anywhere in the PoC.

## 8. Decision record

- Two modules, JSON contract — over one combined module (forces the
  interface, script module stays worker-friendly).
- Same seed *n* for all pipeline stages — over derived per-stage seeds
  (matches `run_script` precedent; one seed in the URL).
- Raw RGBA out of the renderer — over PNG bytes (drops image/png deps;
  canvas does encoding for free).
- Rooms folded in JS — over a pipeline call in the module (keeps the
  module API primitive; the gallery owns the composition semantics).
- `web-app` default-on feature + linker GC — over optional-dependency
  refactor (smallest change that can work; escalate only on measured
  failure).
- 512×512 @ 30M iterations PoC tiles — user-editable in the full
  version.
