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

Formulas and colourings can also be written as text, in **Ultra Fractal's
formula language**, which accepts Fractint's shape too; `escape::lang`
compiles them to WGSL. The plan, its phases and what is left are in
[escape-formula-language.md](../projects/escape-formula-language.md).

**Where it lives.** `src/escape/lang/`: `lexer.rs`, `parser.rs` (a file
into entries and sections), `check.rs` (names and types, UF's rules,
into the typed `ir.rs`), `interp.rs` (the program in f64: the reference
the WGSL is tested against), `wgsl.rs` (the shader), `derive.rs` (a
formula's derivative), `switch.rs` (a `switch:` pair as one program),
`builtins.rs` (the function table).
`src/escape/custom.rs` registers a compiled entry.

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
what belongs to a pixel (`#pixel`, `#z`, `#x`, `#random` ...). Without
arrays it holds single values: a formula's go into the uniform's `fdata`
(256 floats), a colouring's share its 64-float parameter block. A
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
rebases only where the orbit ends. No BLA and no derivative on this
tier, so a deep custom view iterates every step.
`interp::run_perturbed` is the f64 model of the path.

**Derived equations.** A formula without `perturbloop:` -- most of
UF's, every Fractint one -- gets equations derived from its own loop
when it can (`lang/delta.rs`, at compile time in
`custom::compile_formula`, which keeps the formula as written if the
derived WGSL fails validation). Each value becomes a pair: its value
at the reference (from `#z` and the reference point `#pixel -
#dpixel`) and the pixel's deviation (from `#dz` and `#dpixel`), by
rules free of the cancellation `f(Z + d) - f(Z)` suffers: sums,
products, the quotient form `(da - (A/B) db) / (B + db)`, binomial
integer powers, `sqr`, `recip`, `|a|`, `conj`/`flip`/`real`/`imag`,
and `abs` by `diffabs` (`|A + d| - |A|` by sign case, per part for a
complex: `BinOp::DiffAbs`, the compiler's own operator; `uf_diffabs`,
`cfe2_diffabs`). The init's variables are constants per pixel (their
values substituted in order). A branch derives when its conditions are
the same for every pixel (a parameter's choice, a constant), so the
reference and the pixel take the same arm; z written in steps reads
its own reference value after each. What does not derive: a
transcendental function, a real or complex power, a user function of
the iterate, a branch on it, a loop inside the loop, a branch in the
init, state carried between iterations besides z. Such a formula stays
on the direct path, as before.

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
`a_hybrid_renders_as_its_built_in`). Hybrids render on the direct path
for now (deep zoom for them is the plan's step 5b), and distance
estimation does not reach those that fold: the derivative pass is
complex-analytic.

**Not yet:** UF's classes and plug-ins (`import`), arrays, colour values
and direct colouring.

**Measured against UF's public formula library** (its full download,
read for measurement and never bundled; `measure_a_uf_corpus`, an
ignored test pointed at a folder by `UF_CORPUS`): 9,250 of its 9,831
formula entries compile (94%), and 423 of its 1,209 colourings. The
WGSL of every one that compiles validates, its derivative's too. What
stops the rest: classes (`import`, 325 entries), arrays (296), the
colouring state cap (335 colourings at 4 floats; it is 32 since, which
admits 96% of the colourings that reach it), parameters past 64 floats
(about 200), colour functions (46). Deep zoom: 2 of the formulas carry
their own perturbation equations and 308 more get derived ones; the
commonest blockers for the rest are a test that reads a variable the
loop changes, a user function, and a branch in the init.

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
  either plane. A user function's derivative is chosen with it (zero for
  a choice with none: `abs`, `conj`, `real` ...). A formula whose
  iterate passes through a fold (`abs`, `conj`, `real`, `imag`, `|x|`, a
  real variable that follows the pixel) has none, and the panel reports
  the gap as it does for Burning Ship.
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
off, biomorph is off and the view is past the direct path's limit at the
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
Entries carry `f64` mantissas with a shared exponent. It applies only to
the holomorphic `zᵖ + c` tier (Mandelbrot, integer Multibrot), and not
when the base or layer colouring accumulates or detects periods, since
skipped steps never run those updates. `ESCAPE_BLA=0` disables it for
bisection.

### Nucleus finding

