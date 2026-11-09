# Rhai variations in Focused Rendering and the Flame Attractor

Status: **Parts A (steps 1-2), B (B1-B3) and C (C1-C3) built** (2026-11-08,
branch `rhai-planners`): every twin whose built-in derives a bound derives
one from its own code; a Rhai variation declares its inverse and the Flame
Attractor draws it, alone, summed with an affine, and as a solid; a
built-in with no inverse in Rust takes its twin's; and Focused Rendering's
inverse walk plans a flame with a Rhai variation that has an inverse --
its arms read off its forward map, its deep zoom through a difference form
derived from its body -- as the built-in's, to 1e6. The items under *Open*
are left. The user (2026-11-08): both features compatible with Rhai
scripts; decisions 1-3 below taken as proposed, all of Part B; of the
inverse walk refusing a Rhai inverse, "This is an important feature, and
part of the reason we're doing all this"; and the goal, "I want Rhai twins
to be able to do everything the built-ins can."

## Why a Rhai variation does not work in them

Neither feature runs a variation's forward code alone. Each needs maths
about the variation that its code does not state:

- **Focused Rendering** pushes a disc forward through each map, so it
  needs a forward bound that may only over-estimate
  (`variations::bound`). A few are written by hand; the rest are
  derived by interval arithmetic from the variation's WGSL
  (`variations::derive`). Until this branch that read shipped bodies
  alone, so a Rhai variation was refused before its code was looked at.
- **The Flame Attractor** (escape mode D, `ifs_flame`, and its solid)
  walks the attractor backwards, so it needs each variation's exact
  inverse (`variations::inverse::INVERSES`: 26, hand-written in Rust,
  12 affine roles, 11 planar kernels, 3 solid). Every other variation is
  refused, built-in or Rhai.

An unedited twin borrows its built-in's inverse and bounds
(`twins::planner_name`), "unedited" meaning the same maths since
2026-11-08 (`twins::builtin_computed_by`). An edited twin, or the user's
own script, has neither.

## Part A: Focused Rendering

### Built (step 1)

- `derive` takes a registered user variation's compiled body, in a probe
  module as a built-in's (`body_for`), and its compiled `prepare` for
  derived slots (`custom_init_module`).
