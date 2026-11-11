# One evaluated form for a Rhai body

Status: **paused 2026-11-10, the user may revisit.** Built and merged
with `rhai-planners` (PR #156, 2026-11-10): step 0's measurements, step
2 (route B, the user's pick: bounds from the Rhai code on every
platform, the web's built-ins through their twins) with the forward
search's time budget, and step 4's registry race. Step 1 is not built
(route B needs no instruction list). Open: step 3 (decide P9 by
measurement) and step 4's juliascope-rays 1% difference at 1e4. Branch
`rhai-planners`,
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

## Step 0: measured (2026-11-09)

On the desktop: an i5-10400F, 6 cores and 12 threads. Each number comes
from an ignored test, kept so that it can be measured again.

### 1. Where a twin's remaining 20% goes: not the tree walk

- **The kernel, one thread** (`time_the_twin_kernel_against_the_builtin`):
  julian's twin batched against the built-in's native root kernel, by
  batch size: 1.76x at 16 points, 1.18x at 64, 1.10x at 145, 1.03x at
  256. Its body is 9 statements and 37 expression nodes.
- **The plan's batches** average about 205 points: at 1e2, 37.5M points
  in 182,703 forward batches. The other views are the same.
- **The plan, one thread** (`profile_the_twins_plan`, its one-thread
  rows): twin and built-in take the same time.

  | View | Built-in | Twin |
  |---|---|---|
  | 1e2 at 0.75 | 3.34 s | 3.35 s |
  | 1e4 at 0.75 | 6.30 s | 6.30 s |
  | 1e4 at 0.25 | 13.46 s | 13.11 s |
  | 1e6 at 0.75 | 4.90 s | 5.12 s |

  Forward batches are 83-86% of the twin's plan, at 73-74 ns a point
  against the native kernel's 70.
- **The plan, by threads** (`scale_the_twins_plan`, best of three), the
  twin's time over the built-in's:

  | Threads | 1e4 at 0.75 | 1e4 at 0.25 |
  |---|---|---|
  | 1 | 1.02x | 0.97x |
  | 2 | 1.00x | 1.00x |
  | 4 | 1.03x | 1.00x |
  | 6 | 1.07x | 1.03x |
  | 8 | 1.11x | 1.07x |
  | 12 | 1.16x | 1.16x |

- **The kernel on every thread at once**, 256 points a batch: native
  113.6 ns a point a thread, the twin 128.7 ns (1.13x, against 1.03x on
  one thread).

So the 1.17x appears only where two threads share a core. The batch
writes every node's column to memory and reads it back; the native
kernel keeps its values in registers. Two hyperthreads share a core's
L1 and L2 caches and its load and store units, and the batch's memory
traffic is what they compete for. Run alone, the interpreter costs
nothing at the plan's batch sizes. A typed instruction list would cut
dispatch per node, and dispatch is not what this costs.

### 2. The Jacobians' share: 1%

`step_jacobians`, counted as its own kind of call: 0.035-0.157 s of the
twin's one-thread plan, 1% at every view, and 0.1-0.2% of the
built-in's. An exact dual pass would make a twin's Jacobians exact, as
the built-ins' are; it would not make the plan noticeably faster.

### 3. Bounds today (`what_twins_derive`)

On the desktop, each built-in's bound from its own WGSL against its
twin's bound from the twin's compiled WGSL, at one disc (centre
(0.6, -0.4), radius 0.25), with borrowing off:

- 542 of the 640 twins derive a bound. 540 built-ins do, and every one
  of their twins does too. Two twins (`julia`, `glsl_circuits`) derive
  where their built-in does not. 98 derive neither way.
- Where both derive, the twin's disc over the built-in's: median 1.000,
  90th percentile 1.000, 99th 1.018, worst 1.122 (`sym_ng7`; the
  widest ten are the `sym_*` family). 519 of the 540 are as tight or
  tighter.
- A derivation takes 12-14 ms either way, nearly all of it naga parsing
  a module the first time.

**Borrowing.** In the app an unedited twin borrows its built-in's
planner data (`twins::planner_name`), its bound included. On the web
the built-in has only the five hand bounds, so an unedited twin has
only those too. Bounds from the Rhai form would reach the web's
built-ins through their twins, edited twins, and users' own
variations.

### What step 0 changes

- **Step 1's speed case is gone.** What remains of the gap is memory
  traffic under hyperthreading. What might close it is evaluating a
  batch in blocks small enough to stay in L1 (32 or 64 lanes); today
  that costs dispatch (1.18x at 64 points on one thread), and that is
  where a cheaper dispatch would help. It is unmeasured and is an
  experiment, not a step: chunk `call_batch` in the current interpreter
  and time it at 12 threads first.
- **Step 1's other case, one form for four evaluators, stands, but
  smaller.** Of the four, the scalar and batch evaluators already
  exist and are at parity; the dual evaluator buys exactness at 1% of
  the time; the interval evaluator is the one the web needs.
- **The web's bounds have three routes:**
  - A. The naga walk on the web. wgpu on the web carries no naga (the
    browser takes WGSL), and `derive` builds its module through the
    probe's shader assembly, which is desktop only. The cost is naga's
    WGSL front end and the probe's assembly in the web build: wasm
    size, not measured.
  - B. An interval walker over `lang::ir`, the tree, with `derive`'s
    rules. No naga, no new form, one more walker.
  - C. The instruction list as planned, its interval evaluator a loop
    over it.

  Each of the three would be judged against the census above: 542 twins
  deriving, and discs as tight as these.

## Step 1: the instruction list -- not built (decision 1: route B)

The user chose route B on 2026-11-09: no instruction list. What follows
is the step as it was planned, kept for the record of what was weighed.

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

## Step 2: bounds from the Rhai code (route B) -- built (2026-11-10)

An interval evaluator over the Rhai variation's own tree, `lang::ir`
(`variations::rhai_bound`), with `derive`'s arithmetic: `Interval`, its
rules and the box-to-disc step moved out of `derive.rs` into
`variations::interval`, which the web builds too (`derive.rs` re-exports
them, so its paths are unchanged).

**The rules are `derive`'s**, over the tree's statements instead of
naga's: each value the closed range it takes over the input box,
widened for f32; a draw `[0, 1)` or a pinned arm's slice; a maybe
condition runs both arms and joins them, a return, `break` or
`continue` inside one too; a loop runs until its exit is certain and
refuses past 512 rounds (the GPU's cap is 4096); a pole or a domain exit
is a value with no bound, refused only where it reaches the point; state
`init` writes refuses where it is read before it is written; the colour
a call is given is unknown (a point that depends on it refuses). Arrays
are tracked element by element, an index that is a range reading or
writing each element it can reach. `prepare` runs through the same rules
at the flame's parameters, so a value it picks stays one number (eJulia's
`sign == 1.0` is decided), with the interpreter's widened numbers where
the rules refuse it.

Three things the tree needed that naga's walk did not, each found by the
census below:

- **Narrowing by a condition.** `max(a, b)` lowers to `t = a; if b > t
  { t = b; }`. With the test undecided, both arms joined give the hull
  of `a` and `b`, not the max, so `1 / max(r, 1e-6)` reached zero. Each
  arm now narrows the variables its comparison sets against a value
  (`b > t` holding: `t` no higher than `b` can be). This removed every
  pole and domain refusal the census had (109 + 79 discs).
- **A value times itself is its square** (`z.re * z.re`), never
  negative; two draws are two values.
- **A complex times or over a real** scales each part: spherical's
  `z / r2` was `x c / c^2`, twenty times too wide at the census's
  widest disc.

### Where the bounds come from (`ifs_ball::derived`)

| | Desktop | Web |
|---|---|---|
| A built-in | its WGSL, by naga (as before) | its twin's code |
| A Rhai variation | its code; its compiled WGSL by naga where that refuses | its code |
| A built-in with no twin (7) | its WGSL | none: refused, as before |

The five hand bounds still come first, on both.

### The census (`what_the_twins_bound`)

Every twin at its built-in's defaults, at four discs (centres (0.6,
-0.4), (0, 0), (-1.5, 1), (2, 0.3); radii 0.25, 0.5, 0.1, 1): 2,560 discs.

