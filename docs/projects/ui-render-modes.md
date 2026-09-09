# UI by render mode: one Mode menu, one visibility policy

**Status:** plan of record, 2026-09-09. Branch `ui-modes`, off
`simulation-mode`. Surveyed before planning; every claim below carries
its `file:line` so a reader can argue with the code rather than with
the prose. **All six phases built and gated** (section 4), 2026-09-09,
and nine of the ten bugs found while surveying (section 5) fixed the
same day. The tenth is deliberately left, with its reasoning in the code.

Section 1 describes the code **as it was before this project**, and is
left in the past tense on purpose: it is the evidence the decisions
were made from. What each phase actually changed is recorded under
section 4.

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

Verified control by control against `src/ui/settings.rs`, 2026-09-09.
Almost nothing in this panel survives a non-flame mode.

| Control | `settings.rs` | Esc | Sim |
|---|---|---|---|
| Pause / Resume | `:19` | hide | hide |
| Reset Accumulation | `:24` | hide | hide |
| Max iterations + progress | `:36-62` | hide | hide |
| Iterations per thread | `:73` | hide | hide |
| Advanced ▸ burn-in | `:90` | hide | hide |
| Advanced ▸ orbit cache | `:100-136` | **keep** | hide |
| Advanced ▸ deterministic RNG | `:139` | hide | hide |
| Dynamic blend, fixed blend rate | `:152,168` | hide | hide |
| VSync, target FPS | `:185,201` | **keep** | **keep** |

Pause deserves the note. It looks universal, but both readers already
exclude the non-flame modes: `should_iterate` ands it with
`!is_non_flame` (`src/app/mod.rs:2665`), and the redraw check excludes
Escape and Simulation explicitly (`src/app/mod.rs:1190-1193`). So the
button is inert in both, and the Simulation panel's own Run/Step/Reset
transport is the real control there.

That leaves the panel showing **VSync and target FPS alone in
Simulation**, plus the orbit cache in Escape. Since both are
`SystemSettings` device preferences rather than fractal parameters,
phase 4 should decide whether the panel is worth showing at all in
Simulation or whether those two move to a preferences home. Recorded as
an open question, not settled here.

The same policy removes the Iterations-per-Thread submenu and Reset
Accumulation from the Rendering menu in non-flame modes.

## 4. Phases and gates

Each phase leaves the app working and every existing test green.

| # | Builds | Gate |
|---|---|---|
| 1 | `RenderMode` replaces the boolean; `switch_render_mode` moves to `src/ui/render_mode.rs`; all five switch sites route through it; coalescing off | a switch from every mode to every other lands in the right mode with one undo entry; Fly Mode enabled in 3D alone — a table test over all four modes — **built**, see below |
| 2 | `src/ui/visibility.rs` with the panel table; dispatcher and both menus consult it; per-case hint text | every `PanelType` × `RenderMode` has an explicit answer (exhaustive match, no `_` arm); no menu row opens onto a stub; the existing layout tests stay green — **built**, see below |
| 3 | The Mode menu; the two menu-bar 2D/3D pairs deleted (**the View panel keeps its own**); per-mode layout memory | the menu offers exactly `RenderMode::ALL`; a test that no other site writes `ConfigPath::RenderMode`; switching away and back restores the arrangement — **built**, see below |
| 4 | Control-level policy in Colors, Rendering, Effects per §1.4; dead sections hidden, dead controls greyed | every control in the §1.4 table has an explicit answer; a test asserting the tone-map preset dropdown and Reset Colors are unreachable in non-flame modes — **built**, see below |
| 5 | *Separable.* Free the inactive engine on switch | VRAM falls on leaving Escape at high supersample, measured; returning re-renders correctly — **built**, see below |
| 6 | Update the standing UI documentation (§4.1) | `docs/main/UI.md` describes the mode machinery as built; the doc-links gate stays green — **built**, see below |

Phase 4 is where the user-visible win is; phases 1–3 are what make it
expressible in one place instead of forty.

### 4.1 Phase 6: leave the standing documentation true

This plan is a record of one project. The next person to touch the UI
will read `docs/main/UI.md`, which is where the architecture is
supposed to live, and phase 6 is what stops them finding a description
of a program that no longer exists.

`docs/main/UI.md` is already stale independently of this work: it dates
from the 2025-11-13 dock migration, says the UI "consists of a menu bar
plus 7 dockable panels" when there are 29, lists a menu bar with a
Fractal menu that has never been wired, and documents the render-mode
switch as a 2D/3D toggle inside the Performance window. What phase 6
must add or correct:

