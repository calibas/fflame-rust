# Rhai twins of the solids and fields

Status: **steps 1-6 done 2026-11-10** (every solid and field twinned,
each with Edit as Rhai; the reference is ESCAPE.md, *The built-in
solids' twins*, *A Rhai field*, *The built-in fields' twins*). Branch
`rhai-solids-fields`. Part of [rhai-plugins.md](rhai-plugins.md), P7
("every built-in editable as Rhai"): the last built-in kinds without held
twins.

## What is to be twinned

| Kind | Built-ins | Where | Rhai today |
|---|---|---|---|
| Mode D solids | Mandelbox, Mandelbulb, KIFS, Quaternion Julia (`IFS_DEFS`; the two Flame Attractor entries are the flame itself, nothing to twin) | `escape/ifs.rs` | a Rhai solid kind (KIFS plan B3, `custom::compile_solid`); examples for Mandelbox, a KIFS Menger, quaternions, none held to a built-in |
| Mode B fields | Weierstrass, Markus-Lyapunov, Standard Map FTLE, and three field colourings (Value, Diverging, Hillshade) | `escape/fields.rs` | none |

A twin is held to its built-in as every other kind's are: its parameters
by name (a config's values carry over), its picture, its speed. Each
gets an **Edit as Rhai** where the built-in is chosen.

## How a Rhai solid already lines up with a built-in

Read 2026-11-10:

- **Parameters.** A Rhai solid's block is the built-ins' seven template
  parameters (Iterations, Beam, March Steps, Shadows, Shadow Sharpness,
  Occlusion, Slice Value), then its own, then Colour Depth and Ball
  Radius (`compile_solid`). A twin declaring the built-in's own
  parameters by name keeps every config value.
- **Colouring.** `lang::wgsl::solid` generates the built-ins' colouring
  scaffold: the trap as the closest approach of `z` within Colour Depth
  in units of the ball, the level as `log2` of a state `dr` at that
  depth, the address from a state `address`, the escape depth. A twin
  whose state keeps `dr` and accumulates `address` as its built-in does
  gives the same values.
- **The ball.** Built-ins compute theirs in Rust, in f64
  (`mandelbox_radius`, `mandelbulb_radius`, the KIFS's); a twin's
  `prepare` gives `ball`, which `solid_ball` runs in f64 too.
- **What is missing:**
  - The template's parameters cannot be read from Rhai. The quaternion
    Julia reads Slice Value (`w_slice`, slot 6); Halayka's example
    declares a `w` of its own for want of it. A `ctx` read of the
    template slots fixes it.
  - A Rhai solid's default colouring is always Orbit Trap; the KIFS's and
    the quaternion Julia's are Level. An `INFO` key fixes it.
  - Edit as Rhai: the Escape panel hides Edit as Formula for every mode D
    entry. The solid twins go through the same swap
    (`edit_as_formula`), as a solid's source.

## Steps

1. **The Mandelbox twin and the gate.** `escape::twins::SOLID_TWINS`,
   `assets/formulas/twins/solids/mandelbox.rhai`: its parameters by name
   (scale with its `|s| >= 1.05` hold, the radii, the folding limit, Mode
   and the Julia constant), the address accumulated as the built-in's
   (base 12: axes reflected x 3 + the sphere fold's case), `prepare`'s
   ball as `mandelbox_radius`. The gate renders both through
   `renderer::render` at views and settings that exercise each path
   (Mandelbox and Julia, a negative scale, each colouring, a zoom), and
   compares the pictures -- byte for byte where the arithmetic is the
   same, else by a stated tolerance -- and times both.
2. **The Mandelbulb twin**: power by spherical coordinates, Mode and the
   Julia constant, `step_factor`.
3. **The Quaternion Julia twin**: the template read (`ctx` for Slice
   Value), the slice axis, power, bailout; `INFO` default colouring.
4. **The KIFS twin**: the six folds, two rotations (`prepare`'s
   matrices), the scale and offset, Address Symmetry, the Menger's
   correction to its ball.
5. **Edit as Rhai for the solids**, in the Escape panel, with the
   panel test the formula and colouring swaps have.
6. **The fields** -- the approach is decision 1 below; then the three
   fields and three field colourings, Weierstrass's analytic gradient
   (for Hillshade) the part to check, and Edit as Rhai.

## What was built

| Step | Done | Measured (`*_twins_render_as_their_builtins`, 192x48) |
|---|---|---|
| 1. Mandelbox | template parameters a solid declares (`custom::solid_layout`) | at most ~10 of 36,864 bytes differ; 1.04-1.09x |
| 2. Mandelbulb | the state's `trap`, `level`, `color` read by the scaffold | as above |
| 3. Quaternion Julia | Slice Value declared; `INFO.coloring` | as above |
| 4. KIFS | `branch` as the walk ends; `trap_point` | one rotated pixel's branch digit (f64 rotations) |
| 5. Edit Solid as Rhai | the panel button and its test | |
| 6. Fields (decision 1: B) | `EntryKind::Field` / `FieldColoring`, `reduce`, `library::Kind::Field` / `FieldColoring`, six twins, Edit as Rhai for both | byte for byte but 19 bytes off by one and two chaotic pixels; median 1.01x, worst 1.09x |

The solid twins compare within 0.1% of bytes because the driver fuses
multiply-adds where it chooses. Neither the field twins nor the solid
twins carry presets (Edit as Rhai keeps the view), and a field has no
`prepare`.

## Decisions for the user

1. **How the fields become Rhai.** The KIFS plan's decision D
   (2026-10-17) kept the fields in mode B and said that if they moved,
   it would be "into the formula language as twins (a never-escaping
   program and an averaging colouring)":
   - **A. Mode A twins**, as written: a formula that never escapes and a
     colouring that averages. But a field's colourings read its running
     sum and analytic gradient (`field_color(sum, grad, n_terms)`), which
     a mode A colouring has no way to see; each field colouring would
     need the formula to carry them in its state and every colouring to
     know where. The pictures would come from a different template.
   - **B. A Rhai field kind in mode B**, as Rhai solids are a kind
     compiled into mode D's template: `fn init(pos, p)` and `fn step(n,
     pos, s, p)` returning the value and gradient, compiled into
     `FIELD_TEMPLATE`, and a Rhai field colouring `fn color(sum, grad,
     n, p)`. The same template, so the twins can be held exactly, and
     the fields stay in mode B as decided.

   Recommended: B. **Decided 2026-11-10: B** (the user's pick), built as
   `fn start(pos, p[, ctx])` and `fn term(s, p[, ctx])` (the state with
   `value` and an optional `grad`) and `fn shade(sum, grad, n[, p])`.
2. Whether the solid twins then replace their built-ins (D7, kind by
   kind): not part of this branch.
