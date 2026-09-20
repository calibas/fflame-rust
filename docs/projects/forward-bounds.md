# Forward Bounds — deriving a variation's disc bound from its WGSL

**Status:** phases 1 and 2 built and measured 2026-09-19 (§11, §12).
Planned 2026-09-19. The shared piece both
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

---

## 12. Phase 2, built and measured, 2026-09-19

Branches, selects, switches, loops and component stores.
**432 of 647 bodies derive**, up from 170, and the 450 that derive at
any of the gate's discs hold against the shipped shader across
**361,581 GPU evaluations with zero escapes**.

### The join is the whole of it

A condition answers yes, no or maybe. Deciding it where the ranges do
not overlap is what keeps a branch from doubling the result —
`if (p.y < 0.0)` over a disc entirely below the axis takes one arm —
and a maybe evaluates both from the same incoming state and unions
every local either one assigned. A `return` inside a maybe arm goes
into a list; the function's answer is the union of everything it
might have returned, and execution continues down the other path,
which over-approximates in the safe direction.

Loops iterate under the same join until their exit is definitely
true. A counter bounded by a literal or a parameter is an exact
`Int`, so it exits on the right iteration; anything still moving at
the cap refuses.

### Two refinements worth more than the control flow

**`x * x` is a square.** Interval arithmetic has no memory that both
factors move together, so `x * x` over `[-1, 1]` comes out `[-1, 1]`
when it is `[0, 1]` — and a denominator like `x*x + y*y + 1e-6` then
straddles zero and reports a pole that does not exist. Recognising
the identical operand costs one handle comparison, and the same trick
on `dot(p, p)` — the commonest expression in the catalogue — matters
even more, because without it every radial variation refuses at its
first division. Worth 74 bodies.

**An unwritten lane is zero, not an error.** Reading a component of a
`var` the body had only partly assigned was the largest refusal the
moment phase 2 landed, at 107 bodies, and all of them were legitimate
— WGSL zero-initialises a local.

### The gate caught two more, and they are opposites

Both were found only by containment against the real shader, and both
made a bound too TIGHT.

**`yin_yang`: declaration initialisers were ignored.** `var inv = 1.0;`
is a `LocalVariable` carrying an `init` expression, not a `Store`, so
a body that assigns the local inside one arm of a branch looks
unwritten on the other path. Joining its `-1` against a phantom zero
instead of the declared `1` put every output on the wrong side of the
origin. Locals are now pre-populated from their initialisers before
the body runs.

**`scry2`: the phase-1 cache fix was too broad.** Phase 1 stopped
caching `LocalVariable` *and* `Load` after `r_circleblur` read a stale
local. But `let r1 = r2;` is a `Load`, and its value must be r2's
value AT THAT POINT — leaving it uncached made `r1` re-read `r2`
after a later `r2 = r2 * r2`, so `d = r1 * (r2 + inv_w)` used the
squared value twice. At a point input the derived output was exactly
`1/p.x` times the shader's.

The rule that is actually right is narrower than either: **`Emit`
always recomputes**, so every expression it covers — including the
`Load` behind a `let` — is refreshed where the source says it is
computed and caching it is correct. Only `LocalVariable`, a pointer
that is never emitted and so never refreshed, must stay uncached.
One bug in each direction, from the same gate, in successive phases.

### Where the remaining 215 go

    50  a store through something other than a local
    48  no interval rule for an intrinsic   (bitwise 21, unary 19, Acosh 7)
    46  a pole
    21  per-thread state
     8  outside the domain of log
     7  pow with a negative base
     7  an unmodelled expression
     5  arithmetic on a raw RNG word
     4  arithmetic on the variation weights
     4  a join of incompatible shapes
     3  a non-constant parameter slot

The bitwise operators are integer hashing inside the noise family and
want an integer-range rule rather than an interval one. The 46 poles
are the interesting number: some are genuine — `curl` really is
unbounded — and some are the dependency problem the plan's §5
anticipated, which subdivision is for. Telling those apart is
phase 3's first measurement, because a spurious pole is a flame
refused for nothing.

### Gates

Unchanged in shape, larger in reach: 450 bodies and 361,581
evaluations, plus the five hand bounds still checked separately and
still agreeing.

## 13. Phase 3, built and measured, 2026-09-19

**514 of 647 bodies, wired into the renderer — and the number that
matters went from 37 blocked flames to 28, while the number that
actually renders moved by one.** Phase 3 was supposed to be
integration; it turned into the phase that found out what integration
was worth.

### Subdivision: measured, and mostly not the answer

§5 said a spurious pole is the dependency problem and subdivision is
the cure. Measured across the corpus it is barely that. Splitting the
input box `k×k` and unioning the outputs:

    k        bodies bounded (origin disc)   poles
    1                              350        46
    3                              365        44
    8                              365        43

Sixty-four times the work for three more bodies. `curl` and `rays`
refuse at every `k`, because their poles are real — `curl` divides by
`(1 + c₁x + c₂(x²−y²))²+…` which genuinely vanishes.

But `elliptic` comes back at `k=3`, and it is the textbook case: it
divides by half the sum of the distances to `(±1, 0)`, which the
ellipse property puts at `≥ 1` and which intervals read as possibly
zero because they cannot see the two square roots are linked. So
subdivision ships as a **retry on refusal at `k=3`**, not as a
blanket. The common path pays nothing and the one body that needs it
gets it.

