# Deep zoom past f32: the replay in offsets (plan, 2026-09-23)

Tracker item C1 ([deep-zoom-tracker.md](deep-zoom-tracker.md)). Stage 3
of [flame-deep-zoom.md](flame-deep-zoom.md), for the replay arm that
cylinder targeting actually uses.

## 1. The problem

A forced sample is computed in f32 world coordinates, and so is the view
centre (`GpuParams::pan_x/pan_y`). The pixel is `(p − pan)·zoom`. Once
f32's spacing near the view is a sizeable fraction of a pixel, points
snap to a grid and pixel columns get unequal shares of it: stripes. At
the saved grand-julian view (x = 0.268, 1280x720), the spacing in x is
0.18 px at 32k zoom, 0.35 px at 64k (the 2:3 stripes seen in the app),
and 5.4 px at 1e6.

The GPU planner has the same limit. Its replays and landing checks are
f32 too, so past about 1e6 its checks against the view disc stop being
reliable (at 1e6 they already disagree with the CPU at the rim,
[gpu-cylinder-planning.md](gpu-cylinder-planning.md) §13). The CPU
planner is f64, good to about 1e12.

## 2. The approach: a reference orbit, and offsets

As escape-time perturbation does, but forward and per word.

- **A reference orbit per word.** Take a sample point the walk found
  landing in the view under the word (the walk has them), and run it
  through the word on the CPU in f64: `z_0 … z_n`. It passes through the
  region that matters at every step, because it lands in the view.
- **The sample carries an offset.** Up to a switch step `m`, the replay
  runs as today, in absolute f32. There the sample is still at a scale
  f32 resolves. At `m` it becomes `δ_m = x_m − z_m`. From then on each
  step is `δ_{k} = D_k(z_{k−1}, δ_{k−1})`, where
  `D(z, δ) = F(z + δ) − F(z)` is computed **without forming the
  difference** (§3).
- **The plot needs no big number on the GPU.** The pixel is
  `((z_n − c) + δ_n)·zoom`: `z_n − c` is formed on the CPU in f64 and
  sent as a small f32, and `δ_n` is small. The final transform, affine
  (the only kind the analysis accepts), is folded in the same way: its
  translation on the CPU, its linear part applied to `δ`.
- **Choosing `m`.** Late enough that the offset form pays off, early
  enough that the absolute f32 error at `m` stays sub-pixel after the
  remaining maps shrink it. That error is about `6e-8·|x_m|`, and the
  remaining maps scale it by the same factor as the region. So `m` is
  the last step whose region is at least `1e-3·|z_m|`, which puts it
  under 0.1 px. The region's size at step k is the view's preimage
  through the steps after k, from the reference's Jacobians (in f64).
  A word that never gets below that (shallow zoom) has no offset steps
  and replays exactly as today.
  **Superseded, §8:** a GPU's absolute step is off by far more than
  f32's rounding, and an error is carried by the rest of the word's
  largest stretch, not its region's. `m` is chosen from both.

### Words with more than one region

A word can be reached from several separate regions. Bubble and disc are
many-to-one going forward, and the walk merges the preimage branches of
one symbol into one word. A single reference tracks only the samples near
it. A sample from another branch has an offset as large as the distance
between branches, and once the branches merge, its offset is known only
to f32 of that distance, which at depth is garbage.

- **So a word carries up to `R` references:** landed sample points whose
  positions at step `m` are far apart (farthest-point choice among the
  landed points). At `m`, a sample takes the nearest.
