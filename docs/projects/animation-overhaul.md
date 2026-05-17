# Animation system overhaul (subflame-aware, stable identity, complete affine ops)

## Goal

Bring the Animation system back into alignment with the rest of the app
after the subflame live-editing refactor. Three concrete user-visible
problems:

1. **No flame targeting per track.** Tracks animate "whatever flame is
   currently selected in the editor" because `update_param_silent`
   routes via `ConfigManager.editing_target`. Loading an animation
   while editing a subflame silently writes the keyframes to the
   subflame's transforms — and vice versa.
2. **Incomplete affine high-level ops.** Rotation / Scale / Origin X /
   Origin Y exist only for **normal-pool pre-affines**
   ([`src/config/delta.rs:100-109`](../../src/config/delta.rs)). Final
   pre, linked pre, and all three pools' post-affines are
   unanimable at this level — you can only animate raw `a..g`.
3. **Fragile transform identity.** Track targets reference transforms
   by index (`Transform.{N}.…`). Adding, deleting, or reordering
   transforms in the editor silently breaks animations. The existing
   `Animation::on_transform_removed`
   ([`src/animation/mod.rs:253`](../../src/animation/mod.rs))
   uses string-prefix matching to shift indices on delete, but
   doesn't handle reorder and doesn't know about pool moves
   (normal ↔ linked ↔ final).

Plus the surrounding work to support those fixes:

- **UI** — the target selector
  ([`src/ui/target_selector.rs`](../../src/ui/target_selector.rs))
  only enumerates the current main flame's pools. It needs to walk
  every flame (Main + each subflame) so users can pick targets across
  the hierarchy.
- **Migration** — every existing `.anim` file targets the main flame
  by construction (subflame targeting didn't exist). Migration must
  preserve that.

## Current state

### Track model
[`src/animation/mod.rs:106-117`](../../src/animation/mod.rs)
```rust
pub struct Track {
    pub target: String,           // stringified ConfigPath via to_string_key()
    pub source: TrackSource,      // Keyframes | Signal
    pub interpolation: Interpolation,
}
```
No flame target. Path strings are flat (`"Transform.0.Rotation"`,
`"Zoom"`, `"BlendFactor"`).

### Apply path
[`src/app/animation_update.rs:122-143`](../../src/app/animation_update.rs)
```rust
for (path_str, json_value) in frame_values {
    if let Some(path) = ConfigPath::from_string_key(&path_str) {
        if let Some(value) = json_to_config_value(&json_value, &path) {
            self.config_manager.update_param_silent(path, value)?;
        }
    }
}
```
`update_param_silent` → `set_value` → `active_flame_mut()` which
reads `self.editing_target`. **That's the bug.** Animation has no say
in which flame it writes to.

### Target selector
[`src/ui/target_selector.rs:208-253`](../../src/ui/target_selector.rs)
walks `flame.transforms`, `flame.linked_transforms`,
`flame.final_transforms` of the **currently active flame only**. No
visibility into subflames.

### Affine high-level ops coverage
Currently registered ([`src/config/delta.rs:99-110`](../../src/config/delta.rs)):
- `TransformOriginX`, `TransformOriginY`, `TransformRotation`, `TransformScale`

All four apply only to the **normal pool's pre-affine**
([`src/config/delta.rs:424-433`](../../src/config/delta.rs)). Five
slots are uncovered:

| Pool   | Pre-affine | Post-affine |
|--------|------------|-------------|
| Normal | ✅ existing | ❌ missing  |
| Linked | ❌ missing | ❌ missing  |
| Final  | ❌ missing | ❌ missing  |

Each missing slot needs the four ops: Origin X, Origin Y, Rotation, Scale.
Five slots × 4 ops = **20 new ConfigPath variants**.

## Design

### Per-track flame targeting

Add a `flame_target` field on `Track`. Keep path strings as they are
(flat, no flame prefix) — the field carries the scoping.

```rust
pub struct Track {
    pub target: String,
    #[serde(default)]                              // ← old files: Main
    pub flame_target: EditingTarget,
    pub source: TrackSource,
    pub interpolation: Interpolation,
}
```

This means every existing `.anim` file deserializes with
`flame_target = Main`, which matches old behavior exactly. No path
rewriting needed.

The apply path becomes:

```rust
for track in &animation.tracks {
    let path = ConfigPath::from_string_key(&track.target)?;
    let value = …;
    self.config_manager.update_param_silent_on(
        track.flame_target, path, value,
    )?;
}
```

`update_param_silent_on(target, path, value)` is a new ConfigManager
helper that routes via `target_flame_mut(target)` (already exists) for
the duration of the apply, regardless of what `self.editing_target`
is. The existing `update_param_silent(path, value)` becomes a thin
wrapper: `update_param_silent_on(self.editing_target, path, value)`.

#### Alternative considered: encode target in path string

`"Main.Transform.0.Rotation"` vs `"Subflame.0.Transform.0.Rotation"`.
Rejected because: it complicates path parsing, requires migration of
every existing track's string, and `ConfigPath` already encodes a
typed identity that's better expressed as a separate field than
folded into a string key.

### Stable transform IDs

Index-based references break under any structural change. Replace
with stable IDs:

```rust
pub struct Transform {
    #[serde(default = "Transform::new_id")]
    pub id: u64,
    // ... existing fields
}

impl Transform {
    fn new_id() -> u64 { /* monotonic counter on creation */ }
}
```

New ID-based ConfigPath variants alongside existing index variants:
- `TransformAffineById { id, param }`, `TransformWeightById { id }`, etc.
- Mirror for Linked / Final / Subflame pools.

Resolution at apply time: walk the target flame's transforms looking
for the ID, return `InvalidIndex` if not found.

Animation tracks use ID-based paths exclusively going forward.
Index-based paths still work for the UI's own use (Triangle Editor,
Transforms panel) where the user *is* operating on "the third
transform in the list" by visual position.