| | Discs |
|---|---|
| Both bound | 2,080 |
| The tree alone | 128 |
| Naga alone | 0 |
| Neither | 352 |

- Where both bound, the tree's disc over naga's: median 1.000, 90th
  percentile 1.000, 99th 1.057, worst 1.360 (`sym_bg5`; the widest are
  the `sym_*` and `crop` families). 1,674 of the 2,080 are as tight or
  tighter.
- **Sound at every disc:** each tree bound holds the interpreter's image
  of a 9 x 9 grid of its box at 16 draws; a miss fails the test.
- A tree bound takes 0.07 ms; naga's 12-14 ms, nearly all parsing.

### Planning the fixtures both ways

`the_web_route_plans_the_fixtures_as_the_desktop` (ignored; sets a
global switch, `ifs_ball::WEB_ROUTE`, that sends every variation down
the web's route on the desktop, so it runs alone): 171 2D fixtures (the
deep zoom corpus in `output/flame-zoom`, the visual suite's 2D and
per-variation configs) at their own framing and 64, 1024 and 16384
times nearer, 684 views.

| | Views |
|---|---|
| Both plan | 161, every one with the same words |
| The desktop alone | 0 |
| The web's route alone | 2 (glynnlissa, glynnspiro, at their framing) |

(The corpus of `.flame` files `how_many_corpus_flames_enumerate` reads
from `output/` is not on this machine; these fixtures stand in.)

