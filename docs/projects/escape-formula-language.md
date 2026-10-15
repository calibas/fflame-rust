# Escape-time formula language: formulas and colourings in Rhai, Ultra Fractal and Fractint imported

Status: **phase 1 done 2026-10-14** on `escape-expansion`: the language
(`src/escape/lang/`), the runtime registry (`src/escape/custom.rs`), the
engine and config, the Formula Editor panel and its library, the script
functions, the docs (ESCAPE.md, "The formula language"). **Phase 2
done 2026-10-14**: Fractint's conventions, the derivative, interior
detection and the embossed relief for programs, `global:`, `switch:`,
the measurement against UF's public library, and the test files. The
state cap is 32 floats (the user's decision, 2026-10-14, from the
measurement under phase 2). **Phase 3 in progress** (deep zoom for
custom formulas; its plan and order of work below): steps 1 to 5 done
2026-10-14. **Phase 4 decided 2026-10-14** (formulas in Rhai, the user's
decisions below): the formula language becomes Rhai, with explicit
state; UF and Fractint files stay loadable and convert to Rhai when
opened; the built-in formulas get Rhai twins. Phase 3's step 6 is
folded into phase 4's order of work. Phase 4's steps 1 (the animation
bug) and 2 (the Rhai front end) done 2026-10-14; step 3's formulas (UF
and Fractint converted to Rhai) 2026-10-15, its colourings next.

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
5. **A colouring's per-pixel state is capped at a fixed size.** The
   engine's accumulator is one `vec4` (4 floats), shared by every
   built-in colouring, and it is what the perturbed path and the recolour
   records keep per pixel (`IterState`, `IterResult`). Phase 1 capped
   custom colourings at those 4 floats; phase 2 measured UF's public
   colourings and the cap became 32 (below, "the state cap").

## Decisions (2026-10-14): Rhai

The user's, after phase 3: reproduce the systems, not Ultra Fractal --
in a way that complements this program, and not bound by UF, Fractint
or Kalles Fraktaler where newer ways are better. Wanted: open a
built-in (the Mandelbrot) as a formula and change it; convert whole
families of the built-ins; hybrids of any formulas, not only KF's
operators; parameters that animation and scripts can target; scripts
that can change the formulas themselves.

1. **The formula language is Rhai**, the scripting language this
   program already embeds: one language for scripts and formulas. A
   formula is a Rhai script whose GPU subset compiles to the existing
   IR (`lang::ir`), so everything below the IR stays: the interpreter,
   the big-float reference, the WGSL back ends, the derivative, the
   derived perturbation equations, hybrids' phases.
2. **UF and Fractint stay loadable, and convert on opening.** A config
   carrying UF or Fractint source still compiles as it does now; the
   Formula Editor converts an entry to Rhai when it opens it, and what
   it saves is Rhai.
3. **Explicit-state step functions**: a formula is `start`, `step` and
   `escaped`, its state a map passed in and returned -- not UF's
   variables shared across sections. What a step carries from pass to
   pass is then visible, which deep zoom (carried state), hybrids
   (composition) and scripts (calling and composing steps) all need.
4. **Source twins for now.** Each built-in that can be written in the
   language gets its Rhai source beside its native code, held equal by
   tests; native speed for the compiled ones (BLA, nucleus references,
   the Jacobian DE) comes later, and then the native code retires
   family by family.

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