#### `on_transform_removed` etc. become redundant for animations

These helpers were patching string-encoded indices on
delete/reorder. With ID-based paths in animation tracks, structural
changes don't invalidate targets. A track whose target ID no longer
exists is a *broken* track (visibly so, see UI plan below), not a
silently-rewritten one.

We keep `on_transform_removed` until all tracks have migrated to IDs;
afterwards we can delete it. Same for `on_color_effect_removed` /
`on_density_effect_removed` — these are about effects not transforms,
but the same pattern applies if we want stable effect IDs (not in
scope for this overhaul).

### Missing affine high-level ops

Add 20 ConfigPath variants in four families:

```rust
// Normal post-affine
TransformPostAffineOriginX { index }
TransformPostAffineOriginY { index }
TransformPostAffineRotation { index }
TransformPostAffineScale { index }

// Linked pre + post (8 variants)
LinkedTransformOriginX, LinkedTransformOriginY,
LinkedTransformRotation, LinkedTransformScale,
LinkedTransformPostAffineOriginX, …

// Final pre + post (8 variants)
FinalTransformOriginX, …, FinalTransformPostAffineScale
```

The 4-tuple (OriginX, OriginY, Rotation, Scale) decomposes an affine
into intuitive geometric parameters. The existing
`TransformRotation` path applies the decomposition and re-composes
the affine on write
([`src/config/delta.rs:430-433`](../../src/config/delta.rs) and
its set_value handler) — same machinery, pointed at a different
affine slot. The implementation is largely a copy-paste with a
different field selector.

### Subflame transform paths

Subflames already have full transform pools but no ConfigPath
variants targeting them today (subflame transforms are reached via
`active_flame()` while `editing_target = Subflame{i}`). Once
`flame_target` lives on the Track, the *path* can stay scoped to
"the active flame" — i.e., we don't need
`SubflameTransformAffine{subflame_idx, transform_idx, param}`
variants. The track's `flame_target` selects the flame; the existing
`TransformAffine{index, param}` (or `TransformAffineById{id, …}`)
selects within it.

