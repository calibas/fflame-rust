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
- [BUFFERS.md](BUFFERS.md) — `GpuVariationParams` and the state buffers
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
  `VariationDef`, `VariationParamDef`, `Feature`, the `param!` macro
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
- [src/storage/plugins.rs](../../src/storage/plugins.rs) — locally
  installed variations
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
| `provenance` | `Builtin`; a download is `Api { version }`, a local plugin `Local` |
| `wgsl_source`, `wgsl_source_3d`, `wgsl_source_init`, `wgsl_source_state_init` | the bodies (optional, because a download may lack one) |
| `init_param_count`, `state_count` | copied |

**Registration.** `VariationRegistry::new()` registers every entry of
`defs::ALL_VARIATIONS` in order. The list is **append-only**:
registration order is the stable order the registry reports
(`names()`), the fallback dispatch order and the UI order within a
category, and legacy configs that stored weights as an array are read
by it. With the `engine-flame` cargo feature off, `ALL_VARIATIONS` is
`[&LINEAR]` alone — a default config still names `linear` — and the
linker drops the other definitions.

**Global access.** The registry is a process-wide
`RwLock<VariationRegistry>`. `global_registry()` returns a
`RwLockReadGuard`; `global_registry_mut()` a write guard, used to
register downloads and plugins at run time and to clear the download
cache. Every runtime add or remove bumps `version()`, which the shader
cache watches. Drop a read guard before anything that may take the write
lock; `local_plugin_dependencies` drops its guard before moving on to
the effect registry for that reason.

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
are compiled in, and local plugins are the user's files.
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
`spray_blur`. See
[multi-emit-stereograms.md](../archive/projects/multi-emit-stereograms.md).

---

## Parameters

### Types

`VariationParamDef` has `name`, `display_name`, `param_type`,
`default_value`, `min_value`, `max_value` and an optional
`description` (the tooltip). `param!` writes one:

| `param!` form | `ParamType` | stored as |
| --- | --- | --- |
| `float, default, min, max` | `Float` | the value; the slider is clamped to min–max |
| `unlimited_float, default, min, max` | `UnlimitedFloat` | min–max is the slider range; any value can be typed |
| `int, default, min, max` | `Integer` | an integer in an `f32` |
| `unlimited_int, default, min, max` | `UnlimitedInteger` | as `Integer`, with typing past the slider |
| `angle, default` | `Angle` | degrees, slider −360…360; the body converts |
| `bool, default` | `Boolean` | 0.0 or 1.0 |
| `enum, default, &[choices]` | `Enum { choices }` | the choice's index |

Each form also takes a trailing description. The Transforms panel
builds every slider from these definitions.

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
  recompute from its parameters every iteration.

The layout is packed per flame: each active variation gets the same
offset in every transform's block, in local-index order, and the
generated `get_param(xform_id, variation_id, slot)` maps the local
index to that offset. A transform's block holds
`MAX_VARIATION_PARAM_SLOTS = 1600` floats; a flame that needs more logs
an error and loses parameters. See [BUFFERS.md](BUFFERS.md) and
[SHADERS.md](SHADERS.md).

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

**Per-flame specialisation.** A body can be re-emitted per flame:
`synth` and `mondrianomies` cut their unused modes before compilation
(`variation_specialized_source` in `generate_variation_code`). A new
specialiser is a new arm there.

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

See [forward-bounds.md](../projects/deep-zoom/forward-bounds.md) and
[ifs-general.md](../projects/deep-zoom/ifs-general.md).

---

## Downloaded variations and local plugins

The registry also takes variations at run time, in the API's
`VariationDownload` JSON, through `register_from_api(download,
provenance)`:
- **Downloaded** (`Provenance::Api`). When a flame names an unknown
  variation, the app fetches it from the API, caches it, and registers
  it.
- **Local plugins** (`Provenance::Local`). The same JSON, installed by
  the user under `plugins/variations/` in the app data directory
  (localStorage on the web).

`storage::load_installed_resources` loads the cache and then the
plugins at startup.

The registry's rules:
- A built-in can never be replaced.
- A download cannot displace a local plugin.
- A plugin whose name is already taken is refused, not shadowed.
- A download with no 2D body is refused unless its category is
  `only_3d`, and an `only_3d` one with no 3D body is refused.
- A variation with no 3D body is dropped from 3D flames, with a
  warning.

A flame that uses a local plugin renders only where the plugin is
installed (`local_plugin_dependencies` reports it).

### Variations written in Rhai

A **custom variation** (`Provenance::Custom`, `variations::custom`) is
a Rhai transformation from the Formula Editor -- `fn variation(z, p)`,
a point to a point ([ESCAPE.md](ESCAPE.md#the-formula-language)) --
compiled to the same `VariationDownload` a plug-in is: its parameters
(a complex one as two, `name_re` and `name_im`), category `plugin`,
phase normal, and WGSL from the formula language's emitter. The 2D
function takes the point, and the ids when it has parameters (as
`shader_builder_v2::variation_call` passes them); the 3D one runs the
2D body on `p.xy` and passes `p.z` through. Helpers are prefixed
`cv_<name>_`.
- **The flame carries the source** (`Flame::custom_variations`, skip if
  empty; `ConfigPath::FlameCustomVariations`, one undo step), so a saved
  flame renders anywhere. `custom::ensure` registers a flame's sources
  (and its subflames') before anything reads the registry for it: on
  load (`missing_variations_in`, so it is not fetched from the API),
  when a `FlameRenderer` is built, `load_config`, `update_flame`, and
  `render_with` (an escape lens, CLI, thumbnails, video).
- **Names**: a lowercase identifier from `INFO.name`; refused when a
  built-in, a download or a plug-in has it, and a download or plug-in
  cannot displace one. Using a source again replaces it, and the
  registry's version rebuilds the shader.
- **`.flame` export drops it** (JWildfire has no such variation): its
  weight, its parameters and its `plugins=` entry, with a warning.
- **As a lens**: the lens menu lists custom variations after the
  measured ones (`lens::lens_menu`).
- **Inside a formula**: `vary("name", z)` inlines one into an escape
  formula or colouring, the entry carrying its source
  ([ESCAPE.md](ESCAPE.md#the-formula-language)).
- **Hooks** (`escape::lang::wgsl::variation_shape`): `rand()` declares
  `NeedsRng`, `ctx.accum` `NeedsAccum`, a `#{ z, color }` result
  `WritesColor`; `init(p)` and `this.name` are per-thread state
  (`state_count`, the seeding call as `wgsl_state_init`); state or
  `ctx.weight` without parameters declares `NeedsTransform` for the
  ids. The function takes them in `variation_call`'s order. Gate:
  `the_hooked_twins_render_as_the_builtins` (`blur`, `macmillan`).
- **3D**: `fn variation3(v, p)` is the 3D body, its point a vector; alone
  it makes the variation `only_3d` (no 2D body, kept out of the lens
  menu). Either body takes the hooks either needs, and a 3D body
  declares `AlwaysZ`. Gate: `the_3d_twins_render_as_the_builtins`
  (`spherical3D`, `hemisphere`, `zcone`, in a pitched 3D flame).
- Gates: Rhai twins of linear, spherical, swirl, polar and horseshoe
  render as the built-ins in a flame, by density
  (`the_twins_render_as_the_builtins_in_a_flame`), and as lenses, pixel
  for pixel (`the_twins_are_lenses_as_the_builtins`).
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
   or more to an app frame, list it in `variations::SLOW_PARAMS`, so its
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
