# Cameras

**Overview:** The app has seven cameras, one for each way a picture is
seen: the flame's 2D view and its 3D camera, the escape plane, mode D's
solid camera, the escape terrain's, a simulation's 2D view and its
terrain's. Each keeps its own config fields, units and zero, because the
files, the JWildfire/Apophysis exchange, the animation tracks and the
scripts all address them. One module, `src/camera/`, is the only place
that knows what those fields mean. Every gesture, the View panel and fly
mode go through it, so the cameras move alike.

**See also:**
- [UI.md](UI.md): the user-facing controls table (*Input Handling*) and
  the View panel (*The View panel*). This doc does not repeat the table.
- [TERRAIN.md](TERRAIN.md): the terrains and mode D's solid, which three
  of the 3D cameras look at.
- [ESCAPE.md](ESCAPE.md): the escape plane, mode D, and the camera lens.
- [RENDERER.md](RENDERER.md): the flame's projection in the compute pass.
- [CONFIG.md](CONFIG.md): `ConfigManager`, history and coalescing.
- [ANIMATION.md](ANIMATION.md): every camera field is a track.

**Design record:**
- [camera-unification.md](../archive/3d-camera/camera-unification.md):
  one model for the seven cameras, one gesture set, fly mode in every 3D
  view, touch's Alt and right button (phases P0 to P8).
- [free-camera-movement.md](../archive/3d-camera/free-camera-movement.md):
  the flame's free camera and fly mode (stages 1 to 4), and where this
  camera deliberately differs from JWildfire's.
- [3d-camera-system-apophysis.md](../archive/3d-camera/3d-camera-system-apophysis.md):
  the original 4-angle camera after Apophysis and JWildfire.

**Code locations:**
- [src/camera/mod.rs](../../src/camera/mod.rs): `CameraEdit`
- [src/camera/quat.rs](../../src/camera/quat.rs): `Quat`, the
  orientation every 3D turn composes in
- [src/camera/chain.rs](../../src/camera/chain.rs): the one Euler chain,
  and `decompose_near`, the way back to angles
- [src/camera/view3d.rs](../../src/camera/view3d.rs): `Convention`,
  `Angles`, `Frame`, and each 3D camera's adapter
- [src/camera/gesture.rs](../../src/camera/gesture.rs): `ViewKind`,
  `view_kind`, and `pan`, `zoom`, `turn`, `orbit`, `pinch`, `reset`
- [src/camera/fly.rs](../../src/camera/fly.rs): `look` and `fly`
- [src/app/fly_camera.rs](../../src/app/fly_camera.rs): fly mode's keys
  and drags
- [src/ui/panel_viewer.rs](../../src/ui/panel_viewer.rs): `view_drag`,
  `view_scroll`, `TouchTracker`
- [src/ui/view.rs](../../src/ui/view.rs) and
  [src/ui/view_controls.rs](../../src/ui/view_controls.rs): the View
  panel

---

## The seven cameras

`gesture::view_kind(config)` names the camera the viewport shows. It
reads the render mode and, in Escape and Simulation, whether a terrain
is on (`EscapeConfig::terrain_active`, `SimConfig::terrain_active`) and
whether the escape formula is a solid (`escape::ifs::formula_is_solid`).

