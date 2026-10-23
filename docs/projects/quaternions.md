# Quaternions in Rhai, Halayka's sets, escape paths

Decided 2026-10-22; built the same day (branch `quaternions`). The
user's words: "I found other 3d quaternion
fractals ... I want to see about implementing the above fractals, and I'd
like to make quaternion fractals compatible with our Rhai scripts", and,
of the second paper, "a way of coloring these fractals that I'm not sure
we offer yet". Then: "Add the quat type. Let's try fibres in the flame
engine first. And yes, quat for Rhai 3d variations."

The papers (in `output/pdf/`, not in the repository):
- S. Halayka, *Some visually interesting non-standard quaternion fractal
  sets*, Chaos, Solitons and Fractals 41 (2009) 2842-2846: the quaternion
  Julia set with its step replaced -- Z^4 + C, sin Z + C sin Z,
  cos Z + C cos Z, exp(Z^2) + C (its complement drawn),
  inv(sinh Z) + C inv(sinh Z), C (inv(sinh Z) cosh Z) (its complement
  drawn); the point (x, y, z) lifted to Z = (x, y, z, 0) scalar first,
  C = (0.3, 0.5, 0.4, 0.2), 8 iterations, threshold 4, the box -1.5..1.5.
- S. Halayka, *Visualizing the escape paths of quaternion fractals*
  (2018; source at github.com/sjhalayka/bezier_fractal): each lattice
  point whose orbit stays under the threshold for its 8 iterations keeps
  the orbit Z0..Z8 as the control points of a Bezier curve, drawn from
  t = 0 to 0.2 as cylinders, each segment's colour its place along the
  path (`HSBtoRGB(300 t, 75, 100)`).

## Decisions

1. **A `quat` type in the Rhai dialect**, in solids and in 3D variations
   (`variation3`) alike.
2. **The escape paths as fibres in the flame engine** first: a Rhai 3D
   variation draws points along the paths, the 3D flame pipeline splats
   them. Real tubes (a renderer of their own) only if the fibres fall
   short.

## The `quat` type (`src/escape/lang/rhai/vec.rs`)

The vector machinery generalises from three parts to a kind and its parts
(`vec3`: x, y, z; `quat`: w, x, y, z, w the scalar part), so a quaternion
local is four float locals, a quaternion field four fields (`q__w` ...),
a `PARAMS` entry `quat(w, x, y, z)` four float parameters, and the IR,
the interpreter, the WGSL emitter and the derivation passes still see
scalar code.

- `quat(w, x, y, z)`, `quat(w, v)` with `v` a `vec3`; parts `.w`, `.x`,
  `.y`, `.z` read and set; `.v` the vector part, a `vec3`.
- `+`, `-` between quaternions; a number added to or taken from one is
  added to its scalar part (`1.0 + c`); `*` the Hamilton product between
  quaternions, or a scaling by a number; `/` by a number; unary `-`.
  `q / r` is refused: `q * inv(r)` and `inv(r) * q` differ.
