# Camera unification: one 2D view, one 3D camera, one fly mode

Branch `camera-unification`. Status: **plan, decided** -- written
2026-10-08, the user's answers recorded 2026-10-09 (§3). Nothing is
built yet.

## 0. What is being asked for

The user, after the heightfield-3d merge:

> I want to merge our 3d camera systems and the View panel as much as
> possible. There's at least 3 different places to adjust camera pitch
> and panning. Multiple places for fog, FOV, etc... All the view options
> should be in the View Panel, camera controls and fog sliders should be
> in the same place, even if they don't work exactly the same in the
> different renderers. I also want to review what's missing, can we add
> things like material settings to all the views now that it's
> available in 3D terrain? Camera X/Y/Z for terrain? And I want to
> review the controls, they should work the same way. Drag to pan,
> alt-drag for pitch/yaw, mouse wheel for zoom. This is also where we'd
> add fly mode to our 3d terrain.

And on starting it:

> I don't just want fly mode for terrain, I want the same
> quaternion-camera-based fly mode for anything 3D. I'm trying to unify
> camera controls and UI. One version for 2D, one for 3D.
>
> Note, for simulation, if there's a periodic boundary and a fixed grid
> size, we can do things like tiling and rotations in 2D almost for free
> without affecting the field itself. So you could pan/zoom/rotate 2d
> simulations as well.

So, four things:
1. **One View panel with two layouts:** a 2D one and a 3D one, used by
   every render mode. Camera, projection and fog controls live there,
   even where a renderer reads them differently.
2. **What's missing:** materials for every 3D view, and a camera
   position (X/Y/Z) for the terrains.
3. **One set of controls:** drag pans, Alt+drag turns (pitch/yaw), the
   wheel zooms -- in every mode -- and one quaternion fly mode for every
   3D camera.
4. **A 2D view for simulations:** pan, zoom and rotate where the picture
   is drawn, tiling a periodic field, without touching the field.

## 1. Where things stand

Surveyed 2026-10-08 (three read-only inventories: the UI's controls,
the viewport's input, and each camera's fields and math). The facts the
design rests on:

### 1a. Seven cameras, four conventions

