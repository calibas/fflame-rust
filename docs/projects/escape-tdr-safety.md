# Escape-time TDR safety, chunk pacing, and animation consistency

Plan agreed 2026-08-28. Items A-D address the remaining device-loss
class after the first round of fixes (row bands, direct-path breaker,
GPU-reinit recovery — see the queue in `escape-time-fractals.md`);
the animation sections address live playback in escape mode. Nothing
here is implemented yet; each item says what to build, where, how to
test it, and what it costs.

## The mechanism being defended against

Field data (crash.log, 2026-08-28): repeated
`DEVICE LOST (Unknown): driver implementation is at fault` — the
Vulkan driver's TDR presentation — at ~10 s intervals, at BOTH ends
of the zoom range, always with max_iter far above what the view
plausibly needs (~1M+ at zoom ~1 and ~15). The direct-path breaker
converges after a few losses; the perturbed path has no breaker yet,
and its adaptive chunk sizing has a structural blind spot:

The chunk doubles whenever the previous frame's wall-clock gap came
in under target (~30 ms), capped at `CHUNK_ITERS_MAX` = 1,048,576.
At high max_iter over a view containing set interior, early chunks
are nearly free (most pixels escape immediately; BLA skips the
survivors) so the chunk grows to the ceiling — then the cost profile
flips: the surviving pixels are the ones where skips stop applying,
and they grind per-step. A 1M-iteration chunk over ~100k surviving
supersampled pixels is a multi-second dispatch. Two compounding
lies: cost-per-iteration is violently non-stationary between
consecutive chunks (the feedback can't see the cliff coming), and
wall-clock under-measures while 2-3 submissions are in flight, so
doublings get free passes before backpressure tells the truth.

## A. Perturbed-path circuit breaker (small)

Mirror of the shipped `DIRECT_BUDGET_SHIFT` (renderer.rs):

- `PERTURB_ITERS_SHIFT: AtomicU32` halves `CHUNK_ITERS_MAX` (and the
  seed budget) on a device loss attributed to a perturbed dispatch;
  `PERTURB_RENDER_IN_FLIGHT: AtomicBool` opens while perturbed chunks
  of an unfinished render are being submitted, closed on settle and
  on entering the direct path (same shape as the direct flag).
- `note_device_lost()` consumes whichever flag is open (they are
  mutually exclusive by construction).
- **Persist both shifts** across sessions: a small
  `gpu_tuning.json` next to the orbit cache (renderer-side helper —
  the renderer has no ConfigManager access, and SystemSettings
  round-tripping from a static context is the wrong coupling).
  Loaded lazily via OnceLock, written on change. Without
  persistence every session re-learns by losing the device 1-4
  times, and each loss is a spin of the driver-state roulette.
- Test: extend `device_loss_halves_the_direct_budget_only_when_attributable`
  with the perturbed flag; a serde round-trip test for the tuning
  file; manual: the user's zoom-15 repro must converge within ~2
  losses on a cold tuning file and ZERO on a warm one.

## B. Trust-bounded growth (small)

Two rules in `next_chunk`:

1. A doubling requires the previous chunk's measurement to be
   PIPELINE-CLEAN: only grow every 3rd call (past the in-flight lie
   window), not every call.
2. Cap the chunk at 32x the largest chunk whose FOLLOWING inter-call
   gap stayed under target ("proven-completed work"), seeded by the
   static budget. Bounds the overshoot past a cliff to one honest
   doubling over proven ground instead of `CHUNK_ITERS_MAX`.

Sizing-only change: the render is chunk-invariant (pinned by the
`ESCAPE_CHUNK_MS` invariance test), so this cannot change images.
Costs a slower ramp on genuinely-cheap renders (a few extra frames).

## C. GPU timestamp pacing (moderate — the principled fix)

Replace the wall-clock inference with actual per-dispatch GPU time:

- `Features::TIMESTAMP_QUERY` requested at the three device-creation
  sites (`gpu/device.rs`, headless in `renderer/render.rs`, the
  repro-test device) WHEN the adapter offers it; wall-clock fallback
  otherwise (feature-gate, not a requirement — SwiftShader and some
  mobile adapters lack it).
- A 2-slot query set around the escape compute pass,
  `resolve_query_set` into a small buffer, async map; results arrive
  2-3 frames late, which is fine — pacing is a trailing control
  loop either way. Vulkan `timestamp_period` scaling via
  `Queue::get_timestamp_period`.
- With honest measurements the doubling rule is sound as-is;
  B's caps become belt-and-suspenders. Benefits the direct path's
  band sizing too (same helper).
- Test: assert the chunk-invariance property still holds with
  timestamps enabled; a headless run comparing paced iterations/sec
  with and without (sanity, not a pin).

## D. Interior detection — SHIPPED 2026-08-28, and stronger than planned

The plan below proposed a TOLERANCE check (|z − z_s| within ~1e-6,
two confirmations, a config toggle, an agreement test allowing
bounded differences). What shipped is exact-repeat detection, which
is a different and better bargain:

The direct path's entire iteration state IS z, and the arithmetic is
deterministic, so a BIT-EXACT repeat of a snapshot proves the f32
orbit is periodic from that point on. Such a pixel can never escape,
and every later iterate is a value already seen — so stopping is not
an approximation of running to max_iter, it is the same render.
That collapses the whole correctness envelope the plan worried
about: no tolerance to tune, no confirmation count, no false
positives to bound (a slow escape that merely LOOKS periodic cannot
repeat bit-exactly), and no user toggle, because there is nothing to
trade off. Comparison goes through `bitcast<u32>`, not float `==`:
+0.0 == -0.0 is true while the two continue differently under maps
that divide or take logs, and integer compares are immune to Metal's
fast-math.

Spliced only when the coloring cannot draw the interior (and does
not accumulate, and is not the period coloring, which terminates on
its own cycle test): for those colorings a non-escaping pixel
renders the background whatever iteration it stopped on. Brent
epochs — snapshot at powers of two — so any cycle is caught within
one period of entering it, from a single stored value and no extra
per-pixel memory.

Measured on the home view (~1/4 interior), 256x192, `smooth`:
byte-identical to a detection-off build at every max_iter tested,
45 ms -> 5 ms (**8.2x**) at max_iter 400,000, 1.2x at 20,000. The
win scales with max_iter exactly as the theory says (baseline is
O(max_iter) per interior pixel, detection is O(period)), so the
field's 10.1M-iteration configs — the ones whose bands were reaching
Windows' TDR window — gain far more than the measured 8x.

THE PERTURBED PATH IS DEFERRED, deliberately. Its state is (delta,
reference index), not z: two different (w, m) pairs can reconstruct
the same z_full and then evolve differently, so a z repeat does NOT
prove periodicity there and the exactness argument collapses. A
tolerance check would work but brings back everything above — the
toggle, the false positives, an accuracy study — for a path that
already has bounded chunks and a circuit breaker (items A/B). Worth
doing only if measurement shows deep interior renders still hurt.

### The original plan, for reference

Per-pixel periodicity checking so interior pixels stop after ~one
cycle period instead of burning all of max_iter. This is the "1M+
iterations at zoom 1/15" scenario: those iterations are spent almost
entirely on interior pixels that were never going to escape.

- Algorithm: Brent-style — keep one snapshot z_s; every power-of-two
  iterations, replace the snapshot; between replacements, compare
  |z - z_s| against a RELATIVE tolerance (~1e-6 of |z|). Two
  consecutive confirmations before declaring interior (halves false
  positives near the boundary). Declared-interior pixels write the
  max_iter classification (same color as today's exhausted pixels).
- Direct template: registers only — snapshot + counter live in the
  band loop. Trivial state cost.
- Perturbed template: needs the snapshot in `IterState` — full z is
  available each step (ref + delta). Store as f32 pair + compare
  counter; repack the 48 B/px struct (the escaped flag already
  shares a word) or grow to 56 B/px — decide at implementation
  against the resize cap math.
- Correctness envelope: tolerance-based detection can misclassify
  pixels VERY near the boundary (slow escapes that look periodic).
  Ships behind a config toggle (`interior_detection`, default ON for
  interactive, OFF for exports? — decide with data), with an
  agreement test rendering detection-on vs detection-off across the
  escape suite and asserting differences stay confined to
  max_iter-adjacent pixels below a block threshold.
- BLA interaction: skipped iterations bypass the check — fine
  (skips only apply while deltas are small; interior convergence
  still reaches the checker between skips). Verify in the agreement
  test at depth.
- Expected: interior-dominated views drop from O(max_iter) to
  O(period) per interior pixel — orders of magnitude at 1M+
  max_iter; directly removes most TDR pressure rather than merely
  containing it.

Ordering: A+B first (small, ends the crash-loop class for good),
then D (biggest win for the observed usage), C alongside or after D.
A silent max_iter-by-zoom clamp was considered and REJECTED (it
changes escape counts and images); an advisory hint in the escape
panel when max_iter looks orders past the zoom's plausible need is
fine and cheap.

## Animation playback in escape mode (frame-consistent, not frame-racing)

Mechanism today (`app/animation_update.rs`): every display frame,
`update_animation` advances `animation_controller.current_time` by
the WALL delta and `apply_animated_values` writes track values into
ConfigManager; in escape mode each write marks `escape_dirty`, the
frame renders ONE chunk/band of the new config, and the next frame
moves the targets again — so playback shows a smear of partial
renders (the reported mess). The video-export path is already
correct (`render_escape` in `renderer/render.rs` settles each frame
before encoding); this is live playback only.

Design — settle-then-jump:

- When `render_mode` is Escape and the controller is playing:
  sample the controller ONCE, apply values, then HOLD (skip
  `controller.update` + `apply_animated_values`) until
  `escape.render` reports settled. Present that frame. Then advance
  the controller by the ACCUMULATED wall time since the last sample
  (dropping intermediate frames) and repeat.
- Playback rate becomes "as fast as frames settle" — possibly 1 fps,
  but each displayed frame is a REAL frame at a REAL timestamp,
  which is the requested behavior.
- Audio: `sync_audio` keeps working — audio runs on wall time and
  the controller jumps to wall time at each sample, so sync drift is
  bounded by one settled-frame duration.
- Auto-stop (LoopMode::Once) checks fire at sample points.
- Test: an app_repro-style sequence driving a 2-keyframe escape
  animation, asserting every presented frame's config matches a
  sampled timestamp exactly (no frame renders a blend of two
  configs' chunks).

## Live preview (Overwrite mode) in escape mode

`render_mode.rs` Overwrite is the flame live-preview: rapid
parameter changes reset accumulation each frame for immediate
feedback. Escape mode has no accumulation loop (`should_iterate` is
forced false), so Overwrite's remaining effects there are vestigial
at best and confusing at worst (FSM transitions, the
post-param-change overwrite window, history coalescing behavior
tuned for flames). Plan: audit the Overwrite entry points and gate
the FSM off in Escape mode — escape param edits already re-render
through `rerender_escape`, which is the correct escape-native
"live preview". Small; mostly deletion plus a test that a rapid
slider drag in escape mode produces no Overwrite transitions.
