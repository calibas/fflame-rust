# UI by render mode: one Mode menu, one visibility policy

**Status:** plan of record, 2026-09-09. Branch `ui-modes`, off
`simulation-mode`. Surveyed before planning; every claim below carries
its `file:line` so a reader can argue with the code rather than with
the prose. **No code written yet.**

## 0. What is being asked for

Four render modes exist — `TwoD`, `ThreeD`, `Escape`, `Simulation`
(`src/scene/transforms.rs:1834`). The UI was built for the first two
and has had the other two grafted on. The ask:

1. Switching modes gets its own top-level **Mode** menu, replacing the
   scattered toggle buttons.
2. Panels and menu items that mean nothing in the active mode stop
   pretending otherwise.
3. Individual controls inside *shared* panels — Colors, Tone Mapping,
   Rendering, Effects — get the same treatment.

Loading a file already switches mode correctly and is not in scope
(`src/app/mod.rs:1364-1406`).

## 1. What exists today

### 1.1 The mode itself

`render_mode` lives on `FractalConfig`, not on `Flame`
(`src/config/fractal_config.rs:39`), moved there in config v3
(`fractal_config.rs:971-1003`). It is always serialised so the cloud
blob's top-level field is never absent. `RenderMode::ALL`
(`transforms.rs:1879`) publishes the four in wire order and is held in
step with the enum by an exhaustive match plus a length assertion —
**a Mode menu should iterate it rather than hand-listing four items.**

`flame`, `escape` and `sim` sub-configs all live on the config at once
and are preserved while inert (`fractal_config.rs:30, 181, 188`), so
mode round-trips already keep their state. Nothing needs adding there.

**The mode is switched from five places, none sharing code:**

| # | Site | Reaches | Resets tone mapping |
|---|---|---|---|
| 1 | View menu (`src/ui/menu_bar.rs:143-151`) | 2D, 3D | no |
| 2 | Compact View submenu (`src/ui/compact_menu.rs:208-216`) | 2D, 3D | no |
| 3 | View panel (`src/ui/view.rs:138-164`) | 2D, 3D | no |
| 4 | Escape panel toggle (`src/ui/escape_panel.rs:35-53`) | Escape | yes |
| 5 | Simulation panel toggle (`src/ui/sim_panel.rs:90-119`) | Simulation | yes |

Only 4 and 5 route through `switch_render_mode`
(`src/ui/escape_panel.rs:1478-1517`), the one helper that batches the
tone-map reset into a single undo entry. Sites 1–3 call
`update_param` directly, so **leaving Escape by the menu keeps
escape-calibrated tone mapping on a flame.**

`ConfigPath::RenderMode` (`src/config/delta.rs:225`) returns
`UpdateType::IterationReset` (`delta.rs:2620-2633`), which deliberately
does *not* reset accumulation (`src/config/manager.rs:302`). It is
undoable, but **coalescing is on**: the line that would exempt it is
commented out at `src/config/manager.rs:170`, so rapid mode changes
merge into one undo entry.

Two consequences of the mode being carried into the menus as a
**boolean** (`render_mode_2d`, `src/ui/menu_context.rs:81`):

- In Escape and Simulation the View menu draws "3D Mode" as selected.
  It is simply wrong.
- Fly Mode's gate is `!render_mode_2d` (`menu_bar.rs:497`), so the
  button stays enabled in both non-flame modes. Only a runtime check
  in `src/app/fly_camera.rs:303,508` stops it doing anything.

### 1.2 Panels

29 `PanelType` variants. All gating today is two `matches!` blocks in
`PanelViewer::render_panel` (`src/ui/panel_viewer.rs:762-813`), which
replace the panel body with one shared hint line. **The menus do no
gating at all** — every panel can be opened in every mode, and eight of
them lead to a dead stub in Escape.

Legend: ● works · ◐ shown but partly or wholly inert · ○ hidden today

