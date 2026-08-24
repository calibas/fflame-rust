# Scripting guide

Scripts build and modify flames. They are written in
[Rhai](https://rhai.rs) — a small Rust-flavoured language — run inside
a sandbox with no file, network or process access, and behave
**identically** on desktop, in the browser and in Python. The same
script with the same seed and parameters produces a byte-identical
flame everywhere, forever. That promise is load-bearing: a script +
seed is a shareable artifact, and the test suite enforces it down to
the RNG's bit stream.

This guide covers the language and the working patterns. Every
callable function is listed with its signature in
[SCRIPTING.md](SCRIPTING.md) — the reference a staleness test keeps
complete. The shipped starters in `assets/scripts/` are written to be
read and are the best worked examples: **Basic Random** for the
general shape, **Mandala** for symmetry, **Gnarl** for parameter feel,
**Zoom Dive** for animation, **Decompose Group** for heavy machinery.

## The shape of a script

```rhai
// Orbit Ring
//
// The header comment is the script's description — the panel shows it,
// and the first line becomes its title.
script("Orbit Ring", "generator");          // ← always first

let count = param_int("count", 5, 2, 12);   // parameters next: they
let spread = param("spread", 1.0, 0.2, 3.0); //   become sliders

// ...then do the work: build transforms, set the palette, print.
```

- `script(name, kind)` must be the first call — before any `param`.
  The kind is `"generator"` (starts from a default config) or
  `"modifier"` (starts from the flame currently open). An optional
  third argument lists flags: `script("T", "modifier", ["norng"])`.
- **Parameters make sliders.** The panel discovers them by running the
  script once in a *collect* pass before the real run, so declare them
  unconditionally at the top — a `param()` hidden behind an `if` only
  appears once that branch has been taken.
- A `"norng"` flag tells the panel the script ignores the seed (hides
  the reroll controls); `"palette"` offers it in the Palette Editor.

## Rhai in ten minutes (for programmers)

Rhai will feel like Rust with the types relaxed. What you need:

```rhai
let x = 1;                  // int (i64)
let y = 1.5;                // float (f64) — a DIFFERENT type
let s = "text";             // string
let ok = true;
let a = [1, 2.0, "three"];  // arrays are heterogeneous
let m = #{ x: 1, y: 2 };    // object map — note the #
```

**Int and float are distinct, and native arithmetic does not mix
them.** `1/3` is integer division (`0`), and `1 + 0.5` is an error in
pure Rhai. Convert explicitly: `n.to_float()`, `f.to_int()`. The one
place you never worry about it: **every function this app registers
accepts an int where a float is wanted** — `t.weight = 1` works. The
pitfall is only in your own arithmetic:

```rhai
let frac = i.to_float() / count.to_float();   // NOT i / count
```

Control flow is Rust-shaped, and blocks are expressions:

```rhai
for i in 0..count { ... }          // half-open range
for item in my_array { ... }
while x < 10 { ... }
loop { ...; if done { break; } }
let kind = if big { "major" } else { "minor" };   // if is an expression
switch x { 1 => ..., 2 => ..., _ => ... }
```

Functions and closures:

```rhai
fn wobble(t, amount) {              // top-level fn; last expression
    amount * sin(t)                 //   is the return value
}
let double = |v| v * 2;             // closure
```

Note one Rust difference: a top-level `fn` is **pure** — it cannot see
outer variables like `flame` (pass what it needs as arguments, or use
a closure, which does capture).

Errors and strings:

```rhai
try { start = config.get("zoom"); } catch { }   // catch and continue
let msg = "count is " + count;      // + concatenates and converts
let name = `flame ${i} of ${n}`;    // backtick strings interpolate
s.trim(); s.sub_string(0, 3); s.len();
a.push(v); a.len(); a[0];
```

The standard math you expect is present and works on floats: `sin`,
`cos`, `tan`, `sqrt`, `abs`, `floor`, `ceil`, `exp`, `ln`, `min`,
`max`, and `PI()` as a function. Angles for `sin`/`cos` are radians;
angles in this app's API (`rotate`, `color_hsv` hue) are **degrees**
where the reference says so.

What is deliberately missing: `eval` is disabled, there is no
`import`, no file or network access, no clock, and no unseeded
randomness. If it isn't in the [reference](SCRIPTING.md) or Rhai's
core language, it isn't there.

## The three globals

Every script gets three objects:

- **`flame`** — the structure: transforms, palette, effects, xaos.
  This is where most work happens, through typed, validated calls.
- **`config`** — everything else in a `.fflame`, by the same key names
  the saved JSON uses: `config.set("brightness", 4.0)`,
  `config.set("camera_rotation_x", 30.0 * PI() / 180.0)`. Open any
  saved `.fflame` in a text editor to discover what's settable.
- **`anim`** — optional keyframes. Touching `anim` at all makes the
  script emit a `.anim` alongside the flame; ignoring it emits none.

One sharp edge on `config.get`: settings equal to their default are
not stored, and reading an absent key **throws**. Guard it:

```rhai
let zoom = 1.0;
try { zoom = config.get("zoom"); } catch { }
```

## Randomness — seeded, reproducible

`rand()`, `rand(lo, hi)`, `rand_int(lo, hi)`, `chance(p)`,
`pick(array)`, `shuffle(array)` all draw from one seeded stream.
Same seed → same draws → same flame; the reroll button is just
seed + 1. Consequences worth designing for:

- Draw in a **stable order**. Adding an early `rand()` call reshuffles
  everything after it — that's expected, but know that editing a
  script redefines what its seeds produce.
- `seed()` returns this run's seed, mainly to hand to `run_script` so
  a sub-script reproduces the way it would standalone.

## Running scripts

**In the app:** Scripts panel (the Scripting layout arranges it beside
the viewport and browser). Parameters appear as sliders; `print(...)`
output and errors appear in the panel; errors carry line numbers.

**From the command line** — how the examples below were verified:

```bash
FractalArtEditor generate --script my_gen.rhai --seed 42 -o out.fflame
FractalArtEditor generate --script my_mod.rhai --base in.fflame -o out.fflame
FractalArtEditor generate --script my_gen.rhai --list-params
FractalArtEditor generate --script my_gen.rhai --set "count=7" --set "style=Bold"
FractalArtEditor export -i out.fflame -o out.png --width 1920 --height 1080
```

`--seed` wraps modulo 2⁶⁴ (so `--seed -1` is the last seed on the
ring). A modifier without `--base` runs against the default config,
which is valid but rarely what you meant.

## Worked examples

### A generator: ring of transforms

The core moves of most generators: build transforms in a loop, place
them geometrically, add variations, set colours, keep the system
contractive.

```rhai
// Orbit Ring
//
// A ring of transforms orbiting a spherical core — the classic
// two-ingredient flame: something that scatters, something that pulls.
script("Orbit Ring", "generator");

let count  = param_int("count", 5, 2, 12);
let radius = param("radius", 0.9, 0.2, 2.0);
let curl   = param("curl", 0.4, 0.0, 1.0);

for i in 0..count {
    let t = flame.add_transform();
    t.add_variation("linear", 1.0);

    // Place each copy on the ring: shrink, then step around it.
    t.scale(0.55);
    let angle = 360.0 * i.to_float() / count.to_float();
    t.rotate(angle);
    t.translate(radius * cos(angle * PI() / 180.0),
                radius * sin(angle * PI() / 180.0));

    // Spread the palette around the ring; keep colour moving.
    t.color = i.to_float() / count.to_float();
    t.color_speed = 0.7;

    if chance(curl) {
        t.add_variation("swirl", rand(0.1, 0.3));
    }
}

// The core: heavily weighted, so orbits keep falling through it.
let core = flame.add_transform();
core.add_variation("spherical", 1.0);
core.weight = 2.0;
core.color = 0.5;

// A flame diverges into haze when it expands on average. Pull the
// weighted mean below zero and every transform keeps its character.
if flame.contractiveness() > -0.1 {
    flame.set_contractiveness(-0.25);
}

flame.set_palette_colors("Ring", [
    color_hex("#1b2a49"), color_hsv(rand(0.0, 360.0), 0.8, 1.0),
    color_hex("#f0e0c0"),
]);
print("ring of " + count + ", contractiveness "
      + flame.contractiveness());
```

### A modifier: read what's there, then bend it

A modifier's discipline is *nudging what it finds* rather than
overwriting it — and the final-transform idiom is the one everyone
trips on: a final lives in a pool and **does nothing until attached**.

```rhai
// Lens
//
// Puts the whole image through a Julia lens (an attached final
// transform) and thickens whatever spherical the flame already has.
script("Lens", "modifier");

let power = param_int("power", 2, 2, 6);

// Nudge, don't replace: scale existing weights instead of setting them.
for i in 0..flame.transform_count() {
    let t = flame.transform(i);
    if t.has_variation("spherical") {
        let w = t.variation_weight("spherical");
        t.add_variation("spherical", w * 1.2);
    }
}

// The lens. add_final_transform() only fills the pool —
// attach_to_all() is what makes it run, on every normal transform
// that exists RIGHT NOW (so attach after building).
let lens = flame.add_final_transform();
lens.add_variation("julian", 1.0);
lens.set_variation_param("julian.power", power);
lens.post_rotate(rand(-15.0, 15.0));    // enables the post affine itself
lens.attach_to_all();

print("lens power " + power + " over "
      + flame.transform_count() + " transforms");
```

### An animation: keyframes ride along with the flame

```rhai
// Slow Turn
//
// A turntable with a breathing zoom. Exponential interpolation moves
// by equal RATIO per second — the only zoom that feels constant.
script("Slow Turn", "modifier", ["norng"]);

let seconds = param("seconds", 8.0, 2.0, 30.0);

anim.name = "Slow Turn";
anim.duration = seconds;

// Flame-level targets use the same names config.set takes.
anim.key("rotation", 0.0, 0.0);
anim.key("rotation", seconds, 360.0);

anim.key("zoom", 0.0, 1.0);
anim.key("zoom", seconds / 2.0, 1.6, "ease_in_out");
anim.key("zoom", seconds, 1.0, "ease_in_out");
anim.interpolation("zoom", "exponential");

// Per-transform keys live on the handle: weight, colour, an affine
// coefficient, or a "variation.param".
if flame.transform_count() > 0 {
    let t = flame.transform(0);
    t.key("weight", 0.0, t.weight);
    t.key("weight", seconds / 2.0, t.weight * 1.5);
    t.key("weight", seconds, t.weight);
}
```

### Composition: scripts calling scripts

Any shipped or saved script can be called by its id (its file stem).
The callee works on the **same flame** — that is the return value.

```rhai
run_script("random_palette");                       // continue my stream
run_script("iq_palette", #{ preset: "Rainbow" });   // with parameters
run_script("jitter", #{}, seed());                  // reproduce standalone
```

Passing `seed()` makes the sub-script produce exactly what it would
produce run by itself at that seed — the right choice when the user
might re-run it alone to adjust. Without a seed it continues your
stream, so two calls give two different results.

## Gotchas, collected

- **Finals and linkeds must be attached** (`attach_to_all()` /
  `attach_to(i)`), and attachment happens against the normals that
  exist at call time.
- **`config.get` throws on unset keys** — settings at their default
  are omitted from storage. `try`/`catch` with a fallback.
- **`param_choice` returns the option string**, not an index.
- **Camera angles are radians** in `config.set`; `rotate()` and hue
  are degrees. The reference marks units on every entry.
- **`remove_transform` shifts indices.** Handles point at positions,
  not identities — re-fetch handles after structural changes, or
  delete from the highest index down.
- **A modifier that adds `rand()` calls changes nothing visibly** when
  run twice with one seed — same stream, same draws. That's the
  design; use the seed control to explore.
- **The operation budget is shared** across a script and everything it
  `run_script`s — about five million interpreter steps. A runaway loop
  is stopped with an error naming the budget, not a hang.

## Limits (not tunable from script)

| limit | value |
|---|---|
| interpreter operations, shared incl. sub-scripts | 5,000,000 |
| function call depth | 64 |
| `run_script` nesting depth | 8 |
| array length | 100,000 |
| string length | 1,000,000 |
| map size | 10,000 |
| script source size | 256 KB |
| transforms per flame (normals + linked + finals) | 128 |