| | stored as | zoom / distance | angles | screen roll |
|---|---|---|---|---|
| **Flame 2D** | `pan_x/pan_y` (f64), `zoom` (linear), `rotation` | `zoom`: the short side spans `4/zoom` | -- | `rotation` |
| **Flame 3D** | the 2D fields, plus `camera_rotation_x/y` (pitch, yaw), `camera_bank`, `camera_x/y/z` (moves the camera rigidly; the projection's viewpoint sits `1/persp` behind it), `perspective_strength` | `zoom` is a 2D scale after projection | pitch 0 = looking straight down (JWF) | `rotation`, after projection |
| **Escape 2D** | `center_re/im` (exact decimal strings), `zoom_log2` (f64), `rotation` | `zoom_log2`: the HEIGHT spans `4/2^z`; Im up | -- | `rotation` |
| **Mode D solid** | `cam_target_x/y/z` (decimal strings; empty = the attractor's centre), `cam_pitch/yaw/bank/fov`, `zoom_log2`, `rotation` | distance `3.2 r / 2^zoom_log2` | pitch 0 = horizon; yaw 0 looks -x | `rotation`, inside the matrix |
| **Escape terrain** | **no fields of its own**: target = `center_re/im` at the ground height, angles = mode D's `cam_*`, `zoom_log2` = the world unit | eye fixed 1.3 view widths from the target; the wheel is `zoom_log2` | pitch 0 = horizon; yaw 0 looks up the picture | none: `rotation` is added to the **heading** |
| **Sim 2D** | none | none (the Octaves warp's display magnification only) | -- | none |
| **Sim terrain** | `target_x/y` (f32 grid fractions), `cam_pitch/yaw/bank/fov`, `cam_distance` (grid widths) | `cam_distance` | as the escape terrain | none |

- **One matrix underneath.** Every 3D camera is the flame's Euler chain
  `Rz(roll)·Rx(pitch)·Ry(bank)·Rz(−yaw)` (`build_camera_matrix` in
  `shaders/core/utilities.wgsl`), re-zeroed: `solid_frame`
  (`src/escape/ifs.rs`) shifts pitch by π/2 and yaw by π/2 or π. So the
  conventions differ by constants, not by kind.
- **Two projections.** Flames divide by `1 − persp·z` (Apophysis's): a
  pinhole whose viewpoint sits `1/persp` behind the stored camera
  position, the stored position's plane at unit magnification, set by
  perspective strength rather than a field of view. Mode D and both
  terrains are pinholes with a vertical FOV.
- **Precision.** The escape centre and mode D's target are exact
  decimals, moved by fixed-point adds (`FixedPoint::decimal_add_floatexp`)
  so a pan works at any deep zoom. Flame pan is f64. The sim terrain's
  target is an f32 fraction.

### 1b. The controls today

| | drag | Alt+drag | Shift / right drag | wheel | touch |
|---|---|---|---|---|---|
| Flame 2D | pan | **nothing** (and no pan) | nothing | zoom (to the cursor when zooming in) | pan, pinch |
| Flame 3D | 2D pan | look (fly math) | nothing | 2D zoom | pan, pinch |
| Flame 3D, fly mode | look | look | look | zoom (centred) | look; pinch still pans |
| Escape 2D | pan (fixed point) | **nothing** | nothing | zoom to the cursor (fixed point) | pan; pinch in **f64**, zoom clamped at 300 not 1e8 |
| Mode D | target slides | **nothing**; **no orbit at all**, angles are sliders only | nothing | dolly toward the cursor | pan; pinch writes the plane centre, which a solid ignores |
| Escape terrain | **orbit** | orbit (Alt ignored) | pan | dolly | **pan** (not orbit); pinch uses the plane mapping |
| Sim 2D | refused | nothing | refused | refused | refused |
| Sim terrain | **orbit** | orbit | pan | dolly (another curve) | **nothing** |

- **Fly mode** is flame 3D only (`fly_mode_available`, `toggle_fly_mode`,
  and the look delta dropped outside ThreeD in `app/mod.rs`). Its
  FreeLook already turns **without gimbal lock**: each mouse event
  composes a true 3D rotation (a 3x3 matrix, the same math a quaternion
  does) onto the view. What remains Euler is the storage: the result is
  decomposed back into the stored angles (`to_euler_near`), so the
  sliders can jump near straight up or down while the view moves
  smoothly. It also ignores `camera_bank`, and it turns about the stored
  camera position, not the projection's viewpoint (see C4).
- **Orbit** on the terrains is its own code: 0.005 rad a pixel,
  hard-coded, no invert-Y, pitch clamped to [0.02, π/2).
- **History** names differ by mode for the same gesture (`pan`,
  `pan_view`, `escape_cam_target_x`, `wheel_zoom`, `zoom`,
  `escape_zoom`), so some drags coalesce and some don't.
- The **tab-bar strip** above the viewport handles only drag and wheel,
  through the flame/escape helpers: over a terrain it pans instead of
  orbiting, over a sim terrain it does nothing.

### 1c. Where the controls live

- **The View panel** (`src/ui/view.rs`) is flame-only: greyed in escape
  and simulation. It holds the flame camera, fly settings, depth of
  field, fog, depth density, far fade -- and two things that are not
  view at all (Preserve Z, post symmetry).
- **Escape** keeps its centre, zoom (shown log10), rotation (drawn below
  the Reference and Diagnostics sections, away from the View heading),
  lens, antialiasing, mode D's camera, and the terrain's camera, far and
  haze in the Escape panel. The terrain's camera block is mode D's
  (`show_solid_camera`).
- **Simulation** keeps the terrain camera, target, distance, far and
  haze in the Simulation panel; it has two sliders labelled "Distance".
- **Solid Lighting** holds the lights and the lit tier's shading for all
  four 3D views, but no workspace docks it, its hint says "3D render
  mode only", and its lights mean camera space for flames and mode D,
  world space for the terrains.
- **Path tracing** (tier, samples, bounces, sky, gloss, roughness, glow,
  aperture, focus) is drawn inside the Escape panel (shared by mode D
  and the escape terrain) and again inside the Simulation panel (its own
  copy).
- **Fog** is in four vocabularies: flame fog and far fade (View panel);
  mode D reads the flame's fog but has **no control for it in Escape**;
  the terrains derive fog from Far and Haze.
- **Depth of field** is two systems: the flame's focus/blur, and the path
  tracer's aperture/focus.

### 1d. Defects found, to fix along the way

*Status after P8:* all fixed (P1: the View menu and compact Reset, the
escape pinch, the right-button readback; P4: mode D's fog and the Solid
Lighting hint and gating; P6 and P8: the stuck fly keys, on focus loss
and on a release a text field takes; P8: the stale comments) except
**mode D's lens sign**, still to verify with an asymmetric lens.

- The **View menu** is shown in Escape, but Reset/Zoom In/Zoom Out write
  the flame's fields: they do nothing there. The compact menu's Reset is
  not gated at all and also appears in Simulation.
- **Mode D's fog** can't be edited in Escape (its only control is in the
  greyed View panel).
- The **escape pinch** works in f64 and clamps zoom at 300.
- **Holding the right button** over the viewport runs a blocking path
  readback every frame in every mode, though only PathMap draws it.
- **Fly keys stick** if the window loses focus or a text field takes a
  key release.
- **Mode D's lens** may apply with the vertical sign the planar paths
  don't (the lens runs on y-down pixel coordinates before the up-vector
  is negated). To verify with an asymmetric lens before relying on it.
- The **Solid Lighting** hint and grey reason are wrong outside flames,
  and its gating is an inline `matches!` against the visibility rule.
- Stale comments: "identity in 3D" pan frames (`view.rs`,
  `panel_viewer.rs`), fly settings "in Settings → Preferences", the
  fly toggle "F key" (it's F2).

## 2. The shape of it

### C1. Storage stays; each renderer gets a camera adapter

The cameras' fields stay as they are. The saved files, the JWF/Apophysis
XML mapping, every animation track, the script API and the undo history
all address those fields, and the escape cameras' decimal strings are
what deep zoom runs on.

What changes is that nothing else reads them directly. Each renderer
gets an **adapter** -- a small, pure, tested layer that answers the
camera questions in one vocabulary and turns every camera operation into
a ConfigPath batch:

- `View2d` for flame 2D, escape 2D and simulation 2D:
  - read: the centre (as an exact decimal and as f64), the zoom (as log2
    magnification), the rotation;
  - write: `pan_by_pixels(dx, dy, viewport)`, `zoom_at(factor, cursor)`,
    `rotate_by(angle, about)`, `set_centre(...)`, `reset()`, `frame()`.
- `Camera3d` for flame 3D, mode D and both terrains:
  - read: the orientation (a quaternion), the eye and target (in the
    renderer's world units, exact where the renderer is exact), the
    distance, the projection (`Pinhole { fov }` or `Apophysis { persp }`);
  - write: `pan(dx, dy)`, `orbit(q)` about the target, `look(q)` about the
    eye, `dolly(factor)` / `zoom(factor)`, `fly(delta)`, `set_eye`,
    `set_target`, `reset()`, `frame()`.

Every gesture, panel control, menu row and the fly mode go through
these. A batch is still a ConfigPath batch, so undo, animation and
scripts see nothing new.

**Decided** (§3): keep the fields. The user likes a shared camera struct
but it is not the point -- combining the features and the UI is. The
adapters are exactly where a shared struct would plug in later.

### C2. Orientation is a quaternion at runtime

The flame's FreeLook is already gimbal-free in how it turns (§1b). This
generalises it: the same turning for every 3D camera, with bank
included, and the slider jumps confined to where they can't be avoided.
What the user wants from it is no gimbal lock in free-look.

- A `Quat` type (hand-written, a few dozen lines -- no new dependency)
  composes every rotation: orbit, look, FPS-style world-up turns.
- Each adapter converts its stored angles to a quaternion and back. Its
  conversion back picks the Euler solution nearest the previous angles
  (the existing `to_euler_near` logic, generalised to all four angle
  slots, bank included), so a slider never jumps unless the camera
  passes within a hair of straight up or down -- where the stored angles
  are ambiguous whatever we do.
- While a fly session or a drag runs, the **quaternion is the source of
  truth**: held in the app between events and written through each time,
  so repeated round trips can't accumulate drift. Anything that writes
  the angles from outside (a slider, undo, a track, a script) ends the
  session's hold.
- Stored angles stay because the files, tracks and JWF exchange speak
  them (decided, §3).

### C3. One set of controls

| | 2D | 3D | 3D, fly mode |
|---|---|---|---|
| Drag | pan | pan (in the screen plane; on a terrain, along the ground) | look (turn about the eye) |
| Alt+drag | rotate the view (roll) | orbit the target (pitch/yaw) | look |
| Right drag | as Alt+drag | as Alt+drag | look |
| Wheel | zoom to the cursor, in and out | dolly toward the cursor (a flame: its 2D zoom) | change fly speed |
| Shift | finer steps | finer steps | sprint |
| W A S D Q E | -- | -- | fly |
| Arrows, + / − | pan, zoom | pan, dolly | -- |
| One finger | pan | pan | look |
| One finger, Turn toggle on (touch's Alt) | rotate | orbit | look |
| Long press, then drag (touch's right button) | rotate | orbit | look |
| Pinch / twist | zoom / rotate | dolly / orbit | -- |

- **Every mode gets every row.** Mode D gains orbit; both terrains swap
  (drag now pans, Alt+drag orbits -- today it's the other way round);
  simulations gain the 2D row; the tab-bar strip runs the same handler
  as the viewport.
- **One orbit and look implementation**, using the fly settings
  (sensitivity, invert Y, FreeLook or FPS) for every 3D camera. FPS keeps
  the horizon level (turns about world up); FreeLook turns about the
  screen's axes and can roll.
- **One history name per gesture** (`pan_view`, `orbit_camera`,
  `look_camera`, `zoom_view`, `fly_camera`), all coalescing.
- **Per-renderer meaning is allowed**, as the user said: a flame's wheel
  stays its 2D zoom (JWF's meaning) where a pinhole camera dollies; a
  terrain's pan runs along the ground.

### C4. One fly mode for every 3D camera

Fly mode works on the adapter's eye and quaternion, so it is one piece of
code. What each renderer needs underneath:

- **Flame 3D.** `camera_x/y/z` moves the whole camera rigidly, so the
  camera already goes anywhere. But the projection divides by
  `1 − persp·z`: a pinhole whose viewpoint sits `1/persp` behind the
  stored point (10 units at perspective 0.1), with the stored point's
  plane at unit magnification. Today's look rotates about the stored
  point, so the viewpoint swings round it, reading on screen as a short
  orbit. Look about the eye keeps the viewpoint still and moves the
  stored point instead; the field and its meaning don't change. An
  orthographic flame has no viewpoint; its look stays about the stored
  point.
- **Mode D.** Flying moves the target (a fixed-point add, so it works at
  any zoom) with the eye; looking moves the target around the eye at the
  same distance. Speed scales with the distance to the target, so a
  flight feels the same at every zoom (`free-camera-movement.md` left
  this open).
- **Escape terrain.** Its camera is defined by a target ON the ground
  (the view centre at height H), the eye 1.3 world units from it, and the
  world unit set by the zoom -- so it can never look above the horizon,
  and climbing means zooming out. **Decided: the 3D camera can look
  anywhere** (§3), so the terrain gets an eye-based camera: the eye's
  ground point (exact decimals, as the centre is today) and height, the
  orientation, and the target derived. It can then look at the sky the
  far field (T2e) already draws.
  - What has to hold: the sections stay sharp near the eye. Today the
    zoom sets both the world unit and the sections' resolution band
    (`canonical_anchor`). With an eye-based camera, the eye's height
    above the ground sets that scale -- a phase with its own
    measurement, before/after renders of the shipped terrains.
  - Saved terrains keep their pictures: a terrain saved with the old
    camera converts to the eye-based one on load, to the same view.
- **Sim terrain.** No deep zoom, so f32 is enough; the target is pinned
  at half the height field's height, so a free look needs one new field,
  the target's height.
- Fly mode is offered wherever the 3D layout is (`visibility.rs` decides,
  not `matches!` in `toggle_fly_mode`); the stuck-key defect is fixed
  with it (release all on focus loss).

### C5. The View panel: a 2D layout and a 3D layout

One panel, shown in every mode. Its sections ask `visibility.rs` (new
`Control` variants), never the render mode directly.

**2D layout** (flame 2D, escape 2D -- planar, fields and planar IFS --
and simulation 2D):
- **Camera:** centre X/Y (escape: Re/Im as exact decimals), zoom shown
  as a magnification on a log scale (flame and escape alike), rotation,
  arrow pad, Reset View, and Frame where a mode has one (escape IFS:
  Frame Attractor; simulation: Fit Grid).
- **Lens** (escape): moved from the Escape panel.
- **Image:** antialiasing (escape supersample and downsample; simulation
  fit, upscale and downscale; tiling, for a periodic simulation).
- **Deep zoom** (flame): Focused Rendering and auto exposure, as today.

**3D layout** (flame 3D, mode D, escape terrain, simulation terrain):
- **Camera:** Position X/Y/Z (the eye -- this is the terrains' "Camera
  X/Y/Z"), Target X/Y/Z where the renderer has one, distance, pitch, yaw,
  roll, FOV (or Perspective for a flame), Reset, Frame.
- **Fly mode:** the toggle and its settings, for every 3D camera.
- **Atmosphere:** fog (the flame's, now also editable for mode D), far
  and haze (terrains, one label each -- "Far" and "Haze", not two
  "Distance"s), far density fade (flame).
- **Depth of field:** the flame's focus and blur; the path tracer's
  aperture and focus where it runs.

**Lighting and material** (decided, §3: in the View panel for now; its
own panel later if it grows complicated enough):
- the lights and the lit tier's shading (today's Solid Lighting panel,
  folded in -- its `PanelType` stays so saved layouts still load, and
  opens the View panel);
- the render tier, samples, bounces, denoise, sky and the path tracer's
  material (gloss, roughness, glow), moved from the Escape and
  Simulation panels.

**Stays in the View panel** (decided, §3): Preserve Z and post symmetry.
**Moved out of the Escape and Simulation panels:** every camera, lens,
fog/far/haze, lighting, material and antialiasing control.

**Angles in the UI** (decided, §3): degrees, never radians, on a −180 to
180 slider that takes a typed value beyond the range. That covers every
camera angle, the 2D rotations (escape's is an unbounded drag value
today), the simulation warp's rotation per step (raw radians today),
and the animation track editor's values for angle tracks (raw radians
today). Stored values stay radians, so files, tracks and scripts are
untouched.
- Each camera keeps its own zero (a flame's pitch 0 looks straight
  down, as in JWildfire; mode D's and the terrains' pitch 0 is the
  horizon), so the panel and the track editor show the same number the
  file stores. Each slider's tooltip says where its zero is.

### C6. A 2D view for simulations

- **Display only.** The resolve already maps each output pixel to a grid
  position (`gf0` in `COLOR_TEMPLATE`, `src/sim/assembler.rs`), and its
  only transform today is the Octaves magnification. A view is one more
  transform there -- `gf = centre + R(−θ)·(gf0 − c)/zoom` -- and the field
  is never touched, so no reseed and no re-run.
- **Tiling** where the boundary is Periodic: wrap the index instead of
  clamping, so a zoomed-out view repeats the torus seamlessly. Under any
  other boundary, outside the grid is empty (coverage 0, the background).
- **New config:** `sim.view` (centre in cells, zoom, rotation, tile),
  skip-if-default, so no saved simulation changes; ConfigPaths; tracks;
  `SimRerender` (not `SimReseed`). `Control::ViewNavigation` becomes Show
  for simulations, and the arrow and +/− keys stop being swallowed.
- **Every grid** (decided, §3): pan, zoom and rotate work on any grid,
  window-bound or fixed; a non-periodic grid simply shows its edges, and
  a periodic one tiles.

### C7. Materials for every 3D view

Today two material models describe the same surface depending on the
tier:
- the **lit tiers** (flame splats' shade pass, mode D's walk, the
  terrains' walk) use Solid Lighting's ambient, diffuse, specular and
  shininess;
- the **path tracer** (mode D and the terrains) uses gloss, roughness
  and glow;
- **flames** have no path tracer, so no gloss, roughness or glow at all.

The goal is one material per picture, which both tiers read, so
switching tier changes the light's accuracy, not the material. That
changes pictures (a lit tier would start reading gloss and roughness, or
the path tracer specular and shininess), so it gets its own design note,
with before/after renders, after the camera work (§4, P6). Until then
the panel shows both sets in one place and labels which tier reads
which.

## 3. Decisions (the user, 2026-10-09)

1. **Storage (C1): keep each camera's fields**, behind adapters. A
   shared struct is liked but not the point; combining the features and
   the UI is.
2. **Orientation (C2): a runtime quaternion,** written back as the
   stored angles. What matters is no gimbal lock in free-look. (The
   flame's FreeLook already turns without it -- §1b -- so this carries
   that to every 3D camera, bank included.)
3. **Controls (C3): as proposed** -- right drag as Alt+drag, Alt+drag
   rotating a 2D view, the wheel setting the fly speed in fly mode.
4. **Fly look (C4): about the eye,** for every camera; Alt+drag orbits
   the target. (The flame's Camera X/Y/Z does move the camera freely;
   only its turning pivots on the stored point -- C4.)
5. **The 3D camera can look anywhere**, the escape terrain included: an
   eye-based terrain camera (C4).
6. **The simulation view works on every grid**: pan, zoom and rotate;
   a non-periodic grid shows its edges, a periodic one tiles (C6).
7. **Lighting and material go in the View panel for now**; their own
   panel later, if it grows complicated enough (C5).
8. **Preserve Z and post symmetry stay** in the View panel.
9. **Angles show in degrees**, never radians: a −180 to 180 slider that
   takes a typed value beyond it (C5). Each camera keeps its own zero, so
   the panel shows the number the file stores (decided here, not by the
   user -- a converted zero would make the panel and the track editor
   disagree with the file).

## 4. Phases and gates

Each phase is a commit (or a few) on `camera-unification`. Gates for
every phase: `python scripts/release.py check`, the phase's new tests,
and the visual suite -- **pictures unchanged** unless the phase says
otherwise. Gesture feel can't be judged by a test; each phase that
changes input lists what to try in the app.

- **P0. Adapters and the quaternion (no behaviour change).** *Built
  2026-10-09.*
  - New module `src/camera/`: `quat` (the orientation); `chain` (the one
    Euler chain, and the way back to its angles); `view3d` (each 3D
    camera's angles as a `Convention` on that chain); `gesture` (the
    viewport's pan, zoom, orbit and dolly, moved out of
    `ui::panel_viewer` unchanged, as `CameraEdit`s the viewport applies).
  - **The way back holds the roll and solves pitch, yaw and bank** -- a
    Tait–Bryan sequence singular only at bank ±90°, a camera on its side.
    Today's flame free-look solves pitch, yaw and roll, singular at pitch
    0: the flame's home pose, which is why its sliders jumped there. When
    the fly mode moves onto this (P6), free-look turns will move the
    pitch, yaw and bank sliders and leave `rotation` -- the 2D view's
    rotation too -- alone.
  - Tests: the chain equals the main pass's flame camera
    (`effective_camera_rows`) and each convention's frame equals its
    renderer's (`solid_frame`, `terrain_camera`, `sim_terrain_camera`)
    over an angle grid; angles round-trip through a quaternion in every
    convention; free-look from the home pose moves the angles by about
    the turn; a 3000-step free-look path stays faithful; the moved
    gestures keep the point under the cursor and carry the picture with
    a drag (flame and escape plane), and mode D's solid-navigation tests
    moved with them.
  - The `View2d`/`Camera3d` dispatch -- which camera a gesture moves --
    is P1's: it is where the gestures change.
- **P1. Gestures through the adapters.** *Built 2026-10-09.*
  - The table in C3, for the mouse, touch, keys, the tab-bar strip and
    the View menu: one dispatcher (`camera::gesture::{pan, turn, zoom,
    pinch, reset}`, deciding by `ViewKind`), one viewport input path
    (`ui::panel_viewer::{view_drag, view_scroll}`, the body and the strip
    alike).
  - **Orbit is a turntable** (my decision, reported): about world up and
    the level axis across the view, so the horizon stays as level -- or
    as tilted -- as it was; the near side of the scene follows the drag.
    The terrains orbited this way already and still do, to the radian.
    Fly mode's look keeps the FreeLook/FPS choice. The flame's Alt+drag
    was a free-look, which tilted the horizon; it is the turntable now.
  - **2D Alt+drag** rotates the picture by the angle the pointer sweeps
    round the screen's centre (grab and twist).
  - **Fixes:** the escape pinch is the wheel's fixed-point zoom (it was
    f64, clamped at 300, and wrote the plane's centre for a solid); one
    history name per gesture (`pan_view`, `zoom_view`, `rotate_view`,
    `orbit_camera`), all coalescing; gestures are silent during playback,
    as single-parameter edits already were; the right-button path
    readback only in PathMap; the View menu moves the shown camera (it
    wrote the flame's in Escape); zoom anchors to the cursor out as well
    as in; the tab strip orbits a terrain and moves a simulation
    terrain; touch and the keys reach the simulation terrain.
  - **Changes behaviour:** terrains pan on drag and orbit on Alt+drag or
    a right drag; mode D gains orbit; 2D Alt+drag rotates; in fly mode
    the wheel sets the fly speed; a flame's arrow keys move 5% of the
    view a press (they moved about 2.5%), as the escape keys did; the
    View menu's zoom steps are the keys' 1.5 (they were 1.2).
  - Tests: an orbit carries the near side with the drag for every 3D
    camera, a rolled screen included; the terrains orbit as before; the
    horizon's tilt is kept; a terrain's eye stays above the ground, and a
    fast flick's orbit is taken in quarter-radian steps; a 2D turn keeps
    the point under the pointer (flame and escape); zooming out keeps the
    point under the cursor; a pinch is one entry; Reset View per camera.
  - Try in the app: each mode's drag, Alt+drag, right drag, wheel, keys,
    and the tab strip; touch where available.
- **P2. The simulation's 2D view (C6).** *Built 2026-10-09.*
  - `sim.view` (`SimView`: centre as grid fractions, zoom over the fit,
    turn, tile), its ConfigPaths (`SimView*`, tracks `Sim.View.*`,
    `SimRerender`), the colour pass's camera, the gestures, and
    `ViewNavigation` Show for simulations.
  - **The default view keeps the letterbox code path**, so every saved
    simulation renders the same bits (visual suite 375/375); a camera
    path draws any other view. The step pass never reads the view, and
    the octave mode's cull keeps its own window.
  - **Tiling is a choice, off by default** (my decision, reported): on, a
    periodic field fills the letterbox bars with copies of itself, which
    would have changed every saved periodic picture. Under any other
    boundary the toggle is disabled and the grid's edges show.
  - A simulation terrain's ground is coloured by the same pass at the
    default view: the 2D view never moves it.
  - Tests: a pan of whole cells shifts the picture by them, a half turn
    flips it, a tiled view wraps round where the untiled one shows its
    edge, and the camera's mapping at the default's values is the
    letterbox's bit for bit (GPU, 64x64 at a cell a pixel); a zoomed,
    panned, turned, tiled view under the octave cull steps the same
    field bit for bit; the gestures keep the point under the cursor and
    the pointer.
- **P3. The View panel's 2D layout.** *Built 2026-10-09.*
  - One View panel for every mode, docked in the escape and simulation
    workspaces too. Its **Camera** section is one widget for every 2D
    view and a 3D flame's projected picture (`view_controls::camera_2d`):
    zoom (a magnification; the escape plane's as 10^n, since a deep zoom
    is no slider's), centre (the escape centre as exact decimals),
    rotation in degrees, an arrow pad and Reset -- the buttons through
    `camera::gesture`, as the keys and the mouse.
  - **Display options moved to the View panel** (my rule, reported): the
    escape lens, antialiasing and downsample; a simulation's upscale,
    fit, downscale and tiling. What each engine computes stays in its
    panel (the escape Reference and Diagnostics, a simulation's grid).
  - **Reset View** is the camera's reset, the same as the View menu's;
    the flame panel's old Reset also reset depth of field and fog, which
    are not the camera, and no longer does.
  - Angles in degrees: the 2D rotations (the escape one was an unbounded
    drag value), the warp's rotation per step (raw radians before), and
    the track editor's angle tracks (`is_angle_path`), shown and typed
    in degrees, stored in radians.
  - The animation picker's View and Rendering categories offer what the
    shown picture reads: the flame's camera only in the flame modes,
    the lights and fog where a solid or a terrain reads them.
  - The 3D views other than a flame's keep their camera in the Escape and
    Simulation panels until P4 (the View panel says so).
  - Tests: the View panel lays out in all seven views (headless egui, two
    frames); the workspace and visibility rules now require the View
    panel in every mode.
- **P4. The View panel's 3D layout, with lighting and material.** *Built
  2026-10-09.*
  - **3D Camera** (`view_controls::camera_3d`), one section for the four
    3D cameras in one order: Position, Target, zoom or distance, pitch,
    yaw, bank, roll, FOV or Perspective, Reset, fly mode and its
    settings. A flame's position is editable (its `camera_x/y/z`); the
    others store a target and a distance, so their Position is a
    **readout** of the eye (mode D in the attractor's units, the escape
    terrain in the plane's, a simulation terrain in grid widths --
    a window-bound grid read at 1080p) until P5/P6 give them an eye to
    edit. A 3D flame keeps the 2D Camera section above it (its picture's
    pan, zoom and roll -- the roll is that section's Rotation).
  - **Atmosphere:** fog -- the flame's two fields, which mode D reads and
    no panel offered in escape mode before -- the flame's depth density
    and far fade, a terrain's Far and Haze (a simulation's only on a
    repeated ground, as before; otherwise the panel says why).
  - **Depth of Field:** the flame's focus and blur; the path tracer's
    aperture and focus (moved out of its Material fold), or a note that
    the lit tier draws everything sharp.
  - **Lighting & Material** (`view_controls::lighting_3d`): the render
    tier and the path tracer, a terrain's shadows and occlusion, then the
    lights and the lit tier's shading (`solid_panel::lighting_rig`). A
    solid and the terrains see only what they read -- shading, ambient,
    diffuse, specular, shininess, occlusion strength, the lights -- with
    a note that an untouched rig is a default sun; the splat-only
    controls (solid strength, thickness, SSAO radius, normal smoothing,
    gap fill, shadow maps) are the flame's.
  - **Solid Lighting leaves the Window menus** (my decision, reported --
    it reverses the July 2026 move out of the View panel, as §3.7 asked).
    Its `PanelType` stays, so a saved layout that docks it still loads; it
    draws the same Lighting & Material section, and its "3D only" hint
    now asks the view (it said so over a solid and a terrain, which are
    3D).
  - **Out of the Escape and Simulation panels:** mode D's camera and path
    tracer; the escape terrain's tier, path tracer, shadows, occlusion,
    far, haze and camera (its height source, interior, lake and Detail
    stay); the simulation terrain's far, haze, shadows, occlusion, path
    tracer and camera (its layer, channel, tiling, height and softness
    stay). Each panel says where they went. The terrains' gesture hints
    describe P1's controls.
  - Angles: every 3D camera angle on the −180..180 degree slider that
    takes typed values beyond (mode D's pitch was a −90..90 slider, a
    simulation terrain's −89..89); each tooltip says where its zero is.
  - Fly mode is offered by `visibility::fly_mode(ViewKind)` -- the 3D
    flame alone until P6.
  - Tests: the View panel lays out in all seven views with every folded
    section drawn, lit and path traced; the simulation path coverage
    scan reads the View controls too.
- **P5. A terrain camera that can look anywhere (C4).** *Built
  2026-10-09.*
  - **A target height, not an eye-based camera** (my decision, reported:
    it changes the plan above). Both terrains' targets get a `target_lift`
    -- above the relief's top for the escape terrain, in view widths;
    above half the relief for a simulation's, in grid widths -- which
    carries the eye with it. With the lift, the eye goes anywhere and
    looks anywhere, sky included, and the three non-flame 3D cameras
    share one model (target, distance, angles). What it avoids: no
    conversion on load, and saved terrains' animation tracks on the
    centre keep their meaning (an eye-based camera would have made the
    centre the eye's ground point). Skip-if-default, so no file changes;
    `Escape.Terrain.TargetLift` and `Sim.Terrain.TargetLift` are tracks.
  - **The sections needed nothing.** `wanted_sections` already measures
    each node from the eye in 3D and culls by the frustum, and the walk
    clips to the relief's slab from an eye inside it or a ray going up --
    so the resolution near the eye is the same machinery at any lift,
    and a lift of 0 is the camera as it was (visual suite 375/375).
  - **The orbit's floor follows the eye**, not the pitch: where the eye
    would be at a pitch of 0.02 with the target at rest -- exactly the old
    clamp for an unlifted target -- so a lifted one orbits below the
    horizon, and a pitch already under the floor is not snapped up.
  - The View panel's 3D Camera has a Target lift row for both terrains,
    and Reset returns it to 0.
  - Tests: a lifted target carries the eye straight up and keeps the
    frame; a lifted orbit reaches below the horizon with the eye at the
    old floor's height; a GPU render from a lifted eye looking ten
    degrees up has the sky across the top, ground along the bottom, and
    no ray under the horizon within reach through the ground (3586
    checked). Before/after renders in `output/camera-p5/`.
- **P6. One quaternion fly mode (C4).** *Built 2026-10-09.*
  - `camera::fly`: `look` and `fly`, for the 3D flame, mode D and both
    terrains, as `CameraEdit`s under the fly history entry (one undo a
    flight). `app::fly_camera` is now only the keys and the drags; its
    own ZXZ algebra (singular at the flame's home pose, bank ignored) is
    gone with its tests.
  - **Look about the eye.** FreeLook turns about the screen's axes;
    FPS is a turntable about world up and the level axis, stopping a
    hair short of the poles. Composed on the quaternion, written back as
    pitch, yaw and bank with the roll held -- so a FreeLook roll now
    shows in Bank, and the flame's 2D Rotation is never touched (it used
    to drift). A solid-convention pitch past ±90° is written as its
    equivalent the other way round (heading reversed, camera upside
    down), since those cameras' pitch is ±90°.
  - **The pivot.** Every camera's stored point sits a distance in front
    of its eye and a look moves it by the forward's change times that
    distance: mode D's and the terrains' targets, and a perspective
    flame's `camera_x/y/z`, which sits `1/persp` in front of its
    viewpoint -- so a flame's look no longer swings the viewpoint round
    a short orbit. An orthographic flame turns about the screen's
    centre, as before.
  - **Flight:** a flame in world units a second, as before; the others
    in distances to the target a second, so a flight feels the same at
    every zoom. Mode D's steps are fixed-point adds to its decimal target
    (it flies at 2^-200). A terrain's eye is kept above the orbit's floor.
    A simulation terrain's lift is P5's.
  - **Offered for every 3D camera** (`visibility::fly_mode`): the View
    panel, the menu bar's button (it asked the render mode) and F2; fly
    mode drops when the view stops being 3D by any path.
  - **Held keys are released when the window loses focus** (the key-up
    went to another window, and the camera flew on).
  - A simulation terrain's pitch now reaches ±90°, as the escape
    cameras' does (it stopped at 89.4°, with no recorded reason; its frame
    holds at the poles).
  - Tests: a look turns toward the drag about a still eye, both look
    modes, every camera; FPS keeps a level horizon level; a 400-step
    FreeLook path through the poles stays faithful to the composed
    rotation, the pitch in range; W/D/E fly along the view's axes; a
    terrain's flight stops at the floor; mode D flies at 2^-200; the
    canonical angles are the same camera; every 3D view and no 2D one
    flies.
- **P7. Materials across the views (C7).** *Built 2026-10-09.* The
  design note, its renders and the decisions are
  [materials.md](materials.md): one material per picture, beside the
  lights -- Diffuse, the highlight (kept, in both tiers), and a new
  Gloss, Roughness and Glow -- which the lit tiers (the flame's shade
  pass, mode D's and the terrains' rig) and the path tracer all evaluate
  alike on the direct light (a coated plane under a sun: the same picture
  in both tiers to 7e-5). The 3D flame gains the coat and the glow. Old
  pictures' path-tracer materials are lifted beside the lights on load;
  the old paths are aliases. No shipped picture moves.
- **P8. Docs, text, and touch.** *Built 2026-10-09.*
  - **Touch equivalents of Alt and the right button** (the user's ask):
    the viewport's **Pan/Turn toggle** (bottom left, shown on the
    compact layout and once a touch has been seen) is the touch's Alt --
    a one-finger drag turns -- and a **long press** (half a second, within
    10 points) is its right button: held, it reads the path in PathMap;
    dragged, it turns. A ring under the finger shows the press. Tests:
    a drag pans with all its movement and is never a long press; a still
    finger holds, then its drag turns; two fingers or a tap are neither.
  - Docs: `docs/main/UI.md` (a View panel section; Input Handling
    rewritten around `camera::gesture`, with the controls table);
    `free-camera-movement.md` (Stage 4); `CLAUDE.md`; the Help panel's
    controls (mouse, fly mode, touch); `SCRIPTING.md` and the guide's
    Slow Turn (angles are radians: it keyed 360, about 57 turns).
  - Fixes: a fly key's release that a text field takes still lets go;
    the stale comments of §1d.
  - The other locales hold none of the changed keys (they fall back to
    English).
- **P8 (as planned).** `docs/main/UI.md`, `free-camera-movement.md`,
  help text (the "Alt-Drag - Rotate view" line becomes true everywhere),
  the locales, `SCRIPTING.md` (the `anim.key("rotation", ...)` example
  implies degrees; tracks are radians), and the stale comments in §1d.

## 5. Risks

- **Muscle memory.** The terrains' drag changes from orbit to pan. That
  is the point of the change, but it is the one gesture people have
  used since T2.
- **Terrain ground.** A free camera can fly under a terrain's ground,
  which the walk doesn't expect (the orbit clamps pitch above the
  horizon today). The terrain adapters keep the eye above the ground.
- **The escape terrain's camera** is the largest change: the zoom sets
  the sections' resolution band today, and the eye's height has to take
  that over without blurring the near ground. P5 measures it before
  anything else builds on it.
- **Deep zoom.** Every 3D move on mode D must stay a fixed-point add to
  the target, never an f64 round trip -- the pinch's f64 path is exactly
  that defect in 2D.
- **JWF round trips.** Flame cameras keep their stored angles and XML
  mapping; the panel shows the stored angles in degrees.
