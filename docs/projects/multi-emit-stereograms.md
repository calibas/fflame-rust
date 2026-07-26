# Multi-Emit Plotting + Stereograms

Status: **Phases 1–2 implemented** (plumbing + `stereogram`). Branch: `multi-emit`.

## Motivation

Primary driver: **stereograms as a final-transform variation** — one render
producing a fused left/right pair. Secondary beneficiaries of the same
plumbing: high-performance kaleidoscopes (N rotated copies per iteration
instead of N× iterations) and spray-style blur effects.

The stereo analysis this is built on (from design discussion):

- **Dual-plot beats a 50/50 split**, and not just for throughput: a
  stochastic split gives each eye a disjoint random subset of the
  trajectory, so residual density noise is *independent* between eyes and
  fusion degrades into shimmer exactly in the high-detail regions where the
  depth cue is strongest. Dual-plot gives both eyes the identical sample
  set differing only by projection — correlated noise, clean fusion. Same
  RNG seed comes for free since it is one trajectory.
- **Disparity must be depth-dependent**, so the fork happens on the 3D
  point in *camera space*, before projection — not as a constant image
  offset (which fuses into a flat billboard).
- **Off-axis, not toe-in**: pure translation along the camera's right
  vector, axes stay parallel. No keystone, no vertical parallax.
- **Cross vs parallel view is a sign flip** on the panel offset. An
  L-R-L triptych (3 emits) serves both viewing modes at once.
- **Convergence** is a per-eye horizontal image shift of ∓f·b/(2·Z_conv),
  a parameter, not a geometry change.
- **Baseline as a ratio to Z_conv** (~1/30 default): flame coordinate
  scales vary by orders of magnitude, an absolute baseline is unusable as
  a saved param.

## Part 1 — multi-emit plumbing (general capability)

### Authoring contract

Carried as **data on the features slice**: `Feature::PlotEmits(u8)`.

(The spec originally called for a new `VariationDef` field, but defs
initialize every struct field explicitly — a new mandatory field means
editing all ~640 definitions. A data-carrying Feature variant is the
same information with zero churn, and `VariationInfo::plot_emit_cap()`
reads it out.)

The builder derives a `HAS_PLOT_EMIT` template gate from "any active
variation has `plot_emits > 0`" — the same pattern as `HAS_W` /
`HAS_VOLUME_SIDE`. Flames without an emitting variation compile to
byte-identical WGSL (assert this in a builder test, like the `point_w`
one).

WGSL side, emitted only under the gate:

```wgsl
var<private> plot_emit_points: array<vec4<f32>, PLOT_EMIT_CAP>; // xyz + weight
var<private> plot_emit_count: u32;

fn emit_plot(p: vec3<f32>) { emit_plot_weighted(p, 1.0); }
fn emit_plot_weighted(p: vec3<f32>, w: f32) {
    if (plot_emit_count < PLOT_EMIT_CAP) {
        plot_emit_points[plot_emit_count] = vec4<f32>(p, w);
        plot_emit_count = plot_emit_count + 1u;
    }
}
```

`PLOT_EMIT_CAP` = max over active variations' `plot_emits` (bounded at,
say, 16). The count is reset each iteration in the main loop, next to the
`volume_side_flag` reset. 2D bodies use a `vec2` helper that stores z = 0.

### Semantics (the propagate-vs-discard question)

The existing return value keeps its exact meaning, unchanged:

- **Normal transform**: the returned point is the trajectory state that
  propagates. Emissions are plot-only extras — they never re-enter the
  chaos game.
- **Final transform**: the returned point is the main plotted point, as
  today (finals already shape only the plot; the iterate continues from
  the pre-final state). Emissions are additional plotted points.

So "propagate or discard" needs no option: the single returned point IS
the propagated position; the emit array IS the separate plot path. A
variation that wants *only* emitted points (stereogram: two eyes, no
center image) pairs `plot_emits` with the existing `Feature::CanHide` to
suppress the main plot.

### Where emissions enter the plot pipeline

Emitted points are collected in the private array during variation
application (any stage — normal, linked, or final) and plotted in the
plot section alongside the main point:

- They pass through the **post-symmetry loop** and all **plot-time
  effects** — DoF, fog, depth-density compensation, far-fade, opacity —
  identically to the main point (each emit is projected through the same
  `project_3d_full`).
- They do **not** pass through final transforms. Rationale: applying the
  final chain per-emit means another `apply_variations` call site, and
  drivers inline that function at every call site — compile cost for
  every flame, paid even by non-emitters (this is the measured ~3×
  heavyweight-variation compile multiplier from the finals/linked call
  sites). Emitting variations are intended to sit ON the final (that is
  the stereogram use case, and the natural kaleidoscope shape); an
  emitter on a normal transform in a flame that also has finals gets
  raw-plotted emissions, documented as such.
- They share the iteration's color (color is computed once outside the
  plot loop today; per-emit color is out of scope for v1).

### Density weights

`emit_plot_weighted(p, w)` rides the existing per-sample
`density_weight` multiplication — all four histogram channels (R, G, B,
density) scale by the same weight, and the recovery ratio Σcolor/Σdensity
is already weight-invariant, so color is unaffected. Five points at
w = 0.2 deposit exactly the density of one point at 1.0.

Why weights matter beyond aesthetics: the tonemap normalizes brightness
by `sample_density = total_iters / pixel_count`. Multi-emit deposits
more than one density unit per iteration, so an N-emitter brightens the
image relative to mono. `w = 1/N` restores mono-equivalent brightness
exactly. For stereograms, each eye at w = 1 is usually right (each panel
is its own image); the doc for the variation should say so.

**Floor caveat**: the histogram deposit is
`u32(100.0 * density_weight)` — truncation floors weights below ~0.01 to
zero. Headroom for 5–50 emits, not thousands. Clamp/document.

### The three output paths (parity requirement)

Plots are consumed in three places; all three must loop the emit array
or preview and export silently diverge:

1. **Direct histogram** (in-app + small exports) — the main loop change.
2. **Sample-emit / tiled scatter** (large exports): one buffer slot per
   plot via `atomicAdd` on `sample_counter`. `samples_per_dispatch()`
   in `export/high_res.rs` assumes ≤1 sample per iteration — must
   multiply by `(1 + PLOT_EMIT_CAP)` when an emitting variation is
   active, or the buffer overflows and silently drops samples.
3. **CPU-histogram fallback** (`high_res.rs`) — same loop on the CPU
   side.

Solid mode's depth region and the analytic-blur buffer both consume
plots too; see Known Broken below.

## Part 2 — the `stereogram` variation

A normal variation (Advanced2D category per the Only3D rule; genuinely
meaningful only in 3D render mode), `plot_emits: 3` (two eyes + spare
for triptych), `Feature::CanHide` (suppresses the main center plot).
Designed for a final transform via `final_attachments`, but legal
anywhere (emissions from a normal transform just skip later finals, as
above).

### Math — the world-space round trip

The fork must happen in camera space, but a variation runs in world
space *before* the plot-time camera transform. Both are satisfied
without any renderer plumbing by a round trip through the module's own
camera machinery (`build_camera_matrix` is already in scope in the
assembled shader; M is orthogonal so the inverse is the transpose):

```
cam  = M · (p − cam_pos)                 // world → camera (roll-less, like project_3d_full)
zr   = 1 − persp · cam.z                 // the Apophysis divisor (utilities.wgsl:225)
// left eye (right eye = sign flips):
cam'.x = cam.x + b/2                     // depth-dependent disparity (the actual 3D fork)
       + (∓ b/(2·Z_conv) · zr)           // convergence: constant IMAGE shift needs · zr
       + (panel_shift · zr)              // panel placement: same
cam'.y = cam.y ; cam'.z = cam.z
p'   = Mᵀ · cam' + cam_pos               // camera → world
emit_plot(p')                            // standard projection now lands it correctly
```

Key detail: under the Apophysis projection `u = x / zr`, a **constant
image-space shift c requires a world offset of c·zr** — that is what
keeps convergence and panel placement flat in raster space while the
±b/2 term alone carries the depth-dependent disparity. (In a pinhole
`f/Z` formulation this is the `∓f·b/(2·Z_conv)` term; ours just uses zr
as the divisor.)