`nucleus.rs` finds the minibrot governing a view: the period by the ball
method (iterate a disc until it contains 0), then Newton on
`f_c^p(0) = 0` in big-float. A reference at the nucleus is periodic, so
the rebase is exact and the stored orbit is `p` entries long. The
panel's **Center on minibrot** button (`locate_minibrot`) recentres on it
with as many digits as the depth needs; it is offered for Mandelbrot and
integer Multibrot on the parameter plane.

### The orbit store

`orbit_store.rs` (desktop only) persists expensive references under
`orbit_cache/` in the app data directory, keyed on the full request
identity, with the live fixed-point state so a reloaded orbit deepens
like a fresh one. Only orbits that cost enough to rebuild are saved;
eviction is newest-first under a byte cap — **Orbit Cache (MB)** in
Settings (`SystemSettings::orbit_cache_mb`, default 1024) — and a cap of
50 files. Any parse failure or version mismatch is a miss.

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

### The derivative under perturbation

Both rungs carry the derivative for every single-term tier, stored per
**render pixel** (`dz/dc` times the pixel size) so it stays a moderate
number at any depth. `distance_estimate`, `normal_map` and analytic
relief therefore work at depth on formulas that define a derivative.
Design record: [derivative-under-perturbation.md](../archive/escape-time/derivative-under-perturbation.md).

### Which formulas perturb

Common conditions: past the view's [direct limit](#where-the-direct-path-ends), no damping, biomorph off.

| formula | tier | condition | BLA |
| --- | --- | --- | --- |
| `mandelbrot` | `Power(2)` | — | yes |
| `multibrot` | `Power(p)` | integer `p` 2–12 | yes |
| `tricorn` | `Tricorn(p)` | integer `p` 2–12 | no |
| `burning_ship` | `Ship(v)` | all six variants | no |
| `phoenix` | `Phoenix` | — | no |
| `manowar` | `Manowar` | floatexp rung at every depth | no |
| `lambda` | `Lambda` | — | no |
| `feather` | `Feather(p)` | integer `p` 2–8 | no |
| `mcmullen` | `McMullen(n, m)` | Julia mode; integer `n` 2–8, `m` 1–8 | no |
| `magnet` | `Magnet(v)` | both variants | no |
| `newton` | `Newton` | integer `p` 2–12; the three polynomial functions; `rootfinder_has_delta` (Chebyshev declines the two non-unity ones) | no |
| `nova` | `Nova(p)` | integer `p` 2–12 | no |
| `kaliset` | `Kaliset` | both sign branches | no |
| `ducks` | `Ducks(v)` | variants 0 and 4 | no |

`EscapeRenderer::perturb_tier` is the authority. Everything else —
the transcendental formulas, Spider, Barnsley, Cactus, Collatz,
Littlewood, Origami, Lattès, Novaretti, the fields and mode D — renders
the direct path at every depth. The per-tier error measurements are in
[escape-time-completion.md](../archive/escape-time/escape-time-completion.md);
the theory in [pertubation-theory.md](../archive/escape-time/pertubation-theory.md).

---

## Colouring and appearance

Everything here acts after the iteration, so it is a recolour, not a
re-render, unless noted.

