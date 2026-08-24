# Scripting reference

Every function a script can call, with signatures. The companion
[SCRIPTING-GUIDE.md](SCRIPTING-GUIDE.md) covers the language, how to
run scripts, and worked examples; this page is the complete
vocabulary. Anything not listed here (or core Rhai) does not exist in
the sandbox — no filesystem, no network, no clock.

Conventions used below:

- `float` accepts an int anywhere — every numeric entry point coerces,
  so `t.weight = 1` works.
- Errors are real errors with line numbers, not silent no-ops.
  Unknown variation names, bad parameter keys and out-of-range indices
  all say what they know (usually with the valid options).
- **Degrees vs radians** is marked per entry. House rule: this API's
  own angles are degrees; `config.set` stores camera angles in
  radians; Rhai's `sin`/`cos` are radians.

The staleness test `every_script_api_name_is_documented`
(`src/script/api.rs`) extracts every registered name from the source
and fails if one is missing from this page or the guide.

---

## Declarations

| call | returns | what it does |
|---|---|---|
| `script(name, kind)` | — | Declares the script. Must be the first call. `kind` is `"generator"` (starts from a default config) or `"modifier"` (starts from the current flame). |
| `script(name, kind, flags)` | — | Same, with a flag array: `"norng"` (ignores the seed → panel hides reroll), `"palette"` (offered in the Palette Editor). Unknown flags warn and are dropped. |

## Parameters (become sliders in the panel)

Declared before other work; the collect pass discovers them by running
the script, so keep them unconditional. Labels are auto-generated from
the key (`"scale_max"` → "Scale Max").

| call | returns | what it does |
|---|---|---|
| `param(key, default, min, max)` | float | A float slider; returns the user's value clamped to `[min, max]`. |
| `param_int(key, default, min, max)` | int | An integer slider, inclusive range. |
| `param_bool(key, default)` | bool | A checkbox. |
| `param_choice(key, options, default_index)` | string | A dropdown; returns the **chosen option's text**, so compare against the option strings. |
| `param_string(key, default)` | string | A text field (up to 2048 characters). |
| `param_color(key, "#rrggbb")` | Color | A colour picker, declared with a hex default. |

## Randomness (seeded — same seed, same draws, everywhere)

| call | returns | what it does |
|---|---|---|
| `rand()` | float | Uniform in `[0, 1)`. |
| `rand(min, max)` | float | Uniform in `[min, max)`; returns `min` when `min >= max`. |
| `rand_int(min, max)` | int | Uniform integer, **inclusive** of both ends. |
| `chance(p)` | bool | True with probability `p`. |
| `pick(array)` | any | One element, uniformly; error on an empty array. |
| `shuffle(array)` | array | A seeded Fisher–Yates shuffle (returns the shuffled array). |
| `seed()` | int | This run's seed — mainly to pass to `run_script`. |

## The `flame` object

### Structure

| call | returns | what it does |
|---|---|---|
| `flame.add_transform()` | Transform | Appends a normal transform; errors past 128 total (normals + linked + finals share the budget). |
| `flame.add_final_transform()` | Transform | Appends to the final pool — **inert until attached** (see Attachments). |
| `flame.add_linked_transform()` | Transform | Appends to the linked pool — same attachment rule. |
| `flame.transform(i)` | Transform | Handle to normal transform `i`; bounds-checked. |
| `flame.final_transform(i)` | Transform | Handle to final transform `i`. |
| `flame.transform_count()` | int | Number of normal transforms. |
| `flame.final_count()` | int | Number of finals in the pool. |
| `flame.remove_transform(i)` | — | Removes normal transform `i`. Later indices shift down — refresh handles. |
| `flame.clear_transforms()` | — | Removes all normal transforms. |
| `flame.name` | string | Read/write property: the flame's name. |

### Convergence

| call | returns | what it does |
|---|---|---|
| `flame.contractiveness()` | float | Weighted mean log linear-scale of the normal transforms; below zero converges. Affine + variation weights only — a curved variation's own bounding is invisible, so treat as a guide. `-inf` when nothing carries weight. |
| `flame.set_contractiveness(target)` | float | Scales every transform's linear part by one shared factor to land the mean on `target`; returns that factor. |

### Xaos

`xaos[from][to]` biases which transform may follow which: `1.0`
neutral, `0.0` forbids the transition.

| call | returns | what it does |
|---|---|---|
| `flame.set_xaos(from, to, weight)` | — | Sets one entry; creates (and grows) the table as needed. Indices are normal-transform indices. |
| `flame.clear_xaos()` | — | Drops the whole table (all transitions neutral). |

### Effects

| call | returns | what it does |
|---|---|---|
| `flame.add_effect(name)` | — | Appends a post-effect instance; routed to the colour or density chain by its registered category. |
| `flame.set_effect_param(name, param, value)` | — | Sets a parameter on the **most recently added** instance of that effect; error if none was added. |

