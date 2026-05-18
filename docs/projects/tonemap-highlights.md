# Tonemap: hue-preserving highlights

## Goal

Fix the per-channel RGB clip that pushes bright colors toward
cyan/magenta/yellow/white at high exposure, gamma, brightness — any
setting that drives channels past 1.0. Every palette exhibits the
same drift: orange `(1, 0.5, 0)` × exposure 5 → `(5, 2.5, 0)` → clamps
to pure yellow; peach × 5 clamps to white. The current "correct"
exposure range is tiny because the per-channel clamp at
[`shaders/tonemap.wgsl:559`](../../shaders/tonemap.wgsl) is a hard
cliff — any single channel hitting 1.0 starts shifting hue
immediately.

Two reference apps handle this differently:

- **Apophysis / JWildfire**: per-channel clip (same problem as us).
  JWildfire exposes a `whiteLevel` parameter ("Fade to White") that
  divides chroma in the log-density curve while leaving alpha
  untouched, so bright dense pixels bloom toward the background via
  alpha blend before RGB clips. Hue-preserving, density-dependent.
  See
  [`LogDensityFilter.java:850`](https://github.com/thargor6/JWildfire/blob/master/src/org/jwildfire/create/tina/render/LogDensityFilter.java)
  and
  [`LogScaleCalculator.java:75`](https://github.com/thargor6/JWildfire/blob/master/src/org/jwildfire/create/tina/render/LogScaleCalculator.java).
- **Chaotica**: hue-preserving by default. Exposes a "Highlight Power"
  knob (power-curve, non-linear) that opts users *into* bleach-to-white
  behavior in the brightest regions specifically. Decided to skip this
  for now — `white_level` + `MaxNorm` covers the same ground without a
  redundant knob.

We already have JWildfire's math in our shader at
[`shaders/tonemap.wgsl:466`](../../shaders/tonemap.wgsl) — `let fp3 =
ls * bucket_count * tonemap_params.white_level` — but the value is a
hard-coded constant (`DEFAULT_WHITE_LEVEL = 200.0` in
[`src/config/defaults.rs:36`](../../src/config/defaults.rs)), never
exposed in config or UI. So the cheapest possible fix is "expose what
we already have."

## Phases

### Phase 0 — Expose `white_level` as "Highlights" slider

The smallest possible change. No shader math changes.

- **Field**: add `white_level: f32` to
  [`FractalConfig`](../../src/config/fractal_config.rs), default
  `DEFAULT_WHITE_LEVEL = 200.0` (preserves current behavior on load).
- **ConfigPath**: add `WhiteLevel` variant in
  [`src/config/delta.rs`](../../src/config/delta.rs) — Display, i18n
  key, `UpdateType::ToneMappingOnly`, to_string/from_string, include
  in the tone-mapping bulk lists at lines 2359 and 2837.
- **ConfigManager**: get_value + apply in
  [`src/config/manager.rs`](../../src/config/manager.rs).
- **TonemapParams plumbing**: replace `white_level: DEFAULT_WHITE_LEVEL`
  with `white_level: config.white_level` at the 5+ construction sites
  in [`src/renderer/compute_kernel.rs`](../../src/renderer/compute_kernel.rs)
  and [`src/export/high_res.rs`](../../src/export/high_res.rs).
- **UI**: add slider in
  [`src/ui/tone_mapping.rs`](../../src/ui/tone_mapping.rs), labeled
  **"Highlights"** (not "Fade to White" or "White Level" — clearer for
  end users). Range `50.0..=1000.0` log-style. Wire into preset apply
  block alongside `Exposure`/`Gamma`/etc.
- **i18n**: `tonemap.highlights` + `tonemap.tooltip_highlights` in
  [`locales/en.yml`](../../locales/en.yml). Tooltip explains: lower =
  more saturated highlights, higher = bleaches to white.
- **Migration**: old `.fflame` files without the field deserialize
  via serde default to 200.0 → identical render.

Stop point. Probably solves the user's "narrow correct range"
complaint on its own.

### Phase 1 — `HighlightMode` enum (Clip vs MaxNorm)

Address the per-channel clamp at
[`shaders/tonemap.wgsl:559`](../../shaders/tonemap.wgsl) directly.

- **Enum**: new `HighlightMode { Clip, MaxNorm }` in
  [`src/scene/tonemap.rs`](../../src/scene/tonemap.rs) alongside
  `ToneMapMode`. Default `Clip` (Apophysis-compatible — back-compat).
- **Shader**: replace `color = clamp(color, 0, 1)` with a uniform
  branch:
  - `Clip` (mode `0u`): current per-channel clamp.
  - `MaxNorm` (mode `1u`): `let m = max(color.r, max(color.g,
    color.b)); if (m > 1.0) { color /= m; }` — exact hue
    preservation.
- **Config**: `highlight_mode: HighlightMode` field, new `ConfigPath`
  variant, get/apply, UI dropdown next to existing `ToneMapMode`
  dropdown.

### Phase 2 — Reinhard + Filmic (optional)

Only if Phase 0 + 1 don't feel like enough. Adds two more variants to
`HighlightMode`:

- **Reinhard**: luminance-preserving. `let L = dot(color,
  vec3(0.2126, 0.7152, 0.0722)); let L_mapped = L / (1 + L); color *=
  L_mapped / max(L, epsilon);`. Smooth roll-off, slight desaturation in
  highlights.
- **Filmic** (ACES knee or Hejl-Burgess-Dawson): the film-look
  S-curve. Slight contrast boost, more "cinematic." ~10 lines WGSL.

Pure shader additions, no new uniform params, no UI work beyond two
more enum dropdown entries.

## Out of scope

- **Chaotica "Highlight Power"** — overlaps with `white_level` in
  function. Revisit if a user explicitly asks.
- **Linear-light render path / sRGB OETF** — separate, larger project.
  Our pipeline is sRGB-throughout right now; a linear refactor is
  worth doing but isn't required to fix the hue-shift complaint.
- **HDR export (EXR / HDR formats)** — already in
  [`OPTIONAL.md` "Medium Priority"](../../CLAUDE.md). Decouple.

## Testing

- **Visual regression**: existing `.fflame` baselines must produce
  identical hashes after Phase 0 (default `white_level = 200.0`
  matches the previous constant). Add new baseline configs with
  non-default `white_level` for coverage.
- **Manual**: load a preset known to drift to white at high exposure
  (anything with strong oranges/cyans), crank exposure to 5×, verify
  with `Highlights` slider that saturation is preserved at higher
  values and the bleach behavior is recoverable at lower values.
- **MaxNorm correctness** (Phase 1): take an in-gamut color, push
  exposure 10×, confirm hue (as measured by atan2 of color vectors in
  some color space) is preserved exactly.

## Config / API versioning

This is additive (one new field per phase, all with defaults). No
schema break. Lands on config v1; rolls into v2 when that ships.