| setting | config | what it does |
| --- | --- | --- |
| **Palette Mapping** | `palette_map` | a transfer curve `k·f(v/k)` on the value before the wrap (`TransferCurve`, pivot `k`), a curve on the position within each cycle (`PaletteCurve`), and a **stepped palette** (each stop a flat band) |
| **Auto Contrast** | `contrast` | fits the value range to the palette from a 96×72 probe of the settled frame: `AutoRange`, `Flatten` (subtract the fitted plane first — the fix for non-escaping fields that become a plane at depth), `Equalize` (rank-equalised, a 256-entry CDF), with clip, strength and palette turns. A terrain pins one fit across its sections |
| **Texture Layer** | `layer` | a second mode-A colouring blended into the first before the palette lookup (`Screen`, `Multiply`, `Add`, `Overlay`, `Mix`, at a weight), subject to `layer_refusal`. Its needs combine with the base's: either can compile in the accumulator, period detection or the derivative |
| **Relief Shading** | `shading` | a lit surface composited over the finished colour (below) |
| **Antialiasing**, **Downsample** | `supersample`, `downsample` | see [Antialiasing](#antialiasing) |
| **Biomorph** | `biomorph` | the escape test reads `(Re z)²` or `(Im z)²`; re-iterates; turns perturbation off |
| **Damping α** | `damping_re`, `damping_im` | Mann iteration `z ← (1−α)z + α·f(z)` with complex α, compiled in only when α ≠ 1; re-iterates; turns perturbation off |
| **Texture** | `texture`, `texture_overlay` | a simulation texture (below) |

**Relief shading** is deliberately not a colouring: a colouring returns
one scalar and replaces the others, while relief runs after the palette
lookup on the finished RGB, so it composes with every colouring. The
surface is a height field — by default the colouring's own value,
finite-differenced — lit by a light at `light_angle` degrees
counter-clockwise from east (135 by default, the upper left; a file
without the key means 315). The slope can come from (`ShadingField`):

- `Smooth` — the value before the wrap: terrain;
- `Banded` — the wrapped position: every band edge a step;
- `Layer` — the texture layer's value;
- `Analytic` — the distance estimate's slope from the derivative;
- `Offset` — the value at orbits a small step beside the pixel;
- `Embossed` — Ultra Fractal's bevelled contours from two such orbits;
- `Creases` — a folding formula's paper (below).

`Analytic`, `Offset`, `Embossed` and `Creases` re-iterate, and the last
three run on the direct path only.

**Creases** (`FormulaFeature::Creases`; Origami's) draws the folds as
paper. Each fold is a crease, and the paper rises or falls in a straight
line with the distance from it, the same slope on both sides. Fold `i`'s
crease weighs `decay^i` (`CreaseSettings`: **Decay**, 0.6 by default),
so the first is the main crease and each later one is finer; the
iteration count is the crease count, one per fold, at most 64. The
**Pattern** sets each fold's direction: Valley (every flap toward the
viewer), Alternate, or Random from the line seed. **Flip** swaps
mountain and valley. Each crease keeps its fold's direction along its
whole length, on every layer, which keeps the sheet in one piece. This is
how gently folded paper bends. Reversing a crease on a face-down layer
(the flat-folded valley/mountain pattern) tears the height wherever it
crosses an earlier crease. The slope is exact, not
differenced: every fold reflects, so the folded point is `M p0 + t` for
an orthogonal `M`, and the gradient of a distance from a line is
`Mᵀ n`. `formula_creases` returns the height and gradient. The
iterate pass stores the height in the relief channel, and stores a
tenth of `−∇h`, turned into the screen frame, as the slope (at the
default relief Height of 10, the first crease tilts 45°). Every pixel
gets one, the interior's too. The recolour shortcut cannot recompute
them, so a crease render always iterates
(`EscapeConfig::wants_creases`); Origami's 64 folds are cheap. The
settings reach the shader through the uniform's `crease` row. The
model was prototyped in `scripts/escape_prototypes/origami_creases.py`
([plan](../projects/origami-creases.md)).

Lighting is `Tilt` (the signed lean toward the light) or
`Lambert` (with an elevation); shadow and highlight each have a colour,
strength and blend (`ShadingBlend`, blended in a perceptual space so the
two strengths read alike); `softness` is a Gaussian on the height, in
display pixels; a slope stencil, a height curve and a micro-texture
complete it. The simulation's relief colouring uses the same lighting
(see [SIMULATION.md](SIMULATION.md)).

**The surface texture** (`ShadingTexture`: Grain, Paper, Rock or the
simulation texture) is shared with the 3D terrain
([surface-textures.md](../projects/surface-textures.md); on a terrain it
is bumped into the surface, [TERRAIN.md](TERRAIN.md)). In 2D it tilts
the relief's slope, in display pixels: it is fixed to the screen until
that plan's step 3. **Where** (`TextureWhere`: everywhere, the steep
faces, the flats) masks it by the relief's tilt against
`texture_angle` ± `texture_angle_soft`. Rock is ridged gradient noise,
four octaves.

**Textures** are simulation recipes, not image files: an `EscapeTexture`
embeds a whole Simulation-mode config, so the file that uses it is
self-contained, and the image is generated (or read from a PNG cache) by
`crate::textures`. They come from the shipped presets in
`assets/textures/` and from Simulation mode's **Save as texture**, and
are picked in the **Textures** panel. Three things read one
(`EscapeConfig::uses_texture`): the **overlay** (`texture_overlay`,
Kalles Fraktaler's texture warped by the count's slope and mixed into the
colour), the relief's `Simulation` micro-texture, and the `image_trap`
colouring. Design record: [sim-textures.md](../archive/projects/sim-textures.md).

---

## The camera lens

A lens is a **variation applied to the screen offset**: each pixel's
normalised offset `n` becomes `L(n)` before the view scale, rotation and
centre, so the fractal is untouched and only which point each pixel
samples changes.

