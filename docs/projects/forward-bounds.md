# Forward Bounds — deriving a variation's disc bound from its WGSL

**Status:** phase 1 built and measured 2026-09-19 (§11). Planned
2026-09-19. The shared piece both
[flame-deep-zoom.md](flame-deep-zoom.md) (§7 item 1) and
[ifs-general.md](ifs-general.md) (§8, "forward reach without an
inverse") named and neither built, now with a plan of its own because
the measurement in §1 says the obvious approach — write the bounds by
hand — does not get there.

**What exists:** the contract, five hand-derived bounds, the
composition through a transform, the enumeration that consumes them,
and the GPU gate that checks a bound against the shipped shader
(`src/variations/bound.rs`, `src/scene/ifs_ball.rs`,
`src/scene/cylinder.rs`). This plan replaces the *supply* of bounds,
not the contract or its consumers.

---

## 1. Why this plan exists: the meter

`what_blocks_targeting_across_the_corpus` (ignored test in
`cylinder.rs`) runs the 45-flame corpus against each blocker
independently:

| blocker | blocks | sole cause |
|---|---|---|
| a variation has no forward bound | 44 | **16** |
| colour is not affine (DC/RGB) | 23 | **0** |
| xaos | 8 | **0** |

Bounds are the wall; colour and xaos are the wall behind it and are
worth nothing until bounds exist. The same meter's greedy set cover
says what hand-deriving buys: **14 bounds free 19 of 45**, about 1.4
flames each, with 55 distinct blockers and no head — the commonest
(`poincare3D`, 7 flames) frees one on its own.

So the supply has to be automatic. `how_much_wgsl_would_an_interval_evaluator_need`
(ignored test in `bound.rs`) sizes that: of 647 2D bodies, **247 are
straight-line arithmetic, 332 carry a branch or `select`, 68 a loop,
284 read the RNG or per-thread state**; the callee head is a small set
of ordinary intrinsics. A regex pass over the definition files adds:
per-thread state in ~6 bodies, `params.*` uniforms in none, the
transform's own weight (`transforms[xform_id].variations[...]`) in
113, loops with a literal bound 24 and another bound 14, and 127
bodies with a `wgsl_init`.

Interval arithmetic through the body is sound by construction, and
one evaluator covers all 647 plus every variation added later.

## 2. The idea in one paragraph

A bound answers "given a disc of inputs, name a disc containing the
outputs", and it may over-estimate but never under-estimate
(`bound.rs` module docs: too loose costs efficiency, too tight drops
measure from the render silently). Interval arithmetic is exactly an
over-estimating evaluator: run the body with every value replaced by
the closed range it could take, and every operation replaced by a rule
that contains all results. The input disc becomes a box, the body
runs once on boxes, the output box becomes a disc. Where the body
draws a random number, the range is `[0, 1)` — which is precisely how
`blur` and `julian` got their bounds by hand, so the 284 stochastic
bodies are the *easy* case, and the class of variation the inverse
direction can never touch comes along free.

## 3. Don't write a parser: naga is already here

The bodies are WGSL strings, but `wgpu::naga` (29.0.4) is a dependency
and is already called in five files (`shader_builder_v2.rs`,
`probe/shader.rs`, `escape/assembler.rs`, …) for validation. Its
front end parses a whole module to an IR that is built for exactly
this kind of walk: `Function { arguments, result, local_variables,
expressions: Arena<Expression>, body: Block }`, with
`Expression::{Literal, Compose, Access, AccessIndex, Splat, Swizzle,
FunctionArgument, GlobalVariable, LocalVariable, Load, Unary, Binary,
Select, Relational, Math, As, CallResult, …}` and
`Statement::{Emit, Block, If {condition, accept, reject}, Switch,
Loop {body, continuing, break_if}, Break, Continue, Return, Store,
Call, …}`. `for` loops are already lowered to `Loop` with a
`break_if`. `MathFunction` has 79 variants; the survey says ~40 occur.

**Where the module comes from.** `probe::shader::build(batch, false)`
already assembles a complete, validating module for a batch of
variations through the same `build_from_template` the app uses — so
every helper library a body needs (`complex.wgsl`, `noise.wgsl`,
`ff_atan2`, the per-variation `fn <name>_helper`s) is spliced in
alongside. Parse that with `naga::front::wgsl::parse_str`, index
`module.functions` by name, and every `variation_*`, `init_*` and
helper is a `Handle<Function>` in one arena. The lens gate
(`bound.rs::gpu_tests`) already builds these batches; the derivation
reuses the batching and caches the parsed `Module`s in a `OnceLock`.

Nothing hand-written parses WGSL. If a body uses something the
evaluator does not model, the answer is a refusal that names the
construct, not a wrong disc.

## 4. The evaluator

### 4a. Values

```text
Interval  { lo: f64, hi: f64 }        closed; ±inf allowed; NaN never
IVal      Scalar(Interval)
          Vec(SmallVec<[Interval; 4]>)  componentwise, for vec2/3/4
          Int(i64 range)                loop counters, `u32` params
          Bool(Yes | No | Maybe)        conditions
```

f64 intervals bounding an f32 computation: every rule rounds
outward, and the GPU gate (§6) has a relative slack of 1e-4 for the
f32-against-f64 comparison already. Widening by a few ulps at each
op is cheap insurance and is done.

### 4b. The environment a body sees

| in the body | evaluator |
|---|---|
| `p` (`FunctionArgument 0`) | the input box: `[c−r, c+r]` per lane |
| `get_param(xform_id, variation_id, slot)` | `Call` intercepted by name → exact interval of the parameter at `slot`. Slot → name via `VariationDef.parameters[slot]`, value via the existing `ParamFn` |
| init-derived slots (`slot ≥ parameters.len()`) | evaluate the body's `init_<name>` from the same module with exact-interval params. 127 bodies have one; `julian`'s `cpower` is the worked example |
| `transforms[xform_id].variations[variation_id]` | the `Access` chain on that global → exact interval of the weight (113 bodies) |
| `rng_nextf(rng)` | `[0, 1)` |
| `get_state` / `set_state` | **refuse** ("reads per-thread state"). ~6 bodies. A per-call bound cannot know what a previous iteration stored |
| `params.*` | never occurs; refuse if it does |
| any other `Call` | evaluate the callee's IR (helpers are in the same module) |

### 4c. Statements

- **`Emit`** — straight-line; evaluate the expression range into a
  table `Handle<Expression> → IVal`.
- **`If`** — evaluate the condition to `Yes | No | Maybe`. Yes/No
  take one arm. Maybe evaluates both arms *from the same incoming
  state* and joins: every local assigned in either arm takes the
  union. This is the whole treatment of 332 bodies.
- **`Select`** — same rule as an expression.
- **`Switch`** — union over reachable cases (rare).
- **`Loop`** — run `body`, then `continuing`, then test `break_if`;
  continue while it is `No` or `Maybe`, joining locals across
  iterations, up to a cap (64). A loop whose counter is bounded by a
  literal or a parameter (24 of 38) has an exact `Int` counter and
  exits exactly. One that has not settled at the cap → **refuse**
  ("loop did not converge").
- **`Return`** — the result's `IVal`; a body with several returns
  unions them.
- **`Store` / `Load` / `LocalVariable`** — a local table joined at
  merges as above.

### 4d. The math table

Implemented, in survey order of use: `sin cos abs max sqrt select
floor pow dot atan2 cosh clamp log min fract sinh length exp mix round
sign tan smoothstep tanh trunc step acos asin atan normalize distance
fma`. Each rule is written down with its domain condition:

- `a / b` with `0 ∈ b` → `(−∞, ∞)`. That propagates to an unbounded
  output and a refusal that says **"pole"** — which is the honest
  answer for `curl` (`1/(re²+im²)`), and for `rays` via `tan` over
  `π/2 + kπ`. These are genuinely unbounded maps and a sound bound
  cannot pretend otherwise; §7 measures how many corpus flames that
  is.
- `sqrt`, `log`, `pow` with a negative base, `acos`/`asin` outside
  `[−1, 1]`: **refuse** in v1 rather than reason about the NaN the GPU
  would produce. Bodies that guard the domain (`elliptic`'s
  `sqrt(max(0.0, …))`) are handled naturally, because `max(0, X)` has
  a non-negative lower bound. Reasoning about "a NaN sample is
  discarded by bad-value recovery, so its input contributes nothing"
  is sound in principle and unsound on Metal in practice (fast-math,
  CLAUDE.md); not in v1.
- `sin`/`cos`/`tan`/`fract`/`floor` on a wide interval: the standard
  periodic rules (an interval spanning a period is the full range).
  Trig of a *huge* argument is garbage on every platform
  (`docs/accepted-divergences.txt`); the rule still returns `[−1, 1]`,
  which is sound whatever the GPU did.

Anything else → **refuse naming the intrinsic**, and the survey's
frequency list is the order to add them in.

### 4e. In and out

Input disc `B(c, r)` → box `[c−r, c+r]²`. Output box → disc at the
box centre with radius half the diagonal. Each direction loses a
factor `√2` at worst, which the enumeration absorbs and §5 recovers.

The returned value is the body's raw output; `ifs_ball::one` applies
the weight for `Normal`-phase variations exactly as it does for the
hand bounds (the contract's weight argument is kept so `pre_blur`,
which reads its own weight inside the body, keeps working).

## 5. Looseness, and the knob that fixes it

Interval arithmetic over-estimates whenever a variable appears more
than once (`x − x` is `[−2r, 2r]`, not `0`). For a bound that is
only inefficiency, but the *enumeration* needs a word's disc to
shrink, and a bound loose by a constant factor can stop it shrinking
altogether — which is exactly what the first `bubble` bound did
(`bound.rs` history: "sound and useless"), and it presents as
`NotContractive` or `TooManyWords`.

The standard fix is subdivision: split the input box `k × k`,
evaluate each cell, union the output discs. The union converges to
the true image as `k` grows, at `k²` the cost. `derive()` takes `k`;
the enumeration asks at `k = 1` and retries a transform that fails the
root contraction check at `k = 3` before refusing. Affine arithmetic
would be the next step past that and is not in this plan.

## 6. How it is proven

Everything below is a test that exists or is a small extension of
one that exists.

1. **Unit rules.** Each math rule against sampled ground truth: for
   an interval and 10⁴ points inside it, every `f(point)` lies in
   `rule(interval)`. Catches a transcribed rule, cheaply, without a
   GPU.
2. **Containment against the shipped shader** —
   `bounds_contain_the_real_wgsl` extended to iterate every
   *derivable* variation at the registry's default parameters over
   the same 12 discs, evaluating the real WGSL on the GPU through
   `lens::run_batch` and asserting every landing is inside the
   derived disc. This is the soundness gate, it already exists for
   the five hand bounds, and it is the single most important line in
   the plan: an unsound interval rule shows up here as a point that
   escaped, with the variation, the disc and the point named.
3. **Tightness.** For the five hand bounds, print derived radius over
   hand radius per disc. Expect 1–2×; a large ratio names a rule
   worth sharpening.
4. **The meter, re-run.** `what_blocks_targeting_across_the_corpus`
   gains a column: "bounded, but does not contract". Expect the "no
   forward bound" row to collapse to the genuinely unbounded set
   (poles, `tan`, state, 3D-only bodies) and the corpus to move from
   1 clear to the mid-teens on bounds alone.
5. **A picture.** One corpus flame newly unlocked, rendered targeted
   against untargeted at a paying zoom: overlap and brightness, the
   same gate `a_replayed_word_is_the_untargeted_render` runs on the
   gasket. The evaluator has done nothing until this passes on a
   flame somebody actually made.

## 7. Phases, each ending in a number

1. **Extraction + straight-line + math table.** Parse the probe
   batches, evaluate `Emit`-only bodies, params/weight/rng/init.
   Report: of 647, how many derive, how many refuse and why (by
   construct). Gate 2 runs on everything that derives.
2. **Branches and loops.** `If`/`Select`/`Switch`/`Loop` per §4c.
   Same report; the refusal set should now be poles, state, 3D-only
   and unmodelled intrinsics.
3. **Integration and looseness.** `ifs_ball::one` falls through
   `bound::for_name` → `bound::derive` → `NoBall::Unbounded` carrying
   the evaluator's reason (the panel already prints it). Subdivision
   retry in `plan`. Gates 3–5. Report: corpus flames that enumerate
   with `speedup > 1`.
4. **Colour in the replay arm** — *after* this, not before, because
   it blocks 23 flames and is never the sole blocker. The replay arm
   already runs `apply_variations` and computes the DC register; it
   discards it in favour of the affine fold. Using it lifts
   `ColourNotAffine` for every flame that reaches the replay arm,
   which is all the nonlinear ones.
5. Then xaos (8 flames; a per-predecessor sampling table).

## 8. Cost, and the fallback if it bites

`plan` calls `transform_ball_2d` once per candidate word — thousands
of times per view change. Evaluating a small IR is tens of
microseconds, so the worst case is a fraction of a second on a view
change with subdivision on, and the parsed modules are cached. If
that is felt in the app, the fallback is to derive a **Lipschitz
constant on the root ball once** per transform and use `L·r` per
node, which is what the affine path already does with `σ_max`. It is
looser, and it is the shape the deep-zoom plan originally assumed.

## 9. Not in this plan

- **3D bounds.** Targeting is 2D (the view test is a disc). A 3D
  body (`poincare3D`, `julia3D`, `spirograph3D` — 7, 2 and 3 corpus
  flames) refuses with "3D-only".
- **Per-thread state.** ~6 bodies; refused with the reason.
- **Affine arithmetic** or any tighter domain than intervals plus
  subdivision.
- **A hand-written WGSL parser.** naga or nothing.
- **Reasoning about NaN** as "discarded, therefore ignorable".

## 10. Bloat ledger

Zero new dependencies: naga is already linked. One new module
(`src/variations/derive.rs`, the evaluator and its rule table), a
`derive` entry point in `bound.rs`, a fall-through in `ifs_ball::one`,
a retry in `plan`, and the gate extensions in §6. The hand bounds
stay: they are tighter, they are the tightness reference, and the
evaluator's first job is to agree with them.

---

## 11. Phase 1, built and measured, 2026-09-19

`src/variations/derive.rs`. **170 of 647 bodies derive a bound**, and
the 173 that do so at any of the gate's discs were checked against the
shipped shader across **145,800 GPU evaluations with zero escapes**.

### What the plan got right

naga carried the whole extraction. `probe::shader::build` yields a
module that parses first time, with every helper (`cmul`, `csqrt`,
`ff_atan2`, `rng_nextf`) beside the bodies, and the evaluator compiled
against the real IR without a single surprise in its shape. The
environment is as small as §4b claimed.

### Two things it got wrong, both worth more than the code

**The weight chain was off by one step.** `transforms[i].variations[j]`
is three hops — table, transform, weight array — and collapsing the
first two made the chain arrive one index early, so the real
`[variation_id]` fell through to arithmetic on an opaque. Worth **71
bodies** of spurious refusal, and invisible except as a suspiciously
popular refusal reason.

**Init-derived slots were deferred and should not have been.** They
are the second-largest refusal at **122 bodies**, and the fix is
fifteen lines: the `wgsl_init` bodies are self-contained, so they parse
alone and evaluate with the user parameters as exact intervals. The
plan filed them under phase 1 and then the implementation postponed
them; the census said no.

Together those two took the count from 158 to 170, and moved the
refusals from *my gaps* to *the language*.

### The gate earned its place on the first run

`derived_bounds_contain_the_real_wgsl` found an unsoundness
immediately, in `r_circleblur`: the shader sent `(0.8, 0)` to a point
`1.65` from the origin while the derived disc claimed radius `1.41`.

The cause was **caching a local's value by expression handle**. Every
other expression in naga's IR is pure, so a handle names one value
forever — but a `var` is reassigned, and `bx = round(bx * rad)`
followed by `bx = bx + …` reads the same `LocalVariable` handle either
side of a store. The cached read returned the value from *before* the
store, silently dropping everything assigned after it, which makes the
bound too TIGHT: the one direction that corrupts a render.

Nothing but a containment check against the real shader would have
caught that. Not the unit rules — every interval rule was correct.
Not a tightness comparison — the disc looked plausible. It took a
point that escaped, and the fix is confirmed by putting the caching
back behind an env var and watching the gate fail with the same point.

### Where the remaining 477 go

    159  a branch                 phase 2
    159  a select                 phase 2
     70  a store to a component   phase 2 (a local's lane, `out[0] = …`)
     28  a switch                 phase 2
     20  a loop                   phase 2
     12  per-thread state         refused for good
     11  no rule for an intrinsic  (Acosh 6, bitwise 5)
      6  a pole                   refused for good, and correctly
      4  an unmodelled expression
      2  reads `subflame_metadata`
      2  arithmetic on the weights
      2  leaves the domain of log

Phase 2 is 436 of them — branches, selects, component stores, switches
and loops — which is close to the survey's prediction and confirms the
phase ordering. The permanent refusals are 18.

### Cost, and one decision deferred

`naga` is not re-exported by `wgpu` on wasm32, which is built without
the WGSL front end, so `derive` is desktop-only. Nothing depends on it
yet, so this costs nothing today; **phase 3 has to decide** whether
the web bundle carries its own parser (a real download cost for a
feature the web app may never ask for) or whether wasm keeps the hand
bounds. It is called out here so it is not discovered at wiring time.

### Gates

| | |
|---|---|
| every derived bound contains the real shader's output | 173 bodies, 145,800 evaluations, 0 escapes; fails on the caching bug |
| the interval rules contain their functions | sampled, no GPU, catches a transcribed rule |
| the weight chain is the only chain | asserts `variations` is the only field of `transforms` any body reads |
| a zero-crossing denominator is a pole | the rule that makes `curl` refuse honestly |