- The **menu bar list**, including the Mode menu, and the removal of
  the View menu's 2D/3D pair. Note that the View *panel* keeps its
  2D/3D switch deliberately, since choosing between the flame's two
  projections is a view-level decision.
- **`src/ui/render_mode.rs` as the one way to change mode**, with the
  rule that nothing else may write `ConfigPath::RenderMode`, and why
  (the tone-map rescue on entering a non-flame mode).
- **`src/ui/visibility.rs` as the one answer to "is this available"**,
  the meaning of `Show` / `Grey(reason)` / `Hide`, the rule that no
  panel is ever hidden, and the instruction to add a case there rather
  than a `matches!` in a panel.
- **`WINDOW_MENU` as the single source** of what the two Window menus
  offer, so nobody adds a row to one and not the other.
- **Per-mode layout memory**: `switch_layout` remembers, `apply_layout`
  forces, and which callers should use which.
- The **panel-by-mode table** (§1.2 here) belongs in `UI.md` in some
  form, since it is the thing a UI change most needs to consult.

Also worth a line in `docs/ARCHITECTURE.md`, which routes readers to
the topic docs, and a check of `CLAUDE.md`'s UI Architecture section,
which still describes the panel set loosely. The doc-links gate in
`scripts/release.py check` only verifies that links resolve, not that
prose is true, so this is a read-and-rewrite job rather than something
a test will catch.

### Phase 1 as built, 2026-09-09

`src/ui/render_mode.rs` holds `switch_render_mode` (moved out of
`escape_panel.rs`, where it was an odd tenant), plus `is_non_flame`,
`fly_mode_available` and `mode_label_key`. `MenuState.render_mode_2d`
became `render_mode: RenderMode`, and `ViewMenuActions`' two booleans
became one `set_mode: Option<RenderMode>` — with four modes, a flag per
mode would have been four booleans that must not disagree.

All five switch sites now route through the helper, so the tone-map
rescue stopped being a property of *which control you used*. Fly Mode's
gate is `fly_mode_available`, so it is offered in 3D alone rather than
in everything that is not 2D. Coalescing is off for
`ConfigPath::RenderMode`.

One behaviour added rather than moved: switching to the mode you are
already in now returns early instead of writing an undo entry. A Mode
menu shows a row for the active mode, and clicking it must cost
nothing.

Six tests in the new module, all table-driven over `RenderMode::ALL` so
a fifth mode cannot be added without answering for it. The undo test
was checked against the unfixed code and does fail there — one entry
where two are needed — so it is load-bearing rather than decorative.
1,046 unit tests and all release gates pass.

The four locale keys `mode.two_d`, `mode.three_d`, `mode.escape` and
`mode.simulation` name the modes once, ready for phase 3.

### Phase 2 as built, 2026-09-09

`src/ui/visibility.rs` holds `Vis { Show, Grey(reason_key), Hide }` and
`panel(PanelType, RenderMode) -> Vis`, written with no `_` arm, so a
new panel or a new mode does not compile until someone decides what it
means. The dispatcher's two `matches!` blocks collapse to one consult.

**No panel is ever `Hide`.** Hiding a panel's menu row would make it
unreachable *and* undiscoverable, so an unavailable panel is greyed in
the menu with a hover naming the reason, and its body — if it was open
when you switched — says the same thing. `Hide` stays in the enum for
whole sections inside a panel, which is phase 4.

Four reasons replace the single shared string that used to claim every
panel "edits the flame and is inactive in Escape mode", including in
Simulation and including for the two engine panels, which edit no flame
at all: `visibility.flame_only`, `three_d_only`, `other_engine` and
`makes_a_flame`.

**The panel list is hoisted**, closing the section-6 risk. `WINDOW_MENU`
is the one source of which panels a menu offers and what they are
called; the desktop menu's 22 hand-written rows became a loop over it.
`COMPACT_WINDOW_MENU` keeps the compact submenu's own touch-priority
order but takes its labels from the shared table, and a test asserts it
is a subset. Palette Editor, Palette Library, Path Editor, Random
Generator and Account stay out of the compact menu as before.

**Two deliberate behaviour changes** beyond moving the gate:

- **Random Generator is greyed in Escape and Simulation.** It worked
  there, but everything it produces is a flame, so using it silently
  replaced your work and left the mode. Greying it says so.
