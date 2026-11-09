# Rhai variations in Focused Rendering and the Flame Attractor

Status: **Parts A (steps 1-2) and B (B1-B3) built** (2026-11-08, branch
`rhai-planners`): every twin whose built-in derives a bound derives one
from its own code; a Rhai variation declares its inverse and the Flame
Attractor draws it, alone, summed with an affine, and as a solid; and a
built-in with no inverse in Rust takes its twin's. A3 and the items under
*Open* are left. The user (2026-11-08): both features compatible with
Rhai scripts; decisions 1-3 below taken as proposed, all of Part B.

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
   Needs a way to say "this `rand()` picks one of `n` arms"
   (an `INFO.arms` naming it, say). Proposed last, if wanted.

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
  kernel: its replay shader has no body for one.
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

## Order of work

A1, A2, B1 with the twins' inverses and the borrowing built-ins, B2 and
B3 done (2026-11-08); A3 and *Open* left.

## Decisions for the user

1. **The inverse's form:** `fn inverse(z, p, branch)` with
   `INFO.inverse_branches` (proposed), and how a body says "no
   preimage here" -- a built-in helper `no_preimage()` returning the
   sentinel the kernels use (proposed), or any non-finite value.
2. **Scope:** B1 first, then B2 and B3 -- or B1 alone for now.
3. **The twins' inverses** written by hand in Rhai for the 11 planar
   kernels (proposed), so an edited twin keeps its inverse.