| Panel | 2D | 3D | Esc | Sim | Note |
|---|---|---|---|---|---|
| FractalViewport | ● | ● | ● | ◐ | Sim: pan/zoom write flame config the sim ignores |
| Transforms | ● | ● | ○ | ◐ | Sim: only when `sim.use_transforms`, default off |
| TriangleEditor | ● | ● | ○ | ◐ | same |
| Variations | ● | ● | ○ | ● | informational; sim uses transforms as layer maps |
| Colors | ● | ● | ◐ | ◐ | §1.4 |
| PaletteEditor | ● | ● | ● | ● | both engines sample the shared palette texture |
| PaletteLibrary | ● | ● | ● | ● | |
| FractalBrowser | ● | ● | ● | ● | rows carry a mode badge |
| Effects | ● | ● | ● | ● | shared tail; all entries live |
| History | ● | ● | ● | ● | |
| Animation | ● | ● | ● | ● | target picker already adds per-mode categories |
| Signal | ● | ● | ● | ● | |
| Scripts | ● | ● | ● | ● | script API covers both engines |
| Performance | ● | ● | ● | ● | iteration lines sit at 0 |
| Help | ● | ● | ● | ● | |
| KeyboardShortcuts | ● | ● | ◐ | ◐ | lists fly mode and Alt-drag regardless |
| ConfigDialog | ● | ● | ◐ | ◐ | Flame XML is flame-only by format |
| Export | ● | ● | ● | ◐ | Sim transparent PNG re-tonemaps an empty buffer |
| LoginDialog | ● | ● | ● | ● | |
| SaveOnlineDialog | ● | ● | ● | ● | both modes now map to the API enum |
| Rendering | ● | ● | ◐ | ◐ | chaos game is off; orbit cache is escape-only |
| View | ● | ● | ○ | ○ | |
| XaosEditor | ● | ● | ○ | ○ | correct even though Transforms survives in Sim |
| Subflames | ● | ● | ○ | ○ | subflames are never layer maps |
| PathEditor | ● | ● | ○ | ○ | no chaos game to filter |
| RandomGenerator | ● | ● | ◐ | ◐ | works, but its output leaves the mode |
| SolidLighting | ○ | ● | ○ | ○ | already self-declares "requires 3D" |
| Escape | ◐ | ◐ | ● | ○ | in flame modes: a bare enter button |
| Simulation | ◐ | ◐ | ○ | ● | same |

Notes worth carrying into the design:

- **The hint text is wrong in two of the three places it is used.**
  `escape_panel.flame_only_hint` (`locales/en.yml:2130`) says the panel
  "edits the flame and is inactive in Escape mode", and is reused for
  Simulation and for the Escape/Simulation cross-hiding, where the
  panel edits no flame at all.
- **Transforms, TriangleEditor and Variations in Simulation are
  conditionally live.** `sim.use_transforms` defaults to false
  (`src/config/sim.rs:1156`) and is buried in the Simulation panel's
  Layers header. With it off, all three panels are fully interactive,
  write real config changes and undo entries, and change nothing on
  screen. Nothing in them mentions the flag.
- **The default Simulation layout and the gate disagree.** The layout
  test asserts Transforms is absent from the Simulation layout
  (`src/ui/workspace.rs:722`) while the gate deliberately keeps it
  usable there. Defensible as opt-in, but it should be deliberate.
- `src/ui/panels.rs` is a zero-byte file, and `locales/en.yml` still
  carries `panels.preset_library`, `panels.file_browser`,
  `panels.settings`, `panels.tone_mapping` and `menu.fractal` with no
  matching variant or call site.

### 1.3 Menus

The desktop bar is File, Edit, View, Rendering, Window, Help
(`src/ui/menu_bar.rs`). There is **no Fractal menu** despite the
`menu.fractal` key existing. Flame-only items, by menu:

| Menu | Item | Meaningful in |
|---|---|---|
| File | Export Flame XML | 2D, 3D — the format has no escape/sim form |
| File | Random Flame, Random Batch | 2D, 3D — output is a flame |
| View | 2D Mode / 3D Mode | becomes the Mode menu |
| Rendering | Reset Accumulation | 2D, 3D — nothing else accumulates |
| Rendering | Iterations per Thread (6 entries) | 2D, 3D — chaos-game concept |
| Rendering | Reset to Defaults | 2D, 3D in substance |
| right strip | Fly Mode | 3D only, currently live in Esc and Sim |
| Window | 22 panel rows | ungated; §1.2 says which are dead |
| Window | Workspace Layout ▸ Escape Time, Simulation | rearranges panels **without changing mode** |

Everything else — open, save, undo, redo, export PNG, config import,
palettes, language, account, help — is universal.

`compact_menu.rs` is a hand-maintained partial copy: 18 of the 22 panel
rows, its own duplicate of the 2D/3D pair, and no workspace submenu.
Any mode-aware change to a menu has to be made twice unless the panel
list is hoisted into shared data first.