- **Solid & Lighting is greyed in 2D**, where it previously opened and
  explained itself. The panel keeps its own guard as a second line of
  defence; the menu now says it before you open it.

Seven tests. The load-bearing one pins the exact set of greyed panels
per non-flame mode, because a panel quietly dropping out of a mode is
the failure this module exists to prevent. Another asserts every reason
key resolves to real text and is longer than ten characters, since
`t!` returns the key itself when it is missing. 1,053 unit tests and
all release gates pass.

**Not changed, deliberately.** The Transforms, Triangle Editor and
Variations panels stay available in Simulation even though
`sim.use_transforms` defaults to off, because those panels are how you
find the feature. Telling the user about the flag is a control-level
job for phase 4. The disagreement noted in section 1.2 — the default
Simulation layout omits Transforms while the gate keeps it usable — is
now the deliberate answer: available, not opened for you.

### Phase 3 as built, 2026-09-09

A top-level **Mode** menu sits between View and Rendering, built by
iterating `RenderMode::ALL` so it cannot fall out of step with the
enum, each row carrying a one-line description on hover. The compact
menu gets the same four rows as its own submenu; without it, a phone
could reach Escape and Simulation only through the panels' own buttons.
`set_mode` moved off `ViewMenuActions` to the top of `MenuActions`,
since it is no longer a View concern.

**The View panel keeps its 2D/3D switch, by request.** Choosing between
the flame's two projections is a view-level decision and belongs beside
the camera. What went is the *duplication in the menu bar*: the View
menu's pair and the compact menu's copy of it, both superseded by the
Mode menu. So the switch exists in two places rather than five, and
they mean different things — the panel chooses a flame's projection,
the menu chooses an engine.

**The engine panels became a way in, not a toggle.** Each shows a
"Switch to … mode" button only when inactive; leaving is the Mode
menu's job. That retires the hardcoded exit-to-3D, which meant turning
Escape on from 2D and off again left you somewhere you had never been.

**Per-mode layout memory landed here**, not in a phase of its own —
the plan's section 3.3 had no phase assigned, and a Mode menu without
it would throw away your arrangement on every switch, which is worse
than the buried toggles it replaces. `Workspace::switch_layout`
stashes the dock state it is leaving and restores the one it is
entering; `apply_layout` stays the forcing version, so Reset Workspace
and the Workspace Layout menu still rebuild. Session-only, as section
3.3 said: persistence still needs `serde` on `DockState`.

**The layout now follows the mode whenever it changes**, closing the
hole from section 1.1. It used to hang off the load generation alone,
so a manual switch out of Escape left the Escape workspace up, and an
undo of a mode change moved nothing. The app compares the mode each
frame instead. The `RenderMode → (layout, panel)` table moved to
`render_mode::layout_for`, so `app::follow_loaded_render_mode` no
longer keeps its own copy.

Four new tests, thirteen in the two modules. The two load-bearing ones
were both checked against unfixed code: a source scan asserting nothing
outside `render_mode.rs` writes `ConfigPath::RenderMode` (planted a
violation in `view.rs`; it was caught and named), and a stash test
asserting a panel opened in Standard survives a trip through the
Simulation layout. Whole-config load paths are exempted from the scan
by name, since they carry a mode in from a file rather than switching.
1,059 unit tests and all release gates pass.

## 5. Bugs found while surveying

Filed unfixed under decision 3, then **worked through on 2026-09-09**.
Nine of the ten are fixed; the tenth is deliberately not, with its
reasoning now in the code rather than in a doc nobody will open.

None was caused by this project. Each was investigated before it was
touched, and three of the ten turned out to affect **more sites than
the original filing named**.

| Bug | Status |
|---|---|
| Simulation viewport navigation is a silent no-op | **Fixed** — refused rather than misdirected |
| Levels diverge between viewport and export | **Fixed** — one predicate, four sites |
| Transparent PNG export ignores Simulation | **Fixed** — three sites, not two |
| Leaving a non-flame mode never restores tone mapping | **Not fixed, deliberately** |
| PathMap in a non-flame mode costs memory for nothing | **Fixed** — 58 MB at 1080p |
| `alpha_blend_low/high` are no-ops in Linear | **Fixed** — disabled, shader untouched |
| `TonemapParams.num_transforms` is never read | **Fixed** — dead parameter, uniform kept |
| Workspace Layout ▸ Escape Time / Simulation | **Fixed** — entries removed |
| Dead locale keys and a zero-byte file | **Fixed** |
| Stale Save Online comment | **Fixed** |