This means: no new ConfigPath variants for subflame addressing. The
20 new affine ops listed above are for completeness across pools,
not for subflame access.

### UI

#### Target selector
Group categories hierarchically by flame:

```
Main flame
  ├─ View
  ├─ Color
  ├─ ...
  ├─ Transform 1
  └─ Linked 1
Subflame 0 (name)
  ├─ Transform 1
  ├─ Transform 2
  └─ Final 1
Subflame 1 (name)
  └─ Transform 1
```

Selection writes `Track.target` + `Track.flame_target` together.

View / Color / Tone / Rendering / Effects (the non-transform
categories) only appear under Main — there's no per-subflame view
state. (Subflames have their own color/palette via `subflame_wf`,
but the bulk of the per-flame state lives on the parent.)

#### Track row
Show the flame target as a prefix in the track row label, e.g.
`"Subflame 0 / Transform 2 / Rotation"`. Currently we strip
`Transform.N.` to `Transform.{N+1}.` for 1-based display
([`src/ui/track_editor.rs:91-100`](../../src/ui/track_editor.rs)) —
extend that to include the flame prefix.

#### Broken track visualization
If a track's `(flame_target, path)` resolves to nothing — flame index
out of range, transform ID not found, etc. — show the row with a
warning icon and disable evaluation. Don't auto-delete; the user
might want to fix it manually (rebind to a different target).

Add a "Tracks targeting missing parameters" tally in the Animation
panel header so broken tracks are visible at a glance.

### Migration

#### Old animations: `.anim` file format
Old files have:
```json
{ "tracks": [{ "target": "Transform.0.Rotation", "source": …, "interpolation": … }] }
```

New files have:
```json
{ "tracks": [{
    "target": "Transform.0.Rotation",
    "flame_target": "Main",
    "source": …,
    "interpolation": …,
}] }
```

`#[serde(default)]` on `flame_target` with default = `Main` covers
old files. No file-format version bump needed.

#### Index → ID migration (animations applied at load)
On load, after both the embedded `base_config` and the animation are
deserialized, walk every track:
1. Parse the path string.
2. If it's a `Transform.{N}.X` / `LinkedTransform.{N}.X` /
   `FinalTransform.{N}.X` variant, resolve `N` in the corresponding
   pool of the track's `flame_target` flame.
3. Replace the path string with the ID-based variant.

Tracks whose index doesn't resolve (out of range) stay as-is and
will surface as broken in the UI.

For animations *without* an embedded `base_config`, the migration
happens against whatever fractal is currently loaded. Same logic.

#### Transform IDs
On load of any `FractalConfig`, walk all four pools (main + each
subflame, normal + linked + final) and assign IDs to any
transform with `id == 0` (the serde default). Use a monotonic
counter scoped to the config. Newly-created transforms also get
unique IDs via `Transform::new()` /
[`scene::transforms::Transform`](../../src/scene/transforms.rs).

## Phasing

Three PRs. Each is independently shippable; PR 1 is the only one
that's strictly required for the user-visible fix.

### PR 1 — Per-track flame targeting (the core fix)

**What:** Track gets `flame_target: EditingTarget`. Apply path
honors it. Target selector shows subflame pools. Existing
animations migrate via serde defaults.

**Files:**

