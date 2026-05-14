# Tonemap and palette panel improvements

## Goal

Two related groups of UX improvements to the Colors / Tonemap panel:

1. **Tonemap presets.** A library of named "looks" (Default, Subtle,
   Vivid, etc.) that snap the brightness/curve controls to a known
   sensible point without touching the user's flame definition,
   palette, or background. New users get an on-ramp; expert users
   get a fast reset.

2. **Palette transform improvements.** Live preview of the
   *effective* palette (after rotation/squeeze/size/etc. are
   applied), plus new transforms (geometric squeeze, logarithmic
   redistribution, reverse) that compose with the existing ones.

## Context

The corpus survey from the `levels-scale-invariance` project found
that ~90% of 141 checked-in flames use identical default tonemap
values, and the deviants cluster into 3-4 recipes. That's the
evidence base for tonemap presets: most flames don't need bespoke
tuning, and the ones that do reach for the same handful of looks.

For the palette improvements: the existing palette panel displays
the *raw* palette gradient, but the fractal renders using a
transformed lookup (`palette_rotation`, `palette_squeeze`,
`palette_size`). The user has to mentally reconstruct what's
actually being applied. Adding a live preview that mirrors the
GPU's transform pipeline removes that gap.

The new transforms (geometric squeeze, logarithmic redistribution,
reverse) extend the existing palette-shaping vocabulary in ways
that aren't expressible today.

## Pipeline architecture

Every palette lookup currently goes through:

```
t (color index 0..1)
  → squeeze (linear)
  → rotation
  → palette[t × size]
```

After this branch:

```
t (color index 0..1)
  → squeeze (Linear OR Geometric — mode dropdown)
  → logarithmic redistribution (off by default; strength parameter)
  → rotation
  → reverse (toggle; applied last)
  → palette[t × size]
```

**Default behavior unchanged.** When squeeze mode is `Linear` with
factor 1.0, log strength is 0, rotation is 0, and reverse is off,
the lookup degenerates to today's identity — same byte output as
before.

The transform pipeline gets factored out of `Buffers::update_palette`
(in `src/gpu/buffers.rs:1438-1480`) into a reusable helper so the UI
preview can call the exact same code path the GPU does. CPU-side is
fine — palettes are tiny (256-4096 entries) and the preview
re-renders at most a few times per frame on slider drag.

## Phases

### Phase 1 — Extract palette transform helper

Pull the squeeze + rotation logic out of
`Buffers::update_palette` into a free function in `src/scene/palette.rs`:

```rust
pub fn transformed_lookup_table(
    palette: &Palette,
    transform: &PaletteTransform,
    out_size: usize,
) -> Vec<[f32; 3]>;

pub struct PaletteTransform {
    pub squeeze_mode: SqueezeMode,
    pub squeeze_factor: f32,        // linear repeats, or geometric falloff
    pub log_strength: f32,          // 0 = off, sign = direction, magnitude = strength
    pub rotation: f32,              // -1.0..1.0
    pub reverse: bool,
}

pub enum SqueezeMode { Linear, Geometric }
```

`Buffers::update_palette` becomes a thin wrapper that calls this
helper + uploads the result to the GPU palette texture. The shader's
existing palette lookup is unchanged — it still reads from the
texture as before.

No behavioral change. Should produce byte-identical GPU palette
texture content for all existing configs.

### Phase 2 — Reverse palette toggle

`palette_reverse: bool` field on `FractalConfig` (default false).
Wired into `PaletteTransform`. In the helper, applied as the final
step: `output[i] = transformed[size - 1 - i]` when enabled.

UI: checkbox in the palette panel near the existing rotation slider.

Smallest behavior addition; validates the Phase 1 refactor by adding
a new transform without touching the GPU shader.

### Phase 3 — Geometric squeeze

Add `SqueezeMode::Geometric { falloff: f32 }` to the squeeze
options. Mode dropdown in the UI; the existing `palette_squeeze`
slider's semantics depend on the selected mode.

Math for geometric mode at falloff `r` (default 0.5):

```
First octave  t ∈ [0, r):              palette_t = t / r
Second octave t ∈ [r, r + r²):         palette_t = (t - r) / r²
Third octave  t ∈ [r + r², r + r² + r³): palette_t = (t - r - r²) / r³
...
```

Closed form: find octave `n` such that `t ∈ [1 - r^n, 1 - r^(n+1))`,
then `palette_t = (t - (1 - r^n)) / (r^n × (1 - r))`.

Falloff range `[0.1, 0.95]` exposed as a slider. Practical default
`0.5` (the original example). Falloff of 1.0 is the degenerate "no
octaves" case — fall back to linear behavior at that boundary.