Because the split is on the camera basis by construction, it is
orientation-agnostic: no requirement that the flame is centered or
unrotated, and camera pitch/yaw/bank/roll all "just work". (Roll is
applied post-projection as a screen rotation — it rotates both panels
about their own centers; acceptable, document.)

### Parameters

| param | default | notes |
|---|---|---|
| `baseline` | 0.033 | eye separation as a **fraction of Z_conv** (≈1/30) |
| `z_conv` | 1.0 | convergence distance (zero-parallax plane), world units |
| `view` | Parallel | enum: Parallel / Cross (sign flip on panel offset) / Triptych L-R-L (3 emits, serves both) |
| `panel_gap` | 0.05 | dead gutter between panels, as a fraction of panel width — must exceed the widest spatial-filter footprint (see Known Broken) |
| `eye_weight` | 1.0 | per-eye density weight (0.5 ≈ mono-equivalent total brightness) |

### Panel clipping

Left-eye content that projects outside the left panel must not bleed
into the right panel (and vice versa). The variation projects each
candidate emission (module helpers are in scope) and simply **does not
emit** points landing outside their own panel. No renderer clipping
stage needed. The gutter comes from the same test.

### 2D render mode

Defined but degenerate: side-by-side duplicate with zero disparity (no
depth to encode). The 2D body does exactly that and the tooltip says
so.

## Part 3 — kaleidoscope / spray notes (same plumbing, later)

- **Kaleidoscope**: a final variation with `plot_emits: N` emitting N
  rotated/mirrored copies at `w = 1/N`. Differs from post-symmetry by
  being arbitrary per-variation math (curved mirrors, per-copy
  transforms) and animatable via ordinary variation-param tracks.
- **Spray blurs**: emit K jittered copies at `w = 1/K` — a K× faster
  approximation of K iterations through a stochastic blur, with
  correlated placement.

## Known broken / accepted tradeoffs (stereo)

- **Solid mode occlusion & shadow maps**: two interleaved "objects" fight
  over one depth region. Unsupported with stereo; documented. (Manual
  two-render stereograms remain the path for solid scenes.)
- **Spatial filters smear across the seam**: density-estimation blur and
  the analytic-blur buffer have no panel awareness — mitigation is the
  `panel_gap` gutter sized ≥ the filter footprint; additionally
  force-disable the analytic-blur eligibility gate when a `plot_emits`
  variation is active (it already has an eligibility check to hook).
- **Depth effects** (DoF, fog, depth-density comp) evaluate per-eye with
  near-identical camera z — correct to first order; extreme DoF will
  decorrelate the eyes' jitter (fusion shimmer in bokeh regions). Accept.
- **Tonemap brightness** shifts with emit multiplicity — uniform across
  the image, correctable via `eye_weight` or exposure. Accept.

## Alternative considered: a renderer plot stage

Stereo could instead be a post_symmetry-style stage (the `sym_k` loop
already plots N copies and has natural camera access). Rejected for v1
because: the variation route works on `final_attachments` per-transform,
is animatable through existing variation-param tracks, adds no new
renderer UI section, and the world-space round-trip trick removes the
one real argument for the stage (camera access). Revisit only if panel
clipping in-variation proves awkward in practice.

## Phases

1. **Plumbing**: `plot_emits` field, `HAS_PLOT_EMIT` gate + cap, emit
   helpers, plot-loop integration in all three output paths, buffer
   sizing multiplier, byte-identical-when-inactive builder test.
2. **`stereogram` variation**: math above, params, panel clipping,
   render verification with a {2,3,7}-style depth scene; manual fusion
   check (parallel + cross); baseline configs (deterministic RNG) for
   the visual suite.
3. **Demos/follow-ups**: triptych mode polish, kaleidoscope variation,
   spray blur experiment, analytic-blur interaction gate.

## Test plan

- Builder: no emitter active ⇒ byte-identical WGSL (existing pattern).
- Parity: same flame through direct / tiled / CPU paths, tolerance-equal.
- Brightness: N-emitter at w = 1/N within tolerance of the mono render.
- Visual baselines: stereogram config in `tests/visual/configs/3d/`
  (deterministic), checked by the thumbnail suite.