### Palette

| call | returns | what it does |
|---|---|---|
| `flame.set_palette(name)` | — | A palette from the loaded library, by name (case-insensitive). |
| `flame.random_palette()` | string | A seeded random library palette; returns its name. |
| `flame.set_palette_colors(name, colors)` | — | Builds a gradient from ≥ 2 Colors, evenly spaced. |
| `flame.set_palette_stops(name, stops)` | — | Builds from `[[position, color], ...]` pairs; positions clamp to `[0, 1]`, stops are sorted for you, at most 256. |
| `flame.set_palette_fixed(name, colors)` | — | Builds a **locked 256-slot** palette, resampling the given Colors to fill it. |
| `flame.palette_to_fixed()` | — | Converts the current palette to fixed 256-slot form (the Palette Editor's Fixed switch). |
| `flame.palette_colors()` | array of Color | The current palette's stops as Colors — read, remix, re-apply. |
| `palette_names()` | array of string | Every palette in the loaded library. |

## The `Transform` handle

Handles come from `add_transform()` / `transform(i)` and the
final/linked equivalents. A handle names a *position* in a pool;
after `remove_transform` it may point at a different transform or
error.

### Properties (read and write)

| property | meaning |
|---|---|
| `weight` | Selection probability in the chaos game (normals only meaningfully). |
| `color` | Palette position, 0–1. |
| `color_speed` | 1 = keep incoming colour, 0 = jump fully to `color`. |
| `opacity` | Plot opacity. |
| `direct_color` | Blend toward variation-written colour. |
| `a`, `b`, `c`, `d`, `e`, `f` | Pre-affine coefficients: `x' = a·x + b·y + e`, `y' = c·x + d·y + f`. |
| `g` | Z offset (the 3D translation). |
| `post_a`, `post_b`, `post_c`, `post_d`, `post_e`, `post_f`, `post_g` | The post-affine's coefficients, same convention. Writing them does **not** flip the switch: |
| `post_affine_enabled` | bool — whether the post affine runs at all. The `post_*` geometry helpers below set it for you; the raw properties leave it alone so you can stage values. |

### Placement helpers

| call | returns | what it does |
|---|---|---|
| `t.translate(dx, dy)` | — | Moves the transform (adds to `e`, `f`). |
| `t.rotate(degrees)` | — | Rotates the linear part. Degrees. |
| `t.scale(factor)` | — | Scales the linear part uniformly; placement (`e`, `f`) untouched. |
| `t.scale_xy(sx, sy)` | — | Anisotropic scale of the linear part. |
| `t.set_affine(a, b, c, d, e, f)` | — | Sets all six coefficients at once. |
| `t.post_translate(dx, dy)` | — | Same operations on the post affine — and each of these **enables** it. |
| `t.post_rotate(degrees)` | — | ” |
| `t.post_scale(factor)` | — | ” |
| `t.post_scale_xy(sx, sy)` | — | ” |
| `t.set_post_affine(a, b, c, d, e, f)` | — | ” |
| `t.index()` | int | The handle's position in its pool. |
| `t.area_scale()` | float | `abs(det)` of the linear part — how much this map scales area. |

### Variations

Parameter keys are `"variation.param"` — the same names the `.fflame`
JSON shows. Unknown names error with the valid options listed.

| call | returns | what it does |
|---|---|---|
| `t.add_variation(name, weight)` | — | Adds the variation, or sets its weight if present. |
| `t.remove_variation(name)` | — | Removes it (no error if absent). |
| `t.has_variation(name)` | bool | Is it on this transform? |
| `t.variation_names()` | array of string | This transform's variations, in canonical order. |
| `t.variation_weight(name)` | float | Its weight (0.0 if absent). |
| `t.variation_param("var.param")` | float | A parameter's value, falling back to the variation's declared default. |
| `t.set_variation_param("var.param", value)` | — | Set by dotted key… |
| `t.set_variation_param(var, param, value)` | — | …or by two names. |

### Attachments (finals and linkeds only)

A final or linked transform is a pool entry; it **runs only where
attached** to normal transforms. Lists are ordered (chains apply in
attachment order) and appends are idempotent.

| call | returns | what it does |
|---|---|---|
| `t.attach_to_all()` | — | Attach to every normal transform existing right now — the classic "global final". |
| `t.attach_to(i)` | — | Attach to normal transform `i`. |
| `t.detach_from(i)` | — | Remove the attachment. |
| `t.attached_to()` | array of int | Which normal transforms carry it. |

Calling any of these on a *normal* transform is an error explaining
that normals are what finals attach **to**.

### Prefab constructions

| call | returns | what it does |
|---|---|---|
| `t.set_mobius([re_a, im_a, re_b, im_b, re_c, im_c, re_d, im_d])` | — | Makes the transform a Möbius map (the `mobius` variation with its 8 params). |
| `t.set_inversion([cx, cy], r)` / `([cx, cy, cz], r)` | — | Makes it a circle/sphere inversion (affine conjugation around `spherical3D_wf`). |
| `t.set_segment([x1, y1, x2, y2])` | — | The similarity carrying the unit segment (0,0)–(1,0) onto the given one. |
| `t.set_segment(seg, mirror)` | — | Same, reflected across the segment — for mirror-paired curve rules. |
| `t.set_segment(seg, mirror, thickness)` | — | Squashed variant: the perpendicular axis is scaled by `thickness` (0 = the Barnsley stem map). |
| `t.set_matrix3d(piece, thickness)` | — | An exact 3D affine piece: the `matrix3D` variation from 12 row-major coefficients (a `*_pieces3` entry), off-axis columns scaled by `thickness`. |

### Per-transform keyframes

| call | returns | what it does |
|---|---|---|
| `t.key(target, time, value)` | — | A keyframe on this transform. Targets: `"weight"`, `"color"`, `"color_speed"`, `"opacity"` (normal transforms), an affine letter `"a"`–`"f"`, or a `"variation.param"`. |
| `t.key(target, time, value, easing)` | — | Same with an easing (see `anim.key`). |

## The `config` object

Everything outside the flame structure, by the exact key names a saved
`.fflame` uses (dotted paths reach nested groups). The flame itself is
refused here — use `flame`.

| call | returns | what it does |
|---|---|---|
| `config.set(key, value)` | — | Sets a field. A key that changes nothing warns (it may be misspelled — or a legitimate no-op). |
| `config["key"] = value` | — | The same, as an index assignment. |
| `config.get(key)` | any | Reads a field. **Throws when the field is unset** — defaults are omitted from storage — so wrap in `try`/`catch` with a fallback. |

Camera angles are stored in **radians**.

## The `anim` object

Touching `anim` at all makes the script emit a `.anim` alongside its
flame; never touching it emits none.

| call | returns | what it does |
|---|---|---|
| `anim.name` | string | Read/write: the animation's name. |
| `anim.duration` | float | Read/write: seconds; defaults to the last keyframe. |
| `anim.key(target, time, value)` | — | A keyframe on a flame-level setting — same names `config.set` takes (`"zoom"`, `"rotation"`, `"camera_rotation_x"`, …). |
| `anim.key(target, time, value, easing)` | — | With per-key easing: `"linear"`, `"ease_in"`, `"ease_out"`, `"ease_in_out"`, or the `_quad`/`_cubic` variants. |
| `anim.interpolation(target, mode)` | — | The whole track's interpolation: `"step"`, `"linear"`, `"smooth"`, `"sinusoidal"`, `"exponential"`. Use **exponential for zoom** — equal ratio per unit time reads as constant speed. |

## Colors

A `Color` is a value type; the modifier methods return a **new**
colour.

| call | returns | what it does |
|---|---|---|
| `color(r, g, b)` | Color | From 0–1 floats. |
| `color_hsv(h, s, v)` | Color | Hue in **degrees**; s, v in 0–1. |
| `color_hex("#rrggbb")` | Color | From a hex string. |
| `c.r`, `c.g`, `c.b` | float | Channels, 0–1. |
| `c.h`, `c.s`, `c.v` | float | HSV view (hue in degrees). |
| `c.rotate_hue(degrees)` | Color | Hue rotated. |
| `c.with_hue(h)` / `c.with_saturation(s)` / `c.with_value(v)` | Color | One component replaced. |
| `c.mix(other, t)` | Color | Linear blend, `t` in 0–1. |
| `c.hex()` | string | `"#rrggbb"` (also what printing a Color shows). |

## Calling other scripts

The callee runs on the **same flame** — that is its output. Ids are
file stems (`"random_palette"`, `"jitter"`, your saved scripts).
Nesting is capped at 8 and shares the caller's operation budget.

| call | returns | what it does |
|---|---|---|
| `run_script(id)` | — | Run it, continuing this script's RNG stream. |
| `run_script(id, params)` | — | With a parameter map: `#{ scheme: "Analogous", stops: 5 }`. |
| `run_script(id, params, seed)` | — | With its own seed — the callee reproduces exactly what it would produce standalone at that seed (pass `seed()` for "this run's"). |

## Engine queries

| call | returns | what it does |
|---|---|---|
| `variation_names()` | array of string | Every registered variation (500+). |
| `variation_exists(name)` | bool | Is it registered? |
| `variation_always_z(name)` | bool | Does it write Z unconditionally (survives `preserve_z = false`)? |
| `effect_exists(name)` | bool | Is the effect registered? |

(`t.variation_names()` — the per-transform version — is under the
Transform handle above. `palette_names()` is under Palette.)

## Built-in geometry

Heavy constructions exposed as single calls. The shipped **L-System
Curve**, **L-System Plant**, **Hilbert Curve 3D** and **Decompose
Group** scripts use all of these in context.

A *segment* below is `[x1, y1, x2, y2, depth, symbol]`. The maps
returned by the `*_pieces*` calls carry a `note` field: an empty note
is success, a non-empty one explains an empty result.

### L-systems

| call | returns | what it does |
|---|---|---|
| `lsystem(axiom, rules, depth)` | string | Expands the system `depth` times (≤ 32). `rules` is a map of single-character symbol → replacement. |
| `turtle(expanded, angle)` | array of segment | Walks the expanded string with a turtle turning `angle` degrees. |
| `lsystem_bounds(axiom, rules, depth, angle)` | array | `[min_x, min_y, max_x, max_y, depth_used]` of the drawn curve. |
| `lsystem_bounds3(axiom, rules, depth, angle)` | array | `[min_x, min_y, min_z, max_x, max_y, max_z]`, 3D turtle. |
| `lsystem_uses_3d(axiom, rules)` | bool | Does the rule set use the 3D turtle commands (`&`, `^`, `\`, `/`)? |
| `normalize_segments(segments)` | array of segment | Rescales a turtle path so its overall run is the unit segment — turning pieces into IFS contractions. Errors for closed paths. |
| `segment_symbol(seg)` | string | The symbol that drew a segment. |
| `lsystem_reverse_symbol(rules, primary)` | string | The symbol whose rule is the reverse of `primary`'s (or `""`) — its pieces need a reflected transform. |
| `lsystem_mirror_symbol(rules, primary)` | string | Same for the mirrored partner. |
| `lsystem_plant_pieces(axiom, rules, angle)` | map | `#{ branches, stems, note }` — the Barnsley-fern construction for bracketed rules; pieces are segments. |
| `lsystem_node_pieces(axiom, rules, angle)` | map | `#{ pieces, note }` — the node-rewriting (space-filling) construction. |
| `lsystem_graph_pieces(axiom, rules, angle)` | map | `#{ pieces, note }` — multi-variable pieces `[12 floats, depth, occ, owner]`; a piece may follow another only when it consumes what the other produced (wire with xaos). |
| `lsystem_curve_pieces3(axiom, rules, angle)` | map | `#{ maps, anchor, bounds, node, note }` — measured self-similar 3D pieces (12 floats each); refusals explained in `note`. |
| `lsystem_pieces3(axiom, rules, angle)` | map | `#{ branches, stems, note }` — the 3D plant/curve construction; pieces are `[12 floats, depth, symbol]`. |
| `hilbert3d_maps()` | array | Eight `[12 floats]` affine maps of a self-similar 3D Hilbert curve. |

### Kleinian and packing groups

Generator arrays are Möbius coefficient lists — feed each entry to
`t.set_mobius(...)`.

| call | returns | what it does |
|---|---|---|
| `schottky_generators(circles, twist_a, twist_b)` | array | From four `[x, y, r]` circles: the four generators `[a, a⁻¹, b, b⁻¹]`, 8 numbers each. |
| `apollonian_generators(deform, theta, eta, delta)` | array | The Apollonian gasket group's generators, optionally deformed. |
| `klein_generators(recipe, a_re, a_im, b_re, b_im, weight)` | array | A Kleinian group from recipe 0–6 and two complex traces; 4 × 8 numbers. |
| `sphere_packing_mirrors(mode, size, ring_n, ring_scale, cap_scale, jitter, tilt, three_d)` | array | Sphere-inversion packings as `[x, y, z, r]` mirrors — feed each to `t.set_inversion`. |

### Xaos rows for group dynamics

Each returns one row of weights; apply with
`for to in 0..n { flame.set_xaos(from, to, row[to]); }`.

| call | returns | what it does |
|---|---|---|
| `avoid_xaos_row(from)` | array | The Schottky "never undo the last generator" rule, for generator `from` of 4. |
| `exclude_xaos_row(forbidden, count)` | array | Forbids one index outright; the rest equal. |
| `repeat_xaos_row(from, count)` | array | Blocks repeating the same mirror (inversions are involutions). |

## Output

| call | returns | what it does |
|---|---|---|
| `print(text)` | — | Shows in the Scripts panel (and the CLI). `debug(...)` is captured the same way. |

## Limits

See the [guide's limits table](SCRIPTING-GUIDE.md#limits-not-tunable-from-script):
a shared 5M-operation budget (sub-scripts included), call depth 64,
`run_script` depth 8, array/string/map size caps, 256 KB source,
128 transforms. `eval` is disabled; there is no `import`. Runaway
scripts stop with an error naming the budget, not a hang.