| File | Change |
|---|---|
| [`src/animation/mod.rs`](../../src/animation/mod.rs) | `Track.flame_target` field with `#[serde(default)]`. Update `Track::new`, `Track::constant`, `Track::linear`, `Track::signal`, `Track::signal_with_smoothing` to take a target (default `Main` for backwards-compatible constructors). |
| [`src/config/manager.rs`](../../src/config/manager.rs) | New `update_param_silent_on(target, path, value)`. Existing `update_param_silent` becomes thin wrapper. |
| [`src/app/animation_update.rs`](../../src/app/animation_update.rs) | `apply_animated_values` calls `update_param_silent_on` with `track.flame_target`. |
| [`src/animation/controller.rs`](../../src/animation/controller.rs) | `evaluate_at_time` returns `Vec<(EditingTarget, String, Value)>` instead of `(String, Value)`. |
| [`src/animation/export.rs`](../../src/animation/export.rs) | Same return-type change propagates through export. |
| [`src/ui/target_selector.rs`](../../src/ui/target_selector.rs) | New top-level grouping by flame. Categories per flame. Returns `(EditingTarget, ConfigPath)` instead of just `ConfigPath`. |
| [`src/ui/track_editor.rs`](../../src/ui/track_editor.rs) | Use the new selector return. Display target prefix in track row labels. Broken-track warning. |
| [`src/ui/animation_panel.rs`](../../src/ui/animation_panel.rs) | Header tally of broken tracks. |

**Migration:** Pure serde-default. No config file changes.

**Test plan:**
- Load an old `.anim` file, verify all tracks land on Main.
- Create a subflame, add a track targeting Subflame 0 / Transform 0
  Rotation, play it — verify the subflame's transform rotates and
  the main flame doesn't.
- Save the animation, reopen, verify `flame_target` round-trips.
- Animation playback while user switches `editing_target` should not
  affect which flame the tracks write to (formerly a bug).

**Scope:** ~600-800 LOC across 7-8 files. The bulk is the UI
selector rework — the apply-path change is small.

### PR 2 — Stable transform IDs

**What:** Add `id: u64` field on `Transform`. Add `*ById` ConfigPath
variants. Migrate animation tracks from index-based paths to ID-based
paths on animation load. Index-based paths still exist for in-editor
use (Transforms panel, Triangle Editor).

**Files:**

| File | Change |
|---|---|
| [`src/scene/transforms.rs`](../../src/scene/transforms.rs) | `Transform.id`, monotonic counter, on-load assignment for `id == 0`. |
| [`src/config/delta.rs`](../../src/config/delta.rs) | New `*ById` variants for every existing index-based transform path (normal/linked/final, pre/post, affine/rotation/scale/origin/weight/color/color_speed/opacity/variation/variation_param). Apply/get handlers resolve ID → index, then dispatch to existing logic. |
| [`src/animation/mod.rs`](../../src/animation/mod.rs) | On `Animation::from_json`, migrate index-based paths to ID-based paths (where the ID is resolvable against `base_config` or the live config). |
| [`src/app/animation_update.rs`](../../src/app/animation_update.rs) | Broken-track logging when ID doesn't resolve. |

**Migration:** Sequential ID assignment on first load. Animation
files with index paths transparently rebind to IDs.

**Test plan:**
- Load an animation, add a transform before the one being animated,
  verify the animation still targets the original transform.
- Delete an animated transform, verify the track is flagged broken
  (not silently retargeted).
- Round-trip a config + animation: save with IDs, reload, verify
  IDs are preserved.

**Scope:** ~1500-2000 LOC. The ConfigPath additions are mechanical
but broad. The hardest part is the migration to make sure every
track-loading site does the rebind.

**Depends on PR 1?** Yes for cleanliness: doing IDs first then
adding `flame_target` would force two migration passes on every
animation file. With PR 1 already shipped, this is a clean
single-pass ID migration.

### PR 3 — Missing affine high-level ops

**What:** Add the 20 missing variants (Origin/Rotation/Scale for the
five uncovered affine slots).

**Files:**

| File | Change |
|---|---|
| [`src/config/delta.rs`](../../src/config/delta.rs) | 20 new ConfigPath variants. Apply / get / undo / update_type / string-key parse / display all extend. Pattern matches the existing `TransformOriginX` etc. handlers. |
| [`src/ui/target_selector.rs`](../../src/ui/target_selector.rs) | Surface the new ops in the post-affine sub-sections of normal/linked/final transforms. |

**Migration:** None — purely additive.

**Test plan:**
- Animate a final-pool transform's post-affine rotation, verify the
  rendered output rotates that affine over time.
