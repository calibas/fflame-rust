# Subflame live editing (decouple editing target from rendered flame)

## Goal

Let the user edit a subflame's transforms (via Triangle Editor or
xform panel) while the viewport renders the **parent** flame with
the live edits flowing through `subflame_wf`. The post-Phase-6
behavior — selecting a subflame swaps the entire UI/render pipeline
to render that subflame in isolation — is useful as a preview mode
but not as the primary editing experience: the user wants to see
the subflame's effect on the parent's render as they tweak.

## Where we are

- **Phase 6 (committed):** the Subflames panel + `EditingTarget`
  swap mechanism in `ConfigManager`. Selecting a subflame physically
  swaps it into `current.flame`, with the original main stashed
  separately. Every existing read site (renderer, Transforms panel,
  Triangle Editor) operates on `current.flame` and "just works" for
  whichever flame is selected. The viewport renders whatever is
  selected — main *or* subflame in isolation. This is what's shipped.
- **Live-edit attempt (reverted in commit after `e09d5aa`):** I added
  a `view_subflame_in_isolation` checkbox to the Subflames panel and
  routed the *renderer's* flame separately from `App.flame`. When
  the toggle was off and the user edited a subflame, the renderer
  was fed `logical_config().flame` (the un-swapped parent with the
  edited subflame re-inserted at its slot) while `App.flame`
  remained the swapped-in subflame for the panels. **This produced a
  reproducible bug** (see below) we couldn't pin down from code
  reading. Reverted; isolation-only behavior restored.

## What we proved still works

- CLI export of [`tests/visual/configs/3d/subflame-smoke.fflame`](../../tests/visual/configs/3d/subflame-smoke.fflame)
  renders a Sierpinski-in-a-Sierpinski correctly. The data flow —
  parent flame with `subflame_wf` → subflame buffers → nested chaos
  game → histogram → tonemap — is sound end-to-end on the GPU.
- The Phase 6 swap mechanism works for editing both Main and any
  subflame in isolation. Undo/redo is target-aware (see commit
  `e09d5aa`).
- The architecture itself has no obstruction to live editing — the
  shader can ingest a parent flame and consume edited subflame data
  via the subflames buffer on every dispatch. The pieces are there.

## What we tried that failed

The "less invasive" hybrid approach: keep the Phase 6 physical swap,
but at render time hand the renderer a different flame than what the
editor panels operate on.

```text
ConfigManager.current.flame = subflame (swapped in)
App.flame                    = subflame  ← what UI panels read/write
render_source                = logical_config().flame ← what renderer ingests
                              = parent flame with edited subflame re-inlined
```

Implementation: `gpu_updates::process_gpu_updates` computed
`render_source` per frame and called `renderer.update_flame(render_source, ...)`.
The subflame's edits flowed into `current.flame` via the normal
ConfigPath path (`current.flame` *is* the subflame), and
`logical_config()` reconstructed a fresh parent with the edited
subflame at its slot for the renderer.

**Behavior observed:** initial state and view-mode toggle both
worked correctly. The moment the user made *any* edit (via Triangle
Editor or xform panel) to the subflame while viewing the parent,
the rendered output collapsed to a single dot at the origin.
Toggling the view-mode checkbox on then off "fixed" it
(briefly — until the next edit).

**Critically, the bug appeared even when the parent had no
`subflame_wf` variation** — i.e., when the subflame data was
irrelevant to the parent's render. That ruled out subflame-buffer
content as the root cause.

### Things we ruled out

| Hypothesis | Test | Result |
|---|---|---|
| Histogram has stale samples | Force `actions.reset_accumulation = true` on every flame update in this mode → `renderer.reset()` clears the histogram + accumulation buffer | No fix |
| Shader/pipeline state is stale | Added `ShaderCache::force_rebuild_next()` and called it on every edit in this mode → full shader recompile + new compute_bind_group every frame | No fix |
| Subflames buffer is corrupting other buffers | Bug appears without `subflame_wf` on the parent (subflames buffer is unused by shader in that case) | Ruled out by the "no `subflame_wf`" observation |
| Triangle Editor visualizing the wrong flame | `App.flame` was kept as the editing target (subflame) — Triangle Editor read the subflame's transforms, so what it drew matched what was being edited | Ruled out by inspection |