### The ones with something to say

**Simulation viewport navigation.** The tempting fix — wire the drag to
`sim.warp` — is wrong, and this is the one worth recording. The warp is
a **per-step transform of the field**, a velocity applied to the
content, not a camera over it. Setting a pan from a drag would advect
the field forever rather than by the drag's distance, blur it through a
resample every step, do nothing at all while the simulation is paused,
and do nothing in octave mode, where pan and rotation are not applied.
So the gesture is refused: `Control::ViewNavigation` is `Hide` in
Simulation, and drag, wheel, pinch, the arrow keys and the View menu
all consult it. A genuine display-time view for the simulation is a
feature — `params.view.x` is already exactly that, minus pan and minus
user control — and when it exists that arm becomes `Show`.

**Levels.** The flag reached the shader from four places, not the two
filed: `load_config`, `set_transparent_mode`, the live frame loop, and
the resize path, whose gate tested Escape alone so Simulation leaked
through. All four now call `FractalConfig::effective_levels_enabled`.
The claim that this changes no pixels was **verified rather than
argued**: the flag flips from 1 to 0 for all 179 escape and simulation
baselines, and 330 of 330 stayed byte-identical. The predicate lives on
`RenderMode` in `scene`, not in `ui`, because the headless renderer
needs it and `ui` is behind the `web-app` feature — that split is how
the two paths came to disagree in the first place.

**Transparent PNG.** Three sites carried a byte-identical copy of the
same Escape-only test: the export tonemap, the restore after it, and
the wasm viewport export. All three now ask for whichever non-flame
engine is active. This is the one fix that deliberately changes output
— a transparent simulation PNG used to encode the empty flame
accumulator. Written as field access rather than a helper method,
because the call sites already hold `&mut self.flame_renderer` and only
field-level borrows are disjoint from it.

**The alpha-blend pair.** It mixes between a gamma-corrected alpha and
a linear one, but only the logarithmic branch gamma-corrects alpha, so
under Linear the two operands are computed by the same expression. Git
history shows the feature was correct when written, before the tonemap
grew per-mode branches. Making it act under Linear would rewrite every
escape and simulation baseline, so the **control is disabled and the
shader is untouched**. This is why `control` now takes the tone-map
mode as well as the render mode: it is a fact about the mapping, not
about the engine, and it is equally true in a flame on Linear.

**`num_transforms`.** The uniform stays; the dead *parameter* goes. The
field is followed by six others, so removing it shifts their offsets —
and a WGSL-only edit would still validate, because the struct's 16-byte
alignment absorbs the loss, while every field after it silently read
its neighbour's bytes. A `const _: () = assert!(size_of == 144)` now
pins the Rust half, which nothing did before.

### The one left alone

**Leaving a non-flame mode does not restore the flame's tone mapping.**
The restore information is *session* state — the config holds one
tone-mapping triple that two mode families take turns owning, and a
saved escape config carries its Linear mapping legitimately. Keeping it
anywhere else creates a side channel undo cannot see: redo re-applies
the mode batch without going through the switch helper, so memory and
history drift apart after any undo tour. There are sharper edges too,
recorded in full at `switch_render_mode`. Against that, the tone-map
*mode* selector is already disabled in the non-flame modes, so a round
trip cannot lose a chosen mapping — it loses exposure and gamma, and it
is visible the moment it happens. The doc comment now carries the whole
argument, including the shape a fix would have to take.

### Found while fixing, still open

- **WASM custom-size export has an escape-only generator**
  (`src/app/mod.rs:2068` and the block below it), so a custom-size
  Simulation export on the web encodes the empty accumulator. Desktop
  custom-size is fine — it routes through `render.rs`, which handles
  both engines. Same shape as the transparent-export bug, different
  code path.
- **The DensityVisualization branch's alpha blend is accidentally
  meaningful**: it mixes an exposure-scaled alpha with a
  density-scale-scaled one, which is not what the control describes.
  Left alone as the smaller instance of the same drift.

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


### Phase 4 as built, 2026-09-09

`visibility.rs` gained `Control` and `control(Control, RenderMode)`,
grouped by shared fate rather than one variant per widget — the answer
is the same for every slider the logarithmic branch alone reads, and a
variant each would be nine ways to get one decision wrong. A `gated`
helper draws a body normally, disabled with a hover, or not at all.

