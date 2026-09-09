# The Simulation panel: one list per concept

**Status:** plan of record, 2026-09-09, branch `simulation-mode`. **No
code written yet.** This is phase 5 of
[simulation-layers.md](simulation-layers.md), the only unbuilt phase
there, widened by what the panel has accumulated since that plan was
written.

Surveyed before planning; every claim carries its `file:line`.

## 0. What is being asked for

> There's the Model section and the Layers section just below. It's
> kinda confusing, because Model doesn't do anything when there's
> multiple layers, and Layers don't do anything when there's only one.

Four things:

1. **Merge Model and Layers.** Layer 0 is the primary model; extra
   layers are optional; Couplings becomes active above one layer.
2. **Merge Colouring and Colouring layers** the same way.
3. **Five sections** — controls, Model, Settings, Warp, Colouring —
   with the controls pinned so they are always visible.
4. **Spacebar runs and pauses the simulation** in Simulation mode,
   rather than the animation.

## 1. What exists today

### 1.1 The panel, section by section

`render_sim_content` (`src/ui/sim_panel.rs:82-783`) plus two delegates.
The five target sections are already contiguous, which is why this
reorganisation is a reshuffle rather than a rewrite.

| Lines | Today | Goes to |
|---|---|---|
| 95-113 | Enter-simulation button, early return | stays at top |
| 117-153 | Run / Step / Reset, step and grid readout | **Controls** (pinned) |
| 157-176 | Model combo | **Model** |
| 178-276 | Model presets | **Model** |
| 278-287 | Model parameters | **Model** |
| 1044-1266 | `render_layers`: layers, `use_transforms`, layered presets, Couplings | **Model** |
| 297-340 | Grid | **Settings** |
| 342-381 | Upscale / fit / downscale | **Settings** |
| 385-398 | Seed | **Settings** |
| 400-461 | Init | **Settings** |
| 463-478 | Boundary | **Settings** |
| 482-551 | Max steps, steps per frame, dt | **Settings** |
| 555-676 | Warp (already a collapsing header) | **Warp** |
| 680-708 | Colouring combo and parameters | **Colouring** |
| 714-779 | Matte | **Colouring** |
| 790-1038 | `render_color_layers` | **Colouring** |

### 1.2 The model side already treats layer 0 as the model

This is the finding that decides the whole design.
`SimConfig::layer_model_name` (`src/config/sim.rs:1106-1133`) falls back
to the flat `model` when `layers` is empty, and `layer_count()` clamps
to at least 1. So **a one-entry `layers` list and an empty one are
indistinguishable in arity**; only the source of layer 0 differs.

Consequences, all verified:

- There is **no `layers.is_empty()` fast path** anywhere in `src/sim/`.
  Every consumer goes through `layer_count()`.
- The pipeline key (`src/sim/renderer.rs:1139-1155`) holds a vector of
  model names of length `layer_count()`, and keys `coupled` off
  `couplings`, not `layers`. A one-layer uncoupled config therefore
  compiles the **byte-identical** step shader
  (`src/sim/assembler.rs:1501`, gated at `assembler.rs:2232-2252`).
- Layer 0's RNG salt is zero (`assembler.rs:163-165`), so seeding is
  unchanged.

### 1.3 The colouring side does not

There are no `color_layer_*` accessors. The single-versus-stack choice
is made inline at three sites, each on `color_layers.is_empty()`:
`src/sim/renderer.rs:802` (`wants_sdf`), `renderer.rs:1206-1216` (**which
shader is assembled**), and `renderer.rs:1828-1856` (parameter packing).
The two paths are genuinely different WGSL — `SINGLE_SHADE`
(`assembler.rs:1946-1957`) against the stack's `sim_blend` preamble
(`assembler.rs:2032-2075`).

They are proven equivalent for a one-layer Normal stack, pixel for
pixel, by `a_single_normal_colour_layer_is_the_single_colouring`
(`src/sim/app_repro_test.rs:6393-6476`) over three fixtures. Three
fixtures is not 92, which is why §2 decides the way it does.

### 1.4 Half the merge is already written

- **Colour promotion**: `sim_panel.rs:806-825` — the empty-stack "Add
  colouring layer" button already lifts `coloring` / `coloring_params`
  / `matte` into `color_layers[0]`.
- **Colour demotion**: `sim_panel.rs:1023-1036` — "Flatten to one
  colouring" writes layer 0 back to the flat fields.
- **Layer demotion**: `sim_panel.rs:1147-1153` — removing down to one
  layer collapses back to `model` / `model_params` and clears
  couplings.