**Types.** `bool`, `int`, `float`, `complex`; `color` is phase 5 (direct
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
colour functions (phase 5), `length` / `setLength` (dynamic arrays),
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

**Packing a colouring's state.** A variable counts against the cap
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

**A custom colouring.** `#z` is the iterate; `#pixel` the pixel's point
in the plane, from private globals the templates set only when a program
reads them (`lang::wgsl::ENV_DECLS`: also `#x`, `#y`, `#width`,
`#height`, `#center`, `#magn`, `#random`), so every built-in shader is
byte-identical. The recolour pass rebuilds the point from the pixel's
position without the lens, so a lensed view whose custom colouring reads
`#pixel` re-iterates instead of recolouring. `#numiter` and `#maxiter`
come from the summary and params. `#solid` sets a private flag the
templates read straight after their own `coloring_map` call (offset
relief calls it again beside the pixel), and the pixel is left to the
background, like an interior one.
Tags: untagged colours inside and outside (`ColorsInterior`),
`OUTSIDE` outside only, `INSIDE` inside only. A separate inside
colouring is the survey's P8 (`escape-coloring-survey.md`), not this
project.

**Parameters.** 64 floats each for the formula and the colouring
(`PARAM_VEC4S` = 16 vec4s; 16 floats until the measurement below):
float, int, bool and enum take one, complex two (`name.re`, `name.im` in
the config's map), a user function one (its index in the built-in list;
the WGSL switches on it, so changing it does not recompile). Past 64 the
compile fails.

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

**Measured (step 5, `custom_entries_render_as_their_built_ins`):** the
language's Mandelbrot, Lambda and Magnet I render as the built-ins do,
pixel for pixel, with a custom escape-count colouring too; Multibrot 3
differs on 0.057% of pixels and Tricorn on 0.008% (the built-ins raise
to a power in polar form, the language multiplies); Burning Ship on
1.95% (the built-in doubles one product, `sqr` adds two, which the
driver may fuse into an fma, and the ship's boundary is chaotic). A
custom accumulating colouring at zoom 20, perturbed, matches its
built-in exactly.

### Phase 2: Fractint, the rest of UF's formula language, the derivative

Order of work (2026-10-14): (1) Fractint's numeric conventions, `ismand`
as the Julia toggle; (2) the derivative; (3) interior detection and the
embossed relief for programs; (4) `global:`; (5) `switch:`; (6) the
colouring cap measured against real UF colourings; (7) test files for
both dialects. All done, 2026-10-14.

**Fractint, from its `parser.c`** (xfractint 20.4.10): comparisons and
`&&` / `||` push a real 1 or 0 with a zero imaginary part; a test is true
when its real part is nonzero (`dStkJumpOnFalse`, and `Formula()` stops
iterating when the last value's real part is exactly 0); `==` and the
orderings compare real parts only; `sqr()` sets `LastSqr` to its
argument's squared modulus, `|x|` does not; `round(x)` is
`floor(x + 0.5)`; `whitesq` is `(row + col) & 1`; `scrnpix` is
`(col, row)`; `ismand` is a global flag, 1 for the Mandelbrot. Writing to
a parameter or a predefined name works in Fractint; here it makes a
per-pixel copy (Fractint's write outlived the pixel, a quirk not kept).
**Unverified:** UF's `magn`. Its help says only that the default is 1;
the claim that magnification 1 shows a height of 4 comes from another
program's documentation. Not applied until checked.

**The derivative (done).** `lang/derive.rs` differentiates a checked
program with respect to `#pixel`, forward mode, into another ordinary
program: each complex variable the iterate depends on (a fixpoint over
the assignments; tests only choose a branch) gets a `d_` companion
assigned just before it from the values before it, and `dz_var` names
the iterate's. Differentiating by the pixel covers both planes at once
(`z = 0, c = #pixel` gives `2 z dz + 1`; `z = #pixel` starts `dz` at 1).
The interpreter and the WGSL back end run the result unchanged; the
registry compiles it as `wgsl_derivative` (with `formula_dz(s)`), and
the direct template uses it in place of the plain program when a
colouring reads the derivative, stepping `dz = formula_dz(fs)`.
Decisions: a fold in the iterate's dependencies (`abs`, `conj`, `real`,
`imag`, `flip`, `|x|`, a real variable that follows the pixel) means no
derivative, reported as for a built-in; a user function's derivative is
chosen at run time with it (`uf_user_d`, `Builtin::deriv_c`), and is
zero for a choice that has none. A complex product with a real factor
now scales the other operand in both back ends (`IExpr::is_lifted_real`),
as the built-ins write `2.0 * z`: it differs from the full product only
where an operand is infinite (`0 * inf`), and it made Magnet I exact.
Measured: distance estimation on the language's Mandelbrot is identical
to the built-in's on both planes, UF and Fractint.

- Fractint semantics: every variable complex, bool as `(1, 0)` / `0`,
  truthiness by real part, writable parameters (copied to locals),
  `LastSqr` (set by `sqr` and `|x|`), `whitesq`, `scrnpix`, `scrnmax`,
  `ismand` (the Julia toggle), `center`, `magxmag`, `rotskew`, Fractint's
  precedence.
- `default:` view settings as a preset (`center`, `magn`, `angle`,
  `maxiter`; **to verify:** what span UF's `magn = 1` shows).
- ~~`switch:` (a Mandelbrot / Julia pair becomes our Julia toggle).~~
  Done. UF's help ("Switch feature"): the last section; `type` names a
  formula in the same file; `destination = source` copies a parameter,
  `#pixel` the clicked point, to a complex parameter only; a copy
  between different types is ignored; what is not copied takes its
  default. Its `seed` / `bailout` "lookup" is the example's own copies,
  not an implicit rule. Here (`lang/switch.rs`): a switch that passes
  `#pixel` makes the pair ONE program, each section
  `if (ismand) formula else target`, so the toggle is Fractint's,
  already driven; the target's variables follow the formula's, a copied
  parameter reads the formula's (or the new `Predef::JuliaC`), an
  uncopied one is the target's own, renamed `<target>_<name>` when the
  name is taken. A switch without `#pixel` (a Julia's back to its
  Mandelbrot), a missing target or one that does not compile leaves the
  formula alone, with a warning. The editor and the examples carry the
  target's text in the config (`custom::carried_source`), so the hash
  covers both and Live re-applies an edit to either. Measured: UF's own
  example pair renders the built-in Julia set exactly with the toggle
  on; off, 0.031% of pixels differ from the built-in Mandelbrot (the
  f64 interpreter runs the pair identically to each entry alone, so
  the ulps are the driver's code for the loop inside the branch).
- ~~`global:` (evaluated on the CPU into `fdata`).~~ Done. UF's help
  ("Global sections"): run once per image; its variables are "treated
  as read-only in other sections". Here: the checker marks what
  `global:` makes as global, refuses a write elsewhere and a per-pixel
  symbol inside it, and packs the globals the other sections read
  (`Program::globals`; one float each, two for a complex). The
  interpreter computes them (`interp::globals`) whenever the renderer's
  `params_for` sees a parameter or the image change (cached per name in
  `custom::globals`); a formula's fill `fdata` (256 floats, unused by
  custom formulas otherwise), a colouring's follow its parameters in
  `cparams` (16 floats in all, refused past it). They are not per-pixel
  state, are constant to the derivative, and are not compared by
  interior detection. The image's names in `global:` are the shader's:
  `#width` / `#height` in render pixels, `#magn` = 4 / span, `#center`
  the (sample-jittered) centre. Arrays, UF's main use for `global:`,
  wait with arrays. Measured: a formula and a colouring computing their
  built-ins' constants in `global:` render the built-ins exactly,
  before and after a parameter change.
- ~~A symbolic derivative: distance estimation, normal maps, analytic
  relief for holomorphic programs.~~ Done (above).
- ~~Interior detection and the embossed relief for programs.~~ Done:
  the repeat is the whole `FormulaState`'s, bit for bit
  (`formula_same`, derivative variables included: they repeat too, and
  a true repeat is all that matters), and Embossed's pair is
  `EMBOSS_WGSL` with each orbit a `FormulaState` run by `formula_init`,
  `formula_loop` and `formula_bailout` (`program_emboss_wgsl`).
  Measured: interior detection invisible on a program with an int, a
  float and a bool in its state (5.5x faster at 20,000 iterations on the
  home view); the language's Mandelbrot embosses as the built-in does
  under all six reductions.
- ~~Measure the colouring cap against real UF colourings; widen past 4
  floats if many need it.~~ **The state cap is 32 floats** (the user's
  decision on the numbers below, 2026-10-14). Floats past the
  accumulator's four live in the colouring's private
  `ufc_ext: array<vec4<f32>, k>` (k up to 7), packed by its own
  `ufc_init`, `coloring_accum` and `coloring_map`; `AccumWidth`
  (Narrow, Wide, Extended(k)) replaces the wide flag, and every template
  stores `ufc_ext` after `accum2` (records and perturbed resume state)
  and restores it after each load. The direct template saves the
  pixel's right after its orbit and restores it after the offset
  relief's orbits beside it. Sizes: a record 48 + 16k bytes, the resume
  state its width plus 16k, rounded to 16 (measured against the shader
  for every tier). Too big for the device, the records fall back to
  re-iterating and the frame to the direct path, as before. Measured
  exact: a palette edit recolours an extended colouring to a fresh
  render's picture, and a perturbed render in 37-iteration chunks to a
  one-chunk render's (each check fails with its restore removed).
  Measured 2026-10-14 against UF's full
  public library (its download, `https://www.ultrafractal.com/cgi/
  uf-formulas.zip?full`, read for measurement only and never bundled;
  `custom::tests::measure_a_uf_corpus`, ignored, `UF_CORPUS=<folder>`).
  Of the 758 colourings that compile or fail on the cap alone, the cap
  admits: 4 floats 56%, 6 66%, 8 74%, 12 85%, 16 89%, 24 94%, 32 96%.
  The measurement also drove the compatibility work done with it:
  - strings continued over lines with `\` (3,188 entries stopped there);
  - UF's compiler directives (`$define`, `$undef`, `$ifdef`, `$else`,
    `$endif`, case-insensitive; UF 6 defines `ULTRAFRACTAL` and `VER20`
    to `VER60`, its reference "Compiler directives");
  - an enum parameter compared with an item's name (`@type == "Sine"`);
  - `switch:` copies naming parameters with `@`, and settings separated
    by commas on one line;
  - Fractint's screen and view names in UF's language too (UF-format
    conversions of Fractint formulas test `whitesq`, 731 uses);
    `LastSqr` stays Fractint's, as it changes what `sqr()` does;
  - the parameter blocks widened from 16 to 64 floats (1,640 entries
    needed more: 71% of them fit in 32, 90% in 64, 98.7% in 128);
  - two compiler bugs: `acos` alone missed its `asin` helper, and WGSL
    refused constants whose value is not finite (`sqrt(-3)`), now
    emitted through an opaque identity (`uf_k`) so the GPU computes
    UF's NaN or infinity.
  Result: formulas 4,791 to 9,250 of 9,831 compile; colourings 289 to
  423 of 1,209; every compiled WGSL validates (it did not: 96, then 117
  invalid). What still stops entries: classes (`import`, 325), arrays
  (296), the state cap (335 colourings), parameters past 64 floats
  (about 200), colour functions (46).
- ~~Test files covering every construct, in both dialects.~~ Done:
  `tests/formulas/language.ufm`, `language.ucl` and `fractint.frm`,
  written for this project (24 entries). `every_test_file_entry_compiles_to_valid_wgsl`
  compiles each to WGSL that validates (a formula's derivative too);
  `test_files_run_on_the_gpu_as_the_interpreter_does` (GPU) runs each
  formula two iterations at five points and compares the iterate, its
  derivative and the test with the f64 interpreter, and each colouring
  beside a Mandelbrot at seven points, comparing `#index`, `#solid` and
  the count (134 comparisons; a mutation of Fractint's `round` fails
  it). The probe feeds the parameter blocks and globals through a
  storage buffer in the uniform's layout. The interpreter now applies
  an `INSIDE` colouring's region as its WGSL does.

### Phase 3: deep zoom for custom formulas

UF's model (its help, "Perturbation equations", "Perturbation
calculations", the `perturb` setting), which this follows:
`perturbinit:` sets `#dz`, the pixel's deviation from the reference
orbit, and `perturbloop:` steps it, with `#z` the REFERENCE's iterate
there and `#dpixel` the pixel's offset from the reference point (the
example: `#dz = 2*#z*#dz + sqr(#dz) + #dpixel`). The formula's own
`bailout:` tests the pixel's full `z`. UF keeps only the reference's
`z`. `perturb = <bool>` in `default:` (parameters and constants; absent,
true) says when the equations hold. UF fixes glitches with more
references; this engine rebases (Zhuoran), as its built-in tiers do.

Order of work (2026-10-14):

1. **The language side** (done). The sections, `#dz` / `#dpixel` /
   `#z` there, their own variables (no other section's: the reference
   keeps only z), the `perturb` setting (`lang/check.rs`); the WGSL
   functions `formula_perturb_init(z_ref, pixel, dpixel)`,
   `formula_perturb_loop(z_ref, dz, pixel, dpixel)` and, when the test
   reads only z, `formula_perturb_bailout(z, pixel)` (`lang/wgsl.rs`);
   `interp::run_perturbed`, the f64 model of the perturbed path
   (reference orbit, deviation, rebase), measured to iterate as the
   formula does; `lang/bigeval.rs`, the program in big floats for the
   reference orbit (measured equal to the f64 run, every function, to
   1e-12, and keeping digits past f64); `lang/perturb.rs`, whether a
   formula perturbs (equations, a test of z alone, no screen position)
   and whether it rebases (only when the loop carries nothing but z).
2. **The custom tier on the scaled rung** (done). `PerturbTier::Custom { hash,
   rebase }` (Copy + Eq; the hash is the formula's name's, so the
   pipeline key and the reference identity follow the source). The
   reference: a new big family (`MAP_CUSTOM`, never stored on disk, as
   no big family is) whose state is a `BigMachine`, its identity the
   program and its parameter values encoded into the two `map_params`
   floats the reference tuple already threads everywhere (normal f32s,
   no NaN patterns, so equality works); the orbit ends where the
   program's own test fails. The step: `w' = F(Z, S w, S d0) / S`, F
   the compiled `perturbloop:` (the delta in f32 is S*w, representable
   down to the scaled rung's floor), the start `perturbinit:` the same
   way, `d0` passed as `#dpixel` on both planes (so the built-ins'
   Julia `d0_term` convention does not apply), the test the program's
   own on `z_full`, rebase default or only at the orbit's end. No BLA,
   no derivative under perturbation (`derivative_gap` reports it).
   Refused past the scaled rung (zoom 48) until step 3, so a deep view
   renders on the direct path, as for a formula with no tier.
   As built: the step is not `F(Z, S w, S d0) / S` after all. Measured
   against the truth (each sampled pixel iterated in big floats), that
   form's three extra roundings a step cost it in chaotic regions (391
   of 439 exact against the built-in tier's 404 with the same plain
   reference, seahorse valley, zoom 20, 2000 iterations). So the
   equations compile with step 3's Small/Full classes already, Small
   values held in units of S (`Gen::sgen` in `lang/wgsl.rs`): a Small
   times a Full is Small, two Smalls' product carries one S, a Small
   over a Full is Small, a ratio of Smalls Full; anything else runs on
   true values. UF's Mandelbrot equations then compile to the built-in
   step's arithmetic: 402 of 439 against 404, 439 of 439 at 300
   iterations, and 439 of 439 on a deep Julia view
   (`a_custom_formula_perturbs_by_its_own_equations`; 410 for the
   built-in with its nucleus reference, which the custom family has
   not). The reference identity rides in `map_params` as two normal
   f32s (46 bits of a hash over the program, its packed parameters,
   the plane and the image values it reads); `custom::reference_machine`
   starts the program for the orbit. Found on the way and fixed: the
   blocking cache path loaded stored orbits with the family clamped to
   5, so Magnet I, Kaliset (-c) or Ducks v0 could be served a stored
   Feather(2) orbit at the same centre.
3. **The floatexp rung** (done). Planned with Small/Full classes as on
   the scaled rung; built simpler and more precise: every value of the
   equations is a `CFe2` (the deep template's double-float mantissa
   with an exponent), Full and Small alike, so `#z` keeps the
   reference's double-float entry and a coefficient like `2 #z` the
   precision the built-in deep steps give it (a bare f32 `Z^(p-k)` is
   what split a depth-426 power-4 view along a rebase seam once).
   `formula_perturb_init_fe` / `formula_perturb_loop_fe`
   (`lang::wgsl::formula_perturb_fe`, `ufe_` helpers) and
   `PERTURB_FE_HELPERS` (neg, sub, conj, flip, parts, `abs2`, `powi`,
   and a reciprocal refined by one double-float Newton step) are
   spliced into the deep template only; what has no floatexp form runs
   on true values in f32. Measured against the truth (big floats,
   `a_custom_formula_perturbs_by_its_own_equations`): the visual
   suite's deep edge views, zoom 48.5 with 8000 iterations, 223 of 237
   pixels exact (195 distinct counts) against the built-in tier's 194,
   and zoom 60 with 30000, 104 of 106 (48 distinct) against 97 -- the
   built-in skips by BLA there, an approximation; the custom tier
   iterates every step, exact and slower.
4. **The delta derived from the IR** (done), so a formula needs no
   `perturbloop:` (`lang/delta.rs`). Planned as the Small/Full algebra
   over the loop's expressions; built one level up, as a source
   transformation: every value of the loop becomes a pair of new
   variables of the equations (its value at the reference, from `#z`
   and `#pixel - #dpixel`; the pixel's deviation, from `#dz` and
   `#dpixel`), and the result is ordinary `perturbinit:` /
   `perturbloop:` sections that steps 2 and 3's code generators compile
   as they do a formula's own. The rules: sums, products, the quotient
   form `(da - q db)/(B + db)`, binomial integer powers, `sqr`, `recip`,
   `|a|`, `conj`/`flip`/`real`/`imag`, and `abs` by `diffabs` (a new
   internal operator, `BinOp::DiffAbs`, never parsed; `uf_diffabs`,
   homogeneous so the scaled rung passes `C/S` as the built-in Ship
   does; `cfe2_diffabs`, and `cfe2_abs` so `abs(#z)` keeps the double
   float on the floatexp rung -- without it the deep Ship view below
   counted 70, not 84). The init's variables are constants per pixel,
   their values substituted in order. One pair per variable, whichever
   arm writes it, the deviation assigned first (a reference value
   reads no deviation), so a branch whose conditions are the same for
   every pixel (a parameter, a constant) derives arm by arm; a variable
   is written after it when written before or in every arm. z written
   in steps gets its own reference variable, `#z` until the first
   write. The test may read the init's constants
   (`perturb::perturb_test` puts their values in, for a formula's own
   equations too: UF-format conversions of Fractint formulas set
   `bailout = 4` in `init:`). One change in step 2's classes: a
   literal zero no longer makes a variable Full (an arm that leaves a
   deviation at zero). `custom::compile_formula` derives at compile
   time and keeps the formula as written when the derived WGSL fails
   validation (desktop; the measurement found none).
   Measured against the truth (`a_custom_formula_perturbs_by_derived_equations`,
   exact counts of the sampled pixels; the built-in tier at the same
   view beside it):

   | View | Built-in | Derived |
   |---|---|---|
   | Mandelbrot, seahorse valley, zoom 20, 600 iterations | 420 of 439 | 416 |
   | Mandelbrot, zoom 48.5 (floatexp), 4000 iterations | 234 of 237 | 236 |
   | Cubic, a walked boundary, zoom 24, 1000 iterations | 381 of 439 | 381 |
   | The Mandelbrot through a parameter branch and two writes of z | 420 / 234 | 416 / 236 |
   | Burning Ship, ship-deep-perturbed's view, zoom 16, 2000 iterations | 418 of 439 | 418 |
   | Burning Ship, ship-deep-floatexp's view, zoom 60, 25000 iterations | 64 of 106 | 84 |

   The built-in Ship's deep step takes the reference's components as
   f32 (`sfe_diffabs(X: f32, ..)`); whether that is its whole gap is
   not investigated. UF's public library: 2 formulas perturb by
   their own equations and 308 by derived ones (249 before
   branches, z in steps, the test's constants and `abs`). What stops
   the rest, by the first thing the measurement finds: a test that
   reads a variable the loop changes (2,747), a user function (1,425),
   a branch or loop in the init (1,400), a branch on the iterate
   (1,105), a real or complex power (643), carried state or an int or
   bool written in the loop (582), a loop inside the loop (327), and
   the transcendental functions (`sqrt` 224, `sin` 130, `cos` 90, and
   others). The next candidates, should step 4 be extended: branches in
   the init on parameters (the init's values as a selection), and the
   transcendental functions by their difference forms (`sin(A + d) -
   sin(A) = 2 cos(A + d/2) sin(d/2)` and kin, which a user function's
   choice would need too).
5. **Kalles Fraktaler-style hybrids** (planned 2026-10-14). KF 2.15's
   hybrid designer, as its manual describes it (`mathr.co.uk/kf/manual.html`,
   "Hybrid Formula dialog"; nothing taken from its source): up to four
   groups, each run `repeats` times (one iteration a repeat), in order,
   looping back to the group marked "start loop" (the groups before it
   a prefix, run once); a group's lines (up to two) run in sequence,
   each combining two operators of the current z by `+`, `-` or `*`
   (`/` since 2.15.5, "incomplete" there); an operator is
   `(±|x| ± |y| i)^p · a` -- abs and negation per part, a positive
   integer power, a complex multiplier; `+ c` once, after the lines.
   Its manual's own limitation is the one that shapes step 5b: "Hybrids
   are broken when using single reference method ... a proper fix would
   need multiple reference orbits at different phases of the hybrid
   (pre)periodic loop."

   **5a. The model, the direct path, the designer** (done
   2026-10-14, as planned; the grey reason and `on_release` drags
   below are the only additions).
   As built: the presets iterate on the CPU exactly as their
   definition by hand (`hybrids_iterate_as_written`), on the GPU as
   the interpreter (`hybrids_run_on_the_gpu_as_the_interpreter_does`),
   and each one-group preset renders as its built-in through the whole
   renderer: 0.03% of pixels differ for the Mandelbrot, 0.06% the
   cubic, 0.008% the Tricorn, none for the three folds
   (`a_hybrid_renders_as_its_built_in`). The CLI renders a saved hybrid
   config. The power and repeat drags commit on release
   (`slow_controls::on_release`: each value is a recompile); the panel
   greys outside Escape mode with its own reason.
   - A hybrid is data, `Hybrid { groups, loop_start }` (`lang/hybrid.rs`),
     saved as a custom source of a third dialect, `CustomDialect::Hybrid`,
     its `source` the hybrid's JSON: the registry's `custom-<hash>`
     name, the config, PNG metadata, the CLI and the gallery carry it as
     they carry UF text. The multipliers are the program's parameters
     (`a<group>_<line>_<operator>`, complex), so dragging one recompiles
     nothing; the JSON keeps their starting values, and any structural
     edit (a flag, a power, a repeat count, a group, a line) is a new
     source and a recompile.
   - It compiles to the IR directly, not through text: UF has no way to
     say "the iteration count" in a formula (`#numiter` is read only in
     a colouring's `final:`), and a counter variable is state carried
     from pass to pass, which step 4 refuses. So the IR gains the pass
     index, `Predef::Pass` (0 on the first pass; no name in either
     dialect), and the program records its cycle, `Program::cycle =
     (prefix, period)`: the passes the groups before the loop start
     take, and the passes one round of the loop takes. Each group is a
     branch on the pass index; the init is `z = 0` (or the pixel on the
     Julia plane, by `#ismand` arithmetic, not a branch); the test
     `|z| < @bailout`.
   - The pass index: the interpreter's and the big-float machine's
     count; on the GPU a field of `FormulaState` that `formula_loop`
     advances (only in a program that reads it), so the direct template
     needs nothing new. The derivative pass gives it none.
   - The designer: a new panel, Escape mode only (`visibility.rs`):
     groups as rows (repeats, start-loop), each line two operators
     (abs x, abs y, neg x, neg y, power 1-12, the multiplier) and the
     combining operator; presets for the built-ins KF's "To Hybrid"
     would map (Mandelbrot, the multibrots, Burning Ship, Tricorn, the
     two Perpendiculars) and an alternating one or two.
   - Measured: a one-group hybrid renders as its built-in does, and an
     alternating one on the GPU as the interpreter iterates it.

   **5b. Deep zoom for hybrids.** The derivation (step 4) takes the
   pass index as the same for every pixel; the equations read it as the
   REFERENCE's global index, so the pixel and the reference take the
   same group as long as their phases agree. Rebasing to the orbit's
   start would break that whenever the pixel is not at phase 0, which
   is exactly when a pixel passing near the critical point must rebase.
   So one reference orbit per phase of the loop: reference `q` (`0 <=
   q < period`) is the formula run from its start with its pass index
   starting at `prefix + q`, and the main one starts at 0 and covers
   the prefix. A pixel at pass `n >= prefix` rebases (on Zhuoran's test,
   or at an orbit's end) to reference `(n - prefix) mod period`, index
   0, which has its phase; during the prefix only the main reference
   serves. The GPU keeps the orbits in the one orbit buffer with a small
   table (offset, length, starting pass), and the pixel's state gains
   its reference's number. Measured against the truth (big floats) as
   steps 2 to 4 were, on an alternating Mandelbrot / Burning Ship at
   both rungs, beside a single-reference build to show what the phases
   buy.

   As built (done 2026-10-14), simpler than the table: the `R = 1 +
   period` references are INTERLEAVED in the one orbit (entry `m * R +
   r` is reference `r` at its index `m`), stepped in lockstep by `R`
   big-float machines (`BigMachine::set_pass` starts each at its
   phase), a reference that has ended padded with `ORBIT_PAD_E`. So
   the orbit grows, publishes, uploads and caches as one array, as
   every family's does (the request asks for `R` entries a pass,
   `renderer::orbit_depth`), and the shader's `m` is the flat index:
   the pixel steps by `R` (`//__M_STEP__`, `m = m + 1u` for every
   other tier, so their WGSL is unchanged), its reference and pass are
   `m % R` and `m / R` (`custom_pass`, which the equations take as
   `pass_index`), and the state needs no new word. The rebase
   (`rebase_phased`) goes to entry `1 + (i - prefix) % period` -- the
   phase's reference at its start, which all share, so the floatexp
   rung's `fe_rebase_delta` serves unchanged -- on Zhuoran's test or
   when the reference's next slot is a pad; during the prefix only the
   main reference serves, and its end there counts the pixel escaped.
   `Perturbable::cycle` carries the cycle, at most 31 phases
   (`perturb::MAX_PHASES`); the derivation gained int and bool
   temporaries whose values are the same for every pixel (the phase).
   `interp::run_perturbed` models the same references in f64.
   Measured against the truth (`a_hybrid_perturbs_with_a_reference_per_phase`,
   64x48, every 7th pixel), at a boundary walked to the scaled rung and
   on to the floatexp one:

   | Hybrid | Zoom 24 (scaled) | Zoom 51 (floatexp) |
   |---|---|---|
   | Mandelbrot / turned Mandelbrot, alternating (period 2) | 391 of 439 (223 distinct counts) | 439 of 439 (79) |
   | Cubic once, then Mandelbrot / turned twice (prefix 1, period 3) | 343 of 439 (259) | 380 of 439 (228) |

   The turned copy is `z^2 * (1, 2^-7)`: groups that differ, in tame
   dynamics. Not as planned: the alternating Mandelbrot / Burning Ship
   walks into regions where the counts need 53 to 128 bits a pixel
   (the f64 model misses a fifth of them at zoom 6, where 4 and 10
   limbs agree on every pixel), where no rung can be exact, so that
   preset is held where it is countable (zoom 3, forced perturbed: 400
   of 439, as the direct path). And no single-reference build: in the
   f64 model, the only place it would be cheap, the references are f64
   too, and at the depths where phases matter their own rounding is the
   error. Two findings on the way: the GPU holds parameters as f32, so
   a truth run with a parameter's f64 value iterates a slightly
   different map (0.01 against its f32, two thirds of the pixels wrong
   after 2000 passes) -- the test rounds them as the GPU does; and on
   these views the floatexp rung counts closer to the f64 model than
   the scaled one (416 and 380 against 391 and 343 at zoom 24, the
   model 427 and 419): the scaled rung's f32 on long busy orbits, as
   for every tier. The visual suite has the alternating hybrid at zoom
   51 (`escape/hybrid-deep-phases`).

   Not in step 5: distance estimation for hybrids that fold (abs): the
   derivative pass is complex-analytic, and a fold needs the Jacobian
   (later, with the built-in Ship's DE as the model); KF's `.kfr` hybrid
   import (its manual does not give the keys); script functions for
   hybrids (phase 4, step 8).
6. **Extending step 4** (planned 2026-10-14, after step 5; since the
   Rhai decision, folded into phase 4's order of work: carried state
   becomes its step 5, the rest its step 6), in the order of the
   measurement's blockers:
   - Branches in the init on parameters and constants (1,400 formulas
     stop there first): the init's values as a selection.
   - Transcendental functions by their difference forms, free of
     cancellation: `sin(A + d) - sin(A) = 2 cos(A + d/2) sin(d/2)`,
     `cos` likewise, `exp(A) expm1(d)`, `log1p(d / A)`,
     `d / (sqrt(A + d) + sqrt(A))`, the hyperbolics, `tan` by
     `sin(d) / (cos A cos(A + d))`, a real power by
     `A^p expm1(p log1p(d / A))`; small-argument helpers (`expm1`,
     `log1p`, `sin` near 0) on both rungs, since the GPU's own are not
     accurate there.
   - A user function (1,425): a branch on its choice over the functions
     that have a form; a render whose choice has none stays direct
     (`perturb_tier` reads the parameters).
   - Int and bool temporaries assigned values the same for every pixel,
     and counters (a variable stepped each pass) by step 5's phases when
     the CPU can find their cycle.
   - A test that reads a temporary computed from z after its last
     write, by substitution.

### Phase 4: formulas in Rhai (decided 2026-10-14)

**The shape of a formula.** A Rhai script with three entry functions
and its parameters' metadata; a sketch (the details settle in step 2):

```rhai
const PARAMS = #{
    bailout: #{ value: 4.0, min: 1.0, caption: "Bailout" },
};

fn start(pixel, p) {
    #{ z: complex(0.0, 0.0), c: pixel }
}

fn step(s, p) {
    s.z = s.z * s.z + s.c;
    s
}

fn escaped(s, p) {
    s.z.norm() > p.bailout
}
```

- The state is the map `start` returns: its fields are the per-pixel
  state (`FormulaState`), `z` among them (the iterate the engine
  colours and tests). `step` returns it with the same fields; locals
  (`let`) are temporaries. Fields `step` reads before it writes them
  and also writes are the carried state.
- `p` is the parameters, declared with their metadata in `PARAMS`
  (value, range, step, caption, hint, kind: float, int, bool, complex,
  a choice, a function), which the panel, animation and scripts read.
- The context (since step 3; step 2 had a plane argument in `start`
  and a pass in `step`): every function may take a third argument, the
  plane (`julia`, `c` the Julia constant), the pass (`pass`; a
  pass-dependent step, a hybrid, declares `INFO.cycle` for deep zoom,
  phase 3, 5b) and the view and screen (`maxiter`, `center`, `magn`,
  `angle`, `x`, `y`, `width`, `height`, `random`).
- `prepare(p, ctx)`, optional: the per-image values (UF's `global:`),
  read as `p.name`.
- Helper functions in the script are inlined; no recursion.
- Optional `perturb_start` / `perturb_step` keep UF's hand-written
  perturbation equations for formulas that bring them; otherwise they
  are derived (phase 3, step 4).
- Colourings take the same shape: `start`, `step` (per iteration, with
  the formula's z) and `finish` (the orbit's summary to an index or a
  colour).

**The compiler.** Rhai's own parser (`Engine::compile` with optimisation
off, so the tree is the source's), its syntax tree read through the
`internals` feature -- an unstable API, so the version is pinned exactly
(`=1.25.1`, since step 2: the requirement had been a minimum, and three
gallery lock files held 1.26.x, whose script functions' bodies already
differ), and each Rhai upgrade re-checks the compiler. The GPU subset: int, float, bool and complex values (a
registered `Complex` type, `complex(re, im)`; `**` is the power, `^` is
Rhai's XOR), maps only as the state, the parameters and the plane,
bounded loops, `if`, `let`, `return`. Strings serve metadata alone;
arrays, closures, recursion and dynamic types are refused with the
script's own positions. Types are inferred (Rhai declares none) and the
map's fields unify across `start` and `step`. The result is a
`lang::ir::Program`, so the existing back ends take it unchanged.

**UF and Fractint, converted.** The existing parser and checker remain
as importers: a UF or Fractint entry checks to the IR as now, and a
printer turns the IR into a Rhai formula -- the variables that live
across sections or passes become the state map, the rest `let`s,
`default:` the metadata, `global:` a per-image function, Fractint's
numeric conventions (real-part comparisons, its `round`, `LastSqr`)
written out explicitly. Measured on UF's public library: every entry
that compiles now should convert, compile from Rhai, and iterate as the
original does (the interpreter on sample points). A config's UF source
keeps compiling as UF; the editor converts what it opens.

**Order of work.**

1. **The animation bug the user found**: a complex parameter's key
   (`Escape.FormulaParam.seed.re`) does not parse back, so its track
   never plays, nor exports (`ConfigPath::from_string_key` splits on
   every dot). Rejoin the parameter's name; a round-trip test.
2. **The Rhai front end** for formulas: the GPU subset to the IR, the
   metadata, the plane, the errors; `CustomDialect::Rhai`; the Formula
   Editor edits Rhai. Measured: Rhai Mandelbrot, Burning Ship, Lambda
   and a pass-dependent hybrid render as their UF twins and built-ins,
   on the direct path and perturbed; a Rhai test file covering every
   construct runs on the GPU as the interpreter does.

   Done 2026-10-14 (`lang/rhai.rs`). As built: the Rhai tree is lowered
   to the UF PARSE tree, not to the IR, so the checker types it and every
   rule after the parse serves both languages (`check_rhai`: Rhai's
   `abs` of a complex is the modulus, and a bool and a number do not
   mix). The state's fields and locals become variables under prefixed
   names (`s_c`, `l3_t`), which the messages turn back into the script's
   (`s.c`, `t`). An expression may leave statements pending before the
   statement holding it, which gives `if` as a value anywhere in `start`
   and `step`, helpers with statements, and a loop condition's
   statements re-run each pass; `escaped` stays one test, its `let`s and
   its helpers substituted. Measured: the Mandelbrot, the Burning Ship
   (both planes) and a Phoenix-like formula with carried state, a
   `for`, a `do ... until`, op-assignment, a function parameter and an
   `if` value run as their UF twins in the interpreter; the hybrid
   alternation as the hybrid preset; five test files
   (`tests/formulas/rhai/`) on the GPU as the interpreter (166 pairs
   with the UF files); the Rhai Mandelbrot and Burning Ship render as
   their built-ins (0% and 1.94% of pixels, the latter the UF custom
   Ship's own 1.95%; the Julia plane 0%), and at zoom 48.5 the Rhai
   Mandelbrot counts as its UF twin on all 3,072 pixels. The editor
   offers Rhai first, New starts a Rhai formula, and three examples ship
   (`assets/formulas/*.rhai`). Not reached: Lambda's twin (it comes with
   the built-in twins, step 4). Moved from step 6 to step 4: branches in
   `start` on the plane or a parameter (every plane-aware formula has
   one, the built-ins' twins among them), which deep zoom needs.
3. **The IR to Rhai printer, and conversion on opening** (formulas,
   then colourings in the same shape); the UF corpus measurement above.

   Formulas done 2026-10-15 (`lang/to_rhai.rs`,
   `custom::convert_to_rhai`). As built:
   - **The conversion.**
     - The state is `z`, every variable a pass reads before it has
       certainly written it, every variable the bailout reads, and
       `#pixel` when the loop reads it (the checker's own flow
       analysis). The rest are `let`s, declared at their first
       assignment or, when that is nested or after a read, at the top
       with their type's zero.
     - Implicit conversions are written where Rhai needs them (a `let`
       has its value's type), and left out where the checker widens
       again.
     - Fractint's real-part tests and equality, and its `round`
       (`floor(x + 0.5)`), are written out.
     - The bailout is negated as written, `!(...)`, which a NaN leaves
       as UF does.
     - The entry's comments and those just above it come along.
     - A `switch:` pair becomes one formula on both planes.
   - **What the front end gained for it.**
     - The context argument, which replaces step 2's plane and pass
       arguments, so screen and view symbols (`#x`, `#width`,
       `#maxiter` ...; heavily used in the library) have a home.
     - `prepare` for `global:` (553 sections in the library).
     - `PI()` and `E()`.
     - `to_float` / `to_int`, the checker's own conversions in Rhai
       mode, including from a bool. An `if` there would put a branch
       in the init, which deep zoom cannot derive yet.
     - `exponential`.
     - An `else if` whose condition needs statements first nests in
       the `else`.
     - Rhai's expression-depth limit raised to 2,048. Rhai counts each
       link of a chain like `a + b + c` as a level without recursing,
       and its 64 refused a long sum.
     - UF's own perturbation equations as `perturb_start`,
       `perturb_step` (the deviation `d` their state, set and
       returned, as `step`'s `s`) and `perturb_when` (UF's `perturb`).
   - **The editor.** Each UF or Fractint formula entry has Edit as
     Rhai, which opens the converted formula as a new unsaved `.rhai`.
     The opened file stays as it is, still usable as UF, and a config's
     UF source keeps compiling as UF. So "convert on opening" is per
     entry, on request: a file holds many entries, a Rhai file one.
   - **Measured on UF's public library (345 files):** all 9,250
     compiling formulas convert, compile back and iterate as the
     original (the interpreter at seven points on both planes, 100
     iterations). Deep zoom is kept on all 310 formulas that have it:
     308 derived, and 2 by their own equations, carried.
   - **Tests.** The test files' formulas convert and run as written
     (`to_rhai::tests`). Their conversions run on the GPU as the
     interpreter does (`test_files_run_on_the_gpu_as_the_interpreter_does`).
     The editor's path is covered by
     `a_uf_formula_converts_to_rhai_for_editing`.
4. **Built-in twins and "Edit as formula"**: the Escape panel opens the
   current built-in in the editor as the user's own formula, with the
   current parameters and view. First the families the derivation
   covers now: Mandelbrot, Tricorn, Burning Ship (all six variants),
   Lambda, Cactus. Each held equal to its built-in by test, direct and
   perturbed. With it, from step 6: the derivation takes branches in
   `start` (the init) whose conditions are the same for every pixel --
   the plane, a parameter -- since every built-in has a Julia plane.
5. **Carried state in deep zoom** (the explicit state makes it
   tractable): the reference orbit stores each carried field per step,
   beside z (phase 3, 5b's interleaving, generalised), and the pixel
   carries each field's deviation; Phoenix's two-term rebase is the
   precedent. Unlocks the twins of Phoenix, Manowar and Spider, and the
   convergent formulas' tests (Newton, Nova, Novaretti, Magnet: the
   test reads the pixel's own previous z).
6. **Phase 3's step 6, and guards**: transcendental functions and real
   powers by their difference forms (Exponential, Trig, Lambda-sine,
   Collatz, Tetration, Ducks, Feather; Multibrot with a non-integer
   power), function choices, branches in the start on parameters; and
   **guards**, a formula's checks at a pole or zero (McMullen, Kaliset,
   Lattes) or a branch on the iterate (Barnsley), declared so the
   derivation tests them on the pixel's full value rather than failing.
7. **Hybrids of any formulas**: a hybrid is a schedule (groups, repeats,
   loop start) of formulas' steps, z shared, each member's other state
   and parameters under its own name; KF's operators become one small
   library formula among the members. The designer composes; the
   result is one generated Rhai formula with a `CYCLE`.
8. **Parameters and scripting**: the metadata drives sliders and
   animation targets; a complex parameter animates as a point (x/y, or
   radius and angle); `mix(a, b, t)` and a weighted blend of two
   members give continuous morphs between formulas (linear, so they
   still derive); structural changes are held keyframes. Scripts:
   `formula(source)` with Rhai source, the parameters with their
   metadata, hybrids composed, and -- if Rhai's internals give a script
   function's definition (to verify) -- a formula from a script's own
   function. Later: a formula randomiser over the operations that
   derive.
9. **Later: native speed** -- BLA from the IR's Jacobian (2x2 real for
   folds), nucleus references through the IR's derivative, the Jacobian
   DE for folds; then the native code retires, family by family, and
   the twins become the definitions.

Probably native for good: Origami (fold lines computed on the CPU),
Littlewood, Digit Set.

### Phase 5: distribution

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