The Rendering *panel* (`src/ui/settings.rs`) holds the items the user
called out — max iterations (`:49`), iterations per thread (`:71`),
burn-in (`:88`), deterministic RNG (`:138`) — and gates none of them by
mode. Its orbit-cache block (`:100-136`) is escape-only and working.

### 1.4 Shared colour and effects controls

All three engines feed the **same tail**: generator → density effects →
`tonemap.wgsl` → colour effects. Escape writes an `Rgba32Float` image
in accumulator layout (`src/escape/assembler.rs:310`); Simulation's
colour pass does the same (`src/sim/assembler.rs:1046`). So the Colors
and Effects panels are genuinely shared, and `is_non_flame`
(`src/app/mod.rs:2655`) suppresses only the flame-only stages.

Two facts drive everything below. `accum.a` is 0–1 coverage in the
non-flame modes rather than a cumulative hit count. And
`sample_density` is stale flame data floored at `1e-6`
(`src/renderer/compute_kernel.rs:2577`), which kills every control that
divides by it. Entering a non-flame mode resets tone mapping to Linear,
so in practice these modes sit in the Linear branch.

| Control | Esc | Sim | Why not |
|---|---|---|---|
| Tone-map preset dropdown | **harmful** | **harmful** | every preset is Log-calibrated; applying one re-blacks the image, the exact thing the entry reset prevents |
| Reset Colors to Defaults | **harmful** | **harmful** | same trap (`src/ui/mod.rs:2561`) |
| Exposure, Gamma | ● | ● | read in all branches |
| Saturation, Hue Shift | ● | ● | applied after the mode branch |
| Brightness, Vibrancy, Highlights, Gamma Threshold | ✗ | ✗ | Logarithmic branch only |
| Tone Map Mode | ◐ | ◐ | Log is selectable and unusable |
| Highlight Clipping | ◐ | ◐ | Clip/MaxNorm inert at exposure 1; Reinhard/Filmic reshape in-range values, so two entries work and two do not |
| Alpha Blend Low / High | ✗ | ✗ | exact no-ops in Linear — **and in flame modes too** |
| Density Scale | ● | ● | in Linear it is the whole alpha: an opacity slider |
| Spatial Filter, Blur Edges | ✗ | ✗ | run inside the compute pass, which never executes |
| Tone Curve (enable, presets, editor) | ● | ● | gate passes; LUT bound in both tonemap paths |
| Density Levels (histogram, enable, low/high/gamma) | hidden | **shown, inert** | hard-off at `src/app/mod.rs:2893`; the panel only special-cases Escape (`src/ui/tone_mapping.rs:439`), so in Sim the section renders a histogram of the empty flame accumulator |
| Color Mode (Palette/Speed/PathMap) | ✗ | ✗ | neither generator reads it — but choosing PathMap allocates a ~58 MB path buffer, forces a flame shader recompile, and **hides the palette controls both modes need** |
| Palette + rotation/squeeze/log/reverse/size | ● | ● | this *is* their palette control; neither engine panel has one |
| Background Color | ● | ● | both write coverage 0 so the background shows through |
| Speed Blend Factor, Path Style/Capture/Tracking | ✗ | ✗ | chaos-game only |
| All 13 colour effects | ● | ● | run on the post-tonemap texture |
| `sharpen`, `bilateral_blur` | ● | ● | spatial kernels, no density gate |
| `density_blur` | ◐ | ◐ | thresholds on `a`; with `a ∈ {0,1}` it blurs only empty pixels, giving an edge halo rather than smoothing |

Dead in **every** mode, found on the way past: `alpha_blend_low/high`
in Linear, and `TonemapParams.num_transforms`, uploaded and never read
(`shaders/tonemap.wgsl:183-207`).

## 2. Decisions

Taken 2026-09-09, with the reasoning that produced them.

1. **Hybrid visibility.** A section that is entirely inert disappears;
   an individual dead control inside a live section stays visible,
   greyed, with a hover saying why. Rationale: whole panels are already
   hidden, so hiding whole sections is the same rule one level down,
   while a lone greyed slider is where the "why" matters most.
2. **Per-mode layout memory.** Leaving a mode stashes its dock state;
   returning restores it. First entry gives the mode's default layout.
   Today every layout switch rebuilds from code and destroys the user's
   arrangement (`src/ui/workspace.rs:246-258`), which is tolerable only
   because switching is currently buried.