- **Couplings already hide below two layers**: `sim_panel.rs:1182-1184`.
  Item 2 of the ask is current behaviour.

What is missing is the *promotion* on the model side done as one layer
rather than two, and the presentation that makes the seam invisible.

### 1.5 The two mechanics

**Spacebar** is handled at exactly one site,
`src/app/input.rs:214-236`, driving the animation controller. The app
only sees a key when egui has not consumed it
(`src/app/mod.rs:1043-1046`), and egui consumes keyboard input whenever
**any widget has focus** — so a space typed into a numeric field never
reaches the shortcut, and no new guard is needed. `sim_running` is a
`pub(super)` field on `App` (`src/app/mod.rs:422-431`) and `input.rs` is
`impl App`, so the handler can write it directly. Escape mode already
sets the precedent for a mode-specific early-return branch
(`input.rs:96-126`).

**The pinned header** is not free, because egui_dock wraps every tab
body in its own `ScrollArea` (`egui_dock leaf.rs:1248`). Drawing the
transport row before a scroll area would still let the outer scroller
drag it away, and would put two vertical scrollbars in competition —
the failure `src/ui/scripts_panel.rs:333-337` documents. The repo
already has the idiom for switching the outer scroll off: `scroll_bars`
returns `[true, false]` for compact mode (`src/ui/panel_viewer.rs:432-443`).

## 2. Decisions

Taken 2026-09-09.

1. **One panel with pinned controls**, not five dockable panels. The
   split stays possible later; this is the contained change.
2. **The merge is presentation only.** The panel always shows a layer
   list, layer 0 reading the flat fields until a second layer is added.
   Nothing on disk changes, no migration, no shader path changes.
3. **Model presets apply to the layer being edited.**
4. **Four adjacent problems come along** (§5).

**Why not merge the config.** Always storing a `layers` list looks
tidier and costs more than it returns:

- **Every model preset button would die.** They push `SimModelParam`
  (`sim_panel.rs:181-273`), which writes the flat map
  (`src/config/manager.rs:3054-3072`) that `layer_model_params(0)` would
  stop reading.
- **Saved animation tracks would go dead.** Tracks are addressed by
  string key — `Sim.ModelParam.feed` (`src/config/delta.rs:3017`) — and
  the target picker offers no layer-scoped alternative
  (`src/ui/target_selector.rs:375-403`).
- **The script API writes the flat fields** (`src/script/api.rs:1475-1524`).
- **Every config would move onto the colour-stack shader**
  (`renderer.rs:1206`), where equivalence is proven for three fixtures
  rather than all 92.

And the flat fields could not actually be deleted, because of the
second and third points — they would survive as aliases. **The config
merge relocates the duplication into the path dispatch rather than
removing it.** The duplication the user is complaining about is in the
*panel*, and that is where it will be fixed.

## 3. The design

### 3.1 One list per concept

Two helpers, each taking `Option<usize>`: `None` means "edit the flat
fields", `Some(i)` means "edit layer i". The two bodies already exist
and are near-identical — `sim_panel.rs:159-176` against `1101-1119` for
the model combo, `278-287` against `1157-1167` for the parameters — so
this is one function with two path constructors, not new logic.

```
Model
  Layer 0   [Gray–Scott ▾]              ← flat fields, or layers[0]
    presets: Coral  Maze  Mitosis …     ← write the layer being edited
    feed ──●───  kill ──●───
  Layer 1   [Brusselator ▾]  [x] on  [remove]
    …
  [+ Add layer]
  Couplings                             ← only above one layer
    1 → 0  linear  0.02  channels xy
```

Colouring takes the same shape, with layer 0 reading `coloring` /
`coloring_params` / `matte`.

**Promotion and demotion stay silent and symmetric.** Adding a second
layer promotes the flat fields into `layers[0]`; removing back to one
demotes them. Both already exist (§1.4) and both are one undo step
through the snapshot helper (`sim_panel.rs:1051-1055`).

### 3.2 The pinned header

Three edits, in the order they must happen:

1. `src/ui/panel_viewer.rs:432-443` — `scroll_bars` returns
   `[true, false]` for `PanelType::Simulation`, above the compact arm so
   it wins in both layouts.
2. `panel_viewer.rs:449-459` — exclude Simulation from the compact
   scroll wrapper, as `FractalViewport` already is.
3. `sim_panel.rs` — transport row stays where it is; everything from
   the separator at `:155` moves inside one
   `ScrollArea::vertical().id_salt("sim_panel_body")`.