### What the gate caught this time

A fourth unsoundness, and the most general one yet.

    `iconattractor_js` at input Ball { c: [1.5, 0.0], r: 0.05 }:
    the shader sent [1.453806, 0.019134] to [-1.717208, -0.043104],
    OUTSIDE the derived disc Ball { c: [-12.48, 0.28], r: 10.56 }

`if (i >= max_loop) { break; }` with a condition the evaluator cannot
decide. The `If` handler over-approximated by continuing down the
other path — right for what happens next — and **threw away the state
at the break**. So the loop reported only the state after its last
iteration. `iconattractor_js` rotates its point once per degree and
breaks out early; reading only the 24th rotation put the real output
outside the disc by a hair, in the one direction a forward bound may
never go.

The fix is a `maybe_exits` list on the frame: an undecided branch that
may have broken pushes its state, and each enclosing `Loop` drains
what its own body pushed and unions it into the exit. The same hole
for `continue` has no such repair — falling through runs statements
the real iteration skipped, and the result is not a superset of either
path — so that refuses by name. No shipped body reaches it.

**Four unsoundnesses, four found by the same gate, none findable by a
tightness check.** Caching a local; a missing declaration initialiser;
caching too little; and now a dropped loop exit.

### The rules that moved the number

    456  phase 2's evaluator, plus bitwise, LogicalNot, integer pow
    504  + discard a store through an OPAQUE out-parameter  (+48)
    509  + Acosh / Asinh / Atanh                             (+5)
    514  + pow's negative lower end clamped like sqrt's      (+5)

The first is the phase-2 ledger's biggest line and turned out to be
one rule. 50 bodies take `vc: ptr<function, f32>` — the colour
out-parameter of the `dc_*` family — and assign to it. Where the point
lands does not depend on the colour, and the caller discards it, so
the store can be thrown away. Soundness rests on the pointer being
**opaque**, not on its name: an opaque argument is one of the entry
point's own, so it cannot alias a tracked local, and a read back
through it loads the same opaque and refuses on the arithmetic. One
body does read its colour back, and refuses. That is the rule working.

The `pow` clamp is the same call `sqrt_iv` already made. `widen` is
absolute, so a mathematically non-negative `dot(p, p)` of `[0, 4]`
comes back as `[-ε, 4+ε]`, and the julia family raises exactly that to
a fractional power. Refusing it cost `juliascope` and `julia3D` six
corpus flames between them.

Every intrinsic in the shipped corpus now has a rule.

### What deriving costs

The soundness gate says a derived disc contains the real output. It
says nothing about how much bigger it is, and a bound twice as wide
halves the depth the enumeration reaches. The five hand bounds are the
only place both answers exist for one body:

    variation      hand r    derived r     ratio   worst at
    spherical     500.000   762698.520  1525.40x   c=[0,0] r=0.25
    bubble          0.023        0.065     2.87x   c=[-2,1] r=0.05
    julian          0.224        0.376     1.68x   c=[0,0] r=0.05
    pre_blur        3.050        4.313     1.41x   c=[-2,1] r=0.05
    blur            1.000        1.414     1.41x   c=[0,0] r=0.05

**Away from a pole, the evaluator is within about 3× of a bound
derived by hand.** At a pole it is hopeless: `spherical`'s worst case
is the disc centred exactly on its singularity, where the hand bound's
eigenvalue argument sees a supremum and interval arithmetic sees a
division by something containing zero. That is the whole shape of the
trade — write bounds by hand for the poles, derive everything else.

### The number this was all for

The co-occurrence meter improved a lot:

    blocker                        blocks  sole cause
    a variation has no forward bound   28          10   (was 37)
    colour is not affine (dc_*/rgb)    23           7
    xaos                                8           3

And it is misleading, which is the real finding of phase 3. `plan`
fails on the **first** blocker it hits, not on the union of them. Ask
it directly — run `Cylinders::plan` on each corpus flame at its own
framing, 64× / 1024× / 16384× deep:

    45 corpus flames:
         2 enumerate at some depth
         1 reaches a speedup above 1     (blur_test, 9.9e3×)

    refused at every depth:
        20  ColourNotAffine
         9  Unbounded
         8  Xaos
         5  TooManyWords
         1  NoInvariantBall

So: forward bounds were worth building — 9 flames still blocked on a
bound, down from what would have been 28 — but **colour is now the
dominant blocker by a factor of two**, and no further evaluator rule
touches it. The remaining `Unbounded` nine are `rays`, `curl`,
`subflame_wf`, `lorenz_js` and kin: genuine poles, a shader global the
probe module does not carry, and arithmetic on the running sum. Those
want hand bounds, one at a time, and there are nine of them.

`TooManyWords` at 5 is a new category and not a refusal in the same
sense — those flames enumerate, the antichain just grows past the cap
before the view is covered. Worth a look when the blockers above it
are gone.

### Phase 4 is colour, and the meter says so

Written into the plan as "blocks 23 flames, never sole cause", which
was true of co-occurrence and false of practice. It is the first
blocker for 20 of 45. The replay arm already computes the DC register
and throws it away.

### Gates

536 bodies, 443,148 shader evaluations, every one inside its disc. The
five hand bounds still checked separately and still agreeing. Plus
`what_deriving_a_bound_costs`, which prints the table above rather
than asserting a threshold that would have to be loosened the first
time a rule changed.
