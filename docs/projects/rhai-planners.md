# Rhai variations in Focused Rendering and the Flame Attractor

Status: **Part A step 1 built** (2026-11-08, branch `rhai-planners`);
the rest planned. The user (2026-11-08): both features compatible with
Rhai scripts.

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

### To do

1. **Soundness for Rhai bodies.** The gate checks built-ins. Run it over
   the twins' compiled bodies too (ignored, as it registers them): a
   wrong interval rule for a pattern only the language emits would
   otherwise go unseen, and a bound too small drops part of the picture
   silently.
2. **The 32 twins that refuse where their built-ins derive**, by reason:
   - 11 do arithmetic on an opaque argument (the `glsl_*` family):
     find what is opaque (likely a direct-colour pointer or the weight)
     and model it;
   - 9 divide by a range containing zero (bMod, the cut family, jubiQ,
     murl): check whether each guard is one the evaluator could read;
   - 9 join values of different shapes (the crop family, polarplot2d_wf,
     truchet, w): an `if` whose arms give a vector and a scalar, or two
     vector kinds;
   - 2 a `pow` of a negative base (mobiusN, synth), 1 reads per-thread
     state (littlewood, refused for built-ins too).
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

A1 (soundness for Rhai bodies), A2 (the 32), then B1, B1's twins, B2,
B3; A3 if wanted.

## Decisions for the user

1. **The inverse's form:** `fn inverse(z, p, branch)` with
   `INFO.inverse_branches` (proposed), and how a body says "no
   preimage here" -- a built-in helper `no_preimage()` returning the
   sentinel the kernels use (proposed), or any non-finite value.
2. **Scope:** B1 first, then B2 and B3 -- or B1 alone for now.
3. **The twins' inverses** written by hand in Rhai for the 11 planar
   kernels (proposed), so an edited twin keeps its inverse.
