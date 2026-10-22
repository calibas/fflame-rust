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
bug) and 2 (the Rhai front end) done 2026-10-14; step 3 (UF and
Fractint formulas and colourings converted to Rhai, Rhai colourings)
2026-10-15; step 4 (the built-ins' twins, Edit as Formula, branches in
the init derived) 2026-10-15; step 5 (carried state in deep zoom, one
value) 2026-10-15; step 6 (transcendental functions and powers by
difference forms, user functions, branches on the iterate) 2026-10-15;
step 7 (hybrids of any formulas) 2026-10-15; step 8 (parameters and
scripting; not held structural keyframes) 2026-10-15; step 9a (BLA for
holomorphic custom formulas) 2026-10-16; step 9b (real-linear BLA, for
folds and the conjugate) 2026-10-16; step 9c (nucleus references for
custom formulas) 2026-10-16; step 9d (the Jacobian DE; custom
derivatives under perturbation) 2026-10-16; step 9e (damping and the
biomorph on custom formulas) 2026-10-16.

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
   built-in with its nucleus reference, which the custom family had
   not until phase 4, step 9c). The reference identity rides in `map_params` as two normal
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

   Colourings done 2026-10-15. As built:
   - **Rhai colourings** have `finish` where a formula has `escaped`
     (`rhai::kind_of`).
     - `start(z, p)` gives the state (optional), `step(s, z, p)` updates
       it after each of the formula's passes (optional), and
       `finish(s, z, p)` returns the index, or `#{ index, solid }`, or
       an `if` whose arms give either.
     - `z` is the formula's iterate. A state field may be named `z`: in
       a colouring that name is the colouring's own.
     - The context gained `pixel` (for formulas too, so a converted
       formula reads `#pixel` in its loop without a state field) and
       `numiter` (in `finish`, UF's rule). `ctx.pass` is refused in a
       colouring.
     - `INFO.region` is `"inside"`, `"outside"` or `"both"`.
     - Two examples ship (`smooth.rhai`, `orbit-trap.rhai`). The editor
       checks a `.rhai` file as whichever kind it is.
   - **The conversion.** The state is what the checker packs. `final:`
     becomes `finish`, `#index`/`#solid` its locals, returned. When
     `final:` ends by setting `#index` and touches neither elsewhere,
     that value is returned directly. `init:` and `loop:` are left out
     when nothing they compute reaches the result.
   - **Measured on the library:** all 726 compiling colourings convert,
     compile back, colour as the original beside the Mandelbrot (index
     and solid at the same points), and keep the same state size; the
     formulas still 9,250 of 9,250.
   - **Not carried, by choice.** A converted formula keeps UF's view as
     its `INFO`, but a colouring has none (UF's colourings set no view).
     Direct colouring (`#color`, 410 uses in the library) still waits,
     as it does for UF source.
4. **Built-in twins and "Edit as formula"**: the Escape panel opens the
   current built-in in the editor as the user's own formula, with the
   current parameters and view. First the families the derivation
   covers now: Mandelbrot, Tricorn, Burning Ship (all six variants),
   Lambda, Cactus. Each held equal to its built-in by test, direct and
   perturbed. With it, from step 6: the derivation takes branches in
   `start` (the init) whose conditions are the same for every pixel --
   the plane, a parameter -- since every built-in has a Julia plane.

   Done 2026-10-15. As built:
   - **Branches in the init derive** (`delta::init_cases`). Each way
     through the init's branches on what is the same for every pixel is
     a case, with its variables' values and z's start (z substituted
     like any variable), at most 16. The equations branch on the cases'
     tests, one copy of the loop's each, or one when alike.
   - **An arm a parameter picks may go without equations.** Neither may
     an init case. The rest are derived, and the derived `perturb`
     setting says they hold when that arm is not taken. A branch whose
     arms all go without has none, so equations that could never hold
     are not offered. (Beyond the plan, needed for the Tricorn twin's
     power.)
   - Found on the way: an int converted to a float lost its conversion
     in the derived equations (an `i32` compared with `2.0`, invalid
     WGSL). The library's measurement now validates every derived
     formula's equations.
   - **`fn degree(p)`** (`Program::degree`, our own setting): a
     formula's degree at infinity, which the smooth colourings divide
     by and which offers their recommended bailout, as a built-in's
     `escape_degree`.
   - **The twins** (`escape::twins`, `assets/formulas/twins/`):
     Mandelbrot, Multibrot, Tricorn, Burning Ship (one formula, the
     variant a parameter, as the built-in), Lambda, Cactus. Each keeps
     its built-in's parameters by name, its `bailout` bound to the
     panel's, and the plane through `ctx`. Multibrot is beyond the plan:
     it renders, but its power is a real power, which deep zoom does
     not follow yet (step 6).
   - **Edit as Formula** (the Escape panel, beside the formula
     selector): the twin replaces the built-in in the config, one undo
     step. Parameters carry over by name, and the view and the
     colouring stay. The twin opens in the Formula Editor in use (Live
     re-applies its edits); the app opens the panel. Mann damping and
     the biomorph test are the built-ins' alone; the button's tooltip
     says so.
   - **Measured.**
     - Through the renderer with the smooth colouring, every twin
       matches its built-in on every pixel, on both planes and every
       Ship variant. The exceptions are Multibrot and the multicorns
       (under 0.4%), where the built-in raises to a power by polar form
       and the twin squares or multiplies.
     - At the visual suite's deep views, against the big-float truth at
       439 pixels, the twins count right as often as the built-ins'
       native tiers: Mandelbrot at zoom 42, 401 and 401; Lambda at 30,
       354 and 354; Tricorn at 28, 189 and 190; Burning Ship at 60, 346
       against the built-in's 294.
     - The UF library now derives equations for 501 formulas (308
       before), counting only those that hold at their defaults. The
       conversions keep every one.
   - Not here: retiring the built-ins (step 9); the twins as entries of
     the editor's examples.
5. **Carried state in deep zoom** (the explicit state makes it
   tractable): the reference orbit stores each carried field per step,
   beside z (phase 3, 5b's interleaving, generalised), and the pixel
   carries each field's deviation; Phoenix's two-term rebase is the
   precedent. Unlocks the twins of Phoenix, Manowar and Spider, and the
   convergent formulas' tests (Newton, Nova, Novaretti, Magnet: the
   test reads the pixel's own previous z).

   Done 2026-10-15 for one carried value. As built:
   - **What is carried:** a float or complex the loop writes and either
     reads in a pass before certainly writing it, or the test reads (so
     a convergent test's previous z, written before it is read, counts
     too).
   - **The derivation** (`lang::delta`): each carried variable gets
     three equation variables, listed in `Program::carried`:
     - the reference's record at the pass's start;
     - its current value at the reference through the pass (so a write
       in one arm reads right after the branch);
     - the pixel's deviation, kept from pass to pass.

     The init sets the deviation's start (per init case).
   - **The f64 model** (`interp::run_perturbed`): the references record
     each carried value per pass, the equations read the record, the
     test reads the full value, and a rebase moves the deviation as it
     moves `#dz`, gated on the pair's norm. A Phoenix, a sum carried
     through one arm, a convergent Newton and a carried temporary all
     run perturbed as they run, with and without rebasing.
   - **The GPU.**
     - The reference records the value after z, each pass, per
       reference (`BigMachine::carried`). The orbit's entries per pass
       are `R * W` (the tier's `width` W = 2; hybrids' R as before), and
       `custom_pass`, the step (`ORBIT_S`) and the phased rebase are
       generalised.
     - The equations take the record and a pointer to the deviation,
       which the template keeps in `w_prev`, where Phoenix keeps its
       history. On the deep rung it uses the Phoenix state tail.
     - The test takes the full value. The rebase moves the deviation by
       the records' difference (in double-float on the scaled rung;
       `fe_rebase_between` on the deep), gated on the pair's norm.
     - Such a formula perturbs on the floatexp rung at every depth
       (`deep_rung_always`), as Manowar does, for the reason measured
       there. The Manowar twin at zoom 26, against the big-float truth,
       counts right on 337 of 439 sampled pixels on the scaled rung and
       420 on floatexp, where the built-in Manowar counts 373.
   - **More than one carried value is refused** (`perturbation`), a
     limit of the GPU path alone: 79 library formulas carry two or
     three.
   - **The twins** of Phoenix, Manowar and Spider. Directly they render
     as their built-ins on every pixel. At depth, Phoenix (zoom 22.4 and
     48) counts right on 439 of 439 sampled pixels, as the built-in
     does, and Manowar on 420 against 373. A convergent Newton written in
     the language counts right on 439 of 439 at zoom 30.
   - **The library:** derived equations for 608 formulas (501 after
     step 4), every conversion keeping them. The leading blockers now:
     a user function of the iterate (2,307), a branch on the iterate
     (1,627), a real or complex power (1,255) -- step 6's ground.
6. **Phase 3's step 6, and guards**: transcendental functions and real
   powers by their difference forms (Exponential, Trig, Lambda-sine,
   Collatz, Tetration, Ducks, Feather; Multibrot with a non-integer
   power), function choices, branches in the start on parameters; and
   **guards**, a formula's checks at a pole or zero (McMullen, Kaliset,
   Lattes) or a branch on the iterate (Barnsley), declared so the
   derivation tests them on the pixel's full value rather than failing.
   In three parts:
   - **6a, done 2026-10-15: the difference forms.**
     - `exp`, `log`, `sqrt`, `sin`, `cos`, `sinh`, `cosh`, `tan`,
       `cotan`, `tanh`, `cotanh`, `cabs`, and a real or complex power
       whose exponent has no deviation, each by its form
       (`delta::difference_form`, the module docs list them).
     - `expm1` and `log1p` are new built-ins of the language (both
       dialects, Rhai too), exact near zero: the forms are written over
       them. On the GPU each takes a series near zero (`uf_expm1`,
       `fe_cexpm1` ...), and so does `sin` there: Vulkan lets `sin` err
       by 2^-11 absolutely, and the GTX 1660's returns exactly 0 below
       about 1e-7, which left every deep pixel of the Multibrot twin
       unescaped until `uf_cexpm1` took `uf_sin`
       (`the_difference_functions_keep_their_precision_on_the_gpu`).
     - **Guards at zero** (`Deriver::guarded`): a form that divides by
       something zero where the reference value A is (the power's
       `log1p(d / A)`, sqrt's and cabs's sums) is chosen per pixel --
       the plain difference where `|d| >= |A|`, which has nothing to
       cancel there, the form otherwise. Each guard is a statement
       before the one holding it; their temporaries are reused from
       statement to statement, and the equations' variables are kept
       out of the direct path's state (a formula of 250 parameter arms
       made 1.7 MB of WGSL and overflowed the stack before; 535 KB
       now, of which 426 KB is its own loop).
     - **Else-if chains**: a test that varies after tests that do not
       makes the rest of the chain their otherwise, so it can be left
       out whole, as UF's `else` + `if` already could (a Rhai `else if`
       lowers to one chain; one library formula lost its deep zoom in
       conversion over it).
     - The Multibrot and multicorn twins deep-zoom at any power: the
       Multibrot at power 4, zoom 30, counts right on 396 of 439
       sampled pixels against the built-in's 399. The sine map and a
       map taking sqrt, log, tan, cabs, expm1 and log1p at zoom 30:
       429 and 439 of 439. On the boundary itself, where pixels escape
       after thousands of chaotic steps, f32 deviations lose the count
       as f64 ones do, only sooner (198 and 343, the f64 model 352
       and 406): views for such tests sit just outside it.
     - The library: derived equations for 1,434 formulas (608 after
       step 5), every conversion keeping them (1,435 of 1,435). The
       leading blockers now: a user function of the iterate (2,713), a
       branch on the iterate (1,802), a branch in the init on the pixel
       (731), an int or bool carried (592), a power of a varying
       exponent (523).
   - **6b, done 2026-10-15: function choices.** A user function of
     the iterate (2,713 formulas stopped there first).
     - A user call's deviation is `IExprKind::UserDiff(f, A, d)`. Each
       choice's form is derived once, from the same rules
       (`delta::user_diff_forms`, a small program of its own), and
       emitted once per shader as a helper switching on the choice: on
       true values for the scaled rung (sin and sinh there by the
       helpers exact near zero), over CFe2 for the floatexp rung; the
       f64 model runs the same program. A choice without a form (the
       inverse functions, `cosxx`, the steps) is left out by the derived
       `perturb` setting, which reads the choice as an expression
       (`IExprKind::FuncChoice`, the parameter's slot on the GPU), so
       picking it renders on the direct path. (First built as an arm
       per choice inline: a library formula with four user calls in
       each of nine arms made 29,000 expression nodes; 2,100 now.)
     - **The cut**, found on the way: log, sqrt and a power not a whole
       real jump where A and A + d lie either side of the negative real
       axis, and their continuous forms did not. `delta::cut_turns`
       counts the turns; log adds them back, sqrt and the power take
       the plain difference, large there. Without it every deep pixel
       whose orbit straddles a preimage of the cut went wrong (a log
       pixel near 0.3 + 0.5i never escaped).
     - **Size**: a form that reads a value more than once reads a
       temporary (`Deriver::bound`); within an init case, a loop branch
       the case decides (a converted switch pair's plane) takes its arm
       alone, and cases whose equations come out alike share them; and
       the equations may take at most 20,000 expression nodes, past
       which the formula keeps the direct path. A library formula of
       hundreds of switchable arms with user functions in each had made
       147 MB of WGSL and overflowed the stack.
     - At zoom 30 a map adding `0.1 fn1(z)` counts right on all 439
       sampled pixels with sin and with log.
     - The library: derived equations for 3,064 formulas (1,434 after
       6a), every conversion keeping them (3,065 of 3,065); 29 past the
       size budget. The leading blockers now: a branch on the iterate
       (1,974), a power of a varying exponent (789), a branch in the
       init on the pixel (731), an int or bool carried (592).
   - **6c, done 2026-10-15: branches on the iterate**, which also
     covers the guards planned above (a check at a pole or zero is a
     branch on the iterate), with no declaration: the derivation takes
     every test at the pixel's full value as well as the reference's.
     - Each test at the reference's values and at the pixel's full
       values (reference plus deviation: comparisons of numbers, and
       `&&`, `||`, `!` of tests) gives each an arm. Where they agree,
       that arm's equations hold. Where not, the pixel's arm run from
       the entry state gives its full values, the reference's arm run
       from the same state the reference's, and each deviation is their
       plain difference, large there (`Deriver::iterate_branch`). An int
       or bool set in such a branch is refused (it would no longer be
       the same for every pixel).
     - Each arm's equations appear three times (shared, the pixel's, the
       reference's); the size budget counts them.
     - At zoom 30, Barnsley's test on Re z and a chain on both parts'
       signs count right on all 439 sampled pixels, on both rungs. Near
       Barnsley's boundary pixels escape after hundreds of steps of an
       expanding map, past what f32 (or, far enough, f64) deviations can
       follow; the f64 model test uses orbits under 45 steps for that
       reason.
     - The library: derived equations for 3,418 formulas (3,064 after
       6b), every conversion keeping them (3,419 of 3,419); 48 past the
       size budget. The measurement names what stops a branch on the
       iterate (its test on an int, a bool or the count: 40). The
       leading blockers now: the measurement's catch-all, an int or bool
       carried or a shape it cannot name (1,494); a power of a varying
       exponent (846); a branch in the init on the pixel (731); a test
       reading a variable the loop changes (645); a loop inside the
       loop (455).
7. **Hybrids of any formulas** (done 2026-10-15): a hybrid is a
   schedule (groups, repeats, loop start) of formulas' steps, z shared,
   each member's other state and parameters under its own name; KF's
   operators become one small library formula among the members. The
   designer composes; the result is one generated Rhai formula with a
   `CYCLE`.
   - As built: a group runs KF's lines or a formula (`hybrid::Member`,
     its Rhai source; the designer lists the built-ins' twins and the
     library's Rhai formulas). A hybrid with a formula is composed at
     the IR (`hybrid::compose`), not through UF text: each member's
     variables, parameters (`g<group>_<name>`, captioned "Group N:
     ..."), user functions and globals at their own offsets; every
     member's start in order, only the first's z kept; the first's
     test; the loop a branch on the pass. A group of KF lines is a
     member of its own (its program from the lines alone), its
     multipliers keeping their names. A member reading the pass is
     refused. The composed program prints as one Rhai formula with
     `INFO.cycle`.
   - Found on the way: the test of what a loop carries
     (`perturb::carried_vars`) counted a variable every arm of a branch
     writes as carried, so the Burning Ship twin -- alone, and in a
     hybrid -- rebased only at the orbit's end; it now takes the
     checker's flow (`check::flow`), as the derivation does.
   - Measured: the Mandelbrot's and Burning Ship's twins taking turns
     run as KF's lines taking the same turns, on both planes; a
     Phoenix member keeps its previous z across the Mandelbrot's turns;
     the composed program round-trips through its Rhai. At depth it
     counts as the lines do (103 / 105 of 439 at zoom 24, 83 / 84 at
     51). Those counts are low for both, which was open until
     2026-10-16: the walked edges of a fold map are chaotic. The Ship
     twin alone at its walked edge, zoom 12: the direct render counts
     93 of 439 as the big-float reference does, the built-in tier 105,
     the twin 104, and the paths agree with one another no better
     (built-in and twin on 109). Not a perturbation fault.
8. **Parameters and scripting** (done 2026-10-15, but for held
   structural keyframes): the metadata drives sliders and animation
   targets; a complex parameter animates as a point (x/y, or radius and
   angle); `mix(a, b, t)` and a weighted blend of two members give
   continuous morphs between formulas (linear, so they still derive);
   structural changes are held keyframes. Scripts: `formula(source)`
   with Rhai source, the parameters with their metadata, hybrids
   composed, and -- if Rhai's internals give a script function's
   definition (to verify) -- a formula from a script's own function.
   Later: a formula randomiser over the operations that derive.
   - As built:
     - `exponential` drives a logarithmic slider (the registry notes the
       leaked parameter definitions declared so:
       `custom::is_exponential`; no field on every built-in's
       definition).
     - A complex parameter `name` also reads and sets as `name.r` and
       `name.angle` (degrees), through the manager, the animation's
       apply and the script's `param` (`EscapeConfig::formula_param`,
       `formula_param_writes`); the target selector offers both, and the
       panel shows the point under the two parts. Two scalar tracks, so
       no change to the track model.
     - `mix(a, b, t)` in Rhai, lowered to `a + (b - a) t`.
     - A blend group (`Group::blend`): both formulas' steps from the
       same z, then `mix` by `g<N>_mix`; the designer's "Blended with".
     - Scripts: `escape.custom_formula(source)` (Rhai alone),
       `escape.formula_params()` (each parameter's metadata and value),
       `escape.hybrid(groups[, loop_start])` (maps with `twin` /
       `formula`, `blend_twin` / `blend`, `repeats`, or KF's `lines`),
       `escape.formula_fn(name)` -- a formula from the script's own
       function `f(z, c)`: rhai's `internals` expose the script's
       function definitions (`Module::iter_script_fn_info`), which the
       Rhai front end lowers as helpers inside a Mandelbrot-shaped
       wrapper; the result prints back as Rhai source for the config.
   - **Not done: held structural keyframes.** Keyframe values are JSON
     and a string already steps (halfway between keys, as every Step
     track does), but a formula's structure is two paths that must move
     together -- the source and the name compiled from it -- and the
     target selector offers neither. Needs a coupled target (the
     source, the name derived on apply) and hold-until-the-key
     semantics.
   - The randomiser stays later.
9. **Later: native speed** -- BLA from the IR's Jacobian (2x2 real for
   folds), nucleus references through the IR's derivative, the Jacobian
   DE for folds; then the native code retires, family by family, and
   the twins become the definitions.
   - **9a, done 2026-10-16: BLA for holomorphic custom formulas.**
     Measured first (`measure_the_custom_tier_against_the_native_one`):
     at zoom 42 with 8,000 iterations the built-in and the twin take
     the same 0.04-0.16 s, BLA or not; at zoom 60 with 30,000 (floatexp)
     the built-in takes 0.17 s with BLA and 0.80 s per step, the twin
     1.11 s. The twin's table now comes from its own derivative
     (`custom::bla_steps`; ESCAPE.md, BLA): 0.12 s. Refused where
     skipping is not sound (folds, branches on the iterate, carried
     state, phases, non-analytic user-function choices, tests other than
     z's modulus). The deep twins count as before against the truth
     (Mandelbrot 400 of 439 at zoom 42, Lambda 361, the Multibrot at
     power 4 400); the twin alone at the walked edges 374 and 393.
   - **9b, done 2026-10-16: real-linear BLA**, for folds and the
     conjugate (ESCAPE.md, BLA). A real-linear map is `P d + Q conj(d)`:
     table entries carry Q's for A and B (56 bytes on the GPU), merge as
     the maps compose, and the skip adds the Q terms in both shader
     rungs, through markers that leave the power tier's shader as it
     was. The single steps are now read from the formula's own
     perturbation equations, stepped in f64 at tiny deviations
     (`interp::PerturbStepper`; exact, folds included), not from 9a's
     derivative program; the radius is probed on a ring of directions,
     which finds folds' kinks and branch cuts -- a gap 9a had for log,
     sqrt and non-integer powers near their cuts. The built-in Ship and
     Tricorn tiers take their twins' steps. Interior views: the Ship at
     zoom 60, 0.81 s per step to 0.08 s, its twin 3.44 s to 0.19 s; the
     Tricorn at zoom 30, 0.14 s to 0.05 s. Early-escaping views pay the
     table's build once per orbit, up to ~60 ms (the Tricorn suite view,
     0.13 s to 0.19 s). Against the truth the deep views count as many
     pixels right as per step (the Ship at zoom 60: built-in 314 and
     twin 336 of 439, per step 294 and 346); in the suite views' chaotic
     regions a skip's 2^-24 changes which, so two baselines were
     regenerated.
   - **9c, done 2026-10-16: nucleus references** for custom formulas
     whose state is z alone (ESCAPE.md, Nucleus finding;
     `lang/periodic.rs`). The nucleus is a root of `z_p(c) - z_0(c)`,
     which the wrap needs; the period by a first-order ball, Newton
     through the formula's derivative in big floats, a double root (a
     formula that starts at its pixel: `G = h^2`) by its multiplicity.
     The reference at the nucleus is verified by its own closure
     detection, which any custom reference now has, as the plain map's;
     period hints and the panel's Detect period and Center on minibrot
     work for custom formulas. At the seahorse view (zoom 42, 100,000
     iterations) the twin's reference takes 0.034 s against 0.30 s
     plain; a search finding nothing adds up to ~0.15 s (detection is
     capped at Newton's budget, 20,000 periods). Found on the way and
     fixed: the scaled rung's rebase subtracted `Z_0` from the full
     value in f32, which broke formulas that start at their pixel (58
     of 439 right at the seahorse view at zoom 42, now 401).
   - **9d, done 2026-10-16: the Jacobian DE.** A formula that folds has
     a real derivative, `P d + Q conj(d)`, carried by a second
     companion per variable (`derive::with_jacobian`), and read by the
     colourings as `P + conj(Q) u^2`, the escape potential's gradient
     as a complex derivative reports it: distance estimation, the normal
     map and analytic relief for the Burning Ship's twin, Tricorns,
     folding hybrids and UF formulas with `@fn1 = abs` (whose derivative
     was zero before). Custom formulas also carry their derivative under
     perturbation now (one reference, nothing carried): the derivative
     program run at the full iterate, composed through BLA skips, held in
     extended range on the floatexp rung -- the twins' distance
     estimation at depth, which the built-ins had and they lacked. The
     nucleus search takes the Jacobian too: the Ship's and Tricorn's
     twins find their nuclei. Cost at depth: the formula's own pass each
     step, ~2.4x (the Ship's twin at zoom 60).
   - **9e, done 2026-10-16: damping and the biomorph on custom
     formulas** (ESCAPE.md), the two switches the twins lacked, as
     rewrites of the program (`lang::modifiers`): Mann damping blends
     each pass with z before it, alpha a parameter the panel drives; the
     biomorph rewrites `|z|` in the test as one part's square. A config
     resolves to the variant (`custom::resolve`) at the renderer's entry
     points, so every derived form follows, and damped or biomorph
     custom formulas deep-zoom, which the built-ins cannot. The twins
     render as their built-ins with either on.
   - **Speed (2026-10-16)**, measured twin against built-in
     (`measure_twins_against_their_builtins`; ESCAPE.md, How fast the
     twins are). The custom references were the gap: the nucleus
     search, a second orbit in big floats with the derivative program,
     cost 2 s before a 0.19 s reference at a Ship's interior; it now
     rides on the reference's build with the slope in f64, and Newton
     takes the slope in f64 too, in big floats only to finish a double
     root. Big floats keep their limbs inline (2.5x a pass); whole real
     powers by squaring (the Multibrot's twin 79 us a pass to 1.6). The
     Mandelbrot's twin is now faster than the built-in at depth. Custom
     references now go through the orbit store, their machines' state
     saved and restored from the formula's identity.
   - **Speed, the GPU's part (2026-10-16)**: the floatexp rung's
     generated code runs a real value on its real halves and a constant
     factor as a scale (the Ship's twin at zoom 60: 126 ms of GPU time to
     62; the built-in 34, on a single-f32 mantissa); the product rule
     takes two products, one for a square or for deviations the same or
     opposite (Lambda's `z (1 - z)`); a power whose exponent is a whole
     number from 1 to 64 derives by degrees, not by `expm1(b log1p(d /
     A))` (the Multibrot's twin 9 ms to 7, the built-in 5). And the BLA
     table backs off per pixel where its lookups keep failing -- for the
     built-ins too, which the table made up to twice slower where it
     could not skip (the Tricorn's suite view at zoom 28: 163 ms to 81).
     Left: the Ship at floatexp (a precision difference); the
     references' builds where no nucleus serves (big floats 1 to 2.4 us
     a pass, three times their arithmetic; the period watch's f64
     derivative pass beside them, the Ship's 2 us); the custom BLA
     table's build (26 ms against 6 for the Mandelbrot at zoom 60).
     Not pursued (decided 2026-10-16): a double-double reference
     machine, which would serve only to zoom ~34 (the precision policy
     asks zoom + 72 bits, double-double has 106) beside the big floats
     that serve every depth. Nor would the built-ins' fixed point help:
     its general multiply-add at 3 limbs costs 187 ns against the big
     floats' 80 (`measure_a_pass_of_each_twin`); the built-ins' speed is
     their hand-written in-place steps, and a big-float pass spends two
     thirds of its time interpreting. What would close the references'
     gap at every depth: the program flattened to in-place operations
     on preallocated big floats.
   - **Next: the order of work below** (agreed 2026-10-17), whose item 2
     is twins for every built-in that can have one and item 6 retiring
     (decided 2026-10-16). A native formula retires where its
     twin performs nearly the same or better; that is the last step,
     after converting as many as possible. Without twins today:
     McMullen, Kaliset, Barnsley, Exponential, Trig, Lambda Sine,
     Lattès, Ducks, Tetration, Collatz, Feather, Newton, Nova, Magnet,
     Novaretti -- each in the language already (complex arithmetic,
     branches on parameters and the iterate, the escape tests `Re z` and
     `|Im z|`, no test, a convergence test) -- and Origami (below). The
     fields (Weierstrass, Markus-Lyapunov, Standard Map FTLE) are mode B,
     a third entry kind the language does not have. Origami: considered,
     not started. Open: the deep
     rung's BLA at 2^-24 costs the floatexp rung's double float a
     little in chaotic views (the twin from zero at the seahorse view,
     zoom 42: 419 right per step, 405 with BLA).

The formulas thought native for good, examined (2026-10-16):
- **Littlewood, with its digit sets: converted.** The greedy digit is
  comparisons and a branch on the iterate; the twin renders as the
  built-in on every pixel, every set, both planes, and deep-zooms by
  derived equations where the built-in has no perturbed tier (439 of
  439 at zoom 30, sets `{±1}` and `{0, ±1}`, both rungs). "Digit Set"
  is Littlewood's parameter, not a formula of its own.
- **Origami: not yet.** What it needs that a formula cannot say:
  - a table built once per image -- 64 fold lines, line j's endpoints
    folded through lines 0..j-1, O(F^2) (per pixel it was a TDR);
  - an integer hash for the seed's lines (xor, shifts, wrapping
    multiplies), bit-exact on CPU and GPU;
  - its crease relief, `formula_creases(p0)`, a second function the
    terrain and the relief read beside the step;
  - its feature flags (non-escaping, the step's index, dynamical only)
    are already a formula's: no test, `#pass`, a start at the pixel.

  Two ways to give a formula the table: arrays in the language (a
  `global:` array, computed by the interpreter on the CPU as `global:`
  already is, read by index on the GPU), which UF formulas could use
  too; or a Rhai-only `fn data(p)` run by the Rhai engine itself on the
  CPU, its floats uploaded as the built-ins' `derived_data` is and read
  by a `data(k)` built-in -- no IR arrays, and Rhai's own bit operations
  give the hash. The crease relief would be a third function a formula
  may define. Decided 2026-10-17: arrays in the language, with
  everything else Origami needs (the order of work, item 3). Origami
  does not deep-zoom either way (its folds are isometries, ESCAPE.md).

### After phase 4: the order of work (agreed 2026-10-17)

Each item is ticked here when done; findings go in as they are made.

1. [x] **The pathological compile** (a bug, found 2026-10-17 measuring
   UF's library). Four entries of `as.ufm` -- Chaotic-fly (9,314
   lines), DaddyBrot (2,932), Insect3 (5,326), Insect3M (5,323): chains
   of `if`/`elseif` on parameters and nested user functions -- take
   over 60 s each through `measure_a_uf_corpus`, and the whole library
   overflows even a 64 MB stack (the test thread's 2 MB too). The 14 Oct
   build took 6 s for Chaotic-fly and 20 s for DaddyBrot, Insect3 and
   Insect3M already over 60 s; by 16 Oct 15:20 all four were over 60 s,
   and GenericBrot went from 0.5 s to 19 s (the per-entry folders and
   timings: `output/uf-one-entries.txt`, `output/uf-by-binary.txt`). In
   the app, opening such an entry hangs the Formula Editor, and the
   overflow crashes the process.
   - Find the stage (the check, the derivative, the derived equations,
     the conversion to Rhai, the WGSL); bound its work as it grows (a
     size budget charged while expressions are built, not after); a
     regression test from a generated formula of the same shape (the
     library is not ours to bundle, nor may a test read `output/`);
     compile off the UI thread if it is not already.
   - Then the library's measurement in full again. Seven eighths of it
     measured 2026-10-17: formulas 8,214 of 8,723 compile (94%),
     colourings 649 of 1,061 (61%; 35% before the state cap rose from
     4 floats to 32).
   - **Done 2026-10-17.** The stage was the derivative: a user function
     in a Jacobian (step 9d) expanded into an arm per choice at every
     call, and the Jacobian of these giants is large besides --
     DaddyBrot's 435 KB of WGSL had a derivative of 16.7 MB, Chaotic-fly's
     1.4 MB one of 54 MB, which naga took minutes over and overflowed
     the stack on. A user call is now `IExprKind::UserJac` (A or B of
     `A d + B conj(d)`, one helper, the forms each function's own rule on
     the unit pair): 54 MB to 20 MB, and a user call costs the
     derivative 1.5 times a fold's (the arms: 3.9 times). And a
     derivative adding more than 2,048 variables is refused
     (`MAX_DERIVATIVE_VARIABLES`): over the library half add 15 or fewer,
     99% at most 455; 57 giants of 9,250 go without. The largest now
     compile in 6 to 8 s, nearly all of it naga validating their own
     WGSL, and on a 1 MB stack (the desktop main thread's). The library
     in full, 206 s: formulas 9,250 of 9,831 compile (94%), colourings
     726 of 1,209 (60%), 9,193 with a derivative, 3,417 deep-zoom, every
     conversion to Rhai round-tripping.
   - Open, smaller: the editor still compiles on the UI thread, so those
     few giants freeze it for their 6 to 8 s; and a derivative's
     temporaries are fields of the per-pixel state (`FormulaState`), not
     pass-local values -- DaddyBrot's had 12,890 -- which locals would
     shrink for every formula with a derivative.
2. [x] **Twins for the 14 built-ins without one** (done 2026-10-17:
   every built-in formula but Origami has one; Origami's came with item 3).
   - First, in the language and tests (the first three done 2026-10-17:
     `perturb::settle_of` and `formula_settled` /
     `formula_perturb_settled`; `escaped` optional, `never_escapes`, the
     Bailout control by the program's `bailout`; the biomorph's
     `is_metric_of_z`):
     - **converged**: the built-ins' convergent formulas set
       `converged` beside `escaped`, which Root Basin's general key
       reads; a program's test only ends the orbit. Where a test is
       "escaped or settled", the settled half sets `converged`.
     - **No escape test**: `escaped` may be left out of a Rhai formula;
       such a program is non-escaping (`coloring_suits_formula`'s gate
       applies), and the Bailout control hides where a program has no
       `bailout` parameter; the twin test's bailout rule follows.
     - **The biomorph on `Re z` and `|Im z|` tests**, which the built-ins
       offer on any metric (the rewrite now replaces `|z|` only).
     - **Guards**: the built-ins' helpers return 1e20 for a division by
       zero (`esc_cdiv`), -34.5 for log 0, clamp `exp` at 80, and take
       powers in polar form; the language's follow UF. Per twin, the
       guard written out where it changes the picture, else a measured
       tolerance.
   - Batches: (1) Exponential, Trig, Lambda Sine, Collatz, Tetration
     (no native deep tier, so they gain deep zoom; Tetration written
     `exp(z ln c)`, since `c^z`'s exponent varies and has no derived
     form) -- **done 2026-10-17**: every pixel as the built-in's (Tetration
     99.8%), deep at zoom 30 against big floats 430 to 439 of 439 on
     both rungs; exp-like loops perturb on the floatexp rung
     (`grows_past_f32`); (2) Feather, McMullen (native tiers to compare; McMullen's
     on the Julia plane only) -- **done 2026-10-17**: at depth as the
     native tiers (Feather 439 and 439, McMullen 431 and 437), after the
     custom tier took the built-ins' delta-aware escape margin for a
     `|z|^2` test (`modulus_bound`; Feather's twin was 219: a band's edge
     crossed its suite view); (3) Barnsley, Kaliset, Ducks, Lattès
     (branches on the iterate, folds, three non-escaping) -- **done
     2026-10-17**: every pixel as the built-in's (Ducks' variant 0 98.4%,
     a chaotic map's rounding); Barnsley deep at zoom 30 against big
     floats 426 to 439 of 439, the three non-escaping by the orbit's mean `|z|` against big
     floats, both rungs, under 2e-7 mean relative error (Ducks' scaled
     rung 7.5e-5, below). It took: the derivation of `arg` (Im of
     log1p, the cut's turns) and of `log |a|^2` (2 Re log1p); a complex
     divided by a lifted real part by part, as the built-ins; big floats'
     real log without the complex one's sentinel; `log1p` near -1 by
     the log of `1 + z` itself (the squared form cancelled to log 0 on an
     orbit grazing a log's singularity: Ducks' floatexp rung read inf);
     and the floatexp rung's log, arg and log1p by mantissa and exponent;
     (4) Magnet,
     Nova, Novaretti, Newton (convergent; Newton 6 functions by 3
     schemes) -- **done 2026-10-17**: Magnet as its built-in on every
     pixel; the others' powers round otherwise (`exp(p log z)` against
     polar), and the settle test's pass is f32 rounding's, so their
     counts part by one on up to 30% of pixels, off f64's by more than
     one no more often than the built-ins'. At the native tiers' deep
     views 439 of 439 each (Magnet's native tier 356); at basin
     boundaries within one of f64's own count but `z^p sin z - 1`'s (410
     against 437; item 6). It took: a test that only settles never
     escapes (Novaretti); a loop variable holding one value at every
     pixel derives as one (`delta::uniform_vars`: `let k = p.power; z **
     k`); a carried value only the test reads rebases on z's deviation
     alone (`Perturbable::pair`; Newton's twin at a `z^3 - 1` boundary
     was 377 of 439, now 436, the native 439).
   - Each: renders as its built-in on both planes (where it has two),
     and deep views against the big-float truth, beside the native tier
     where there is one.
   - Not twins: Origami (item 3); the fields, Weierstrass,
     Markus-Lyapunov and Standard Map FTLE (mode B: a third entry kind,
     later).
3. [x] **Arrays, and everything Origami needs** (decided 2026-10-17;
   done 2026-10-17, steps 3a-3f below. Sized by `#maxiter` became
   allowed within the same 256 floats an array has, not refused as
   planned; screen-sized arrays, per-pixel orbit buffers and dynamic
   arrays are not done, by the library's measurement, 3f).
   - **Arrays in the language**, both dialects: global arrays (UF's
     `global:`, Rhai's `prepare`), computed on the CPU once per image as
     `global:` is now and read by index on the GPU; small fixed
     per-pixel arrays within the state cap; arrays the length of
     `#maxiter` or of the image refused, as no GPU pixel can hold them.
     UF's static and dynamic arrays (`setLength`, `length`), Rhai's
     arrays. Reach: 131 formulas and 131 colourings stop at arrays first
     (seven eighths of the library).
   - **Integer bit operations** for Origami's hash: xor, and, or,
     shifts, a wrapping 32-bit multiply (Rhai's operators; UF has none),
     bit-exact with the built-in's `origami_hash`.
   - **The crease relief as a formula's function**: a Rhai `fn
     creases(p0, p, ctx)` returning the height and its slope, as the
     built-in's `formula_creases`, reading the panel's crease settings;
     the Creases relief field and terrain source offered for a formula
     that defines it. Chosen over a crease colouring, which would lose
     the exact slope and the panel's settings.
   - **Origami's twin**: its lines in `prepare`, a fold a pass by
     `ctx.pass`, no escape test, dynamical only, its creases; renders
     as the built-in, relief and terrain included.
   - **UF's rules** (its help, "Arrays" and "Dynamic arrays"): a static
     array `int a[8]`, `float a[10, 2 * 6 - 2]`, each dimension a
     constant integer expression -- parameters and most predefined
     symbols count as constants; indices zero-based integers, in the
     declared order (`a[3, 5 - 3]`); not initialized on declaration
     (we zero them, as WGSL does); same-size arrays assign whole
     (`x = y`); a constant index out of range is a compile error, a
     computed one a run-time message under `$define debug` only (the
     GPU clamps). Dynamic arrays (`int a[]`) are one-dimensional,
     `setLength` sizes them and zeroes new elements, `length` reads the
     size. Used in the library (survey 2026-10-17, 1,830 declarations
     in 331 entries): `global:` 1,278 (colourings 903), `init:` 475,
     `loop:` 38, `final:` 41; float 851, complex 506, int 360, bool
     77, color 38; one dimension 1,441, two 317, three 14, dynamic
     60; sizes constant 1,061, a parameter or `#maxiter` most of the
     rest, the image's (`#width`) a few.
   - **Steps** (order decided 2026-10-17):
     - **3a.** Constant-size static arrays in UF: the IR (`Var::dims`,
       `IExprKind::Elem`, `IStmt::SetElem`), the checker, the
       interpreter, big floats, both GPU paths. Global arrays packed
       after the scalars into `fdata` (formulas, 256 floats) or the
       parameter block (colourings, 64 with the parameters, until item
       4 grows it); per-pixel ones in the state (a colouring's within
       its 32 floats). Deep zoom and the derivative take a global
       array read at an index the same at every pixel (a parameter,
       the pass) as a parameter; anything else with an array refuses
       them for now. **Done 2026-10-17**: an array formula renders as
       the built-in Mandelbrot on every pixel, direct and at zoom 20 by
       derived equations reading its tables; an array colouring as its
       scalar twin on both paths; the interpreter's array programs as
       their scalar ones, the clamp included. A write at a constant
       index defines that element for the pass (`Flow::written_elems`),
       so a scratch array is no colouring state. Found on the way: a
       factor of one read at run time (`z^2 * k + c`, array or scalar
       global alike) moves 0.03% of the Mandelbrot's pixels, the
       driver contracting the product otherwise -- the language's
       arithmetic, not the arrays'. The library after it (2026-10-17):
       formulas 9,267 of 9,831 compile (9,250 before), colourings 728 of
       1,209 (726); every one's WGSL validates. First errors left that
       are arrays': sized by a parameter or a predefined symbol 160
       entries (74 formulas, 86 colourings), a size not a constant
       integer expression 28, an array past 256 floats 20 colourings;
       17 formulas and 2 colourings compiled and stopped converting to
       Rhai (converting since, 3b). So 3f's sizes from parameters reach
       the most of what is left of arrays.
     - **3b.** Rhai arrays (a literal, a sized constructor, `a[i]`,
       `a.len()`, `p.name[i]` from `prepare`), and UF arrays converted
       to Rhai. **The Rhai half done 2026-10-17**: `[a, b]`, `array(n,
       v)`, indexing in `prepare`, `start`, `step` and `escaped`, a state
       field or `prepare` value an array, `let b = a` and UF's `x = y`
       copying whole (`Checker::copy_array`); one dimension (arrays of
       arrays refused). A Rhai formula with every form runs as its UF
       twin. **The conversion done 2026-10-17** (`lang::to_rhai`): an
       array declared as `array(n, zero)`, a global one built in
       `prepare` and read as `p.k[i]`, a carried one a state field, the
       checker's element loop for `x = y` printed back as `s.prev =
       s.last` (`Printer::copy_of`), a two-dimensional one flattened as
       the checker flattens it (`m[1, 2]` of `[2, 3]` is `m[5]`). The
       library again converts every compiling entry (2026-10-17): 9,268
       of 9,268 formulas and 728 of 728 colourings convert, compile back
       and run as the original; deep zoom kept 3,425 of 3,425.
     - **3c.** 32-bit integers as UF's (wrapping `+ - *` in the
       interpreter, as the GPU's), and Rhai's bit operations (`^ & |
       << >>`, logical shifts on the 32-bit pattern), bit-exact with
       `origami_hash`. **Done 2026-10-17**: the interpreter's ints were 32-bit and
       wrapping already; the bit operations are `BinOp::BitAnd` ..
       `Shr` (`interp::bits`, the GPU's the same), and Origami's hash in
       a formula gives `formulas::origami_hash`'s bits, on the GPU too.
     - **3d.** The `creases` hook (`FormulaFeature::Creases` for a
       program defining it). **Done 2026-10-17**: a Rhai `fn creases(p0,
       p, ctx)` ending with `#{ height, slope }` lowers to the IR's
       creases section (`Program::creases`, `Place::CreaseHeight` and
       `CreaseSlope`, the settings `Predef::CreaseDecay` ..), which reads
       its own variables, the parameters and the globals (no iterate, no
       pass); the GPU's `formula_creases` and `interp::creases` run it.
     - **3e.** Origami's twin. Its lines are the built-in's in f32 on
       the CPU; `prepare` runs in f64, so they differ at the rounding
       (measure, then decide). **Done 2026-10-17**: measured, the f64
       tables parted by whole units after the first few lines (each
       folds through every earlier one), so the language took `f32` /
       `to_f32` and the twin's `prepare` computes in f32 an operation at
       a time: its table is the built-in's bit for bit at four seeds and
       spreads. It renders as the built-in on every pixel at two views,
       and its crease field matches the reference as the built-in's does
       for every pattern, the flip and a terrain.
     - **3f.** For the library: dynamic arrays (`setLength`, `length`)
       and arrays sized by a parameter or `#maxiter`, within the space
       left. (Whole-array assignment came with 3b.) **Measured first**
       (2026-10-17, `scripts/uf_array_sizes.py`): of about 250 library
       entries declaring a size that is not a literal, the reachable
       ones are tables sized by a parameter or `#maxiter` (global or per
       pixel, small); about 35 colourings size a `global:` array by the
       screen (buddhabrots, attractors, flames: the whole render in
       `global:`, millions of orbits, minutes in our interpreter per
       change, and UF's own comments warn against them for disk
       renders); 48 colourings keep every iterate per pixel
       (`trace[#maxiter + 1]`), which needs the whole orbit in each GPU
       thread (a `var<private>` table once cost enough occupancy to trip
       the TDR watchdog; the chunked render's state holds 32 floats);
       and every dynamic array is in an entry that `import`s (item 5).
       **Done 2026-10-17 for parameters and `#maxiter`**: a size is a
       constant expression of literals, parameters, `#maxiter`, `#pi`,
       `#e` and the built-ins (UF's rule), folded at the settings
       (`check::SizeSettings`, `check::at_settings`; `Var::size` keeps
       it as written, `Var::dims` folded). The entry registers at its
       defaults (its `maxiter`, else the panel's 256); `custom::resolve`
       names a variant for other values (`sizes_of`), compiled on first
       use, formula and colouring alike, with damping and biomorph. A
       stride the settings fix is read as written, so the index means
       the same at every size and Edit as Rhai prints the size and the
       index as written (`array(2 * p.n, ...)`); Rhai's `array(n, v)`
       takes such an `n`, and `len` is it. A `switch:` target's sizes
       read the pair's parameters (`splice` remaps them; they were the
       target's parameter numbers) and fold at the values those hold in
       the pair. A variant that does not fit (past the 256 floats, say)
       leaves the entry at its default sizes, and the Escape panel says
       why (`custom::size_failure`). Found on the way: Rhai's tree keeps
       what follows an index inside it (`a[i].re` is `Index(a, Dot(i,
       re))`, without the `BREAK` flag `a[s.i]` has), and the lowering
       read `i.re` as the index -- latent since 3b, reached by the
       library's complex tables. Measured: the GPU renders a formula and
       a colouring at non-default sizes (a parameter and `#maxiter`) as
       the built-ins they reduce to, converted to Rhai too. **The
       library after it** (2026-10-17): formulas 9,304 of 9,831 compile
       (9,268), colourings 731 of 1,209 (728); every one converts to
       Rhai, compiles back and runs as the original. First errors left
       that are arrays': per-pixel orbit buffers 45 colourings (`this
       array takes 514 floats`, at 256 iterations), screen-sized 47
       entries, `global:` tables past the 256 floats about 20 (a bigger
       `fdata` would reach them: item 4's budgets). Unknown functions
       rose to 52 formulas as entries got past their arrays, UF's
       `random(seed)` the most (item 4; its generator is UF's, not to be
       guessed). **Not done**, by the measurement: screen-sized arrays,
       per-pixel orbit buffers, dynamic arrays.
4. [ ] **Paused 2026-10-17, with all further UF support (the user's
   call):** not as cheap as it looked -- the unknown functions are a
   vague list, and fixing an entry's first error says nothing about
   its next. UF support is revisited later; item 5 waits with it.
   **The cheap UF items**: parameters past 64 floats (31 formulas, 47
   colourings stop there first); colouring state past 32 floats (31);
   the smaller mismatches, triaged by message (about 150 entries: used
   before assigned 29, unknown function 24, a parameter written 20, an
   argument count 17, unknown symbol 28, ...); UF gradients (`.ugr`) as
   palettes.
5. [ ] **Classes and plug-ins: the user decides after item 4.** What they
   would reach: `import` stops 197 formulas and 97 colourings first;
   the `.ulb` libraries hold 3,284 classes (12 MB) -- about 1,040
   formulas (`DivergentFormula` 856, `ConvergentFormula` 138, and
   kin), 308 trap shapes, 165 transforms (UF's transformations live
   here now: the library has no `.uxf` files), about 150 colourings --
   reachable only through classes. The model: class-typed parameters
   (`Formula param formulaClass1 ... default = Standard_Mandelbrot`),
   objects made with `new` in `global:`, arrays of objects, virtual
   calls (`Init`, `Iterate`, `IsBailedOut`), inheritance. Large: needs
   arrays, then objects as structs, methods, plug-in parameters with
   their own nested parameters in the panel, devirtualised at compile
   time by the chosen class. Not worth much, by the library's count:
   `visible=`/`enabled=` and headings (parsed and ignored: such
   parameters always show), image import (9 files), `#calculationPurpose`
   (2), `builtin:` (1). Other UF files: `.upr` parameter files (medium;
   they need the formulas they name).
6. [ ] **Retiring native formulas, last**, each where its twin performs
   nearly the same or better (decided 2026-10-16). Before it:
   - **Custom orbits compress in the store.** The built-ins' orbits
     are replayed by a deterministic double-double shadow of their map
     (`shadow_step`), so a stored file keeps only corrections: a 202 MB
     orbit becomes a couple of MB. A custom orbit is stored raw, 20 B an
     entry. The same shadow, run from the program, would compress
     custom orbits: the one place double-double earns its keep.
   - **The references' speed**: the program flattened to in-place
     operations on preallocated big floats (a pass spends two thirds of
     its time interpreting).
   - **Newton's `z^p sin z - 1` at a basin boundary** (zoom 30): the
     twin counts 410 of 439 within one of the big-float count, f64
     unperturbed 437, the derived equations' f64 model 437, both rungs
     alike -- f32's rounding of what the rungs share, where the pixels
     settle and their reference, its sin growing as `e^|Im z|`, wanders
     on. No native tier to compare. Leads: the reference's sub-values in
     double float; a pixel far from its reference iterating on its own.
   - **The scaled rung's reference values are f32.** A derived equation
     reads the reference's own sub-values (Ducks' `t = f(Z) + c`)
     recomputed from Z's high half; where such a sum cancels -- Ducks'
     reference passing 2.6e-7 from its log's singularity -- f32 keeps
     little of it, and the pixels by it are off: the Ducks twin at its
     deep view, 42 of 6,912 pixels over 1e-3 (mean 7.5e-5) where the
     native tier, which reads `ref_z_lo`, has 1.1e-6. The floatexp rung
     reads the double-float reference and is exact (1.1e-7). A fix:
     reference sums in double float on the scaled rung, or the floatexp
     rung where a program's equations hold such a sum.
   - **Fixed point's limits**, for the record: magnitudes below 2^8,
     absolute precision 2^-(64n - 8), division only by values of size 1
     or more, and an overflow that wraps silently in release builds (the
     carries are `debug_assert`ed). Big floats: an i64 exponent, 64n
     bits relative, no overflow. A lead to check, unverified: a
     Multibrot or Tricorn reference at power 9 or more could wrap on the
     step before escape (|z| <= 2 then, and 2^9 > 2^8). Fixed point
     stays for the view's decimal centres and the cameras.

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