**The time.** Planning all 684 views took 604 s on the desktop's route
and 757 s on the web's. Per view:

- Where both routes come to the same outcome, the web's took 212 s
  *less* in all: a tree bound is cheaper than naga's walk
  (`time_a_bound`: julian 4.2 us against 12.8, jac_asn 0.44 ms against
  0.52, minkowskope 0.17 ms against 0.18).
- All the extra time, 366 s, is where the outcomes differ, and the web's
  route gets further before refusing. jac_asn, truchet2 and glynnspiro
  have no bound from naga, so the desktop refuses them at once
  (`Unbounded`, `NoInvariantBall`); the tree bounds them, and the
  forward planner enumerates until it passes `MAX_WORDS` (jac_asn: 47 s
  a view, about 100,000 bounds at 0.44 ms, to refuse `TooManyWords`).
  minkowskope reaches the cap on both routes, after 10.8 s on the
  desktop's and 62-67 s on the web's: its tree discs are wider at the
  discs the walk asks about, so more words survive to be bounded.

So the cost is the forward planner's search, which had no time budget
and stopped only at its word cap, now reached by variations that used to
be refused for want of a bound.

### The forward search's time budget -- built (2026-11-10)

The user asked for it next. The forward search (`Cylinders::plan_inner`)
now takes the inverse walk's budget and cancel flag
(`PlanOptions::budget`, `cancel`), which it was handed and ignored:
20 s on the desktop, 1.5 s on the web (`INLINE_BUDGET`). Past the
budget it keeps every word not yet expanded as it stands -- the depth
cap's rule, sound because such a word's image is larger than the view --
and the plan says it timed out. It checks between words, so each is
expanded whole or kept whole, and not before the first level (whose
root is the empty word); a frontier wider than the table is still
`TooManyWords`; a cancel returns `TimedOut`.

Gates:

- `a_forward_search_out_of_time_keeps_its_frontier`: the gasket at 1e4
  with no time stops after its first level, says so, and is complete --
  every word of the full plan extends one of its words, none of its
  words extends another, and its mass covers the full plan's.
- `a_cancelled_forward_search_stops`.
- `a_timed_out_forward_plan_draws_the_full_plans_picture` (GPU): the
  gasket at 256x, the full plan (depth 6) against the timed one
  (depth 1), by density, judged as the plans on the way are against the
  landed plan (a tier past 1.5% at four standard errors, a dense block
  past 10%). They agree to 0.04-0.06% over the view, a dense block to
  0.4-0.6%. Both differ from the plain chaos game by more than from
  each other -- the full plan 0.9993 of it, its worst dense block 0.9%;
  the timed plan 0.9997 and 0.7% (`TRUTH=1 IPT=4096`) -- and longer
  orbits do not shrink it, so it is not burn-in. That sub-percent
  difference between a targeted gasket and its chaos game was not known
  before; it is recorded here, not chased.