**Hidden** in both non-flame modes: the chaos-game controls, the
tone-map preset dropdown, Reset Colors, the spatial filter, the density
levels section, and the colour-mode selector. **Greyed**: the tone-map
mode selector, the four logarithmic-only sliders, and the alpha-blend
pair. **Kept**: exposure, gamma, saturation, hue shift, density scale,
the tone curve, the whole palette group, background, and every effect.

Three consequences worth naming:

- **The Rendering menu is hidden entirely** in Escape and Simulation.
  Every item configures the chaos game, and `Reset to Defaults` was
  measured to reset exactly those six parameters, so nothing was left
  to show.
- **The palette controls stopped depending on the colour mode** in the
  non-flame modes. They were gated on it being Palette or Speed, so a
  config sitting on PathMap hid the palette picker that escape and
  simulation genuinely use — while the mode itself did nothing.
- **The density levels section was drawn in Simulation** over the empty
  flame accumulator, because the panel special-cased Escape alone while
  the frame loop hard-offs both. Now neither draws it.

**A pre-existing bug fell out.** The Escape branch of the levels
section printed `t!("tonemap.levels_escape_hint")`, and that key does
not exist in `locales/en.yml` — so escape mode showed the raw key
string. The branch is gone, so the bug is moot rather than fixed.

Three new tests, ten in the module: every control answered in every
mode, the two flame modes offering everything but the orbit cache, and
the tone-map presets unreachable where they would black the picture.
1,062 unit tests and all release gates pass.

**Settled, 2026-09-09.** In Simulation the Rendering panel shows VSync
and the frame cap alone, and that is fine as it stands. The panel is
not moved and those two are not relocated.

### Phase 5 as built, 2026-09-09

`render_mode::keeps_escape_engine` and `keeps_sim_engine` say which
engine a mode needs resident; `App::release_inactive_engines` drops the
other on every mode change, hanging off the same frame-loop comparison
phase 3 added. It mirrors what a synchronous high-res export has always
done to the escape renderer, and for the same reason.

`EscapeRenderer::resident_bytes` walks the same resource list `destroy`
frees, so what it reports is what the switch gives back. Measured on a
fresh renderer: **16.0 bytes per pixel**, identical at 512² and 2048²,
which extrapolates to **506 MB for a 1080p viewport at 4x
antialiasing**. That is a floor, not a ceiling — a fresh renderer holds
only its output texture, and a working one adds the accumulation pair,
the reference orbits and the bilinear-approximation tables.

**The two engines are not symmetric, and the difference is the one
thing to know here.** The escape renderer rebuilds itself from the
config, so returning costs only the re-render — at a deep zoom, that
means rebuilding reference orbits and a visible wait. The simulation's
grid **is** its state, so freeing it means returning restarts from the
seed at step 0. A run that took 200,000 steps to get where it was does
not come back. That follows from decision 4 as taken, and it is called
out here rather than buried because the escape framing that decision
was made under did not imply it. Making the release escape-only is a
two-line change to `keeps_sim_engine` if the trade reads differently in
use.

Two tests: each engine resident in its own mode alone and never both,
and the per-pixel measurement above. 1,064 unit tests and all release
gates pass.

### Phase 6 as built, 2026-09-09

`docs/main/UI.md` was rewritten where it had gone false and gained a
**Render modes and the UI** section covering the mode vocabulary, the
single writer of `ConfigPath::RenderMode`, the visibility policy, the
shared window-menu table, layout memory and engine lifetimes.

What it had been claiming: 7 dockable panels (there are 29), a Fractal
menu that has never been wired, the render-mode switch as a 2D/3D
toggle inside the Performance window, and a Performance window with a
preset dropdown and camera controls it does not have. Five of its six
per-panel sections pointed at `src/ui/mod.rs::render_ui()` for code
that moved to its own file at the 2025-11-13 dock migration. Those are
replaced by a panel-to-file table taken from the dispatch match, plus
prose for the three panels with mechanics worth knowing.

The "Add New Window" recipe described the pre-dock design — a show/hide
boolean and a menu checkbox. It is now "Add a panel", six steps, with
the `visibility::panel` case and the `WINDOW_MENU` row among them, and
an explicit "do not write `matches!(config.render_mode, ...)` inside a
panel". That instruction is the one most likely to keep this design
intact.

`docs/ARCHITECTURE.md` and `CLAUDE.md` got the same corrections in
miniature, both pointing here.

Deliberately left alone: the UiResponse, input-handling and
delta-state sections, which were not surveyed for this project and are
not this project's to vouch for.