3. **UI layer only; bugs filed, not fixed.** Nothing outside `src/ui/`
   changes except where the feature cannot be built without it. Every
   bug the survey found is recorded in §5 with `file:line`.
4. **Free the inactive engine on switch.** Chosen over keeping both
   resident. See the scope note below.

**Scope conflict, stated rather than resolved quietly.** Decision 4
cannot be done inside `src/ui/` — the renderers are owned by the app's
frame loop (`src/app/mod.rs:412, 421`) and are dropped today only on
device loss. It is therefore phase 5, last and separable, and dropping
it costs nothing else in the plan. Two consequences to accept if it
proceeds: returning to a deep escape zoom rebuilds reference orbits
with a visible wait, and the "show the last frame while re-rendering"
behaviour goes away.

## 3. The design

### 3.1 One mode vocabulary

`MenuState.render_mode_2d: bool` (`src/ui/menu_context.rs:81`) becomes
`render_mode: RenderMode`, and `ViewMenuActions::set_mode_2d/3d`
becomes one `set_mode: Option<RenderMode>`. That single change fixes
the View menu's false reading and lets Fly Mode's gate become
`== ThreeD`. Both are consequences of the feature, not separate fixes.

A new **Mode** menu sits between View and Rendering, built by iterating
`RenderMode::ALL` — four radio rows, the active one checked. All four
route through `switch_render_mode`, which moves out of
`src/ui/escape_panel.rs` to a neutral home (`src/ui/render_mode.rs`;
**not** `src/app/render_mode.rs`, which is an unrelated undo and
animation state machine). The View menu's pair, the compact menu's
duplicate and the View panel's pair are deleted. The Escape and
Simulation panels keep a single call-to-action button when inactive —
"Switch to Simulation mode" — which is a way in, not a fifth toggle.

Coalescing is turned off for `ConfigPath::RenderMode`
(`src/config/manager.rs:170`, already written and commented out) so a
mode change is its own undo entry.

### 3.2 The visibility policy

One module, `src/ui/visibility.rs`, answering questions rather than
holding scattered `matches!`. It is consulted by the panel dispatcher,
by both menus, and by the shared panels:

```rust
pub enum Vis { Show, Grey(&'static str), Hide }

pub fn panel(p: PanelType, m: RenderMode) -> Vis;
pub fn menu_item(i: MenuItem, m: RenderMode) -> Vis;
pub fn control(c: Control, m: RenderMode) -> Vis;
```

The `&'static str` is an i18n key naming the reason, so a greyed
control explains itself and the three wrong uses of the single shared
hint (§1.2) are replaced by per-case text. `RenderMode` is the only
axis built now; the signature leaves room for the skill level that
`docs/projects/ux-improvements.md` §2 proposes and for the compact flag
already threaded through `PanelViewerContext`
(`src/ui/panel_viewer.rs:400`), without building either.

Tables in this module encode §1.2, §1.3 and §1.4 directly. The Window
menu and the compact menu consult `panel()`, so a panel that would open
onto a stub is greyed in the menu instead — which is the user-visible
half of the fix, since today the menu happily offers all 22.

### 3.3 Per-mode layout memory

`Workspace` gains a stash keyed by mode. `apply_layout` stays the only
mutation point, so call sites are untouched. The mode→layout table
already exists as an explicit match (`src/app/mod.rs:1378-1382`) and
becomes the place that consults the stash. The hole to close at the
same time: that function runs only on a load-generation bump, so manual
switches and undo/redo of a mode change never move the layout.

Persisting the stash across sessions needs `serde` on
`DockState<PanelType>` plus a versioned `SystemSettings` field
(`src/storage/settings.rs:25,454`). **Deferred** — session-only memory
delivers the decision; persistence is a separate, larger question about
what a user expects a restarted app to look like.

### 3.4 What the Rendering panel becomes

Max iterations, iterations per thread, burn-in and deterministic RNG
hide in Escape and Simulation. The orbit-cache block stays in Escape
and hides elsewhere. VSync and target FPS are global and stay. The same
policy removes the Iterations-per-Thread submenu and Reset Accumulation
from the Rendering menu in non-flame modes.

## 4. Phases and gates

Each phase leaves the app working and every existing test green.