- **A sample far from all of them** (offset over a quarter of its
  reference's region) finishes in absolute f32, as today. It is almost
  surely not landing.
- **Measure before choosing R:** how often a kept word's landed points
  form more than one cluster at `m`, on the corpus. `R = 1` is the
  first build.

**Measured, §8:** on the corpus flames every word's region is ONE piece
at `m`, disc included, so one reference is enough. The chains support
up to four regardless.

## 3. The forward difference forms

`F = post ∘ (w·K or lin + kw·K) ∘ pre`, per the analysis's map
(`Map2::Affine`, `Nonlinear`, `Sum`). Affines are linear in `δ`. The
kernels, with `v` the reference in the kernel's frame and `ε` the
offset there:

| kernel | forward | difference form |
|---|---|---|
| Root {n, d}, arm k | `\|v\|^{d/\|n\|}·e^{i(θ+2πk)/n}` | `K(v)·expm1(L)`, with `L = (log\|u\|·d/\|n\|, (arg u + adj)/n)` and `u = 1 + ε/v`, where `log\|u\| = ½·log1p(2Re(ε/v) + \|ε/v\|²)` and `adj ∈ {0, ±2π}` when `v + ε` is across `atan2`'s cut from `v` |
| Spherical | `v/\|v\|²` = `1/conj(v)` | `−conj(ε) / conj((v+ε)·v)` |
| Bubble | `4v/(\|v\|²+4)` | `4(ε(\|v\|²+4) − v(2v·ε + \|ε\|²)) / ((\|v+ε\|²+4)(\|v\|²+4))` |
| Hemisphere | `v/√(\|v\|²+1)` | `ε/s' − v(2v·ε + \|ε\|²)/(s s'(s+s'))`, with `s, s'` the roots at `v` and `v+ε` |
| Disc | `(θ/π)(sin πr, cos πr)`, θ from +y | `(Δθ/π)(sin πr', cos πr') + (θ/π)(Δsin, Δcos)`; `Δθ` from cross and dot (plus the cut's 2π), `Δr = (2v·ε+\|ε\|²)/(r+r')`, `Δsin = 2cos(π(r+Δr/2))·sin(πΔr/2)` |
| Blob | `s(θ)·R(v)` | `s(θ')R(ε) + (s(θ') − s(θ))R(v)`, where `s(θ') − s(θ) = (high−low)·cos(w(θ+θ')/2)·sin(wΔθ/2)` |

Every term is O(ε): nothing subtracts two positions. `log1p` and `expm1`
use series below 0.01 and the plain functions above, because the usual
`(1 + a) − 1` trick is exactly what Metal's fast-math may fold. They are
written once in Rust, generic over `Real` beside the inverse forms (so
f64 and `BigFloat` share one body), and once in WGSL, generic over a
per-map row the CPU fills, in place of the variation code.

## 4. Where it runs

- **CPU, at plan time (the plan job, sliced on the web):** per kept word,
  its reference(s), the f64 orbit, `m`, and the per-step reference
  points from `m` on. The work is words × steps map evaluations, a few
  milliseconds.
- **GPU, the render (`CYLINDER_REPLAY` plus a new template flag, so
  shader dumps stay byte-identical when off):**
  - A forced sample replays in absolute f32 to `m`, picks its reference,
    and carries `δ` through the difference forms.
  - The final transform is folded as in §2, and the final chain is
    skipped.
  - The plot takes the relative position: `world_to_pixel` without the
    pan subtraction.
- **Per-word layout:** symbols, `m`, the reference count, then per
  reference `z_m`, the f32 base points for the steps after `m`, and
  `z_n − c`. Per flame: one row per forward map (pre and post linear
  parts, the sum's linear part, the kernel and its parameters, the
  weight), and the final's linear part.
- **The planner:** past the zoom where f32 landing checks stop being
  reliable (view radius under 100 f32 spacings at the view centre), plan
  on the CPU. The GPU planner in offsets is a later item.

## 5. Gates

1. **The forms are exact.** For each kernel, including roots with
   negative `d` and every arm, the f64 difference form against a direct
   `F(v+ε) − F(v)` in `BigFloat`, over `|ε|/|v|` from 1e-12 to 1e-1:
   relative error at f64 rounding.
2. **The shader's forms are the CPU's.** WGSL against f64 over the same
   grid: relative error at f32 rounding.
3. **Nothing changes where it was fine.** At 1e3 the offset replay and
   today's replay draw the same picture: the existing picture gates,
   with the offset path forced on.
4. **It holds at depth.** At 1e7 and 1e9, the GPU render against a CPU
   f64 render of the same plan (forced samples through the f64 map,
   binned the same way), per-pixel density agreeing within noise; plus
   a stripe measure (the column sums' spectrum). Today's replay fails it
   at 64k.
5. **Shader dumps byte-identical with the flag off**, and the existing
   suites green.

## 6. Order of work

1. The forward forms in Rust, and gate 1.
2. The references: the walk keeps landed points per kept word; the
   orbit and `m`; measure the cluster count.
3. The WGSL forms and rows, and gate 2.
4. The render: layout, flag, plot; gates 3 and 4.
5. The planner's CPU fallback past the f32 limit.

## 7. Not here

- The GPU planner in offsets.
- Zoom past f64's pan (about 1e12-1e13), where the pan itself needs
  exact decimals as escape-time already has.
- 3D.

## 8. Built and measured (2026-09-23)

### The forms

- **Gate 1** (`the_forward_forms_are_exact`): every forward kernel, with
  roots of both signs of distance and several powers and arms, is exact
  against 512-bit `BigFloat` from ε = 1e-30 to 1e-1 of the point. That's
  64,410 differences, 410 of them across a cut, worst 1.2e-14 relative.
  The direct f64 subtraction has no correct digit at 3e-18
  (`the_direct_forward_subtraction_loses_everything`).
  - One fix found on the way: where a map is continuous across its cut
    (a whole number of blob waves, a root of power one), the cut's 2π
    has to be reduced exactly rather than added to a small angle, or the
    angle's digits round away (`sin_cos_shifted`).
- **Gate 2** (`the_shader_forward_forms_are_the_cpu_ones`): the WGSL
  forms against f64 on the same f32 inputs are worst 2.7e-6 relative
  (roots 1.5e-6, spherical, bubble and hemisphere under 6e-7).
  - It found that **a GPU's `sin` is accurate to an absolute error, not
    a relative one**: `sin(1e-12)` came back a thousandth of itself on
    NVIDIA. Every small angle the forms take the sine of goes through a
    series (`fd_sin`).
  - For the same reason `log1p` and `expm1` switch to their series at
    0.1 in the shader (0.01 in f64): the hardware `log` and `exp` are
    good to an ulp of their result.

### The references

- One reference orbit per word, from a point the walk found landing.
  `what_the_references_hold` checks that the offset carry reproduces the
  absolute replay in f64. For the seeds the worst was 5.5e-5 px. For 3
  million random attractor points through grand-julian's and
  julian-disc's words at 1e4, where most points are far from the
  reference and the forms work at large offsets, none were more than
  0.01 px apart (worst 7e-10 px).
- `Backward::pieces` walks each word's end back through every inverse
  branch of its offset steps. Every word of grand-julian, random1 and
  julian-disc at 1e4, 1e6 and 1e8 has one piece.
- The references cost 3-15 ms per plan for grand-julian and random1
  (3-4k words) and 160-680 ms for julian-disc (40-66k words), on twelve
  threads.

### The switch rule, corrected

`how_far_the_plain_replay_is_from_f64` runs the render's own replay
(`ct_apply_symbol`, through the planner kernel's endpoint mode) against
f64 from the same f32 starts:

| flame, zoom | plain replay: mean error | mean error vector (bias) |
|---|---|---|
| grand-julian 1e4 | 0.29 px | (−0.11, +0.25) px |
| grand-julian 1e5 | 3.1 px | (−1.15, +2.63) px |

That is about 4e-7 world units at every depth, on grand-julian and
julian-disc alike: the GPU's transcendental functions, accurate to an
absolute error near 1e-7 over a turn. It is a systematic displacement,
not noise, and about seven times f32's own spacing near the saved
grand-julian view. So before this work the replay was also displacing
the picture, by a few pixels at 64k, on top of the stripes.

The switch rule of §2 assumed f32's rounding (6e-8) for an absolute
step, and a region's size for how the rest of the word carries it. Both
were wrong for julian-disc, where the offset replay first measured 33 px
off at 1e6 and 148 px at 1e8:

- An absolute step costs `GPU_STEP_ERROR` = 4e-6 of the point's distance
  from the origin (the measured 4e-7 units, with margin).
- The rest of the word carries it by its Jacobian's LARGEST singular
  value, along the reference. Disc stretches radius and angle by very
  different factors, and the error in the stretched direction is what
  reaches the picture.
- So `m` is the last step whose error, carried to the end, stays within
  a twentieth of a pixel at 4K (`PLOT_TOLERANCE`). A plan needs offsets
  once the plain replay's own last step misses that: views under a fifth
  of the scale, which is nearly every deep zoom.

### Gate 4, per sample

`the_offset_replay_holds_per_sample`: the render's own code, both the
plain replay and `ct_offsets` (`PlanGpu::offset_endpoints`), against f64
from the same starts, for about 20,000 samples per flame and zoom that
land in view:

| flame | zoom | plain: mean error | offsets: 99th percentile | offsets: bias |
|---|---|---|---|---|
| grand-julian | 1e4 | 0.29 px | 0.008 px | 0.0001 px |
| grand-julian | 1e6 | 30 px | 0.002 px | 0.0004 px |
| grand-julian | 1e8 | 2,745 px | 0.005 px | 0.0003 px |
| random1 | 1e4 | 0.20 px | 0.002 px | 0.0001 px |
| random1 | 1e6 | 4.2 px | 0.002 px | 0.0001 px |
| random1 | 1e8 | 408 px | 0.004 px | 0.0002 px |
| julian-disc | 1e4 | 0.71 px | 0.018 px | 0.0002 px |
| julian-disc | 1e6 | 72 px | 0.006 px | 0.0001 px |
| julian-disc | 1e8 | 10,127 px | 0.015 px | 0.0009 px |

One sample in 20,078 (grand-julian 1e6) is 1,565 px off in both replays.
It is a point within an ulp of a root's cut, which the GPU sends round
the other side from f64; the free chaos game does the same. Gated: the
99th percentile under 0.05 px, the bias of the rest under 0.01 px, and
at most one sample in a thousand off by a pixel.

### The picture

`a_deep_replay_has_no_stripes_and_no_lattice`, grand-julian at 320x320
with 48M samples, each rung with offsets and without:

| zoom | offsets: lit, joined, stripes | plain: lit, joined, stripes |
|---|---|---|
| 65,536 | 44,343, 0.976, 0.080 | 43,594, 0.974, 0.113 |
| 1e6 | 51,011, 0.986, 0.071 | 1,208, 0.000, 13.3 |
| 1e8 | 57,802, 0.987, 0.070 | nothing lit |
| 1e10 | 48,667, 0.989, 0.071 | nothing lit |

"Stripes" is the spread of each column's and row's brightness about its
neighbours'. It is the picture's own structure at about 0.07, the same at
every depth with offsets. The rungs are saved in `output/deep-offsets/`.

Gate 3 compares, at 1e3, where the plain replay is off by about 0.03 px:
the offset render against the plain one is within the noise between two
plain samplings, per pixel and over 4x4 blocks. At 1e4 it is not, and
that is the plain replay's 0.29 px displacement, which the per-sample
gate measured directly.

### The planner

Past `GPU_PLAN_RADIUS` (a view radius under 2e-6, scaled by the view
centre's distance from the origin where that is over one), plans are
made on the CPU in f64. The GPU planner's landing checks are as displaced
as the plain replay, about 4e-7 units. It was measured complete at 1e6 on
grand-julian (radius 4.1e-6), where that error is a tenth of the view.
GPU plans at 1e6 are unchanged, bit for bit.

(The walk refuses a final transform, so §2's folding of one is not
needed; relative plotting requires no attachments anyway.)

### Still open

- **The GPU planner in offsets**, so deep plans get its speed back.
- **Zoom past f64's pan**, about 1e12-1e13.
- **The references' cost on julian-disc** (hundreds of ms), with its
  plans' size (tracker P3).
- **The references' cost on the web.** They run on the page's thread, a
  word a tick: the browser gate's saved view plans in 1.7 s warm against
  1.2 s before, at 60 Hz, with no step over 11 ms. Only the first seed
  that lands is run through a word, and the others only where a bubble
  or disc among the offset steps could make another piece. On twelve
  threads it is 3-7 ms for grand-julian and random1.