- Two patterns of the formula language's code the interval evaluator
  now reads as what they are, both exact:
  - a value times itself through separate reads (`z.x * z.x`, the
    language's squared norm) is a square (`same_value`); it gave a
    negative lower bound, then a `pow` of a "negative" base;
  - Rhai's `max` and `min`, which the language writes as a test and an
    assignment for their NaN rule, are the max and min of ranges
    (`as_extremum`); joining the two arms lost the guard
    (`max(|y|, 1e-20)` reached zero, a pole).
- Measured over the 640 twins' own compiled bodies
  (`measure_derived_bounds_of_twins`, ignored): of the 433 whose
  built-in derives, **401 derive** (322 before the two patterns). Ten
  more built-ins derive too.
- The built-ins' soundness gate passes
  (`derived_bounds_contain_the_real_wgsl`: 542 derived bounds, each
  containing what the shipped WGSL computes at sampled points).

### Built (step 2: soundness, and the 32)

- **Soundness for Rhai bodies.** `BOUNDS_TWINS=1` runs the gate over
  every twin's own compiled body as well (`derive_own`).
- **What refused, and what the evaluator does now** (each a rule that
  holds for built-ins too):
  - **An opaque value is carried, not refused** (the `glsl_*` family,
    which loads its direct colour `(*vrc).xyz` into locals at the top):
    arithmetic, parts, casts and joins of an opaque give an opaque;
    a condition on one is undecided (both arms); it refuses where it
    reaches the returned point, a parameter slot, or a rule that needs a
    number. Writes through an opaque pointer, lane by lane, are dropped.
  - **A pole or a domain exit is an opaque too**, refused by its own
    name where it reaches the point (`lost`, `found`): ten twins divided
    by a range through zero in a computation the point never read.
  - **Per-thread state is opaque** (`get_state`): littlewood's colour
    register, read at the top of its twin.
  - **A local unwritten on one path joins as its type's zero** (a
    `bool` as `false`, a vector lane-wise), not the number 0 (the crop
    family's flags); a vector against a scalar joins lane-wise.
  - **`floor`, `ceil`, `round`, `trunc` are exact**, not widened, so a
    `to_float` of a whole number stays one and `power != 1.0` decides
    (murl's prepare).
  - **`+ - * /` widen each end by its own magnitude** (`round_out`;
    correctly rounded, so the least rounded result is within an ulp of
    the least exact one), and no `sqrt` is below zero: by the larger end,
    `sqrt(r2) + 1e-6` reached below zero (mobiusN).
  - **Casts by their target type** (`cast`), and **an integer division
    truncates** (by naga's own expression types, `is_integer`). The gate
    caught this one: lazysensen's parity `n - (n / 2) * 2` read `1 / 2`
    as 0.5 once `floor` stopped widening; it had passed by the widening's
    luck.
  - **The language's helpers by their own rules**: a trig of
    `uf_reduce(t)` is a trig of `t` (bMod's `cos` lost what `sigma`
    knew), and `uf_powf` (Rhai's `pow`, signed at a whole exponent) as
    its arms are, not through its IR (synth).
- **Measured** (`measure_derived_bounds_of_twins`): of the 472 twins
  whose built-in derives, **472 derive**, and 2 more whose built-in does
  not; 470 built-ins derive at that disc (433 before step 2).
- **The gates pass:** built-ins, 566 bodies, 484,053 shader evaluations;
  with the twins (`BOUNDS_TWINS=1`), 1,130 bodies, 961,383 evaluations,
  all inside their discs.

### To do

3. **Many-valued Rhai variations.** julian's and juliascope's bounds
   pin their random arm by rewriting the built-in's WGSL text
   (`bound::force_arms`), which a Rhai body is not, so an edited twin of
   either has no bound, and neither does a user's many-valued script.
   Needs a way to say "this `rand()` picks one of `n` arms". Now C1:
   the arms read off the forward map, nothing declared.

## Part B: the Flame Attractor

### The design

A Rhai variation declares its inverse beside its forward map:

```rhai
const INFO = #{ title: "My Julia", inverse_branches: 2 };
fn variation(z, p) { ... }
fn inverse(z, p, branch) { ... }   // the preimage on branch 0 .. n-1
```

- **The check.** At compile time the interpreter runs forward then
  inverse at sampled points over a disc, and requires one branch to give
  back each point (to a relative tolerance). An inverse that does not
  undo the map is refused, with the worst point in the message. So a
  twin can ship an inverse, and an edit to its forward map that the
  inverse no longer fits drops it with a message, not a wrong picture.
- **CPU** (`scene::ifs_analysis`). `Kernel` must stay `Copy +
  PartialEq` (`Map2`, `IfsMap`, `IfsSpace` depend on it), so a
  `Kernel::User { slot, params }`, `slot` indexing a table of compiled
  programs, with an arm in every `match` on `Kernel`:
  - inverse and forward by the interpreter (f64), the forward refused
    for a body that draws a random number;
  - the inverse Jacobian by the language's derivative pass
    (`lang::derive`) or central differences; the local σ as `1/σ_max` of
    it, as `Elliptic` has it; the Hessian by differences of the
    Jacobian, as the sums' path does;
  - no closed difference form (`third` measured), no BigFloat inverse
    (the deep-zoom handover stops early, which is sound), not walk-only;
  - discovered beside `registry.inverse()` in `variation_stage` and
    `kernel_from_registry` (an `InverseDef` is a `fn` pointer and cannot
    hold a compiled program).
- **GPU** (escape mode D). A new map kind (8). The inverse compiled to
  WGSL per flame, `ifs_user_inv_<slot>(v, branch, params)` with its
  Jacobian, spliced into the mode D shader as `IFS_NEWTON` is (marker
  lines, `assemble_ifs_rig`), dispatched before the bubble fallthrough
  at every kernel switch (an unknown kind falls to bubble today:
  `ifs_kernel_inverse`, `ifs_kernel_sigma`, `ifs_kernel_jacobian`). Its
  parameters in a new group-1 storage binding, offset per row (a row's
  `params` holds four floats, two of them taken by kinds 1-5). The
  pipeline key gains the generated code's hash.
- **Twins.** The 11 planar inverses (julia, julian, spherical, bubble,
  disc, blob, hemisphere, and the walk-only elliptic, splits, cylinder,
  juliascope) written once in Rhai into their twins, so an edited twin
  keeps its inverse while the check passes. The affine roles need none
  (a twin of `linear` is affine by its maths, borrowed while unedited).

### Built

- **B1** (`91e60913`, `58769dd5`): `fn inverse(z, p, branch)`,
  `INFO.inverse_branches`, `no_preimage()`, and `fn image_gap(z, p)` --
  the gap was not in the design: where an inverse has no preimage the
  walk knows nothing of that piece, and the built-ins' unit discs score
  such a point by their gap, so a twin without one drew less than its
  built-in. `variations::user_inverse` compiles and checks them (400
  points, radius 4 -- radius 2 never reached bubble's outer branch -- one
  in fifty spared), at the defaults when compiled and at each transform's
  values when used (`kernel_slot`, one slot per values). `Kernel::User {
  slot }`; derivatives by central differences, carried through the dual
  numbers by `Real::outside2`; no difference form, no BigFloat inverse
  (the deep-zoom handover stops at it), no smooth radius (the delta
  walk's remainder unbounded). Kind 8 on the GPU, its values behind the
  xaos graph. Many-valued forward maps at sixteen fixed draws (seeded
  draws missed an arm of julia's and its ball came out short); a forward
  map unbounded at the origin probed, so spherical's twin takes the
  built-in's bulk ball.
- **No preimage, on the GPU** took the longest. A branch with none must
  leave as the CPU's does -- at infinity, no term folded, the path
  keeping only a positive bound it had (the frozen rule) -- and a finite
  far point does not: at 1e30 with σ one a gapped branch's step bound
  the piece that far (bubble's built-in row has it so, and agrees 98.9%
  with its own reference walk); with σ zero a path claimed distance
  zero. So the row's point leaves at 3.4e38, past the walk's 1e37 for a
  number lost, before the pre-affine. Every twin and borrowing built-in
  then agrees 100% with its reference walk.
- **The twins' inverses** (`scripts/twin_inverses.py`): the eleven planar
  kernels', transcribed from `kernel_inverse_gen` against the twin's
  forward map where they differ (spherical's 1e-6); disc's rings fixed at
  twelve and cylinder's turns at -2..2, where the built-ins read them off
  the ball. Juliascope, elliptic, splits and cylinder, which the built-in
  walk alone takes, now render in mode D through their twins. Then nine
  built-ins with none in Rust borrow their twins' (`user_inverse::of`):
  swirl, horseshoe, polar, exponential, exp, log, fisheye, eyefish,
  sinusoidal.
- **B2** (`bef63f10`): summed with an affine, as a `SumMap2`: Newton over
  the forward map, each inverse branch seeding a solve; on the GPU the
  forward map compiled beside the inverse and dispatched by the row's
  kernel kind inside the Newton shader. A forward map that draws (julia's
  twin) has none to compile and is not summed. Bubble, spherical and
  hemisphere twins summed with linear agree 99.5-100% (the built-ins'
  sums 98.4-100%).
- **B3** (`f642d975`): `fn inverse3(v, p)`, `Kernel3::User3`, kind 4,
  σ from the 3×3 Jacobian's σ_max in closed form; julia3D's and
  julia3Dz's twins. Each agrees with a CPU march of its own analysis
  (99.9%, 100%); julia3Dz's solid is the built-in's to the pixel,
  julia3D's a little thinner -- its σ the exact local one where the
  built-in kernel takes `1/n²` of it everywhere, a bound.

### Open

- **An inverse with no preimage on most branches at most points** read
  every pixel as the set on the GPU (splits' twin, three of whose four
  branches have none at any point, when it gave `no_preimage()` there),
  where its CPU reference did not. Not understood; splits' twin gives
  each branch's point instead, as the built-in kernel does.
- **Focused Rendering's inverse walk** (`Backward`) refuses a user
  kernel: Part C.
- **Deep zoom** through a user kernel: no BigFloat inverse and no
  difference form, so the handover stops at the first one and f32 takes
  over from there.
- **Cost**: a user kernel's σ is four extra inverses a step on the GPU
  (six in 3D), and a sum's Newton solve runs the forward map in the
  interpreter on the CPU. Disc's twin, at twelve fixed rings, summed with
  linear took the per-pixel CPU reference hours at 96².
- **A3** below.

### Phases

- **B1. Planar, alone in its transform:** the language (`fn inverse`,
  `INFO.inverse_branches`, the check), `Kernel::User`, kind 8, the
  parameter binding; then the twins' inverses. Gates: a Rhai bubble with
  an inverse renders as the built-in's Flame Attractor (density compare,
  as the twins' gates), and the check refuses a wrong inverse.
- **B2. Summed with an affine** (a transform of `julia` + `linear`, kind
  7 today, Newton over the kernel's forward body): needs the user's
  forward map compiled for mode D's calling context as well.
- **B3. The solid** (`ifs_flame_3d`): `fn inverse3(v, p)`,
  `Kernel3::User`, its own kind.
- **Focused Rendering's inverse walk.** It plans an "armed" flame
  backwards (`Backward`), which is how julian's flames get planned; a
  Rhai variation with an inverse could take that route too. Not needed
  for B1.

### Fixed after B3 (2026-11-08, from use)

- **A Rhai inverse beside a camera lens drew nothing**: the lens brings
  the flame's `get_param` into mode D's shader, and the user kernels
  defined another, so the shader did not compile (the reported grand
  julian with a Rhai julian). They read through their own
  (`ifs_user_param`); and the solid's path tracer, which had no user
  kernels at all, splices them as the walk does
  (`assemble_ifs_path_user`). Gate: `a_user_kernel_compiles_beside_a_lens`.
- **A Rhai julian of negative distance drew another picture than the
  built-in**: an inversion's image of the ball leaves a hole about the
  origin, which the walk scores as a gap rather than inverting into
  (`Kernel::hole_radius`), and a user kernel had none. Its hole is now
  the least modulus of its forward map over the ball's reach, sampled
  (`UserKernel::hole_radius`), where the map is unbounded at the origin
  -- and that probe, which asked for past 100 at 1e-3 and so missed
  every julian of negative distance (32 at power 2, 1.6 at power 15),
  also takes a modulus rising toward the origin as a pole's does, by the
  same factor ring after ring -- not log's, which rises by a logarithm,
  and read as an inversion took the bulk ball and lost a quarter of its
  set in the borrowing gate. The two
  CPU walks then agree on every pixel, and the GPU pictures on all but
  68 of 147,456. Gate: `a_user_inversion_has_its_builtins_hole`.

## Part C: Focused Rendering's inverse walk

The walk that plans `grand-julian` (`scene::backward`) refused any flame
with a user kernel, saying its replay shader had no body for one. That
was wrong: a word is replayed through the flame's own compiled variations
(`ct_apply_symbol` calls `apply_variations`), Rhai bodies among them.
What is missing is three other things.

1. **Arms** (A3). The walk exists for many-valued flames: its alphabet
   is (transform, arm), and the GPU forces a word's arm by rewriting the
   variation's draw (`bound::ARMED`, julian's and juliascope's WGSL
   spelled out). A Rhai body's `rand()` lowers to `rng_nextf(rng)`,
   which nothing rewrites, and the CPU knows the map only at sixteen
   fixed draws, not its arms.
2. **The walk's own arithmetic**: `forward_arms`, `forward` and
   `arm_of` know a root's arms by `Kernel::Root`; the inverse it can
   already take (the user's, in the interpreter).
3. **Deep zoom past f32**: a word replays as an offset from a reference
   orbit once its plain replay's error would show (`backward::WordRefs`),
   through each kernel's forward difference form `F(z + δ) − F(z)`,
   computed without the cancellation -- written by hand per built-in
   kernel (`scene::forward_delta`, `shaders/core/replay_delta.wgsl`). A
   user kernel has none, so its words would stay absolute: good to a
   view some 1e-4 of the attractor's size, where grand-julian goes to
   1e6.

### Built

- **C1, arms** (`6c4f6661`): read off the forward map per kernel
  (`UserKernel::arms`): every draw of a call set to each step of a grid of
  256 in turn, at six points; a run of steps with the same images an arm's,
  its share bisected (a pick of `floor(2.5·u)` shares 0.4, 0.4, 0.2), its
  code the middle step. The grid is the one draw convention everywhere: the
  analysis's sixteen sample draws are sixteen of its steps, a word's symbol
  carries an arm's code, and a replay build wraps a Rhai body's draws to
  read it, `(code + ½)/256`, exact in f32 and the interpreter's fixed draw
  to the bit (`bound::force_arms`). A flame with a Rhai variation that
  draws and has an inverse is the walk's (`cylinder::armed`); a twin that
  borrows its built-in (julia's) is planned and replayed as the built-in.
- **C2, the walk** (`6c4f6661`): a user kernel's arms and shares in the
  alphabet and the chaos game; built-in roots keep their arithmetic to the
  bit. Grand-julian with every julian its twin plans within five words of
  the built-in's in thousands, coverage 1.0000 alike at 1e2 and 1e3, on the
  CPU and the GPU planner (`a_rhai_julian_plans_as_the_builtin`); the
  twin's walk reads in 2.1 s against 0.38 s, its maps in the interpreter.
- **C3, the forms**: `lang::delta::variation_difference` as proposed below
  -- the body one pass of an escape loop, the derived equations a
  transform section again; julian's and bubble's twins' forms are the
  built-ins' hand forms to 3.9e-13 down to deviations of 1e-13
  (`a_twins_derived_difference_is_its_builtins`). On the GPU
  (`lang::wgsl::difference_kernel`) it needed its sines exact near zero:
  `sin(d/2)` of a small deviation through a GPU's own `sin`, good to an
  absolute 1e-7, was noise -- the hand forms' `fd_sin` reason -- so the
  lowering takes the escape engine's series (`exact_near_zero`); then
  every twin's shader form is its CPU form to 1.7e-6
  (`the_shader_user_forms_are_the_cpu_ones`). Spliced per flame
  (`forward_delta::replay_delta_wgsl`): `ct_fwd_diff`'s call of
  `fd_kernel` becomes a dispatcher's only where a flame has a form, so
  every other shader is its text; a row of kernel 9 names its form by the
  key's 24-bit hash (a sticky superset may keep a variation the flame
  dropped) and has its values behind the rows. Grand-julian's twin then
  renders the built-in's density block by block at 1e2, 1e4 and 1e6, no
  block past five standard errors (at 1e4 before the forms, 37)
  (`a_rhai_julian_renders_as_the_builtin`).
- **The corpus** (`the_corpus_plans_through_rhai_twins`): every deep-zoom
  fixture the built-in walk plans, each kernel whose twin has an inverse
  its twin, borrowing off -- 30 of 32 walked through user kernels with the
  built-in's alphabet, their plans covering what the built-in's cover at
  the saved view and at 1e3 (julian-disc's in a fifth of the words: its
  twin's twelve fixed rings). A `pre_blur` beside a user kernel needed a
  renewal of its own (the disc holding all the blurred transform can
  output): its forward map's image of the dilated input, sampled on rings
  evenly spaced and halving, about the input's centre and the kernel's
  origin -- rings at quarters of a reach of 30 missed bubble's peak at
  `|z| = 2`, and grand-julian's zooms lost 4% of their saved views.
- **Open, the walk**: a user kernel that draws, summed with an affine
  (juliascope-arms, random1). Its sum's maps are one per arm, each solved
  by Newton from the inverse's branches, where `SumMap2::branches` counts
  a user kernel's inverse branches alone; and mode D's Newton has no
  forward map with a forced draw. The corpus gate names the two and checks
  they are refused for this reason.

### As proposed

- **C1. Arms from the forward map.** Per kernel (per parameter values,
  as the hole): every draw of a call set to one `u` (the fixed-draw
  convention the analysis already uses), `u` on a grid of 256, grouped
  by image at a handful of probe points; each group an arm, its
  probability the measure of its interval of `u` (the boundaries
  bisected), forced by its middle draw. A symbol's arm field carries that
  draw's index for a user kernel, so the replay's forced value is
  `(index + ½)/256`: a `ff_forced_draw` around every `rng_nextf` of a
  Rhai body in a replay build, as `force_arms` wraps julian's. A body
  whose draws do not fall into at most 64 arms (a blur's continuous draw)
  is refused, as `blur` is. The forward planner's bounds take the same
  arms (A3 as written, with nothing for the script to declare).
- **C2. The walk takes user kernels**, absolute replay: arms, forward
  maps and `arm_of` through C1; the refusal goes. Gate: the reported
  grand julian with a Rhai julian plans the words its built-in plans and
  renders its density, at zooms the absolute replay holds (1e2, 1e4).
- **C3. A derived difference form** for a Rhai body: `lang::delta`
  already derives them for escape formulas (pairs of reference value and
  deviation, by cancellation-free rules for every function it knows --
  `atan2` of a complex, with the turns across the cut, and a power the
  same for every pixel among them). The way in, without a second
  deriver: the variation's body is one pass of an escape loop, `z =
  F(z)` (its input point `#z`, its result written to `z`), so the
  derived loop equations are `#dz' = F(Z + dz) − F(Z)` -- the forward
  difference. Its draws are a uniform global while it derives (a forced
  draw is the same at the reference and the sample). The equations are
  then a transform section again: the reference point its input, the
  deviation `ctx.accum`, the draw the colour channel, the new deviation
  its result -- so the interpreter runs it for the references (f64) and
  the variation lowering compiles it for the replay, spliced per flame
  behind a marker in `replay_delta.wgsl` as mode D's user kernels are,
  its values behind the forward rows (kernel 9). Gates: the derived form
  of each twin against its built-in's hand form at tiny deviations; the
  shader's against the CPU's, as `the_shader_forward_forms_are_the_cpu_ones`;
  the grand julian at 1e6 as its built-in.

## Order of work

A1, A2, B1 with the twins' inverses and the borrowing built-ins, B2, B3,
C1, C2 and C3 done (2026-11-08); *Open* left.

## Decisions for the user

1. **The inverse's form:** `fn inverse(z, p, branch)` with
   `INFO.inverse_branches` (proposed), and how a body says "no
   preimage here" -- a built-in helper `no_preimage()` returning the
   sentinel the kernels use (proposed), or any non-finite value.
2. **Scope:** B1 first, then B2 and B3 -- or B1 alone for now.
3. **The twins' inverses** written by hand in Rhai for the 11 planar
   kernels (proposed), so an edited twin keeps its inverse.
