# Free 3D Camera Movement

**Status**:
- Stage 1 (rotation) shipped on `camera-bank-and-matrix-port` and merged.
- Stage 2 (position + input handlers) shipped on `free-camera-position`:
  - PR A — camera position fields + JWildfire `cam_pos_x/y/z` round-trip (`2534b22`)
  - PR B — fly-mode input handlers + UI (`ee4109c`)
  - Forward-direction fix under rotation (`ae64788`)
  - Behind-camera perspective wraparound clip (`baf1547`)
  - Dynamic pan + rotation compensation in fly mode (`514821f`)
- Stage 3 (free-look mouse-look) — shipped on `free-camera-position` (see below for the corrected analysis that reshaped it).
- Follow-ups shipped on the same branch:
  - Camera-relative Q/E + angle wrapping into the slider range (`a05a795`)
  - Bank composition order fixed to match JWildfire (`556f9bd`)
  - 3D pan unified with 2D semantics (`875cdc4` input side, `f151233` pipeline)
  - Camera modes: FreeLook | FPS as a SystemSettings preference (below)

**Goal**: FPS-style free-fly navigation of the fractal. Mouse drag in the viewport rotates the view (look-around). `WASD` translates the camera along its current look/right axes. `Q` / `E` move down / up along the world-up axis.

The goal is **free-fly**, not orbital. The camera is not tethered to a focus point; it moves freely in 3D space. We don't read JWildfire's `cam_xfocus` / `cam_yfocus` / `cam_zfocus` attributes (no orbital camera support for now).

## Stage 1 — Shipped

The rotation half shipped on the `camera-bank-and-matrix-port` branch (merged to main):

