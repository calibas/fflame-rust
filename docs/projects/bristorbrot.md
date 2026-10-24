# The Bristorbrot and the Quaternionbrot

Built 2026-10-23 (branch `bristorbrot`). The user's words: "Next up, I
want to add the Bristorbrot and the Quaternionbrot. Implementation should
be similar to the quaternion work we just did. I'd like Rhai versions for
both." Done as Rhai solids with presets and tests, as Halayka's sets were
([quaternions.md](quaternions.md)); no built-in WGSL formula.

## Sources

- **Doug Bristor's project**: [bristorbrot.org](https://bristorbrot.org),
  [github.com/dougbristor/bristorbrot](https://github.com/dougbristor/bristorbrot)
  (code MIT, data CC0; attribution requested: "Bristorbrot / Doug Bristor,
  bristorbrot.org"). His WebGL2 viewer sets the Bristorian `z^2 + c` and
  cubes beside the quaternion maps `q^2 + c` and `q^3 + c` -- the
  Quaternionbrot, here. Bristor found the formula in 1995; Jos Leys named
  it in 2009 (bristorbrot.org).
- **The algebra** (his README): real part and ranked imaginary units
  e1, e2, e3 (i, j, k), with e_a e_a = -1 and e_a e_b = sgn(b - a) e_b
  for a != b, extended bilinearly. Not commutative, associative or
  alternative, so the cubes `(zz)z` and `z(zz)` differ. Its square in
  (real, i, j) is Mandelbulber's "Bristorbrot" (`bristorbrot.cl`, citing
  fractalforums.com/theory/bristorbrot-3d), and with k the 4D form of
  Mandelbulber issue #127 -- both checked against the rule.
- **His viewer** (`packs/brot_viewer/v3`): 48 iterations, escape at
  |z|^2 > 64; Julia constants (-0.7, -0.35, 0.05, 0) for the square,
  (0.15, 0.4, 0.05, 0) for the cubes, (-0.2, 0.65, 0, 0) and
  (0.35, 0.62, 0, 0) for q^2 and q^3; the 3D window (real, i,
  cos b j + sin b k), the hidden axis carrying an offset, b = 27 degrees
  in his README's picture; the scalar distance 0.5 |z| ln|z| / dr with
  dr = n |z|^(n-1) dr (+ 1 for the Mandelbrot set), a hit on it, a step
  by the directional derivative leashed to it.

## What was built

- `assets/formulas/solid-bristorbrot.rhai`: Formula (`z^2 + c`,
  `(zz)z + c`, `z(zz) + c`), Variant (Mandelbrot, Julia), Julia C, Section
  Roll (degrees) and Offset, Distance (Bristor's, Jacobian). The product
  written part by part over a `quat` (parts (real, i, j, k) as (w, x, y,
  z); `*` would be Hamilton's).
- `assets/formulas/solid-quaternionbrot.rhai`: Power (2-8; `pow` past 2),
  Variant, Julia C, Section Roll and Offset; the classic estimate.
- Both escape at |z| > 1e4 (a better distance than Bristor's 8; the set
  differs only for points that escape in the last iterations).
- Eight presets (`escape::bristor` writes them from the files and keeps
  them to them), on the Quaternion Julia preset's 3D look, coloured by
  Escape Level: Bristorbrot (24 iterations, side view), Bristorbrot Julia
  (his picture's C and roll, 48), its two cubes, Quaternionbrot,
  Quaternionbrot Julia, Quaternionbrot Cubed, Quaternionbrot Cubed Julia.

## The distance (measured)

- **The Bristorian product is not conformal.** Its square stretches space
  by up to 3.3 |z| (sup over |z| = 1 of the Jacobian's spectral norm),
  the cubes by 6.7 and 7.2 |z|^2 -- not 2 and 3. Hamilton's measure 2 and
  3 exactly, so the Quaternionbrot's running derivative bounds its orbit.
- **Marched by Bristor's scalar reading, uncapped**, rays passed through
  his Julia sets: 1 of 13 for the square, 13 of 33 for the cubes
  (prototype, f64). The Mandelbrot sets did not.
- Compared on the same rays: the stretch-constant bound (safe, ten times
  the steps), the Jacobian's Frobenius norm (safe, ~1.2x the steps), the
  potential gradient `|J^T z| / |z|` (sharpest; one ray stepped through in
  the Rust test).
- **The Julia sets are dust**: drawn exactly (the inside march, a hit only
  where the point stays), the square's Julia at Bristor's C is scattered
  specks finer than a pixel. So what any of these draws is a level of its
  estimate at the pixel, not the set: the Jacobian's is the dust grown by
  a pixel (fuller, its gaps closed); Bristor's reads long -- by the
  stretches' ratio compounded over the orbit -- so it shows the dust's
  core as thin ribbons, his pictures' look.
- **A Julia orbit through 0** (the critical point) has no derivative and
  reads infinitely far; the reading is capped at a tenth of the ball.
  Capped, neither estimate passed through the sets (600 rays a set for
  Bristor's), so Bristor's is the default and the Jacobian the option;
  the Jacobian's three tangents are carried only when it is chosen.

## Tests (`src/escape/bristor.rs`)

- `bristors_rule_gives_his_witnesses`: i j = j, j i = -i, not associative,
  (-1, 1, 2)^2 = (-4, -4, -2) (Hamilton's (-4, -2, -4)), the cubes at
  i + j, the 3D square's parts.
- `the_solids_run_as_their_algebras`: at 512 points of each set (every
  formula and power, Mandelbrot and Julia, the 3D set and a rolled,
  offset section, both Distances) the file's reading is the one written
  here from the rule (and Hamilton's product) in f64.
- `neither_is_stepped_through`: rays marched as mode D's near march
  (`ifs_refine` included) stop no later than the first point sampling
  finds. Weakened (the Jacobian's reading tripled) it fails at once.
- `the_presets_carry_the_examples`.

## Not built

- A built-in WGSL formula for either (the Rhai solids are the
  implementation).
- Bristor's split of the estimate -- a hit on one reading, a step by
  another, the directional derivative leashed to the scalar -- which mode
  D's single distance cannot express.
- Escape paths (Halayka's fibres) for these sets.
