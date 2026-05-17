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

### Stable transform IDs (runtime-only)

Index-based references break under in-session structural changes
(add / delete / reorder). The save file is fine — indices are static
once written — so IDs are needed *only* during an editing session.
Don't persist them.

```rust
pub struct Transform {
    #[serde(skip, default = "next_transform_id")]
    pub id: u64,
    // ... existing fields
}

pub struct Track {
    pub target: String,                 // stays index-based
    pub flame_target: EditingTarget,
    #[serde(skip)]
    pub bound_id: Option<u64>,          // resolved on load, dropped on save
    // ... existing fields
}
```

**Flow:**

1. **On load** of `FractalConfig`: walk every pool (normal / linked /
   final, on Main and each subflame) and assign IDs to any transform
   whose `id == 0` (the serde default). Monotonic counter.
2. **On load** of `Animation`: for each track, parse its target,
   find the referenced transform in the current config, store its ID
   as `bound_id`. Tracks with non-transform paths (View, Color, etc.)
   leave `bound_id = None`.
3. **On structural change** (add / delete / reorder in the editor):
   walk all tracks with a `bound_id`; find that ID in the current
   pool; rewrite the index inside `track.target` to the new
   position. ID gone → leave as-is, mark broken.
4. **On save**: `bound_id` is skipped by serde. `track.target` is
   already current.

**No new ConfigPath variants.** No `*ById` parallel enum. The existing
`Transform.{N}.X` string path is the source of truth on disk; IDs are
purely an in-flight handle so we can rewrite the string when N
shifts.

**Existing `on_transform_removed` etc. get replaced.** Today those
helpers do string-prefix matching to shift indices on delete. With
ID-keyed rebinding, the same hooks handle delete + reorder + pool
moves uniformly, and tracks that lose their target surface as broken
in the UI instead of being silently retargeted to the wrong slot.

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

#### Bind-on-load (in-memory only)
On load, after the `FractalConfig` and `Animation` are deserialized:
1. Walk every pool (main + each subflame, normal + linked + final),
   assign a fresh ID to any transform whose `id == 0`.
2. Walk every track: parse its target string, find the referenced
   transform in the track's `flame_target` flame, store its ID as
   `bound_id`. Tracks with no resolvable transform leave
   `bound_id = None` and surface as broken.

Newly-created transforms (from `Transform::new()` /
[`scene::transforms::Transform`](../../src/scene/transforms.rs))
also get fresh IDs from the same counter.

No file format changes. No migration logic for existing `.fflame` /
`.anim` files — they round-trip through serde untouched. IDs are
session-local; a save written today loads identically tomorrow
because indices in the file were correct by construction at save
time.

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

### PR 2 — Stable identity for every index-based list

**What:** Add session-local IDs to every list whose entries can be
animated: the three transform pools, the subflames list, and the two
effect lists. Each animation track gets a `bound` field (runtime-only)
that records the ID of the thing it's targeting. When the user adds /
deletes / reorders any of these lists, a rebind hook rewrites the
index inside `track.target` (and inside `track.flame_target` for
subflame moves) so the track keeps pointing at the same item.

Track strings stay index-based on disk; IDs are the in-memory handle
that lets the rewrite happen. ConfigPath does not gain new variants.

**Six lists in scope:**

| List | Items get `id: u64` on | `Track.bound` variant |
|---|---|---|
| `flame.transforms` | `Transform` | `TargetBinding::Transform(id)` |
| `flame.linked_transforms` | `Transform` (same struct) | `TargetBinding::Linked(id)` |
| `flame.final_transforms` | `Transform` (same struct) | `TargetBinding::Final(id)` |
| `flame.subflames` | `Flame` | `TrackBinding.flame: Option<u64>` |
| `color_effects` | `ColorEffect` | `TargetBinding::ColorEffect(id)` |
| `density_effects` | `DensityEffect` | `TargetBinding::DensityEffect(id)` |

**Deferred:** `Xaos { src, dst }` tracks and `SoloTransform` tracks
both reference transforms-by-index but aren't commonly animated;
they'll continue to silently misbind when transforms reorder.
Follow-up if it becomes an issue.

**Track binding model:**