The `id_salt` matters: the panel's `Ui` id derives from the tab, so a
second scroll area added later would collide without it.

**Sizing is not a problem.** `available_height` inside an egui 0.34
scroll area is the visible viewport, not infinity, so the body can size
itself from what the header left.

### 3.3 Spacebar

An early-return branch above the shared match in
`src/app/input.rs`, mirroring the escape-mode one: in Simulation mode,
space toggles `self.sim_running` and returns.

**It must write the field directly, not the UI response.**
`UiResponse::sim_running` is an `Option<bool>` the panel sets
unconditionally from its own local each frame (`src/ui/mod.rs:2407`), so
a keyboard write into it would be overwritten by the panel's stale
value in the same frame. Writing `App::sim_running` is correct; the
panel picks the new value up next frame.

## 4. Phases and gates

Each phase leaves the app working and every test green.

| # | Builds | Gate |
|---|---|---|
| 1 | Spacebar in Simulation mode; help text and Animation button labels made mode-honest; the missing shortcut key added to the coverage test | a table test over all four modes for what space does; `every_rendered_shortcut_resolves` covers every key `help.rs` renders |
| 2 | Pinned transport header: the two `panel_viewer` arms and the body scroll area | the panel scrolls with the header fixed, in both normal and compact layouts; no nested vertical scrollbars |
| 3 | Model and Layers merged into one list; presets target the edited layer; "Add layer" promotes rather than doubling | a one-layer config is byte-identical through the renderer; promotion then demotion round-trips the config exactly; every sim baseline unchanged |
| 4 | Colouring and Colour layers merged the same way | same round-trip gate on the colour side; the existing one-layer-stack equivalence test still passes; baselines unchanged |
| 5 | Section reshuffle into Controls / Model / Settings / Warp / Colouring; `use_transforms` rehomed | every control that existed before still exists and still writes the same `ConfigPath` — a test over the panel's path set |

Phases 1 and 2 are independent of 3–5 and can land first.

The gate that matters most is in 3 and 4: **promotion followed by
demotion must return the config to exactly what it was.** That is what
makes an invisible seam safe.

## 5. Adjacent problems, in scope

Found while surveying; all four chosen for inclusion.

- **Layer parameters cannot be animated.** `SimLayerParam` and
  `SimColorLayerParam` exist as config paths (`src/config/delta.rs:403,
  427`) but the target picker offers only the flat ones
  (`src/ui/target_selector.rs:375-403`), so nothing inside a layer can
  be keyframed. The merge makes layers a first-class concept, which
  makes this gap conspicuous.
- **The dt ceiling reads the wrong model.** `src/config/manager.rs:2952`
  and `src/ui/sim_panel.rs:539` compute the cap from the flat `model`
  even in a layered config, where the renderer correctly takes the
  minimum over all layers (`src/sim/renderer.rs:1570`). The slider can
  therefore offer a dt that one layer will not survive.
- **"Add layer" adds two.** `sim_panel.rs:1090-1093` splits a single
  system into two layers of the same model rather than promoting the
  existing one and adding one more. Invisible today, obviously wrong
  once layer 0 is on screen as itself.
- **The spacebar's documentation.** `locales/en.yml:163` says space
  plays the animation; the Animation panel's buttons are labelled
  "(Space)" (`en.yml:1282,1284`). Both become half-true. And
  `play_pause_animation` is rendered at `src/ui/help.rs:113` but missing
  from the list in `every_rendered_shortcut_resolves`
  (`help.rs:154-160`), so the one shortcut this work changes is the one
  the test does not cover.

## 6. Risks

- **The invisible seam.** Adding a second layer silently rewrites where
  layer 0's data lives. It is already what the code does, it is one
  undo step, and the round-trip gate in phase 3 is what keeps it
  honest — but it is the thing most likely to surprise.
- **Presets carry a whole recipe.** A model preset sets dt, steps,
  colouring and matte as well as parameters, and those are shared
  across layers. Applying one to layer 1 restyles the whole simulation.
  Decision 3 accepts this; the alternative considered was to apply only
  the model parameters, which would make presets do less than they do
  today with one layer.
- **Two scroll areas.** Phase 2 removes the outer one for this tab. If
  a future panel adds its own without the same treatment, the double
  scrollbar returns. `panel_viewer.rs:1052-1062` (Solid & Lighting)
  already has exactly that bug and would be fixed by the same arm.
- **Compact mode has less height to give.** Pinning two rows costs
  proportionally more on a phone. `undo_history.rs:61` already
  special-cases compact with a smaller floor for the same reason.
