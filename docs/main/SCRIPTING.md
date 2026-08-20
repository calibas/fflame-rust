# Scripting reference

Scripts are [Rhai](https://rhai.rs) — Rust-flavoured, sandboxed, and
identical on desktop, in the browser and in Python. A script can only
call what this page lists; there is no filesystem, network or process
access to reach for.

Two kinds, declared on the first line:

```rhai
script("My Generator", "generator");   // builds a flame from scratch
script("My Modifier", "modifier");     // changes the flame already open
```

The shipped starters in `assets/scripts/` are written to be read — open
**Basic Random** for the general shape, **Mandala** for symmetry,
**Gnarl** for parameter feel, **Zoom Dive** for animation. Everything
below is the vocabulary they draw on.

**Determinism is a promise:** script + seed + params produce a
byte-identical flame everywhere, forever. Use the seeded helpers
(`rand`, `rand_int`, `chance`, `pick`, `shuffle`) — never wall-clock
time or unseeded randomness, neither of which exists here anyway.

---

## Transforms

A transform handle comes from `flame.add_transform()`,
`flame.transform(i)`, or the final/linked equivalents.

### Properties — read and write

```rhai
let t = flame.add_transform();
t.weight = 1.5;          // how often the chaos game picks it (probability)
t.color = 0.3;           // palette position, 0..1
t.color_speed = 0.9;     // 1 = keep incoming colour, 0 = jump fully to t.color
t.opacity = 1.0;
t.direct_color = 0.0;    // blend toward variation-written colour
```

The six affine coefficients are properties too — `a`, `b`, `c`, `d`,
`e`, `f`, in the Apophysis convention `x' = a·x + b·y + c`,
`y' = d·x + e·y + f`. Prefer the helpers below unless you need the raw
matrix. Every property reads as well as writes, which is what makes a
modifier a modifier:

```rhai
t.weight = t.weight * 1.2;      // nudge what you found
let w = flame.transform(0).weight;
```

### Placement

```rhai
t.translate(dx, dy);     // relative
t.rotate(degrees);       // degrees, not radians
t.scale(factor);         // uniform
t.scale_xy(sx, sy);
t.set_affine(a, b, c, d, e, f);
```

### Post-affine

Applied after the variations. Same shape as the pre-affine surface:
raw coefficient properties `post_a`, `post_b`, `post_c`, `post_d`,
`post_e`, `post_f`, `post_g`, the switch `post_affine_enabled`
(bool), and the geometry helpers:

```rhai
t.post_affine_enabled = true;
t.post_a = rand(0.95, 1.05);

t.post_translate(dx, dy);
t.post_rotate(degrees);
t.post_scale(factor);
t.post_scale_xy(sx, sy);
t.set_post_affine(a, b, c, d, e, f);
```

The helpers **enable the post affine automatically** — calling
`post_rotate` declares you want one. The raw `post_*` properties do
not touch the switch, so you can stage coefficients and flip
`post_affine_enabled` yourself.

### Variations

```rhai
t.add_variation("spherical", 0.8);      // add, or set the weight if present
t.remove_variation("linear");
t.has_variation("julian");               // bool
t.variation_names();                     // array of names on this transform
t.variation_weight("julian");            // f64
t.variation_param("julian.power");       // f64
t.set_variation_param("julian.power", 3);
```

Parameter keys are `"variation.param"`. Unknown variations and unknown
parameter names are **errors**, not silent no-ops — with suggestions.

### Advanced placement

```rhai
t.set_mobius(re_a, im_a, re_b, im_b, re_c, im_c, re_d, im_d);
t.set_inversion(cx, cy, radius);
t.set_matrix3d(#{ xx: .., xy: .., ... , tx: .., ty: .., tz: .. });
t.set_segment(#{ x1: .., y1: .., x2: .., y2: .., thickness: .. });
t.area_scale();          // |determinant| — how much this map shrinks area
t.index();               // its position in the pool
```

## The flame

```rhai
flame.add_transform();
flame.add_final_transform();
flame.add_linked_transform();
flame.transform(i);
flame.final_transform(i);
flame.transform_count();
flame.final_count();
flame.remove_transform(i);
flame.clear_transforms();
flame.name = "Aurora";                   // also read: flame.name
```

### Final and linked transforms must be attached

**This is the one that catches everyone.** `add_final_transform()` puts
a transform in a *pool*; it does nothing until some normal transform
references it. A final you build and never attach produces no error, no
warning, and no visible difference — it simply never runs.

```rhai
let fin = flame.add_final_transform();
fin.add_variation("spherical", 1.0);
fin.attach_to_all();                 // ← the classic Apophysis "global final"
```

```rhai
fin.attach_to(2);        // just normal transform 2
fin.detach_from(0);      // take it back off
fin.attached_to();       // array of normal indices it runs on
```

`attach_to_all()` attaches to the normals that exist **when it is
called** — build your transforms first, or call it again afterwards.
Attaching twice is a no-op, so a modifier can call it safely.

Attachment lists are **ordered**, and these append: attach several
finals in the order you want them chained. Linked transforms use the
same four calls and their own list.

**Final vs. linked** — finals are a *view filter*: they shape only what
gets plotted, their colour writes are discarded, and their output does
not feed the next iteration. Linked transforms are part of the
*dynamics*: their output does feed forward. Reach for a final when you
want to bend the picture, a linked when you want to change the attractor.

### Contractiveness — the one number that decides haze vs. structure

```rhai
flame.contractiveness();                 // read it
flame.set_contractiveness(-0.25);        // set it
```

The **weighted average** across all transforms, not any single one:
expansions must be paid for by contractions elsewhere, or by being
picked rarely (weight is a probability). Below zero converges.
`set_contractiveness` scales every transform by one shared factor, so
each keeps its own character.

Caveat worth knowing: this measures the **affine part only**. Curved
variations change the picture completely — `spherical` keeps its output
bounded no matter how expansive the affine feeding it — so treat the
number as a guide, not a verdict.

### Palette

```rhai
flame.set_palette("Fire");               // by name
flame.random_palette();                  // from the library
flame.set_palette_colors([c1, c2, c3]);  // Color values, evenly spaced
flame.set_palette_stops([#{ pos: 0.0, color: c1 }, ..]);
flame.set_palette_fixed(colors);         // no interpolation between stops
flame.palette_colors();                  // read the current palette
flame.palette_to_fixed(n);
```

### Effects and xaos

```rhai
flame.add_effect("plasma");
flame.set_effect_param("plasma", "scale", 2.0);
flame.set_xaos(from, to, weight);        // chaos-game transition weights
flame.clear_xaos();
```

## Config

Any field of a saved `.fflame` by name — open one in a text editor to
see what exists.

```rhai
config.set("brightness", 4.0);
config.set("camera_rotation_x", 30.0 * PI() / 180.0);
let z = config.get("zoom");
```

`get` **throws when the field is at its default**, because settings are
only stored when they differ. Guard it:

```rhai
let start = 1.0;
try { start = config.get("zoom"); } catch {}
```

Angles are stored in **radians**; the View panel converts for display.

## Animation

Touching `anim` at all makes the script emit a `.anim` alongside the
flame. A script that never mentions it produces none.

```rhai
anim.name = "Turntable";
anim.duration = 8.0;
anim.key("zoom", 0.0, 1.0, "linear");        // target, time, value, easing
anim.interpolation("zoom", "exponential");
```

**Interpolation:** `"step"` / `"linear"` / `"smooth"` / `"sinusoidal"` /
`"exponential"`. Use **exponential for zoom** — it moves by equal ratio
per unit time, so every doubling takes the same number of seconds and
the dive doesn't appear to slam on the brakes. It reads identically in
both directions.

**Easing** (per key): `"linear"`, `"ease_in"`, `"ease_out"`,
`"ease_in_out"`, and the quad/cubic/sine variants.

Transforms carry their own `key`, since the handle already knows its
pool and index:

```rhai
t.key("weight", 0.0, 0.5);
```

## Parameters — the sliders your script gets

Declared at the top; they appear as controls in the Scripts panel.

```rhai
let n      = param_int("count", 3, 1, 10);          // name, default, min, max
let spread = param("spread", 1.0, 0.0, 3.0);
let fancy  = param_bool("fancy", false);
let mode   = param_choice("mode", ["A", "B", "C"], 0);   // default is an INDEX
let label  = param_string("label", "hello", 32);         // max length
let tint   = param_color("tint", "#ff8800");
```

`param_choice` returns the **chosen string**, so compare against the
option text: `if mode == "Ease in and out" { .. }`.

## Randomness — seeded, reproducible

```rhai
rand(lo, hi);            // float
rand_int(lo, hi);        // integer, inclusive
chance(p);               // true with probability p
pick([a, b, c]);         // one element
shuffle(array);
seed();                  // this run's seed, to hand to run_script
```

## Colour

```rhai
color(r, g, b);          // 0..1 floats
color_hex("#ff8800");
color_hsv(h, s, v);      // h in degrees
c.hex(); c.r; c.g; c.b; c.h; c.s; c.v;
c.with_hue(200.0); c.with_saturation(0.5); c.with_value(0.8);
c.rotate_hue(30.0); c.mix(other, 0.5);
```

## Calling another script

```rhai
run_script("random_palette", #{ scheme: "Analogous", stops: 5 }, seed());
```

Pass `seed()` rather than letting the callee continue your random
stream: the result then matches running that script standalone at the
same seed, which makes it a starting point the user can go and adjust.

## Querying the engine

```rhai
variation_names();               // every registered variation
variation_exists("julian");
variation_always_z("zcone");     // writes Z unconditionally?
palette_names();
effect_exists("plasma");
```

## Built-in geometry

Heavy math that would be painful in script, exposed as one call. See the
shipped L-system and Kleinian starters for use in context.

**L-systems:** `lsystem`, `lsystem_bounds`, `lsystem_bounds3`,
`lsystem_pieces3`, `lsystem_curve_pieces3`, `lsystem_graph_pieces`,
`lsystem_node_pieces`, `lsystem_plant_pieces`, `lsystem_uses_3d`,
`lsystem_mirror_symbol`, `lsystem_reverse_symbol`, `segment_symbol`,
`normalize_segments`, `turtle`.

**Decompositions:** `klein_generators`, `schottky_generators`,
`apollonian_generators`, `sphere_packing_mirrors`, `hilbert3d_maps`,
and the xaos-row helpers `avoid_xaos_row`, `exclude_xaos_row`,
`repeat_xaos_row`.

## Output

```rhai
print("built a flame with " + count + " transforms");
```

`print` output appears in the Scripts panel. Errors carry the line
number where one applies.

## Limits

Scripts run under budgets — operation count, call depth, array and
string sizes — so a runaway loop in a shared script terminates cleanly
instead of hanging the app. Transform counts stop at the renderer's
limit. These are not tunable from script.

---

**Staleness:** `every_script_api_name_is_documented` (in
`src/script/api.rs`) reads the registrations out of the source and
fails if one is missing from this page. Adding a script API means
adding a line here.