- Round-trip JSON of the new ConfigPath variants.
- All four ops (Origin X/Y, Rotation, Scale) covered for all five
  new slots.

**Scope:** ~400-600 LOC. Almost entirely mechanical extension of
existing patterns.

**Depends on PR 1 or 2?** No — independent. Could ship at any time.
Easiest standalone.

## Phasing rationale

The order I'd recommend: **PR 1 → PR 3 → PR 2.**

- **PR 1 first** because it's the user-reported pain point: animations
  silently writing to the wrong flame. Everything else is secondary.
- **PR 3 second** because it's small, additive, and useful. Doing it
  before PR 2 means PR 2's ID migration covers the new variants too,
  rather than needing a second migration pass.
- **PR 2 last** because it's the largest, has the broadest blast
  radius, and benefits from PR 1's `flame_target` already being in
  place (so the ID migration knows which flame to resolve indices
  against — there'd be ambiguity otherwise).

Alternative: bundle PR 1 + PR 3 into one PR. Both are small enough
that splitting is a judgment call. I'd lean separate because the
review surface is cleaner (selector UI changes in one, ConfigPath
plumbing in the other), but happy to combine if the user prefers
fewer review rounds.

## Out of scope

- **Per-subflame palettes / per-subflame tone mapping.** Currently a
  subflame's only color contribution to the parent is via the
  `subflame_wf` variation's blend, with no independent palette. If
  that changes, animation will need to follow — but it's a subflames
  feature, not an animation feature.
- **Nested subflames.** Tracked in
  [`docs/projects/subflames.md`](subflames.md). The animation system
  inherits whatever model that lands.
- **Stable effect IDs.** ColorEffect / DensityEffect have the same
  index-fragility as transforms. The pattern from PR 2 would extend
  cleanly, but isn't in scope here — flag for a follow-up.
- **Animation of subflame creation/deletion** (structural changes).
  Animations operate on parameter values, not structural mutation.
  Adding/removing transforms or subflames is a separate gesture.
- **Per-track validation/health at load time, beyond "broken track"
  marking.** Things like checking that a Rotation track's keyframe
  values are reasonable, or that a Signal track references an
  existing signal generator. Out of scope; surface as warnings if
  cheap, defer otherwise.

## Risks

| Risk | Mitigation |
|---|---|
| ID assignment collisions on cross-config copy (paste a transform from one config to another, IDs collide) | Counter is per-config; on import, walk the imported transforms and reassign any ID that's already taken in the destination. |
| Migration silently drops tracks whose index doesn't resolve on load (e.g. animation loaded against a fractal with fewer transforms than the animation expects) | Don't drop — keep the original path, mark broken in UI. User can rebind. |
| `EditingTarget` is currently in `src/config/manager.rs`. Track needs to reference it. Pulling it into a shared module may break public API. | Re-export from `crate::animation` if needed; otherwise move it to `src/config/mod.rs`. Internal-only enum so this is safe. |
| UI selector becomes too crowded with many subflames | Hierarchical collapsing (each flame is one collapsible group); only Main expands by default. Existing search filter
([`src/ui/target_selector.rs:104-112`](../../src/ui/target_selector.rs)) cuts across all flames. |
| Animation export captures `flame_target` but a downstream consumer doesn't honor it | Export tests should round-trip a multi-flame-target animation through render-to-PNG and verify the right flame moved. |
| Old `on_transform_removed` and friends keep running but become inconsistent with ID-based tracks | After PR 2, these helpers either no-op for ID-based tracks or are deleted entirely. Add a `#[deprecated]` note as part of PR 2. |

## Decisions

1. **`flame_target` lives on `Track`** (per-track, not per group).
   Flexible enough to support mixed-target animations; UI can collapse
   the noise by grouping rows visually by target.
2. **Deleting a subflame marks its tracks broken** — same policy as
   transform deletion under PR 2. Don't drop, don't silently rebind.
3. **Audio export honors `flame_target`** — it goes through the same
   apply path, so PR 1's `update_param_silent_on` covers it.