Linear and Geometric are mutually exclusive (mode dropdown). The
existing `palette_squeeze` field gets renamed to a struct or split
into two fields depending on serde compatibility — preserve old
configs by defaulting `squeeze_mode = Linear` when not present.

### Phase 4 — Logarithmic redistribution

New `palette_log_strength: f32` field on FractalConfig (default 0.0).
Applied *after* squeeze in the pipeline. Composes with whichever
squeeze mode is active.

Math (using exponential since "logarithmic" remap is the inverse):

```
strength > 0: t → (exp(strength × t) − 1) / (exp(strength) − 1)
              bunches values toward palette end
strength < 0: symmetric remap toward palette start
strength = 0: identity (no-op)
```

UI: slider with center detent at 0, range `[-5.0, 5.0]` (or whatever
proves useful in testing).

### Phase 5 — Live palette preview in the panel

Render a small gradient bar in the palette panel showing the
*effective* palette after the full transform pipeline. Reuses the
Phase 1 helper:

```rust
let lut = transformed_lookup_table(&palette, &transform, 256);
// draw `lut` as a gradient strip in egui
```

The preview re-computes when any palette-affecting field changes.
At 256-entry resolution recomputing is essentially free.

Optionally: show *two* bars (raw palette + transformed) for
side-by-side comparison. Worth considering after the basic single-
bar version is working.

### Phase 6 — Tonemap preset library

Bundle a small JSON file in `assets/tonemap-presets/` (or built into
the binary similar to palette packs) holding named preset entries.
Each entry is a partial `FractalConfig` containing *only* the
brightness/curve fields:

- `exposure`, `gamma`, `gamma_threshold`, `brightness`
- `vibrancy`, `saturation`, `hue_shift`
- `levels_low`, `levels_high`, `levels_gamma`
- `alpha_blend_low`, `alpha_blend_high`

UI: dropdown in the tonemap panel. Selecting a preset writes those
fields into the current FractalConfig via ConfigManager (one batch
update so it's a single undo step). The user's flame, palette,
background, view state are untouched.

Initial preset set, derived from the corpus survey:

- **Default** — current defaults
- **Subtle (Gamma 2.2)** — the `gamma=2.2`-only flames
- **Apophysis Bubble** — high gamma + high gamma_threshold (the
  bubble3d cluster)
- **Apophysis Discus** — the discus cluster's exact values

Plus a few hand-tuned "stylistic" presets the user can author over
time:

- **Vivid** — moderate saturation+vibrancy boost
- **High Contrast** — gamma 1.5 + levels_high ≈ 2.0 (clip earlier)
- **Low-Key** — exposure ≈ 0.6, dark moody look

## Phasing rationale

Phase 1 is the refactor that unblocks Phases 2-5. Each subsequent
palette phase adds one feature and validates the helper signature.
Phase 6 (tonemap presets) is independent of the palette work but
ships in the same branch since it's the same panel.

Could be split into two PRs if the diff gets unwieldy:

1. Palette transforms (Phases 1-5) — one logical refactor + feature set
2. Tonemap presets (Phase 6) — independent

Decision deferred until we see the diff size.

## Out of scope

- **Auto-tonemap from histogram.** Discussed earlier; would be an
  "Auto" button on the tonemap panel that does histogram analysis
  and picks values. Real value but more complex; ship presets first.
- **Per-flame custom preset save.** "Save current tonemap as
  preset" UI. Worth doing later as a polish item once the preset
  library proves itself.
- **Color-only effect interactions.** The existing color effects
  chain (post-tonemap) operates orthogonally. Presets do not touch
  it.
- **Reverse rotation interaction with squeeze octaves.** When
  geometric squeeze is active, "reverse" still applies to the final
  output — octaves get reversed wholesale, which may look strange
  but is the literal definition of "flip the result." Acceptable for
  v1.

## Risks

| Risk | Mitigation |
|---|---|
| Refactor (Phase 1) changes byte output for some flame that hits an edge case in the current squeeze+rotation math | Pre-Phase-1: render the visual-regression suite. Post-Phase-1: re-render, diff. Any divergence is a bug in the helper. |
| Geometric squeeze produces visually jarring "octave seams" at boundaries | Acceptable artistic effect; document and let users avoid if undesired. Could revisit with smoothing if it becomes a usability issue. |
| Log strength scale feels wrong (too aggressive or too weak in practical range) | Tune the range and detent during validation. Defer to user feedback on default slider bounds. |
| Old `.fflame` configs deserialize incorrectly after adding `squeeze_mode` enum and `palette_reverse` / `palette_log_strength` fields | Serde defaults on all new fields; `squeeze_mode` defaults to `Linear`, log strength to 0.0, reverse to false. Existing configs deserialize unchanged. |
| Tonemap preset JSON format becomes another asset to maintain | Same pattern as palette packs (already in `assets/palettes/packs/`). Existing infrastructure. |
