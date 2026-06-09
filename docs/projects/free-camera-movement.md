# Free 3D Camera Movement

**Status**: Stage 1 (rotation) shipped on the `camera-bank-and-matrix-port` branch. Stage 2 (position + input handlers) ready to start. This doc tracks the stage 2 plan.

**Goal**: FPS-style free-fly navigation of the fractal. Mouse drag in the viewport rotates the view (look-around). `WASD` translates the camera along its current look/right axes. `Q` / `E` move down / up along the world-up axis.

The goal is **free-fly**, not orbital. The camera is not tethered to a focus point; it moves freely in 3D space. We don't read JWildfire's `cam_xfocus` / `cam_yfocus` / `cam_zfocus` attributes (no orbital camera support for now).

## Stage 1 — Shipped

The rotation half is done as of the [camera-bank-and-matrix-port PR](../../PR.md):

- 4-axis camera matrix (`pitch`, `yaw`, `bank`, `roll`) ported from JWildfire's `createProjectionMatrix`, with empirical convention mapping at the call site so each slider direction matches JWildfire and our pre-branch app.
- `camera_transform` uses all 9 matrix elements (was previously dropping a term).
- New `camera_bank` field with full XML round-trip through `cam_roll` (JWildfire's rename quirk).
- New "Bank" UI slider in the View panel.

The rotation matrix is built per frame and represents the camera's orientation in world space. Stage 2 builds on this: the matrix's columns ARE the camera-local basis vectors (forward / right / up), so converting WASD deltas into world-space translations is a few lines of CPU code.

## Stage 2 — Position + input

### Data layer

**New `FractalConfig` fields**:

- `camera_x: f32`, `camera_y: f32` alongside the existing `camera_z`. All radians-free — these are world-space distances.
- Defaults: `(0.0, 0.0, 0.0)`. Skip-serialize-if-default so existing `.fflame` JSON files stay clean.

**XML round-trip via JWildfire's `cam_pos_x` / `cam_pos_y` / `cam_pos_z`**:

- JWildfire writes these on every random preset (e.g., `cam_pos_x="0.0" cam_pos_y="0.0" cam_pos_z="0.0"`). Currently we drop them on import.
- Import: read each, store on the config field.
- Export: write the three attributes only when non-zero (matching JWildfire's conditional output pattern).

**Out of scope**: `cam_xfocus` / `cam_yfocus` / `cam_zfocus` (orbital focus point). We commit to free-fly. If we ever want to add orbital later, it's a separate flag + math change.

**Legacy interaction**: JWildfire also has the older `cam_zpos` attribute (single-axis Z position, the one our existing `camera_z` field already reads). Strategy:

- On import: if both `cam_pos_z` and `cam_zpos` are present, prefer `cam_pos_z` (the newer field).
- On export: write `cam_pos_z` for the new model; keep writing `cam_zpos` for backward compat with older JWildfire and Apophysis versions.
- Internal field stays as `camera_z` — same value, two on-disk names.

### Shader (camera_transform)

One-line change. Current:

```wgsl
fn camera_transform(p: vec3<f32>, m: mat3x3<f32>, camera_z: f32) -> vec3<f32> {
    let z_t = p.z - camera_z;
    ...
}
```

Becomes:

```wgsl
fn camera_transform(p: vec3<f32>, m: mat3x3<f32>, camera_pos: vec3<f32>) -> vec3<f32> {
    let p_t = p - camera_pos;
    let x = m[0][0]*p_t.x + m[1][0]*p_t.y + m[2][0]*p_t.z;
    let y = m[0][1]*p_t.x + m[1][1]*p_t.y + m[2][1]*p_t.z;
    let z = m[0][2]*p_t.x + m[1][2]*p_t.y + m[2][2]*p_t.z;
    return vec3<f32>(x, y, z);
}
```

`GpuParams` gains `camera_x: f32` and `camera_y: f32` next to the existing `camera_z`. Call sites pass `vec3<f32>(params.camera_x, params.camera_y, params.camera_z)`. Same plumbing pattern as the `camera_bank` work.

### Input handlers

**Mouse-look** (drag in viewport):
- Click + drag in the fractal viewport updates `camera_rotation_x` (pitch) and `camera_rotation_y` (yaw) proportional to the drag delta.
- Sensitivity: pixels-per-radian, configurable via a SystemSettings slider.
- Invert-Y toggle (also in SystemSettings) — some users prefer inverted vertical look.

**Keyboard movement** (WASD + QE):
- W: move forward along camera-look direction
- S: move backward along camera-look direction
- A: strafe left (perpendicular to look, in the horizontal plane)
- D: strafe right
- Q: move down along world-up
- E: move up along world-up

The "camera-look direction" and "strafe" axes are derived from the rotation matrix on the CPU side, then added to `camera_x / y / z`:

```rust
// Each frame, with dt = seconds-since-last-frame:
let m = build_camera_matrix_cpu(rotation_x, rotation_y, bank, rotation);
let forward = vec3(m[0][2], m[1][2], m[2][2]);  // matrix column 2 = look direction
let right   = vec3(m[0][0], m[1][0], m[2][0]);  // matrix column 0 = right direction
let world_up = vec3(0.0, 0.0, 1.0);

let mut delta = vec3(0.0, 0.0, 0.0);
if key_pressed(W) { delta -= forward * speed * dt; }
if key_pressed(S) { delta += forward * speed * dt; }
if key_pressed(A) { delta -= right   * speed * dt; }
if key_pressed(D) { delta += right   * speed * dt; }
if key_pressed(Q) { delta -= world_up * speed * dt; }
if key_pressed(E) { delta += world_up * speed * dt; }

if delta.length_squared() > 0.0 {
    config_manager.update_param(ConfigPath::CameraX, (camera_x + delta.x).into())?;
    // ... same for y and z
}
```

(Sign of `forward` depends on whether the camera looks down `-z` or `+z` — to be determined empirically against the rotation matrix conventions.)

**Speed**: configurable via SystemSettings. A "shift to go faster" modifier is typical FPS UX.

### Activation gating

WASD must not eat keystrokes when the user is typing in a text input (variation names, custom palette colors, etc.). Two reasonable approaches:

1. **Implicit gating**: only consume WASD when the fractal viewport has focus and no text input is active. egui can report which widget owns the focus.
2. **Explicit toggle**: a "Fly mode" button (or hotkey like `F`) enters/exits fly mode. When active, all the camera keybinds capture input; when inactive, keys go to the UI normally.

Option 2 is clearer for users and avoids edge cases. Recommended.

### UI

- Position sliders (`X` / `Y` / `Z`) in the View panel, alongside the existing pitch / yaw / bank / roll sliders. Same numeric-input style as the rotation sliders.
- Fly-mode toggle button in the View panel (and probably also a top-of-viewport overlay button so it's discoverable).
- Sensitivity sliders in SystemSettings → Preferences:
  - Mouse sensitivity (rad / pixel)
  - Invert Y toggle
  - Movement speed (units / second)
  - Sprint multiplier (Shift to go faster)

### Animation system integration

The animation track editor should be able to target the new position fields. Add `ConfigPath::CameraX` / `CameraY` to:

- `src/config/delta.rs`: enum variant + Display + I18nKey + storage-key + GPU-relevant set + coalescing set
- `src/config/manager.rs`: apply + get_value handlers
- `src/ui/target_selector.rs`: animation-target list entry
- `src/ui/track_editor.rs`: read-value mapping

Same pattern as `CameraBank` from the rotation PR — established mechanically.

## Implementation order

Suggest two PRs:

**PR A — Position field plumbing** (independently shippable, ~3 hours)

1. `FractalConfig::camera_x` + `camera_y` fields + Default + serde
2. XML import: read `cam_pos_x` / `y` / `z` (prefer `cam_pos_z` over `cam_zpos` when both present)
3. XML export: write `cam_pos_*` attributes when non-zero; keep emitting `cam_zpos` for legacy
4. `GpuParams` extension + plumbing through `compute_pass` / `resize` / `reset` / etc.
5. WGSL `camera_transform` translation extension
6. `ConfigPath::CameraX` / `CameraY` variants + ConfigManager handlers
7. Position sliders in the View panel
8. Animation target wiring
9. Round-trip XML test

PR A by itself improves JWildfire interop immediately (we stop dropping `cam_pos_*` on import) and lets users author position via sliders.

**PR B — Input handlers + fly mode** (~3-4 hours)

1. Fly-mode toggle state + hotkey + UI button
2. Mouse-drag rotation handler (only active in fly mode, only on fractal viewport)
3. WASD / QE keyboard handler (only active in fly mode)
4. Speed + sensitivity SystemSettings + Preferences UI
5. Invert-Y toggle
6. Camera-local basis extraction (CPU mirror of the rotation matrix math)

PR B is where "navigate around the fractal" becomes real.

## Open questions

- **Movement speed scaling**: should speed scale with the visible fractal extent (zoom)? An FPS speed of 1 unit/second feels different at zoom=1× vs zoom=100×. Probably scale by `1 / zoom` so it stays comfortable at any magnification.
- **Coordinate convention**: which axis is forward? Confirm by extracting a column from the rotation matrix and rendering a debug marker.
- **Sprint key**: Shift is conventional but conflicts with no current shortcut. Confirm.
- **Mouse-look while not in fly mode**: should a right-click drag rotate the camera even without entering fly mode? Many 3D apps do this. Decide after fly mode is in.

## Out of scope

- **Orbital camera** (JWildfire's `cam_xfocus/yfocus/zfocus`). Free-fly only for now. Orbital would be a separate mode with different mouse-drag and WASD math.
- **Gimbal lock handling**. Free-fly with Euler angles will gimbal-lock at pitch=90°. Acceptable for v1 — most fractal flying happens far from these poles. If it becomes a real problem, switch to quaternion storage internally (Euler still on disk for JWF compat).
- **Camera path animation** (auto-fly along a saved path). That's a future "tour mode" feature, separate from manual fly-mode.

## Related

- [`jwf-features.md`](jwf-features.md) — JWF camera rotation entry (shipped this round)
- [`../experimental/PROPOSAL-true-3d-affine.md`](../experimental/PROPOSAL-true-3d-affine.md) — unrelated, but in the same general "make 3D rendering more capable" theme