```
pixel -> offset n -> L(n) -> × span -> rotate -> + centre -> the formula
```

That is the inverse direction of a flame's final transform, so a lens
need not be invertible (a many-to-one lens shows a region twice, and
every pixel still gets one sample), and a variation's lens picture is not
its final-transform picture.

- **Units:** half-height one — `n.y` in [−1, 1], `n.x` in [−aspect,
  aspect] — the scale variations are written for. Each call site
  converts into it and back (`lens_apply_span`, `lens_apply_pixels` for
  the perturbed path, `lens_apply_uv` for mode D).
- **Amount** (`lens_amount`, ±5, `LENS_AMOUNT_LIMIT`) blends
  `mix(n, L(n), amount)`: past 1 it overshoots, below 0 it runs the
  displacement backwards, roughly the inverse. The slider is a camera
  control, so positive bulges the middle out; for the radial lenses in
  the recommended set that is the variation run backwards, so
  `lens_flame` hands the shader the negated value. It rides in the lens
  transform's weight, so dragging it does not recompile.
- **No variation code is duplicated:** `lens_source` builds a
  one-transform flame and has `ShaderBuilder::build_layer_map_at` emit
  the variation, its helper libraries and `flame_map` at bind group 2
  (`LENS_GROUP`); `LENS_GLUE` adds `esc_lens`.
- **The picker** offers `LENS_RECOMMENDED` (26 that read as a camera)
  first, then the rest of `LENS_USABLE` (396 variations measured usable
  by `variation_probe lens`, `src/probe/lens.rs`), with a Show all box
  for the rest.

The lens applies to the escape plane (the direct, perturbed and field
kernels) and to mode D, planar and solid. It does not yet apply to the
terrains, simulations or flames: [camera-lens-everywhere.md](../projects/camera-lens-everywhere.md)
is the plan. Design record: [escape-camera-lens.md](../archive/escape-time/escape-camera-lens.md).

---

## 3D: solids and terrains

Two things in escape mode are three-dimensional, and both are covered in
depth elsewhere.

- **Mode D solids** (`ifs_flame_3d`, `quaternion_julia_solid`) march a
  ray through a distance field on a camera of their own: a target as
  exact decimal strings (`cam_target_x/y/z`, empty for the attractor's
  centre), `cam_pitch`, `cam_yaw`, `cam_bank`, `cam_fov`, with the
  screen roll in `rotation` and the distance from `zoom_log2`.
  `solid_tier` chooses the lit walk (`Lit`, the default for a solid) or
  the path tracer (`renderer/solid_path.rs`).
- **The escape terrain** (`terrain`, Cargo feature `terrain`) draws the
  2D picture as a lit height field to the horizon: `footprint.rs` renders
  the ground in sections, each an ordinary `EscapeRenderer` render;
  `terrain.rs` walks it; `terrain_tiers.rs` picks the tier;
  `path_core.rs` is the path tracer the terrain and the solids share.
  `terrain_active()` is true when the terrain is on and the formula is
  not a solid.

Both share the View panel's camera, lighting, material, fog and path
tracer settings. See [TERRAIN.md](TERRAIN.md) and [CAMERA.md](CAMERA.md).

---

## The config

`EscapeConfig` ([src/config/escape.rs](../../src/config/escape.rs)) sits
in `FractalConfig` behind skip-if-default, and so does each sub-block,
so a flame that never touched escape mode serialises as before.

| group | fields |
| --- | --- |
| formula | `formula`, `formula_params`, `julia`, `julia_re`, `julia_im` |
| view | `center_re`, `center_im` (strings), `zoom_log2`, `rotation` |
| iteration | `max_iter` (256), `bailout` (1e4 new, 4 when absent), `damping_re`, `damping_im`, `biomorph`, `reference_period` |
| colour | `coloring`, `coloring_params`, `palette_map`, `contrast`, `layer`, `shading`, `texture`, `texture_overlay` |
| antialiasing | `supersample` (1), `downsample` |
| lens | `lens`, `lens_params`, `lens_amount` (1) |
| 3D | `cam_target_x/y/z`, `cam_pitch`, `cam_yaw`, `cam_bank`, `cam_fov`, `solid_tier`, `terrain`, `path` |

Defaults: formula `mandelbrot`, colouring `smooth`, centre `-0.5`, `0`.

