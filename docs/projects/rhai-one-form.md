# One evaluated form for a Rhai body

Status: **planned 2026-11-09, not started.** Branch `rhai-planners`,
after its Parts A to E
([rhai-planners.md](../archive/projects/rhai-planners.md), archived).
It comes from a review of that branch, which read the plan record, the
reference docs and the module layout, not every line: the costs below
marked *measured* were, the rest are to be measured in step 0 before
anything is built on them.

## Why

A Rhai variation's body is compiled once to the language's typed tree
(`lang::ir`: `Program`, `IExpr`, `IStmt`). After Parts A to E that tree
is walked by five things, each with its own rules and its own tests:

| Walker | Where | What for |
|---|---|---|
| The scalar interpreter | `lang/interp.rs` (`Machine`, `VariationRun::call`) | one point: the reference of everything else |
| The batch interpreter | `lang/interp/batch.rs` (`call_batch`) | the walk's replays, inverses and arms, many points a call |
| The WGSL compiler | `lang/wgsl.rs` | the GPU's code |
| The difference derivation | `lang/delta.rs` (`variation_difference`) | the deep zoom's forms, as a new `Program` |
| The forward bound | `variations/derive.rs`, by way of WGSL and naga | Focused Rendering's forward planner; desktop only |

What that costs today:

- **Speed.** A twin's plan takes 1.17 to 1.26 times its built-in's
  (*measured*, E4b). The batch interpreter still walks boxed tree nodes
  and matches on a column's type at each one.
- **A Jacobian is five evaluations.** `map_forward_jacobians` takes a
  Rhai kernel's Jacobian by a five-point stencil of the forward map
  (`h = 6e-6 |x|`), where a built-in's is exact by dual numbers
  (`ifs_real::Dual`). It is slower than it need be, and it is an
  approximation where the built-in's is not.
- **The web has five bounds.** `derive` needs naga's WGSL front end,
  which the web build does not carry, so there `ifs_ball` falls to the
  five hand bounds (`bound::BOUNDS`) and refuses every other variation.
  The Rhai compiler is in the web build.
- **Two families of difference forms.** The tracker's P9
  ([deep-zoom-tracker.md](deep-zoom/deep-zoom-tracker.md)) plans to
  replace Focused Rendering's per-kernel forward difference forms by
  Jacobian chains. Parts C3 and E2 added derived forms for every twin
  beside the 15 hand-written ones, without deciding P9.

The numeric walkers (scalar, batch, dual, interval) can share one
lowered form. The two symbolic ones (WGSL, the difference derivation)
want a tree and keep it.

## Step 0: measure

Before any of it, on the built branch, each a printed number:

1. Where a twin's plan spends its remaining 20%: the batch
   interpreter's tree walk and type matches, or its buffers and masks.
   `profile_the_twins_plan` and `time_a_batch` exist; add a counter for
   nodes visited a point. If the tree walk is under half of it, step 1
   is worth less than it looks and the plan is revisited here.
2. The Jacobians' share of a plan (`step_jacobians`), which says what an
   exact dual pass can save.
3. On the desktop, how many of the 640 twins `derive` bounds through
   their WGSL today, and how long deriving one takes
   (`what_deriving_a_bound_costs`). This is the target the Rhai form has
   to match in step 2.

## Step 1: the instruction list

`lang/tape.rs`: a variation body lowered once to a list of typed
instructions over numbered registers.

- **Typed at compile time.** Every register is a float, a complex, an
  int, a bool or a vec3, known when the list is built, so no evaluator
  matches on a `Value` or a `Col` as it runs. An instruction is an
  opcode and register indices; constants and parameters are loaded into
  registers before the first instruction.
- **Structured control flow.** `If` and the loops stay as instructions
  that hold a range of the list, not jumps: the batch evaluator runs a
  range under a mask and the interval evaluator joins two ranges, and
  neither can do that with jumps.
- **Its scope is the batch interpreter's** (`batch::shape`): a
  variation's `fn variation`, `fn inverse`, `fn image_gap` and their 3D
  forms, with draws, state and the helpers those call. A body outside
  it stays on the tree interpreter, as it stays off the batch path
  today. Formulas, colourings, simulations and effects are not touched.
- Common subexpressions are shared when the list is built (the tree
  repeats them; the difference forms most of all).

Evaluators moved onto it, one commit each:

1. **Scalar**, `f64`. Gate: bit for bit the tree interpreter's result on
   every twin at the probe's inputs, draws and state included. The tree
   interpreter stays as the reference and as what everything else in
   the language runs on.
2. **Batch**, replacing `call_batch`'s tree walk and keeping its pooled
   buffers and masks. Gates: `a_batch_is_its_scalar_calls`,
   `the_batched_replay_is_the_point_replay`, the corpus's 32 fixtures
   through their twins; then the timings (`time_a_batch`,
   `time_the_twin_kernel_against_the_builtin`, `profile_the_twins_plan`,
   unprofiled, best of three). The aim is a twin's plan within 10% of
   its built-in's; the number reached is recorded whatever it is.
3. **Dual**, over `ifs_real::Dual`, replacing the five-point stencil in
   `map_forward_jacobians` and the inverse's. Gate: against the stencil
   within its own error, and against the built-in's exact Jacobian for
   each of the 21 twins with an inverse. This one changes results (an
   exact Jacobian for an approximate one), so the density gates at depth
   run after it (`the_newly_derived_twins_hold_at_depth`,
   `the_newly_derived_twins_render_as_the_builtins`).

## Step 2: bounds from the list, and on the web

An interval evaluator over the list, with `derive`'s `Interval` and its
rules (a draw is `[0, 1)`; a maybe condition evaluates both ranges and
joins; a loop iterates to a settled exit or refuses; state and poles
refuse).

- A **Rhai variation's** bound comes from its list on every platform,
  with no WGSL in between.
- A **built-in with a twin** takes its twin's bound on the web, where
  it has none today. That is sound only as far as the twin computes the
  built-in's function, which the twins' parity tests assert.
- The naga walk stays on the desktop for what has no Rhai body: the 7
  built-ins without a twin and variations downloaded as WGSL. It is not
  removed.

Gates, on the desktop, over all 640 twins: the list's bound contains
the images of sampled points of its disc (soundness); it is compared
with the naga walk's for the same variation (a table of ratios, and
every variation one bounds and the other refuses, by name); the corpus
flames the forward planner plans today still plan, with the same word
counts within the bounds' difference. Then a web build plans a flame
whose variation has no hand bound.

Open until measured: whether the list's bounds are as tight as naga's.
The WGSL bodies and the Rhai twins are written differently, and
interval arithmetic's width depends on how an expression is written.

## Step 3: decide P9, by measurement

Not a build. With exact Jacobians from step 1, run P9's own gate
offline on the CPU for the Rhai kernels and the built-ins: carry
`d = J d` along each reference chain in place of the difference forms
and report `the_offset_replay_holds_per_sample`'s 99th percentile at
1e4 to 1e8, with P9's second-order remainder (`hessian2`) beside it,
and the samples that cross a seam (an `atan2` cut, a `splits` axis)
counted apart.

It answers one question for the user: do the forward difference forms
in Focused Rendering go (P9 is built, and `lang::delta`'s forward
derivation with them), or stay. The Flame Attractor's inverse
differences stay either way; they are the escape engine's perturbation
and P9 does not reach them. No more forward forms are added until this
is decided.

## Step 4: the tests

- **Twin tests share the global registry.** A test that registers twins
  (`custom::ensure`) writes `global_registry_mut`, so two of them in
  one `cargo test` run race, and each must be run alone. Give them a
  registry of their own; where a code path reads the global one with no
  way to pass another, record it here rather than thread a parameter
  through the renderer for a test.
- **The juliascope-rays density gate** at 1e4 judges 28 blocks and
  allows one past five standard errors; alone it gives 0 or 1, beside
  other GPU tests it has given 2. Measure first whether more samples a
  render move the borderline block (if its excess shrinks with samples
  it was noise; if it does not, it is a real difference to find), then
  either raise the samples or fix what it found. The threshold is not
  loosened.

## Order

Step 0, then step 4 (small, and it makes every later gate cheaper to
run), then step 1's three evaluators, step 2, step 3.

## Decisions for the user

1. After step 0: whether the measured shares justify step 1.
2. After step 2's table: whether the web takes twins' bounds for
   built-ins.
3. After step 3: P9.

## Not in this plan

Ways to widen what the two engines take, and Focused Rendering in 3D,
are experiments for later:
[planner-experiments.md](../experimental/planner-experiments.md).
