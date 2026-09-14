# The camera lens: a variation on the escape engine's screen offset

A **lens** is a variation applied to the normalised screen offset
*before* the view scale, in every escape-time kernel. The fractal is
untouched; only which point each pixel samples changes.

```
pixel -> normalised offset n -> L(n) -> * span -> rotate -> + centre -> the formula
                                ^^^^
                                the lens
```

## 1. Why this is not a final transform

A flame's final transform maps attractor points **to** the screen. A
lens maps the screen **to** sample points. It is the other direction,
and two things follow.

**A lens need not be invertible.** Many-to-one is fine: a region of
the set appears more than once, kaleidoscope-fashion, and every pixel
still gets exactly one deterministic sample. That is why the eligible
set is far larger than the invertible variations mode D can accept.

**The picture is not the one the same variation gives as a final.**
`eyefish` as a lens is `eyefish`-inverse as a view. Measured radial
profiles (screen radius to sampled radius) put `eyefish` and `bubble`
on opposite sides of the identity, so the pair covers both bulge
directions; see §6.

## 2. What it applies to

Every escape kernel derives its sample point from a normalised screen
offset, and there are five sites. A lens that reaches only the first
would silently stop working at zoom ~14, which is where the direct
kernel hands over to the perturbed one.

| site | file | what it computes |
|---|---|---|
| direct kernel | `assembler.rs` ~228 | `d = (uv - 0.5) * span` |
| field / statistics kernel | `assembler.rs` ~4316 | the same |
| perturbed kernel | `assembler.rs` ~620 | `centered`, in PIXEL units |
| mode D planar walk | `assembler.rs` ~4590 | `uv` in `[-1/2, 1/2]`, into `ifs_evaluate` |
| mode D solid ray | `assembler.rs` ~4701 | `ifs_ray`, which the relight pass also calls |

The convention the lens sees is **half-height one**: `n.y` spans
`[-1, 1]` and `n.x` spans `[-aspect, aspect]`. That is the scale
variations are written for — the unit disc is where `fisheye`,
`spherical` and the rest do something recognisable — and each site
converts into it and back. The perturbed site divides by `height/2`
rather than by a span, because its offset is in pixels.

The recolor pass needs no change of its own: mode D's cached walk
already has the lens baked in, and its relight derives the ray from
`ifs_ray`, which carries the lens.

## 3. How the variation gets into the shader

**Not by hand.** The simulation engine already splices arbitrary
variation WGSL into a non-flame shader, and its
`every_variation_validates_in_the_layer_warp` test asserts that all
647 do. This reuses that machinery rather than reimplementing the
part of it that decides which helper library a variation needs.

- `ShaderBuilder::build_layer_map(&flame)` emits the variation
  functions, the helper libraries, the packed `get_param`, the buffer
  declarations and a `flame_map(xform_id, u, seed) -> vec2<f32>`. It
  rewrites `@group(0)` to `@group(1)`, so the host keeps group 0.
- The escape templates gain a `//__LENS__` marker and call
  `flame_map`.
- The renderer builds a one-transform flame from the config, packs it
  with `pack_gpu_transforms` / `pack_gpu_variation_params`, and binds
  group 1 exactly as `SimRenderer::set_layer_transforms` does.

Because the layer map carries the whole variation system, no
variation needs refusing on shader grounds — RNG, colour-writing and
`needs_transform` variations all compile. Whether they are *good*
lenses is a separate question, answered by measurement in §6.

## 4. Storage and the UI

**Storage is escape-native.** `EscapeConfig` gains three fields
shaped like the `formula_params` pair it already has, so `.fflame`
files stay byte-stable for anything that does not use a lens:

```rust
lens: String,                        // variation name, empty = none
lens_params: BTreeMap<String, f32>,  // name-keyed, like formula_params
lens_amount: f32,                    // 1.0 = full, blends to identity
```

`lens_amount` is not decoration. A lens is all-or-nothing otherwise,
and the sim's layer warp already found the same need
(`mix(q, mapped, rate)`). It blends `L_t(n) = (1-t)·n + t·L(n)`, so
dialling an effect in is one slider.