Every field but `texture` has a `ConfigPath::Escape*` — `EscapeFormula`,
`EscapeZoomLog2`, `EscapeCenterRe`, the keyed `EscapeFormulaParam`,
`EscapeColoringParam`, `EscapeLayerParam` and `EscapeLensParam`, and the
shading, contrast, palette-map, terrain and path families — whose string
keys read `Escape.Formula`, `Escape.ZoomLog2`, `Escape.FormulaParam.<name>`.
(The Textures panel sets `texture` as one structural undo step.) They all
route to `UpdateType::EscapeRerender`, which sets the app's
`escape_dirty`; there is no flame-style reset/accumulate distinction to
encode.

What an animation track can drive is decided by
`delta::json_to_config_value`: the numeric paths — zoom, rotation, the
Julia seed, bailout, damping, `max_iter`, every parameter, the camera
angles and most shading, contrast, terrain and path-tracer values — and
the solid camera's target, whose strings are rebuilt from an `f64`. The
formula and colouring selectors, the biomorph mode, the reference period
and the planar centre strings are not animatable: a planar zoom runs far
past what an `f64` centre could hold.

### Presets

A formula's identity is more than its step: Origami wants zoom −1 at 32
iterations with a position map, the Mandelbrot smooth colouring at 512.
An `EscapePreset` carries the centre, zoom, `max_iter`, colouring,
optional Julia seed, both parameter sets and an optional bailout. The
first preset of a formula or field is its default, applied as one undo
step when the user switches to it, so the previous formula's view does
not follow.

`src/escape/presets.rs` is **generated** by `scripts/gen_escape_presets.py`
from the visual-regression configs in `tests/visual/configs/escape/`
(chosen in the script's `SELECTION`), then committed: each preset is a
view the visual suite already renders and compares. It holds 53 presets
covering all 26 formulas and the three fields; mode D defs have none.

---

## Integration

| piece | where |
| --- | --- |
| **Dispatch** | `render_with` → `render_escape` (or `render_escape_terrain`) in `src/renderer/render.rs`, so CLI export, thumbnails, video and the gallery modules inherit it. `prepare_escape` hands mode D its analysed flame and coarse pass, and a texture its image, before the first chunk. A caller passing `RenderEngines` (video export) keeps one renderer and its warm reference across frames |
| **The app** | `App::escape_renderer`, created lazily on the first escape frame with `progressive` on (desktop), and `escape_dirty`, set by escape edits, palette changes and loads and cleared when a render reports final. The frame loop re-analyses a mode-D flame every frame and marks dirty only when `set_ifs` reports a change. Held instead by `escape_terrain` while a terrain is drawn |
| **Animation** | playback advances only on a settled frame (`!escape_dirty`), so no preview frame becomes an animation frame |
| **Panels** | the **Escape Fractal** panel (`src/ui/escape_panel.rs`: preset, formula, view, iteration and colouring controls, then collapsible sections for Palette Mapping, Texture Layer, Auto Contrast, Relief Shading, Camera Lens, 3D Terrain, Reference orbit and Diagnostics) and the **Textures** panel (`src/ui/textures_panel.rs`). The **Escape Time** workspace puts the Escape panel where Standard puts Transforms. Every control writes through `ConfigManager` with an `Escape*` path |
| **Visibility** | `src/ui/visibility.rs`: flame-only panels and chaos-game controls are hidden or greyed, the tone-map mode is Linear-only, the orbit cache setting is shown only in Escape, and lighting follows `Solid::of` (a solid formula or a terrain), not the mode |
| **Scripting** | the `escape` object; touching it switches the config to escape and applies the same entry tone mapping. See [SCRIPTING.md](SCRIPTING.md) |
| **Online** | `ApiRenderMode::Escape` (`"escape"`): stored as flames on the same endpoints, told apart by `render_mode`, so `transform_count` is 0 and `variation_names` empty. See [ONLINE.md](ONLINE.md) |
| **Cargo** | `engine-escape` gates `src/escape`; without it an escape config still parses and `render_with` returns `RenderError::EngineMissing`. `terrain` implies `engine-escape`. `wasm/escape` builds `engine-escape` without `engine-flame`, so its built-in variation catalogue — which the lens and the Flame Attractor draw on — is `linear` alone, and a lens naming any other variation is inactive there; it also builds without `terrain`, rendering a terrain config as its 2D picture |
| **Export** | the CLI's `--supersample` doubles an escape picture's own antialiasing; see [EXPORT.md](EXPORT.md) for memory during exports and the video loop |

`.flame` XML neither reads nor writes escape configs; they are `.fflame`
only.

---

## Platform notes

21 escape-time visual renders differ between macOS and Windows (listed
in [CLAUDE.md](../../CLAUDE.md), *Known Issues*). Two real causes found
on the way — the double-float fold and trig range reduction — were wrong
on both platforms and are fixed; reassociation and ulp amplification
were measured and ruled out. The residual is unexplained and parked:
perceptually the renders are indistinguishable. The measurements and the
next leads — `normalize`/`inversesqrt`, the `clamp(dot(...))`
discontinuity, `pow` in the output conversion, `fract` and palette
texture filtering — are in the design record's macOS sections (from
*The macOS visual divergence*, 2026-09-02).