| # | Builds | Gate |
|---|---|---|
| 1 | `RenderMode` replaces the boolean; `switch_render_mode` moves to `src/ui/render_mode.rs`; all five switch sites route through it; coalescing off | a switch from every mode to every other lands in the right mode with one undo entry; Fly Mode enabled in 3D alone — a table test over all four modes |
| 2 | `src/ui/visibility.rs` with the panel table; dispatcher and both menus consult it; per-case hint text | every `PanelType` × `RenderMode` has an explicit answer (exhaustive match, no `_` arm); no menu row opens onto a stub; the existing layout tests stay green |
| 3 | The Mode menu; View/compact/View-panel toggles deleted | the menu offers exactly `RenderMode::ALL`; a test that no other site writes `ConfigPath::RenderMode` |
| 4 | Control-level policy in Colors, Rendering, Effects per §1.4; dead sections hidden, dead controls greyed | every control in the §1.4 table has an explicit answer; a test asserting the tone-map preset dropdown and Reset Colors are unreachable in non-flame modes |
| 5 | *Separable.* Free the inactive engine on switch | VRAM falls on leaving Escape at high supersample, measured; returning re-renders correctly |

Phase 4 is where the user-visible win is; phases 1–3 are what make it
expressible in one place instead of forty.

## 5. Bugs found, filed not fixed

Per decision 3. None is caused by this work; all were found surveying
for it.

| Bug | Where | Effect |
|---|---|---|
| Simulation viewport navigation is a silent no-op | `src/ui/panel_viewer.rs:477,523,1559` branch on Escape only | dragging the viewport writes `config.zoom`/`pan_x/y`, which the sim ignores, creating undo entries for nothing; the sim's own view is `sim.warp.*` |
| Levels diverge between viewport and export | `src/app/mod.rs:2893` hard-offs for non-flame; `src/renderer/compute_kernel.rs:1886` does not | CLI, thumbnail and video export enable Levels for escape and sim where the app does not. A numerical no-op today only because `sample_density` floors at `1e-6` |
| Transparent PNG export ignores Simulation | `src/app/mod.rs:1843,2282` branch on Escape only | a transparent sim PNG re-tonemaps from the empty flame accumulator |
| Leaving a non-flame mode never restores tone mapping | `src/ui/escape_panel.rs:1487-1493` | the reset is one-way, so a mode round trip silently rewrites a flame's tone mapping |
| PathMap in a non-flame mode costs memory for nothing | tonemap PathMap branch | allocates ~58 MB, forces a flame shader recompile, and hides the palette controls the mode needs. Phase 4 hides the control, which masks but does not fix it |
| `alpha_blend_low/high` are no-ops in Linear | `shaders/tonemap.wgsl:451` vs `:710` | the two mixed values are computed by the same expression. Affects flame modes too |
| `TonemapParams.num_transforms` is never read | `shaders/tonemap.wgsl:183-207` | dead uniform in every mode |
| Workspace Layout ▸ Escape Time / Simulation do not switch mode | `src/ui/menu_bar.rs:345-372` | they look like mode switches and do half the job. Phase 3 may absorb this by removing them |
| Dead locale keys and a zero-byte file | `locales/en.yml`, `src/ui/panels.rs` | `menu.fractal`, `panels.preset_library`, `panels.file_browser`, `panels.settings`, `panels.tone_mapping` |
| Stale comment | `src/scene/transforms.rs:1847-1860` | says Save Online is guarded client-side; `src/api/sync.rs:21-33` now maps both non-flame modes |

## 6. Risks

- **The compact menu is a second copy of everything.** Unless the panel
  list is hoisted into shared data in phase 2, every mode-aware menu
  change is made twice and drifts. Hoisting it is the cheaper path and
  is assumed by the phase-2 gate.
- **Hiding a control hides its value, not its effect.** A config that
  carries a non-default Brightness still carries it in Simulation, and
  the user can no longer see it. The greyed arm exists partly for this;
  where a section is hidden entirely, the value is inert in that mode
  by construction, which is why the hybrid split falls where it does.
- **Per-mode layout memory is session-only**, so a restart still gives
  default layouts. If that reads as the feature not working, the
  persistence deferred in §3.3 is what closes it.
- **Freeing engines trades memory for latency**, and the deep-zoom case
  is the worst one. Phase 5 is separable precisely so this can be
  reversed on measurement.
- **Translations are far behind** — 2,204 English lines against roughly
  270 for each of es, ja and zh-CN. New reason keys are an English-only
  cost in practice, and the greyed arm adds one key per reason.
