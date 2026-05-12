# Levels scale-invariance (Phase 8c follow-up)

## Goal

Make the Levels feature's input range invariant to iteration count, so
that the same `levels_high` slider value produces the same visual
clipping behavior whether the user is rendering a quick 1M-iter
preview or a 200B-iter export. Today the slider operates on raw
cumulative density count, which scales linearly with total iterations
— meaning the "right" value at one iteration target is wrong at
another.

This is the deferred follow-up flagged in commit `960074a`'s message:

> Side note for follow-up: Levels feature's `levels_high` default is
> 1000.0, calibrated against the OLD scaled-density storage. With raw
> counts in the accumulator, typical density values land in a different
> numerical range — saved configs that explicitly set levels_low/high
> values may need re-tuning. Not addressed here.

## Context

Phase 8b (`6dec47d`) switched the accumulator from
`count × 0.01 × blend_factor` to raw cumulative iteration count per
pixel. Density values changed scale dramatically: at 1080p / 40B
iterations, average density is ~20,000 hits/pixel; bright cores
exceed 100,000.

The Levels feature's `apply_levels()` function in
`shaders/tonemap.wgsl` compares density to absolute thresholds
(`levels_low`, `levels_high`) and remaps to opacity ∈ [0, 1]:

```wgsl
let normalized = (density - low) / (high - low);
let clamped = clamp(normalized, 0.0, 1.0);
```

With the legacy default `levels_high = 1000.0`, post-Phase-8b density
values overshoot the ceiling for virtually every non-empty pixel.
`clamped` is 1.0 everywhere, `apply_levels` returns 1, and the line
`fractal_alpha = min(base_alpha, leveled_opacity)` reduces to just
`base_alpha`. Levels is a no-op at default settings — confirmed by a
fresh re-render of the thumbnail cache showing every preset
significantly too bright.

User testing landed on `levels_high = 20,000` as a reasonable manual
override for a 40B-iter 1080p render. That number is exactly the
mean per-pixel density at that iter count — i.e., 1× the
`sample_density = total_iters / pixel_count` uniform that Phase 8a
already computes.

## Decision: normalize density inside the shader, not the default

Two ways to make the slider iteration-count-invariant:

| Option | Shader change | Slider semantics | Saved configs | Per-iter-count override |
|---|---|---|---|---|
| **A. Default tracks `sample_density`** | None | Absolute density count (1000, 20000, …) | Compatible | User must re-find override at each iter count |
| **B. Normalize density before comparison** | One division | Multiple of mean density (1.0 = clip at mean) | All existing serialized values now mean different thing | Same value works at every iter count |

Option B is the correct fix. It's the same insight as Phase 8a applied
to Levels: the artistic decision is "clip at how many multiples of the
mean," not "clip at what absolute count." Option A leaves the user
needing to retune any time iteration target changes, which is exactly
the problem.

Migration risk for option B is **bounded to zero in the corpus** — a
survey of all 141 checked-in `.fflame` files (2026-05-12) found zero
configs serialize `levels_high` or `levels_gamma` to disk. The
`skip_serializing_if = "is_default_levels_high"` sentinel kept the
default out of every file. Local user files with explicit overrides
exist (the user reports manual tuning to 20000), but the corpus
breakage is nil.

## What changes

### Shader (`shaders/tonemap.wgsl`, `shaders/tonemap_export.wgsl`)

`apply_levels()` normalizes density by `sample_density` before
applying the linear remap. Guard against `sample_density == 0` (empty
buffer / first frame):

```wgsl
fn apply_levels(density: f32) -> f32 {
    let low = tonemap_params.levels_low;
    let high = tonemap_params.levels_high;
    let gamma = tonemap_params.levels_gamma;

    if (high <= low) {
        return select(0.0, 1.0, density > low * tonemap_params.sample_density);
    }
    if (tonemap_params.sample_density <= 0.0) {
        return 1.0;  // No samples yet; let base_alpha decide
    }

    let normalized_density = density / tonemap_params.sample_density;
    let normalized = (normalized_density - low) / (high - low);
    let clamped = clamp(normalized, 0.0, 1.0);

    if (gamma != 1.0 && gamma > 0.0) {
        return pow(clamped, gamma);
    }
    return clamped;
}
```

Both `tonemap.wgsl` and `tonemap_export.wgsl` carry copies. Both need
the same change.

### Default (`src/config/fractal_config.rs`)

- `default_levels_high()`: `1000.0 → 1.0` (clip at mean density)
- `is_default_levels_high()`: sentinel comparison updated to `1.0`
- `default_levels_low()` already returns `0.0` — unchanged semantically
  ("don't clip the floor"). Note that low is also now in "× mean
  density" units; `0.0` still means the same thing.

### UI (`src/ui/histogram.rs`)

Three sites need updating:

1. **Auto button** (`render_levels_controls_managed`, ~line 397). The
   one-shot writes `histogram.percentile_1` and `percentile_99`
   directly to `levels_low/high`. After the change, these need to be
   divided by `sample_density` before writing. The histogram itself
   still bins raw density values; only the slider value is normalized.

