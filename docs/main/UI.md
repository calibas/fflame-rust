# UI Architecture (egui_dock)

All UI is dockable panels built on egui + egui_dock. A panel can be
rearranged, detached, docked to any edge, or closed and reopened from
the Window menu. On small screens a compact layout replaces the menu
bar with a single menu button (see [Compact (mobile) UI](#compact-mobile-ui)).

**See also:**
- [ARCHITECTURE.md](../ARCHITECTURE.md) - system design and module organization
- [CAMERA.md](CAMERA.md) - the camera model behind the View panel and the input table below
- [CONFIG.md](CONFIG.md) - ConfigManager, undo/redo, `UpdateType`
- [I18N.md](I18N.md) - translations, the language picker, fonts
- [FRACTAL-BROWSER.md](FRACTAL-BROWSER.md) - the Fractal Browser panel
- [PALETTE_LIBRARY.md](PALETTE_LIBRARY.md) - the Palette Library panel
- [ESCAPE.md](ESCAPE.md), [SIMULATION.md](SIMULATION.md), [TERRAIN.md](TERRAIN.md) - the two non-flame engines and the 3D terrain view
- [ANIMATION.md](ANIMATION.md) - the Animation and Signals panels
- [EFFECTS.md](EFFECTS.md), [FOCUSED-RENDERING.md](FOCUSED-RENDERING.md), [ONLINE.md](ONLINE.md), [EXPORT.md](EXPORT.md), [SCRIPTING-GUIDE.md](SCRIPTING-GUIDE.md)
- Design records: [ui-render-modes.md](../archive/projects/ui-render-modes.md)
  (the render-mode survey, decisions, and the bugs it found but did not
  fix), [mobile-ui-redesign.md](../archive/projects/mobile-ui-redesign.md),
  [dynamic-docking-refactor.md](../archive/projects/dynamic-docking-refactor.md),
  [camera-unification.md](../archive/3d-camera/camera-unification.md)

---

## Panels

`PanelType` in [src/ui/workspace.rs](../../src/ui/workspace.rs) defines
**32 panels**. Each renders from its own `src/ui/*.rs` file;
`src/ui/mod.rs` coordinates docking and returns cross-panel results to
the app through `UiResponse` (see [Cross-panel results](#cross-panel-results-uiresponse)).

| Group | Panels |
|---|---|
| The picture | Fractal Viewport (effectively always present) |
| The camera, every mode's | View (see [The View panel](#the-view-panel)) |
| The flame | Transforms, Triangle Editor, Variations, Xaos Editor, Subflames, Paths |
| The other engines | Escape Fractal, Formula Editor, Hybrid Designer, Simulation, Textures |
| Appearance, shared by every engine | Colors, Palette Editor, Palette Library, Effects |
| Light, every 3D view's | Lighting & Material (see [The Lighting & Material panel](#the-lighting--material-panel)) |
| Workflow | Fractal Browser, History, Animation, Signals, Scripts, Random Generator, Export, Config Import/Export, Rendering, Performance, Help, Keyboard Shortcuts, Account, Save Online |

**Which of these is available depends on the render mode** — see
[Render modes and the UI](#render-modes-and-the-ui). Do not add a
`matches!(render_mode, ...)` to a panel; add a case to
`src/ui/visibility.rs`.

### Workspace layouts

`WorkspaceLayout` (same file) holds six presets:

| Layout | Left dock | Right dock (tabs) | Other |
|---|---|---|---|
| Standard | Transforms | Colors, View, Triangle Editor, Rendering, History | |
| Animation | Transforms | Colors, View, Triangle Editor, Rendering, History | Animation docked below the viewport |
| Scripting | Scripts | Fractal Browser | |
| Escape Time | Escape Fractal | Colors, View, Textures, History | |
| Simulation | Simulation | Colors, View, Textures, History | |
| Compact | — | — | the viewport alone |

- Window ▸ Workspace Layout offers Standard, Animation and Scripting.
  Escape Time and Simulation are not offered there: they arrive with
  their mode (`render_mode::layout_for`).
- Window ▸ Reset Workspace rebuilds the mode's own workspace (Standard
  for the flame modes, Escape Time, Simulation;
  `render_mode::workspace_for`), as does the compact menu's Desktop
  View. Window ▸ Mobile View applies Compact.
- The layouts other than Standard and Compact re-add the Help panel as a
  floating window if it was open.
- A newly built layout raises each side dock to at least 260 points
  (`MIN_SIDE_DOCK_POINTS`) on its first frame, once, so a laptop window
  does not open on a clipped Transforms panel.

Two ways to change layout:

- `Workspace::apply_layout` **forces** a rebuild from code. This is
  what Reset Workspace and the Workspace Layout menu do.
- `Workspace::switch_layout` **remembers**: it stashes the dock state
  it is leaving and restores the one it is entering, falling back to
  `apply_layout` the first time a layout is entered. Mode changes use
  this, so switching modes does not throw away an arrangement.

The stash is session-only. Nothing about the dock tree is persisted, so
a restart opens on the built-in layouts.

---

## Menu bar

[src/ui/menu_bar.rs](../../src/ui/menu_bar.rs) draws the desktop bar;
[src/ui/compact_menu.rs](../../src/ui/compact_menu.rs) draws the compact
equivalent. They share `MenuActions` / `MenuState`
([menu_context.rs](../../src/ui/menu_context.rs)): a menu sets a flag
or an `Option`, and `EguiLayer` ([src/ui/mod.rs](../../src/ui/mod.rs))
acts on it after the menus are drawn — a mode change directly, file
actions by folding them into the frame's `UiResponse`.

| Menu | Holds |
|---|---|
| File | New, Open, Save As, Export Flame XML, From Preset Library (the Fractal Browser), Random Flame, Random Batch (the Random Generator), Online features (the `online_mode` switch; [ONLINE.md](ONLINE.md)), Save Online (online mode only), Export PNG (the Export panel), Config Import/Export, Quit |
| Edit | Undo, Redo (Preferences is a disabled stub) |
| View | Reset View, Zoom In, Zoom Out — present in every mode (`Control::ViewNavigation`) |
| **Mode** | The four render modes as radio rows, built from `RenderMode::ALL`, each with a hover description |
| Rendering | Pause/Resume, Reset Accumulation, Iterations per Thread (128 to 4096), Frame-Time Governor, and the Workgroups a frame dispatches while the governor is off, Reset to Defaults. **Hidden outside the flame modes** — every item configures the chaos game |
| Window | Reset Workspace, Mobile View, the panel rows, Workspace Layout |
| Help | Help, Keyboard Shortcuts, Report Bug, About |

Right-aligned, when the window is at least 500 points wide, from the
right: the progress bar, the language picker (a globe menu; see
[I18N.md](I18N.md)), the connectivity and account status (online mode
only), and the Fly Mode button, which is greyed in a 2D view
(`MenuState::fly_available`, from `visibility::fly_mode`).

**The panel rows in both Window menus come from one table**,
`visibility::WINDOW_MENU`. Each row asks `visibility::panel` whether it
is available; an unavailable row is drawn disabled with its reason on
hover. The compact menu keeps its own order — touch priority, transforms
first — in `COMPACT_WINDOW_MENU`, but takes its labels from the shared
table, and a test asserts it is a subset. Add a panel row there, not in
either menu.

### The progress bar

One bar for every rendering task
([src/ui/render_progress.rs](../../src/ui/render_progress.rs)). The app
picks the one task it reports each frame (`App::render_progress` in
[src/app/render_progress.rs](../../src/app/render_progress.rs)), in this
order:

| Task | The bar |
|---|---|
| A PNG or video export | Fills to the export's fraction. Its headline and detail are the hover text. The export takes the bar over, as it pauses the viewport's render |
| Animation playback | Sweeps: it runs without end in every mode |
| A path tracer (an escape solid or terrain; a simulation terrain while the run is at rest) | Fills by samples toward the target; grey once reached |
| Escape | Sweeps while the render refines (unsettled, interaction preview, texture in flight); full and grey once settled |
| Simulation | Fills to `step / Max Steps` while short of the cap; sweeps when running past it (or with no cap); full and grey when paused at the cap |
| Flame (2D / 3D) | Fills to `total_iterations / max_iterations`; grey when finished or paused |

It costs nothing it can avoid:
- It reads counters the frame loop already keeps, with no readback.
- It never requests a frame. Every state in which it sweeps is one in
  which the app is already redrawing.
- Its text is built only while the pointer is on it.

Two event-loop rules exist for it:
- **An export redraws at 10 Hz**, not at the display rate, so the bar
  does not compete with the export thread for the GPU.
- **A stale bar gets one more frame.** The UI is built before the
  frame's render runs, so the bar can show a frame-old state. When the
  state differs from what was drawn (`progress_drawn`), the loop draws
  once more and then sleeps.

The compact menu shows the same bar as a strip under its button, only
while something runs.

---

## Render modes and the UI

There are four render modes — `RenderMode::{TwoD, ThreeD, Escape,
Simulation}` in [src/scene/transforms.rs](../../src/scene/transforms.rs)
— and they are peers, not a flame with two add-ons. `render_mode` lives
on `FractalConfig`, and the `flame`, `escape` and `sim` sub-configs all
exist at once, each preserved while inert, so switching modes and back
round-trips.

`RenderMode::ALL` is the enum in wire order, kept in step by an
exhaustive match and a length assertion. **Iterate it** rather than
listing four modes by hand.

### Changing the mode

[src/ui/render_mode.rs](../../src/ui/render_mode.rs) is the only place
that writes `ConfigPath::RenderMode`, and a test
(`nothing_outside_this_module_writes_the_render_mode`) scans the source
tree to keep it that way. Whole-config load paths are exempt by name:
they carry a mode in from a file rather than switching.

`switch_render_mode` exists because the switch is not just a field
write. Both non-flame engines produce a unit-range image, and a flame's
Log-calibrated exposure renders that black. So entering either from a
flame mode while the tone map is Logarithmic sets it to Linear at the
mode's own calibration (`config::defaults::entry_tone_mapping`: escape's
exposure and gamma, or the flame defaults for a simulation), batched
with the mode change into one undo entry. Switching between the two
non-flame modes does not reset, and **leaving does not restore**; the
function's doc comment records why.

The Mode menu (desktop and compact), the enter buttons on the Escape
Fractal and Simulation panels, and the View panel's 2D/3D switch all go
through `switch_render_mode`. A script sets the mode in the config it
returns, applying the same tone-map rule (`entry_tone_mapping`), and
that config loads as a whole.

Entering a lit 3D view (a solid formula, a terrain) from one that is not
sets **the 3D look** (`look_3d`: Linear, exposure 2, gamma 1, saturation
1, no hue shift, an untouched rig at strength 1 and ambient 0) in the
same undo entry. Besides the mode switch, the Escape panel's formula
picker and terrain switch, the Simulation panel's terrain switch and the
Formula Editor's Use go through `apply_with_look`.

Also in that module:

- `layout_for(mode)` — the workspace and editor panel a mode wants.
  `App::follow_loaded_render_mode` ([src/app/mod.rs](../../src/app/mod.rs))
  consults it: a non-flame mode switches to its layout, and a flame mode
  leaves the Escape Time or Simulation layout for Standard. In the
  compact layout it docks the engine's panel instead.
- `keeps_escape_engine` / `keeps_sim_engine` — which engine's GPU state
  a mode needs resident (below).
- `space_runs_the_simulation` — the spacebar drives the simulation's
  transport in Simulation mode and the animation everywhere else.
- `mode_label_key` / `mode_tip_key` — the `mode.*` locale keys.

The app compares the mode every frame, so a change from the Mode menu,
a panel, a script or an **undo** all bring the workspace with them.

### What each mode makes available

[src/ui/visibility.rs](../../src/ui/visibility.rs) is the single
answer, and it is exhaustive on purpose: there is no `_` arm, so a new
panel or a new mode will not compile until someone decides what it
means.

```rust
pub enum Vis { Show, Grey(&'static str), Hide }
pub fn panel(p: PanelType, m: RenderMode, solid: Solid) -> Vis;
pub fn control(c: Control, m: RenderMode, tone: ToneMapMode) -> Vis;
pub fn fly_mode(kind: ViewKind) -> Vis;
```

The `&'static str` is a `visibility.*` locale key naming the
**reason**, shown on hover, so a user who goes looking for a control
learns why it is not there instead of wondering whether it exists.

**No panel is ever `Hide`.** Hiding a panel's menu row would make it
unreachable *and* undiscoverable, so an unavailable panel is greyed in
the Window menu and its body shows the reason instead of its controls
(`PanelViewer::render_panel`). `Hide` is for whole sections and
controls inside a panel.

Panels:

| Panel | 2D | 3D | Escape | Simulation |
|---|---|---|---|---|
| Transforms, Triangle Editor, Variations | Show | Show | Show | Show |
| Xaos Editor, Subflames, Paths | Show | Show | Grey (flame only) | Grey (flame only) |
| Random Generator | Show | Show | Grey (makes a flame) | Grey (makes a flame) |
| Escape Fractal | Show | Show | Show | Grey (other engine) |
| Simulation | Show | Show | Grey (other engine) | Show |
| Textures | Grey | Grey | Show | Show |
| Formula Editor (escape-time formulas and colourings, Rhai and Ultra Fractal's and Fractint's languages; in the flame modes, Rhai variations; [ESCAPE.md](ESCAPE.md#the-formula-language)) | Show | Show | Show | Grey (other engine) |
| Hybrid Designer (Kalles Fraktaler-style hybrid formulas; [ESCAPE.md](ESCAPE.md#the-formula-language)) | Grey | Grey | Show | Grey (other engine) |
| Lighting & Material (follows `Solid::of`, not the mode) | Grey | Show | Show for a solid formula or a terrain, else Grey | Show for a terrain, else Grey |
| Every other panel | Show | Show | Show | Show |

- **The transform editors are available in every mode.** Escape mode D
  renders the flame's attractor as a distance field, and a simulation
  uses the flame's transforms as its per-layer warps when
  `sim.use_transforms` is on, so in both, editing a transform edits the
  picture. Elsewhere the panels prepare a flame before switching.
- **Each engine's panel is the way into its mode**, so both are offered
  from a flame mode; each greys the other's, because a config is exactly
  one mode.
- **Textures** go into an escape-time fractal and are made in Simulation
  mode (Save as texture), so the panel is available in both; picking a
  texture acts only in Escape.
- **Everything else is available everywhere**, because both non-flame
  engines write an image in the flame accumulator's layout and go
  through the same density-effects → tonemap → colour-effects tail.
- **Fly mode** is a question about the camera, not the mode:
  `fly_mode(ViewKind)` offers it for every 3D view (a 3D flame, a solid,
  an escape terrain, a simulation terrain).

`Control` groups controls that share a fate rather than naming each
widget:

| Control | What it covers | 2D | 3D | Escape | Simulation |
|---|---|---|---|---|---|
| `ChaosGame` | pause, reset accumulation, max iterations, iterations per thread, burn-in, deterministic RNG, blend controls, the Rendering menu | Show | Show | Hide | Hide |
| `TonemapPresets` | the tone-map preset dropdown and Reset Colors | Show | Show | Hide | Hide |
| `SpatialFilter` | the spatial filter and blur edges | Show | Show | Hide | Hide |
| `DensityLevels` | the density levels section | Show | Show | Hide | Hide |
| `DeepZoom` | the View panel's Deep Zoom section | Show | Show | Hide | Hide |
| `ColorMode` | colour mode and what hangs off it | Show | Show | Hide | Hide |
| `FocusedRendering` | Focused Rendering and PathMap | Show | Grey | Hide | Hide |
| `OrbitCache` | escape's deep-zoom reference orbit cache | Hide | Hide | Show | Hide |
| `ViewNavigation` | drag, wheel, pinch, arrow keys, the View menu | Show | Show | Show | Show |
| `TonemapMode` | the tone-map mode selector | Show | Show | Grey | Grey |
| `LogOnlyTone` | gamma threshold, brightness, vibrancy, highlights | Show | Show | Grey | Grey |
| `AlphaBlendCurve` | alpha blend low/high | Show (Grey under Linear) | Show (Grey under Linear) | Grey | Grey |

`visibility::gated(ui, control, mode, tone, |ui| ...)` draws a widget
under that policy; tests pin each non-flame mode's exact set.

### Engine lifetimes

Both non-flame renderers are created lazily and are released when the
mode changes away from them (`App::release_inactive_engines`). The two
cases differ:

- The **escape** renderer rebuilds itself from the config, so returning
  costs only the re-render. It holds 16 bytes per pixel of
  supersampled area at minimum — about 506 MB for a 1080p viewport at
  4x antialiasing — which is why releasing it is worth doing.
- The **simulation** grid *is* its state, so returning restarts it from
  the seed at step 0.

### The timeline and the simulation grid

`sim.steps` means two things, and **whoever is driving decides which**.
To the transport it is Max Steps, a CAP: the run stops there once, and
`0` means uncapped. To the animation timeline it is a TARGET: the
picture at a given time is the state at that step count. The exporter
reads it the second way.

**The timeline owns the step count** when it is playing an animation
that has a `Sim.Steps` track, or when a target it committed has not
been reached yet (`App::timeline_owns_sim`). While it does:

- the grid is advanced toward the track's value, **budgeted** per
  display frame from the measured cost of a step
  (`SimRenderer::steps_in`), so a two-thousand-step jump is walked
  over a few frames rather than freezing the window — a scrub can ask
  for one on every slider event;
- Run / Pause / Step and the spacebar are inert, and the panel greys
  them. Run is *disengaged* when playback starts rather than merely
  ignored: left engaged it would resume the instant the timeline let
  go;
- playback is **paced by the grid**, as escape playback is by
  settling — the controller is held while the grid is short of the
  frame's target, then advanced by everything that elapsed. A heavy
  model plays slower rather than showing a lagging state, so what
  plays is what exports.

An animation with **no** step track leaves all of this alone: the
transport keeps working and the run keeps going underneath, so
animating a colouring parameter over a free-running simulation works.

**Going backwards.** The rule is not invertible, so reaching a lower
step count means reseeding and re-running. A falling target is
therefore HELD while the motion is continuous and applied only on a
discrete event — a scrubber release, a `Loop` wrap, or a `PingPong`
turnaround at t = 0 (`sim::timeline_target_applies`,
`app::animation_update::playback_motion`). Without the hold, dragging
left would restart the run on nearly every slider event, and ping-pong
would restart once per frame for half of every cycle.

So a track written 2000 → 0 plays in-app as a still of the highest
state reached, with the panel saying why and pointing at the
scrubber; **export renders every frame of it correctly**, because a
video frame is the state at its time whatever the direction. Both
rules are pure functions in [src/sim/mod.rs](../../src/sim/mod.rs)
with tests that need neither a GPU nor an `App`. The timeline itself is
in [ANIMATION.md](ANIMATION.md).

---

## Panel reference

Every panel renders from its own file. `PanelViewer::render_panel`
([panel_viewer.rs](../../src/ui/panel_viewer.rs)) is the dispatch, and
it is the authoritative list — the table below is a map, not a
contract.

| Panel | File | Notes |
|---|---|---|
| Fractal Viewport | `panel_viewer.rs` (`render_fractal_viewport`) | the picture and all viewport input |
| Transforms | [transforms.rs](../../src/ui/transforms.rs) | transform list, affine, variations; see [TRANSFORMS.md](TRANSFORMS.md) |
| Triangle Editor | [triangle_editor.rs](../../src/ui/triangle_editor.rs) | below |
| Variations | [variations.rs](../../src/ui/variations.rs) | browse the registry; see [VARIATIONS.md](VARIATIONS.md) |
| Xaos Editor | [xaos_editor.rs](../../src/ui/xaos_editor.rs) | |
| Subflames | [subflames.rs](../../src/ui/subflames.rs) | which flame the editors operate on |
| Paths | [paths_panel.rs](../../src/ui/paths_panel.rs) | Focused Rendering's plan as a tree of paths; see [FOCUSED-RENDERING.md](FOCUSED-RENDERING.md) |
| View | [view.rs](../../src/ui/view.rs), [view_controls.rs](../../src/ui/view_controls.rs) | below |
| Lighting & Material | [lighting_panel.rs](../../src/ui/lighting_panel.rs) | below |
| Escape Fractal | [escape_panel.rs](../../src/ui/escape_panel.rs) | below |
| Simulation | [sim_panel.rs](../../src/ui/sim_panel.rs) | below |
| Textures | [textures_panel.rs](../../src/ui/textures_panel.rs) | below |
| Colors / Tone Mapping | [tone_mapping.rs](../../src/ui/tone_mapping.rs) | below; see [TONE-MAPPING.md](TONE-MAPPING.md), [COLOR.md](COLOR.md) |
| Palette Editor | [palette_editor.rs](../../src/ui/palette_editor.rs), [palette_generate.rs](../../src/ui/palette_generate.rs) | see [PALETTE_LIBRARY.md](PALETTE_LIBRARY.md) |
| Palette Library | [palette_library.rs](../../src/ui/palette_library.rs) | see [PALETTE_LIBRARY.md](PALETTE_LIBRARY.md) |
| Effects | [effects_panel.rs](../../src/ui/effects_panel.rs) | see [EFFECTS.md](EFFECTS.md) |
| Rendering | [settings.rs](../../src/ui/settings.rs) | below |
| Performance | [performance.rs](../../src/ui/performance.rs) | |
| History | [undo_history.rs](../../src/ui/undo_history.rs) | the undo/redo history |
| Animation | [animation_panel.rs](../../src/ui/animation_panel.rs), [track_editor.rs](../../src/ui/track_editor.rs) | see [ANIMATION.md](ANIMATION.md) |
| Signals | [signal_panel.rs](../../src/ui/signal_panel.rs) | audio files, live capture, generators, signal files; see [ANIMATION.md](ANIMATION.md) |
| Scripts | [scripts_panel.rs](../../src/ui/scripts_panel.rs) | below |
| Fractal Browser | [fractal_browser.rs](../../src/ui/fractal_browser.rs) | see [FRACTAL-BROWSER.md](FRACTAL-BROWSER.md) |
| Random Generator | [random_generator.rs](../../src/ui/random_generator.rs) | batches land in the Fractal Browser |
| Export | [export_panel.rs](../../src/ui/export_panel.rs) | below |
| Config Import/Export | [config_dialog.rs](../../src/ui/config_dialog.rs) | |
| Help, Keyboard Shortcuts | [help.rs](../../src/ui/help.rs) | |
| Account, Save Online | [login_dialog.rs](../../src/ui/login_dialog.rs), [save_online_dialog.rs](../../src/ui/save_online_dialog.rs) | see [ONLINE.md](ONLINE.md) |

### Triangle Editor

Visual affine editing: each transform is drawn as a triangle whose
vertices are the affine basis (O, X, Y). The mouse mode is Move Points,
Translate, Rotate (the default) or Scale; a drag converts screen space →
fractal space → affine coefficients and writes them back through
`update_batch`. A drag is one **modify session**
(`ConfigManager::start_modify_transform`): the intermediate edits apply
silently and the release records one history entry, so a drag is one
undo for any pool member (normal, linked or final). A plane selector
offers XY, YZ and ZX (the JWildfire plane affines); the YZ and ZX planes
have no effect in 2D.

### View

See [The View panel](#the-view-panel).

### Lighting & Material

See [The Lighting & Material panel](#the-lighting--material-panel).

### Escape Fractal

The whole editing surface for escape-time mode. Outside Escape it shows
only an enter button. Inside: the formula and its parameters (sliders
generated from the formula registry), presets, the Julia switch and
constant, the iteration cap and bailout, the coloring and its
parameters, and the sections Auto Contrast, Palette Mapping, Relief
Shading, Texture Layer, 3D Terrain, Reference orbit (engine) and
Diagnostics. The view itself (centre, zoom, rotation) is the View
panel's. Every control writes an `Escape*` `ConfigPath`, so undo,
coalescing and animation tracks work unmodified. See
[ESCAPE.md](ESCAPE.md) and [TERRAIN.md](TERRAIN.md).

### Simulation

The same shape as the escape panel, plus a **transport**: Run, Pause,
Step and Reset, and the step counter, because a simulation is stateful.
Then the Model section (one list whose first entry is the model and the
rest its layers), Settings (the grid, Warp and more), Colouring, 3D
terrain, Matte and Couplings, with parameter sliders generated from the
model and colouring registries, and Save as texture. See
[SIMULATION.md](SIMULATION.md).

### Textures

The simulation textures an escape-time config can use: the shipped
presets (`assets/textures/*.fflame`) and the user's own, saved from
Simulation mode with Save as texture, each with a generated preview
(desktop generates one a frame; the web spawns them). Picking one
writes it into the config as one undo step, and only in Escape mode; in
Simulation the panel browses and manages the saved list. The Overlay
section sets how the texture is drawn over the picture.

### Paths

Focused Rendering's plan drawn as a tree of *paths* (the words the plan
draws through), with trim, remove, solo and restore, plus the Focused
Rendering switch, which the View panel's Deep Zoom section shows too.
See [FOCUSED-RENDERING.md](FOCUSED-RENDERING.md).

### Scripts

Runs generator and modifier scripts. Declared parameters appear as
sliders above Run, and the seed makes a result reproducible. The panel
only produces a config (or a batch, which lands in the Fractal
Browser's Random tab); the app applies it as one undo step. See
[SCRIPTING-GUIDE.md](SCRIPTING-GUIDE.md) and [SCRIPTING.md](SCRIPTING.md).

### Export

PNG export: custom resolution, supersampling, transparency (with or
without background, premultiplied alpha) and stripping metadata. Video
export is the Animation panel's. See [EXPORT.md](EXPORT.md).

### Rendering

Almost everything here configures the chaos game — max iterations,
iterations per thread, burn-in, deterministic RNG, the blend controls,
pause and reset accumulation — so it is hidden outside the flame modes
(`Control::ChaosGame`). What remains everywhere is VSync and the frame
cap, which are `SystemSettings` device preferences rather than fractal
parameters. In Escape mode on desktop it also shows the deep-zoom
reference orbit cache (its disk budget and a Clear button,
`Control::OrbitCache`).

### Colors

Four sections: tone mapping, tone curve, density levels, and colour &
appearance (colour mode, the palette dropdown, palette transforms).
Both non-flame engines go through this same tail, so most of it applies
to them — but the logarithmic branch never runs there, density levels
are suppressed in the frame loop, the spatial filter lives inside a
compute pass that is never dispatched, and the preset dropdown would set
a Log-calibrated look that renders a unit-range image black. `Control::*`
in `visibility.rs` encodes which is which.

---

## The View panel

One panel, in every mode ([camera-unification.md](../archive/3d-camera/camera-unification.md),
C5): [view.rs](../../src/ui/view.rs) lays it out by the shown camera's
`ViewKind`, from the shared widgets in
[view_controls.rs](../../src/ui/view_controls.rs). The camera model is
in [CAMERA.md](CAMERA.md).

| Section | Shown for | Holds |
|---|---|---|
| Camera | 2D views, and a 3D flame's projected picture | zoom, centre (the escape centre as exact decimals), rotation, an arrow pad and Reset, through `camera::gesture` as the keys and the mouse are |
| 3D Camera | every 3D view | position, target, zoom or distance, pitch, yaw, bank, roll, field of view or perspective, Reset, and fly mode with its settings. A camera that stores a target shows its eye as a readout; a terrain's target has a lift |
| Camera Lens | an escape plane or solid | a variation used as a lens on the pixels or rays |
| Image | every escape view; a 2D simulation | escape: antialiasing (supersampling) and how the samples combine; simulation: the resolve filters (upscale, fit, downscale) and tiling a periodic field |
| Atmosphere, Depth of Field | every 3D view | fog, a flame's depth weighting, a terrain's far and haze; a flame's focus and blur or the path tracer's lens; then a button that opens the Lighting & Material panel |
| Deep Zoom | flames (`Control::DeepZoom`) | auto exposure and the Focused Rendering switch |
| Render Mode, Preserve Z, post symmetry | flames | the 2D/3D switch (through `switch_render_mode`), JWildfire's `preserve_z` (3D), post symmetry |

Angles show in degrees on a −180..180 slider that takes typed values
beyond (`view_controls::angle_slider`); the config stores radians.

---

## The Lighting & Material panel

Every lit 3D view's light in one place (`PanelType::SolidLighting`, the
name saved layouts know; [lighting_panel.rs](../../src/ui/lighting_panel.rs);
[3d-look-and-tools.md](../projects/3d-look-and-tools.md), P3). It was
the View panel's Lighting & Material section from camera-unification C5
until 2026-10-19, with a solid's shadows among the Escape panel's
formula parameters and a terrain's texture and lake in the Escape
panel. Available where `Solid::of` says the view draws a surface;
sections by the view:

| Section | Solid | Escape terrain | Simulation terrain | 3D flame |
|---|---|---|---|---|
| Rendering | render tier; samples, noise threshold, bounces, denoise (not Lit) | the same | the same | -- |
| Sky | sky colour (the background, as in Colors), gradient and zenith, Studio light, Sky light (not Lit, unless the studio is on) | the same | the same | -- |
| Lights | Light 1 to 4: kind, colour, its settings, intensity | the same | the same | directional only, once Shading strength is above 0 |
| Shading | strength, ambient, occlusion strength and reach (not Path traced) | the same, the terrain's reach | the same, the terrain's reach | strength, ambient, SSAO strength and radius, normal smoothing, gap fill |
| Shadows | Shadows, Shadow sharpness (formula parameters), Ray tolerance | the terrain's | the terrain's | shadow maps' strength |
| Material | diffuse, highlight and its size, gloss, roughness, glow | the same | the same | the same, once lit |
| Surface | -- | the surface texture; the lake's tint and roughness | -- | solid strength, surface thickness |

A solid's **Shadows, Shadow sharpness and Occlusion reach** stay formula
parameters in the file (`escape::ifs::LIGHTING_PARAMS`); the Escape panel
leaves them out of its list and says where they are. A plane's relief
keeps its texture in the Escape panel.

**An untouched rig** (`SolidShadingSettings::rig_untouched`) lights a
solid or a terrain with a default key light, not with its stored
values. The panel shows its rows from the start, with the values the
first change makes current, and says so: that change takes the rig over
(`render_mode::rig_taken_over`, the 3D look's own -- strength 1, ambient
0, a terrain's light 0 at the default sun) with it, in one undo step.
The coat and the glow are not the rig and leave it untouched. Until
2026-10-19 a change made the stored rig current at its strength of 0,
and the lit tier fell to the bare colours.

## Input Handling

One set of controls drives every camera. The gestures are pure
functions on the config in
[src/camera/gesture.rs](../../src/camera/gesture.rs) and
[src/camera/fly.rs](../../src/camera/fly.rs), each returning a
`CameraEdit` (the writes and their history entry) that the caller
applies; `gesture::view_kind` decides which camera they move. The model
behind them is in [CAMERA.md](CAMERA.md).

| | 2D view | 3D view | 3D, fly mode |
|---|---|---|---|
| Drag | pan | pan (a terrain: along the ground) | look (about the eye) |
| Alt+drag, right drag | rotate (grab and twist) | orbit the target (a turntable) | look |
| Wheel | zoom toward the cursor | dolly toward the cursor (a flame: its 2D zoom) | flying speed |
| Shift | finer steps | finer steps | sprint |
| W A S D Q E | -- | -- | fly (Q/E: screen up in FreeLook, world up in FPS) |
| Arrows, + / − | pan, zoom | pan, dolly | -- |
| One finger | pan (rotate with the Turn toggle) | pan (orbit with the Turn toggle) | look |
| Long press, then drag | rotate | orbit | look |
| Two fingers | pinch zoom, twist rotate, drag pan | pinch dolly, twist orbit, drag pan | -- |

- **The viewport** ([panel_viewer.rs](../../src/ui/panel_viewer.rs)):
  `view_drag` and `view_scroll` are the one input path, used by the
  viewport body and the tab-bar strip above it alike.
- **Touch**: `TouchTracker`, because egui's own multi-touch does not
  work on the web. A finger held still within 10 points for half a
  second is a **long press -- the touch's right button**: held, it reads
  the path in PathMap (as holding the right button does); dragged, it
  turns. The **Pan/Turn toggle** at the viewport's bottom left is the
  touch's **Alt**: a sticky choice of what a one-finger drag does. It
  shows on the compact layout and once a touch has been seen.
- **Keys** ([src/app/input.rs](../../src/app/input.rs),
  `handle_keyboard`): arrows pan 5% of the view a press and + / −
  zoom by 1.5, through the same gestures; Ctrl/Cmd+Z and +Y undo and
  redo; Space plays the animation, or runs the grid in Simulation mode
  (`render_mode::space_runs_the_simulation`; inert while the timeline
  owns the step count); F toggles full screen; **F2 toggles fly mode**
  in any 3D view. Keys go to the app only when egui has not consumed
  them -- except a fly key's release, which always lets go, as does
  losing the window's focus.
- **History**: one entry name per gesture, all coalescing -- pan, zoom,
  rotate, orbit, and the fly camera's -- and Reset View as its own entry.
  During animation playback a gesture writes silently.
- **Path readback**: in PathMap mode only, holding the right button (or
  a long press) queries the path under the pointer.

---

## Compact (mobile) UI

The Compact layout is the viewport alone, with a menu button anchored
top right ([compact_menu.rs](../../src/ui/compact_menu.rs)). Design
record: [mobile-ui-redesign.md](../archive/projects/mobile-ui-redesign.md).

- **Entering it.** On first launch the app picks it when the window is
  under 600 logical points wide and saves that as
  `SystemSettings::compact_mode`; afterwards the saved value decides.
  Window ▸ Mobile View and the compact menu's Desktop View switch, and
  the switch is saved too.
- **The menu** holds a Window submenu (`COMPACT_WINDOW_MENU`, with the
  same mode policy as the desktop rows), File, View (Reset View), Mode,
  the animation transport (Play/Pause, Stop), Reset Accumulation, Undo,
  Redo, the account button (online mode), Help, and Desktop View. There
  is no language picker on the compact layout.
- **Panels open docked**, not floating (`Workspace::open_panel` →
  `open_compact_panel`): the first splits off the viewport (below it at
  0.55 in portrait, beside it at 0.60 in landscape) and later ones join
  it as tabs.
- **Touch sizing**: while compact, the style uses larger hit targets
  (interact size 40×28, wider button padding and item spacing) and
  always-visible scrollbars.

---

## Cross-panel results: UiResponse

Most controls write the config directly through `ConfigManager`, and
the GPU follows from that (next section). `UiResponse`
([src/ui/response.rs](../../src/ui/response.rs), about 70 fields) carries
what is not a config write: file and dialog requests, palette library
management, transform-pool structure edits and attachments, a generated
flame, script result or batch, a preset or online selection, undo and
redo requests, requests to open a panel, the simulation transport
(`sim_running`, `sim_step_once`, `sim_reseed`), animation seeks, and the
online actions.

`EguiLayer` builds it once a frame ([src/ui/mod.rs](../../src/ui/mod.rs)),
and `App::handle_ui_responses`
([src/app/ui_handlers.rs](../../src/app/ui_handlers.rs)) dispatches it
to a fixed sequence of handlers — config, transforms, attachments,
generated flames, palettes, files, subflames, undo/redo, panel
requests, preset selection, animation, word solo, online saves and
loads, and the variation, effect and script catalogs. Export requests
are handled in [src/app/mod.rs](../../src/app/mod.rs). The menus write
`MenuActions`, which `EguiLayer` turns into `UiResponse` fields or acts
on itself.

---

## State Management Integration

All parameter changes flow through `ConfigManager`
([src/config/manager.rs](../../src/config/manager.rs)); the full account
is [CONFIG.md](CONFIG.md).

```rust
// A single parameter
let update_type = config_manager.update_param(ConfigPath::Exposure, value.into())?;

// Several as one undo entry (triangle editor, colour pickers, mode switch)
config_manager.update_batch(changes, "history.action.some_edit".to_string())?;
```

- **Slider helpers** ([src/config/slider.rs](../../src/config/slider.rs)):
  the `ConfigSlider` builder (`ConfigSlider::new(manager, path, range,
  label).lazy(true).tooltip(..).show(ui)`) and the `ConfigSliderUi` /
  `LazyUndoUi` extension traits on `egui::Ui` (`ui.config_slider(..)`,
  `ui.config_drag_value(..)`, `ui.lazy_slider(..)`, `ui.lazy_drag(..)`).
  Each returns a `ConfigSliderResult` (`changed`, `should_capture`,
  `update_type`). Many panels call `update_param` directly instead.
- **What a change requires** is its `UpdateType`
  ([src/config/delta.rs](../../src/config/delta.rs)): `None`, `ViewOnly`,
  `ToneMappingOnly`, `ShadingOnly`, `ColorOnly`, `IterationReset`,
  `EscapeRerender`, `SimRerender`, `SimResample`, `SimReseed`, ordered so
  `merge` keeps the stronger. The frame loop reads the accumulated
  `ConfigManager::get_pending_actions()` in
  [src/app/gpu_updates.rs](../../src/app/gpu_updates.rs).
- **Undo/redo**: delta-based, up to 500 entries; rapid changes to the
  same control coalesce into one entry (a 500 ms pause or a 3 s span
  starts a new one). Ctrl/Cmd+Z and Ctrl/Cmd+Y, Edit ▸ Undo/Redo, or the
  History panel.
- **Device preferences** (`SystemSettings`) go through
  `update_system_setting` and are not undoable.
- **Slow settings** ([src/ui/slow_controls.rs](../../src/ui/slow_controls.rs)).
  A slider takes its value from the pointer every frame, so a setting
  that slows the next frame overshoots when dragged: the frame after each
  change comes late, the pointer has moved on, and the value jumps. Such
  settings get one of two controls:
  - `whole_numbers`, a dropdown, for a range of at most sixteen whole
    numbers: Beam Width, the path tracer's bounces, the cyclic
    automaton's range, and a few variations' step counts;
  - `on_release`, which shows the value while dragged and commits it when
    the drag ends (a click, an arrow key or a typed number commits at
    once), for the rest: an IFS walk's depth, March Steps, the iteration
    cap, Lenia's and SmoothLife's radii, physarum's population, steps per
    frame, the grid's size, and the variations' iteration counts.

  What is slow is listed beside each registry: `escape::SLOW_PARAMS`,
  `sim::SLOW_PARAMS` and `variations::SLOW_PARAMS`, each held to real
  parameter names by a test. The variations' list is measured
  (`renderer::param_cost`).

### Variation ordering

Panels list variations by category, then in registration order:
`registry.by_category(cat)` returns a category's variations in
registration order, and `registry.names()` is the whole registry in that
order ([src/variations/mod.rs](../../src/variations/mod.rs)). Never
iterate the registry's `HashMap`.

---

## Common UI Modification Tasks

### Add a panel
1. Add a variant to `PanelType` ([workspace.rs](../../src/ui/workspace.rs)), a title arm in its `Display` impl, and a `default_size_for_panel` arm.
2. Add a case to `visibility::panel` — the match is exhaustive, so this is not optional — and add it to the test list `ALL_PANELS` (the count is asserted in `the_test_list_covers_every_panel`).
3. Add a row to `visibility::WINDOW_MENU` (and to `COMPACT_WINDOW_MENU` if it belongs on a phone).
4. Add a dispatch arm in `PanelViewer::render_panel` ([panel_viewer.rs](../../src/ui/panel_viewer.rs)).
5. Add the `menu.window_*` and `panels.*` keys to `locales/en.yml`.

### Add a control that only some modes can use
1. Add a variant to `visibility::Control`, or reuse one that shares its fate.
2. Answer it in `visibility::control`, and add it to `ALL_CONTROLS` in the tests.
3. Wrap the widget in `visibility::gated(ui, Control::X, mode, tone, |ui| { ... })`, or check `control(...) != Vis::Hide` for a whole section.
4. Add the reason's key under `visibility.*` in `locales/en.yml`.

Do **not** write `matches!(config.render_mode, ...)` inside a panel.
That is what the policy module replaced, and scattered copies are how
the menus and the dispatcher came to disagree.

### Add a result another part of the app must act on
Most controls write straight through `config_manager.update_param` and
need nothing more. For anything else:

1. Add a field to `UiResponse` ([response.rs](../../src/ui/response.rs)) and its default.
2. Set it from the panel, through the `PanelContext` the panel renders with ([panel_viewer.rs](../../src/ui/panel_viewer.rs)) and `EguiLayer`'s collection of it in [src/ui/mod.rs](../../src/ui/mod.rs).
3. Handle it in one of the `handle_*` methods in [ui_handlers.rs](../../src/app/ui_handlers.rs).

### Add a keyboard shortcut
1. Add a case to `handle_keyboard()` in [src/app/input.rs](../../src/app/input.rs).
2. A key that moves the view goes through `camera::gesture` (as the
   arrows and + / − do), so it moves whichever camera is shown.
3. List it in the Help panel's controls ([help.rs](../../src/ui/help.rs)).

---

## Internationalization

Every label is a `t!("group.key")` lookup in `locales/*.yml`; English is
the reference and the fallback. The language picker is the globe menu
in the menu bar's right strip. Translations, fonts and the locale tests
are in [I18N.md](I18N.md).
