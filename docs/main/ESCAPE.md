# Escape-Time Rendering

**Overview:** The second render family (`RenderMode::Escape`). Each
pixel is computed on its own — a formula iterated from that pixel, a
series summed at it, or a walk measuring its distance to an attractor —
and coloured once. There is no chaos game and no accumulation: one
settled render is the whole picture. Mandelbrot and its relatives live
here, with deep zoom by perturbation to effectively unlimited depth.

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) — overall system design
- [RENDERER.md](RENDERER.md) — the flame pipeline whose tail this shares
- [TONE-MAPPING.md](TONE-MAPPING.md) — the Linear mapping escape output needs
- [COLOR.md](COLOR.md), [PALETTE_LIBRARY.md](PALETTE_LIBRARY.md) — the palette every colouring samples
- [TERRAIN.md](TERRAIN.md) — an escape picture as a 3D terrain; the Lit and path-traced tiers
- [CAMERA.md](CAMERA.md) — the solid camera, the gestures, fly mode
- [EFFECTS.md](EFFECTS.md), [ANIMATION.md](ANIMATION.md), [EXPORT.md](EXPORT.md), [ONLINE.md](ONLINE.md)
- [SCRIPTING.md](SCRIPTING.md) — the `escape` script object
- [SIMULATION.md](SIMULATION.md) — the third generator, built on the same registry pattern
- [FOCUSED-RENDERING.md](FOCUSED-RENDERING.md) — deep zoom for *flames*, a different feature
- Design record: [escape-time-fractals.md](../archive/escape-time/escape-time-fractals.md)
  (archived; §1-3 and §5-8 are the structure, the dated sections the measurements)
- Open items: [escape-sim-followups.md](../projects/escape-sim-followups.md),
  *The escape engine: not built*

**Code locations:**
- [src/escape/mod.rs](../../src/escape/mod.rs) — `FormulaDef`, `ColoringDef`, the
  `FORMULAS` and `COLORINGS` registries, the pairing rules
- [src/escape/formulas.rs](../../src/escape/formulas.rs) — 26 formulas, inline WGSL
- [src/escape/colorings.rs](../../src/escape/colorings.rs) — 28 colourings
- [src/escape/fields.rs](../../src/escape/fields.rs) — mode B: 3 fields, 3 colourings
- [src/escape/ifs.rs](../../src/escape/ifs.rs) — mode D: 3 distance functions, 5 colourings
- [src/escape/assembler.rs](../../src/escape/assembler.rs) — WGSL assembly, every template
- [src/escape/renderer.rs](../../src/escape/renderer.rs) — `EscapeRenderer`
- [src/escape/lens.rs](../../src/escape/lens.rs) — the camera lens
- `fixedpoint.rs`, `bigfloat.rs`, `reference.rs`, `bla.rs`, `nucleus.rs`,
  `orbit_store.rs` in `src/escape/` — deep zoom
- [src/escape/presets.rs](../../src/escape/presets.rs) — generated presets
- [src/config/escape.rs](../../src/config/escape.rs) — `EscapeConfig`
- [src/ui/escape_panel.rs](../../src/ui/escape_panel.rs) — the Escape Fractal panel

---

## What an escape picture is here

**A pixel is a function of the config.** The planar modes sample nothing
at random, so a render is fixed by the config and the output size, on a
given device. A picture is never "more converged": `max_iter` is a
ceiling on each pixel's orbit, and a settled render is final. (The path
tracer, for solids and terrains, is the exception: it accumulates
samples.) This is what the renderer's chunking is held to: however a
render is split into dispatches, the settled image is the same.