---

## Testing

- **Visual corpus**: `tests/visual/configs/escape/` (119 configs —
  every formula, the perturbation thresholds and deep zooms, the
  colourings, relief, layers, textures, terrains, mode-D solids),
  rendered and compared against baselines with a tolerance by
  `tests/visual/run_tests.py` (see [TESTING-GUIDE.md](../TESTING-GUIDE.md)).
  Presets are generated from it.
- **Shader tests** (CPU, in `assembler.rs`): every combination
  validates; fast-math lints; a lens-free shader is byte-identical to
  one built before lenses (`a_shader_without_a_lens_is_byte_identical`);
  every variation compiles as a lens.
- **Registry tests** (`mod.rs`, `formulas.rs`): unique names, unknown-name
  fallback, labelled choices, `DynamicalOnly` and the iteration controls
  checked against the assembled WGSL (`dynamical_only_matches_the_shader`,
  `iteration_controls_match_the_assembled_shader`).
- **GPU tests** (`src/escape/app_repro_test.rs`, `#[ignore]`d — run them
  deliberately): the app's frame sequence reproduced headless,
  `perturbed_agrees_with_direct_at_moderate_zoom`,
  `recolor_cache_is_invisible_and_invalidates_correctly`, chunk invariance
  (`batched_chunks_render_the_same_image`,
  `fixed_chunk_renders_the_same_image_as_adaptive`), and
  `every_preset_renders_a_picture`, the only check that every shipped
  preset draws something.
- **Probes**: `gpu_bignum_probe.rs` measures a GPU big-number multiply
  against the CPU reference cost, the go/no-go of
  [escape-ntt-reference.md](../archive/escape-time/escape-ntt-reference.md);
  `variation_probe lens` (`src/probe/lens.rs`) classifies every
  variation as a lens.
- **Diagnostic switches**: `ESCAPE_CHUNK_MS` and `ESCAPE_CHUNK_BATCH`
  force chunk sizes (the image must not change), `ESCAPE_BLA=0` turns
  BLA off.

---

## Adding a formula or a colouring

1. Write a `static FormulaDef` in `formulas.rs` (or a `ColoringDef` in
   `colorings.rs`) with inline WGSL, citing its source. Prefix helper
   functions with its name. Declare the features the WGSL needs; the
   flag tests read the WGSL.
2. Append it to `FORMULAS` or `COLORINGS` in `mod.rs` (**append-only**).
3. Give a formula a derivative if it has one, `escape_degree` if it is
   polynomial at infinity, and a `wgsl_param_seed` at its critical point.
4. Add a visual config under `tests/visual/configs/escape/`, list it in
   `scripts/gen_escape_presets.py`, regenerate `presets.rs`, and run
   `every_preset_renders_a_picture` on a GPU.
5. A perturbation tier is separate work: a delta form for both rungs,
   an arm in `perturb_tier`, and a measurement against exact orbits.
6. A parameter that multiplies the work -- a walk's depth, a beam, a
   march's steps -- goes in `SLOW_PARAMS` in `mod.rs`, so the panel
   commits it when a drag ends instead of overshooting (UI.md, *Slow
   settings*).

---

## Gotchas

**A raw-metric formula wants a raw bailout.** The exponential and trig
families test `Re z` or `|Im z|`, not a square: a bailout of 1e4 carried
over from a polynomial formula overflows `exp` long before it trips. The
presets carry their own.

**Non-escaping statistics go flat at depth.** An orbit average is a
smooth function of `c`; in a small enough window it is a plane, which a
cyclic palette shows as parallel bands. That is a colouring problem, not
a precision one: Auto Contrast's Flatten is the answer.

**A deep view is the direct path's mush when it does not perturb.**
Damping, biomorph, or a formula without a tier all stop resolving past
zoom 14; the panel's depth line says which case applies.
