# Experiments: wider coverage for Focused Rendering and the Flame Attractor

Ideas from the review of branch `rhai-planners` (2026-11-09). None is
started, none is measured, and each says what would have to be found
out first. The work that is planned from the same review is
[rhai-one-form.md](../archive/projects/rhai-one-form.md).

## Where the two engines stand

They share the map model (`scene::ifs_analysis`) and the inverses
(`variations::inverse`, a Rhai variation's `fn inverse`). Each has its
own conditions:

| | Needs | Refuses |
|---|---|---|
| The Flame Attractor ([ESCAPE.md](../main/ESCAPE.md), mode D) | an inverse for every map, every map contractive, a bounding ball | a root variation summed with another, a final that is not affine, more than one final |
| Focused Rendering, forward ([FOCUSED-RENDERING.md](../main/FOCUSED-RENDERING.md)) | a forward bound for every variation, an invariant disc every map shrinks | expanding maps |
| Focused Rendering, inverse walk | maps it can invert | linked transforms, finals other than affine and one `bipolar` |

Both are 2D, but for the Attractor's solid.

## 1. A distance field from Focused Rendering's samples

The Flame Attractor's colourings and terrains need a distance to the
attractor, which it gets by inverse iteration, so only for maps with an
inverse. A flame render is already a picture of the attractor: threshold
the density histogram and run a jump flood over it, and every pixel has
its distance to the nearest lit pixel. Focused Rendering keeps that
histogram filled at depth.

- **Would give:** the distance colourings and a terrain for any flame
  the chaos game draws, with no inverse and no contraction asked.
- **Limits:** the distance is to the samples plotted so far, at pixel
  resolution, and it is not a lower bound on the true distance, so it
  cannot drive the solid's sphere tracing. A threshold has to be chosen,
  and the field moves as samples arrive.
- **Find out first:** how far it is from mode D's distance on flames
  both can draw, by sample count; whether the colourings that read the
  walk (Escape Level, Address, Measure) have any counterpart.

## 2. Inverses by Newton's method

The inverse walk refuses a variation with no inverse. It already keeps
an indexed sample of the attractor, and a forward Jacobian for any map.
Newton's method on the forward map, seeded from the sample points near
where a preimage should be, inverts any smooth map locally: sums of
variations, and Rhai variations with no `fn inverse`.

- **Would give:** the inverse walk, and perhaps the Attractor's CPU
  reference, for variations nobody wrote an inverse for.
- **Limits:** Newton finds the preimage near its seed. A branch with no
  seed near it is missed, and a missed branch is a dark hole in the
  picture, not a refusal. The walk's soundness would rest on the
  sample's coverage. On the GPU a per-pixel walk has no seeds.
- **Find out first:** on the twins that have a declared inverse, how
  many of the declared preimages Newton recovers from the sample, by
  sample size and depth.

## 3. Plans from the Attractor's beam walk

For a flame both engines take, mode D's walk already finds, for a pixel,
the branch addresses that reach it. A low-resolution pass that returns
those addresses for each tile is a list of words: a plan. It runs the
inverse as compiled WGSL, a Rhai variation's included, so no
interpreter is in it.

- **Would give:** a second planner for qualifying flames, on the GPU at
  every depth the Attractor's deep zoom reaches.
- **Limits:** only flames the Attractor takes (contractive, invertible),
  which the forward planner mostly plans already. A beam follows a
  fixed number of addresses a pixel, so a tile many words reach is cut
  short, and the plan's draw probabilities need every word.
- **Find out first:** on the corpus, the words a beam pass finds against
  the CPU walk's, and what share of the measure the missing ones carry.

## 4. Focused Rendering in 3D: flames where z is a passenger

The tracker's E2 ([deep-zoom-tracker.md](../archive/deep-zoom/deep-zoom-tracker.md))
asks how a plan can be made in screen space when a projected map is not
an IFS in the plane. It need not be: plan in the flame's own space and
test against the camera's frustum. A camera move replans, as a pan does.

Many 3D flames have 2D dynamics: the transforms and variations move x
and y without reading z, and z is written by a variation (`zcone`,
`hemisphere`, ...) from x and y. For those the plan is today's plan in
xy with a z interval carried per word, as a word's colour is carried,
and the view test is "a disc times a z interval meets the frustum".

- **Would give:** Focused Rendering for `.flame` imports as they load
  (in 3D), without switching them to 2D.
- **Limits:** only flames where no active variation reads z. With
  perspective the frustum narrows toward the eye and the test must stay
  conservative. Depth of field and the solid shade pass take forced
  samples as they take any other; that is assumed, not checked.
- **Find out first:** a census of which variations' 3D bodies read z
  (nothing records it; `scripts/audit_z_write_semantics.py` classifies
  writes only), and from it how many of the corpus's 3D flames are
  passengers.

## 5. Focused Rendering in 3D: true 3D

Flames whose maps do read z. The Attractor's solid has the model:
`Map3`, `Kernel3`, a Rhai variation's `fn inverse3`. A plan needs
bounds as balls in space (`derive` over the 3D bodies), a 3 by 3
composed arm in the shader, and wider table rows against
`TABLE_FLOATS`.

- **Limits:** coverage looks thin. An Apophysis-style transform under
  `preserve_z` scales z by exactly one, so it is not contractive, and
  the solid's own survey found none of 34 candidate flames qualifying.
- **Find out first:** experiment 4's census, which also says how many
  flames are left for this one.

## Order, if they are taken up

1 is the cheapest and reaches the most variations. 4 follows its
census. 2 is research. 3 and 5 wait for a flame that needs them.
