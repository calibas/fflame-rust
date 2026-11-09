# Variations

**Overview:** A **variation** is a nonlinear map applied inside a
transform, after its affine. Each is a static definition holding its
metadata and its WGSL — a 2D body and a 3D body — and the registry
gathers them, together with any downloaded or locally installed ones.
A flame's shader compiles in only the variations the flame uses, and
each transform blends them by weight. The registry holds 647 shipped
definitions, mostly ported from flam3, Apophysis and JWildfire.

**See also:**
- [TRANSFORMS.md](TRANSFORMS.md) — the transform and the iteration the
  variations run inside
- [SHADERS.md](SHADERS.md) — how `apply_variations` is generated, the
  WGSL lints, the Metal fast-math rules, the math probe and the census
- [BUFFERS.md](BUFFERS.md) — the variation parameter rows and the state buffers
- [FOCUSED-RENDERING.md](FOCUSED-RENDERING.md) — the planners that
  consume inverse definitions and forward bounds
- [ESCAPE.md](ESCAPE.md) — the escape engine's IFS modes, which use the
  inverse definitions too
- [ONLINE.md](ONLINE.md) — downloaded variations, the catalog, plugins
  as a distribution channel
- [VARIATIONS_WIRE_FORMAT.md](../projects/VARIATIONS_WIRE_FORMAT.md) —
  the client ↔ API contract

**Code locations:**
- [src/variations/definition.rs](../../src/variations/definition.rs) —
  `VariationDef`, `Feature`, the `param!` macro (a parameter is a
  `params::ParamDef`, [src/params/mod.rs](../../src/params/mod.rs))
- [src/variations/mod.rs](../../src/variations/mod.rs) —
  `VariationRegistry`, `VariationInfo`, `ParamType`, `VariationPhase`,
  `VariationCategory`, `global_registry()`, the shader lints
- [src/variations/defs/](../../src/variations/defs/) — the definitions,
  in 208 files; [defs/mod.rs](../../src/variations/defs/mod.rs) holds
  `ALL_VARIATIONS`
- [src/variations/inverse.rs](../../src/variations/inverse.rs) —
  inverse definitions for the inverse walks
- [src/variations/bound.rs](../../src/variations/bound.rs),
  [src/variations/derive.rs](../../src/variations/derive.rs) — forward
  bounds, by hand and derived from the WGSL
- [src/variations/docs.rs](../../src/variations/docs.rs) — prose read
  back out of the definitions' doc comments
- [src/variations/analytic_blur.rs](../../src/variations/analytic_blur.rs)
  — kernels for the analytic blur path
- [src/shader_builder_v2.rs](../../src/shader_builder_v2.rs) — the
  per-flame dispatcher and helper-library splicing

---

## Definitions

A variation is a `pub static` `VariationDef` in a `defs/*.rs` file:

| field | means |
| --- | --- |
| `name` | The wire name — the key in a transform's `variations` map and in `.flame` XML. Usually snake_case; JWildfire names keep their case (`linear3D`, `roundspher3D`). |
| `aliases` | Foreign names (Apophysis, JWildfire, Chaotica, or a name we renamed) that resolve to this one. |
| `display_name` | UI label. |
| `category` | Where the UI files it (*Categories*). |
| `phase` | `Pre`, `Normal`, `Post` or `Any` (*Phases*). |
| `features` | Capability flags (*Features*). |
| `parameters` | User parameters, usually written with `param!`. |
| `init_param_count`, `wgsl_init` | Derived parameter slots and the WGSL that computes them (*Parameter slots*). |
| `state_count`, `wgsl_state_init` | Per-thread state slots and an optional initialiser. |
| `wgsl_2d`, `wgsl_3d` | The two bodies. Both are required. |

The definition's `///` doc comment is part of it too: it is the only
place a variation's description and authors exist (*The docs corpus*).

## The registry

`VariationRegistry` holds a `VariationInfo` per name, an alias table,
the names in registration order, and a version counter.
`VariationInfo` is the runtime form, shared by every source:

| field | from a `VariationDef` |
| --- | --- |
| `name`, `display_name`, `category`, `phase`, `parameters` | copied |
| `wgsl_function` | `variation_<name>`; the one exception is `julia`, whose function is `julia` |
| `features` | copied, as a `Vec` |
| `provenance` | `Builtin`; a download is `Api { version }`, a Rhai variation `Local` |
| `wgsl_source`, `wgsl_source_3d`, `wgsl_source_init`, `wgsl_source_state_init` | the bodies (optional, because a download may lack one) |
| `init_param_count`, `state_count` | copied |

**Registration.** `VariationRegistry::new()` registers every entry of
`defs::ALL_VARIATIONS` in order. The list is **append-only**:
registration order is the stable order the registry reports
(`names()`), the fallback dispatch order and the UI order within a
category, and legacy configs that stored weights as an array are read
by it.

**Global access.** The registry is a process-wide
`RwLock<VariationRegistry>`. `global_registry()` returns a
`RwLockReadGuard`; `global_registry_mut()` a write guard, used to
register downloads and plugins at run time and to clear the download
cache. Every runtime add or remove bumps `version()`, which the shader
cache watches. Drop a read guard before anything that may take the write
lock -- `variations::custom::ensure`, which registers a config's Rhai
variations, does.

**Lookups.**
- `get(name)` and `has(name)` resolve aliases.
- `resolve_alias(name)` returns the canonical name.
- `aliases_for(canonical)` lists a variation's aliases, sorted.
- An alias may not shadow a real name, and the first registration of
  an alias wins.
- Transforms always store the canonical name. `.flame` import and
  `.fflame` deserialisation both canonicalise, parameter keys included.

**Ordering for the UI.** Iterate `names()`, `all()` or
`by_category(c)`, never the internal `HashMap`. The Variations panel
and the Transforms panel's picker walk the categories in a fixed order
and list each one with `by_category`, which keeps registration order.

**Removal.** `clear_api()` removes only `Api` variations — built-ins
are compiled in, and `Custom` ones are the user's own.
`remove_by_name()` removes one variation of any provenance.

### Categories

| category | holds |
| --- | --- |
| `Basic2D` | the flam3 originals (`linear`, `sinusoidal`, …) |
| `Advanced2D` | most 2D ports |
| `Depth3D` | z-only maps (`zcone`, `flatten`, `zscale`, …) |
| `Rotation3D` | `pre_rotate_x/y`, `post_rotate_x/y` |
| `Full3D` | 3D in character, with a real 2D body too |
| `Only3D` | no meaningful 2D reading; empty today |
| `Plugin` | JWildfire plugin ports, and any downloaded variation whose category this client does not know |

Categories are cosmetic except `Only3D`, which `ShaderBuilder` drops
from 2D shaders. Every other variation contributes its 2D body in a 2D
flame. The per-category counts are published in
[docs/generated/engine-contract.json](../generated/engine-contract.json).

---

## Phases

A transform applies its variations in three phases
([TRANSFORMS.md](TRANSFORMS.md), step 4):

| phase | input | combine | weight |
| --- | --- | --- | --- |
| `Pre` | the point after the affine | `temp = f(temp)`, chained in order | applied inside the body |
| `Normal` | `temp` after the pre phase | `result += w · f(temp)` | applied by the dispatcher |
| `Post` | `result` | `result = f(result)`, chained in order | applied inside the body |

