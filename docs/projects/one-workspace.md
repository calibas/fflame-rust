# One workspace, every panel in every mode

Decided 2026-10-22. It replaces the render-mode UI policy of
[ui-render-modes.md](../archive/projects/ui-render-modes.md) (panels
greyed by mode, a workspace per mode, the Mode menu) with one workspace
in which every panel works or says why not.

The user's words: "I'm not sure I want to disable panels based upon the
mode, it's kinda confusing. ... For a new user, they wouldn't
necessarily know how they have to switch to escape-time first. ... I
want to show every panel in every mode, instead of disabling them like
we do now. If the panel doesn't do anything in the current mode, it
should tell the user. I also don't really want to close panels and
switch workspaces when changing modes."

## Decisions

1. **One workspace.** Transforms, Escape and Simulation are tabs in
   the left dock. A mode change brings the mode's tab to the front, and
   opens it if it was closed; nothing closes, nothing is swapped. The
   Escape Fractal panel is renamed **Escape**.
2. **A tab does not switch the mode.** Looking at a panel changes
   nothing.
3. **Every panel draws its contents in every mode, editable.** A panel
   that does not act on the picture says so at its top, clearly: a
   banner naming the mode it is not used in, why, and a button into the
   mode it is for.
4. **Choosing enters the mode; tweaking does not.** An action that picks
   something for another engine takes the config into that engine's
   mode, in the same undo step. "Use as Coloring" from a flame enters
   Escape with the escape formula the config already has (Mandelbrot by
   default).
5. **The mode switcher is in the menu bar**, and the Mode menu goes.

Then, the same day, after a first look:

6. **The mode is a button showing the current mode**; clicking it lists
   the four.
7. **Flames, Escape Time and Simulation return as workspace layouts**,
   picked from Window ▸ Workspace Layout and never applied by a mode
   change. Flames is Standard without the Escape and Simulation panels.
   Escape Time and Simulation are Standard without the other engines'
   editors, Transforms, the Triangle Editor and Rendering, with the
   Lighting panel where the Triangle Editor was. The Lighting & Material
   panel is renamed **Lighting**.

## The design

### The workspace (`src/ui/workspace.rs`)

- `WorkspaceLayout` loses the per-layout stash (`switch_layout`,
  `stashed`), and no layout follows the mode. Standard (the default) and
  Animation carry the three main panels as left-dock tabs: Transforms,
  Escape, Simulation. The right dock is unchanged: Colors, View, Triangle
  Editor, Rendering, History. Textures stays in the Window menu.
- Flames, Escape Time and Simulation (decision 7) are presets in the
  Workspace Layout menu: Transforms with Standard's right dock; Escape,
  or Simulation, with Colors, View, Lighting, History. Reset Workspace
  rebuilds the layout that is up.
- `Workspace::show_main_panel(panel, ctx)`: the panel's tab to the front
  where it is; if closed, a tab beside another main panel, or a floating
  window when none is open (the Scripting layout). In the compact
  layout: a tab that exists comes to the front; Escape and Simulation
  open docked as before; a flame mode opens nothing (a phone's screen is
  the picture).
- `render_mode::main_panel_for(mode)` names the panel: Transforms for
  2D and 3D, Escape, Simulation. It replaces `layout_for` and
  `workspace_for`.
- The app calls `show_main_panel` when the mode changes (a switch, a
  load, an undo: `last_render_mode`, as now) and on the first frame
  after a layout is built (`Workspace::needs_main_panel`, as
  `needs_startup_dock_widths`), so Reset Workspace and a boot in Escape
  open on the right tab. A load that does not change the mode moves
  nothing.
- `UiResponse::workspace_layout_requested` goes: only the Escape and
  Simulation panels' enter buttons used it.

### Panels (`src/ui/visibility.rs`, `src/ui/panel_viewer.rs`)

`visibility::panel` answers whether a panel acts on the picture, and
when not, why and which modes it is for:

```rust
pub enum PanelUse { Active, Inactive { reason: &'static str, enter: &'static [RenderMode] } }
pub fn panel(p: PanelType, m: RenderMode, solid: Solid) -> PanelUse;
```

- `PanelViewer::render_panel` draws an `Inactive` panel's banner above
  its contents (no more early return): "Not used in *mode* mode", the
  reason, and a Switch to *mode* button per entry of `enter`.
- Both Window menus enable every row; an inactive panel's row shows its
  reason on hover.
- What stays inactive, and where it leads:

| Panel | Inactive in | Leads to |
|---|---|---|
| Xaos Editor, Subflames, Paths | Escape, Simulation | 2D, 3D |
| Escape | 2D, 3D, Simulation | Escape |
| Simulation | 2D, 3D, Escape | Simulation |
| Textures | 2D, 3D, Simulation | Escape |
| Hybrid Designer | 2D, 3D, Simulation | Escape |
| Lighting | no surface (`Solid::of`) | 3D from 2D; otherwise none (the reason says to pick a solid or turn on a terrain) |

- Active everywhere now: Random Generator (a generated flame loads with
  its own mode already) and the Formula Editor (it makes formulas,
  colourings and solids for Escape and variations for the flame).
- Inside the panels:
  - the Escape and Simulation panels lose their enter-button blocks and
    draw their contents in any mode; the Escape panel leaves out its
    render-progress line, the Simulation panel its transport and step
    readout (no run exists outside the mode);
  - the Formula Editor offers every Use in every mode (Use as Variation
    in the flame modes and Simulation, Use as Lens in Escape);
  - the Textures panel lets a texture be picked, and shows its overlay
    and bump controls, in any mode.
- Control-level gating (`visibility::control`: the tone-map presets,
  Log-only sliders and the rest) is unchanged.

### Choosing enters the mode (`src/ui/render_mode.rs`)

`render_mode::choose(cm, changes, description, mode)`: the changes, the
mode, and what entering implies -- the 3D look into a lit 3D view
(`look_3d`), else the tone-map rescue (`entry_tone_mapping`) -- as one
undo step. In the mode already, it is `apply_with_look`.
`switch_render_mode` becomes `choose` with no changes. The sites:

- Formula Editor: Use as Formula, Use as Coloring, Use as Solid.
- Escape panel: the formula picker (and the preset it lands on), the
  Preset picker, the colouring pickers.
- Simulation panel: the model picker, its presets, the layered presets.
- Hybrid Designer: Use and Use Preset. A structural edit of a hybrid in
  use re-applies it without entering (tweaking).
- Textures: picking a texture (a structural snapshot; the mode change
  and its implications applied to it detached, `entering_changes`).

### Menus

- The menu bar draws the mode as a button after Help, named by the
  current mode and outlined as Fly Mode is; its menu lists the four, the
  active one selected, each with its hover description. The Mode menu
  goes.
- The compact menu's first row is the same: "Mode: *current*", a
  submenu of the four.
- The Rendering menu is always present; outside the flame modes its
  rows are disabled with the reason on hover (it configures the chaos
  game). A menu that came and went moved everything after it.

## Order of work

1. `render_mode`: `choose`, `entering_changes`, `main_panel_for`;
   `switch_render_mode` on `choose`; `layout_for`/`workspace_for` gone.
2. The workspace: the layouts, `show_main_panel`, the app's follow,
   `workspace_layout_requested` gone.
3. The panels: `PanelUse`, the banner, the Window menus, each panel's
   contents in every mode.
4. Choosing enters: the sites above.
5. The menu bar switcher, the compact menu's, the Rendering menu; the
   panel renamed Escape.
6. Docs: UI.md (layouts, menus, render modes and the UI), ARCHITECTURE.md,
   CLAUDE.md, and the Mode menu's mentions in ESCAPE.md, SCRIPTING.md and
   TONE-MAPPING.md.

Gates: `cargo test --release --lib` over `ui::` and `app::`, the wasm and
single-engine checks (the panels sit behind `engine-escape` /
`engine-sim`), and the app run by hand for the banner and the tabs.
