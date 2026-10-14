# Escape-time formula language: Ultra Fractal and Fractint formulas and colourings

Status: **planned 2026-10-13; phase 1 in progress** on `escape-expansion`:
steps 1-3 (front end, interpreter, WGSL back end) done in
`src/escape/lang/`; step 4 (engine integration) next.

Escape-time formulas and colourings written as text, compiled in Rust to
WGSL, so their number is unbounded: typed in by the user, read from
Ultra Fractal and Fractint formula files, and later downloaded from the
API. This is the plan being followed; it is updated as the work lands.

## Decisions (2026-10-13)

1. **The language is Ultra Fractal's, a subset of it.** UF's own design
   accepts Fractint's shape (an empty label starts the loop, the last
   loop statement is the bailout, `pixel` / `p1..p6` / `fn1..fn4` /
   `maxit` / `rand` work without `#` or `@`), so one parser reads both
   kinds of file. Fractint support is that shape plus a list of
   numeric differences (phase 2). UF classes (`.ulb`, `import`,
   `common.ulb`, objects, plug-in parameters) are out of scope.
2. **Colourings are in phase 1**, with formulas: the same compiler.
3. **A config carries its formula and colouring source**, so a `.fflame`,
   a PNG or an online upload renders anywhere without a library entry
   or the API.
4. **Deep zoom waits for phase 3.** Custom formulas render on the direct
   path until then (the f32 limit, roughly zoom 14). Custom colourings
   work at every depth from phase 1, because the perturbed path hands
   every iteration's full `z` to the colouring.
5. **A colouring's per-pixel state is capped at a fixed size for now.**
   The engine's accumulator is one `vec4` (4 floats), shared by every
   built-in colouring, and it is what the perturbed path and the recolour
   records keep per pixel (`IterState`, `IterResult`). Phase 1 caps
   custom colourings at those 4 floats; phase 2 measures how many real UF
   colourings need more before widening it.

## Why compile, not paste shader code

A compiled formula gives us everything a built-in carries by hand: a
CPU evaluator, a derivative (phase 2), a reference orbit and a delta
form (phase 3), and generated WGSL that obeys the cross-platform rules
(`ff_atan2`, no self-division, no subnormal literals; CLAUDE.md, Metal).
Pasted WGSL is a black box on all of those, and a downloaded one could
loop forever on the GPU.

## The language (reference: the UF online help, read 2026-10-13)

