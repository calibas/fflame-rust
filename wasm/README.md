# fflame WASM modules — usage guide

Two standalone WebAssembly modules extracted from the fractal-flame
renderer, built to power the Endless Gallery (and any other web
embedding). This guide is self-contained on purpose — copy it into
whatever repo consumes the modules.

| module | does | size (raw / gzip) | needs |
| --- | --- | --- | --- |
| `fflame-script` | script + seed + params → FractalConfig JSON | 3.98 MB / 1.01 MB | nothing (pure CPU; worker-safe) |
| `fflame-render` | FractalConfig JSON + dims → RGBA pixels | 3.19 MB / 0.74 MB | WebGPU |

**Browser support:** WebGPU is required for rendering — Chrome/Edge
113+, Firefox 121+; Safari behind a flag. There is no WebGL fallback
(the renderer is compute-shader based). The script module runs
anywhere wasm runs, including Node.

## Building

```bash
# from the source repo
cd wasm/script && wasm-pack build --target web --release
cd wasm/render && wasm-pack build --target web --release
```

Each produces a `pkg/` directory (ES module + `.wasm`) that is the
whole deliverable — copy or publish it as-is. The `Cargo.toml`s
already carry the wasm-opt flags the bundled binaryen needs
(`--enable-bulk-memory`, `--enable-nontrapping-float-to-int`).

Serve over http(s) — wasm won't instantiate from `file://`. Serve the
`.wasm` files with `Content-Encoding` support if you can; both modules
are text-heavy (embedded WGSL) and compress ~4×.

## The contract between the modules

A **FractalConfig JSON string** — the same format as the desktop app's
`.fflame` files, with skip-if-default field stability. The script
module emits it, the renderer consumes it, and any `.fflame` file from
the desktop app works too. Treat it as an opaque pass-through: don't
re-serialize it (key order and float formatting are part of the
byte-level determinism guarantee).

## `fflame-script`

```js
import init, * as script from "./pkg/fflame_script.js";
await init();   // in Node: await init({ module_or_path: bytesOfTheWasmFile })
```

### Calls

```js
script.list_scripts()  // → JSON string
// [ { id, name, kind: "generator"|"modifier", summary,
//     flags: { norng, palette }, params: [ <decl>... ] }, ... ]
//
// flags.norng    — the script ignores the seed: every seed produces the
//                  same output, so don't build a seed-varying UI on it.
// flags.palette  — the script generates a palette (belongs in a palette
//                  picker as well as the modifier list).

script.script_source(id)          // → the script's source text, by id
script.collect_params(source)     // → {name, kind, flags, params} for arbitrary source

script.run(source, seed, paramsJson)               // generator: starts from the default config
script.run_on(source, seed, paramsJson, baseJson)  // modifier: applied to baseJson
// both → envelope JSON:
// { name, kind, warnings: [..], messages: [..],
//   config_json: "<FractalConfig JSON — pass through untouched>",
//   animation_json: "<.anim JSON>" | null }
```

- **`seed` is a `u64`, passed as a JS `BigInt`** (`run(src, 42n, "{}")`).
  See "Seeds" below.
- `paramsJson` is a `{key: value}` object string. Values by declared
  type: `float`/`int` → number, `bool` → boolean, `text` → string,
  `choice` → option name (case-insensitive) or index, `color` →
  `"#rrggbb"` or `[r, g, b]` (0–1 floats). Unknown keys are an
  **error**, not a warning. `"{}"` runs the declared defaults.
- `messages` are the script's `print()` output; `warnings` are
  non-fatal problems. All errors reject/throw with a message that
  includes the script's line number where one applies.
- `animation_json` is non-null when the script defined an animation
  (e.g. the turntable modifier): the same standalone `.anim` JSON the
  desktop CLI writes, carrying the flame as its `base_config`.

### Param declarations (`params` entries)

```
{type:"float"|"int",  key, label, default, min, max}
{type:"bool",         key, label, default}
{type:"choice",       key, label, options: [..], default: index}
{type:"text",         key, label, default, max_len}
{type:"color",        key, label, default: [r, g, b]}
```

### Composing a pipeline (the gallery's hallway + rooms)

Fold modifiers over a generator, every stage with the same seed `n` —
image `n` is then a pure function of (generator, rooms, n):

```js
let env = JSON.parse(script.run(genSource, n, "{}"));
for (const room of rooms) {
  env = JSON.parse(script.run_on(room.source, n, JSON.stringify(room.params), env.config_json));
}
// env.config_json → renderer
```

### Determinism guarantee

Script + seed + params ⇒ **byte-identical** `config_json` everywhere —
browser wasm, Node wasm, desktop CLI, Python bindings. The script RNG
is a pinned-algorithm PCG64-MCG with a fixed seed-expansion function;
neither can shift under a dependency bump. Enforced by
`tests/cli_parity.rs` (native, against committed CLI fixtures) and
`check_node_parity.mjs` (the built wasm artifact under Node).

### Seeds — a ring of 2⁶⁴

**Seeds are a circle, not a line.** The step after `u64::MAX` is `0`,
and the step before `0` is `u64::MAX`. A gallery is a loop, so walking
past either end continues rather than stopping or erroring.

```
… 18446744073709551614 → 18446744073709551615 → 0 → 1 → 2 …
                                      (wraps here)
```