**`Any`** — most shipped variations — is phase-agnostic: it runs in the
normal phase unless a transform overrides it with JWildfire's
`fx_priority`. `Transform::variation_priorities` holds the raw JWF
integer, sparsely: an entry exists only where it differs from the
variation's natural priority (`Pre` −1, `Normal`/`Any` 0, `Post` 1).
The dispatcher buckets by sign: negative is pre, positive post. Only
`Any` variations honour an override; `Pre`, `Normal` and `Post` stay
where their definition puts them. Every transform's override counts --
normal, Linked and Final alike, as JWildfire partitions the phases per
XForm.

An `Any` variation moved to pre or post is still a normal-style body,
so the dispatcher applies its weight: `temp = temp + w·f(temp)`, or
`temp = w·f(temp)` if it has `Feature::Replace` (and the same on
`result` in post). When one variation lands in different phases on
different transforms, each placement is gated on `xform_id`, and the
normal placement is the catch-all, so Linked and Final members run it
there.

**Order within a phase.** Pre and post run in local-index order, which
follows the flame's variation order ([TRANSFORMS.md](TRANSFORMS.md),
*Per-flame local indices*). In the normal phase, accumulating
variations run first and `Replace` variations last — a `Replace`
assigns `result = w · f(temp)`, overwriting what the others summed, as
JWildfire's `pVarTP.x = …` does. Order matters only for chained pre and
post variations, `Replace`, `NeedsAccum`, and several variations
writing one colour register; a weighted sum is order-free up to
rounding. Where one transform's recorded order differs from another's,
the flame-wide local index decides. The app's sticky superset reorders
the map canonically, but only for flames where none of these can bite
([SHADERS.md](SHADERS.md#the-sticky-variation-superset)).

The generated code is shown in [SHADERS.md](SHADERS.md); the
`fx_priority` model in [jwf-features.md](../projects/jwf-features.md).

---

## Features

Each flag in `features` changes the variation's signature, the
dispatch, or what the shader compiles in. The shader builder checks
them with `has_feature`; the vocabulary on the wire is
`Feature::to_api_str`.

| feature | effect |
| --- | --- |
| `NeedsRng` | The body takes `rng: ptr<function, RngState>` and draws with `rng_nextf(rng)`. |
| `NeedsTransform` | The body takes `xform_id, variation_id` even without parameters, to read `transforms[xform_id]` — its affine, colour, or its own weight. |
| `WritesColor` | Direct colour: the body takes `vc: ptr<function, f32>` and writes a palette index, blended in by the transform's `direct_color`. Compiles `HAS_DC`. |
| `WritesRgb` | Direct RGB: the body takes `vrc: ptr<function, vec3<f32>>` and writes a colour (it must stay well inside ±1e29; the register starts at a −1e30 sentinel). Compiles `HAS_RGB`. |
| `NeedsAccum` | The body takes `accum` — the running weighted sum — right after `p`, for JWF variations that read `pVarTP`. Normal and post phases only. |
| `AlwaysZ` | Its z contribution survives `preserve_z = false` (*Z semantics*). |
| `NeverZ` | Its z contribution is always zeroed. Mutually exclusive with `AlwaysZ`. |
| `Replace` | It assigns rather than accumulates (*Phases*). |
| `CanHide` | The body takes `hide: ptr<function, bool>` and may set it to drop this iteration's splat (JWF `doHide`; the `cut_*` family). On a normal or Linked transform the iteration also reverts its point. |
| `VolumeSideEmit` | Writes an extra depth-only point to `volume_side_point` / `volume_side_flag`, splatted into the solid depth buffer and shadow maps; the image is untouched. 3D, solid rendering only. |
| `PlotEmits(n)` | May plot up to `n` extra points per call (*Multi-emit plotting*). |
| `AnalyticBlur` | An input-independent blur that the analytic blur path can render as a mean splat plus a convolution ([analytic-blur-buffer.md](../archive/projects/analytic-blur-buffer.md)). Carried by `analytic_blur` and `analytic_gaussian_blur`. |
| `NeedsW` | Carries the per-thread 4th coordinate: read `point_w`, write the raw output to `point_w_out`; the dispatcher weight-sums it like x/y/z. 3D only. |
| `NeedsMobiusLib` | Splices in the SL(2,ℂ) Möbius library (*Helper libraries*). Changes nothing else. |
| `ResetsOrbits` | A flame using it at a weight other than 0 starts its orbits afresh every dispatch, the config's `reset_orbits` held on ([RENDERER.md](RENDERER.md) *Persistent orbits*): for a map whose orbits settle, whose picture is their way from their starts. A renderer flag only; a Rhai variation declares it with `resets_orbits: true` in `INFO`. With a parameter named `orbit_length`, its value is the orbits' length, saved with the flame, in place of the device's Iterations per Thread (`Flame::orbit_length_source`, at most 10,000). Carried by no built-in; Iterated Dynamics' orbit-map examples (`variation-hopalong.rhai`, ...) have it, as their pictures change with how far each orbit goes. |

Per-feature counts are in the generated contract.

### Z semantics

JWildfire has three classes of z behaviour, and a 3D shader reproduces
them in the normal phase:

| class | JWildfire source | our flag | z under `preserve_z = true` | under `false` |
| --- | --- | --- | --- | --- |
| gated (default) | `if (isPreserveZCoordinate()) pVarTP.z += amount·z` | none | the body's z | zeroed |
| always | unconditional `pVarTP.z` writes | `AlwaysZ` | the body's z | the body's z |
| never | no z write | `NeverZ` | zeroed | zeroed |

A 2D-shaped 3D body passes `p.z` through, which is exactly the gated
add when kept and exactly JWF's skip when zeroed. `preserve_z` is a
`FractalConfig` field ([TRANSFORMS.md](TRANSFORMS.md)).
`scripts/audit_z_write_semantics.py` classifies the JWildfire sources
in `output/variation-jwf-source/` and adds `AlwaysZ` where the source
writes z unconditionally; it reports "never" candidates without
applying them, because some of our 3D bodies extend a 2D variation on
purpose. Set either flag by hand only with the source in front of you.
See [preserve-z-semantics.md](../archive/projects/preserve-z-semantics.md).

### Multi-emit plotting

A `PlotEmits(n)` variation plots extra points without changing the
walk. The shader builder generates, only when such a variation is
active (`HAS_PLOT_EMIT`):
- `emit_plot(p)` and `emit_plot_weighted(p, w)` — an absolute point;
- `emit_plot_offset(p, w)` — an offset from wherever the iteration
  finally plots, after post-affine and the Final chain, so a blur-style
  emitter works on any transform;
- `emit_suppress_main()` — drop the iteration's own plot but keep the
  emissions.

The contract:
- The return value keeps its usual meaning; emissions never enter the
  chaos game.
- Emissions respect the opacity draw and pass through post-symmetry and
  every plot-time effect, but not through a Final chain — put an
  emitter on the Final.
- `w` scales all four histogram channels, so `w = 1/N` keeps an
  `N`-emitter's brightness.
- Do not pair with `CanHide`: a hide on a normal transform abandons the
  iteration, emissions included.
- The cap is the largest `n` among the active variations, clamped to
  16; calls past it are dropped.

Three variations use it: `stereogram`, `multi_kaleidoscope`,
`spray_blur`. A Rhai variation emits with `emit(z[, w])`,
`emit_offset(d, w)` and `suppress_main()`, its `n` declared as
`INFO.emits` ([ESCAPE.md](ESCAPE.md), *A Rhai transformation*). See
[multi-emit-stereograms.md](../archive/projects/multi-emit-stereograms.md).

---

## Parameters

### Types

A parameter is a `params::ParamDef`, the definition every kind shares
([one-parameter-system.md](../archive/projects/one-parameter-system.md)): a
name, a label, a hint (the tooltip), its kind with its default and
range, and whether it is slow. `param!` writes one:

| `param!` form | kind (on the wire) | stored as |
| --- | --- | --- |
| `float, default, min, max` | a float (`float`) | the value; the slider is clamped to min–max |
| `unlimited_float, default, min, max` | a free float (`unlimited_float`) | min–max is the slider range; any value can be typed |
| `int, default, min, max` | an int (`integer`) | an integer in an `f32` |
| `unlimited_int, default, min, max` | a free int (`unlimited_integer`) | as an int, with typing past the slider |
| `angle, default` | an angle (`angle`) | degrees, slider −360…360; the body converts |
| `bool, default` | a switch (`boolean`) | 0.0 or 1.0 |
| `enum, default, &[choices]` | a choice (`enum`) | the choice's index |

Each form also takes a trailing description, and `param!(slow ...)`
marks any form slow. The Transforms panel, the escape transformations
and the lens draw them through the one renderer (`ui::params`). The
wire names are the contract's vocabulary (`ParamType`), which
`params::wire_type` spells a definition in.

On a transform, values live in `Transform::variation_params` under
`"variation.param"` keys. A missing key means the definition's default,
both on the CPU (`variation_param_or_default`) and in the upload.

### Parameter slots

A variation owns `slot_count() = parameters.len() + init_param_count`
consecutive floats in its transform's parameter block:

- **User parameters**, in declaration order, are uploaded as stored.
- **Derived parameters** are computed on the GPU. `wgsl_init` defines
  `fn init_<name>(user: array<f32, N>) -> array<f32, M>` with `N` user
  parameters and `M = init_param_count`. When parameters change, an
  init compute pass runs it for every (transform, variation) pair that
  uses it — subflame transforms included — and writes the results after
  the user parameters. Use it for anything a body would otherwise
  recompute from its parameters every iteration -- or read from a
  constant table at a run-time index, which naga copies whole at every
  read: polychoron and menger copy their vertices from polychora.wgsl's
  table here (their name-gated library is the init's too), and ran
  2-10x faster where they read it, 2D and 3D.

The layout is packed per flame: each active variation gets the same
offset in every transform's block, in local-index order, and the
generated `get_param(xform_id, variation_id, slot)` maps the local
index to that offset. A transform's row holds up to
`MAX_VARIATION_PARAM_SLOTS` = 262,144 floats (1 MiB), and the buffer
grows to fit the flame's rows, so an init may compute a table of
thousands of floats once rather than a body reading a constant at a
run-time index (which naga copies whole at every read). See
[BUFFERS.md](BUFFERS.md) and [SHADERS.md](SHADERS.md).

### Per-thread state

`state_count` slots of `f32` per (transform, variation) instance, per
thread, read and written with `get_state(xform_id, variation_id, slot)`
and `set_state(xform_id, variation_id, slot, value)`. They start at
zero; `wgsl_state_init`, if given, runs once at thread start with
`xform_id`, `variation_id` and `set_state` in scope.

- Slots are allocated only for instances with a non-zero weight; the
  accessors exist whenever an active variation declares state.
- Two transforms using the same stateful variation get independent
  state.
- With persistent orbits the state rides in the orbit buffer, so it
  survives from one dispatch to the next. A flame carrying state
  restarts its orbits at every restart of the accumulation, as
  JWildfire reinitialises variations per render
  ([RENDERER.md](RENDERER.md)).
- `MAX_STATE_SLOTS_PER_FLAME = 1024` is a soft cap: past it the build
  logs a warning.

`subflame_wf` keeps its nested chaos game in five state slots
([TRANSFORMS.md](TRANSFORMS.md), *Subflames*).

---

## Writing the WGSL

**Names.** The function is `variation_<name>`, in both bodies. Any
helper is a top-level `fn` prefixed with the variation's name
(`fn myvar_helper`), because every active variation's bodies share one
module. The builder splits bodies into top-level functions and emits a
byte-identical duplicate once — so sibling variations can share a
helper — and panics, naming both variations, when two bodies define one
name differently.

**Signature.** Arguments follow `p` in this order, each present only
when its condition holds:

| argument | when |
| --- | --- |
| `p: vec2<f32>` (2D) / `vec3<f32>` (3D) | always |
| `accum: vec2/3<f32>` | `NeedsAccum` |
| `xform_id: u32, variation_id: u32` | the variation has parameters, or `NeedsTransform` |
| `rng: ptr<function, RngState>` | `NeedsRng` |
| `vc: ptr<function, f32>` | `WritesColor` |
| `vrc: ptr<function, vec3<f32>>` | `WritesRgb` |
| `hide: ptr<function, bool>` | `CanHide` |

The return type is the point type. Read parameter `k` with
`get_param(xform_id, variation_id, k)`.

**Both bodies.** `wgsl_3d` is required. For a 2D-shaped map it repeats
the 2D math on `p.xy` and returns `p.z` unchanged, which gives the
gated z behaviour above.

**Weight inside the formula.** Some JWildfire variations use their
weight inside the formula rather than as a final scale. A normal-phase
port reads its own weight,
`transforms[xform_id].variations[variation_id]` (so it needs
`NeedsTransform` or parameters), and divides its result by it, so the
dispatcher's `w · f(p)` gives back the intended value. `idisc` in
[defs/apo_misc.rs](../../src/variations/defs/apo_misc.rs) is the
pattern. Native pre- and post-phase variations always apply their own
weight.

**Per-flame specialisation.** A body can be re-emitted per flame
(`variation_specialized_source` in `generate_variation_code`): `synth`
cuts the modes the flame does not use before compilation, and
`mondrianomies` bakes in its Exact fill's rectangles, a table the CPU
computes from the drawing for each seed, depth and drift in the flame
(`exact_combos`), in place of a stub. A new specialiser is a new arm
there. One that bakes in data makes the def's WGSL less than the
variation, so mondrianomies has no Rhai twin (below).

**GPU arithmetic.** The shader lints reject self-compares (`x != x`),
self-divisions (`x / x`) and f32-subnormal literals (`1e-40`) in every
variation body; Metal runs with fast-math, so NaN tests and `atan2` at
zero behave differently there — use `!(abs(x) <= 1e32)` and `ff_atan2`.
The rules and their reasons are in [SHADERS.md](SHADERS.md).

### Helper libraries

| library | spliced in when |
| --- | --- |
| `complex.wgsl` | always (dead-code-eliminated when unused) |
| `subflame.wgsl` | `subflame_wf` or `pre_subflame_wf` is active |
| `fractwf.wgsl` | any `fract_*_wf` is active |
| `noise.wgsl` | `dc_perlin`, `crackle`, `dc_crackle_wf` or `crackle_fast` |
| `voronoi.wgsl` | `crackle`, `dc_crackle_wf` or `crackle_fast` |
| `polyhedra.wgsl` | `polyhedron` or `polyhedron_volume` |
| `polychora.wgsl` | `polychoron` or `menger` |
| `su_mobius.wgsl` | any active variation has `NeedsMobiusLib` |

All but `su_mobius.wgsl` are gated on **names**, in
`ShaderBuilder::build_definitions`; the name-gated libraries from
`fractwf.wgsl` on are one table, `NAME_GATED_LIBRARIES`
(`src/shader_builder_v2.rs`), which the engine contract publishes as
`helper_libraries.name_gated`. A downloaded variation can therefore
reach only `complex.wgsl` and the Möbius library; anything else it
needs must be in its own body.

---

## What variations tell the planners

Focused Rendering and the escape engine's IFS modes analyse a flame
rather than iterate it, and need two kinds of extra knowledge per
variation. Both live beside the variation, in side tables keyed by its
name, so most definitions carry nothing.

**Inverse definitions** ([inverse.rs](../../src/variations/inverse.rs)).
An `InverseDef` names a variation and its `InverseKernel`:
- `Affine` — what the variation contributes to the transform's affine
  map at weight `w`, in the planar or the solid space;
- `Planar` — a 2D kernel the inverse walk can invert;
- `Solid` — a 3D one.

A kernel may refuse at given parameters (`Refusal::Degenerate`, or
`Mode` for a mode the walk does not invert). `INVERSES` is append-only
and holds 26: twelve affine roles, eleven planar kernels and three
solid ones. The `InverseDef` statics sit in the variations' own
`defs/*.rs` files. `VariationRegistry::inverse(name)` resolves through
aliases. The users are the backward planner and IFS analysis in
`src/scene/` and the escape engine's IFS code
([FOCUSED-RENDERING.md](FOCUSED-RENDERING.md), [ESCAPE.md](ESCAPE.md)).

**Forward bounds** ([bound.rs](../../src/variations/bound.rs)). Given a
disc of inputs, a bound names a disc that contains every output, as the
variation's contribution lands (weight included). It must
**over-estimate**: too loose only costs the planner efficiency, too
tight silently drops part of the picture.
- `BOUNDS` holds hand-derived bounds — `spherical`, `bubble`, `julian`,
  `pre_blur`, `blur` — each with its derivation beside it.
- Anything else is derived from the shipped WGSL by
  [derive.rs](../../src/variations/derive.rs): interval arithmetic over
  the naga IR, refusing any construct it does not model. Desktop only;
  the web build has no WGSL front end and uses the hand bounds alone.
- `ARMED` lists the variations whose random draw picks one of finitely
  many arms (`julian`, `juliascope`). The planner enumerates the arms,
  and a replay build rewrites the draw to read a forced arm.

**A Rhai variation's own** ([rhai-planners.md](../projects/rhai-planners.md)):
`derive` reads a registered Rhai variation's compiled body as a
built-in's, so it has a derived bound wherever its built-in twin would
(472 of the 472 twins whose built-in derives); and one may declare its
inverse (`fn inverse`, *Variations written in Rhai*), which the analysis
takes as `Kernel::User` / `Kernel3::User3` -- the Flame Attractor in the
plane, summed with an affine, and as a solid. **A built-in with no
`InverseDef` takes its twin's** (`user_inverse::of`): writing a twin's
inverse in `scripts/twin_inverses.py` gives the built-in the Flame
Attractor (swirl, horseshoe, polar, exponential, exp, log, fisheye,
eyefish, sinusoidal, hyperbolic so far).

See [forward-bounds.md](../projects/deep-zoom/forward-bounds.md) and
[ifs-general.md](../projects/deep-zoom/ifs-general.md).

---

## Downloaded variations

The registry also takes variations at run time, in the API's
`VariationDownload` JSON, through `register_from_api(download,
provenance)`: when a flame names an unknown variation, the app fetches
it from the API, caches it, and registers it (`Provenance::Api`).
`storage::load_installed_resources` loads the cache at startup. (Local
WGSL plug-in files under `plugins/variations/` were retired 2026-11-01 ([rhai-plugins.md](../projects/rhai-plugins.md), D3): nothing in the UI ever installed one, and Rhai plug-ins replace them.)

**Rhai plug-ins** ([rhai-plugins.md](../projects/rhai-plugins.md), P3): a Rhai variation, effect, formula, colouring, solid or simulation colouring in the user's formulas with **Load at start** set in the Formula Editor (`formulas/_links.json`, `storage::links::Link::plugin`) is registered at every start, after the downloads (`storage::library::load_plugins`, from `load_installed_resources`: desktop, web and headless), under its key in the user's namespace (`Provenance::Local`). It is then in the lists without a config carrying it -- the variation pickers, the Effects panel's Add lists and the escape panel's formula and colouring pickers, each under "Your plug-ins" -- and choosing it carries its source into the config, as Use does. Saving the file registers the new version; turning it off unlists it; deleting it forgets the flag. One that does not register is named at start (a notice; stderr headless: `storage::library::plugin_failures`).

The registry's rules:
- A built-in can never be replaced.
- A download cannot displace a `Custom` (Rhai) variation.
- A download with no 2D body is refused unless its category is
  `only_3d`, and an `only_3d` one with no 3D body is refused.
- A variation with no 3D body is dropped from 3D flames, with a
  warning.

### Variations written in Rhai

A **custom variation** (`Provenance::Local`, `variations::custom`) is
a Rhai transformation from the Formula Editor -- `fn variation(z, p)`,
a point to a point ([ESCAPE.md](ESCAPE.md#the-formula-language)) --
compiled to the same `VariationDownload` a download is: its parameters
(a complex one as two, `name_re` and `name_im`), category `plugin`,
phase normal, and WGSL from the formula language's emitter. The 2D
function takes the point, and the ids when it has parameters (as
`shader_builder_v2::variation_call` passes them); the 3D one runs the
2D body on `p.xy` and passes `p.z` through. Helpers are prefixed
`cv_<name>_`.
- **`fn prepare(p)`** is its derived parameters: compiled to
  `fn init_<key>(user: array<f32, N>) -> array<f32, M>`
  (`lang::wgsl::variation_init`, `shader_init` and `init_param_count` on
  the wire), every value its map returns in the `M` slots after the
  parameters, in declaration order, so the 2D and 3D bodies read them
  alike (`get_param(xform_id, variation_id, N + k)`). The init pass runs
  it wherever a built-in's runs -- the render, the exporter, the GPU
  planner, the probe -- and the interpreter runs it at f64 before the
  body (`VariationRun::new`). Not in the escape lens, which runs no init
  pass (`lens_menu` leaves such a variation out), nor inside a formula
  (`vary` refuses it). A transformation's parameters and derived values
  share its transform's block (`MAX_VARIATION_PARAM_SLOTS`), not the
  formulas' 64 floats.
- **The flame carries the source** (`Flame::custom_variations`, skip if
  empty; `ConfigPath::FlameCustomVariations`, one undo step), so a saved
  flame renders anywhere. `custom::ensure` registers a flame's sources
  (and its subflames') before anything reads the registry for it: on
  load (`missing_variations_in`, so it is not fetched from the API),
  when a `FlameRenderer` is built, `load_config`, `update_flame`, and
  `render_with` (an escape lens, CLI, thumbnails, video).
- **Keyed by id and version** (design rule 7): it registers as
  `scripted::key(id, source)`, `u<the id's 32 hex>_<the first 8 hex of
  the source's SHA-256>` -- an identifier, so it is the WGSL function's
  name (`variation_<key>`) and its parameters' prefix (`<key>.k`). Two
  versions are two entries, and a flame renders the version it carries.
  The id is `INFO.id`, a UUID; a source from before ids has `INFO.name`
  alone, and its id is derived from that name (`scripted::derived_id`,
  a v8 UUID of its SHA-256), so the same source keys the same way
  everywhere. `INFO.name`, a lowercase identifier, stays what
  `vary("name", z)` calls it by. Configs from before (v6) carried it
  under its name; the v7 migration renames it to its key wherever the
  config names it (`migrate_custom_variation_keys`).
- **Use** (the Formula Editor) writes a UUID into a source that has
  neither an id nor a name, or a hand-written id (`custom::needs_id`).
  Using a new version moves the config to it in one undo step
  (`custom::rekey`: every transform's weight, parameters, order and
  priority, the lens, the escape transformations), the old source
  going; the old version stays registered, so the undo renders. Live
  follows a variation by its id. Pickers offer one version of each id
  (`VariationRegistry::superseded`): the one the flame carries, else
  the last registered; a picked one the flame does not carry comes with
  its source (`custom::carry_change`). The sticky shader set keeps no
  older version of an id that is live (`renderer::sticky`), as each
  Live edit would otherwise fill its cap with dead code.
- **`.flame` export drops it** (JWildfire has no such variation): its
  weight, its parameters and its `plugins=` entry, with a warning --
  unless it is a built-in's twin, unedited, which is written as the
  built-in (*Twins*).
- **As a lens**: the lens menu lists custom variations after the
  measured ones (`lens::lens_menu`).
- **Inside a formula**: `vary("name", z)` inlines one into an escape
  formula or colouring, the entry carrying its source
  ([ESCAPE.md](ESCAPE.md#the-formula-language)). A Rhai solid calls a
  BUILT-IN variation's 3D body on a vector the same way, through the
  maps flame built in 3D (`flame_map3`).
- **Hooks** (`lang::wgsl::variation_shape`): `rand()` and
  `rand_u32()` (`rng_nextf`, `rng_next`) declare `NeedsRng`, `ctx.accum` `NeedsAccum`, a `#{ z, color }` result
  `WritesColor`; `init(p)` and `this.name` are per-thread state
  (`state_count`, the seeding call as `wgsl_state_init`); state or
  `ctx.weight` without parameters declares `NeedsTransform` for the
  ids. The function takes them in `variation_call`'s order. Gate:
  `the_hooked_twins_render_as_the_builtins` (`blur`, `macmillan`).
- **Its own inverse** (`variations::user_inverse`; [rhai-planners.md](../projects/rhai-planners.md),
  Part B), for the Flame Attractor: `fn inverse(z, p, branch)` gives the
  preimage of `z` under `variation` on branch `0 .. INFO.inverse_branches`
  (1 by default, at most 16), or `no_preimage()` where there is none;
  `fn image_gap(z, p)` a lower bound on how far `z` lies outside the
  image (0 or less inside), where the image is not the plane, which the
  walk scores a point off it by as it does the built-ins' unit discs;
  `fn inverse3(v, p)` the solid's, one branch. Each is lowered as a
  transformation (`rhai::inverse_transformation`, its `branch` read from
  the colour coordinate the callers set) and **checked where it is
  used**: forward then inverse at 400 points over the disc (ball) of
  radius 4 must give each point back on some branch to a relative 1e-6,
  one in fifty spared, and the gap must be a lower bound near the image
  -- else the analysis refuses the transform, naming the worst point
  (`NotAffine::Inverse`). A many-valued forward map (`rand()` picking a
  root) has its arms read off it -- every draw of a call set to each step
  of a grid of 256, a run of steps with the same images an arm, its share
  bisected (`UserKernel::arms`; rhai-planners C1). It may reach for
  nothing only a flame has (a random number in the inverse, state, the
  weight, a texture). In mode D its WGSL is spliced per variation
  (*ESCAPE.md*, Mode D). **Focused Rendering's inverse walk** takes it
  too (C2): a flame with one that draws is the walk's, a word names an
  arm by its grid draw, and a replay build wraps the body's draws to
  read it (`bound::force_arms`). Past f32 the replay runs in offsets
  through the variation's **forward difference derived from its body**
  (C3, `lang::delta::variation_difference`: the body as one pass of an
  escape loop, whose derived equations are `F(Z + d) − F(Z)`), on the
  CPU in the interpreter and on the GPU spliced behind the hand forms
  (`forward_delta::replay_delta_wgsl`); a body that derives none replays
  absolute. Grand-julian with its julians Rhai twins renders the
  built-in's density block by block to 1e6. The walk's CPU evaluator
  replays a word over its points at once, map by map, and runs a user
  kernel's forward map as one pass of the interpreter for all of them
  (`VariationRun::call_batch`, E4: each node dispatched once, values the
  same in every lane kept single, branches under masks, every lane the
  scalar call's to the bit), 3.4x the scalar replay; grand-julian's
  twin plans in about twice the built-in's time, where it took three to
  six times. The twins of the eleven planar kernels carry one, and of
  julia3D and julia3Dz a 3D one (`scripts/twin_inverses.py`, which the
  translator appends): an edited twin keeps its inverse while it still
  undoes the map. Gates: `the_twins_inverses_undo_their_maps`;
  `a_twins_own_inverse_renders_the_flame_attractor`,
  `a_builtin_borrows_its_twins_inverse`,
  `a_twins_own_3d_inverse_renders_the_solid` (GPU, against the
  reference walk).
- **3D**: `fn variation3(v, p)` is the 3D body, its point a vector; alone
  it makes the variation `only_3d` (no 2D body, kept out of the lens
  menu). Either body takes the hooks either needs, and a 3D body
  declares `AlwaysZ`. Gate: `the_3d_twins_render_as_the_builtins`
  (`spherical3D`, `hemisphere`, `zcone`, in a pitched 3D flame). A 3D
  body may compute with quaternions (`quat`; [ESCAPE.md](ESCAPE.md), the
  formula language's Quaternions): `assets/formulas/variation-escape-paths.rhai`
  draws Halayka's escape paths of quaternion orbits as a flame's fibres.
- **A variation that builds its own points**: one that ignores its input
  and returns a fresh point each call draws a whole set from one
  transform. `assets/formulas/variation-moran.rhai` draws level-dependent
  IFSs (Moran sets: one map per level, each level's from its own set, so
  a Sierpinski triangle can be made of ferns); `escape::moran` holds its
  presets and tests. Levels of the user's own transforms need no
  variation: xaos and a subflame draw them ([TRANSFORMS.md](TRANSFORMS.md#xaos),
  and the record,
  [level-dependent-flames.md](../archive/projects/level-dependent-flames.md)).
- Gates: Rhai twins of linear, spherical, swirl, polar and horseshoe
  render as the built-ins in a flame, by density
  (`the_twins_render_as_the_builtins_in_a_flame`), and as lenses, pixel
  for pixel (`the_twins_are_lenses_as_the_builtins`).
- **Twins of the built-ins** (`variations::twins`,
  `assets/variations/twins/`; [rhai-plugins.md](../projects/rhai-plugins.md),
  P7 step 3): 640 of the 647, each a Rhai variation under its
  built-in's display name with its parameters exactly (a parameter may
  be `free: true` with both bounds, a built-in's unlimited number, and
  `exponential: false` over five decades) and its drawing features
  (`every_variation_twin_compiles_as_its_builtin`; with its z rule,
  `INFO.z`, `INFO.replace`, `CanHide`, `WritesRgb`, `NeedsW`, `INFO.slow`, and `INFO.emits` as its `PlotEmits`). Seven written by hand, 633 translated from the built-ins'
  WGSL (the def's `wgsl_init` as `prepare`, its slots named as the body
  reads them; its colour, direct colour and hide pointers locals from
  `ctx.color`, `ctx.rgb` and `ctx.hide`, returned as `#{ z, color, rgb,
  hide }`; its transform's coefficients (`transforms[xform_id].e`, or
  through a `let xf = transforms[xform_id]`) as `ctx.e`; a 3D body's
  `point_w` as `ctx.w` and its `point_w_out` a local returned as `w`; a
  `vec4` the language's four-part kind (part by part, as WGSL's), a
  part of one assigned; a `switch` whose every arm returns the body's
  end; an integer vector (`vec2<u32>`) refused, as the language's
  vectors are floats; an array of numbers or of complex numbers the
  language's (`array(n, zero)`, a `[...]` literal); the shared
  helpers it calls (`shaders/core/`: complex.wgsl's `cmul`, `cdiv`,
  `csqrt` ..., noise.wgsl's, voronoi.wgsl's, polyhedra.wgsl's,
  polychora.wgsl's) translated from their own WGSL as its helpers, so
  their guards are the built-in's; its helpers wherever they stand, and
  the init's (`prepare` calls them, the language inlining helpers); a
  helper reading the parameters (taking the ids) takes `p` in their
  place, passed whole; `loop` as `while true`; a parameter whose
  name is no Rhai name (`3side`) refused; `rng_next(rng)` as
  `rand_u32()`; the flame palette's read (`textureSampleLevel(palette_texture,
  ...)`) as `palette_srgb(t)`; structs as helpers (the user,
  2026-11-06: no struct types), one whose fields share a type an array
  of them, its fields by place (su_mobius.wgsl's `SuMat`, complex.wgsl's
  `CMat2`, fractwf.wgsl's result); an integer vector an array of its
  integers (`.x`, `.y` by place: the range helpers' pairs); an array of
  `vec3` or `vec4` an array of their floats, flat; a module constant
  written out where it is read, a constant table a literal at the top
  of each function that reads it (a vector's floats flat), which the
  language writes as a module constant (polychora.wgsl's faces,
  su_mobius.wgsl's base matrices) -- in `prepare` a local, filled once
  a call; a table copied whole into a variable never written (`var
  verts = POLYCHORA_VERTS;`, a built-in init's one copy) read as the
  table; a
  helper's pointer argument its `this` (`x.f(...)`, called on what the
  pointer points at; of several, a pointer to an array it never writes
  through the array passed by value), a helper reading
  the context taking `ctx`, one reading or setting the variation's
  state called on it (`this.f(..)`, klein_group); a 3x3 matrix its nine
  floats, column by column, `m * v`, `m[c][r]` and `transpose(m)`
  written out (glsl_apollonian, stereogram); the flame's camera in the
  shader's `params` (`params.camera_rotation_x` ..) the context's
  (`ctx.camera_pitch` ..), utilities.wgsl's `build_camera_matrix` and
  `camera_transform` carried in (stereogram); the emission collector's calls (`emit_plot`,
  `emit_plot_offset`, `emit_suppress_main`) the language's emissions
  (multi_kaleidoscope, spray_blur); a `u32` loop counter a `u32` again
  in the body, as WGSL's is; a
  helper giving an array with its early returns folded into an `if`
  value; `bitcast` of an integer the language's conversion (a float's
  refused); `countOneBits` a helper of the twin's own, Kernighan's;
  a variation reading its slots by a computed index (`get_param` of a
  slot it computes, or its init's `out[e]`) has a `prepare` returning one
  array, `slots`: its parameters, then the built-in's derived values,
  so `p.slots[e]` is the built-in's slot `e` (the lsystems, the
  matrices fuchsian_triangle, schottky_group and su_custom compute);
  one setting its state by a computed slot keeps it as one array,
  `this.st[k]` (farblur) -- Rhai's own terms, no syntax of ours (the
  user asked for that, 2026-11-06); a 2D `normalize` as `v / v.abs()`; a function WGSL takes part by part of a vector written out
  part by part where the language has no such vector form (`sin` of a
  complex is the complex sine); its temporaries never a name the WGSL
  uses; its state slots fields
  named as the body reads them, `get_state` / `set_state` as `this.name`
  and its `wgsl_state_init` as `fn init(p)`; its `u32`s the language's
  (`u32(N)` for a `Nu`, hex as Rhai's, `~x` as `x ^ 0xffffffff`); a
  float's bits (`bitcast`) refused, as the language has none; a `for` that is not a
  counted range a `while`, its step last, when nothing `continue`s;
  `smoothstep` written out; a vector and a number added part by part,
  where Rhai's complex + float adds to the real part)
  (`scripts/translate_variation_twins.py`, into the skeletons
  `twins::skeleton` writes; `scripts/list_variation_twins.py` writes
  the table).
  - **Held per point** by the probe's pairs (`probe::pairs`, the
    built-in and its twin as two transforms, the RNG seeded from the
    point so the two draw alike, the colour written out): at the probe's
    adversarial inputs and a grid, 2D and 3D, at the defaults and each
    parameter's range's ends (`every_variation_twin_runs_as_its_builtin`).
    93.3% of the outputs are bit for bit. The rest are judged by the
    twin's own formula in f64 (the interpreter): the built-in off it
    where the twin is on it (its hardware trig, WGSL's `pow(0, 0)`), or
    giving no number where it has one (WGSL's `pow` of a negative base,
    which JWildfire's `Math.pow` takes: `ngon` at sides -10); the
    truth ill-conditioned (it moves by as much when the point or a
    parameter moves by a few ulps: a derived sine at an exact turn); or
    both within f32's reach of it. A built-in off its formula at more
    than 16 of a pair's 432 parts doubts the twin instead (`OFF_CAP`;
    at most 9, measured): judged by itself, a twin of another formula
    would be on it everywhere, as the control -- julia beside exp's
    twin, judged by exp's formula -- shows. The language's trig is
    range-reduced where the built-ins' is the hardware's: the twin is
    the portable side.
  - **In a flame, by density** (`every_variation_twin_draws_its_builtins_flame`),
    and by colour for a twin that writes one, a palette index or a
    direct colour -- its transform blending it in whole -- each twin's
    flame drawn in 2D and in 3D, its two bodies (a 4D twin's in 3D alone,
    where its 4th coordinate feeds back through every iteration;
    `VARIATION_TWINS_DIMS=2` or `3` draws one; `VARIATION_TWINS_PARAMS=k=v`
    sets the variation's parameters in both flames, a mode the defaults
    do not reach, as the timing tool takes it too), each channel's light
    (colour times density) held as density is; a twin reading the
    camera drawn in 3D with it turned, moved and in perspective (pitch
    and yaw swapped in the generated WGSL is the control: 75 of its 101
    blocks off); dc_linear's colour blended in and not is the control --
    which judges the twins that draw (no f64 truth matches the GPU's
    stream). rays1's flame sends points past 2^24, where an f32's `tan`
    is no formula's on any side (`docs/accepted-divergences.txt`): reduced
    below 1e5 it is the built-in's exactly, measured, and it is held to
    drawing (`HUGE_TRIG`). mobius_strip takes a coordinate's remainder,
    rounding on every side past 2^24, so its pairs are not held at points
    that far (`FAR_REMAINDER`). r_circleblur's cells are placed by a `sin(...) * 43758.5453`
    hash, which the range-reduced sin hashes otherwise: its twin draws
    its own noise, as cut_jigsaw's (its pieces flip by a `sin(...) *
    1e5` hash), cut_fingerprint's, cut_hextruchetflow's and
    cut_metaballs's (`fract(sin(x) * 43...)`) do -- with the hardware's
    sin each flame is the built-in's exactly, measured.
  - **Timed** (`time_variation_twins`, a tool, ignored): each twin's
    flame against its built-in's on one renderer, the two loaded in
    turn. Two renderers of the same flame ran up to 2.3x apart, by
    where their buffers landed (a cheap variation's dispatch is bound by
    the histogram's atomics), so an A/A (`VARIATION_TWINS_SELF=1`, the
    built-in against itself) checks the measure first: 0.997 to 1.002.
    Of the 597 (2026-11-06, GTX 1660 SUPER): median ratio 1.002,
    quartiles 1.000 to 1.008, 24 more than 5% slower, 3 more than 25%
    -- glsl_circlesblue 1.61, glsl_acrilic 1.48, electron_orbital 1.32,
    each calling trig inside a loop, where the language's is
    range-reduced (the likely cause, not yet measured as it). Measured,
    not held: a twin is what an edit starts from, and the built-in still
    draws an unedited flame. Its dispatches are small (64 workgroups x
    256 iterations) and one is probed first, a twin over 250 ms a
    dispatch skipped: a slower variant of a 370 ms dispatch once reset
    the driver, and with it every program on the GPU. `TWIN_WGSL=dir`
    runs a twin's WGSL from `dir/<name>-2d.wgsl` / `-3d.wgsl` in place of
    its compiled code (`TWIN_WGSL_WRITE=1` writes it there first), to time
    a change of the compiler's before making it. Found so: a whole-array
    copy written as a loop over its elements kept small arrays out of
    registers (von_dyck 1.68, su_mobius 3.06, dc_hexes_wf 1.59), now
    WGSL's own `dst = src` (1.01, 1.08, 1.09); and polychoron's 5.65 was
    its table, which naga copies to a local at every read of it indexed
    at run time (`spill_to_internal_variable`), the built-in paying the
    copy once a read too. Both now keep their vertices in derived slots
    (docs/archive/projects/variation-param-storage.md): the built-in 2-10x
    faster, and the twin, translated from it, 1.00-1.04 of it -- menger's
    the same, in its 4-gap modes.
  - **Renamed parameters** (the user, 2026-11-06: no Rhai or WGSL
    reserved word as a name): w's and z's `super` is `super_shape`,
    hexnix3D's `3side` `triangle_scale` (`variations::RENAMED_PARAMS`).
    A `.flame` keeps JWildfire's names both ways (`w_super`), and a
    flame or an animation track saved under an old name reads under the
    new (`Transform`'s loading, `set_variation_param`).
  - **Edit as Rhai**: a built-in variation with a twin has it in its
    row's settings menu, putting the twin under a UUID of its own in the
    built-in's place in that transform (weight, parameters, order,
    phase), the flame carrying its source, and opening it in the Formula
    Editor in use (`formula_editor::variation_as_twin`).
  - **Without a twin** (7; `translate_variation_twins.py --report`
    names each one's reason): the analytic blurs (`analytic_blur`,
    `analytic_gaussian_blur`) and the subflames (`subflame_wf`,
    `pre_subflame_wf`), renderer hooks that would not translate well
    (the user, 2026-11-07); the volume emitters (`polyhedron_volume`,
    `sphere_volume`), possible later work; mondrianomies, its Exact fill
    a table baked in per flame (*Per-flame specialisation*).
  - `to_float(floor(x))` (`trunc`, `ceil`) is the float operation in
    every kind and in the interpreter, exact at any size.
  - **A literal first in `min` and `max`**: Rhai's `max(a, b)` keeps a
    NaN `a` (a test and an assignment, the same in the interpreter),
    where WGSL's returns the other operand, so a built-in's
    `sqrt(max(x, 0.0))` is a guard; the translator writes
    `max(0.0, x)`, WGSL's exactly, a literal never being NaN (`arcsinh`
    at 1e30, where `x` is inf - inf).
  - **Nothing of the def is lost** (the user, 2026-11-05): each twin
    opens with its def's doc block verbatim -- its description and
    authors, read as text by the API (ESCAPE.md, *The doc block*;
    `every_variation_twin_keeps_its_builtins_doc`) -- and its `INFO`
    gives the def's `category` (where the pickers group it) and its
    `phase`: `any` for 327 of them, a phase the user may move to pre or
    post, which the wire cannot carry, so a variation of the user's takes
    it from its source at registration (`custom::phase_of`,
    `VariationRegistry::set_phase`).
  - **An unedited twin is its built-in** where only a def is known:
    one that computes what the twin does, noted per key at registration
    (`twins::note`, `note_maths`, `builtin_of_key`). The twin's text
    (with or without the id Edit as Rhai gives it, line ends aside;
    `twins::builtin_of`), or its maths (`twins::builtin_computed_by`:
    its compiled shaders with its own names blanked, its phase,
    features, state, derived slots, emissions and parameters' names and
    kinds, against the twin its `INFO.name` or its functions name): a
    changed title, `INFO.name`, description, comment, or a parameter's
    caption, default or range keeps it (the user, 2026-11-08: a twin
    retitled "Bubble2" lost the Flame Attractor and Focused Rendering).
    Compared after the registry's lock, as compiling may read it. The deep-zoom planner's tables --
    inverses, bounds, interval bodies -- find the built-in's under its
    key (`twins::planner_name`, in `inverse::for_name`,
    `bound::for_name` and `derive`), except a many-valued one's: the
    replay forces an arm in the built-in's WGSL text
    (`bound::force_arms`), so a twin of `julian` or `juliascope` would
    plan as unknown. A `.flame` export writes it under its built-in's
    name, its weight, parameters, order and `fx_priority` with it
    (`flame_xml::twins_as_builtins`). Its maths changed and it is a
    variation of the user's, with neither
    (`a_twin_is_unedited_by_its_maths_not_its_text`,
    `an_unedited_twin_keeps_its_builtins_planner_data`,
    `an_unedited_twins_transform_is_analysed_as_its_builtins`,
    `an_unedited_twin_exports_as_its_builtin`). Still a def's alone: its
    aliases (a `.flame`'s foreign names).
  - **Named by its label** where the user reads it
    (`VariationRegistry::label`: a Rhai variation's title, never its
    key; one whose carried source did not compile by that source's
    title, `custom::title_of`): the Transforms panel and its collapsed
    summary, Undo History, animation targets and the track editor, the
    Reset Orbits tooltip, the Variations panel, the Random Generator (one
    version of each), the gallery, the browser (a key it does not know
    as "a Rhai variation"), the Formula Editor's status, and the Flame
    Attractor's and Focused Rendering's refusals
    (`Disqualification::describe`, `NoBall::describe`: transforms from
    1, "has no inverse", and for a Rhai variation "bounds are worked out
    for built-in variations only, so far"; their `Display` keeps the
    key, for logs). A carried source that did not compile is reported by
    its title and its error, not fetched from the library
    (`custom::error_of`).
`VariationPhase::Any` has no wire form yet, so a downloaded variation
cannot honour `fx_priority`. The catalog, caching and curation are in
[ONLINE.md](ONLINE.md); the design record is
[api-shared-resources.md](../archive/projects/api-shared-resources.md)
and [api-managed-variations.md](../archive/api-managed-variations.md).

---

## The docs corpus

A variation's description and authors exist only as the `///` doc
comment directly above its `pub static` — no blank line between — with
an optional `# Authors` section:

```rust
/// What it does, in markdown.
///
/// # Authors
/// - Someone
pub static MYVAR: VariationDef = VariationDef {
    name: "myvar",
```

[docs.rs](../../src/variations/docs.rs) parses that back out
(`parse_source`), keyed by the `name:` field. `cargo run --release --bin
export_variations_json` writes the full corpus the API imports — the
registry's structural data plus that prose — to
`output/variations-corpus.json`, and **refuses** to write it while any
built-in lacks a description. Prose reaches the app only through the
API catalog. Name only authors the source names; an unknown author
stays unknown.

---

## Testing

- **Unit tests never write the global registry** (`register`,
  `register_from_api`, `custom::ensure` on a flame that carries a
  source). Tests run in parallel, and some read it under a guard they
  re-take inside (`every_variation_validates_in_the_layer_warp`): a
  writer queued behind the first read blocks the second, and the suite
  hangs. Use a local `VariationRegistry`, or the `custom` module's own
  map; GPU gates (`#[ignore]`, run alone) may register.
- **Shader lints** (`shader_lint` in [mod.rs](../../src/variations/mod.rs))
  scan every registered variation's four WGSL strings and every file
  under `shaders/`.
- **The math probe** (`cargo run --release --bin variation_probe`)
  evaluates every variation at a fixed adversarial input grid, in 2D
  and 3D, for cross-platform comparison.
- **The reachability census** counts which input classes real renders
  feed each variation.
- **The contract** — `docs/generated/engine-contract.json`, checked by
  `contract_is_current` — publishes per-category, per-phase and
  per-feature counts, so it changes whenever a variation is added.

All four are described in [SHADERS.md](SHADERS.md) and
[TESTING-GUIDE.md](../TESTING-GUIDE.md).

---

## Adding a variation

1. **Port from the source.** Read the JWildfire/Apophysis source, keep
   its quirks, and cite the source file in the definition's doc
   comment. Check the variation is not already registered first: grep
   `src/variations/defs/` for `name: "<name>"`, and the aliases.
2. **Write the definition** in a new or existing
   `src/variations/defs/<file>.rs` — see `defs/octapol.rs` or
   `defs/szubieta.rs`. Give it a `///` description and, if the source
   names one, an `# Authors` section.
3. **Choose phase and features.** Most ports are `Any`. Add `Replace`
   if the source assigns `pVarTP` rather than adding to it, and
   `AlwaysZ` if it writes `pVarTP.z` unconditionally (or run
   `scripts/audit_z_write_semantics.py`).
4. **Write both bodies** to the signature rules above, helpers
   name-prefixed, epsilons normal in f32.
5. **Register it** in `src/variations/defs/mod.rs`: `mod <file>;`,
   `pub use <file>::*;`, and `&MYVAR` appended to the **end** of
   `ALL_VARIATIONS`.
6. **Unit conversions.** If the foreign app stores a parameter in other
   units, add the pair to `variation_param_from_xml` /
   `variation_param_to_xml` in [flame_xml.rs](../../src/flame_xml.rs).
   If a parameter bounds a loop the variation runs every iteration, time
   it (`PARAM_COST_ONLY=<name> cargo test --release --lib -- --ignored
   --nocapture measure_variation_param_costs`). If its maximum adds 15 ms
   or more to an app frame, write it `param!(slow ...)`, so its
   control commits on release (UI.md, *Slow settings*).
7. **Regenerate the contract**
   (`UPDATE_CONTRACT=1 cargo test --lib contract_is_current`) and read
   the diff; run the shader lints; render a flame that uses it in 2D
   and 3D, and compare against JWildfire.

The variation appears in the UI under its category with no further
wiring.

## Porting and trackers

- [variation-port-blockers.md](../projects/variation-port-blockers.md) —
  what is not ported, and what feature would unblock it
- [jwf-common-variations-port.md](../projects/jwf-common-variations-port.md)
  — JWildfire's common set
- [VARIATIONS_TODOS.md](../projects/VARIATIONS_TODOS.md) — running notes
- [jwf-features.md](../projects/jwf-features.md) — non-variation
  JWildfire features, including `fx_priority` and variation order
- [VARIATIONS_WIRE_FORMAT.md](../projects/VARIATIONS_WIRE_FORMAT.md) —
  the wire contract
- [accepted-divergences.txt](../accepted-divergences.txt) — known,
  accepted differences from the references

## Design record

- [VARIATION-SYSTEM-V2.md](../archive/VARIATION-SYSTEM-V2.md) — named variations and per-flame indices
- [global-registries.md](../archive/global-registries.md)
- [packed-variation-params.md](../archive/packed-variation-params.md)
- [variation-init-dispatch.md](../archive/variation-init-dispatch.md)
- [intra-iteration-state-and-accum.md](../archive/intra-iteration-state-and-accum.md)
- [needs-transform-and-direct-color.md](../archive/needs-transform-and-direct-color.md)
- [complex-math-and-klein-group.md](../archive/complex-math-and-klein-group.md)
- [preserve-z-semantics.md](../archive/projects/preserve-z-semantics.md)
- [multi-emit-stereograms.md](../archive/projects/multi-emit-stereograms.md)
- [analytic-blur-buffer.md](../archive/projects/analytic-blur-buffer.md)
- [variation-bulk-port.md](../archive/projects/variation-bulk-port.md)
- [VARIATIONS_BULK_METADATA_IMPORT.md](../archive/projects/VARIATIONS_BULK_METADATA_IMPORT.md)
- [variation-math-probe.md](../archive/projects/variation-math-probe.md), [variation-reachability-census.md](../archive/projects/variation-reachability-census.md)
