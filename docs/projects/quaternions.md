# Quaternions in Rhai, Halayka's sets, escape paths

Decided 2026-10-22. The user's words: "I found other 3d quaternion
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

## Halayka's sets as Rhai solids (`assets/formulas/`)

The six, each with its exact running derivative, at the paper's settings
by default (8 iterations, threshold 4): the distance outside is
`(|Z_k| - 4) / dr_k` at the iteration `k` that escaped (a lower bound on
the distance to that iteration's level set, which holds the set). The two
complements: inside, the least of `(4 - |Z_k|) / dr_k` over the
iterations, intersected with the box. Presets at the paper's views.
Z^2 + C in Rhai checked against the built-in quaternion Julia solid.

## Escape paths as fibres (`assets/formulas/`, a flame preset)

A Rhai `variation3` that ignores its input: a random lattice point of
the box, iterated 8 times (another drawn if it escapes, a few tries), a
random `t` in [0, 0.2], the Bezier point of the orbit at `t` summed as
the orbit runs (Bernstein weights; no arrays), out with its colour `t /
0.2`. A preset 3D flame of it with a rainbow palette from the paper's
hues; the lattice's spacing and the path's share are parameters.

## Order of work

1. The `quat` type, generalising the vector lowering; tests that it runs
   as its parts written out, in a solid and in a 3D variation.
2. The Rhai Z^2 + C against the built-in quaternion Julia (GPU).
3. The six solids and their presets.
4. The escape-path variation and its preset.
5. Docs: ESCAPE.md (the language: `quat`; the examples), SCRIPTING-GUIDE
   if it covers the dialect.