```rust
pub struct Track {
    pub target: String,                  // on-disk: "Transform.0.Rotation"
    pub flame_target: EditingTarget,     // on-disk: Main | Subflame{i}
    #[serde(skip)]
    pub bound: TrackBinding,             // runtime: what to follow
    // ...
}

#[derive(Default, Clone, Copy)]
pub struct TrackBinding {
    /// Subflame ID when flame_target is Subflame{i}. None for Main.
    pub flame: Option<u64>,
    /// The list-item the path resolves to. None for non-list paths
    /// (View, Color, Tone, etc.).
    pub target: Option<TargetBinding>,
}

#[derive(Clone, Copy)]
pub enum TargetBinding {
    Transform(u64),
    Linked(u64),
    Final(u64),
    ColorEffect(u64),
    DensityEffect(u64),
}
```

**Flow:**

1. **ID assignment.** Process-global atomic counter. `Transform::new()`,
   `Flame::new()`, `*Effect::new()` each pull a fresh ID. A
   `fixup_ids(config: &mut FractalConfig)` pass runs after any
   `FractalConfig` deserialize and assigns IDs to anything with
   `id == 0` (the serde-skip default), walking every pool of every
   flame plus the two effect lists.
2. **Bind-on-load.** When an `Animation` is loaded (against a
   `FractalConfig` that's already had IDs assigned), walk every track:
   parse the `target` and `flame_target`, look up the referenced item,
   store its ID in `track.bound`. Tracks whose target is non-list
   (View, Color, Tone, Rendering) leave `bound = Default::default()`.
3. **Rebind-on-mutation.** After any structural change to one of the
   six lists, the rebind hook walks all tracks:
   - For each track with a `bound.flame = Some(id)`: find the subflame
     with that id in `config.flame.subflames`; if found, rewrite the
     index inside `track.flame_target = Subflame{new_index}`.
   - For each track with a `bound.target = Some(TargetBinding::X(id))`:
     find item with that id in the relevant pool *of the bound flame*;
     if found, rewrite the index inside `track.target`.
   - If an id doesn't resolve: leave both fields as-is. Track is
     "broken" in the UI sense (visibly flagged) but `bound` stays set
     so a future undo / restore can naturally rebind it.

**Files:**

| File | Change |
|---|---|
| [`src/scene/transforms.rs`](../../src/scene/transforms.rs) | `Transform.id: u64` with `#[serde(skip)]`; `Flame.id: u64` same. Process-global atomic counter (`next_id()`). `Transform::new()` and `Flame::new()` allocate. |
| [`src/effects/`](../../src/effects/) | `ColorEffect.id`, `DensityEffect.id` same treatment. |
| [`src/config/fractal_config.rs`](../../src/config/fractal_config.rs) | `fixup_ids(config: &mut FractalConfig)` walks every pool + every subflame + effect lists, allocates fresh IDs where `id == 0`. Called from any place we load a `FractalConfig` from disk (config loader, preset loader, animation `base_config` load). |
| [`src/animation/mod.rs`](../../src/animation/mod.rs) | `Track.bound: TrackBinding` with `#[serde(skip)]`; `TrackBinding` and `TargetBinding` enum. Bind-on-load helper invoked from `AnimationController::load`. |
| [`src/config/manager.rs`](../../src/config/manager.rs) | New `rebind_animation_tracks(animation: &mut Animation)` method on `ConfigManager`. Called from `add_transform` / `delete_transform` / `clone_transform` / reorder paths for normal/linked/final pools, from `add_subflame` / `delete_subflame`, from `AddColorEffect` / `RemoveColorEffect` (& density), and from `undo()` / `redo()` after snapshot restore. |
| [`src/ui/track_editor.rs`](../../src/ui/track_editor.rs) | `is_track_broken` switches to checking `bound` against the current config (resolves the bound IDs; broken if either flame or target ID doesn't resolve). |
| [`src/animation/mod.rs`](../../src/animation/mod.rs) | Delete the now-redundant `Animation::on_transform_removed`, `on_color_effect_removed`, `on_color_effect_reordered`, `on_density_effect_removed`, `on_density_effect_reordered`. Their job is fully subsumed by ID-keyed rebinding. |

**Migration:** None on disk. IDs are session-local, assigned fresh
on every load. Old `.fflame` / `.anim` files round-trip through serde
untouched. `ConfigPath` enum unchanged.

**Interaction with ConfigManager undo/redo:** Two rules make this
clean:

1. **IDs are normal Rust fields** (`#[serde(skip)]` only affects I/O).
   `DeleteTransform` snapshots clone the Transform with its `id`
   intact; undo restores it. The undo machinery doesn't need to know
   IDs exist.
2. **Never auto-clear `bound`.** Broken state is purely "the ID
   doesn't currently resolve." If undo later restores the missing
   item with the same id (which it will, since snapshots preserve
   the field), the next rebind tick re-resolves and the track is
   automatically un-broken.

The rebind hook fires after the mutation in `undo` / `redo` /
`apply_structural_change` — same one-line call as the structural-edit
sites.

**Test plan:**
- Add a transform *before* an animated transform → track's `target`
  index updates; animation keeps targeting the same xform.
- Delete an animated transform → track flagged broken; do not write to
  a different transform that slid into the deleted index.
- Reorder transforms → tracks follow.
- Delete a transform then undo → track rebinds automatically.
- Delete an animated subflame → broken; undo → rebinds.
- Same suite for ColorEffect and DensityEffect (add / delete / reorder
  / undo).
- Save mid-session after rebinding → file has current indices → reload
  → bind-on-load reproduces the same bindings.

**Scope:** ~400-600 LOC total. Mostly the rebind hook +
finding every structural-mutation site (~10 call sites). No
ConfigPath enum changes, no file format changes, no migration logic.

**Depends on PR 1?** Yes, for `flame_target` — the rebind needs to
know which flame's pool to scan when resolving a track's ID. With
PR 1 shipped, this drops in cleanly.

**Commit structure:**
1. `Transform.id` + `Flame.id` + counter + `fixup_ids` for transform pools and subflames
2. `*Effect.id` + extend `fixup_ids` to cover effects
3. `Track.bound` + `TrackBinding` types + bind-on-load
4. Rebind hook on `ConfigManager` + wire into all structural mutation sites + undo/redo
5. Delete redundant `on_*_removed` / `on_*_reordered` string-shifters; update `is_track_broken`

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

The order shipped: **PR 1 → PR 3 → PR 2.** PR 1 and PR 3 are already
merged.

- PR 1 first because it's the user-reported pain point: animations
  silently writing to the wrong flame.
- PR 3 second because it's small, additive, and unblocks animating
  Origin / Rotation / Scale on every affine slot.
- PR 2 last because it's purely an in-session safety net. The
  runtime-only ID design means there's no file format coupling and
  no migration concern, so it can land independently whenever.

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
| ID counter is a process-global static — fine within one process; can collide if a config is moved across processes (e.g. paste from clipboard). | Counter only needs uniqueness within one config in-flight. On any deserialize (config load, transform import), reassign IDs that are already used in the destination. Cheap O(N) walk. |
| Track loaded against a fractal with fewer transforms than the animation expects → `bound_id` resolves to `None` for some tracks. | Don't drop — leave `target` as-is, mark broken in UI. User can rebind by re-picking the target. |
| `EditingTarget` is in `src/config/manager.rs`. Track needs it. Pulling it into a shared module may break public API. | Already re-exported from `src/config/mod.rs` in PR 1. |
| UI selector becomes too crowded with many subflames. | Hierarchical collapsing (each flame is one collapsible group); only Main expands by default. Existing search filter ([`src/ui/target_selector.rs:104-112`](../../src/ui/target_selector.rs)) cuts across all flames. |
| Animation export captures `flame_target` but a downstream consumer doesn't honor it. | Export tests should round-trip a multi-flame-target animation through render-to-PNG and verify the right flame moved. |
| `Animation::on_transform_removed` (string-prefix shifting) and the new ID-keyed rebind both run, doing conflicting things. | PR 2 deletes the old string-prefix helpers; their job is fully subsumed by ID rebinding. |

## Decisions

1. **`flame_target` lives on `Track`** (per-track, not per group).
   Flexible enough to support mixed-target animations; UI can collapse
   the noise by grouping rows visually by target.
2. **Deleting a subflame marks its tracks broken** — same policy as
   transform deletion under PR 2. Don't drop, don't silently rebind.
3. **Audio export honors `flame_target`** — it goes through the same
   apply path, so PR 1's `update_param_silent_on` covers it.