- `dot`, `length`, `normalize`, `conj`, `inv` (`conj(q) / |q|^2`).
- `sin`, `cos`, `tan`, `sinh`, `cosh`, `tanh`, `exp`, `ln` (`log` base 10, as Rhai's), `sqrt`, and
  `pow(q, n)` for a number `n`: each the complex function `g` of
  `t + i r` (t the scalar part, r the vector part's length), carried back
  along the vector part: `g = a + i b` gives `a + b v / r`. Exact for
  every function with real coefficients, and what Halayka's equations
  (12)-(21) are.
- `q.complex()`: `complex(t, r)`, the plane the functions act in. A
  distance estimate's running derivative reads it: the functions stretch
  by at most `max(|g'(t + i r)|, |b| / r)` (the plane, and across it), so
  a script's derivative can be exact.
- A solid's state `z` may be a quaternion; the walk's trap reads its
  vector part.

## Halayka's sets as a Rhai solid (`assets/formulas/solid-halayka-quaternions.rhai`)

One solid, its Step a choice of the six, its exact running derivative
(each step stretches by `|1 + C|` or `|C|` times `max(|g'|, |Im g| / r)`).
As built:

- **Outside**, `(|Z_k| - 4) / dr_k` at the step `k` that escaped, halved
  and capped at a tenth of the box: first-order, it holds near the
  surface; past a critical point (exp(Z^2)'s origin, where `dr` is 0) the
  cap keeps it a number. (Neither reading is a clearance: *After use*,
  below.)
- **Inside** there is no bound -- an orbit that settles shrinks `dr`, and
  `(4 - |Z_k|) / dr_k` read units where the surface was a tenth away
  (found by the ray test below) -- so the set is marched through in
  steps of a fortieth of the box, the reading taking over within one.
- **Draw**: the set, the complement, or the surface between them. The
  paper's figures are marching-cubes isosurfaces of a lattice, which have
  no faces where the set meets the lattice's edge: the complement drawn
  as a solid shows the box's walls, so figures 6 and 8 are drawn as the
  surface -- a shell a step thick on the escaped side, so a step from
  inside the set stops in it.
- **A point with no number** (inv(sinh Z) at Z = 0) is outside, as the
  paper's lattice has it (`!(m <= threshold)`).
- **What it is**: at 8 iterations these are the shapes of a few
  iterations, smooth -- Z^4 + C is a ball, Z^2 + C a ball with a hollow
  core and a thin shell; the paper's coarse lattice smooths what we draw
  sharp. Membership is checked against the paper's definition at a
  lattice (`halaykas_sets_hold_what_the_paper_says`), and marching
  against sampled crossings along rays (`halaykas_sets_are_never_stepped_through`).
- Presets, figures 3 to 8 (1 and 2 are the standard set, the built-in
  Quaternion Julia's): `escape::halayka`, written from the files and kept
  to them by `the_presets_carry_the_examples`.

### After use (2026-10-22)

The user's words: "It's tricky to color it properly, none of our current
coloring methods match the structure very well ... I'm thinking we can
derive coloring using the same method that determines the escape path?
Other techinques? Something odd about the shadows sometimes, they make
strange patterns that warp as I move around ... There's also times when
the camera seems distored ... especially when turning the camera." Their
config: C inv(sinh Z) cosh Z, C = (1, 2.7, -4, -0.6), threshold 16, the
complement, a box of 3.

- **The patterns were surfaces that were not there.** Either reading is
  small wherever an orbit nearly crosses the threshold, near a surface or
  not, and the march stopped within a pixel of the solid: it stopped
  there, at a depth that moved with the pixel, so with the camera; soft
  shadows and occlusion read the same readings as clearances. Where an
  orbit grazed, the march crawled until its steps ran out (the black
  frames). And with the paper's settings the outside reading is small
  over wide regions, its bound loose: the near march drew every one of
  the six presets fattened -- figure 3's "can" was mostly those false
  hits; the set itself, sampled on the CPU and on the GPU (a slab at
  z = 0), is the two lobes the inside march draws.
  - Fix: `INFO.march = "inside"` (ESCAPE.md, *A Rhai solid*): a hit only
    in the solid, each step at least a pixel, the hit refined back to
    where the ray entered (`ifs_refine`), hard shadows, occlusion by
    membership. The distance is signed (negative in the solid) so the
    normal, taken where the march stepped in, sees both sides; a flat 0
    inside snapped normals toward the axes and left the presets dark.
  - What remains is real: at threshold 16 and |C| near 5 the complement
    has sheets far thinner than a pixel (the poles' preimages), which
    alias as any sub-pixel geometry does.
- **The distortion was the ball.** Box 3 reaches 5.2 from the centre;
  the ball was 2.6, so the march started on a sphere cut through the
  solid, the camera sat 0.17 outside it, and a turn took the eye inside
  the solid (a black frame). Fix: `prepare` gives `ball` (the cut's: a
  box's corner or the ball, a hair over), which the march and the camera
  take in place of the Ball Radius (`custom::solid_ball`). The user's
  tubes came from that sphere cut: **Cut** offers a ball, a real cut
  with its own normals and shadows.
- **Colour** (`address`, the Branch Address colouring; `INFO.depth = 12`
  so it reads the whole orbit): each piece of the surface is where
  `|Z_k|` reaches the threshold for one `k`, and from outside the orbit
  escapes there, from inside it comes nearest to escaping there, so the
  step of the orbit's largest `|Z|` and the angle `Z_k` makes with the
  real axis there colour a piece alike from either side: Nearest step,
  Exit angle, Step and angle. And the escape path's (2018): Path heading
  (where its Bezier curve leaves the point, `Z1 - Z0` about the z axis)
  and Path length (how far the orbit runs). The presets: Nearest step,
  ambient 0.3 (one light left most of the rings dark).

## Escape paths as fibres (`assets/formulas/variation-escape-paths.rhai`)

A Rhai `variation3` that ignores its input: a random lattice point of
the box, iterated 8 times (another drawn if it escapes, 24 tries), a
random `t` in [0, Length], the Bezier point of the orbit at `t` summed as
the orbit runs (Bernstein weights; no arrays), out with its colour `t /
Length`. Its points are checked against the paths in f64
(`the_escape_paths_lie_on_their_orbits_paths`). Two presets, the 2018
paper's figures 1 (Z^2 + C) and 5 (sin Z + C sin Z): a 3D flame on black
with the solid shade pass (`solid_strength` 1), which lights the fibres
as tubes; the paper's palette, `HSBtoRGB(300 t, 75, 100)`.

## Order of work

1. The `quat` type, generalising the vector lowering; tests that it runs
   as its parts written out, in a solid and in a 3D variation.
2. The Rhai Z^2 + C against the built-in quaternion Julia (GPU).
3. The six solids and their presets.
4. The escape-path variation and its preset.
5. Docs: ESCAPE.md (the language: `quat`; the examples), SCRIPTING-GUIDE
   if it covers the dialect.