| `ViewKind` | Stored in | Scale | Angles' zero | Screen roll |
|---|---|---|---|---|
| `Flame2d` | `pan_x`, `pan_y` (f64), `zoom`, `rotation` | the short side spans `4 / zoom` | none | `rotation` |
| `Flame3d` | the 2D fields, plus `camera_rotation_x` (pitch), `camera_rotation_y` (yaw), `camera_bank`, `camera_x/y/z`, `perspective_strength` | `zoom` scales the picture after projection | pitch 0 looks straight down (JWildfire's) | `rotation`, after projection |
| `EscapePlane` | `escape.center_re`, `center_im` (exact decimal strings), `zoom_log2` (f64), `rotation` | the height spans `4 / 2^zoom_log2`, Im up | none | `rotation` |
| `Solid` (mode D) | `escape.cam_target_x/y/z` (decimal strings; empty is the attractor's centre), `cam_pitch`, `cam_yaw`, `cam_bank`, `cam_fov`, `zoom_log2`, `rotation` | the eye is `3.2 r / 2^zoom_log2` from the target, `r` the attractor's measured radius | pitch 0 is the horizon | `rotation`, inside the chain |
| `EscapeTerrain` | no fields of its own: the target is `center_re/im` on the relief's top, lifted by `terrain.target_lift`; mode D's `cam_*` angles; `zoom_log2` sets the world's unit | the eye is 1.3 view widths from the target (`footprint::FRAME_DISTANCE`) | pitch 0 is the horizon; yaw 0 looks up the 2D picture | none: `rotation` turns the heading |
| `Sim2d` | `sim.view`: `center_x/y` (grid fractions), `zoom` (over the fit), `rotation`, `tile` | `zoom` over the letterbox or cover fit | none | `sim.view.rotation` |
| `SimTerrain` | `sim.terrain`: `target_x/y` (grid fractions), `target_lift`, `cam_pitch`, `cam_yaw`, `cam_bank`, `cam_fov`, `cam_distance` (grid widths) | `cam_distance` | pitch 0 is the horizon; yaw 0 looks north | none |

`ViewKind::is_3d` is true for `Flame3d`, `Solid`, `EscapeTerrain` and
`SimTerrain`.

Some details the table leaves out:
- **The escape plane** covers every escape picture that is not a solid
  or a terrain: the 2D formulas, the 2D fields and the planar IFS (the
  Flame Attractor).
- **The terrain cameras have no field for their eye.** Each stores a
  target, a distance (or a zoom that sets one) and angles. The View
  panel shows their eye as a read-only Position.
- **The simulation's 2D view is display only.** The colour pass reads
  it and the step never does, so moving it never changes the run. `tile`
  repeats the field past its edges, under a periodic boundary only. See
  [SIMULATION.md](SIMULATION.md).

### Precision

- **The escape centre and mode D's target are exact decimal strings.**
  A pan, a zoom toward the cursor and a fly step are added to them in
  fixed point, as a mantissa and a power of two
  (`FixedPoint::decimal_add_floatexp`, `src/escape/fixedpoint.rs`). An
  f64 step would stop at the centre's own ulp, and underflow outright
  past about 2^1060. If the centre text does not parse (mid-edit), the
  plane falls back to f64 so a pan never dead-stops.
- **The flame's pan** is f64 in the config. It is f32 on the GPU.
- **A simulation terrain's target** is an f32 grid fraction. There is
  no deep zoom there to need more.

---

## One orientation model

### The chain

Every 3D camera's orientation is the flame's Euler chain
(`build_camera_matrix` in `shaders/core/utilities.wgsl`), world to camera:

```text
M = Rz(roll) · Rx(pitch) · Ry(bank) · Rz(−yaw)
```

The rows of `M` are the camera's axes in world coordinates:
- row 0 is screen-right;
- row 1 is screen-down;
- row 2 is the camera's +z. The camera looks along −row 2.

`chain::rows` builds it in f64. The test `the_chain_is_the_flames` holds
it to the flame's main pass (`renderer::shade_pass::effective_camera_rows`).
JWildfire applies its matrix transposed, which is why the shader's
call-site slot mapping looks swapped. The comments at
`build_camera_matrix` explain it.

### Conventions

Each camera stores its angles in its own terms. A `view3d::Convention`
says how they enter the chain:

| Convention | Used by | Into the chain |
|---|---|---|
| `Flame` | the 3D flame (`flame_angles`) | as stored; `rotation` is the roll |
| `Solid { yaw_offset: 0 }` | mode D (`solid_angles`) | pitch from the horizon (`π/2 − pitch`), yaw less a quarter turn, the roll reversed, the screen's x mirrored |
| `Solid { yaw_offset: rotation − π/2 }` | the escape terrain (`escape_terrain_angles`) | as mode D, with the 2D view's rotation added to the heading; no roll |
| `Solid { yaw_offset: −π/2 }` | a simulation terrain (`sim_terrain_angles`) | as mode D, yaw 0 looking north; no roll |

The mirror is a matter of handedness:
- **Flames draw world +y down the screen**, as Apophysis does, so the
  flame's frame is the mirror of a physical camera's.
- **The solid cameras look the physical way.** `escape::ifs::solid_frame`
  negates the chain's screen-x row.

A mirror is not a rotation, so `Convention::frame_of` applies it to the
frame and never stores it in the orientation. Tests in `view3d` hold
each convention's frame to its renderer's camera for a grid of angles:
`solid_frame`, `footprint::terrain_camera` and
`sim::terrain::sim_terrain_camera`.

### Composing and coming back

A turn is composed on a `quat::Quat` in f64, the camera's world-to-camera
rotation:
- a turn about an axis in camera coordinates is a left product,
  `Quat::from_axis_angle(axis, angle) * q`;
- a turn about a world axis is a right product.

Composition is gimbal-free. Only the way back to stored angles can
alias.

**`chain::decompose_near` holds the roll** and solves pitch, yaw and
bank: a Tait–Bryan sequence, singular only at bank ±90°, which is a
camera rolled on its side. It returns the solution nearest the angles it
was given, each unwrapped toward them, so a turn moves the sliders by
about the turn. At the pole it holds the yaw and puts the rest into the
pitch.

The flame's old free-look solved pitch, yaw and roll instead (a ZXZ
sequence). That is singular at pitch 0, the flame's home pose, where its
sliders jumped and its 2D `rotation` drifted. Holding the roll means a
3D turn never writes `rotation`.

**No quaternion is held between events.** Each gesture reads the stored
angles, builds the orientation, composes the turn and writes angles back.
An orbit is split into steps of at most a quarter radian (at most 64), so
the nearest-solution rule cannot jump to the far side of a pole on a fast
flick.

---

## Camera edits

A camera operation returns a `CameraEdit`: the `(ConfigPath,
ConfigValue)` writes it makes, and the history entry they go under.
- `history: None` is one parameter's edit, coalescing as that
  parameter's edits do.
- `history: Some(name)` is a named batch.

`CameraEdit::apply` makes the writes through the `ConfigManager`, so
undo, animation and scripts see the same paths they always did. During
animation playback it writes silently (`update_param_silent`), so a
gesture during playback does not fill the history.

| Entry | Name | Made by |
|---|---|---|
| `gesture::PAN` | `history.action.pan_view` | every pan |
| `gesture::ZOOM` | `history.action.zoom_view` | a zoom, a dolly, a pinch |
| `gesture::ROTATE` | `history.action.rotate_view` | a 2D rotation |
| `gesture::ORBIT` | `history.action.orbit_camera` | a 3D orbit |
| `gesture::RESET` | `history.action.reset_view` | Reset View |
| `fly::FLY` | `history.action.fly_camera` | fly mode's looks and flights |

A drag writes a varying set of paths each frame, since an axis that did
not move is left out. So the `ConfigManager` coalesces the gesture
entries by their shared name, path by path (`GESTURE_HISTORY_DESCS` in
`src/config/manager.rs`). A whole drag, or a whole flight, is one undo
step. Reset is not coalesced.

Every input that moves a camera produces these edits: the viewport body,
the tab-bar strip above it, touch, the arrow and +/− keys
(`src/app/input.rs`), the View menu (`src/ui/mod.rs`) and the View
panel's buttons. That is why they move a camera identically.

---

## Gestures

The gestures are pure functions of the config and the gesture's size in
pixels. `pan`, `zoom`, `turn`, `pinch` and `reset` dispatch on
`view_kind`, and each returns `None` when it has nothing to do. What a
gesture moves is each camera's own. The table of which input does what
is in [UI.md](UI.md#input-handling).

### Pan

The picture follows the drag.
- **Flame:** `flame_pan`. A pixel is `4 / (zoom · short side)` world
  units, turned into the pan frame by
  `FractalConfig::screen_delta_to_pan_frame` (rotation-aware, the same in
  2D and 3D).
- **Escape plane:** `escape_pan_plane`. The centre moves opposite the
  drag by a symbolic delta, in fixed point.
- **Mode D:** the target slides across the screen plane at its own
  depth, `solid_pixel_step` world units a pixel, in fixed point. An empty
  target becomes explicit (the attractor's centre) the first time the
  camera moves.
- **Escape terrain:** `terrain_pan`. The target slides across the
  ground, along the camera's right and along its heading. The heading
  share is foreshortened by `1/sin(pitch)`, capped near the horizon.
  It is added to the centre in view widths, in fixed point.
- **Simulation terrain:** `sim_terrain_pan`, the same, in grid
  fractions.
- **Simulation 2D:** in grid cells, through the colour pass's own
  mapping.

### Zoom

`wheel_factor` turns a scroll into a factor (`1.1^(0.03 · scroll)`, about
15% a notch); Shift takes its fourth root. Each zoom keeps the point under
the cursor where it is, in and out:
- **Flame:** `zoom` clamped to `0.01..max_view_zoom()`. That is 1e4, or
  1e30 with Focused Rendering on.
- **Escape plane:** `zoom_log2` clamped to −8..1e8, and the centre
  anchored in fixed point.
- **Mode D:** a dolly. The zoom walks the target under the cursor, in the
  target's plane.
- **Escape terrain:** a dolly toward the target, the screen's centre. The
  point under the cursor is on the ground, at a depth an anchor in the
  plane knows nothing of.
- **Simulation terrain:** `sim_terrain_dolly` divides `cam_distance` by
  the factor, clamped to 0.01..100.
- **3D flame:** zooms its 2D picture after projection, as JWildfire's
  scale does.

### Turn and orbit

`turn` reads `TurnSettings`, which are the fly mode's sensitivity and
Invert Y: every turn shares them.
- **A 2D view rotates** by the angle the pointer swept round the screen's
  centre (grab and twist). Within 4 pixels of the centre the angle is
  noise and is ignored.
- **A 3D camera orbits its target**, as a turntable (`orbit`): the drag
  across turns about world up, the drag down turns about the level axis
  across the view.
  - The horizon stays as level, or as tilted, as it was, and the bank is
    kept.
  - The near side of the scene follows the drag.
  - The screen's roll turns the drag first, so a drag is in the screen as
    drawn.
  - A mirrored screen turns the other way for the same look.

### Pinch

`pinch` is a two-finger gesture as one history entry (`ZOOM`). It
applies, in order, to a working copy of the config:
1. the zoom about the fingers' midpoint;
2. the midpoint's translation as a pan;
3. the twist, as a 2D rotation or, in 3D, an orbit about world up.

### Reset View

`reset` returns the shown camera to its start:
- **Flame:** zoom 1, pan 0, rotation 0. In 3D also the angles, the bank
  and `camera_x/y/z` (perspective is kept).
- **Escape plane:** the default centre, `zoom_log2` 0, rotation 0.
- **Mode D:** the target cleared (the attractor's centre again), the
  default angles, `zoom_log2` 0, rotation 0.
- **Escape terrain:** the default angles, `zoom_log2` 0, rotation 0 and
  the lift 0, over the same ground (the centre is kept).
- **Simulation 2D:** the default centre, zoom and rotation. Tiling is the
  picture's choice and is kept.
- **Simulation terrain:** the default angles, distance, target and lift.

### The terrains' floor and target lift

A terrain's target rests on its ground:
- **The escape terrain's** rests on the relief's top at the view's
  centre.
- **A simulation's** rests at half a field unit's height.

`target_lift` raises it, and the eye with it, in the terrain's units:
view widths for the escape terrain, grid widths for a simulation. Lifted,
the camera can look level, or up at the sky, from above the ground. At 0
it is the camera every terrain had before.

**The orbit's floor follows the eye.** An orbit keeps a terrain's pitch
between a floor and just short of straight down. The floor is where the
eye would be at a pitch of 0.02 rad with the target at rest:

```text
floor = asin(sin 0.02 − lift / distance)
```

It is held above −90°. For an unlifted target this is exactly the old
clamp; a lifted target orbits below the horizon, down to looking straight
up. A pitch already under the floor, typed or flown there, is a floor of
its own: an orbit never snaps it up. Fly mode keeps the eye above the
same floor by raising the lift -- or, with collisions, above the ground
under the eye (*A terrain's ground*, below).

The View panel's 3D Camera section has a Target lift row for both
terrains, and Reset sets it to 0.

---

## The headlamp

A light can ride with the camera (`LightKind::Headlamp`; TERRAIN.md,
*Kinds of light*): it sits at the eye -- lifted by `lift` camera
distances along the camera's up, so its shadows show -- and its beam is
aimed down the view, `cone` degrees each side. Its reach is in camera
distances, the eye's distance to its target, so it lights the same share
of the view at any zoom; flying inside a solid, it lights what you look
at. Offered in mode D and on both terrains. A point light can be dropped
where the camera is with **Place at Camera** in the light's row.

## Fly mode

One fly mode for every 3D camera (`camera::fly`):
- **Turning it on:** F2, the View panel's button, or the menu bar's.
- **Where it is offered:** `ui::visibility::fly_mode(ViewKind)` shows it
  for the four 3D kinds. Anywhere else, a request to turn it on is
  ignored.
- **Turning off by itself:** `App::update_fly_camera` drops fly mode when
  the camera shown stops being 3D by any path (a mode switch, a load,
  undo).
- **Held keys** are released when the window loses focus, and when a
  text field takes a key's release.

### Look

A drag in fly mode is a look about the eye (`fly::look`). The drag
becomes a world rotation, composed on the camera's quaternion and
written back as pitch, yaw and bank with the roll held.

| | FreeLook (default) | FPS |
|---|---|---|
| A drag turns about | the screen's axes (`free_turn`) | world up, then the level axis (`level_turn`) |
| The horizon | can roll; the roll shows in Bank | keeps its tilt |
| Near the poles | turns through them | stops a hair short of straight up or down |
| Q / E rise along | the screen's up | world up |

A solid-convention camera's pitch is ±90° in the config. A look past
that is written as the same orientation the other way round: the pitch
reflected, the heading reversed, the bank `π − bank` (`canonical`).

### The pivot

Each camera stores a point a distance `D` in front of its eye. A look
keeps the eye still and moves that point by the forward's change times
`D`:
- **A perspective flame's `camera_x/y/z`.** The projection divides by
  `1 − persp·z`, a pinhole whose viewpoint sits `1/persp` behind the
  stored point, so the point moves by `Δforward / persp`.
- **An orthographic flame** has no viewpoint. It turns about the point at
  the screen's centre: `camera_x/y/z` itself when the pan is 0, and with
  a pan, `camera_x/y/z` is shifted so that point stays put.
- **Mode D's target:** a fixed-point add of `Δforward` times
  `solid_distance`, so it works at any depth.
- **The escape terrain's** centre (decimal) and lift.
- **A simulation terrain's** target fractions and lift.

### Flight

`fly::fly` pushes the camera along its axes: W/S forward and back, A/D
right and left, Q/E down and up. Each frame moves it
`fly_move_speed × dt`; `dt` is clamped to 0.1 s, and Shift multiplies the
speed by `fly_sprint_multiplier`.
- **A flame** flies in world units a second.
- **The other cameras** fly in distances to their target a second, so a
  flight feels the same at every zoom. Mode D's steps are fixed-point
  adds to its decimal target, so it flies at any depth.
- **A terrain's eye** is kept above the orbit's floor, or above the
  ground (*A terrain's ground*).

In fly mode the wheel sets the speed: `view_scroll` multiplies
`fly_move_speed` by the wheel factor, clamped to 0.05..20.

### A solid's surface (2026-10-19)

A solid knows how far its surface is: its own distance estimate at the
eye, `ifs_distance_at` -- every solid kind, Rhai ones included, at the
render's precision. While fly mode is on, `App::update_fly_camera` has
the escape renderer measure it every frame (`probe_clearance`: the walk's
shader to its entry point, and one thread writing one float at binding
7) and read it back without waiting (`poll_clearance`), as the terrain's
nearest-ground probe is read. The last measure that landed is a
`fly::Clearance`: the eye it stood at (the world's, f64) and the distance.
`fly::fly` takes it in `FlyLimits`:
- **Collisions** (`fly_collisions`, on): a step never takes the eye
  further than half the clearance (`CLEARANCE_FRACTION`) from where it
  was measured -- a sphere-traced step, safe in every direction. The eye
  closes on a wall in ever smaller steps and stops short of it; moving
  away, each new measure lets it go further.
- **Proximity speed** (`fly_proximity`, off): a step is a share of the
  clearance rather than of the distance to the target, and the zoom
  eases a quarter of the way (`ZOOM_FOLLOW`, in log2) toward the
  clearance each step, the eye held where the step put it: the target
  slides along the view by what its distance changes. Flying into the
  surface slows and zooms in -- a deep zoom, as deep as the solid's
  precision goes (the IFS flame solids' seeded walk; f32 elsewhere).

Without a measure yet, or in any other camera, a flight is as above.
Gates: `the_clearance_probe_reads_the_solid_at_the_eye` (the Julia set at
c = 0 is the unit ball, its estimate 0.5 r ln r exactly: within 1e-6),
`the_clearance_probe_lands_for_every_shipped_solid`,
`a_solid_flies_within_its_clearance`.

### A terrain's ground (2026-10-19)

The orbit's floor keeps an escape terrain's eye above the relief's top --
never down into a valley -- and a simulation terrain's above half a field
unit, through any taller ground. The terrain's probe of the nearest ground
in view (`TerrainRenderer::probe_nearest`) now measures the ground under
the eye too: one ray straight down from above the relief's slab, so an
eye inside a hill finds the hill's top, its height into the probe's
second word (`ground_under_eye`; None where no ground lies under the
eye). The escape terrain's frame probes every changed view already; the
simulation terrain's probes while fly mode uses it (`probe_ground`, before
the frame's render: the probe writes the view's uniform). `fly::fly` takes
the height as `FlyLimits::ground`, in the terrain's world units (current
widths; cells), and its pivot (`Floor`):
- **Collisions**: the floor is the ground under the eye rather than
  where the target rests -- the eye stays `sin 0.02` distances above it
  (`FLOOR_PITCH`, the orbit floor's own height). It flies down into a
  valley; it climbs a hill flown into, a probe later (the ground is the
  last measured, at the eye's place then). No ground measured, or
  collisions off: the orbit's floor.
- **Proximity speed**: the step in heights of the eye above the ground
  rather than in distances to the target (the floor's height at least),
  so it slows as it nears the ground. The zoom does not follow over a
  terrain.

Gates: `the_probe_finds_the_ground_under_the_eye` (the bilinear surface's
height on the analytic cone, from above and from inside it, to 1e-6;
none past a ground's edge), `a_terrain_flies_above_its_ground` (a dive
stops over the valley measured, at the orbit's floor without a measure or
without collisions; a simulation's hill raises the floor; proximity paces
by the height).

### Settings

Device preferences in `SystemSettings` (`src/storage/settings.rs`), not
`FractalConfig`. They are set in the View panel's Fly Mode Settings fold.

| Field | Default | UI | Also used by |
|---|---|---|---|
| `fly_camera_mode` | `FreeLook` | Camera mode: Free-look / FPS | |
| `fly_mouse_sensitivity` | 0.005 rad a pixel | Mouse sensitivity (0.0005..0.05) | every orbit |
| `fly_move_speed` | 1.0 | Move speed (0.05..20); the wheel in fly mode | |
| `fly_sprint_multiplier` | 3.0 | Sprint multiplier (1..20) | |
| `fly_invert_y` | false | Invert Y axis | every orbit |
| `fly_collisions` | true | Stop at surfaces | a solid's flight, a terrain's floor |
| `fly_proximity` | false | Speed by distance | a solid's flight, a terrain's pace |

---

## JWildfire and Apophysis

The flame's 3D camera is JWildfire's 4-angle camera. Its `.flame`
attributes round-trip in `src/flame_xml.rs`:

| XML | Field | Note |
|---|---|---|
| `cam_pitch`, `cam_yaw` | `camera_rotation_x`, `camera_rotation_y` | radians |
| `cam_roll` | `camera_bank` | JWildfire's rename: it writes its bank under `cam_roll` |
| `rotate` | `rotation` | degrees in the file, radians in the config |
| `cam_pos_x/y/z` | `camera_x/y/z` | `cam_pos_z` wins over the legacy `cam_zpos`; export writes both |
| `cam_perspective` / `cam_persp` | `perspective_strength` | Apophysis's and JWildfire's names; export writes both |
| `cam_zoom` | folded into `zoom` | `zoom = scale / 200 × cam_zoom` |
| `cam_dof` | `dof_blur_strength` | |

The projection is Apophysis's, `zr = 1 − persp·z`. Points with
`zr < 1e-3` are clipped, as JWildfire does (Apophysis wrapped them round
instead). JWildfire's orbital focus (`cam_xfocus/yfocus/zfocus`) is not
read: the camera is free-fly.

**Deliberate divergences** (recorded in
[free-camera-movement.md](../archive/3d-camera/free-camera-movement.md)):

- **`cam_pos` is a world-space position.** Ours is applied before the
  rotation, as `M·(p − camera_pos)`, which is what free flight needs.
  JWildfire adds it after the rotation, in camera space, and adds a
  further `camPosZ` term to the perspective denominator. A flame with a
  non-zero `cam_pos` renders differently here, in both directions. An
  exact conversion exists only for orthographic flames, so none is made.
  The TODO at the import site gives the formula.
- **3D pan composes as 2D does.** Pan, then rotate, then zoom, after the
  projection: the Apophysis order. The roll is passed as 0 into the
  matrix and applied as the 2D rotation, which is exact because the
  outermost roll never touches camera-space z. JWildfire pans 3D in
  post-projection screen coordinates. The two agree whenever the pan or
  the rotation is 0. `FractalConfig::screen_delta_to_pan_frame` is the
  one screen-to-pan conversion.

The escape and simulation cameras have no foreign format to match.

---

## A lens sees its view's own y

The escape engine's camera lens (a variation applied to the screen
offset; see [ESCAPE.md](ESCAPE.md)) works in half-height-one units: `n.y`
spans [−1, 1] and `n.x` spans [−aspect, aspect]. Which way is up is the
view's own:
- **The escape plane** is y up: Im runs up the screen.
- **The Flame Attractor and mode D** are y down, as flame mode draws its
  world. `escape::ifs::view_basis` stands a Flame Attractor the way its
  flame stands, so a lens bends it as the same variation would bend the
  flame on screen.

The test `a_lens_sees_its_views_own_y` (`src/escape/lens.rs`) asks with
`bent`, a lopsided lens.

Today the lens reaches the escape plane, the Flame Attractor and mode D's
rays (`//__LENS_APPLY_RAY__` in `ifs_ray`). The terrain passes splice
their sub-pixel jitter there instead, and the View panel hides the Lens
section for a terrain. Flames and simulations have no lens yet: the plan
to bring it to every mode, under this convention, is
[camera-lens-everywhere.md](../projects/camera-lens-everywhere.md).

---

## Where the UI lives

- **The View panel** (`src/ui/view.rs`) lays itself out by `ViewKind`.
  It uses the shared widgets in `src/ui/view_controls.rs`:
  - `camera_2d` for the 2D views and a 3D flame's projected picture;
  - `camera_3d` for every 3D camera: position (a readout where the camera
    stores a target), target, lift, zoom or distance, pitch, yaw, bank,
    roll, field of view or perspective, Reset, then fly mode;
  - `atmosphere_3d`, `depth_of_field_3d` and `lighting_3d`.

  Its user-facing description is in [UI.md](UI.md#the-view-panel).
- **Angles show in degrees**, on a −180..180 slider that takes typed
  values beyond the range (`view_controls::angle_slider`). The config
  stores radians. Each camera keeps its own zero, so the panel shows the
  number the file stores.
- **The viewport** (`src/ui/panel_viewer.rs`): `view_drag` and
  `view_scroll` are the one input path, for the viewport body and the
  tab-bar strip alike.
  - Alt+drag or a right drag turns, a plain drag pans, and Shift makes
    either a quarter as fine.
  - In fly mode a drag accumulates into a look, which the app applies
    (`App::apply_fly_mouse_look`).
- **Touch** (`TouchTracker`):
  - a finger held still for half a second within 10 points is a long
    press, the touch's right button: dragged, it turns;
  - the viewport's Pan/Turn toggle is the touch's Alt;
  - two fingers are `pinch`.

---

## Adding a camera

1. Store its fields where its engine's config lives, skip-if-default,
   with `ConfigPath`s, so files, tracks and undo see them.
2. Add a `ViewKind` and a case in `view_kind`.
3. If it is 3D, give it a `Convention` and an adapter in `view3d` (its
   angles, the paths of its pitch, yaw and bank). Add a test holding its
   `Convention::frame` to its renderer's camera.
4. Add its case to `pan`, `zoom`, `reset` and, for 3D, `turnable` and
   `fly::flyer` with its pivot in `pivot_moved`.
5. Add its rows to `ui::visibility::fly_mode` and the View panel's
   `camera_2d` or `camera_3d`.