2. **Slider range bounds** (~line 418, 431). Currently `0.0..=histogram.max_density.max(100.0)` — under the new units this becomes `0.0..=(histogram.max_density / sample_density).max(10.0)`. Practical max around 10× mean covers any sensible setting.

3. **Histogram visualization markers** (`levels_to_screen_x`, line 214). The marker positions for `levels_low/high` on the density-axis histogram need to multiply by `sample_density` to land in raw-density coordinates for display alignment. The slider in the UI shows the normalized value; the marker on the histogram shows where that value lands in absolute terms.

### `LevelsState::update_from_histogram` (`src/ui/histogram.rs:298`)

This is the live-Auto path (also used by the Auto button). Same fix:
divide by `sample_density` before writing to `input_black` /
`input_white`. Needs the `sample_density` value available — pass it
in as a parameter, since `LevelsState` doesn't know about iter count.

### Tests

- Unit test in `src/config/fractal_config.rs`: round-trip a config
  with default levels, confirm the JSON does not serialize the field.
- Unit test for `apply_levels` semantics: at `sample_density = 100,
  levels_high = 2.0`, a pixel with density `200` should map to
  opacity `1.0`; density `100` → `0.5`; density `0` → `0.0`. Validate
  the math in Rust by porting the formula into a helper.
- Visual smoke test: re-render thumbnails on a few presets, eyeball
  for "default Levels now usefully clips, not no-op."

## What does NOT change

- The shader plumbing for `sample_density` — already wired and updated
  every frame as of Phase 8a-fix (`43643c9`).
- The `apply_levels` call site (`fractal_alpha = min(base_alpha, leveled_opacity)`) — unchanged.
- `levels_gamma` semantics — same gamma curve, still applied after the
  linear remap.
- The histogram readback path (DensityHistogram bins still hold raw
  density values; only the *display interpretation* of the levels
  values changes).

## Migration story

For checked-in `.fflame` files: nothing needed. Survey confirms zero
overrides serialized.

For local user files with explicit overrides: option B silently
reinterprets, but the practical impact is mild — old `levels_high = 1000` becomes "clip at 1000× mean density," which is effectively no
clipping (same as the broken pre-fix behavior the user is already
fighting). Old `levels_high = 20000` becomes "clip at 20000× mean,"
also effectively no clipping. **In both cases, the user's existing
file degrades back to the pre-fix "Levels does nothing" state, not
toward something visually broken.** They can then re-author with the
new units.

If we wanted belt-and-suspenders: add a deserialize-time auto-migrate
that treats any `levels_high > 100` as "old format" and divides by the
config's `max_iterations / (width × height)` if those are available.
Adds code complexity for a niche case; skip unless the user reports
their personal flames break in a worse way than expected.

## Phases

1. **Shader + default change.** Both tonemap shaders, both sentinel
   functions. No UI changes yet — slider still displays whatever
   number is in the field; with the new defaults that number is now
   `1.0` and the math interprets it correctly.
2. **UI bounds + Auto-button normalization.** Slider range becomes
   sensible. Auto button writes correct values. `update_from_histogram`
   takes `sample_density` parameter.
3. **Histogram marker alignment.** `levels_to_screen_x` adjusted so
   markers display correctly against the raw-density histogram.
4. **Validation.** Visual smoke test on `simple3.fflame`,
   `bubble-3d.fflame`, and at least one Apophysis-import flame.
   Confirm default Levels now usefully clips; confirm slider feels
   reasonable across iteration counts (1M preview vs 40B export).

Should land as one PR — the phases are sequenced inside the branch
but each one breaks the next; there's no useful intermediate state
to land separately.

## Risks

| Risk | Mitigation |
|---|---|
| User-local `.fflame` files with explicit `levels_high` overrides degrade silently | Document in commit message + release notes; provide deserialize migration as a follow-up if user reports issues |
| `sample_density == 0` on first frame causes division-by-zero | Guard at start of `apply_levels`; return 1.0 (no levels effect) when no samples yet |
| Histogram marker alignment subtle: ratio-of-density vs absolute-density display can drift | Validate visually with a representative flame; the marker should sit on the same x-pixel before and after the change at equivalent settings |
| Slider range bound `0..=10` might be too tight for unusual flames | Use `clamping(SliderClamping::Never)` (already the convention in `render_levels_controls_managed`) so the user can type larger values |
| Apophysis import paths set legacy values — Apophysis `.flame` XML doesn't have a directly-equivalent "levels" field, but importers might write defaults | Audit `src/apophysis_xml.rs:336-337` (sets `levels_low=0, levels_high=1000`) — update to new default `1.0` |

## Out of scope

- The `gamma_threshold` / `ls`-recycling coupling discovered during
  research (`gamma_threshold > 0` quietly drives RGB brightness via
  `ls = vib * alpha / fp3`). This is faithful Apophysis behavior and
  out of scope for this branch; document as a known coupling rather
  than attempt to redesign.
- Tonemap preset library (named recipes like "Bubble", "Discus",
  "Subtle Gamma 2.2"). Separate future project.
- Mixed-precision accumulator (f16 RGB + f32 density) for the ~8% perf
  win. The user has explicitly accepted the regression; not pursuing.