**The view is a point and a depth.** The centre is a pair of exact
decimal strings and the zoom a base-2 exponent (`zoom_log2`). An f32
pixel mapping stops separating neighbouring pixels somewhere between
zoom 6 and 14 -- sooner for a taller render, a higher supersample, or a
Julia plane; past that the engine iterates small deltas against one
full-precision reference orbit (perturbation), and the centre's digits
are what the reference is computed from. See [Deep zoom](#deep-zoom).

**The output is shaped like a flame's.** The engine writes an
`Rgba32Float` image in the flame accumulator's layout — `rgb` linear
colour, `a` coverage — and hands it to the flame renderer's tail:
density effects, tone mapping, colour effects, readback. Escape mode gets
palettes, the effect chain, transparent export and PNG metadata without a
line of its own.

---

## Fragment modes

The design record names three per-pixel shapes; a fourth was added for
the flame as a distance field. One config field, `EscapeConfig::formula`,
names an entry in any of the formula registries, and which registry
resolves the name decides the mode.

| mode | a pixel computes | registries | status |
| --- | --- | --- | --- |
| A — iterate & classify | an orbit of `z ← f(z, c)`, ended by escape, convergence or `max_iter`, coloured from its summary | `FORMULAS`, `COLORINGS` | built |
| B — field | a finite sum or a finite-orbit statistic; no escape, no classification | `fields::FIELDS`, `fields::FIELD_COLORINGS` | built |
| C — escape-time IFS | the Hepting–Hart escape buffer: an image-space fixed point over ping-pong textures | — | **planned, not built**; design record §6 and [escape-time-ifs.md](../archive/escape-time/escape-time-ifs.md) |
| D — distance | the distance from the pixel to a set, by inverse iteration (the loaded flame) or a distance estimator (a solid) | `ifs::IFS_DEFS`, `ifs::IFS_COLORINGS` | built |

**Routing.** `EscapeRenderer::ensure_pipeline` asks `ifs::get_ifs` first,
then `fields::get_field`, and otherwise takes mode A. Mode A's lookup
never fails: `get_formula` renders `mandelbrot` with a warning for a name
it does not know, and `get_coloring` the first colouring (`escape_count`),
so a file from a newer build still opens. Modes B and D resolve a
colouring name their registry lacks to the def's `default_coloring` —
which is also the state right after switching formula families, when
`coloring` still names the previous family's entry.
`render::unknown_names` lists the names no registry knows, for the CLI.

Every escape shader binds group 0; mode D adds group 1 (the flame's
maps), and a lens adds group 2. A shader without either is
byte-identical to one built before they existed.

---

## The registries

Append-only registries of `static` definitions with inline WGSL, in the
same spirit as the variation registry: a definition is registered by
appending it, and registration order is UI order. A pipeline holds
exactly one formula and one colouring, so the WGSL entry points have
fixed names (`formula_step`, `coloring_map`, `coloring_accum`) instead of
prefixed ones; helper functions must be name-prefixed, since a formula
and a colouring share one module.

### `FormulaDef`

| field | means |
| --- | --- |
| `wgsl` | defines `fn formula_step(z: vec2<f32>, c: vec2<f32>) -> vec2<f32>`; features add arguments (below). The template supplies complex helpers such as `esc_cmul` and `esc_cpow` |
| `wgsl_param_seed` | WGSL expression for `z₀` on the parameter plane; empty is the origin. Formulas whose critical point is elsewhere seed there (Lambda at 1/2, Lambda Sine at π/2) and maps with no `c` seed `pixel` |
| `wgsl_prev_init` | the history register's initial value (`NeedsPrevZ`); empty is zero |
| `escape_metric` | what the escape test compares with `bailout` (`EscapeMetric`, below) |
| `escape_degree` | the degree at infinity, from the resolved params, for a formula that is polynomial there. The smooth count takes its logarithm in it (`escape_degree_of`, 2 otherwise), and only such a formula is offered a colouring's recommended bailout |
| `derived_data` | a CPU function of the resolved params whose output (at most 256 floats) is uploaded to the uniform's `fdata`, read as `fdata4(i)`. Origami's fold lines live here |
| `wgsl_derivative` | `fn formula_derivative(z, c, dz, is_julia)`, the derivative orbit, for distance estimation and analytic relief. Empty for 14 of the 26 formulas |
| `parameters` | `EscapeParamDef`s (below) |
| `presets` | named starting points; the first is the formula's default (see [Presets](#presets)) |

**`FormulaFeature`** — what the assembler compiles into the loop:

| flag | effect |
| --- | --- |
| `NonEscaping` | no escape test: the orbit runs `max_iter` steps and only orbit statistics show anything (Kaliset, Ducks, Origami, Lattès, Novaretti) |
| `NeedsPrevZ` | `formula_step` gains `z_prev`, and the loop carries the previous iterate (Phoenix, Manowar) |
| `NeedsIndex` | `formula_step` gains the iteration index `i: u32`, for a rule that changes per step (Origami) |
| `MutatesC` | `c` becomes a `var` passed by pointer (Spider) |
| `Convergent` | the loop also stops on `|z − z_prev|² < 1e-12` and records `converged` (Newton, Nova, Magnet, Novaretti) |
| `DynamicalOnly` | the map ignores `c` and seeds at the pixel, so both planes are one picture and the Julia toggle is hidden (Origami, Newton, Collatz, Lattès) |
| `Creases` | the map folds paper, and its WGSL also defines `fn formula_creases(p0) -> vec3<f32>`: the folded paper's height at a starting point and its gradient, from the settings in `params.crease`. Offers the Creases relief field and terrain source (Origami) |

**`EscapeMetric`**: `NormSq` (`|z|² > bailout`, the default), `Re`
(`Re z > bailout`, raw — the exponential family and tetration) and
`AbsIm` (`|Im z| > bailout`, raw — the trig family and Collatz). The raw
metrics want a bailout near 50, not 4. The biomorph switch replaces the
metric with `(Re z)²` or `(Im z)²`.

### `ColoringDef`

A colouring defines `fn coloring_map(sum: OrbitSummary, state:
vec4<f32>) -> f32`, returning a palette position. `OrbitSummary` carries
the final `z`, the count `n`, `escaped`, `converged`, the detected
`period`, and `dz`, the derivative per *render pixel*. The template wraps
the value with `fract`, so an unbounded value cycles the palette. Also:
`accum_init` and `wgsl_accum` (the per-step accumulator, with
`NeedsOrbitAccum`), `recommended_bailout` (applied when the colouring is
picked, through `suggested_bailout`), and `pick_params` (values a fresh
pick takes where they differ from the defaults, which describe what an
old file without the key meant).

**`ColoringFeature`:**

| flag | effect |
| --- | --- |
| `NeedsOrbitAccum` | the loop carries a `vec4` accumulator updated by `coloring_accum(z, z_prev, c, state)`. Only these colourings store the wide (40-byte) record; the rest store 32 |
| `NeedsDerivative` | reads `sum.dz`; compiled in only when the formula has `wgsl_derivative`, otherwise the colouring degrades to a flat value |
| `NeedsPeriod` | Brent-style cycle detection (power-of-two checkpoints) compiled into the loop |
| `Bounded` | the value is a level in 0..1 and is clamped instead of wrapped, so 1.0 does not wrap to the darkest colour |
| `ColorsInterior` | draws never-escaped pixels too; without it they are left uncovered and the background shows |
| `DirectColor` | also defines `coloring_color(sum, state, v) -> vec3<f32>`, drawing a colour rather than a palette position |
| `PaletteInLoop` | samples the palette inside the loop, so a palette edit re-iterates rather than recolours |
| `TextureInLoop` | samples the config's texture inside the loop (`esc_texture`, group 0 binding 14) |
| `DirectAlpha` | also defines `coloring_alpha`, which becomes the pixel's coverage |
| `SkipsEscapingIterate` | the accumulator runs after the escape and convergence tests, so it never sees the escaping iterate |

### Parameters

`EscapeParamDef` is `name`, `display_name`, `default`, `min`, `max`,
`tooltip` and `choices`. Every parameter is an `f32`; a non-empty
`choices` list makes it a dropdown over the integer values `0..n`, and
`discrete_params_are_labelled_across_their_whole_range` holds `max` to
the list. Slot *i* is `parameters[i]`, packed by `pack_params` with
defaults for absent keys, and read in WGSL as `fparam(i)` / `cparam(i)`.
There are 64 slots each for the formula and the colouring
(`PARAM_VEC4S` = 16 vec4s, which UF's public formulas need) and 16 for
the texture layer (`LAYER_PARAM_VEC4S`). The config keeps values in `BTreeMap`s
keyed by name, and keeps every formula's values when another is active,
so switching back restores them. `param_default` answers a missing key's
default from whichever registry owns it, which is what undo reads.

### Pairing rules

These are properties of the assembled shader, not of taste, so they live
beside the registry and the panel reads them.

- **`coloring_suits_formula`** refuses two dead pairings: an escape-time
  colouring over a formula that never sets `escaped` (non-escaping *and*
  non-convergent — a convergent formula does set it, which is how
  Novaretti shades convergence speed), which renders black; and a
  derivative colouring over a formula with no derivative.
- **`iteration_controls`** says which of bailout, biomorph and damping
  the shader reads: the first two live in the escape test, so a
  non-escaping formula reads neither; a mode-B field reads none of the
  three (`FIELD_ITERATION_CONTROLS`). Tests assemble every formula and
  check the answer against the WGSL.
- **`formula_julia_is_meaningful`** is false for `DynamicalOnly`
  formulas, and the Julia toggle is hidden.
- **`suggested_bailout`**: when a colouring is picked, its
  `recommended_bailout` is applied — but only for a formula with a
  polynomial degree, the `NormSq` metric and biomorph off. The smooth
  count's error falls with the escape radius, which is why a new config
  starts at a bailout of 1e4; a file without the key means 4
  (`LEGACY_BAILOUT`), the default it was written under.
- **`layer_fits` / `layer_refusal`**: a texture layer can go over a base
  colouring unless both accumulate (`BothAccumulate`: there is one
  accumulator) or either draws its own colour (`DirectColor`: the layer
  blends palette positions). `layer_of` is the layer a config actually
  draws — `None` for a refused pair.

### Julia mode

A toggle, not a list of formulas: `julia` on makes the pixel `z₀` with
`c` fixed at `julia_re`, `julia_im`. Toggling off and on keeps the seed.

### The formulas

| name | display name | step and notes |
| --- | --- | --- |
| `mandelbrot` | Mandelbrot | `z² + c`, the two-multiply special form. Derivative; perturbs, with BLA |
| `multibrot` | Multibrot | `zᵖ + c`, any real `p`. Derivative; perturbs at integer `p` 2–12 |
| `tricorn` | Tricorn | `z̄ᵖ + c` (Mandelbar at `p` = 2). Anti-holomorphic derivative; perturbs at integer `p` |
| `burning_ship` | Burning Ship | the quadratic step with abs folds, six variants: Burning Ship, Perpendicular Mandelbrot, Perpendicular Ship, Celtic, Buffalo, Perpendicular Celtic. Perturbs |
| `mcmullen` | McMullen | `zⁿ + c/zᵐ`, Sierpiński-carpet Julia sets; seeds the pixel. Derivative; perturbs in Julia mode |
| `kaliset` | Kaliset | `|z|/⟨z,z⟩ − c` (or `+ c`). `NonEscaping` |
| `phoenix` | Phoenix | `z² + c + p·z_prev`. `NeedsPrevZ`; perturbs |
| `lambda` | Lambda (Logistic) | `λ·z(1 − z)`, seeded at 1/2. Derivative; perturbs |
| `spider` | Spider | Fractint: `z ← z² + c; c ← c/2 + z`. `MutatesC`; derivative |
| `manowar` | Manowar | Fractint: `z² + z_prev + c`, seeded `z = m = pixel`. `NeedsPrevZ`; perturbs |
| `barnsley` | Barnsley | Fractint M1–M3, conditional affine / quadratic maps. Derivative |
| `cactus` | Cactus | `z³ + (c − 1)z − c`. Derivative |
| `exponential` | Exponential | `eᶻ + c`, Cantor bouquets. Escape on `Re z`; derivative |
| `trig` | Sine / Cosine | `sin z + c` or `cos z + c`. Escape on `|Im z|`; derivative |
| `ducks` | Ducks (Kali-log) | `log(fold(z) + c)`, five variants. `NonEscaping`; perturbs variants 0 and 4 |
| `tetration` | Tetration | `w ← cʷ`, seeded at `c`. Escape on `Re w`; derivative |
| `collatz` | Collatz | `¼(2 + 7z − (2 + 5z)·cos πz)`. Escape on `|Im z|`; `DynamicalOnly` |
| `feather` | Feather | `zᵖ / (1 + (Re²z − i·Im²z)) + c`. Perturbs at integer `p` 2–8 |
| `newton` | Newton | root-finder: six functions × Newton, Halley, Chebyshev, complex relaxation. `Convergent`, `DynamicalOnly`; perturbs the polynomial functions |
| `nova` | Nova | the Newton step plus `c`, seeded at 1. `Convergent`; perturbs |
| `magnet` | Magnet | Magnet I and II; escape and convergence to 1 both end the orbit. `Convergent`; perturbs |
| `novaretti` | Novaretti | `−6z(z³ + c)/(2z³ − c)²`; nothing escapes, so classification is convergence. `NonEscaping`, `Convergent` |
| `littlewood` | Littlewood | the root cloud: the greedy sign-choice map `w ← λw + d` stays bounded, over three digit sets |
| `lambda_sine` | Lambda Sine (λ·sin z) | `λ·sin z`, the Cantor-bouquet family, seeded at π/2. Escape on `|Im z|`; derivative |
| `origami` | Origami (Butterfly) | McCabe's Butterfly Origami: one fold per iteration along a seeded line sequence. `NonEscaping`, `NeedsIndex`, `DynamicalOnly`, `Creases`; uses `derived_data` |
| `lattes` | Lattès | rational maps whose Julia set is the whole sphere, three variants. `NonEscaping`, `DynamicalOnly` |

Each def's doc comment cites its source and the convention it pins.

### The colourings

| name | display name | what it shows |
| --- | --- | --- |
| `escape_count` | Escape Count | the discrete count times `scale`: bands |
| `smooth` | Smooth Iteration | the fractional escape count, in the formula's degree. Recommends bailout 1e4 |
| `orbit_trap` | Orbit Trap | the orbit's least distance to a trap shape (including a golden log spiral) |
| `orbit_average` | Orbit Average | the running mean of a distance-to-axes trap: the Kaliset glow |
| `stripe_average` | Stripe Average | the mean of `0.5 + 0.5·sin(density·arg z)` |
| `root_basin` | Root Basin | which root a convergent orbit reached, shaded by speed |
| `triangle_inequality` | Triangle Inequality | the triangle-inequality average |
| `period` | Period | the detected cycle length of a settled orbit |
| `distance_estimate` | Distance Estimate | `|z|·ln|z|/|dz|`, through `−log2` so equal steps are equal depths |
| `magnitude_average` | Magnitude Average | the mean `|z|`: the Ducks look |
| `normal_map` | Normal Map (3D relief) | analytic normal shading from the derivative. `Bounded` |
| `position_average` | Position Average | the orbit's mean position |
| `position_map` | Position Map | the final position looked up in a procedural source, mixed with the branch address. `Bounded` |
| `sphere_average` | Sphere Average | the mean chordal distance to a point of the Riemann sphere |
| `curvature_average` | Curvature Average | the mean turn between consecutive steps |
| `velocity` | Velocity | the mean step length |
| `threads` | Threads | thin filaments where the orbit grazes the axes |
| `exponential_smoothing` | Exponential Smoothing | `Σ exp(−|z|)` diverging, `Σ exp(−1/|z − z_prev|)` converging |
| `decomposition` | Decomposition | the angle of `z` at escape, optionally in sectors (2 is binary decomposition) |
| `basic` | Basic | Ultra Fractal's Basic: the real, imaginary or summed part of the escaped `z` |
| `gaussian_integer` | Gaussian Integer | the distance to the nearest Gaussian integer, reduced over the orbit |
| `external_rays` | External Rays | lines where `arg z` at escape lies near a whole number of turns. `Bounded` |
| `rainbow_fringe` | Rainbow Fringe | Fraktaler 3's: black on white with a rainbow at the boundary. `DirectColor` |
| `infinite_waves` | Infinite Waves | Kalles Fraktaler's sinusoids of the count, driving hue, saturation and value. `DirectColor` |
| `itinerary` | Itinerary | the orbit's angular address over equal sectors |
| `voc_traps` | Visions of Chaos Traps | Softology's circle, cross, ring, square and stalk traps. `DirectColor` |
| `direct_traps` | Direct Orbit Traps | every iterate inside a trap paints a palette colour. `DirectColor`, `PaletteInLoop` |
| `image_trap` | Image Trap | Ultra Fractal's: the texture sampled where the orbit lands, its alpha the coverage. `TextureInLoop`, `DirectAlpha` |

The survey behind the newer colourings, and what is still a candidate,
is [escape-coloring-survey.md](../projects/escape-coloring-survey.md).

---

## The formula language

Formulas and colourings can also be written as text; `escape::lang`
compiles them to WGSL. The language's own form is **Rhai** (the
scripting language the program embeds; since phase 4), and **Ultra
Fractal's formula language** -- which accepts Fractint's shape too --
is read as well. The plan, its phases and what is left are in
[escape-formula-language.md](../projects/escape-formula-language.md).

**Where it lives.** `src/escape/lang/`: `rhai.rs` (a Rhai formula
lowered to the parse tree), `to_rhai.rs` (a checked UF or Fractint
formula printed as Rhai), `lexer.rs`, `parser.rs` (a UF file into
entries and sections), `check.rs` (names and types, UF's rules,
into the typed `ir.rs`), `interp.rs` (the program in f64: the reference
the WGSL is tested against), `wgsl.rs` (the shader), `derive.rs` (a
formula's derivative), `switch.rs` (a `switch:` pair as one program),
`builtins.rs` (the function table), `bigeval.rs` (the program in big
floats: the reference orbit), `periodic.rs` (its nucleus search).
`src/escape/custom.rs` registers a compiled entry.

**A Rhai formula** (`lang/rhai.rs`, a `.rhai` file, one formula; the
dialect `rhai`) is three functions and its parameters' metadata:

```rhai
const PARAMS = #{
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout" },
};
const INFO = #{ title: "Mandelbrot", center: complex(-0.5, 0.0), maxiter: 500 };

fn start(pixel, p, ctx) {
    if ctx.julia { #{ z: pixel, c: ctx.c } } else { #{ z: complex(0.0, 0.0), c: pixel } }
}

fn step(s, p) {
    s.z = s.z * s.z + s.c;
    s
}

fn escaped(s, p) {
    s.z.norm_sqr() > p.bailout
}
```

- **Explicit state**: `start` returns the map each pixel begins with
  (its fields the per-pixel state, `z` the iterate), `step` returns it
  one pass on -- the same variable, or a new map, every value of which
  reads the state before any is set -- and `escaped` says when the pixel
  has left. What a step carries from pass to pass is then visible.
  Without `escaped` a formula never escapes (UF's `bailout: true`): it
  runs `maxiter` passes, is `FormulaFeature::NonEscaping` (the
  colourings' gate as for Kaliset's), and the Bailout control hides
  where it has no `bailout` parameter (`perturb::never_escapes`).
- **Settling** (`perturb::settle_of`): a test whose "done" includes the
  iterate's distance from a variable under a bound -- `(s.z -
  s.zold).norm_sqr() < 1e-12`, UF's `|z - zold| > @tol` to iterate on --
  ends the orbit as converged where that part ends it, as the built-ins'
  settle test does, so Root Basin's general key reads it
  (`formula_settled`, `formula_perturb_settled`), and the program is
  `FormulaFeature::Convergent`.
- **Parameters** in `PARAMS`: a value alone, or `value` with `min`,
  `max`, `caption`, `hint`, `choices` (an int's names), `exponential`
  (the slider runs logarithmically, `custom::is_exponential`), or
  `function` (a user function, called `p.f(z)`). They are the panel's
  controls, animation targets and script parameters
  (`escape.formula_params()`). A complex parameter is two, `name.re` and
  `name.im`, and also reads and sets as a point in polar form,
  `name.r` and `name.angle` (degrees; `EscapeConfig::formula_param`):
  the panel shows both, and animating `name.angle` moves the point
  around the origin.
- **The context**, every function's optional last argument (`ctx`
  here): `ctx.julia` and `ctx.c` (the panel's Julia toggle and
  constant), `ctx.pass` (0 on the first pass; with `INFO.cycle`,
  `#{ prefix, period }`, deep zoom keeps a reference per phase: the
  hybrids), `ctx.maxiter`, `ctx.center`, `ctx.magn`, `ctx.angle` (the
  view's rotation, degrees), `ctx.x`, `ctx.y`, `ctx.width`,
  `ctx.height` (the pixel on the screen), `ctx.random`, `ctx.pixel`.
  `INFO` gives `title`, `center`, `magn`, `maxiter`, `angle`.
- **`prepare(p, ctx)`**, optional, runs once per image (UF's
  `global:`): the map it returns is read as `p.name`.
- **`fn creases(p0, p, ctx)`**, optional: a folding formula's paper,
  its height and gradient at the starting point `p0`, ending with `#{
  height: h, slope: complex(gx, gy) }`. It reads its own variables, the
  parameters, `prepare`'s values and the panel's crease settings,
  `ctx.crease_decay`, `ctx.crease_pattern` (0 valley, 1 alternate, 2
  random), `ctx.crease_sign` (the flip, -1 or 1) and `ctx.crease_reach`
  (in paper sizes: 0 for the 2D relief, a terrain's levels off). A
  formula with it offers the Creases relief field and terrain source
  (`FormulaFeature::Creases`; the GPU's `formula_creases`). Origami's
  twin is the example.
- **`to_f32(x)`** (UF `f32`): the value rounded to f32, the GPU's
  precision. On the GPU the identity; in `prepare`, which runs on the CPU
  in f64, it lets a table be computed as f32 code computes it, an
  operation at a time.
- **Arrays**, of one dimension: a literal `[a, b, c]`, `array(n, v)`
  (n a constant int, or one the settings fix: `array(p.n + 1, 0.0)`,
  `array(ctx.maxiter, 0.0)`, as UF's sizes below; each element `v`),
  `a[i]` read and written (an int index, clamped), an element's part or
  method (`a[i].re`, `a[i].norm_sqr()`), `a.len()`. A state field may be one (`s.hist[i] =
  s.z`), and `prepare` may return one (`p.k[i]`, a table the image's);
  `let b = a` copies one. Arrays of arrays are refused (index one
  array, `a[i * n + j]`). As UF's arrays (below) on every path.
- **UF's own perturbation equations**, optional: `perturb_start(d, p)`
  and `perturb_step(d, p)` set `d.dz` (the pixel's deviation) from
  `d.z` (the reference's value) and `d.dpixel`, and return `d`;
  `perturb_when(p)` says when they hold. Without them, deep zoom
  derives the equations itself (phase 3).
- Helpers (other functions) are inlined and read only their arguments.
  `if` is a value as well as a statement in `start`, `step` and
  helpers; `escaped` is one test, its `let`s substituted.
- Values are int, float, bool and complex (`complex(re, im)`); a bool
  and a number do not mix but by `to_float` / `to_int` (1 for true),
  and two ints divided is refused (Rhai would truncate). `**` is the
  power. Ints are 32-bit, as UF's: `+ - *` wrap, and a literal past
  `i32::MAX` (`0x846ca68b`) is its 32-bit pattern. Rhai's bit
  operations take ints as such patterns -- `^` xor, `&`, `|`, `<<`,
  `>>` logical (a shift by its amount's low five bits) -- so an
  unsigned hash runs bit for bit as in Rust (Origami's does: the
  plan's item 3, step 3c); on bools `&` and `|` are and and or, `^`
  xor. Rhai's names where they differ from UF's:
  `ln` the natural log and `log` base 10, `abs` of a complex its
  modulus and `fold` each part's absolute value (the Burning Ship),
  `norm_sqr` the squared modulus, `.re` / `.im`, `arg`, `atan2(y, x)`,
  `ceiling`, `cot`, `coth`, `PI()`, `E()`, `mix(a, b, t)` (`a + (b -
  a) t`: a morph between two expressions that deep zoom derives through). Loops: `while`, `do ...
  while/until`, `for i in a..b`; no `break`.
- How: rhai's own parser (optimisation off) gives its syntax tree
  through the `internals` feature, pinned with the rhai version; the
  tree is lowered to the UF parse tree -- the state's fields and the
  locals as variables, `p.name` as `@name`, `start`/`step`/`escaped` as
  `init:`/`loop:`/`bailout:` -- which the checker types (`check_rhai`:
  Rhai's meanings for `abs` and bools). So everything after the parse
  serves both languages unchanged; messages carry the script's names
  and positions.
- Examples: `assets/formulas/mandelbrot.rhai`, `burning-ship.rhai`,
  `phoenix.rhai` (the editor's New starts a Rhai formula). Tests:
  `tests/formulas/rhai/` (every construct, the function names, the
  plane, a cycle), run on the GPU as the interpreter does; a Rhai
  formula runs as its UF twin, renders as its built-in, and deep-zooms
  pixel for pixel as its UF twin (`rhai_formulas_render_and_deep_zoom_as_their_twins`).
- **A Rhai colouring** has `finish` where a formula has `escaped`
  (`rhai::kind_of` tells them apart). It runs beside the formula, each
  function taking the formula's `z`:

  ```rhai
  const PARAMS = #{ point: #{ value: complex(0.0, 0.0) }, scale: 2.0 };
  const INFO = #{ title: "Point Trap" };   // region: "inside" / "outside"

  fn start(z, p) { #{ closest: (z - p.point).abs() } }
  fn step(s, z, p) {
      let d = (z - p.point).abs();
      if d < s.closest { s.closest = d; }
      s
  }
  fn finish(s, z, p) { s.closest.sqrt() * p.scale }
  ```

  - `start` (the state the pixel keeps) and `step` (after each of the
    formula's passes) are optional, as UF's `init:` and `loop:` are.
  - `finish` returns the palette position (the index; it wraps), or
    `#{ index, solid }`, `solid` leaving the pixel to the solid colour.
    It may end in an `if` whose arms give either.
  - The context adds `ctx.numiter` (the passes run, in `finish`); a
    colouring counts its own passes in its state.
  - The state is packed under the same 32-float cap as UF's.
  - Examples: `assets/formulas/smooth.rhai`, `orbit-trap.rhai`; the
    test file `tests/formulas/rhai/coloring.rhai` runs on the GPU as the
    interpreter does.
- **A Rhai transformation** has `fn variation(z, p)`: a point to a
  point, the third entry kind (`EntryKind::Transform`, UF's
  `transform:` section, the point `#pixel` read and assigned). It is
  not an escape formula: it compiles to a **flame variation**
  ([VARIATIONS.md](VARIATIONS.md#variations-written-in-rhai)) and, in
  Escape mode, is offered as the camera lens. `INFO.name` is its
  registry name; `PARAMS` as a formula's; no `prepare`. The flame's
  hooks (the KIFS plan, C2), each the `Feature` a built-in declares:
  - `rand()`, a draw in [0, 1) (`NeedsRng`, the flame's stream);
  - `fn variation(z, p, ctx)`: `ctx.accum`, the weighted sum of the
    transform's earlier variations (JWildfire's `pVarTP`;
    `NeedsAccum`), and `ctx.weight`, its own weight;
  - state per thread: `fn init(p)` returns the map each thread starts
    with (its parameters may seed it), and the body reads and sets its
    fields as `this.name` -- Rhai's own way for a function called on an
    object to change it (`state_count` slots, `init` as
    `wgsl_state_init`);
  - a colour: the body may end with `#{ z: point, color: index }`, the
    palette index the transform's `direct_color` blends in
    (`WritesColor`).

  A **3D variation** (B2) has `fn variation3(v, p[, ctx])`, its point a
  vector, ending with a vector or `#{ z: v, color }`; `ctx.accum` is a
  vector there. It is the 3D body (the 2D one is `fn variation`, or
  without one the variation is `only_3d`) and keeps its z
  (`always_z`, as the built-in 3D variations); a 2D-only variation's 3D
  body passes `p.z` through. The program reads and writes the third
  part as `Predef::PixelZ` beside `#pixel`.

  Examples: `assets/formulas/variation-whorl.rhai`, and
  `variation-scatter.rhai` for `rand()` and a colour; the hooks' twins
  of `blur` and `macmillan`, and the 3D twins of `spherical3D`,
  `hemisphere` and `zcone`, are in `variations::custom`'s tests.
- **`rand()`** anywhere else -- a formula, a colouring, a variation
  `vary` inlines there -- draws a stream of the pixel's own, seeded by
  its place on the screen (`uf_draw`; the interpreter draws the same
  numbers), so a still reproduces. An entry that draws has no deep
  zoom: the reference point and the pixel would draw apart. UF's
  `#random` (`ctx.random`) is still one value a pixel.
- **`vary("name", z)`** or `vary("name", z, #{ param: value })`, in
  any function of a formula, colouring or transformation: the user's
  variation `name` applied to `z`, its body inlined as a helper's is
  (so deep zoom and the derivative derive through it), its parameters
  the map's values -- in the caller's terms, `#{ k: p.twist }` -- or
  their defaults; its `ctx` is a formula's identity (`accum` zero,
  `weight` one), a result map gives its point, and a variation with
  state is refused. The source is looked up in what the entry carries
  (`CustomSource::variations`), then the registered custom variations,
  then the formula library and its examples
  (`variations::custom::source_of`). Using an entry fills its
  `variations` with the sources it called (`with_called_variations`,
  flat: a variation a variation calls too), so the config carries them,
  and `name_of` hashes them, so a changed variation is a changed entry.
  A built-in variation is refused (C3, maybe). Example:
  `assets/formulas/whorled-mandelbrot.rhai`.
- **A Rhai solid** (the KIFS plan, B3; `EntryKind::Solid`, a file with
  `fn distance`): `fn start(pos, p)` returns the state from the point
  being measured -- `z`, the iterate, a vector, and `dr`, the running
  derivative, read by the colourings -- `fn step(s, p)` is one
  iteration, `fn escaped(s, p)` the stop (without it every iteration
  runs), `fn distance(s, p)` the estimate after the last one; `INFO`
  has `title` and `radius`. It renders in mode D ("Use as solid"; see
  *Mode D*). The position is `#pixel` and `#pixelz`, the estimate
  `#index` in `final:`; no `prepare` or function parameters yet.
- **Vectors** (`rhai/vec.rs`; the KIFS plan, B1): `vec3(x, y, z)` (or
  `vec3(s)`), its parts `v.x`, `v.y`, `v.z` read and set, `+`, `-`, `*`
  and `/` (by a number, or part by part), `dot`, `cross`, `length`,
  `normalize`, `abs`, `floor`, `min`, `max`, `clamp` -- as functions or
  methods; a vector in a state map or as a helper's argument; a
  `PARAMS` entry `vec3(...)` as three float parameters, `name_x`,
  `name_y`, `name_z`. The lowering splits each vector into three
  scalars, so nothing past it knows vectors exist, and deep zoom derives
  through vector code as through any other. `min`, `max` and `clamp` of
  numbers come with them (a temporary and an `if`; not in `escaped`).
  Refused: a helper ending with a vector, an `if` used as a vector
  value.
- **`fn degree(p)`**, optional: the formula's degree at infinity, which
  the smooth colourings divide by, as a built-in's `escape_degree`
  (`Program::degree`; `custom::escape_degree` serves it to
  `polynomial_degree`); it also offers a smooth colouring's recommended
  bailout. Without it, 2.
- **The built-ins' twins** (`escape::twins`,
  `assets/formulas/twins/*.rhai`): Mandelbrot, Multibrot, Tricorn, the
  Burning Ship family, Lambda, Cactus, (step 5, carrying a value
  besides z) Phoenix, Manowar and Spider, Littlewood with its three
  digit sets, the families escaping on `Re z` or `|Im z|` --
  Exponential, Sine / Cosine, Lambda Sine, Collatz, Tetration --
  Feather and McMullen, Barnsley, Kaliset, Ducks and Lattès, and the
  convergent Magnet, Nova, Novaretti and Newton (every function and
  scheme), and Origami -- every built-in formula -- each a Rhai formula
  that renders as its built-in.
  - Each keeps its built-in's parameters by name, reads the panel's
    Bailout through its `bailout` parameter and the Julia toggle
    through `ctx`, and declares its degree.
  - The Escape panel's **Edit as Formula** puts the twin in the
    built-in's place, one undo step: the parameters carry over by name,
    and the view and the colouring stay. It opens the twin in the
    Formula Editor in use, so with Live on an edit re-renders.
  - Through the renderer, with the smooth colouring, each matches its
    built-in on every pixel, on both planes and every Ship variant.
    The exceptions are Multibrot and the multicorns (under 0.4%),
    where the built-in raises to a power by polar form and the twin
    multiplies.
  - At depth every twin deep-zooms by derived equations, a Multibrot or
    multicorn power by its difference form. Against the big-float truth
    they count as well as the built-ins' native tiers: 401 and 401 of
    439 pixels for the Mandelbrot at zoom 42, 354 and 354 for Lambda at
    30, 189 and 190 for the Tricorn at 28, 346 against 294 for the Ship
    at 60, and 396 against 399 for the Multibrot at power 4, zoom 30.
    Phoenix (zoom 22.4 and 48) counts right on all 439, as the built-in
    does; Manowar (zoom 26) on 420 against the built-in's 373; and a
    convergent Newton written in the language on all 439 at zoom 30
    (`builtin_twins_render_and_deep_zoom_as_their_builtins`).
  - Mann damping and the biomorph test reach them as rewrites of the
    program (below, step 9e): the twins render as their built-ins with
    either on, and deep-zoom with them, which the built-ins cannot.
  - **Littlewood's** greedy digit -- whichever of the set leaves `λw + d`
    smallest -- is a branch on the iterate, which the derived equations
    follow, so its twin deep-zooms where the built-in has no perturbed
    tier: 439 of 439 sampled pixels right at zoom 30 for the sets
    `{±1}` and `{0, ±1}`, on both rungs (`the_littlewood_twin_deep_zooms`).
    The map expands by `|λ|` every step, so a count is only defined to
    the depth the sampling resolves: a pixel an f32 offset from its
    reference sits `2^-24` of that offset off the exact centre, and past
    about `ln(2^24 / offset) / ln|λ|` steps (some 100 to 200 at zoom 30)
    the two sample a field that is noise there -- any renderer's
    picture, past that, is one sample of it.
  - **The families escaping on `Re z` or `|Im z|`** (the order of work,
    item 2, batch 1) write their test as such (`s.z.re > p.bailout`,
    `s.z.im.abs() > p.bailout`), and the built-in's clamp of `exp`'s
    real part at 80 where it is reached. They render as their built-ins
    on every pixel (Tetration on 99.8%: its log rounds otherwise, and the
    map is chaotic), and deep-zoom, which the built-ins cannot: at zoom
    30 against big floats, Exponential, Collatz and Tetration 439 of 439
    on both rungs, Sine and Lambda Sine 430 to 433, where the maps'
    chaos takes the high counts (`the_twins_without_a_native_tier_deep_zoom`).
    A program whose loop takes `exp`, `expm1`, `sinh` or `cosh` of a
    value that changes reaches `e^50` before it escapes, past the scaled
    rung's f32, and perturbs on the floatexp rung at every depth
    (`perturb::grows_past_f32`, `PerturbTier::Custom::deep`; the
    Exponential's twin on the scaled rung: 410 of 439). Tetration's
    deep views must be chosen where it is defined at all: where an orbit
    reaches `Im(z ln c)` near 1e34 and falls back, sin and cos of that
    decide, differently on every machine, and big floats too.
  - **Feather and McMullen** (item 2, batch 2): powers by parameters
    and a pole, the built-in's guard at the pole written out. At depth
    they count as the native tiers do (Feather 439 and 439 of 439 at the
    suite's view, zoom 30; McMullen on the Julia plane 431 against 437).
    Feather's home view is chaotic in f32: its twin's counts differ from
    the built-in's on 5.5% of pixels, and each one's from f64's as much
    (328 and 313 of 6,144) -- rounding decides there, not the formula.
  - **Barnsley, Kaliset, Ducks and Lattès** (item 2, batch 3): branches
    on the iterate and folds; Kaliset, Ducks and Lattès never escape
    (no `escaped`), so they draw by the colourings that read the orbit.
    They render as their built-ins on every pixel (Ducks' variant 0 on
    98.4%: its map is chaotic over the 80 passes, and a rounding moves
    the rest). Ducks writes the built-in's log out -- `-34.5` under
    `|w|^2` 1e-30, `0.5 ln(w.norm_sqr())` above -- and its division's
    1e20 at a pole; Kaliset divides by `|z|^2` part by part, as the
    built-in (a complex divided by a lifted real does so in the
    language: `(x r) / r^2` rounded otherwise). At depth Barnsley counts
    as the big-float truth at zoom 30 on 426 to 439 of 439 pixels, both
    rungs (`the_twins_without_a_native_tier_deep_zoom`); the three that never escape
    match the truth's mean `|z|` over the orbit at the native tiers'
    deep views (Kaliset, Ducks; Lattès at zoom 12, where five passes of
    an ergodic map still carry contrast) to under 2e-7 relative, both
    rungs (`the_twins_that_never_escape_deep_zoom`). The exception is
    Ducks on the scaled rung, 7.5e-5: there the derived equations
    recompute the reference's `t = f(Z) + c` in f32, which cancels where
    the reference grazes the log's singularity; the native tier reads
    the reference's low half (the plan's item 6).
    - The derivation reads `arg z` as `Im log1p(d / A)` with the cut's
      turns, and `log |a|^2` as `2 Re log1p(d / A)`, conditioned by
      `|A + d| / |A|` where the moduli's ratio is by its square.
    - `log1p` near -1 takes the log of `1 + z` itself (exact there):
      the squared form `|1 + z|^2 - 1` cancels to nothing as an orbit
      grazes a log's singularity, which read inf on Ducks' floatexp
      rung. That rung takes log, arg and log1p by mantissa and exponent
      (`cfe2_log`, `cfe2_arg`, `cfe2_log1p_r`), f32 having flushed a
      reference's `|t|^2` under 2^-126 to zero.
  - **Magnet, Nova, Novaretti and Newton** (item 2, batch 4) end an
    orbit as it settles, `(s.z - s.prev).norm_sqr() < 1e-12` (the
    built-ins' test; `s.prev` set from z first in `step`), or escapes;
    Novaretti's test only settles, which makes it non-escaping and
    convergent, as the built-in (`perturb::never_escapes`). Magnet
    renders as its built-in on every pixel, both planes. The others take
    their powers by `exp(p log z)` where the built-ins' are polar, and
    the pass a settle test fires on is f32 rounding's, so counts part by
    one (Nova on 16% of pixels, Newton relaxed on 30%) -- against f64
    each is right on about half of those, and the twins' counts are off
    f64's by more than one no more often than the built-ins'
    (`builtin_twins_render_and_deep_zoom_as_their_builtins` measures
    so). Newton's `sin z - 1` and `cosh z - 1` have double roots, where f
    cancels to f32's noise as z nears one: no f32 path counts their
    settling (the direct one off f64's by more than one on 22% and 51%
    of pixels, the built-in's as the twin's).
    - At the native tiers' deep views (zoom 30) against big floats:
      Magnet's twin 439 of 439, its native tier 356; Nova and Newton (its
      Newton and Halley schemes) 439 and 439.
    - At basin boundaries, where pixels settle long before their
      reference (`the_convergent_twins_deep_zoom`, within one, against
      f64's own count there): Newton's `z^3 - 1` 436 of 439 (f64 439,
      the native tier 439), `z^p sin z - 1` 410 (f64 437: f32's rounding
      of what both rungs share, the reference's sin growing as
      `e^|Im z|`), Novaretti 409 (f64 401: chaotic past double precision
      by 150 passes).
    - A carried value only the test reads (`zold`) rebases on z's
      deviation alone (`Perturbable::pair`): weighed with z's, as a
      value the loop reads (Phoenix's previous z) must be, it held a
      pixel settled on a root to a reference wandering far, and it
      settled late or never (377 of 439 at the `z^3 - 1` boundary).
    - The derivation keeps a loop variable that holds the same value at
      every pixel as one (`delta::uniform_vars`): `let k = p.power;
      z ** k` derives as `z ** p.power` does, where its exponent read as
      varying before.
  - **The escape test at depth.** A custom formula whose perturbed test
    is `|z|^2` under a bound the same for every pixel (`!(|z|^2 > T)`, a
    Rhai `escaped`; UF's `|z| < T`) tests by the built-ins' delta-aware
    margin, `(|Z|^2 - T) + 2 Re(conj Z d) + |d|^2` with `|Z|^2` kept in
    double float (`perturb::modulus_bound`, `PerturbTier::Custom::margin`,
    `escape_margin_custom`): its full z in f32 quantizes away a deviation
    below an ulp of the bound, so where a band's edge -- a level set of
    `|z_n|^2` -- crosses the view, the custom formula's count was noise
    and the built-in's sharp (Feather's twin at its suite view, zoom 30:
    219 of 439 right, now 439). One reference and nothing carried; the
    others test their own way.
  - **Origami** (item 3, step 3e): `prepare` builds its 64 fold lines
    (an array, each line's endpoints two hashed points folded through the
    lines before it), `step` folds once a pass by `ctx.pass`, and `fn
    creases` gives the paper's height and gradient, so the Creases relief
    field and terrain source draw it. It renders as the built-in on every
    pixel (two seeds and spreads, `position_map`), and its crease field
    matches the reference as the built-in's does, for every pattern, the
    flip and a terrain's reach. Its line table is the built-in's bit for
    bit (`origamis_lines_are_the_builtins`): the built-in builds it on the
    CPU in f32, and each line folds through every earlier one, so `prepare`
    computes in f32 an operation at a time (`to_f32`) -- in f64 the tables
    parted by whole units after the first few lines.
- **UF and Fractint formulas and colourings convert** (`lang/to_rhai.rs`;
  the Formula Editor's Edit as Rhai, `custom::convert_to_rhai`): the
  checked program is printed in these shapes, the file opened left as
  it is.
  - The state is `z`, what a pass reads before it writes it, and what
    the bailout reads (`#pixel` too when the loop reads it); the other
    variables are `let`s.
  - `global:` becomes `prepare`, and UF's own perturbation equations
    the `perturb_*` functions.
  - The conversions the checker makes are written out where Rhai
    needs them, and Fractint's conventions are explicit (real-part
    tests and equality, `floor(x + 0.5)` for its `round`).
  - The bailout is negated as written, `!(...)`, so a NaN leaves as it
    does in UF.
  - The entry's comments, and those just above it, come along.
  - A `switch:` pair becomes one formula on both planes.
  - A colouring's state is what the checker packs. `final:` becomes
    `finish`, with `#index` and `#solid` its locals, returned. When
    `final:` ends by setting `#index`, that value is returned directly.
    `init:` and `loop:` are left out when the colouring keeps no state.
  - Measured on UF's public library:
    - all 9,250 compiling formulas convert, compile back and iterate as
      the original (the interpreter, seven points on both planes), and
      deep zoom is kept on all 310 that have it;
    - all 726 compiling colourings convert, compile back, colour as the
      original beside the Mandelbrot, and keep the same state size.
  - The test files' conversions run on the GPU as the interpreter
    does.
  - A config's UF source still compiles as UF.

**An entry** is `Name(setting) { ... }` in a `.ufm` (formulas), `.ucl`
(colourings) or `.frm` (Fractint) file; `;` comments to the end of a
line. A formula has `init:`, `loop:` and `bailout:` (the loop repeats
while the bailout test holds, at least once), or Fractint's shape: an
empty `:` starts the loop, and its last statement is the test. A
colouring has `init:`, `loop:` (after the formula's loop, each
iteration) and `final:`, which writes `#index` (the palette position) or
sets `#solid`. `default:` holds `title`, the view (`center`, `magn`,
`maxiter`, `angle`) and the parameter blocks. Either kind may start with
`global:`, which runs once per image, before any pixel: on the CPU, in
the interpreter, whenever a parameter or the view it can read changes.
What it sets is read-only elsewhere (UF's rule), and it cannot read
what belongs to a pixel (`#pixel`, `#z`, `#x`, `#random` ...). Its
values and arrays, every element: a formula's go into the uniform's
`fdata` (256 floats), a colouring's share its 64-float parameter block. A
formula may end with `switch:`, UF's switch to another formula of the
same file: `type = "Julia"`, then `destination = source` copies, the
source one of its own parameters or `#pixel` (`seed = #pixel`,
`bailout = bailout`). Here a switch that passes `#pixel` is the Julia
toggle: off, the formula runs; on, its target does, with `#pixel`'s copy
read from the Julia constant and the rest of the target's parameters its
own (`lang/switch.rs` splices the pair into one program). The config
carries both entries' text.

**The language.** Types `bool`, `int`, `float`, `complex`; an
undeclared variable is complex. Conversion only widens; `/` never makes
an int. `if / elseif / else / endif`, `while / endwhile`, `repeat /
until` (each loop capped at 4096 passes on the GPU). `|z|` is the
SQUARED modulus; unary minus binds tighter than `^`, so `-x^2` is
`(-x)^2`. Functions: `sqr sqrt log exp abs cabs recip ident zero`, the
trigonometric and hyperbolic functions and their inverses, `atan2`,
`cotan`, `cotanh`, `round trunc ceil floor real imag conj flip isinf
isnan`, and Fractint's `cosxx`. Parameters are `@name` (Fractint's
`p1..p6` without the `@`), declared with `param ... endparam` blocks
(`caption`, `default`, `min`, `max`, `enum`, `hint`); user functions are
`@f(z)` (Fractint's `fn1..fn4`), chosen in the panel from a list.
Symbols: `#pixel`, `#z`, `#numiter` (in `final:`), `#maxiter`, `#pi`,
`#e`, `#random`, `#center`, `#magn`, `#x`, `#y`, `#width`, `#height`,
`#screenpixel`, `#index` and `#solid`. A product with a real factor
(`2 * z`) scales the other operand, as the built-ins write it. An enum
parameter compares with an item's name (`@type == "Sine"`). A string
continues over lines with a `\` at the end of each. UF's compiler
directives choose lines: `$define`, `$undef`, `$ifdef`, `$else`,
`$endif`, with `ULTRAFRACTAL` and `VER20` to `VER60` defined, as Ultra
Fractal 6 defines them. Settings may share a line, separated by commas.
A constant expression WGSL would refuse to fold (`sqrt(-3)`, `1 / 0`)
reaches the GPU unfolded, which computes UF's NaN or infinity.

**Arrays** (UF's static arrays; the plan's item 3, steps 3a and 3f):
`float a[10]`, `complex m[3, 4]`, `float t[@n + 1]`, `complex
trace[#maxiter]`, each size a constant integer expression, which as in
UF may read the parameters and `#maxiter` (and `#pi`, `#e`, the
built-ins: `round(@w * 2.5)`), in any section, at most 256 floats each.
Such a size folds at the settings: the entry is checked at its
defaults (its own `maxiter`, else the panel's), and a config whose
values give other sizes renders a variant compiled at them
(`custom::resolve`), so a change of size recompiles and nothing else
does. Where they do not fit at those settings, the Escape panel says
why and the entry renders at its default sizes. Refused: a size
reading the picture's size (`#width`, `#height`, which differ between
the view, an export and its tiles), any other predefined name or a
variable; dynamic arrays (`setLength`, every use of which in UF's
library is behind `import`). Elements are `a[i]`, `m[i, j]`,
zero-based, the indices ints (`/` never makes one: `trunc` it), in the
declared order; a constant index outside its dimension is refused, as
UF refuses it, and one computed is clamped to the array (UF stops the
render with a message under `$define debug`, which a GPU cannot). An
array is zero until written (UF leaves it undefined). It is read and
written by element, and assigned whole (`x = y`) only from one of the
same size and type; it is no other kind of value.
- A `global:` array is the image's: computed on the CPU and packed with
  the other globals, read by index from `fdata` or the colouring's
  block.
- A per-pixel array lives in the formula's state (compared element by
  element for interior detection), or in a colouring's packed state
  when the loop carries it -- each element a float of its 32. An
  element written at a constant index before it is read in the same
  pass is that pass's, so a scratch array costs no state.
- Deep zoom and distance estimation take a global array read at an
  index the same at every pixel (a constant, a parameter's choice, the
  pass) as they take a parameter. A formula whose loop writes an array,
  or reads one where the iterate decides the index, keeps the direct
  path and has no derivative.
- Edit as Rhai converts them: each declared as `array(n, zero)`, a
  `global:` one built in `prepare` and read as `p.k[i]`, a carried one
  a field of the state (`s.last[i]`), a whole-array copy as `s.prev =
  s.last`. Rhai's arrays have one dimension, so a two-dimensional one
  converts flattened, as the compiler flattens it: `m[1, 2]` of a
  `complex m[2, 3]` reads `m[5]`. A size the settings fix converts as
  written (`array(2 * p.n, ...)`, the index `m[1 * p.n + 2]`), so the
  Rhai means the same at every value.
- A `switch:` target's sizes read the pair's parameters (a copied one
  the formula's) and fold at the values those hold there.

**Fractint's conventions** apply to a `.frm` file, or with the editor's
language set to Fractint (from its `parser.c`): a test is true when its
REAL part is nonzero, and `==` compares real parts; `round` is
`floor(x + 0.5)`; `sqr()` leaves its argument's squared modulus in
`LastSqr`; writing a parameter or a predefined name (`p1`, `pixel`)
makes a per-pixel copy that starts as the original. Its names `whitesq`
(the checkerboard), `scrnpix`, `scrnmax`, `center`, `magxmag`
(magnification, 1), `rotskew` (rotation in degrees, 0) and `ismand` are
read, in UF's language too (UF-format conversions of Fractint formulas
test `whitesq`); a formula that reads `ismand` gets the Julia toggle, which sets it
(the Julia constant is whatever the formula makes it, usually `p1`).
`rand` is a per-pixel hash, not Fractint's time-seeded generator, so a
picture reproduces; `srand` is not supported.

**Deep zoom: UF's perturbation equations.** A formula may carry
`perturbinit:` and `perturbloop:` (UF's): the start and the step of
`#dz`, the pixel's deviation from the reference orbit, with `#z` the
reference's iterate there and `#dpixel` the pixel's offset from the
reference point (UF's own Mandelbrot: `#dz = 2*#z*#dz + sqr(#dz) +
#dpixel`; its Julia starts `#dz = #dpixel`). The sections have their
own variables and see no other section's (the reference keeps only z);
`perturb = <bool>` in `default:` says when the equations hold. Such a
formula takes the perturbed path past the direct path's f32 limit, when
its test reads `#z` alone (or the init's constants: a `bailout = 4`
set in `init:` is put in by value, `perturb::perturb_test`) and it
reads nothing of the screen (`lang/perturb.rs`); its reference orbit is the formula itself run in
big floats (`lang/bigeval.rs`, the `MAP_CUSTOM` family), and the
equations compile with each value classed Full (the reference's scale)
or Small (the deviation's, held in units of S as the built-in tiers'
`w`); on the floatexp rung (past zoom 48) every value of the equations
is a `CFe2`, `#z` read from the reference's double-float entry
(`lang::wgsl::formula_perturb_fe`, with `PERTURB_FE_HELPERS` beside the
template's own). A formula whose loop carries more than z (a `zold`)
rebases only where the orbit ends. On the scaled rung the rebase's new
deviation is the two orbit records' difference plus the old one
(`assembler::rebase_custom_scaled`), never `z_full - Z_0` in f32: a
formula that starts at its pixel (`z = #pixel`, so `Z_0` is the
reference's c) lost the deviation to f32's 2^-24 of |c| that way --
58 of 439 sampled pixels right at the deep seahorse view (zoom 42),
now 401. Its derivative under perturbation where it follows one
reference and carries nothing besides z ([The derivative under
perturbation](#the-derivative-under-perturbation), step 9d); BLA where skipping is sound
([BLA](#bla), phase 4, step 9); nucleus references where the reference
wraps exactly ([Nucleus finding](#nucleus-finding), step 9c).
`interp::run_perturbed` is the f64 model of the path.

**Derived equations.** A formula without `perturbloop:` -- most of
UF's, every Fractint one -- gets equations derived from its own loop
when it can (`lang/delta.rs`, at compile time in
`custom::compile_formula`, which keeps the formula as written if the
derived WGSL fails validation). Each value becomes a pair: its value
at the reference (from `#z` and the reference point `#pixel -
#dpixel`) and the pixel's deviation (from `#dz` and `#dpixel`), by
rules free of the cancellation `f(Z + d) - f(Z)` suffers: sums,
products (`x B + (A + x) y`, two products; one where the deviations are
the same or opposite -- a square's `(2A + x) x`, `a (1 - a)`'s `x (B - A
- x)`), the quotient form `(da - (A/B) db) / (B + db)`, binomial
integer powers, `sqr`, `recip`, `|a|`, `conj`/`flip`/`real`/`imag`,
and `abs` by `diffabs` (`|A + d| - |A|` by sign case, per part for a
complex: `BinOp::DiffAbs`, the compiler's own operator; `uf_diffabs`,
`cfe2_diffabs`).
- The init's variables are constants per pixel (their values
  substituted in order, z's too).
- A branch derives when its conditions are the same for every pixel (a
  parameter's choice, a constant, the plane), so the reference and the
  pixel take the same arm. In a chain, a test that varies after tests
  that do not makes the rest of the chain their otherwise, as UF's
  `else` + `if` writes it (a Rhai `else if` lowers to one chain).
- **A branch in the init** (phase 4, step 4: every plane-aware formula
  has one): each way through the init's branches is a case
  (`delta::init_cases`, at most 16), with its own values and its own
  start for z. The equations branch on the cases' tests, one copy of
  the loop's each, or one copy when they come out alike.
- **An arm a parameter picks may go without:** when it has no
  equations (an inverse function, a user function of the iterate), the
  rest are derived, and the derived `perturb` setting (`perturb_when`)
  says they hold when that arm is not taken. A branch whose arms all go
  without has none.
- z written in steps reads its own reference value after each.
- **State carried besides z** (phase 4, step 5): a float or complex the
  loop carries from one pass to the next (a Phoenix's previous z, a
  Spider's moving c), or that the test reads (a convergent formula's
  previous z).
  - The reference records it each pass after z, interleaved in the
    orbit (`Program::carried`; the tier's `width`, 2).
  - The equations read that record and keep the pixel's deviation from
    pass to pass, in `w_prev` where the built-in Phoenix keeps its
    history.
  - A rebase moves the deviation as it moves `#dz`, on the pair's
    norm, as the built-in two-term tiers do.
  - Such a formula perturbs on the floatexp rung at every depth, as
    Manowar does: a carried deviation accumulates the scaled rung's
    f32 error (the Manowar twin at zoom 26: 337 of 439 sampled pixels
    right on the scaled rung, 420 on floatexp).
  - One value is carried; more is refused for now.
- **Transcendental functions and powers** (phase 4, step 6) derive by
  difference forms, each free of the cancellation:
  - `exp(A + d) - exp(A) = exp(A) expm1(d)`; `log` as `log1p(d / A)`;
    a real or complex power whose exponent has no deviation as
    `A^b expm1(b log1p(d / A))` -- or, where that exponent is a whole
    number from 1 to 64 (a Multibrot's `z ** p.power` at 4), by degrees,
    `Q_1 = d`, `Q_(j+1) = (A + d) Q_j + d A^j`: two products a degree, no
    logarithm and no cut, the branch the same for every pixel (the
    Multibrot's twin at zoom 30: 9 ms of GPU time, now 7; the built-in
    5); `sqrt` as `d / (sqrt(A + d) + sqrt A)`;
    `cabs` as `(2 Re(conj(A) d) + |d|^2) / (|A + d| + |A|)`;
  - `sin`, `cos`, `sinh`, `cosh` by the half difference (`sin(A + d) -
    sin A = 2 cos(A + d/2) sin(d/2)`), `tan` and the rest as
    `sin(d) / (cos A cos(A + d))` and its kin.
  - `expm1` and `log1p` are built-ins of the language too. Their WGSL
    takes a series near zero: the spec lets the GPU's `sin` err by
    2^-11 absolutely, and a GTX 1660's returns 0 below about 1e-7
    (`the_difference_functions_keep_their_precision_on_the_gpu`). On
    the scaled rung a function that is `x + O(x^2)` keeps a small
    value's units (`f(s x) / s`); on the floatexp rung `cfe2_expm1`
    and its kin take their series below 2^-20.
  - **Guards**: a form that divides by something zero where `A` is
    (the power's `log1p(d / A)`, sqrt's and cabs's sums) is chosen per
    pixel: the plain difference where `|d| >= |A|`, which has nothing
    to cancel there, the form otherwise (`Deriver::guarded`; the
    temporaries reused from statement to statement).
  - **The cut**: where A and A + d lie either side of the negative real
    axis, log, sqrt and a power not a whole real jump, which their
    continuous forms do not (`delta::cut_turns` counts the turns): log
    adds them back, sqrt and the power take the plain difference,
    large there. Without it a deep view misdraws the pixels whose
    orbits straddle a preimage of the cut.
  - The sine map and a map taking sqrt, log, tan, cabs, expm1 and
    log1p at once count right, at zoom 30, on 429 and 439 of 439
    sampled pixels. On the boundary itself, where pixels escape after
    thousands of chaotic steps, f32 deviations lose the count as f64
    ones do, only sooner (198 and 343, the f64 model 352 and 406).
- The equations' variables are the perturb functions' locals: the
  direct path's state holds the program's own alone.
- **User functions** (phase 4, step 6b): a user call's deviation is
  `IExprKind::UserDiff(f, A, d)`, `f(A + d) - f(A)` for the function
  the parameter picks. Each choice's form is derived once
  (`delta::user_diff_forms`, a small program of its own) and emitted
  once per shader as a helper switching on the choice, on true values
  (`uff_user_diff`, sin and sinh there by the helpers exact near zero)
  and over CFe2 (`ufe_user_diff_fe`); the f64 model runs the same
  program (`interp::user_diff`). A choice without a form (an inverse
  function, `cosxx`, round, trunc, ceil, floor) is left out by the
  derived `perturb` setting (`IExprKind::FuncChoice`, the parameter's
  slot), so picking it renders on the direct path. At zoom 30 a map
  adding `0.1 fn1(z)` counts right on all 439 sampled pixels with sin
  and with log, on both rungs.
- **Size.** A form that reads a reference value or a deviation more
  than once reads a temporary computed once (`Deriver::bound`), so
  nesting does not multiply copies. Within an init case, a loop branch
  the case decides (the plane of a converted switch pair) takes its arm
  alone, and cases whose equations come out alike share them. The
  derived equations may take at most 20,000 expression nodes
  (`delta::MAX_EQUATION_NODES`); past that the formula keeps the direct
  path.

- **A branch on the iterate** (phase 4, step 6c): the reference and
  the pixel may take different arms. Each test is taken at the
  reference's values and at the pixel's full values (reference plus
  deviation), giving each an arm. Where they agree, that arm's
  equations hold. Where they do not, the pixel's arm, run from the
  entry state, gives its full values; the reference's arm, run from
  the same state, the reference's; and each deviation is their plain
  difference, large there (`Deriver::iterate_branch`). Each arm's
  equations appear three times (shared, the pixel's, the reference's).
  Barnsley's test on Re z and a chain on both parts' signs count right
  on all 439 sampled pixels at zoom 30, on both rungs. Like the
  difference forms, it cannot outlast chaos: near Barnsley's boundary
  pixels escape after hundreds of steps of an expanding map, past what
  f32 deviations can follow.

What does not derive: an inverse function, a power of a varying
exponent, a branch in the init on the pixel, a loop inside the loop or
in the init, an int or bool carried or set in a branch on the iterate,
more than one value carried -- unless it stands in an arm a parameter
picks (or a user function's choice). Such a formula stays on the
direct path, as before.

**Hybrids** (`lang/hybrid.rs`, the Hybrid Designer panel): Kalles
Fraktaler's hybrid designer, as its manual describes it. Up to four
groups, each run its repeat count of passes, in order, looping back to
the group marked as the loop's start (those before it a prefix, run
once); a group's lines (up to two) run in sequence, each combining two
operators of the current z by `+`, `-`, `*` or `/`; an operator is
`(±|x| ± |y| i)^p · a` (a fold and a negation per part, an integer
power 1 to 12, a complex multiplier); the pixel is added once a pass,
after the lines. A hybrid is a custom source of the dialect `hybrid`,
its source the hybrid's JSON, so it travels in configs and PNGs as UF
text does. It compiles through generated UF text, checked with two
names only generated text may read: `#pass`, the loop pass being run
(`Predef::Pass`; on the GPU the `npass` field of `FormulaState`, which
`formula_loop` advances), and `#juliac`; each group is a branch on the
pass, and the program records its cycle (`Program::cycle`: the prefix's
passes, a round's passes). The Julia plane comes by `ismand`
arithmetic. The multipliers are the program's parameters
(`a<group>_<line>_<operator>`), so dragging one recompiles nothing; a
structural edit is a new source and a recompile, one undo step. A
one-group preset renders as the built-in it spells (Mandelbrot, the
cubic, Burning Ship, Tricorn, the two Perpendiculars:
`a_hybrid_renders_as_its_built_in`). Distance estimation reaches those
that fold too, through the Jacobian (phase 4, step 9d), on the direct
path; a hybrid carries no derivative under perturbation.

Deep zoom for hybrids (the plan's step 5b): their equations are
derived as any formula's (the pass and the phase are the same for
every pixel), and a pixel must keep the phase of the reference it
follows, which a rebase onto the orbit's start would break whenever
the pixel is not at phase 0 -- exactly when one passing near the
critical point must rebase (KF's manual names this as what breaks its
single-reference method). So the custom family computes `R = 1 +
period` references: the main one, and one per phase of the cycle,
each from the init's z with its pass starting at `prefix + q`
(`BigMachine::set_pass`). They are interleaved in the one orbit (entry
`m * R + r`), stepped together, a finished one padded with
`reference::ORBIT_PAD_E`; the shader's `m` is the flat index, stepping
by `R`, the equations reading `custom_pass(m)`, and `rebase_phased`
rebases a pixel at pass `i >= prefix` onto entry `1 + (i - prefix) %
period`. At most 31 phases (`perturb::MAX_PHASES`).

**Blends** (phase 4, step 8): a formula group may name a second formula
(`Group::blend`). Both steps run from the same z, and z becomes `mix(a's,
b's, t)` by the group's own parameter `g<N>_mix` (0 the first formula,
1 the second): a continuous morph between two formulas, which animates
as any parameter and deep-zooms (the mix is linear). The second
formula's parameters are under `g<N>b_`, captioned "Group N (blended)".

**Any formula as a group** (phase 4, step 7): in the designer a group
runs either KF's lines or a formula's step -- a built-in's Rhai twin, or
a Rhai formula from the library (`hybrid::Member`, its Rhai source in
the hybrid's JSON). Such a hybrid is composed from its members' programs
(`hybrid::compose`) rather than written as UF text:
- z is shared; each member's other variables, parameters, user
  functions and globals are its own, under its group's name (`g2_power`,
  captioned "Group 2: Power"), so a member keeps its state from its own
  last turn (a Phoenix's previous z).
- Every member's start runs, in order; only the first sets z, and its
  test is the hybrid's (its `bailout` stays the panel's). A group of KF
  lines becomes a member of its own, its multipliers keeping their
  `a<group>_<line>_<operator>` names.
- The loop runs the step of the group whose turn it is, a branch on the
  pass, with the cycle. A member's own perturbation equations, perturb
  setting and degree are dropped (the hybrid's are derived whole); one
  that reads the pass itself is refused.
- The result prints as one Rhai formula with its `INFO.cycle`
  (`lang::to_rhai`), and deep-zooms as any hybrid does.
- In the designer, a group whose formula changes starts at that
  formula's defaults; one that keeps its formula keeps its values.

The Mandelbrot's and the Burning Ship's twins taking turns run as KF's
lines taking the same turns, on both planes, and at depth count as
those lines do (103 and 105 of 439 sampled pixels at zoom 24, 83 and 84
at 51, at the walked edges of `a_hybrid_perturbs_with_a_reference_per_phase`).
Those counts are low for both, and not a fault of either: the walked
edges of a map with a fold are chaotic. Measured on the Burning Ship's
twin alone, walked to an edge: at zoom 12, where f32 resolves the
pixels easily, the plain direct render counts 93 of 439 sampled pixels
as the big-float reference does, the built-in's hand-derived tier 105,
the twin 104, and the paths agree with one another no better (the
built-in and the twin on 109; each with its own floatexp rung on about
as few). The counts there spread from the first steps to the cap and
follow rounding, the reference's included.

**Not yet:** UF's classes and plug-ins (`import`), dynamic arrays and
arrays sized by a parameter, colour values and direct colouring.

**Measured against UF's public formula library** (its full download,
read for measurement and never bundled; `measure_a_uf_corpus`, an
ignored test pointed at a folder by `UF_CORPUS`): 9,250 of its 9,831
formula entries compile (94%), and 423 of its 1,209 colourings. The
WGSL of every one that compiles validates, its derivative's too. What
stops the rest: classes (`import`, 325 entries), arrays (296), the
colouring state cap (335 colourings at 4 floats; it is 32 since, which
admits 96% of the colourings that reach it), parameters past 64 floats
(about 200), colour functions (46). Deep zoom: 2 of the formulas carry
their own perturbation equations and 501 more get derived ones that
hold at their defaults (308 before branches in the init and excluded
arms, phase 4 step 4). The commonest blockers for the rest are a test
that reads a variable the loop changes, a user function, and a branch
on the iterate.

**Test files.** `tests/formulas/` holds entries in both dialects that
between them use every construct (`language.ufm`, `language.ucl`,
`fractint.frm`); each compiles, and on the GPU runs as the f64
interpreter does (`test_files_run_on_the_gpu_as_the_interpreter_does`).
A construct added to the language gets an entry there.

**How it joins the engine.** A compiled entry is registered as a
`FormulaDef` or `ColoringDef` under the name `custom-` plus a hash of
its source, which every name-keyed cache then keys on. The config
carries the source (`EscapeConfig::custom_formula`, `custom_coloring`),
and the renderer, the CLI and the panel compile it before resolving the
name, so a saved file, a PNG or an upload renders anywhere.

- A **formula** is a program (`FormulaFeature::Program`) the direct
  template drives: its own seed, loop and bailout. Without perturbation
  equations it renders on the direct path only, so deep views stop at
  the f32 limit (about zoom 14), as for a built-in with no perturbation
  tier; with them, `PerturbTier::Custom` takes it to any depth (above). It has no Julia toggle
  (its `#pixel` is written in) unless it reads Fractint's `ismand` or
  its `switch:` passes `#pixel`.
  Interior detection compares its whole state (`formula_same`: `z` can
  repeat while the rest has moved on), and the embossed relief runs its
  pair through the program's own functions. A float or complex
  parameter named `bailout` is the panel's bailout, UF's convention, so
  the smooth colourings and the formula escape at the same radius.
- A formula's **derivative** with respect to `#pixel` is derived from
  the program (`lang/derive.rs`, forward mode): every complex variable
  the iterate depends on gets a companion assigned just before it, and
  the result is an ordinary program, compiled in place of the plain one
  when a colouring reads the derivative (`formula_dz`). So distance
  estimation and the analytic relief work for a custom formula, on
  either plane.
- **The Jacobian** (phase 4, step 9d: `derive::with_jacobian`). A
  formula whose iterate passes through a fold (`abs`, `conj`, `real`,
  `imag`, `flip`, `cabs`, `|x|`, an argument, a real variable that
  follows the pixel) has no complex derivative, but it has a real one,
  a 2x2 matrix written as the real-linear map `d -> P d + Q conj(d)`
  (Q zero where the map is holomorphic), as the real-linear BLA tables
  are. Every variable the iterate depends on, real ones included (their
  pair has `Q = conj(P)`), gets two companions, and each rule is the
  real-linear one: `conj` swaps and conjugates the pair, `abs` takes
  each part's by its sign, `cabs` and `|x|` through `a conj(a)`, `arg`
  through `da / a`. A user function on the pixel's path is chosen at run
  time and may be a fold, so it always takes the Jacobian: its
  derivative at `a` is `A d + B conj(d)`, and `IExprKind::UserJac` reads
  A or B for the chosen function from one helper (`uf_user_jac`; the
  forms, `derive::user_jac_forms`, are each function's own rule applied
  to the unit pair, so they cannot drift from it), the pair then
  `A (P, Q) + B conj(Q, P)`. An arm per choice at each call, the first
  way, made a formula of a thousand user calls a derivative of tens of
  megabytes (`as.ufm`'s Chaotic-fly: 54 MB of WGSL, which validating
  took minutes over and overflowed the stack). A derivative adding more
  than `derive::MAX_DERIVATIVE_VARIABLES` (2,048) variables is refused:
  over UF's public library half add 15 or fewer and 99% at most 455;
  57 giants of 9,250 go without, as a formula without a derivative does
  (`a_user_call_costs_the_derivative_what_a_builtin_does`,
  `every_user_choice_has_its_jacobian`). What a colouring reads is one complex
  number: the escape potential's gradient is `J^T u = conj(P) u + Q
  conj(u)` for `u = z/|z|`, and `dz = P + conj(Q) u^2` reports it as a
  complex derivative does (`|dz|` its length, `z / dz` its direction;
  `derive::effective_dz`, `formula_dz_of`). So distance estimation, the
  normal map and analytic relief run unchanged on the Burning Ship's
  twin, a Tricorn, a hybrid that folds, a UF formula with `@fn1 = abs`.
  Checked against central differences of `|z_n|` (`lang::derive`
  tests) and, on the GPU, against the interpreter
  (`derivatives_match_the_interpreter_on_the_gpu`,
  `a_custom_formulas_derivative_drives_distance_estimation`: 98% of
  escaping pixels to a part in a thousand). The built-in Burning Ship has
  no derivative; its twin does.
- A **colouring** compiles to the built-in colouring interface, so it
  runs on every path, perturbed and recoloured included. Its variables
  that live from one iteration to the next keep up to 32 floats, and
  the compile fails past it; an undeclared variable that only ever
  holds real values takes one float, not two. The first four are the
  accumulator's `vec4`; the rest (`AccumWidth::Extended`) are the
  colouring's private `ufc_ext`, which every template saves beside the
  accumulator, in the records after `accum2` and in the perturbed
  resume state, and restores wherever it loads them. Each `vec4` past
  the first adds 16 bytes a render pixel to both. An
  untagged colouring colours inside and outside; `(OUTSIDE)` and
  `(INSIDE)` one side. `#solid` leaves the pixel to the background.

**The Formula Editor** (Window menu, Escape mode; `src/ui/formula_editor.rs`)
lists a file's entries above its source, checks each as a formula and as
a colouring on a pause in typing, and offers it as what it compiles as
(a colouring only if it sets `#index` or `#solid`). Using one puts it in
the config, one undo step; a formula's `center` and `maxiter` come with
it the first time (its `magn` does not yet: what span UF's magnification
1 shows is unverified). **Live** re-applies the entry in use whenever an
edit to it compiles again. Files live in a `formulas/` folder in the app's
data directory (localStorage on the web, `storage::formula_store`); Open
reads a `.ufm`, `.ucl` or `.frm` from disk, Export writes one; two example
files ship (`assets/formulas/`), each entry compiled by
`every_example_entry_compiles`. Scripts: `escape.custom_formula` and
`escape.custom_coloring` ([SCRIPTING.md](SCRIPTING.md)).

Measured against the built-ins it rewrites, the language's Mandelbrot,
Lambda and Magnet I are identical, and so is a custom colouring at
depth on the perturbed path
(`custom_entries_render_as_their_built_ins`); distance estimation on
the language's Mandelbrot is identical on both planes
(`a_custom_formulas_derivative_drives_distance_estimation`), and so is
its Embossed relief, every reduction (`a_program_embosses_as_its_built_in`).

---

## Mode B: fields

A field has no escape and no classification: `field_init` and
`field_step(n, p, state)` return a value and a gradient per term, which
the template sums. `max_iter` is the **term count** here. Per-pixel state
is eight floats (`FieldState`), because the FTLE tangent matrix needs
more than four. A field colouring defines `field_color(sum, grad,
n_terms) -> FieldShade { t, lum }`: `t` is the palette position and
`lum` multiplies the colour, which is how hillshading works. f32 is
enough; the sums converge absolutely and the statistics are averages.

| field | display name | default colouring |
| --- | --- | --- |
| `weierstrass` | Weierstrass Field — the 2D Besicovitch–Ursell lacunary sum, with an analytic gradient | `field_hillshade` |
| `markus_lyapunov` | Markus–Lyapunov — the logistic map's Lyapunov exponent with `r` switching between the pixel's two coordinates | `field_diverging` |
| `standard_map_ftle` | Standard Map FTLE — the finite-time Lyapunov exponent of the Chirikov standard map | `field_diverging` |

The colourings are `field_value` (Field Value), `field_diverging`
(Diverging: zero mid-palette, the sign picks the half) and
`field_hillshade` (Hillshade, from the analytic gradient; flat lighting
on a field without one, `FieldDef::has_gradient`). Fields write no
terminal records, so they have no recolour cache and do not perturb.

---

## Mode D: the flame as a distance field

A mode-D entry answers "how far is this pixel from the set". The WGSL
contract is `ifs_evaluate(uv) -> IfsResult` for a planar entry, with `uv`
the pixel's normalised offset in [−½, ½], and `ifs_color(res) ->
IfsShade { t, lum }` for a colouring. `res.distance` is **in pixels**:
in plane units it underflows f32 at zooms the walk still resolves.

| name | display name | |
| --- | --- | --- |
| `ifs_flame` | Flame Attractor (Distance) | the loaded flame's attractor, by inverse iteration. Planar; needs a qualifying flame; default colouring `ifs_distance` |
| `ifs_flame_3d` | Flame Attractor (Solid) | the same in three dimensions, sphere-traced. Solid; needs a qualifying flame; default `ifs_address` |
| `quaternion_julia_solid` | Quaternion Julia (Solid) | `q ↦ qⁿ + c` on a 3D slice, by the Hart–Sandin–Kauffman distance estimate. Solid; needs no flame; default `ifs_level` |
| `kifs_solid` | KIFS (Solid) | Knighty's kaleidoscopic IFS: rotate, fold (tetrahedral, cubic, octahedral, Menger, icosahedral or dodecahedral), rotate, `z = s z − o (s − 1)`. Solid; needs no flame; default `ifs_level` |
| `mandelbox_solid` | Mandelbox (Solid) | Lowe's box fold, sphere fold and scale, `z = s fold(z) + c`, the point or a Julia constant; the distance `0.95 |z| / |dr|`. Solid; needs no flame; default `ifs_trap` |
| `mandelbulb_solid` | Mandelbulb (Solid) | White and Nylander's power in spherical coordinates, `z = zⁿ + c`, the point or a Julia constant; Hart's estimate times the Step Factor. Solid; needs no flame; default `ifs_trap` |

**Solids written in Rhai** (the KIFS plan, B3; the formula language's
solid entry) register beside these under their `custom-` names
(`escape::custom::compile_solid`, found by `ifs::get_ifs`, never
listed): the template's seven parameters, the solid's own from slot 7,
then Colour Depth and **Ball Radius** (`INFO.radius`, the ball the march
starts from; a parameter the panel can move, which `standalone_radius`
reads). The walk around the program (`lang::wgsl::solid`) is the
built-ins' shape, coloured by "Colouring solids" from the state: the
closest approach of `z` over the first `colour_depth` iterations (the
trap), its `dr` (the level), its `address` field if it has one.
Example: `assets/formulas/solid-mandelbox.rhai`, pixel for pixel the
built-in Mandelbox under the trap colouring.

**KIFS (Solid)** ([escape-kifs-and-variations.md](../projects/escape-kifs-and-variations.md),
A1) takes its folds from Syntopia's "Folding Space" (2011): the
Sierpinski tetrahedron's three plane folds, `abs` of each axis
(cubic), those and a descending sort (octahedral), and the sponge's
correction of `z` (Menger); a rotation before the fold and one after
(Knighty's), three Euler angles each in degrees. Its distance is
`(|z_n| − |o|) s⁻ⁿ` once `|z|` passes `|o|`, zero if it never does
within the iterations: a lower bound, because every fold and rotation
is 1-Lipschitz and fixes the origin, so the set lies within `|o|`
(which is also the ball the camera frames, `standalone_radius`). The
address is the fold history, a base-8 digit per iteration (which of
three mirrors fired) for eight iterations from the `address_symmetry`th
on (from the second, mirror images share their orbit: the colouring is
as symmetric as the set); the colour that first digit. Presets: KIFS Menger,
Tetrahedron, Octahedron (the octahedron flake), Turned, Bloom and
Columns. The tests hold it to a CPU twin (`gpu_tests::Kifs`), pixel for
pixel. Like the quaternion Julia it is f32 and absolute: no deep zoom.

**The flame-less solids colour from the start of the orbit.** A solid is
drawn where the march stops, a pixel's width from the set, and that
width is the camera's, so a quantity read from where or when the orbit
escapes changes with the camera (measured: it never settles as the view
refines). The KIFS and the quaternion Julia read their first
`colour_depth` iterations (default 3) instead: the orbit trap is the
closest approach to the origin, as coordinates the set's symmetry
keeps; the level `log2` of the running derivative at the depth (the
quaternion) or the iteration of the closest approach (the KIFS, whose
derivative is `s^n` everywhere); the quaternion's address is which side
of the plane across `c`'s axis each iterate falls, then the angle from
that axis. `the_solids_colours_settle_as_the_view_refines` holds them
to it on the CPU twins. **The flame solids** (`ifs_flame_3d`) colour the
same way: a colourings' walk at the hit point, `colour_depth` levels
from the sample's own point (link 0 of the chain, the final transform
undone), the child the walk's ranking puts first at each level -- the
walk's own path for a beam of one -- the trap the closest approach to the
ball's centre, the level the band of that approach. It starts from the
sample's point rather than its link because a sample near the target
starts the main walk several levels down, and a window counted from
there would part pixels along the links' edges. The address is the
winning branch's, as before; **Address Start** shifts its first digits
out so every copy of a first piece reads alike. At a deep zoom the
level and the trap are flat (every pixel's point is the target's in
f32); the address is the colouring there.

**The Mandelbox and the Mandelbulb** (the plan's A2, A3) follow the same
rules. Their balls are their escape bounds (`mandelbox_radius`: Lowe's
`2L(|s| + 1) / (|s| − 1)` in the largest coordinate, `(2L|s| + |c|) /
(|s| − 1)` for a Julia; `mandelbulb_radius`: `2^(1/(n−1))`, or `1 + |c|`),
held by `the_solids_lie_within_their_balls`. Lowe's `|z| / |dr|` runs at
0.95 of itself, which makes it a bound at every point the gate samples;
Hart's estimate is no bound for the bulb (measured 1.7 times the distance
beside a thin part), so it carries a **Step Factor** (default 1) for the
user to lower when thin parts break up. Colours: the level `log2` of the
running derivative at the colour depth (the bulb's per power, `/ (n −
1)`); the trap the closest approach (the box's sorted absolute
coordinates in units of its ball, the bulb's `(z, |xy|)`); the address
base 12 for the box (how many axes the box fold reflected, which sphere
case applied: kept by the cube's symmetries) and the hemispheres then the
polar angle for the bulb (kept by its turns about z). The tests hold both
to CPU twins (`gpu_tests::Solid`), pixel for pixel on the GPU.

The colourings are `ifs_distance` (Distance: the set with an antialiased
edge and optional contour bands), `ifs_level` (Escape Level: shells of
constant inverse-orbit depth), `ifs_address` (Branch Address: the branch
taken at each level as a base-N fraction — the analogue of transform
colour, and the one that survives a deep zoom), `ifs_trap` (Orbit Trap,
in the expanded frame) and `ifs_measure` (Measure: the flame's invariant
density and colour, read through the walk instead of sampled by a chaos
game; it needs a coarse pass over the ball, `ensure_coarse`).

**The flame becomes maps on the CPU.** `scene::ifs_analysis` turns the
flame into maps and decides whether it qualifies (affine, or one of the
nonlinear kinds the walk supports), with a bounding ball;
`scene::ifs_estimate` is the CPU reference walk and seeds the beam the
shader starts from. The walk follows `beam` branch addresses at once for
up to `levels` inverse maps. At depth the reference/delta split lives on
the CPU: the shader starts from a handover state where the delta is small
enough to carry as one f32 point (`pack_seeds`), and the centre is read
at the precision the zoom asks for (`centre_at_precision`). A flame that
does not qualify draws nothing, and the panel says which condition
failed. Xaos flames walk their transition graph.

**The plane's orientation is the flame's.** `view_basis` mirrors the
plane so that screen-down is world +y, as on a flame: the escape view's
centre, rotation and zoom are the flame's pan `(x, −y)`, rotation and
zoom, while gestures still move `(re, im)` with Im up. (This was flipped
on 2026-10-10; before it the attractor rendered upside down against its
flame.)

**Solids** (`IfsDef::solid`) march a ray through the distance field on
mode D's camera and lights; see [3D](#3d-solids-and-terrains).
`formula_is_solid` is the question the UI asks: escape mode is not
three-dimensional, one formula in it is.

Mode D has an interaction preview: within 250 ms of the last edit
(`ESCAPE_INTERACTION_WINDOW` in `src/app/mod.rs`) it renders one pixel
in each 2×2 block and fills the block from it (`set_preview`), and the
full render lands when the edits stop.

Design record: [ifs-distance-rendering.md](../archive/escape-time/ifs-distance-rendering.md);
the measure colouring, [ifs-measure-by-inverse-walk.md](../archive/projects/ifs-measure-by-inverse-walk.md).

---

## The pipeline

```
config ──▶ assemble ──▶ iterate ──▶ resolve ──▶ [shared tail]
             │             │            │          density effects
             │             │            │          tonemap (Linear)
             │             │            │          colour effects
             │             │            │          readback
             │             │            └─ relief, texture overlay,
             │             │               downsample of the AA grid
             │             └─ direct / perturbed / field / walk,
             │                chunked; writes colour, coverage,
             │                the height field and terminal records
             └─ one formula + one colouring spliced into a template
```

### Shader assembly

`assembler.rs` splices definitions into fixed templates by **whole-line
markers**: a line that is exactly `//__FORMULA__`, `//__COLORING__`,
`//__ESCAPE_TEST__`, `//__ACCUM_UPDATE__`, `//__LENS_APPLY__` and so on
is replaced by its text, or by nothing. Feature-gated code is spliced or
omitted at assembly time rather than branched on, so a Mandelbrot with
`smooth` carries no accumulator and Kaliset carries no escape test.

There is a template per path: the direct template (`TEMPLATE`), the two
perturbed rungs (`PERTURBED_TEMPLATE`, `PERTURBED_FE_TEMPLATE`), the
recolour pass (`RECOLOR_TEMPLATE`), mode B (`FIELD_TEMPLATE`) and mode D
(the planar walk, its recolour and relight, the solid template
`IFS_3D_TEMPLATE`, and the path tracer). The renderer caches compiled
pipelines by a key naming everything spliced.

The direct template also splices **interior detection** when it is
invisible: a bit-exact repeat of `z` (compared through `bitcast`, at
Brent checkpoints) proves the f32 orbit is periodic, so the pixel stops
early with the same result it would reach at `max_iter`; for a custom
formula, a repeat of its whole state. It is off for
colourings that draw the interior, accumulate, or detect the period
themselves.

Every assembled combination is parsed and validated by naga at test time
(`every_combination_parses_as_wgsl`, `perturbed_template_validates_for_every_coloring`,
`every_field_coloring_combination_validates`, and the lens and solid
variants), and held to the same fast-math lints as the variations
(`escape_wgsl_passes_the_fast_math_lints`: no self-compares, no
self-divisions; see the Metal notes in [CLAUDE.md](../../CLAUDE.md)).
`esc_cpow` returns 0 at the origin, so its polar `atan2` never sees a
zero pair.

### The output contract and the shared tail

The compute pass writes `rgb` in linear light and `a` as coverage, 1
wherever the colouring drew. Palette texels are decoded with `pow(2.2)`
on lookup (`esc_palette`), the same `srgb_to_linear` convention the flame
plot uses, so the tonemap's display encode returns them to the palette's
colours.

`render_with` (`src/renderer/render.rs`) builds the flame renderer with
`FlameRenderer::for_tail` for an escape config — its own per-pixel flame
state made 1×1, since only the tonemap of another engine's image is used
— loads the config (palette, tonemap uniform, curve, background), and
dispatches `render_escape`. That renders to settlement and calls
`escape_tail`: density effects, `tonemap_pass_with_input` on the escape
image, colour effects, readback. The palette an escape render binds is
`escape_palette_view(stepped)`: the ordinary palette texture, or the
stepped table when `palette_map.stepped` is on.

### Tone mapping on entry

A flame's Logarithmic tone map renders a unit-range image black. So
`ui::render_mode::switch_render_mode`, the one writer of
`ConfigPath::RenderMode`, sets Linear tone mapping with exposure 2.0 and
gamma 1.0 (`ESCAPE_EXPOSURE`, `ESCAPE_GAMMA` in `config/defaults.rs`) in
the same undo step as the mode change — when entering from a flame mode
whose tone map is Logarithmic (`entry_tone_mapping`, shared with the
script handles). The reset is one-way on purpose; the function's doc
explains why restoring on exit is worse. In Escape the tone-map mode
selector is greyed out and the tone-map presets are hidden
(`ui::visibility::Control`): every preset is Log-calibrated.

---

## The renderer

`EscapeRenderer` owns the pipelines, the output and per-pixel buffers,
the reference-orbit cache and its GPU mirror, and the pacing state.
`render(device, queue, encoder, escape, palette_view, palette_generation)`
records the next piece of work and returns whether the image is final.

### Paths

| path | when | chunks by |
| --- | --- | --- |
| direct | mode A short of its direct limit (zoom 6–14, [below](#where-the-direct-path-ends)), or a config that does not perturb | row band |
| perturbed, scaled rung | perturbing config past its direct limit, to zoom 48 (`PERTURB_FLOATEXP_ZOOM`) | iteration window |
| perturbed, floatexp rung | past zoom 48; Manowar at every perturbed depth | iteration window |
| field | mode B | row band |
| walk | mode D | row band, by walk steps |
| recolour | the iteration is unchanged since the last settled render | one dispatch |

The perturbed path is taken only when the formula has a tier, damping is
off, biomorph is off (a custom formula takes both into its program, and
perturbs with them: [Damping and the biomorph](#damping-and-the-biomorph-on-custom-formulas))
and the view is past the direct path's limit at the
render's size (`wants_perturbation(escape, render_lines)`); otherwise a
deep view renders the direct path's honest f32 mush. `usable_depth` reports which
case the panel is in (`UsableDepth::Perturbed` or
`UsableDepth::Direct(14)`), and `derivative_gap` why a derivative
colouring would be flat. A perturbing view also falls back to the direct
path when the centre strings do not parse, or when its per-pixel state
would not fit the device (the direct path bands by rows and fits any
size).

### Chunking and the GPU watchdog

A dispatch long enough to trip the driver watchdog (about 2 s on
Windows) loses the device, so no path issues an unbounded one.

- **Direct and field** renders have no per-pixel resume state, so they
  split by **row band**: `DIRECT_DISPATCH_BUDGET` (2.5e11
  pixel-iterations) per dispatch.
- **Perturbed** renders keep per-pixel state (48 bytes a pixel; more for
  Phoenix's deep rung, a wide accumulator or a carried derivative) and
  split by **iteration window**. The first chunk is sized from a static
  seed, `PERTURB_CHUNK_BUDGET` / pixels (8e9; 6e8 on the floatexp rung),
  clamped to 16..65,536 iterations. Later chunks grow from **measured**
  GPU time — timestamp queries where the device has them, the
  `on_submitted_work_done` callback otherwise — but never past 64× the
  seed (`CHUNK_SEED_HEADROOM`) or 1,048,576 iterations. Measurements are
  tagged by cost regime (rung, size, tier) and discarded across a
  change, and a restart is sized only from a first-chunk measurement,
  when every pixel is alive.
- **Mode D** bands by walk steps (`IFS_DISPATCH_BUDGET`, and
  `IFS_SOLID_BUDGET` for a solid's march); its path tracer bands by
  measured row cost under a ceiling (`renderer/solid_path.rs`).

Mid-render, an unfinished pixel keeps the previous frame's value, so a
chunked render never flashes black. Batching does not change the result:
the settled image is the same at any chunk size.

**Circuit breakers.** Each generator has a shift that halves its budget:
`direct_shift`, `perturb_shift`, `path_shift`, clamped to 6. The GPU
device-lost callback (`gpu::device`) calls `note_device_lost`, which
raises the shift of whichever generator had work in flight and saves all
three to `gpu_tuning.json` — in the app data directory on desktop, in
browser storage on the web — so the next session starts below the budget
that lost the device. A direct band that survives but takes over 700 ms
also halves the direct budget, for the session only.

### Antialiasing

`supersample` (1–8, `MAX_SUPERSAMPLE`) renders an N×-per-axis grid and
downsamples it. `affordable_supersample` reduces the factor until the
grid fits a 1.5 GiB per-renderer budget, the device's storage-binding
limit and its texture-dimension limit; the viewport and the export's
allocation precheck share it, so they agree. Where the grid falls short,
**exports make up the rest by accumulation**: several ordinary renders,
each displaced within the pixel (`sample_grid`, `set_sample_offset`), are
averaged into a write-only ping-pong pair (`begin_accumulation`,
`accumulate_sample`). A sub-pixel shift is a shift of the view, which the
direct path expresses through its centre and the perturbed path through
`ref_offset`, so no template changes. Where only a 1× grid fits, 8× is
8 × 8 = 64 accumulated renders.

`downsample` combines the grid: `Box` (the linear average, default),
`Perceptual` (averaged in gamma space, which keeps mixed hues apart) and
`Vivid` (the average, with the saturation the samples had restored).
Coverage is averaged once, as straight alpha.

**During animation playback** with the frame governor on, the viewport's
factor is governed instead (`src/app/aa_governor.rs`): a step is shed
when several recent settled frames came more than 1.5 budgets late,
probed back up after a clean run, and a failed probe waits twice as long
next time. Stills, exports and video render at the config's factor.

### The recolour cache

Each iterate template writes a terminal record per pixel on its
finishing pass — `z`, `dz`, `n`, the classification and the
accumulator: 32 bytes, 40 with a wide accumulator. When the next frame's
**iteration identity** is unchanged (formula, params, view, `max_iter`,
bailout, damping, the compiled derivative and interior flags, the sample
offset, and the colouring itself where it takes part in the iteration —
an accumulator, period detection, `PaletteInLoop`, `TextureInLoop`), the
frame is one recolour dispatch over the records. Palette, colouring and
relief edits at depth cost a fraction of a millisecond instead of a
re-iteration. A pixel count past the device's binding limit turns the
records off and renders uncached. Mode B writes no records; mode D does,
and for a solid the cached path is its relight.
`recolor_cache_is_invisible_and_invalidates_correctly` holds a cache hit
bit-identical to a fresh render.

### Memory, device loss and the web

The renderer allocates per-pixel state on demand: iteration state only
on the perturbed path, the height field only when relief, contrast, the
overlay or a terrain needs it, the accumulation pair only for
accumulation. `release_pixels` gives the per-pixel memory back and keeps
pipelines and orbits (a terrain's section renderer between bursts);
`resident_bytes` reports what is held. `allocation_error` refuses sizes
the device's declared limits cannot hold before allocating, and an
out-of-memory error scope around the whole render turns a rejected
allocation into `RenderError::OutOfMemory` instead of a black image.

On WebGPU, dropping a buffer frees nothing, so everything the renderer
creates is released in `destroy`. The app destroys its renderer when it
leaves escape mode (`release_inactive_engines`) and while an export runs
(`freeze_viewport`); it is rebuilt from the config on the next escape
frame. After a device loss on desktop the app rebuilds the GPU context
and recreates the renderer lazily; a browser cannot recover a lost
device, and the panel asks for a reload.

`diag.rs` keeps one snapshot of where an interactive render's time went
— path, settle time, reference build or reuse, BLA table, orbit upload,
chunk size — which the panel's Diagnostics section shows.
`render_progress` reports iterations done against wanted for the
progress readout.

---

## Deep zoom

### The view

`center_re` and `center_im` are exact decimal strings (plain decimals:
digits, a sign, a point, no exponent). `zoom_log2` is an `f64` base-2
exponent: 0 is the formula's home view (a span of 4) and each +1 doubles
the magnification, so animating it linearly reads as constant zoom
speed. The panel shows base 10 (`zoom_log10`); the engine keeps base 2,
because it is a bit count to the fixed-point sizing and an exponent to
the floatexp arithmetic. The centre strings are undoable but not
animatable.

### Arbitrary precision

- **`fixedpoint.rs`**: sign-magnitude `u64` limbs with 8 integer bits
  (`INT_BITS`) and an implied binary point, plus one guard limb. A
  reference orbit lives in a bounded box, so it needs fixed point, not
  floating point: no exponents, no normalisation, no rounding modes, and
  the core never divides. `limbs_for_view` sizes the precision from the
  zoom or the centre's own digits, whichever is deeper. Integer limbs
  compute a reference orbit bit-identically on every platform, wasm
  included.
  `FloatExp` export (leading-zero count and shift) is exact near zero,
  where rebasing happens.
- **`bigfloat.rs`**: the same limb cores with one exponent, for Newton's
  step, which divides by a derivative no fixed point can hold. Division
  is a Newton–Raphson reciprocal.

### Reference orbits

`reference.rs` computes one full-precision orbit per (centre,
precision, map) and the GPU iterates each pixel's delta against it.
Entries are stored as f32 mantissas with double-float residuals and a
per-entry exponent, plus `|Z|²` as a double-float pair for the
delta-aware escape test. Zhuoran's rebasing restarts a pixel against the
reference's start when its delta outgrows the reference, so near-zero
passes need no wider storage.

- **Deepening is an append**: the live fixed-point state is kept, so
  raising `max_iter` extends the orbit in place (`OrbitCache`).
- **Relocation**: a pan or zoom-to-cursor re-anchors the existing orbit
  (`relocate_to`, `ref_offset`) instead of recomputing it, on the
  parameter plane, within a pixel-offset cap.
- **Progressive in the app**: `progressive` puts the build on a worker
  thread (`OrbitWorker`) and frames render against what has landed;
  during a continuous gesture a frame is served from the last published
  orbit and marked unsettled. Where the build is predicted to be slow,
  the last good frame is held and the panel shows progress instead. In
  the browser there is no worker: the build is sliced per frame
  (`OrbitCache::get_budgeted`). Exports leave `progressive` off, so they
  block on the full orbit and stay deterministic.
- **The period hint** (`reference_period`) names a nucleus's period; the
  renderer verifies the centre's orbit closes at it before trusting it,
  and the panel's Reference orbit section can detect one.

### BLA

`bla.rs` builds a bivariate linear approximation table (Zhuoran's
construction, as described by Claude Heiland-Allen): where the delta step
is locally linear, a run of 2^ℓ steps becomes one affine application.
Entries carry `f64` mantissas with a shared exponent. It applies to the
holomorphic `zᵖ + c` tier (Mandelbrot, integer Multibrot), to the Ship
family and the Tricorn, and to custom formulas where skipping is sound,
and not when the base or layer colouring accumulates or detects periods,
since skipped steps never run those updates. `ESCAPE_BLA=0` disables it
for bisection.

**Real-linear tables** (phase 4, step 9b): a fold or a conjugate makes
a step real-linear, not complex-linear -- any real 2x2 map is `P·δ +
Q·conj(δ)`, holomorphic where Q = 0. Such a table's entries carry Q's
for A and B (`BlaEntry::aq`, `bq`; 56 bytes on the GPU against 32), merge
as the maps compose (`A ← (Py·Px + Qy·conj(Qx), Py·Qx + Qy·conj(Px))`,
the operator norm `|P| + |Q|` in the radius), and the skip adds
`Q·conj(w) + Qb·conj(d0)` (`assembler::real_linear_bla`: the custom,
Ship and Tricorn tiers; the power tier's shader is as before).

**Custom formulas** (phase 4, step 9: `custom::bla_steps`,
`BlaTable::build_from_steps`): each reference step's single entry is
read from the formula's own perturbation equations, stepped in f64 from
`Z_n` (`interp::PerturbStepper`). They are free of cancellation, so a
deviation of `2^-60 |Z|` gives the linear part exactly, folds
(`diffabs`) included: `P` and `Q` from the steps along 1 and i, `Pb` and
`Qb` likewise from the pixel's offset. The radius: the residual at a
small deviation along the axes sizes the quadratic term (for `z² + c`
the power table's `2 eps |Z|`); the candidate is checked on a ring of
eight directions, with and without the view's largest pixel offset,
halving until the residual stays under `BLA_EPS` of the linear part --
which finds a fold's kink or a branch cut as well as curvature -- and
halved once more for the directions between. The built-in Ship and
Tricorn tiers take their twins' (`custom::twin_bla_steps`: the same
map). Skipping is refused where it is not sound: a branch on the
iterate, a value carried besides z or a hybrid's phases, a test other
than z's modulus under a bound (a skip could cross a convergent
formula's settle). The coefficients for a million-step orbit of the
Mandelbrot's twin take 0.53 s on the CPU.

Measured (512x384; `measure_the_custom_tier_against_the_native_one`,
`measure_real_linear_bla`):

| view | built-in per step → with BLA | twin per step → with BLA |
| --- | --- | --- |
| Mandelbrot seahorse, zoom 60, 30,000 (floatexp) | 0.80 → 0.17 s | 1.12 → 0.12 s |
| Burning Ship interior, zoom 60, 20,000 (floatexp) | 0.81 → 0.08 s | 3.44 → 0.19 s |
| Burning Ship interior, zoom 30, 20,000 | 0.15 → 0.08 s | 0.25 → 0.19 s |
| Tricorn interior, zoom 30, 20,000 | 0.14 → 0.05 s | 0.18 → 0.09 s |
| Tricorn suite view (`tricorn-deep`), escaping early | 0.13 → 0.19 s | 0.14 → 0.25 s |

A view whose pixels escape early pays the table's build, once per orbit,
and the skip search without gaining skips. Against the big-float truth
(439 sampled pixels) the deep views count as per step: the Tricorn at
zoom 28 191 built-in and 188 twin with BLA, 190 and 189 per step; the
Burning Ship at zoom 60 314 and 336 with BLA, 294 and 346 per step; the
Mandelbrot's twin 402. Both suite views cross chaotic regions, where a
skip's error (2^-24 of the linear part, the f32 coefficients' too)
decides which pixels come out right, not how many: their pictures'
speckle changed with BLA, and their baselines were regenerated
(`escape-ship-deep-floatexp`, `escape-tricorn-deep`). At the hybrid
test's walked edges the Mandelbrot's twin alone counts 374 and 393 (zoom
24 and 51).

**Backing off.** A lookup that finds nothing costs more than the step
it tried to replace, so where a view's deviations stay past the table's
radii (its orbits close to the reference's critical points) the table
made the render slower, up to twice. Each pixel counts its failed
lookups in a row, `f` (at most 5), and looks only at an index aligned
to `2^(f+1)` -- level `f`'s own alignment -- so a stretch without skips
pays a lookup every `2^f` steps or so rather than every other one, and
a success looks again at once. The count rides across chunks in the
top bits of the stored `m` (`BLA_FAILS_SHIFT`: an orbit of 2^29 entries
would need a buffer past any storage binding), so the image does not
depend on where chunks end. GPU time at 512x384
(`measure_twins_against_their_builtins`, best of three):

| view | no table | table | table, backing off |
| --- | --- | --- | --- |
| Mandelbrot built-in, zoom 30, 20,000 | 110 | 157 | 103-127 |
| Tricorn built-in, suite view zoom 28 | 83 | 163 | 81-117 |
| Lambda's twin, zoom 30 | 16 | 31 | 18 |
| Burning Ship built-in, interior zoom 30 | 120 | 6 | 6 |
| Mandelbrot built-in, zoom 60, floatexp | 652 | 25 | 24 |

Where the table skips, nothing changes.

### Nucleus finding

`nucleus.rs` finds the minibrot governing a view: the period by the ball
method (iterate a disc until it contains 0), then Newton on
`f_c^p(0) = 0` in big-float. A reference at the nucleus is periodic, so
the rebase is exact and the stored orbit is `p` entries long. The
panel's **Center on minibrot** button (`locate_minibrot`) recentres on it
with as many digits as the depth needs; it is offered for Mandelbrot and
integer Multibrot on the parameter plane, and for custom formulas with a
nucleus search (below).

**Custom formulas** (phase 4, step 9c: `lang/periodic.rs`,
`ReferenceOrbit::compute_custom_nucleus_aware`). A reference whose
iterate returns to its start after `p` passes wraps exactly: the
rebase at the orbit's end changes nothing where `Z_p = Z_0`. So a
formula's nucleus is a root of `G(c) = z_p(c) - z_0(c)` -- for
`z^2 + c` from zero the built-ins' `f_c^p(0) = 0`, for a formula
starting at its pixel `f^p(c) = c`, the same points -- found through
the formula's own derivative (`lang::derive`, `G' = dz_p - dz_0`) run
in big floats:

- **Where**: the parameter plane, a formula whose state from pass to
  pass is z alone (nothing carried, no cycle, `#pass` unread:
  `periodic::wraps_exactly`) and which has a derivative -- a Jacobian
  included (step 9d): Newton then inverts the real-linear map, `d =
  (conj(P) G - Q conj(G)) / (|P|^2 - |Q|^2)`, so the Burning Ship's and
  the Tricorn's twins find their nuclei (a period-96 one off the axis
  beside the visual suite's deep Ship view).
- **The period**: the first pass whose Newton step from the view's
  centre, `|G_n / G'_n|`, falls inside the view's radius -- a ball of
  the first order. At the deep seahorse view (zoom 30) it gives 998,
  where the built-ins' ball method says 850 and its Newton finds
  nothing. The test rides on the reference's own build
  (`periodic::SlopeWatch`, `ReferenceOrbit::custom_watch`): `G_n` from
  the big-float orbit being computed anyway, `G'_n` from the derivative
  program stepped in f64 at its iterates (`interp::DerivStepper`, every
  derivative it holds kept in units of `2^e`, as the GPU's floatexp rung
  keeps them). A period found stops the build there for Newton; none
  costs a pass of the derivative in f64 beside each big-float one. (It
  was a second orbit first, the derivative program's in big floats: at
  a Burning Ship's interior, 2 s before a 0.19 s reference.)
- **Newton** at twice the zoom's bits (the closure asked is absolute,
  and `G = h' d` with `h'` up to 2^zoom), 64 octaves past the view's
  closure limit or to the precision's floor. The root can be double: a
  formula that starts at its pixel has `G = h^2`, where Newton halves
  its distance per step; the multiplicity is read from the steps'
  ratio and the step taken `m G / G'`. The orbit in big floats, its
  slope in f64 beside it -- Newton needs the slope to f64's digits only,
  each step then gaining ~50 bits near the root -- until that stalls on
  an orbit that closes, where the derivative program runs in big floats
  for the last steps: at a double root `G' = dz_p - dz_0` cancels in
  f64. A Burning Ship's Newton that finds no root gives up in 15 ms,
  not 100.
- **The reference** at the nucleus is the formula's own, one period
  long, and is kept only if its own closure detection fires there:
  like the plain map's, a new minimum of `|Z_n - Z_0|` below the view's
  limit makes any custom reference periodic on the spot, and a hinted
  period (`reference_period`) is measured the same way.

Detection looks no further than the orbit a plain reference needs, or
Newton's budget (20,000 periods); a search that finds nothing costs its
depth in passes of the formula and its derivative, about twice a plain
pass. Measured on the Mandelbrot's twin, references for 100,000
iterations (`measure_custom_nucleus_references`):

| view | plain | nucleus-aware | built-in |
| --- | --- | --- | --- |
| seahorse, zoom 30 (escapes at 8,057) | 0.024 s | 0.034 s, period 998 | 0.013 s, plain |
| seahorse, zoom 42 | 0.30 s | 0.034 s, period 998 | 0.017 s, period 998 |
| the Misiurewicz point i, zoom 30 | 0.27 s | 0.001 s, period 25 | 0.001 s, period 23 |
| no nucleus in view (zoom 100; inside an atom off its nucleus) | 0.32 s | 0.43-0.47 s | 0.21 s |

(Measured before the search rode on the reference's build and big
floats kept their limbs inline; see "How fast the twins are", below.)

Against the big-float truth the nucleus references count as the plain
ones (`a_custom_formula_renders_from_its_nucleus`: 402 of 439 for both
at the seahorse view, zoom 42). **Detect period** finds a custom
formula's closing period along its centre's own orbit
(`custom::detect_period`).

### The orbit store

`orbit_store.rs` (desktop only) persists expensive references under
`orbit_cache/` in the app data directory, keyed on the full request
identity, with the live fixed-point state so a reloaded orbit deepens
like a fresh one. Only orbits that cost enough to rebuild are saved;
eviction is newest-first under a byte cap — **Orbit Cache (MB)** in
Settings (`SystemSettings::orbit_cache_mb`, default 1024) — and a cap of
50 files. Any parse failure or version mismatch is a miss.

A custom formula's references are stored too (the speed work after
phase 4, step 9e). The key carries the reference identity
(`custom::reference_identity`: a hash of the program, its packed
parameters, the plane and the image values it reads), and the file
carries, after the fixed-point state, each big-float machine's pass,
iterate and variables (`BigMachine::save_state`), always in the raw
array form (the compressed form replays the plain map's shadow). On load
the machines are rebuilt from the identity, which this session's compile
of the formula names, and their state restored
(`ReferenceOrbit::restore_custom`); a formula not compiled, or state
that does not fit its program, is a miss. A restored orbit deepens to
the orbit a build straight to that depth gives, entry for entry
(`a_custom_formulas_reference_goes_through_the_store`). The other
big-float families (Newton, Nova, Kaliset, Ducks) are still not stored.
Big-float entries no longer keep compression corrections, which held 52
bytes an entry of nothing.

### Where the direct path ends

The direct path's f32 error is fixed in the plane -- each step rounds at
the ulp of the iterate -- so what makes it visible is its size against
the colour step from one RENDER pixel to the next. Measured, that ratio
is `A / r`: `r` the pixel spacing in f32 ulps of the centre, `A` the
view's sensitivity, steady to a few percent across zooms and render
heights. Parameter planes measured A = 0.11 to 1.34; Julia planes 0.12 to
15, because a Julia orbit starts at the pixel, and one that passes near
the critical point shrinks the pixel's signal while the rounding stays.

`direct_limit_zoom(escape, render_lines)` therefore switches where a
render pixel spans `DIRECT_ULPS_PLANE` = 32 (Julia: `DIRECT_ULPS_JULIA`
= 256) ulps of the centre -- `A` of 2 and 16 against a target ratio of
1/16 -- held between `PERTURB_FLOOR_ZOOM` (6) and `PERTURB_MIN_ZOOM`
(14). `render_lines` is the display height times the supersample in use,
so a doubled height or supersample moves the switch an octave earlier,
and a centre past |c| = 1 one more. For a centre with |c| in [0.5, 1):

| render lines | parameter plane | Julia plane |
| --- | --- | --- |
| 180 (thumbnail) | 13.5 | 10.5 |
| 720 | 11.5 | 8.5 |
| 1080 | 10.9 | 7.9 |
| 2160 (4K, or 1080 at 2x) | 9.9 | 6.9 |
| 4320 (4K at 2x) | 8.9 | 6 (floor) |

Every tier agrees with direct at those depths (`perturbed_agrees_with_direct_at_moderate_zoom`,
`the_non_escaping_tiers_are_accurate_from_the_floor`), and costs two to
three times the direct path's time there. The calibration is
`diag_tier_sensitivity`. Until 2026-10-13 the switch sat at zoom 14 for
every size, so a tall render or a Julia view showed grain before it --
Phoenix's dynamical plane visibly from zoom 9 at 720 lines -- and
Kaliset waited until 24, for a reason that turned out to be a bug in its
scaled rung's seed (`w_init`).

The panel's notes about the path read the viewport's render height
(`viewport_render_lines`), which the app publishes when it sizes the
viewport; a thumbnail or an export decides at its own size.

### How fast the twins are

A twin against its built-in, each render from cold at 512x384 with BLA
where the tier has it (`measure_twins_against_their_builtins`, the best
of three; a single run swings by half), in ms -- the whole render, then
the GPU's part:

| view | built-in | twin | GPU: built-in | twin |
| --- | --- | --- | --- | --- |
| Mandelbrot, seahorse zoom 42, 8,000 | 28 | 36 | 19 | 24 |
| Mandelbrot, seahorse zoom 60, 30,000, floatexp | 164 | 106 | 24 | 30 |
| Mandelbrot, zoom 30, 20,000 | 128 | 129 | 127 | 128 |
| Multibrot power 4, zoom 30 | 8 | 11 | 5 | 6 |
| Tricorn, suite view zoom 28 | 154 | 141 | 117 | 119 |
| Tricorn interior, zoom 30 | 46 | 64 | 6 | 7 |
| Burning Ship, suite view zoom 60, floatexp | 37 | 77 | 34 | 62 |
| Burning Ship interior, zoom 30 | 71 | 151 | 6 | 8 |
| Lambda, zoom 30 | 20 | 26 | 16 | 18 |
| Phoenix, zoom 48, floatexp | 16 | 19 | 15 | 18 |
| Manowar, zoom 26, floatexp | 127 | 116 | 119 | 111 |

What made the references cheap: big floats keep up to eight limbs
inline (`bigfloat::Limbs`, a `SmallVec`: an operation or a variable's
read was a heap allocation, and a pass of the Mandelbrot's twin took 2.4
us, now about 1); a whole real power (the Multibrot's `z ** p.power`) is
taken by repeated squaring, not `exp(w log z)` (79 us a pass, now 1.6);
and the nucleus search rides on the reference's build (above). What
made the GPU's part cheap: on the floatexp rung a value the program
types real runs on its real halves (`cfe2_mul_r` and kin: a complex
product of two reals did four double-float products for one), a
constant factor scales the mantissa or shifts the exponent
(`cfe2_scale`, `cfe2_ldexp`), and a complex of two reals is aligned, not
summed (`cfe2_pair_r`) -- the Ship's twin at zoom 60 went from 126 ms of
GPU time to 62; the product and power rules above; and the table's
backing off.

What remains:
- **The Ship at floatexp**, about twice: the built-in's deep Ship
  carries a single f32 mantissa (`SFe`), the generated equations a
  double-float one (`CFe2`), more precise by design.
- **The references' builds**, where no nucleus serves: a big-float pass
  costs 1 to 2.4 us (about three times its arithmetic, a 3-limb
  multiply-add being 80 ns), the built-ins' fixed-point steps 0.8; and
  the period search's f64 derivative pass rides on each, 0.45 us (the
  Ship's Jacobian, 2). The Ship's interior view at zoom 30: 51 ms of
  big floats and 43 of watching against the built-in's 16, which
  watches nothing.
- **A custom BLA table's build** from the equations: 26 ms against the
  power table's 6 for the Mandelbrot at zoom 60.

### Damping and the biomorph on custom formulas

A program owns its step and its escape test, so the built-ins' template
switches (`assembler::step_lines`' damping, `escape_metric`'s biomorph)
cannot reach it. Instead the formula compiles again with them in it,
as rewrites of the program (`lang/modifiers.rs`; plan, phase 4, step
9e):

- **Damping**: the loop keeps z before the pass and blends after it,
  `z = z_before + alpha (z - z_before)`, alpha a complex parameter the
  panel's Damping drives. A formula carrying the previous z keeps the
  damped one, as the built-ins' history does. Equations a formula wrote
  for itself describe the undamped map, so they go, and the loop's are
  derived.
- **Biomorph**: the escape test reads `real(z)^2` or `imag(z)^2` where
  it read its measure of z -- `|z|`, or the exponential and trig
  families' `real(z)` and `abs(imag(z))` -- the built-ins' metric
  replaced, as theirs replaces any. A test without one has no biomorph,
  and the panel offers none (`custom::biomorph_ok`).

`custom::resolve` gives the renderer the config to render: the variant's
name (a hash of the base's and the switches), the damping bound to its
parameter, the panel's switches off -- the program carries them, so
nothing downstream applies them twice. The renderer resolves at its
entry points (`render`, `perturb_tier`, `wants_perturbation`,
`usable_depth`, `derivative_gap`). Everything derived from a program
then comes from the variant: the derivative (a damped built-in's is
stepped undamped, so its distance estimate is off; the variant's is
right), the perturbation equations, BLA, the nucleus search. So a
damped or biomorph custom formula deep-zooms. Moving alpha recompiles
nothing; switching damping or the biomorph on compiles the variant once.
The twins render as their built-ins with either on (0 to 0.16% of pixels
differ), and their variants perturbed count 438 or 439 of 439 sampled
pixels right against big floats on both rungs, with BLA and without
(`custom_formulas_take_damping_and_the_biomorph`). Center on minibrot is
not offered damped: the nucleus search finds where the orbit returns to
its start, a minibrot's centre only where the start is the map's
critical point, and damping moves that (to `(alpha - 1) / 2 alpha` for
`z^2 + c`). The reference still relocates there, where it wraps
exactly.