- Full range **u64**: `0` … `18_446_744_073_709_551_615` (2⁶⁴−1).
  Nothing special happens at 2³² or 2⁵³ — those are language limits,
  not ring boundaries.
- **`-1` is the last position** (`2⁶⁴−1`), i.e. one step back from the
  start. Note it is *not* `2⁶³−1` — that value is `i64::MAX` and sits
  exactly halfway round the ring, an ordinary interior position.
- Any integer names a position: values are reduced modulo 2⁶⁴, so
  `2⁶⁴ ≡ 0`, `2⁶⁴+1 ≡ 1`, `-2 ≡ 2⁶⁴−2`.
- Distinct positions are unrelated random streams; consecutive seeds
  are deliberately scrambled far apart (SplitMix64 expansion), so
  "reroll = seed + 1" gives an unrelated flame and a walk never drifts
  through near-identical images.

Every door applies the same reduction, so a position means the same
thing wherever it is opened — verified by test at each one:

| door | how a seed is given | reduction |
| --- | --- | --- |
| JS / wasm | `BigInt` — `run(src, -1n, "{}")` | wasm-bindgen, mod 2⁶⁴ |
| Python | any `int` — `run_script(src, seed=-1)` | `x & (2**64 - 1)` |
| CLI | `--seed -1` | `i128 as u64` |
| Rust | `u64` | already on the ring |

**JS caveat — this is the one that bites.** `Number` is exact only to
2⁵³−1 (`9_007_199_254_740_991`), which is 1/2048th of the ring, and it
*rounds* rather than wrapping. Keep seeds as `BigInt` end to end:

```js
const onRing = (v) => BigInt.asUintN(64, v);   // the same mod 2^64
let seed = onRing(BigInt(urlValue));           // never parseInt/Number
seed = onRing(seed + 1n);                      // walks past the top to 0
```

Also avoid comparing seeds to decide how far to walk — `next < seed + n`
inverts once the value wraps. Count positions instead.

## `fflame-render`

```js
import init, * as render from "./pkg/fflame_render.js";
await init();
await render.probe();   // rejects with a clear message if WebGPU is unavailable
```

### Render

```js
const t = await render.render(configJson, width, height, iterations);
// t.pixels     Uint8Array, RGBA8, width*height*4 bytes
// t.width, t.height
// t.iterations actually run (number)
// t.ms         render wall time

ctx.putImageData(new ImageData(new Uint8ClampedArray(t.pixels), t.width, t.height), 0, 0);
// PNG, if wanted: canvas.toBlob(...) — the module ships no image codecs.
```

- `iterations` caps the total chaos-game budget; omit to use the
  config's own `max_iterations`. Rule of thumb: quality scales with
  iterations **per pixel** — 30M at 512×512 (~115/px) is a good tile,
  and a 2× larger edge wants ~4× the iterations for the same look.
- **Inputs are treated as hostile**, because a config and a URL are both
  shareable artifacts:
  - dimensions are checked against the device's
    `max_texture_dimension_2d` (commonly 8192) and rejected with an
    error — they are not passed through to fail deeper;
  - the iteration budget is clamped to 8e9, from *both* the argument and
    the config's own `max_iterations` (an unclamped 5e11 ran for 90+
    seconds — a frozen tab);
  - wgpu validation errors are captured rather than left to panic. This
    matters in wasm: a panic poisons the module, so one bad request
    would otherwise kill the renderer until the page reloads.
  Renders still fail by returning `Err`; the module should never abort.
- **Serialize renders** (one at a time). The gallery uses a one-deep
  promise queue; concurrent calls would contend for the GPU and win
  nothing.
- GPU renders are *not* bit-reproducible across machines/backends —
  the config JSON is the deterministic artifact, pixels are not.
- Device lifecycle is internal: a device is created per render and
  destroyed after, so long sessions don't accumulate GPU memory
  (WebGPU defers dropped-buffer reclamation to the JS GC). The cost is
  an adapter request plus one shader compile per call — milliseconds
  against a typical multi-hundred-ms render. If you profile a future
  need, the optimization is a persistent renderer + shader cache in
  the module, not in the caller.
- 3D configs (`render_mode: "3d"`) render exactly like the desktop
  app; nothing extra to do. Animation is **not** in this module: it
  renders stills. An Animation-wing consumer renders frames by
  applying the `.anim` track data to configs itself (API to come).

## Minimal end-to-end example

```html
<canvas id="c" width="512" height="512"></canvas>
<script type="module">
  import initScript, * as script from "./pkg-script/fflame_script.js";
  import initRender, * as render from "./pkg-render/fflame_render.js";

  await Promise.all([initScript(), initRender()]);
  await render.probe();

  const src = script.script_source("basic_random");
  const env = JSON.parse(script.run(src, 7n, "{}"));
  const t = await render.render(env.config_json, 512, 512, 30_000_000);

  document.getElementById("c").getContext("2d")
    .putImageData(new ImageData(new Uint8ClampedArray(t.pixels), 512, 512), 0, 0);
</script>
```

## Testing the modules

```bash
cd wasm/script && cargo test          # CLI-parity: byte-identical fixtures
cd wasm/render && cargo test          # GPU smoke test (skips without an adapter)
node wasm/script/check_node_parity.mjs  # the BUILT wasm artifact vs the same fixtures
```