Sources: [Writing fractal formulas](https://www.ultrafractal.com/help/writing/formulas/fractalformulas.html),
[coloring algorithms](https://www.ultrafractal.com/help/writing/formulas/coloringalgorithms.html),
[perturbation equations](https://www.ultrafractal.com/help/writing/formulas/perturbationequations.html),
the Language basics and Reference pages under
[toc-18](https://www.ultrafractal.com/help/toc-18.html), and Fractint's
formula parser, `common/parser.c` in the Debian xfractint 20.4.10 sources.

**Files and entries.** `.ufm` (formulas), `.ucl` (colourings), `.frm`
(Fractint formulas). A file holds any number of entries:
`Identifier[(setting)] { ... }`. `;` comments to end of line; an entry
named `comment` is skipped. Identifiers are case-insensitive. The
parenthesised setting is symmetry for a formula (ignored: a rendering
shortcut) and `INSIDE` / `OUTSIDE` for a colouring.

**Formula sections, in order:** `global:` (once per image), `init:`
(once per pixel), `loop:` (once per iteration), `bailout:` (one boolean;
the loop repeats while it is true, and runs at least once),
`perturbinit:` / `perturbloop:` (phase 3), `default:` (settings and
parameter blocks), `switch:` (the Mandelbrot / Julia pair, phase 2). An
entry without a label starts in `init:`. An empty label `:` starts the
loop, and the loop's last statement is then the bailout (Fractint's
shape). `builtin:` is refused.

**Colouring sections:** `global:`, `init:`, `loop:` (runs after the
formula's loop, each iteration, reading `#z`), `final:` (writes `#index`,
or sets `#solid`), `default:`. An entry without a label is all `final:`.

**Statements.** Assignment (an expression: `a = b = 1` works),
declarations `[bool|int|float|complex] name [= expr]` (an undeclared
variable is created by its first assignment and is **complex**),
`if / elseif / else / endif`, `while / endwhile`, `repeat / until`.
Fractint's `if(...)` with parentheses and commas between statements on
one line parse too.

**Types.** `bool`, `int`, `float`, `complex`; `color` is phase 4 (direct
colouring). Implicit conversion widens only (int to float to complex); a
narrowing assignment is an error; `/` always yields a float or complex.
Bool converts to and from numbers only in Fractint files (UF warns).

**Operators**, high to low: `|x|` (squared modulus, parenthetic),
unary `!` and `-`, `^`, `* / %`, `+ -`, comparisons, `&& ||`, `=`
(right-associative); the rest left-associative. UF's operator page says
negation has the higher precedence than power, so `-x^2` is `(-x)^2`
(pinned by `precedence_follows_uf_s_table`). Fractint's `parser.c` gives
both precedence 2: phase 2 checks what it does with `-x^2`.

**Complex comparisons** compare real parts (Fractint; to verify for UF).

**Predefined symbols.** Read: `#pixel`, `#z` (written by formulas only),
`#numiter`, `#maxiter`, `#pi`, `#e`, `#random`, `#center`, `#magn`,
`#x`, `#y`, `#width`, `#height`, `#screenpixel`. Written by colourings:
`#index`, `#solid`. `#numiter`, `#index` and `#solid` belong to a
colouring's `final:` (UF reads `#numiter` there, and only there do the
templates have the orbit's summary). Phase 3: `#dz`, `#dpixel`. Fractint names map onto
these (`pixel`, `z`, `maxit`, `rand`, `pi`, `e`); Fractint's own
`LastSqr`, `whitesq`, `scrnpix`, `scrnmax`, `ismand`, `center`,
`magxmag`, `rotskew` are phase 2.

**Functions.** UF's list, all complex-capable where it makes sense:
arithmetic `sqr sqrt log exp abs cabs recip ident zero`; trigonometric
`sin asin sinh asinh cos acos cosh acosh tan atan atan2 tanh atanh cotan
cotanh`; conversion `round trunc ceil floor real imag`; miscellaneous
`conj flip isInf isNaN`. Fractint adds `cosxx` (cos with the imaginary
part negated: Fractint's pre-v16 bug, kept on purpose) and `srand`.
`abs` is component-wise, `cabs` the true modulus. Not supported: the
colour functions (phase 4), `length` / `setLength` (dynamic arrays),
`oldz`, `print`, `random`.

**Parameters.** `@name` reads a parameter (read-only). Without a
parameter block a parameter is complex and the UI sorts them by name;
with blocks (`[type] param name ... endparam` in `default:`), the type,
`caption`, `default`, `min`, `max`, `enum`, `hint`, `exponential`,
`visible` / `enabled` apply, in block order. `@fn(x)` calls a user
function chosen from the built-in list, configured by
`func name ... endfunc` (`caption`, `default`, `hint`). Fractint's
`p1..p6` and `fn1..fn4` need no `@`.

**Loops on the GPU.** A `while` or `repeat` that never ends would hang
the device, so every loop the compiler emits carries an iteration cap
(4096), and the direct path's row banding already times each dispatch.

## What we compile to

The front end (lexer, parser, type checker) produces a typed IR. Three
back ends:

1. **WGSL.** A formula becomes a *program*: `struct FormulaState` (z and
   every variable that lives across iterations), `formula_init(pixel)`,
   `formula_loop(state)`, `formula_bailout(state) -> bool`. A colouring
   becomes the existing `ColoringDef` interface, `accum_init`,
   `coloring_accum` and `coloring_map`, with its persistent variables
   packed into the `vec4` state. Helper functions each formula or
   colouring uses are emitted into its own source, prefixed (`uff_` for
   formulas, `ufc_` for colourings; the texture layer's are renamed
   `layer_*` like any colouring's), so the perturbed and recolour
   templates, which have no formula helpers, still compile a custom
   colouring.
2. **A CPU interpreter in f64** over the same IR: tests, and in phase 3
   the reference orbit (through `BigComplex`).
3. **Phase 3: the delta form**, from the IR.

**Packing a colouring's state.** A variable counts against the 4 floats
when it is written in `init:` or `loop:` and read in a later iteration or
in `final:`. UF makes an undeclared variable complex, so `sum = 0` costs
two floats; the compiler narrows a complex variable to one float when
its imaginary part provably stays zero (every assignment real-valued),
so typical untyped colourings fit. Past the cap the compile fails with
the count and the variables.

**Errors** carry line and column and show in the editor. The generated
WGSL is validated with naga at compile time, so a compiler bug surfaces
as an error message rather than a GPU validation failure.

**Measured while building the back end:** the hardware `%` on a negative
`i32` does not follow WGSL on a GTX 1660 under Vulkan: `-5 % 3` gave 2,
where WGSL (and the f64 interpreter) give -2. The generated remainder is
spelled `a - b * (a / b)`, which measured right. Any existing shader
that takes `%` of a signed value that can be negative is worth a look.

## The engine (survey of 2026-10-13)

**Registration.** `get_formula` / `get_coloring` return `&'static`
defs, and every cache keys on the name: the pipeline keys
(`renderer.rs` direct, perturbed, recolour, layer probe), the records'
`iterate_key_for`, the footprint's `picture_key`. A compiled entry is
therefore registered as a leaked `FormulaDef` / `ColoringDef` under the
name `custom-<hash>`, the hash of (dialect, entry, source), in a runtime
table that `get_formula` / `get_coloring` / `layer_of` consult before
falling back. Every existing name-keyed path then works unchanged, and
the escape panel's parameter sliders, presets and hints read the leaked
`EscapeParamDef`s like any built-in's. Recompiling the same source
reuses the entry; a new source leaks one more def (kilobytes; the editor
compiles on a pause in typing, not per keystroke). `accum_is_wide_named`
and `config_accum_is_wide` look names up in `COLORINGS` directly and
must go through the runtime-aware lookup, or a custom accumulating
colouring is sized narrow and loses `.zw`.

**A formula program on the direct path.** A new `FormulaFeature::Program`
switches the direct template's markers: `PARAM_PLANE_SEED` and
`C_DECL` become `var fs = formula_init(pixel); z = fs.z;` (c = pixel for
the colourings), `STEP` runs `formula_loop`, and `ESCAPE_TEST` becomes
"`formula_bailout` false: escaped". UF's order is the template's: the
formula's loop, the colouring's loop, then the bailout. A program does
not get, in phase 1: the Julia toggle (its `#pixel` is written into the
formula; `switch:` and Fractint's `ismand` bring it back in phase 2),
interior detection (its hidden state can differ while `z` repeats), the
embossed relief (it re-emits the step), damping and biomorph, a
derivative, perturbation (`perturb_tier` matches names, so a custom
name never perturbs: no change needed). `visibility.rs` hides those
controls for it.

**The bailout parameter.** UF formulas carry their own bailout
(`|z| < @bailout`), but the smooth colourings read the config's
`bailout`. A custom formula with a float parameter named `bailout` has
the config's bailout bound to it, which is UF's own convention; others
keep the panel's bailout for the colourings only.

**A custom colouring.** `#z` is the iterate; `#pixel` the pixel's c on
the parameter plane, the pixel itself on the Julia plane (the templates
gain `esc_pixel`, also in recolour, which rebuilds it from the pixel
position); `#numiter` and `#maxiter` from the summary and params.
`#solid = true` returns a sentinel from `coloring_map` that the
templates read as no coverage (the background), like an interior pixel.
Tags: untagged colours inside and outside (`ColorsInterior`),
`OUTSIDE` outside only, `INSIDE` inside only. A separate inside
colouring is the survey's P8 (`escape-coloring-survey.md`), not this
project.

**Parameters.** 16 floats each for the formula and the colouring
(`PARAM_VEC4S`): float, int, bool and enum take one, complex two
(`name.re`, `name.im` in the config's map), a user function one (its
index in the built-in list; the WGSL switches on it, so changing it does
not recompile). Past 16 the compile fails.

**The config.** `EscapeConfig` gains `custom_formula` and
`custom_coloring: Option<CustomSource>` (`dialect`, `entry`, the entry's
`source`), skip-if-default, so no version bump. `formula` / `coloring`
hold the `custom-<hash>` name. Anything that resolves a config first
registers its custom sources: the escape renderer, the panel, the CLI's
`unknown_names` check, the script API, the WASM modules. An older build
reading such a file falls back to Mandelbrot and escape count with a
warning (today's behaviour for any unknown name). New `ConfigPath`s
carry the source; picking a custom entry is one `update_batch` with the
name change, so it undoes whole. PNG metadata embeds the config, so it
embeds the source. The API stores the config as an opaque blob
(`api/sync.rs`) and validates no names; when this ships, the API
repository needs telling that escape configs can carry source
(RELEASE.md section 3), and its renderers need a build with the
compiler.

## Phases

### Phase 1: the language, formulas and colourings, embedded, editable

1. **Front end** (`src/escape/lang/`): lexer, parser (UF sections,
   Fractint's empty label and commas), AST with spans, diagnostics; type
   checker (UF rules, predefined symbols, parameters, the function
   table). Unit tests on parsing and every error.
2. **IR and the f64 interpreter.** Tests: Mandelbrot, Magnet I/II, a
   colouring's `#index`, all against hand computation.
3. **WGSL back end.** The formula program, the colouring interface with
   state packing, helper emission, loop caps, naga validation. A fuzz
   test: random expressions compile, validate and pass the shader lint.
4. **Engine integration.** `FormulaFeature::Program` in the direct
   template; the runtime table behind `get_formula` / `get_coloring`;
   `EscapeConfig` fields, `ConfigPath`s and undo; registration at every
   entry point; `esc_pixel`; the `#solid` sentinel; the bailout binding;
   visibility.
5. **Equivalence.** About ten built-ins rewritten in the language
   (formulas: Mandelbrot, Multibrot 3, Burning Ship, Tricorn, Lambda,
   Magnet I and II, Kaliset; colourings: escape count, a minimum-|z|
   trap, the triangle inequality average, the stripe average) render as
   their built-ins do, within f32 tolerance, on the direct path and,
   for the colourings, on the perturbed path at depth.
6. **The editor.** A Formula Editor panel: the source, its entries,
   errors by line, compile on a pause, "use as formula" / "use as
   colouring"; a folder of the user's `.ufm` / `.ucl` / `.frm` files in
   the data directory (the plug-in folder's pattern; localStorage on
   the web); open and save. The script API sets a custom formula or
   colouring from source. ESCAPE.md gets a section; visual configs
   carry a custom formula and a custom colouring.

### Phase 2: Fractint, the rest of UF's formula language, the derivative

- Fractint semantics: every variable complex, bool as `(1, 0)` / `0`,
  truthiness by real part, writable parameters (copied to locals),
  `LastSqr` (set by `sqr` and `|x|`), `whitesq`, `scrnpix`, `scrnmax`,
  `ismand` (the Julia toggle), `center`, `magxmag`, `rotskew`, Fractint's
  precedence.
- `switch:` (a Mandelbrot / Julia pair becomes our Julia toggle),
  `global:` (evaluated on the CPU into `fdata`), `default:` view
  settings as a preset (`center`, `magn`, `angle`, `maxiter`; **to
  verify:** what span UF's `magn = 1` shows).
- A symbolic derivative: distance estimation, normal maps, analytic
  relief for holomorphic programs.
- Interior detection and the embossed relief for programs.
- Measure the colouring cap against real UF colourings; widen past 4
  floats (a second `vec4` in `IterState` and `IterResult`) if many need
  it.
- Test files covering every construct, in both dialects.

### Phase 3: deep zoom for custom formulas

- UF's own perturbation sections first: `perturbinit:` / `perturbloop:`
  give the delta (`#dz`, `#dpixel`); the reference orbit is the loop
  section interpreted in `BigComplex` (which has add, mul, div, recip,
  exp, ln, sin, cos, sqrt, atan2, powf).
- Then the delta derived from the IR, rule by rule (the rules our
  hand-written tiers already use: product, quotient, `diffabs`, `log1p`),
  on the scaled rung, then the floatexp rung.
- Kalles Fraktaler-style hybrids.

### Phase 4: distribution

- The library in the Fractal Browser; an API formula corpus (a contract
  change: tell the API repository).
- Direct colouring (`#color`, the `color` type, the colour functions).
- A custom colouring as the texture layer, if phase 1 has not already
  made it free.

### Later: transformations as flame variations

UF's third entry kind, the transformation (`.uxf`, a `transform:` section
rewriting `#pixel`), is a flame variation's shape: a point to a point,
with parameters. The front end, the function table, the WGSL rules and
the interpreter carry over; a compiled transformation would produce a
`VariationDef` (parameters and 2D WGSL) and register it as the plug-in
and API variations do. The gaps: 3D variations need a vector type,
and variations that draw random numbers or read JWildfire's accumulator
need hooks. The escape engine's camera lens warps the view by any
variation, so one custom transformation would serve both.

## Licensing

Importing the user's own files needs nothing. Bundling anyone else's
formulas (UF's public database, Fractint's `fractint.frm`, the Orgform
collection) needs each author's terms checked first; until then the
examples we ship are written here. Nothing is ported from Fractint's
or UF's source code: the language is implemented from its
documentation.
