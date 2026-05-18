# Tonemap: hue-preserving highlights — **shipped**

## Status

All three phases landed on the `tonemap-highlights` branch (commits
[`331d143`](../../), [`1901698`](../../), [`dbda797`](../../), plus a
small UI tweak moving the new dropdown). Default behavior is
unchanged — old `.fflame` files render bit-identically — but users now
have four highlight modes and a `Highlights` (Apophysis `white_level`)
slider to escape the per-channel clip cliff.

## Problem (now fixed)

The shader's per-channel clamp at the end of the tonemap pass — was
`color = clamp(color, 0, 1)` at
[`shaders/tonemap.wgsl:559`](../../shaders/tonemap.wgsl) — pushed
bright colors toward the CMY/white corners of the RGB cube as channels
saturated independently. Orange `(1, 0.5, 0)` × exposure 5 → clamped
to pure yellow; peach × 5 → white. The user's "correct" exposure
range was tiny because the clamp is a hard cliff.

## What shipped

### Phase 0 — "Highlights" slider (Apophysis `white_level`)

JWildfire's `whiteLevel` math was already in the shader at
[`tonemap.wgsl:466`](../../shaders/tonemap.wgsl) (`let fp3 = ls *
bucket_count * tonemap_params.white_level`) but the value was a
hard-coded `DEFAULT_WHITE_LEVEL = 200.0` constant. Phase 0 just
exposed it as a user-tunable config field + UI slider — no shader math
changed.

What it does: divides chroma channels by `white_level` while leaving
alpha untouched in the log-density curve. Higher values → chroma
shrinks relative to alpha → dense bright pixels saturate against the
background (alpha blend) *before* RGB clips, so they bleach via alpha
rather than per-channel clipping. **Higher = saturated, darker
highlights; lower = brighter, washes out.** (Counterintuitive given
the "Fade to White" name — JWildfire's UI label means "the white
threshold," not "amount of fade.")

Concrete changes:

- `FractalConfig.white_level: f32` (default 200.0)
  in [`fractal_config.rs`](../../src/config/fractal_config.rs).
- `ConfigPath::WhiteLevel` with full plumbing — Display, i18n,
  `UpdateType::ToneMappingOnly`, to/from_string, both tonemap bulk
  lists in [`delta.rs`](../../src/config/delta.rs).
- `ConfigManager` get/apply in
  [`manager.rs`](../../src/config/manager.rs).
- `TonemapParams` construction sites in
  [`compute_kernel.rs`](../../src/renderer/compute_kernel.rs) and
  [`high_res.rs`](../../src/export/high_res.rs) read from
  `config.white_level`. `FlameRenderer` caches it on `self` so the
  internal `update_tonemap_state` helper (called by density/background
  refresh) doesn't clobber it back to default.
- "Highlights" slider in
  [`tone_mapping.rs`](../../src/ui/tone_mapping.rs), range
  `50.0..=1000.0`, with explanatory tooltip.
- serde `#[serde(default)]` — old `.fflame` files load at 200.0
  unchanged.

`TonemapPreset` does *not* include `white_level` — selecting a preset
preserves the user's highlights tuning. Has a TODO comment if we
revisit.

### Phase 1 — `HighlightMode { Clip, MaxNorm }`

Replaced the unconditional per-channel clamp with a uniform-driven
branch.

- `HighlightMode` enum in
  [`scene/tonemap.rs`](../../src/scene/tonemap.rs).
  - `Clip` (default): per-channel `min(color, 1.0)` — Apophysis/
    JWildfire compatible, same CMY shift as before.
  - `MaxNorm`: `let m = max(color.r, max(color.g, color.b)); if m > 1
    { color /= m; }` — brightest channel lands at 1.0, others stay in
    ratio. Exact hue preservation; bright pixels desaturate by
    lowering value, not shifting hue.
- `FractalConfig.highlight_mode`, `ConfigPath::HighlightMode`,
  `ConfigValue::HighlightMode` variant + `From`/`TryFrom`, JSON
  parser, `ConfigManager` get/apply.
- `TonemapParams.highlight_mode: u32` + `[u32; 3]` padding (matches
  WGSL scalar-u32 padding — `vec3<u32>` would have 16-byte align *and*
  16-byte size in std140, pushing the buffer to 160 bytes vs. Rust's
  144). `FlameRenderer` caches on `self`.
- WGSL `tonemap.wgsl` switches on `highlight_mode` at the post-
  exposure clamp site.

### Phase 2 — `Reinhard` + `Filmic` modes

Two more variants added to the same `HighlightMode` enum — pure shader
additions, no new uniforms, no new config fields.

- **Reinhard**: `L_mapped = L / (1 + L)` (Rec.709 luminance weights);
  scale RGB by `L_mapped / L`. Smooth photographic roll-off, slight
  highlight desaturation, hue-preserving.
- **Filmic**: ACES Narkowicz approximation —
  `(x(2.51x+0.03)) / (x(2.43x+0.59)+0.14)`. Game/cinema curve, slight
  midtone contrast boost and gentle highlight roll-off. Acts per-
  channel so it *can* still show a CMY shift on properly-exposed
  flames — close to Clip but with visibly smoother midtones.

### UX polish

Highlight mode UI is a `ComboBox` at the bottom of the Tone Mapping
section (just above the Alpha Blending separator) — placed there so
it's the last knob the user reaches for after tuning exposure /
gamma / brightness / Highlights.

## User-observed behavior

The user reported (after testing all four):

**Over-exposed fractal:**
- Filmic ≈ Clip, with small midtone differences.
- MaxNorm has the strongest effect — best CMY-shift prevention.
- Reinhard sits between MaxNorm and Filmic.

**Properly exposed fractal:**
- MaxNorm ≈ Clip — small differences only in densest regions.
- Reinhard slightly fades but preserves color.
- Filmic starts to show CMY shifts.

All four kept; they cover different use cases.

## Explicitly rejected / out of scope

- **Chaotica "Highlight Power"** — non-linear power-curve knob that
  controls *where* in the brightness range bleach kicks in. Subtly
  different from `white_level` (which is linear) but most users won't
  reach for both. Revisit if anyone asks.
- **Linear-light render path / sRGB OETF** — separate, larger
  project. Our pipeline is sRGB-throughout; a linear refactor is
  worth doing but wasn't required to fix the hue-shift complaint.
- **HDR export (EXR / `.hdr`)** — already in
  [`CLAUDE.md` Medium Priority](../../CLAUDE.md). Decoupled.
- **Apophysis XML `white_level` parser** — `apophysis_xml.rs` has a
  TODO; current import defaults to 200.0.

## Config / API versioning

Strictly additive: two new `FractalConfig` fields, both with serde
defaults that match prior behavior. No schema break. Lands on config
v1; will roll into v2 when that ships. The `api/sync.rs` receive
path defaults both fields too — the v1 wire format doesn't need to
change.