- 4-axis camera matrix (`pitch`, `yaw`, `bank`, `roll`) ported from JWildfire's `createProjectionMatrix`, with empirical convention mapping at the call site so each slider direction matches JWildfire and our pre-branch app.
- `camera_transform` uses all 9 matrix elements (was previously dropping a term).
- New `camera_bank` field with full XML round-trip through `cam_roll` (JWildfire's rename quirk).
- New "Bank" UI slider in the View panel.

The camera matrix maps world space to camera space via `M · (p − camera_pos)`. In that convention the matrix's **rows** are the camera-local basis vectors expressed in world coordinates:

- row 0 = camera right
- row 1 = camera up (within the view)
- row 2 = camera +Z; the **look direction is −row 2** (the camera looks down its local −Z)

Stage 2's WASD math extracts exactly these rows on the CPU side. (An earlier revision of this doc said "columns", which produced a wrong forward vector in the first implementation pass — fixed in `ae64788`. With our slot mapping, `forward = −row 2 = (sin P·sin Y, −sin P·cos Y, −cos P)`.)

## Stage 2 — Position + input (shipped)

### Data layer

**`FractalConfig` fields** (`src/config/fractal_config.rs`):

- `camera_x: f32`, `camera_y: f32` alongside the existing `camera_z`. World-space distances.
- Defaults `(0.0, 0.0, 0.0)`, `skip_serializing_if` zero — existing `.fflame` JSON files stay clean.

**XML round-trip via JWildfire's `cam_pos_x` / `cam_pos_y` / `cam_pos_z`** (`src/flame_xml.rs`):

- JWildfire writes these on every flame (e.g., `cam_pos_x="0.0" cam_pos_y="0.0" cam_pos_z="0.0"`). Before this stage we dropped them on import.
- Import: each attribute populates its config field.
- Export: each attribute is written only when non-zero (matching JWildfire's conditional output pattern).
- Covered by `test_camera_position_roundtrip` in `src/flame_xml.rs`.

**Out of scope**: `cam_xfocus` / `cam_yfocus` / `cam_zfocus` (orbital focus point). We commit to free-fly. If we ever want to add orbital later, it's a separate flag + math change.

**Legacy interaction**: JWildfire also has the older `cam_zpos` attribute (single-axis Z position, the one our `camera_z` field originally read). As shipped:

- On import: if both `cam_pos_z` and `cam_zpos` are present, `cam_pos_z` wins (the newer field; matches JWildfire's own priority).
- On export: we write `cam_pos_z` and also a duplicate `cam_zpos` for backward compat with older JWildfire and Apophysis versions.
- Internal field stays `camera_z` — same value, two on-disk names.

### Shader (camera_transform)

`camera_transform` in `shaders/core/utilities.wgsl` translates by the full position vector before rotating:

```wgsl
fn camera_transform(p: vec3<f32>, camera_matrix: mat3x3<f32>, camera_pos: vec3<f32>) -> vec3<f32> {
    let p_t = p - camera_pos;
    // ... all nine matrix elements applied to p_t
}
```

`GpuParams` (`src/gpu/buffers.rs`) gained `camera_x` / `camera_y` next to the existing `camera_z`, plus an 8-byte pad (`_pad_before_post_symmetry`) so the `post_symmetry` struct stays on a std140 16-byte boundary. Call sites pass `vec3<f32>(params.camera_x, params.camera_y, params.camera_z)`. Same plumbing pattern as the `camera_bank` work.

A side fix that fell out of testing at high perspective: `apply_perspective` now clips points behind the camera (`zr < 1e-3` returns an off-screen sentinel) instead of letting the Apo formula mirror them across the screen — the "parts behind me projected into the sky" wraparound (`baf1547`). A TODO in the shader comment covers re-adding the original Apo behavior behind a toggle if anyone wants it.

### Input handlers (as built — `src/app/fly_camera.rs`)

**Mouse-look** (drag in the fractal viewport while fly mode is on):

- Drag delta × sensitivity (**radians per pixel**, SystemSettings) updates `camera_rotation_x` (pitch) and `camera_rotation_y` (yaw). Invert-Y toggle flips the vertical sign.
- The drag delta is first converted from screen frame to camera frame to account for the screen `rotation` value (`(dx·cosR + dy·sinR, −dx·sinR + dy·cosR)`), so dragging "right" looks toward whatever is at screen-right even on a twisted view (`514821f`).
- When `pan_x` / `pan_y` are non-zero, each mouse-look event also shifts `camera_pos` by `(M_old^T − M_new^T) · (pan_x, pan_y, 0)` so the rotation pivot stays at the visual screen center instead of the panned-away camera axis. Exact at the focal-plane depth; small residual at other depths under perspective. The user's `pan` and `rotation` config values are never written — compensation is read-only with respect to them.

**Keyboard movement** (`update_fly_camera`, called per-frame from `render()`):

- Press/release of W A S D Q E Shift is tracked in a `fly_keys_held` set (`src/app/input.rs` intercepts both edges when fly mode is on; everything else falls through to the normal shortcuts).
- Per-frame integration uses delta-time (`web_time::Instant`, clamped to 0.1 s per step) so speed is frame-rate-independent. The event loop forces continuous redraws while any fly key is held.
- W/S: ± `forward` — the camera-look direction, `−row 2` of the camera matrix
- A/D: ∓/± `right` — `row 0` of the camera matrix, which bakes in `rotation` so D always strafes toward screen-right
- Q/E: ∓/± camera-relative up (screen-down/up, `−row 1`). Shipped as world-up in PR B; switched to camera-relative alongside stage 3 to match the free-look space-sim model
- Shift: multiplies speed by the sprint multiplier

### Activation gating

Shipped as the explicit-toggle option: fly mode is entered/exited via the **F2** hotkey or the 🚀 button in the View panel. When off, WASD does nothing special and all keys go to the UI normally — no risk of eating keystrokes from text inputs.

### UI

- Position sliders (`X` / `Y` / `Z`) in the View panel, alongside the rotation sliders.
- Fly-mode toggle button (🚀) in the View panel; label reflects current state.
- "Fly Mode Settings" collapsing section in the View panel (values persist in SystemSettings, `src/storage/settings.rs`):
  - Mouse sensitivity (radians / pixel, default 0.005 ≈ 0.3°/px)
  - Movement speed (units / second, default 1.0)
  - Sprint multiplier (default 3.0)
  - Invert Y (default off)

### Animation system integration

`ConfigPath::CameraX` / `CameraY` are wired through the same path as `CameraBank`:

- `src/config/delta.rs`: enum variants + Display + I18nKey + storage-key + UpdateType::ViewOnly + coalescing
- `src/config/manager.rs`: apply + get_value handlers
- `src/ui/target_selector.rs`: animation-target list entries
- `src/ui/track_editor.rs`: read-value mapping

## Stage 3 — Free-look mouse-look (shipped)

### Corrected analysis: what gimbal lock actually was here

An earlier revision of this section planned a "matrix round-trip so mouse-look can use world-up yaw instead of our Euler's gimbal-locked slots." That premise was **wrong**, caught during implementation by verifying the algebra numerically before writing code:

- Our camera matrix factors exactly as the ZXZ Euler product `M = Rz(rotation) · Rx(pitch) · Rz(−yaw)` (verified to machine precision against the shipped [`build_camera_matrix`][matrix]).
- Incrementing Euler `yaw` is *exactly* "rotate the camera about world-up" — at every pose.
- Incrementing Euler `pitch` is *exactly* "rotate about the screen-plane axis at angle `rotation` from screen-right" — which the stage-2 drag conversion fed correctly.

So the stage-2 scheme already *was* classic FPS mouse-look, composed exactly in SO(3): smooth everywhere, full-sphere coverage (pitch was never clamped), no instability near the poles. The planned rewrite would have been a no-op at `rotation = 0` and a regression at `rotation ≠ 0` (it dropped the screen-frame conversion that had been tested and approved).

What the FPS scheme actually had — two quirks, both inherent to world-anchored yaw:

1. At the default straight-down pose, horizontal drag spins the screen rather than turning the head (geometric necessity: world-up coincides with the look axis; same as looking at your feet in any FPS).
2. Past a pole (camera upside-down relative to world-up), horizontal drag reverses direction on screen (verified numerically: `+dx` moves the look toward screen-right when right-side-up, screen-left when inverted).

[matrix]: ../../shaders/core/utilities.wgsl

### Decision: free-look (screen-relative)

Chosen over "keep FPS + flip the horizontal sign while upside down" and "keep as-is". Drag rotates the camera about its **own screen axes** at every orientation:

- horizontal drag → rotation about the screen-vertical axis
- vertical drag → rotation about the screen-horizontal axis
- diagonal drag → one rotation about the perpendicular screen-space axis (exponential map — order-free by construction)

Properties: always "turning your head", including at the default pose (no more spin-in-place); never inverts; never locks; full sphere reachable. At the horizon pose (`pitch = π/2`, `rotation = 0`) free-look coincides exactly with the old FPS scheme, so the approved stage-2 feel is preserved where it mattered (unit-tested).

Cost: circular mouse motions accumulate roll (rotation-group holonomy — inherent to any screen-relative scheme), so the `rotation` value drifts during free-look and the horizon can tilt. Re-level with the rotation slider; an auto-level assist can be added later if this annoys in practice.

The stage-2 screen-frame drag conversion is gone: camera-space axes ARE screen axes (the projection uses camera-space x/y directly), so a twisted view gets screen-correct drag directions with no compensation at all.

### Implementation (`src/app/fly_camera.rs`)

Per mouse-look event:

```
read (P, Y, R) from config
M_old = build(P, Y, R)
axis  = (a_pitch, a_yaw, 0) / |·|          // camera space; a_yaw = dx·sens,
angle = |(a_pitch, a_yaw)|                  // a_pitch = dy·sens·invert_sign
M_new = axis_angle(axis, angle) · M_old     // left-multiply = camera space
(P', Y', R') = M_new.to_euler_near((P, Y, R))   // then wrapped into [−π, π]
write P', Y'; write R' only when it changed
pan-pivot compensation with (M_old, M_new)  // unchanged from stage 2
```

No persistent quaternion or matrix between events — the Euler triple in `FractalConfig` remains the source of truth. Written angles are wrapped into `[−π, π]` so the config always stays within the −180°..180° range the View sliders display (the decomposition tracks angles continuously for branch selection, so sustained drags would otherwise walk values past ±π). New `CameraMatrix` methods: `axis_angle` (Rodrigues), `mul`, `to_euler_near`.

Decomposition (away from the poles; exact for the `+acos` branch):

```
P = ±acos(clamp(m22, −1, 1))
Y = atan2(−m20, m21)
R = atan2(m02, −m12)
```

The two-fold ambiguity `(P, Y, R) ≡ (−P, Y+π, R+π)` and all 2π wraps are resolved by scoring each candidate's summed wrapped distance to the prior triple and keeping the closer one. Within `|sin P| < 1e-3` of a pole, `Y` and `R` alias into one angle; we hold `R` at its prior value and put the aliased angle into `Y` (at the poles `m00 = cos(Y∓R)`, `m01 = sin(Y∓R)`; verified by substituting `sin P = 0` into `build`). The 1e-3 threshold also covers f32 `acos` resolution near ±1 (~5e-4).

**Chart-singularity caveat — the residue of gimbal lock, confined to where it's harmless**: the *view* is smooth and faithful everywhere (`to_euler_near` guarantees the returned triple rebuilds the exact same matrix), but the stored P/Y/R values can reshuffle discontinuously when the camera passes near straight-down/straight-up — the longitude-at-the-north-pole effect. Example: a tiny free-look yaw from the exact default pose legitimately decomposes as `(P=δ, Y=90°, R=90°)`. Sliders may jump while the rendered view moves continuously. Unavoidable while storing Euler angles (a JWF-compatibility constraint we keep).

### Tests (6, in `fly_camera.rs`)

- `build_is_rotation_matrix` — orthonormality across an angle grid
- `axis_angle_matches_euler_increments` — pins the two sign-convention identities everything rests on (Euler yaw ≡ world-up rotation; Euler pitch ≡ screen-plane-axis rotation)
- `to_euler_round_trip_grid` — decompose ∘ build = identity away from poles
- `pole_holds_rotation_and_reconstructs` — `R` held at both poles, matrix faithfully rebuilt
- `free_look_through_pole_is_smooth_and_faithful` — 120-step simulated drag path: pitch straight through the pole, then diagonal (roll-drift), then pure yaw; asserts per-step matrix continuity and faithful reconstruction, feeding the rebuilt orientation forward exactly like the real code
- `free_look_yaw_matches_euler_yaw_at_horizon` — free-look ≡ old FPS feel at the horizon pose

### Out of scope for stage 3

- **Quaternion type / glam dep** — 3×3 matrices + Rodrigues suffice; keeps the code surface small.
- **Slider behavior** — View panel sliders still write Euler directly; only mouse-look round-trips through SO(3).
- **Auto-level assist** — a "snap horizon level" button/behavior for accumulated free-look roll; add later if wanted.
- **Other input sources** — animation tracks, preset loads, undo/redo, XML import all continue to write Euler directly and never enter the mouse-look round-trip.

## Camera modes (FreeLook | FPS)

Both mouse-look schemes are available as `SystemSettings::fly_camera_mode`
(a device input preference like sensitivity — deliberately NOT part of
`FractalConfig`). Selector lives in View → Fly Mode Settings; persisted
via `ConfigPath::SystemFlyCameraMode` with string transport
(`"free_look"` / `"fps"`) so future modes (orbital) don't need a new
value type.

| | FreeLook (default) | FPS |
|---|---|---|
| Drag axes | camera screen axes (space-sim) | world-up yaw + screen-plane pitch |
| Horizon | can roll (mouse circles accumulate roll) | always level (XY plane anchored) |
| `rotation` | drifts during look | never written |
| Gimbal | none anywhere | inverts horizontal drag past straight-down/up; spins in place at the straight-down pose |
| Q/E | screen-down/up | world-down/up |
| Implementation | SO(3) round-trip (`to_euler_near`) | plain Euler increments (which ARE the world-anchored rotations — pinned by `axis_angle_matches_euler_increments`) with the drag pre-rotated by `rotation` |

W/S (look axis) and A/D (screen-right) are identical in both modes.

## Pan semantics unified across 2D/3D

The 2D pipeline composes pan → rotate → zoom (Apophysis convention:
Pan X/Y are a position in the fractal plane). 3D used to pan in
post-projection screen coordinates (JWildfire's 3D convention), so the
same pan values showed different locations when toggling render modes
with rotation set — the same jump JWF exhibits when perspective crosses
zero. Apophysis is the internally-consistent one; we unified on it.

Mechanism: the roll factor is outermost in the camera chain and never
touches camera-space z, so it commutes exactly with the perspective
divide — rolling inside the matrix ≡ rotating the projected 2D point.
`world_to_pixel_3d` now passes roll = 0 into the matrix and applies the
identical pan → rotate → zoom block as `world_to_pixel`. Renders are
unchanged whenever pan = 0 or rotation = 0. This deliberately diverges
from JWF's 3D pan; `FractalConfig::screen_delta_to_pan_frame` is the
single screen→pan conversion used by every input path.

## JWildfire cam_pos semantics (documented divergence)

From JWF source (`output/FlameRendererView.java`): JWF applies
`cam_pos_x/y/z` AFTER rotation, in camera space, added — plus an extra
`+ camPosZ` term in the perspective denominator. Ours is a true
world-space camera position applied before rotation, which is what
free-fly needs. Flames with non-zero cam_pos render differently here
vs JWF. An exact conversion exists for the position part
(`c_ours = −M_eff^T·c_jwf`) but JWF's perspective term has no
counterpart in our projection, so a boundary conversion can only be
faithful for orthographic flames — skipped for now; formula documented
at the import site in `src/flame_xml.rs`. (Same source reading
confirmed our behind-camera clip matches JWF: `if (zr < EPSILON)
return false`.)

## Open questions

- **Movement speed scaling**: should speed scale with the visible fractal extent (zoom)? An FPS speed of 1 unit/second feels different at zoom=1× vs zoom=100×. Probably scale by `1 / zoom` so it stays comfortable at any magnification. (Deliberately left out of PR B; revisit after real use.)
- **Mouse-look while not in fly mode**: should a right-click drag rotate the camera even without entering fly mode? Many 3D apps do this. Decide after fly mode has seen real use.
- **Mouse capture / cursor hiding during fly mode**: deferred from PR B (complex on WASM). Drag-based look works without it.

## Out of scope

- **Orbital camera** (JWildfire's `cam_xfocus/yfocus/zfocus`). Free-fly only for now. Orbital would be a separate mode with different mouse-drag and WASD math.
- **Camera path animation** (auto-fly along a saved path). That's a future "tour mode" feature, separate from manual fly-mode.

## Related

- [`jwf-features.md`](jwf-features.md) — JWF camera rotation entry (shipped with stage 1)
- [`../experimental/PROPOSAL-true-3d-affine.md`](../experimental/PROPOSAL-true-3d-affine.md) — unrelated, but in the same general "make 3D rendering more capable" theme