The toggle path *does* fix the symptom; whatever combination of
state changes happens during toggle is the right combination. Neither
half of that combination (reset, shader rebuild) fixes it alone.
The differentiator is that the toggle uploads a *different* flame's
data to the GPU buffers (subflame's transforms when toggling to
isolation, then parent's again when toggling back), while the
edit-only path always uploads the same parent data.

### Probable culprit (un-proven)

Some interaction in the `update_flame` path that goes pathological
when called with a freshly-cloned flame whose content is identical
to the previously-uploaded flame, *while the user is mid-drag in
overwrite mode*. Candidates:

- The double-write of `params` buffer (once in `update_flame`,
  again in `update_iterations` triggered by `actions.update_view`,
  again in `compute_pass`) with `get_rng_seed()` advancing
  `frame_counter` between writes.
- Some state in the WGPU validation/cache layer that's
  pessimistically conservative across `write_buffer` calls on
  the same buffer with identical content.
- The fact that `App.flame` and `render_source` are *different
  Flame instances* (one cloned from `current.flame`, one freshly
  re-constructed by `logical_config()`) — equality by content but
  not by identity. Most of the renderer doesn't care; one corner
  of it apparently does.

Pinning this down would require either GPU debugger inspection
(RenderDoc, wgpu tracing) or instrumented buffer dumps captured
before/after the breaking edit. We chose to step back and design
around the problem instead.

## The right approach: un-swap, make panels/paths target-aware

The hybrid was an attempt to keep editing-target awareness contained
inside `ConfigManager` (via the physical swap) and route the
renderer separately. The cleaner architecture — the one originally
suggested when this came up — moves editing-target awareness *out
of* `ConfigManager` and *into* the UI panels and `ConfigPath`. The
data model never swaps:

```text
ConfigManager.current.flame = Main (always)
ConfigManager.current.flame.subflames[i] = Subflame i (always)
ConfigManager.editing_target = Main | Subflame{i}  ← UI hint only

App.flame = current.flame = Main (always)
renderer source = App.flame = Main (always)

Triangle Editor / Transforms panel:
  if editing_target == Main:       reads flame.transforms
  if editing_target == Subflame{i}: reads flame.subflames[i].transforms

ConfigPath:
  Path::TransformAffine{...}           → applies to flame
  Path::SubflameTransformAffine{i,...} → applies to flame.subflames[i]
  (or: every TransformXxx variant grows a `Scope` field)
```

**Why this dodges the bug:** there's one flame in flight. `update_flame`
is always called with the same flame. The renderer's pipeline sees a
single source of truth across all frames. Whatever the dual-flame
pathway tripped on can't happen.

**Why this is the user's mental model:** they're editing "this
subflame's transform 0" — that's *literally* what the
ConfigPath encodes, instead of "transform 0 on whatever flame
is currently swapped in".

### Estimated scope

Mechanical-but-broad. The work spans:

| File | What changes |
|---|---|
| [`src/config/delta.rs`](../../src/config/delta.rs) | Either (a) add `target: EditTarget` to every transform-related `ConfigPath` variant, or (b) keep variants but add a wrapper `Path::Scoped { target, inner }`. Apply / invert / update_type / get_value walk the target before applying the inner. |
| [`src/config/manager.rs`](../../src/config/manager.rs) | Strip the swap logic out of `set_editing_target` — it just sets the field now. `swap_to_subflame_internal` / `swap_back_to_main_internal` / `stashed_main` all go away. `logical_config` becomes a no-op (current *is* logical). `visible_subflames` / `logical_subflame_count` simplify. `rename_subflame` simplifies. The target-aware undo/redo already in `e09d5aa` continues to work; the silent-swap helper inside undo/redo becomes a no-op. |
| [`src/ui/transforms.rs`](../../src/ui/transforms.rs), [`src/ui/triangle_editor.rs`](../../src/ui/triangle_editor.rs) | Read from `flame.subflames[i]` instead of `flame` when editing a subflame. Construct target-aware `ConfigPath` values when writing. |
| [`src/ui/subflames.rs`](../../src/ui/subflames.rs) | Add the view-mode toggle (since the original motivation persists). Now it's purely a UI hint — there's no dual-flame pathway, only a "render in isolation" mode for the renderer. The toggle gates an alternate render path (likely: temporarily hand the renderer `flame.subflames[i]` instead of `flame`). |
| [`src/app/gpu_updates.rs`](../../src/app/gpu_updates.rs) | Renderer always gets `App.flame` (main). Isolation mode could be a separate option here, but it's far simpler now since we already have a working single-flame path. |
| Existing call sites that construct `ConfigPath::TransformXxx` | Need to be updated to include the editing target. Many call sites — most are in the UI panels themselves, so they have direct access to the current editing_target. Pattern: `path.with_target(config_manager.editing_target())` or constructor that takes target. |

Approximate scope: 200-400 lines of mechanical edits across maybe
8 files. No structural shader/buffer/renderer changes. The hard
part is *coverage* — making sure every ConfigPath construction site
gets updated.

### Phasing suggestion

The work is broad but each phase is shippable and reversible.

1. **Add target awareness to ConfigPath** (delta.rs). The wrapper
   variant (`Path::Scoped { target, inner }`) is probably easier
   than touching every transform variant — backwards compatible
   for existing code paths that don't care about subflames.
2. **Make `ConfigManager` honor the target** when applying paths.
   `apply_value` / `get_value` / `set_value` look at the path's
   scope and route to `flame` vs `flame.subflames[i]`.
3. **Strip the swap mechanism.** Remove `stashed_main`, the
   physical swap, `logical_config`. `set_editing_target` becomes a
   one-liner that updates the field + pushes the SwapTarget undo
   entry. The Phase 6 panel (subflame list, add/delete, rename)
   still works — most of its logic is in `ConfigManager`
   accessors that just need to look at the un-swapped state.
4. **Update Triangle Editor + Transforms panel** to read from
   `flame.subflames[i]` when editing a subflame, and to write via
   target-aware ConfigPath.
5. **Reintroduce the view-mode toggle** in `subflames.rs`. Now
   it gates only a renderer-side decision: when on,
   `gpu_updates` hands the renderer `flame.subflames[i]`; when
   off, `flame` (main).
6. **Tests + smoke.** The existing undo/redo target tests already
   cover the path layer. Add a regression test specifically for
   live editing — apply an edit through `ConfigPath::Scoped`
   while editing target = Subflame and verify the right slot
   changed.

## Lessons / hooks

- The Phase 6 swap mechanism *worked* for everything except live
  cross-target rendering. The undo/redo target-aware machinery in
  `e09d5aa` is already shaped for the un-swap world — entries carry
  an `EditingTarget` that the apply path silent-swaps to. After the
  un-swap refactor, the silent-swap becomes a no-op (since current
  doesn't physically swap), but the *path-routing* it triggers is
  exactly what we need.
- We added `ShaderCache::force_rebuild_next` and a `request_flame_refresh`
  helper during the failed attempt; both were reverted. The
  forced-rebuild helper might still be worth resurrecting as a
  diagnostic tool, but not in production.
- The bug is reproducible against `tests/visual/configs/3d/subflame-smoke.fflame`
  on the desktop build with the reverted view-mode-toggle code
  reapplied. If we ever want to confirm the un-swap refactor
  actually dodges it, that's the test config.

## Out of scope (still)

- Nested subflames (subflame containing subflame_wf). v1 disallowed
  this and the un-swap refactor doesn't change that — it's a
  separate state-allocation / shader-codegen problem from
  [docs/projects/subflames.md](subflames.md).
- Apophysis XML import of `subflame_wf` resources. Tracked
  separately in [docs/projects/subflames.md](subflames.md).
- Per-subflame palettes. Same.

## Risks

| Risk | Mitigation |
|---|---|
| ConfigPath proliferation: adding target to every variant could double the enum | Use the wrapper variant (`Path::Scoped`) — single new variant, applies to any inner path |
| Missing a call site that constructs `ConfigPath::TransformAffine` without a target → silently edits the wrong flame | Make the un-scoped path *unreachable* for transform variants by removing those variants in favor of always-scoped ones; or add `#[deprecated]` and grep for usage |
| Animation system writes paths during playback; if those paths aren't target-aware they break for subflame keyframes | Confirm the animation system only animates Main-flame parameters today (no per-subflame keyframes); add to the scope only if it does |
| Undo/redo: the silent-swap path in `manager.rs::undo/redo` no longer applies after the refactor | Easy: replace silent-swap with a no-op or a comment-out. The `target` field on entries still gates path application correctly via the new ConfigPath scoping |