The slow fixtures again, the web's route (`FIXTURES=jac_asn,minkowskope,
truchet2,glynnspiro`): jac_asn plans at all four depths in 20 s, about
3,700 words each, where it searched 47 s to refuse; minkowskope plans at
three, about 2,000 words, where both routes refused with too many words
(the desktop's still does, at 10.6 s, under the budget); truchet2 and
glynnspiro reach the word cap in about a second either way.

## Step 3: decide P9, by measurement

Not a build. Route B builds no instruction list, so the exact
Jacobians this wanted from step 1 are the built-ins' alone (dual numbers,
`ifs_real::Dual`); a Rhai kernel's are its five-point stencil's, or a
dual walker over the tree would have to be written first. Run P9's own
gate offline on the CPU for the built-ins, and for the Rhai kernels
once they have one: carry
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

### The registry race: fixed (2026-11-09)

It was not the tests. `custom::ensure` marked a source known before it
compiled and registered it, so a second thread ensuring the same source
returned before the registry had it, and its flame read as having no
variations ("transform 1 has no variations"). Reproduced three runs in
three: `the_batched_replay_is_the_point_replay` beside
`a_rhai_variations_draws_are_forced_only_under_replay`, which registers
the same julian twin. Now one registration runs at a time, and a source
counts as known once its attempt is over. The three twin tests pass
together three runs in three. The app calls `ensure` wherever a flame is
read, on the render, thumbnail and export threads, so the app had the
race too.

- Regression test: `every_thread_finds_what_it_ensured` (ignored: it
  registers a variation). Eight threads ensure one new source; it fails
  on the old `ensure` and passes on the new.
- No registry of their own was needed: the twin tests can run in one
  `cargo test` again. The escape, simulation and effect registries'
  `ensure` were read and do not have the race (the first two check what
  is registered, the last holds its lock throughout).

### juliascope-rays at 1e4: a real 1% difference, cause not found

`what_the_juliascope_rays_block_is` (ignored, GPU) measures each block's
difference in standard errors at the gate's samples, then with more:

| Samples | Block 66's z | Its ratio, twin over built-in |
|---|---|---|
| The gate's (8 frames, 6 replicates), three runs | +3.8, +4.4, +2.5 | 1.006-1.011 |
| 4x the replicates | +7.8 | 1.009 |
| 4x the frames | +8.8 to +12.7 | 1.010-1.011 |

Block 66 keeps its place and its z grows with the samples, so the twin
puts about 1% more light in it than the built-in does. It is a
middle-tier block, 0.05 of the mean block's density, beside dense ones.
The other high blocks move from run to run: noise. So the gate is right,
the threshold stays, and raising the samples would make it fail every
run, not pass.

What it is not:

- **The plan.** Both plan one word, arms (2, 1, 0, 0), the masses equal
  to 7 digits (4.164931e-4 and 4.164934e-4), the twin's arm
  probabilities 1/7 to 4e-7. (The plans cannot be swapped between the
  flames: the twin names its arms by draw code on the 256 grid, 18, 54,
  91, ..., the built-in by 0 to 6.)
- **The free orbit, untargeted.** Each flame untargeted at x1 and x30:
  no block past five standard errors, ratios within 0.3%.
- **The offset replay.** `the_offset_replay_holds_per_sample` on the
  twin (`OFFSET_TWINS=1`, new) from 20,000 sample points
  (`OFFSET_POINTS`, new): at 1e4, 20,000 in view, mean error 0.0002 px,
  worst 0.0007, bias (+0.0002, +0.0000) px, the built-in's figures to
  every digit printed; the same at 1e6 and 1e8. The twin's own words
  were replayed (their arms' grid codes).

**Next lead:** what only the targeted render does. A replay build forces
a twin's draws for the word's arms (`bound::force_arms`); if the forcing
reaches the free orbit's own draws in a targeted render, the free orbit
there is not the untargeted one measured above.
`a_rhai_variations_draws_are_forced_only_under_replay` checks the build's
text, not the free orbit's distribution. Measure the free orbit inside a
targeted render (the twin's against the built-in's), then the forced
sample's arm in the render against `PlanGpu`'s.

## Order

Step 0, then step 4 (small, and it makes every later gate cheaper to
run), then step 1's three evaluators, step 2, step 3.

## Decisions for the user

1. **After step 0: decided 2026-11-09, route B.** Step 1 as planned, or the web's bounds by
   route A or B with no instruction list. My recommendation is B: it
   has no wasm size cost and no new form, and the measurements leave
   the instruction list no speed to win. The dual Jacobians, if
   wanted for exactness, would then be a walker over the tree too.
2. **After step 2 (now):** whether the web takes twins' bounds for
   built-ins. Built that way, as the plan proposed; the switch is one
   line in `ifs_ball::derived` if not.
3. After step 3: P9.

## Not in this plan

Ways to widen what the two engines take, and Focused Rendering in 3D,
are experiments for later:
[planner-experiments.md](../experimental/planner-experiments.md).