Paths: `EscapeLens`, `EscapeLensAmount`, and
`EscapeLensParam { param: String }` keyed by name, matching
`EscapeFormulaParam`.

**The UI is variation-native, and reuses what exists.**
`render_variation_params` already renders the whole `ParamType` zoo —
Float, UnlimitedFloat, Integer, UnlimitedInteger, Boolean, Angle,
Enum — with tooltips, undo coalescing and the
"quantising widget must not rewrite the flame on first draw" rule
that cost a real bug to learn. The escape panel must not grow a
second, poorer copy of that.

It is hard-wired to `TransformRef` in exactly two places: reading the
stored value, and building the `ConfigPath`. So it grows a
`ParamTarget` trait with those two methods, `TransformRef` implements
it, and the lens implements it against `lens_params` with the
registry default as the fallback. Behaviour for transforms is
unchanged, and a test pins that.

## 5. The Jacobian, and what it is for

A lens magnifies by a different factor at different points, so the
world footprint of a pixel varies across the frame. Two consequences,
and only one of them is a correctness question.

`distance_estimate` returns `-log2(d)·scale` with `d` in **world**
units, not pixels. A lens shifts it by `log2(J)` — a smooth shading
change, not a break. Making edge width uniform in *screen* space
would need `J`, and that is a preference.

Mode D is the one to check rather than assume: if its hit test
compares a distance against a pixel footprint, a lens that magnifies
changes what counts as a surface. **Step 4 below settles this by
reading the code, before any claim about it.**

`J` is two extra variation evaluations per pixel by central
difference, which is nothing against the iteration count.

## 6. Which variations are worth offering

Measured, not guessed, by `variation_probe lens` (see
`src/probe/lens.rs`): every variation evaluated over a screen grid,
then classified.

| class | count | meaning |
|---|---|---|
| clean | 285 | finite, in frame, neighbours stay neighbours |
| unbounded | 20 | a pole throws part of the frame away — `spherical` is the type |
| degenerate | 29 | collapses; every z-only variation, which returns nothing in 2D |
| broken | 313 | non-finite or shattering; mostly the RNG blur and noise families |

Radial profile, screen radius to sampled radius:

| lens | 0.25 | 0.5 | 0.75 | 1.0 |
|---|---|---|---|---|
| eyefish / fisheye | 0.40 | 0.67 | 0.86 | 1.00 |
| bubble | 0.25 | 0.47 | 0.66 | 0.80 |
| hemisphere | 0.24 | 0.45 | 0.60 | 0.71 |
| spherical | 4.00 | 2.00 | 1.33 | 1.00 |

`fisheye` is `eyefish` with x and y transposed, to 1.9e-6 — the
Apophysis quirk, verbatim in our port. As a lens that mirrors the
picture across the diagonal, so **`eyefish` is the one to reach for**
and the panel says so.

The picker offers every variation in the app's ordinary ordering
rather than a curated allowlist. Curation here would be a judgement
the measurement does not support: "broken" describes the default
parameters, and a variation's parameters are editable.

## 7. Steps

1. **Config and paths.** The three fields, the three `ConfigPath`
   variants and their five arms each, the manager's read and write
   sides, the string-key round trip. Gate: existing `.fflame` files
   unchanged, `escape_paths_round_trip_their_string_keys` extended.
2. **Shader and renderer, direct path.** `//__LENS__`, the group-1
   bind group, the pipeline key, the half-height-one conversion at
   the direct site. Gate: `linear` as the lens is byte-identical to
   no lens, and every variation as a lens validates under naga.
3. **UI.** The `ParamTarget` refactor, the lens section, the picker.
   Gate: transforms behave identically; a lens param edit round-trips
   through undo.
4. **The remaining four sites**, and the Jacobian question answered
   from the code rather than assumed.
5. **Presets and the visual suite.**

## 8. Record

*(Filled in as steps land.)*
