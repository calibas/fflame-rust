# Deep zoom for flames: open items

The running list for cylinder targeting and the inverse walk. The work
behind each item is written up in the design docs:

- [flame-deep-zoom.md](flame-deep-zoom.md): the stages, and the precision plan (stage 3).
- [inversive-targeting.md](inversive-targeting.md): the inverse walk and its completeness.
- [gpu-cylinder-planning.md](gpu-cylinder-planning.md): planning on the GPU, and on the web (§17).

**Priorities (2026-09-23): coverage first, then performance.** Coverage
means more flames, deeper zooms, and images without holes. Testing on
other platforms (macOS/Metal, Firefox) is planned for later.

Status: **open**, **investigate** (measure before building), **done**.

---

## Coverage

### C1. Precision past ~64k zoom: stripes -- done (2026-09-23)

**Seen:** stripes past about 64k zoom.

**Cause:** f32. A forced point is computed in world coordinates, and the
view centre (`GpuParams::pan_x/pan_y`) is f32 too. At the saved
grand-julian view (x = 0.268, y = -0.111, 1280x720), f32 spacing against
the pixel size is:

| zoom | f32 spacing in x |
|---|---|
| 32k | 0.18 px |
| 64k | 0.35 px |
| 131k | 0.70 px |
| 1e6 | 5.4 px |

At 0.35 px, each pixel column holds either two or three representable x
values. Points snap to them, so neighbouring columns get density in a
2:3 ratio, which shows as stripes. y is 4x finer at this view (|y| < 0.125),
so the stripes should appear first as vertical bands.

**Fix:** stage 3 of [flame-deep-zoom.md](flame-deep-zoom.md), built as
[deep-zoom-precision.md](deep-zoom-precision.md). The replay's deep steps
run as offsets from f64 reference orbits. Per sample, the render is
within 0.02 px of f64 at the 99th percentile on grand-julian, random1
and julian-disc at 1e4-1e8, and grand-julian renders cleanly to 1e10.

**Found on the way:** the old replay was also displaced, by about 4e-7
world units at every depth (0.3 px at 1e4, 3 px at 1e5), from the GPU's
transcendental functions, not f32's rounding.

**Left over:**
- The GPU planner in offsets: past a view radius of about 2e-6 (zoom
  ~2e6), plans are made on the CPU, 3-5x slower.
- Zoom past f64's pan (about 1e12).
- The references' cost: on the web, a plan of ~10k words takes about
  0.5 s longer (1.2 s to 1.7 s warm, at 60 Hz), since they are computed
  on the page's thread. On twelve threads it is 3-7 ms.

- The forced word's image of a reference point is computed on the CPU at
  any precision (`BigFloat`). That is the reference orbit.
- Each sample is carried as an offset from it, through forward
  difference forms of the flame's kernels. [ifs-perturbation-delta.md](ifs-perturbation-delta.md)
  §3 has the construction; the CPU inverse forms exist and are gated.
- The splat becomes (reference − centre), in f64, plus the offset in
  f32. Nothing near the view is ever an f32 world coordinate.
- The kernels to port are only those the analysis accepts (affine,
  roots, disc, bubble), because only those flames are targeted.

### C2. `pre_blur` -- done, v1 (2026-09-23)

**Before:** rejected by the analysis as not affine. **Now:** a blur that
forgets its input (a renewal, v1 below) is planned. A smaller blur is
refused with its numbers (C2c).

**The idea:** keep blur out of the plan and let the forced replay apply
it. That is **biased**:

- Holes: a word whose blurred image reaches the view, but whose
  unblurred cylinder does not, is never forced, so its contribution goes
  missing.
- Wasted samples: forced samples get blurred off the view, and the share
  of samples that land falls with zoom.

**The sound version is cheap in principle.** `pre_blur` moves the point
by at most 3x its weight, in a random direction (JWF's six uniforms
minus 3, scaled by the weight). It runs after the affine and before the
variation. So the walk can treat it as a bounded dilation:

- Grow the preimage disc by 3w at each backward step through a blurred
  transform. Completeness stays by construction.
- The replay already applies the blur, since it runs the transform's
  own code.

**What that implies:**

- A word whose last-applied transform is blurred cannot localize below
  about 3w x |J|. Below that scale the true image there is smooth; there
  is nothing to zoom into.
- A word whose blur comes earlier, deeper inside it, has that blur shrunk
  by the maps applied after it, and zooms normally.

**Needs:**

- The analysis must accept `pre_blur` beside an analysable variation, as
  a transform with slack.
- Every place the walk maps a disc backward must add the slack.

**The canonical case** is the true Grand Julian (the first flame of
`assets/presets.fflame`, extracted to
`output/flame-zoom/true-grand-julian.fflame`). Transform 0, drawn about
6% of the time, is `pre_blur` 10 and `bubble` 0.2 on an identity affine.
A blur reaching 30 swamps its input, so it emits a smooth blob on the
disc of radius 0.2 whatever comes in: a **renewal**. Transforms 1-3 are
julian roots.

**Design, v1 (2026-09-23): renewal symbols.**

- **Analysis.** The planner's analysis strips `pre_blur` from a copy of
  the transform and records its reach, 3x the weight, in the frame the
  kernel sees. That is allowed when it is the transform's only
  pre-phase variation. The escape engine's analysis still refuses it.
- **Renewal, or refused.** A blurred transform is a renewal when its
  reach covers the attractor's image in its frame plus the kernel's
  preimage radius: every point then has a positive chance of landing
  anywhere in the transform's output. Bubble's inner branch holds a
  preimage of every image point within radius 2. Anything else is
  refused with its numbers ("a partial blur is not planned yet"), not
  planned with holes.
- **Sample and replays with the blur.** The walk's attractor sample and
  its CPU replays draw the blur, as JWF does. Replays are seeded by word
  and point, so a plan is still the same however it runs. The GPU
  planner and the render run the variation's own code, blur included.
- **A renewal child is kept, never carried**, since its region is the
  whole attractor. It is kept or dropped by geometry, not by replays: at
  depth, a smooth part can land 1e-7 of the time, and 400 replays would
  drop it and leave the view black. So the view is pulled back through
  the node's word, along every branch and with a radius bound. The child
  is kept when that region can reach the renewal's output disc. Replays
  only measure its efficiency.
- **References.** A word that starts with a renewal starts its
  reference after it, at points of the rest's region (its node's own),
  with `m >= 1`, so the blurred step always runs as the shader's own
  absolute code.
- **Gates.** Coverage against an untargeted chaos game with the blur, at
  views it can reach. The targeted picture against the untargeted one.
  A view inside the blob (the smooth part) must not come out black. The
  per-sample gate on its words.

**Built and measured.** `Backward::renewal`, `Backward::reaches`,
`forward_blurred`, `analyse_2d_maps_blurred_sliced`. On the true Grand
Julian (`what_the_true_grand_julian_is`):

| view | words (renewal-first) | coverage | renewal words: mass, efficiency | the rest: mass, efficiency |
|---|---|---|---|---|
| the preset's (1.8) | 26 (1) | 1.0000 | 6%, 1.00 | 94%, 0.96 |
| 1e2 on the attractor | 4,035 (354) | 0.9999 | 54%, 0.15 | 46%, 0.90 |
| 1e3 on the attractor | 1,065 (109) | 0.9997 | 89%, 0.04 | 12%, 0.93 |
| 1e3 inside the blob | 1 (1) | unreachable | 100%, ~0 | none |

- Coverage is against an independent chaos game with the blur, at views
  it reaches.
- Targeted against untargeted
  (`a_targeted_true_grand_julian_render_is_the_untargeted_render`): the
  targeted render lights 0.984, 0.994 and 0.999 of what the reference
  lights at 10, 1e2 and 1e3, with brightness 0.334/0.329 at 10.
- Its julian words hold per sample at 1e4-1e8: 99th percentile under
  0.003 px.
- Inside the blob the single word kept is the renewal itself, by
  geometry. Its replays hit zero times, and the replay rule would have
  dropped it.
- Plans of flames without a blur are unchanged, bit for bit.

**What v1 does not do: the blob is sampled at its own rate.** A
renewal word forces a whole blob through the rest of its word, and only
the part that reaches the view lands. At depth those words carry most of
the plan's probability (89% at 1e3) and land 4% of the time, so the
targeted render's efficiency falls toward zero there. It is correct, and
the picture's soft glow is about a quarter of the view at 1e3.

Since C2b the blur words stay out of the walk's floor and are drawn at
their square-root rate. At 1e3 they hold 59% of the mass at 0.23, and
the plan's efficiency is 0.54, up from 0.15. See C2b.

### C2b. The blob's efficiency at depth -- done for the input-free blurs (2026-09-26)

**What was found.** The user saw a quality cliff on the true Grand
Julian between zoom 1559 and 1560: 72 paths and heavy noise on one side,
1,151 and clean on the other. Brightness also flickered in animation,
and a sparse picture reads darker in the log tone map.

- The cause was the blob word `[t0a0]`, kept by geometry. At 1080x1055
  the view's disc grazes the blob's by 2e-6. That overlap is outside the
  frame: none of 4M blob points landed in it.
  - It held 99.9% of the plan and took 97% of the draws.
  - Its probability, counted into the walk's measure floor, raised the
    floor a hundredfold, so the rest of the plan stopped at 72 words
    (efficiency 0.001).
- A view 1% smaller (256x256) missed the blob and planned 1,116 words at
  0.24.

**Done.**

- **Blur words stay out of the floor.** A blob is not the view's
  measure. The fix went well beyond the cliff: the true Grand Julian's
  deep plans had all been cut short by their blur words.

  | | before | after |
  |---|---|---|
  | 1e3: words | 1,065 | 5,023 |
  | 1e3: efficiency | 0.147 | 0.544 |
  | 1e3: speedup | 432 | 1,426 |
  | 1e5: words | 14 | 4,793 |
  | 1e5: pixels lit at equal iterations | 1,131 | 9,027 |
  | 1e7: pixels lit at equal iterations | 3,394 | 9,038 |

  CPU plans take longer: 210 -> 760 ms at 1e3, and 16 -> 660 ms at 1e5,
  where the old plan was unusable.
- **Blur words are drawn at `prob · sqrt(eff)`** (`Cylinder::draw`), the
  variance-optimal rate, with deposits of `1 / draw`. The deposit
  already carries a weight (`density_weight`, as importance sampling
  uses), so this is not the obstacle C2b named. The tone map is told
  `N / (mass · S)` iterations (`Cylinders::draw_scale`), and the weights
  go in a table section at `header[6]`.
- **A costly blur word that read zero is replayed until it tells**
  (`landings`, up to 2^20 draws, within a `RENEWAL_REPLAYS` budget),
  costliest first, with the shares taken again after each.
  - If nothing lands and the bound `prob · 3/k` is under 1% of the rest
    of the view, it is dropped as negligible (`Trace::renewal_dropped`).
  - Otherwise what landed sets its draw rate.

**Result.**

- Either side of the graze, at either size, the plans agree: mass
  1.85e-4 within 0.2%, efficiency 0.46, 0.89 of draws landing
  (`a_grazing_blob_does_not_take_the_draws`).
- Rendered at 540x527, mean brightness went from 0.036 to 0.212 at 20M
  iterations, and pixel noise at 200M fell from 0.106 to 0.010.
- Targeted against untargeted: 0.984 / 0.997 / 1.000 lit at 10, 1e2 and
  1e3 (was 0.983 / 0.992 / 0.999).
- Flames without a blur are untouched: no word is a blur word.

**The conditional draw.** (Pressing since C2c: the Grand JuliaN
generator's flames all have a blob, and at depth their plans are nearly
all blur words -- `final-14` at 1e6, 100%.) A blur word's samples were
blobs drawn whole, and only the part that reaches the view lands. To
land every sample, draw the blur already inside the word's region, and
weight the sample by the blur's density there over the draw's. The
weight is no obstacle: the deposit carries one (above). What it takes is
the region and the density. Built for the blurs that ignore their input
(below); **still open for `starblur`, and `bubble` with a `pre_blur`**,
whose densities are not closed forms -- bubble's is the attractor
blurred and pushed through its two-branch inverse, an average over the
sample at each point asked.

**Built (2026-09-26): the conditional draw, v1 for the input-free
blurs** -- the plan below, with what building it found.

- **The pull-back is of the whole view.** Pulled back through a word's
  own arms, the view's centre left the arm's sector for nine blur words
  in ten: a 22-arm julian contracts about 22x a step, so three steps
  back the view's pull-back is as wide as a sector, and the word's
  points are the part inside. The pull-back of the whole view holds
  them, which is all the draw needs; nor is a point asked whether the
  symbol before lands near it, since after the blur the points are the
  blob's images. A long word's pull-back then covers most of the blob,
  and a word whose boxes hold `CONDITIONAL_BELOW` (a quarter) or more
  of the blob keeps the ordinary draw.
- **Unbiased.** Each conditional word's share of the view, from its
  boxes' mass and its samples' deposits, against long replays of the
  whole word: within sampling error at every view measured
  (`the_conditional_draw_lands_what_the_whole_draw_does`). Targeted
  against untargeted on the GPU, overlap 1.000 and brightness as before
  for every blob kind and the final flames.
- **What it does to the render** (`where_a_blur_flames_draws_go`, the
  share of draws that land):

  | flame | 1e3 | 1e4 | 1e5 | 1e6 |
  |---|---|---|---|---|
  | seed 3, before | 0.081 | 0.033 | 0.013 | 0.000 |
  | seed 3, conditional | 0.79 | 0.58 | 0.72 | 0.002 |
  | seed 14, before | 0.71 | 0.41 | 0.14 | 0.000 |
  | seed 14, conditional | 0.71 | 0.84 | 0.82 | 0.84 |

  `final-14` at 1e6 (`a_final_at_depth`), at 1e8 iterations: 5,335 of
  65,536 pixels lit to all of them, and the glow at 1e4 and 1e5
  smoother.
- **Not helped: a view in the glow itself** (seed 3 at 1e6: 14 words,
  0.002). Its heaviest-drawn words' pull-backs are not finite -- `julian`
  with a negative distance sends points near its centre to infinity --
  so they keep the ordinary draw, at the floor of `sqrt(1/400)`: they
  are never replayed at length, since `2^20` replays of an 8-map word
  are past `RENEWAL_REPLAYS`. The untargeted picture there is nearly
  empty too.

**The plan it was built to (2026-09-26).**
- *The region.* For a blur word `[B, u]`, the view's centre and a rim
  of points pulled back through `u` exactly, along every branch, as a
  final's pull-back is (`FinalMap::pull_back`): a disc per piece in
  B's output, the farthest rim point with a margin.
- *The density, exactly.* `blur`, `gaussian_blur` and `pie` draw a
  radius and an angle independently, so in B's frame a disc lies in a
  polar box, and the draw restricted to the box is the blur's own draw
  on a smaller range: the radius and angle uniform over it, and the
  deposit the blur's density there over the box's (1 for `blur`;
  `gaussian_blur`'s radial density; `pie`'s wedges). The box's mass is
  analytic.
- *The draw.* The word is drawn at the boxes' mass (`Cylinder::draw`),
  a box chosen by its mass, and each sample deposits the blur's density
  over the box's uniform one, over the box's mass. Unbiased as long as
  the discs hold the region; nearly every sample lands.
- *At depth.* Each piece's centre is its reference orbit's start
  (`m = 1`), and the sample is an offset from it, formed from the box
  offsets in the blur's polar frame without absolute coordinates.
- *In the table.* A conditional word's entry in the weights section
  (`header[6]`) is minus its block's offset, so no other table changes.
- *Not yet:* `starblur`, and `bubble` with a `pre_blur`, whose
  densities are not closed forms: their words keep the draw above.

### C2c. Blurs and finals the walk refused -- done (2026-09-25)

**Where they are.** Not in the saved corpus: of its 81 flames
(`what_the_walk_refuses_across_the_corpus`), every one with a small
`pre_blur` is refused first for other variations (`curl`, `juliascope`,
`julia3Dz`, ...). They are in the app's own **Grand JuliaN generator**,
whose first transform is a blob. Over 400 seeds
(`what_the_walk_refuses_of_generated_grand_julians`), before C2c:

| the walk | flames | why |
|---|---|---|
| read | 41 | |
| refused | 175 | `blur` (half the generator's blobs) |
| refused | 92 | `bubble` with a `pre_blur` too small to be a renewal |
| refused | 41 | `pie3D`, `starblur` |
| refused | 101 | a `bipolar` final transform (with any blob) |

**1. Variations that ignore their input -- done.** `blur`,
`gaussian_blur`, `pie`, `pie3D` and `starblur` draw a random point
whatever comes in, so a transform made of one alone is a renewal by
construction (`FreeBlur`).
- The analysis takes it out whole and is handed a stand-in, `bubble`
  scaled to the draw's radius on an identity affine. The stand-in
  carries the output's extent, for the invariant ball and the renewal's
  output disc; the walk's sample and replays draw the real variation,
  and the GPU runs the flame's own code.
- The generator's flames the walk reads: 41 to 207 of 400.
- Coverage against a chaos game 0.997-1.0 at every view tried
  (`free_blurs_plan_completely`). Targeted against untargeted on the
  GPU, overlap 1.000 at 1e1-1e3 for each kind
  (`a_targeted_free_blur_render_is_the_untargeted_render`), which is
  also the check that the CPU draws as the shaders do.
- Inside a blob, a view is the blob's smooth glow and its efficiency is
  near 0 (C2b's conditional draw). The GPU gate compares outside the
  blob: at 1e1 inside `gaussian_blur`'s, the gate's sample budget
  assumes an efficiency of 0.05 and the targeted render got a fifth of
  the reference's in-frame samples.

**2. A `pre_blur` too small to be a renewal -- done.** It is planned
as a renewal: its child kept, never carried, when the rest of its word
pulled back from the view can reach the transform's output disc.
- The refusal guarded efficiency, not completeness. The output disc
  holds everything the transform can output, so a word dropped for
  missing it cannot land. The blurred symbol is only ever a word's
  first, and every word after it is unblurred and walked as any other.
- What a partial blur's word loses by not being carried is a
  refinement. At a view smaller than its smear there is nothing to
  localize; above it, the replays measure what the word lands. The
  dilated-region sketch above would carry such words, for efficiency at
  shallow zooms. Not built: no view measured needs it.
- The output disc is tighter for a small blur: bubble's radial profile
  `4r/(r² + 4)` rises to 1 at r = 2, so an input that reaches no
  further than `r` comes out within it. The input's reach is taken from
  the walk's sample, drawn with the blur, with a 2% margin; the stripped
  maps' invariant ball need not hold the blurred attractor, and a disc
  drawn too small would drop words that land.
- The generator's flames the walk reads: 207 to 299 of 400, all but the
  `bipolar` finals. Coverage 0.998-1.0 at `pre_blur` 0.50-1.24; GPU
  overlap 0.999-1.000.

**3. Final transforms -- done, v1.** A final reshapes what is plotted
and feeds nothing forward: the orbit's points plotted in a view are
those in its pull-back through the final. The walk plans that, a disc
in the orbit's own space (`FinalMap::pull_back`), and the render
applies the final after the forced word as after every free step. The
analysis sees the flame without its finals.
- Followed: affine finals, and `bipolar` between its affine and
  post-affine, in a chain with at most one `bipolar`. `bipolar` is
  one-to-one: its output is bipolar coordinates, the angle halved,
  shifted and wrapped, and the wrap only turns the angle round the
  circle, so each output has one preimage, `(1+q)/(1−q)`. It is
  conformal, so a small view pulls back to nearly a disc.
- The pulled-back disc bounds the view's rim and interior pulled back,
  from the preimage of its centre, with a 2% margin. **Except where the
  view holds the plotted image of the plane's far points**:
  `bipolar` sends every point far from its poles towards `(0, −shift)`,
  wrapped, and a view holding that pulls back to the outside of a
  circle, which no disc round its rim holds. It pulls back to the whole
  attractor instead.
- Every drawn normal transform must carry the same finals; a linked
  transform is refused (the walk had ignored linked transforms, which
  feed the orbit).
- **Offsets through the final.** Without them a plan with finals
  replayed in absolute f32: on `final-14` (`a_final_at_depth`) clean
  at 1e4, heavy stripes at 1e5, a dot lattice at 1e6. Now the finals
  are the replay's last offset steps.
  - `bipolar`'s difference form (`final_map::bipolar_diff`, and its
    twin `ct_final_diff` in `replay_delta.wgsl`): with `a± = δ/(v ± 1)`
    the log term changes by `(ln|1+a₊| − ln|1+a₋|)/π` and the angle
    term by `(arg(1+a₋) − arg(1+a₊))/π`. A step across the wrap's seam
    lands the strip away, far off any view deep enough for offsets, and
    is dropped. The shader's form is the CPU's to a relative 1e-6 at
    offsets from 1e-1 to 1e-12 (`the_shader_final_forms_are_the_cpu_ones`).
  - The finals' rows go in the table (`header[7]`, 0 without), and each
    reference chain holds the point before each final between its bases
    and its end, which is the plotted point less the view's centre.
  - References are chosen in the plot: an error reaches it through the
    finals' Jacobian, and a word may need offsets for the finals alone.
  - The render skips the final chain for a sample `ct_offsets` took
    through them, and one replayed in absolute f32 has the pan taken
    off after it. Relative plotting is allowed for a flame whose finals
    the plan carries.
  - Per sample against f64 (`the_offset_replay_holds_per_sample`, now
    with three final flames): 99th percentile 0.0009-0.041 px from 1e4
    to 1e8.
  - `final-14` at depth (`a_final_at_depth`): clean at 1e4 and 1e5. At
    1e6 it is sparse (5,335 of 65,536 pixels lit at 1e8 iterations), and
    not for the finals: the plan is 100% the blur's words (98% at 1e4,
    99.9% at 1e5), its mass flat at 1.1e-4 from 1e5 on and its
    efficiency 0.000. The view is lit by the blob's images, smooth below
    their smear, and each blur word lands a sliver of its draws -- C2b's
    conditional draw. Every Grand JuliaN flame has a blob, so for them
    that is the efficiency item at depth.
- **How `m` is chosen: every absolute step must fit** (a C1 fix found
  here). The replay runs a word absolutely to `m`, and each point before
  it carries its step's error to the end. `m` was the last step whose
  error fits, which is enough where the maps contract. `julian` with a
  negative distance expands near its centre, and there an earlier
  step's error grows: `final-1` at 1e6 had a 20-map word off by 1-2 px
  in the orbit's own space, 99th percentile 0.92 px. `m` is now where
  the first step fails to fit. That flame's 99th percentile went to
  0.041 px; the existing flames' went down too (julian-disc at 1e8
  0.044 to 0.0076 px, true-grand-julian at 1e4 0.027 to 0.0025 px).
  Cost (`what_an_iteration_costs`, offset steps per sample weighted as
  drawn): julian-disc at 1e4 33 to 52 steps, its offset rate 162 to 122
  Miter/s; at 1e6 and 1e8, and on grand-julian and random1, a step or
  two or none, and rates within the measurement's noise (the plain
  rates, which the change does not touch, moved by up to 15% between
  runs).
- The walk's cache key and the renderer's replan key now include the
  finals and linked transforms: editing a final used to keep the old
  plan and the old analysis.
- The generator's flames the walk reads: 299 to 400 of 400. Coverage
  1.0 at views from the preset's to 1e4 (`finals_plan_completely`,
  `a_plan_through_a_final_covers_what_it_plots`); targeted against
  untargeted on the GPU, overlap 1.000 and brightness within 2%
  (`a_targeted_final_render_is_the_untargeted_render`).
- Where the view holds many far points' image, the pull-back is large
  and the plan's efficiency low (0.00-0.03 at some 1e2-1e3 views): a
  large share of the attractor is plotted there, so the untargeted
  render is dense there too.

### C3. random1 at 1e3 misses 2.6% -- done (2026-09-25)

The measurements are in [inversive-targeting.md](inversive-targeting.md)
§31-§32 and `why_is_this_view_empty`, which now prints each plan's CPU
time.

**Found.** The missed samples all arrive last through `t1a1`; every
sample point the planner has in the view arrived through `t0a0`. At
depth 1 the `t1a1` child had 5 candidates, none landing, and 400
replays reading zero, so it was dropped as EMPTY. It holds about 3% of
the view. The sample, 10 points in view, is too sparse to resolve it.

- A 10x sample (1M) takes this view to 0.9997, but only moves the hole
  deeper: random1 at 1e4 fell to 0.957. A fixed sample always runs thin
  at some depth, so the fix has to be local.

**Built.**

1. **The rescue** (`Backward::rescue`).
   - An EMPTY child whose probability is over `FORCE_WASTE` of the kept
     mass is looked for near its candidates, nearest first.
   - The nearby points come from the sample's own history
     (`near_points`): a sample point's last k maps applied to other
     sample points lie in its depth-k cylinder.
   - Every depth from 4 to 16 is tried, since the depth that lands
     varies by candidate. Measured on random1, it is 8-12; k <= 6 spreads
     past the view, and k >= 16 gathers round the miss.
   - What lands is carried as a cloud.
2. **The rescued cloud is exempt from the grid's test for
   `RESCUE_LEVELS` (6) levels** (`Node::rescued`).
   - Its points are on the attractor, but where the sample is too sparse
     for `near_landing` to vouch for them. With the test, every preimage
     was pruned and the rescued branch died one level later.
   - Each level widens the region until the sample seeds it.
   - A preimage that is not a real path costs only a replay, since its
     word is measured all the same.

| view | before | rescue | + exemption |
|---|---|---|---|
| random1 1e3 (0.25) | 0.974 | 0.995 | 0.995 |
| random1 1e4 (0.25) | 0.992 | 0.996 | 0.996 |
| random1 1e3 (0.25, off 0.6) | 0.980 | 0.985 | **0.996** |
| random1 1e3 (0.25, off 2) | 0.992 | 0.992 | **0.996** |
| random1 1e4 (0.25, off 0.6) | 0.976 | 0.978 | 0.980 (241 samples: +/-1%) |
| julian-disc 1e3 (0.6, off 2) | 0.978 | 0.978 | 0.978 |

Grand-julian: 0.999-1.000 as before. CPU plan times are unchanged
within noise: random1 230-440 ms, grand-julian 1.1-1.3 s, julian-disc
12-21 s (P3).

**Unseen branches -- done (2026-09-25).** julian-disc's misses were
deep: `t1a3 t0^15` at depth 16, prob 3.95e-4. It was UNSEEN (no
candidates at all, so nothing to rescue near) and its 400 replays read
zero.

3. **The unseen rescue.** An UNSEEN child is rescued like an EMPTY one,
   from candidates gathered over the node's cells widened by
   `UNSEEN_REACH` (4) cells, made once per node from `WIDEN_FROM` (64)
   of them.
   - It found random1's unseen branches (0.995 to 0.999 at 1e3) but not
     julian-disc's: no sample point lands within 4 cells of it.
   - Its cost was grand-julian's plan doubling (1.2 s to 2.7-3.2 s) for
     nothing: ~300 failed rescues at ~5 ms. The kept mass is 0 until the
     first word is kept, so `FORCE_WASTE` of it passes every child. Cut
     back by stopping a candidate's depths once its images no longer
     reach from the miss to the view (a deeper cylinder's are smaller):
     1.4-1.9 s of rescues became 0.11 s, with the same successes.
4. **Hidden pieces** (`Node::alt`). `MISS_WORD` showed why julian-disc's
   branch had no candidates. `t0` is `disc`, which folds radius past 1
   onto radius r-1 with the angle mirrored. The missed samples reach
   `t0^15`'s region on that second sheet, near (1.03, 0.38), a unit
   away from the 4 sample points the walk had of it. The sample holds
   none there.
   - A cloud is pulled back along every branch of an inverse, but an
     indexed node's children come from its sample points, so a piece of
     its region the sample never visited is lost for good.
   - So an indexed node keeps a small cloud of its region's points
     away from its sample points (`ALT_CAP` 32): its points pulled back
     along every branch of a map with several (from `ALT_FROM` 16 of
     them), its own hidden points pulled back along every map, and a
     seeded cloud's points more than a cell from the seeds.
   - An unseen or empty child with hidden points is carried as a cloud,
     exempt from the grid's test like a rescued one.
   - **The rescue goes first, and the hidden points join what it
     finds.** Carried alone, a few hidden points took the place of a
     256-point rescue and covered random1's `t1a0 t0^2 t1a0 t0^3 t1a1`
     worse: 73 misses in 17,876 against 38.
5. **Slices.** A rescue yields at every depth, and step 4 after every
   rescue and every node. The web gate had frames of 40-85 ms: a node
   with dozens of unseen children, each gathering over thousands of
   widened cells.

| view | before | now | now, 20,000 samples |
|---|---|---|---|
| random1 1e3 (0.25) | 0.995 | 0.9993 | 0.9991 |
| random1 1e4 (0.25) | 0.996 | 1.0 | |
| random1 1e3 (0.25, off 0.6) | 0.996 | 0.9997 | 0.9990 |
| random1 1e3 (0.25, off 2) | 0.996 | 0.9973 | 0.9980 |
| random1 1e4 (0.25, off 0.6) | 0.980 | 0.998 (241 samples) | |
| julian-disc 1e2 (0.25) | 0.9967 | 0.9987 | |
| julian-disc 1e3 (0.25) | 0.9976 | 0.9976 | |
| julian-disc 1e3 (0.25, off 0.6) | 0.9958 | 0.9989 | |
| julian-disc 1e3 (0.25, off 2) | 0.9973 | 1.0 | |
| julian-disc 1e3 (0.6, off 2) | 0.978 | **1.0** | |

"Before" is the rescue and exemption above. Grand-julian is 0.999-1.000
as before. At 20,000 samples random1 misses 14-36 in each of its 1e3
views, where the unseen rescue alone missed 15-38, and hidden pieces
carried before the rescue missed up to 73.

CPU plan times: random1 350-690 ms (from 230-440), grand-julian
1.1-1.3 s (unchanged), julian-disc 16-21 s (unchanged, P3). The cost
is in julian-disc's plans: the 0.6 off 2 view plans 80k words at
efficiency 0.14, where it planned at 0.19 with 2.2% missing.

`UNSEEN`, `RESCUED` and `HIDDEN` show in `WATCH` traces;
`why_is_this_view_empty` takes `ONLY`, `COVER_N`, `MISS_DUMP` and
`MISS_WORD` (see its doc).

### C4. Schottky flames: a Möbius analysis -- open

Two corpus flames. They are off the escape-time / deep-zoom diagonal
because the inverse walk's analysis does not model Möbius maps.

### C5. Flames over the forward word cap -- open

Three corpus flames. They would need their own routing.

### C6. An unbounded attractor -- done (2026-09-26); its blur's draw open

`output/flame-zoom/bipolar-elliptic-splits{1,2}.fflame`: `elliptic`,
`splits`, and `cylinder` with a `pre_blur`, under a `bipolar` final.
Refused (`why_a_saved_flame_is_not_targeted`).
- No transform draws among several images, so they went to the forward
  planner, which refused them with `NoInvariantBall`: `splits` 1.5 on a
  0.9 affine scales by about 1.35 and adds a fixed offset, so no disc
  maps into itself. The attractor is unbounded: a run of `k` of them
  reaches about 1.35^k, with probability about (15/21)^k, a power-law
  tail. What brings the orbit back is `elliptic`, whose `y` is
  `(2/pi) acosh(|p|-ish)`: it compresses far points logarithmically and
  bounds none -- no transform here has a bounded output, so there is no
  renewal to start words from.
- With their `bipolar` final they are the inverse walk's now (below),
  which refuses them: its analysis has none of `elliptic`, `splits` or
  `cylinder`.
- **What they need of the walk:** `elliptic` (one-to-one, an elliptic
  coordinate map with a closed inverse) and `splits` (a translation by
  quadrant, with a gap) as kernels -- forward, inverse, difference
  forms, their shader rows; `cylinder` too, whose inverse has a branch
  every `2pi` across the attractor; a `pre_blur` beside a kernel other
  than bubble whose output is unbounded, so not a renewal (the dilated
  regions of C2's sketch); and a grid that survives a power-law tail,
  whose farthest sample point sets the grid's cell today.
- **In stages.** (1) `elliptic` and `splits` as walk kernels; (2)
  `cylinder`, and a `pre_blur` beside it; (3) the grid. Only all three
  together plan these two flames; stage 1 alone plans no flame of the
  corpus (the census: nothing else there uses either).

**Stage 1, the plan (2026-09-26).**
- `Kernel::Elliptic`: forward the flame's body, written without its
  cancellations (below); inverse closed-form on the strip `|v.x| < 1`
  (`s = 2E/(sqrt(1 + 4E) + 1)` with `E = e^L - 1`, `L = (pi/2)|v.y|`,
  `xmax = 1 + s^2`, `x = xmax sin(pi v.x/2)`, `y = sign(v.y) s sqrt(2 +
  s^2) cos(pi v.x/2)`), one branch, none past the strip. Its forward is
  continuous in `x` everywhere and jumps in `y` across the rays `|x| >
  1, y = 0`; it is C1 and not C2 across the segment between the foci
  (JWF's `sqrt(xmax - 1)` where acosh has `sqrt(xmax^2 - 1)`).
- **Its cancellations.** `xmax - 1`, `xmax - |x|` are differences of
  O(1) numbers exactly where the picture needs them small (near the
  segment, near the rays). Each is a sum of `h(d, k) = d - k` terms,
  `d = sqrt(k^2 + y^2)`, taken as `y^2/(d + k)` for `k >= 0` and `d -
  k` otherwise: nothing cancels. The forward difference differences each
  `h` by the same rule chosen at the reference, so every term is O(e).
- `Kernel::Splits { base, x, y }`: `K(v) = v + base + [v.x >= 0] x +
  [v.y >= 0] y`, piecewise a translation; four inverse branches, one per
  quadrant, each valid where its preimage lands in its quadrant. A
  summed affine (`linear + splits`, as splits2 has) is folded in --
  `L v + c + w K(v) = M(v + M^-1(c + w base) + ...)` with `M = L + wI`
  -- so it is never a Newton sum, whose solve does not respect the
  branch.
- Both are the walk's alone: `analyse_2d` (the escape engine's) still
  refuses them, as its shader has no row for either.
- The offset replay's row grows from 20 floats to 24: splits' two steps
  there. Forward difference forms on the CPU and in the shader, gated as
  every kernel's are (exact against 512-bit, shader against CPU).
- The gate for the walk: a code-built bounded flame with each, targeted
  against untargeted at depth.

**Stage 1, done (2026-09-26).** As planned, with one addition: splits
counts as many-to-one going forward in `reference_chains`' `merges`
(overlapping steps put two pieces of a view among the offset steps, each
wanting its own reference).
- The forms: exact against 512-bit on 71,190 differences, 468 across a
  seam, worst 3.7e-14 (`the_forward_forms_are_exact`); the shader's
  against the CPU's, elliptic 8.9e-7, splits 9.6e-8, `linear + splits`
  9.8e-8 (`the_shader_forward_forms_are_the_cpu_ones`). The first try
  took `ln(1 + G)` where the two sides of the segment meet, which kept
  only 4.5e-8 of the digits; `ln1p` there.
- The kernels: round trip, domains and Jacobians
  (`every_registered_inverse_is_reachable_and_inverts`,
  `the_kernels_jacobians_are_the_derivative`, the fixtures of both).
- The walk (`elliptic_and_splits_are_walked`, two flames written to
  `output/flame-zoom/elliptic-splits-{julian,final}.fflame`): a
  julian's arms beside an elliptic and a `linear + splits` whose steps
  overlap, and the elliptic and splits alone under an affine final.
  Targeted against untargeted at 1e1-1e3, two points each: overlap
  1.000 everywhere, brightness equal where the reference is sampled.
- At depth (`the_offset_replay_holds_per_sample`, both flames added):
  the offset replay's 99th percentile 0.0009-0.019 px at 1e4, 1e6 and
  1e8, no bias, where the plain replay is off by 440-4700 px at 1e8. Two
  of 5,473 julian samples at 1e6 are off by more than a pixel (3.7 px
  worst), inside the gate's one in a thousand.
- bipolar-elliptic-splits1 and 2 now stop at `cylinder` alone: "transform
  2 uses `cylinder`, which is not affine" -- stage 2.

**Stage 2, the plan (2026-09-26).**
- `Kernel::Cylinder { k0 }`: forward `(sin x, y)`; inverse two branches a
  turn, `asin(u) + 2 pi k` and `pi - asin(u) + 2 pi k`, one-to-one on the
  strip `|u.x| < 1`. The turns are counted from the invariant ball as
  disc's rings are (`k0` the lowest the ball's reach in the pre-frame's
  `x` meets, set once the ball is known), capped. Many-to-one going
  forward, so it counts in `merges`. Forward difference `(2 cos(x +
  e.x/2) sin(e.x/2), e.y)`. The walk's alone, like elliptic and splits.
  Summed with an affine it is refused: Newton's seeds would not respect
  the turn.
- **A `pre_blur` beside it is a renewal**, as beside bubble: its output
  disc from the attractor's image in the kernel's frame dilated by the
  blur's reach -- `sin` of that `x` range (all of `[-1, 1]` past a turn)
  by the `y` range, boxed, into the post-affine. Complete whatever the
  blur's reach (C2's argument); what a partial blur costs is draw rate,
  measured as bubble's is.
- The gate: a code-built bounded flame shaped like C6's -- elliptic, a
  contracting splits, `cylinder + pre_blur` under a bipolar final -- and
  one with a plain cylinder whose pre-frame spans several turns, targeted
  against untargeted and per sample at depth.

**Stage 2, done (2026-09-26).** As planned, and the offset replay's
references changed for every word with a map many-to-one going forward
(bubble, disc, cylinder, splits).
- The forms: cylinder's exact against 512-bit (with the others, worst
  3.7e-14), the shader's against the CPU's 4.9e-7.
- The walk (`cylinder_is_walked`, `output/flame-zoom/cylinder-{blur-
  bipolar,turns-julian}.fflame`): bipolar-elliptic-splits1 with its
  splits contracting (0.6 where it has 1.5), its `cylinder 0.099 +
  pre_blur 0.5` a renewal, overlap 0.994-0.997 at 1e2-1e3, the targeted
  render the reference's structure with its sampling gaps filled; a
  julian beside a cylinder of two turns (four branches), overlap 1.000.
  Per sample at depth, 99th percentiles 0.0004-0.0009 px (1e4-1e8) and
  0.0045-0.011 px (1e4-1e6); at 1e8 none of the second's sampled words
  land, and the gate now says so rather than judging a handful.
- **The references past a many-to-one map, found on the way.** A julian
  beside a cylinder of pre-scale 6 (stretching `x` by up to 3) was 0.15
  px off at 1e4: its words' regions are in many small pieces, one per
  turn, and `m` was read off the first reference's path alone -- which
  ran by a fold, where the Jacobian is nearly zero -- while a sample one
  turn away amplified its first absolute step by 0.26. And the seeds
  were clustered `4 size[m]` apart, which by that fold read as one
  cluster. Now, where the word has such a map, every landed seed's path
  must fit (`m` is where the first step fails along any), and seeds are
  one cluster only within the distance an offset keeps the plot's
  tolerance along the worst path, up to 16 references
  (`MAX_PIECE_CHAINS`, the shader's bound). The 99th percentile there
  went from 0.16 to 0.029 px; every other flame of the gate is unchanged.
  Its cost (`what_the_references_hold`): julian-disc's references from
  ~400 ms to 1.2-1.6 s, against plans of 17-21 s (+5-7%), for no change
  in its precision; grand-julian's 5 to 12 ms.
  Two things tried and dropped, measured: carrying the whole word from
  its first point (`m = 0`), worse -- the region there is in more pieces,
  farther apart (one sample 3.9 px off); and a reference per piece from
  `pieces()`, no difference -- it misses real pieces (the landing index
  drops the reference's own path where its sample is sparse, and a point's
  preimages miss a piece whose image stops at a fold short of it).
- **Open: more pieces than seeds.** The pre-scale 6 flame still has a
  sample 0.54 px off at 1e4: its piece holds none of the word's 8 seeds
  (`REF_SEEDS`). Where a word's region is in more pieces than that, a
  sample in a piece without a reference is only as good as absolute f32
  at its first step. Its efficiency is 0.02 there, and at 1e6 none of the
  gate's sampled words land. Efficiency is P3's (julian-disc's many-to-one
  words are inefficient for the same reason); the pieces are this item's.
- bipolar-elliptic-splits1 and 2 now stop at stage 3: "no bounding ball:
  the nonlinear maps do not keep the set bounded" (the message said "root
  maps", from before the walk had other kernels).

**Stage 3, done (2026-09-26): both flames plan, completely.** Measured
first: splits1's farthest sample point is 10^4 times its 95% radius
(about its median), splits2's 5*10^7; every other flame of
`output/flame-zoom` is within 28.5 (free-pie3D; grand-julian 15-18). The
mean is dragged by the tail too: splits2's 50%, 90% and 99% radii about
it were one number. Five things, each found by the one before:
- **The ball.** The walk's analysis takes the bulk's ball where none is
  invariant, as the inversions do (S3): it counts branches by it and
  bounds nothing. The escape engine still refuses.
- **The grid** spans the farthest point or `GRID_TAIL_SPAN` (32) of the
  95% radius about the median, whichever is less: no other flame's grid
  moves, and the bulk is resolved as free-pie3D's is. Across the
  farthest point, splits1's whole bulk was one cell of 1,200, and a 1e3
  view drew 84% of what the untargeted render did.
- **A view larger than the grid** is seeded with its sample points, not
  a 64-point cloud: a finals flame's view holding the image of infinity
  pulls back to the whole attractor, a disc 2e10 across about the
  tail-dragged mean, whose cloud lay in empty space -- the walk planned
  the renewal alone, 162 of 9,030 pixels.
- **Cylinder's output is a strip.** A renewal carries one where its
  kernel bounds a coordinate whatever the input (`Renewal::strip`):
  `pre_blur + cylinder` outputs a line `2|w|` wide however far its `y`
  goes. The disc about it, 6e4 across, met every node, and its words
  held all of a 1e4 plan's mass at an efficiency of nothing: speedups
  0.14-0.55, now 3-1250.
- **Hidden pieces, by the region's size.** A hidden point was dropped as
  "beside" the node's sample points when within a cell of them; with
  splits1's cells 3.8 across, a second piece 4.4 from a node whose
  points spread 9e-3 was dropped, and 3.5% of a 1e4 view with it. Beside
  is now also within four of the points' spread. The other flames pay
  for it at depth, measured twice each way on an awake machine (the runs
  agree within 3%): grand-julian 1e6/1e8 1.24/1.39 to 1.42/1.59 s,
  random1 3.05/7.6 to 3.46/9.0 s, julian-disc 1e6 17.5 to 21.6 s (its 1e4
  and 1e8 unchanged). What they buy there: nothing measurable -- the 14
  views of `why_is_this_view_empty` have the same coverage to every
  printed digit, one julian-disc view's efficiency 0.028 to 0.323 and one
  0.141 to 0.133. At 16 and 64 spreads the cost is the same as at four:
  the pieces carried are far outside their node's region, real pieces a
  cell had merged. Merging real pieces is how splits1 lost 3.5% of a
  view, so the cost is kept.
- Tried and dropped, measured to change nothing: discs past a map's
  image counted as unreachable in `reaches`, and the part of a disc
  across a seam pulled back from the image's edge.

Results. Coverage by an independent chaos game through the final
(`the_tailed_flames_plan_completely`): 1.000 at every view it can judge,
1e3-1e5 on both flames at two points (at 1e5 splits2's views land too few
of its samples to say). Targeted against untargeted
(`an_unbounded_attractor_is_planned`): overlap 0.999-1.000 at 1e2-1e3.
Per sample at depth (`the_offset_replay_holds_per_sample`, both added):
99th percentiles 0.0006-0.0074 px at 1e4-1e8, where the plain replay is
54-217 px off at 1e8.

**Open: the renewal's draw.** Speedups are 2-20 at many views and
efficiencies 0.000-0.08, because a shallow word through the `pre_blur +
cylinder` (probability ~1e-3, 3.5% of one view) lands its blur in a
target a few thousandths across once in thousands of draws. It is
C2b's open item for a blur beside a kernel -- a draw conditioned on the
target, per sample, since the blur's output depends on its input here.

**Found on the way, fixed: a final on a flame without arms.** The
forward planner never looked at final transforms: it planned the view
in the orbit's own space, and its composed arm plots view-relative. A
filled carpet under an affine final rendered targeted came out black,
nothing lit where the untargeted render lit every pixel. A flame with
final or linked transforms is now the walk's (`Cylinders::walked`),
which pulls the view back through a final and refuses a linked
transform with its reason: the carpet under an affine and under a
`bipolar` final, targeted against untargeted, overlap 1.000 and
brightness equal to three digits at 1e2 and 1e3
(`a_final_on_an_affine_flame_is_planned`).

### C7. A view holding a final's image of infinity -- open (2026-09-26)

**Seen:** bipolar-elliptic-splits1 and 2 at their saved views, pan (0, 0),
plan three depth-1 words holding the whole attractor at every zoom
(speedup 0.5, never forced): Focused Rendering does nothing there.
`bipolar` sends infinity to the origin, so the view is the far tail seen
from every direction, and its pull-back is everything outside a large
circle, which no disc stands for; `FinalMap::pull_back` answers the
whole attractor. There is much to gain: the view holds 13.8% of splits1's
plotted sample at its saved zoom, 0.45% at 1e3, 0.062% at 1e4, 0.002% at
1e6 (splits2 12.8%, 5.9%, 1.7%, 0.16%).

**The plan.**
1. **The region, through the final.** A `View` may be the plot's
   (`View::plotted`): a point lands where its plotted image is in the
   disc. The walk plans such a view where it holds the image of infinity.
   Its root is the sample points plotted into it, exactly, with the
   view's rings pulled back through the final (`FinalMap::inverse`) as
   its hidden points; every landing test (replays, gathers, the rescue)
   goes through the final. The GPU planner is not asked (CPU only), a
   renewal is kept in doubt, and no conditional draw is made, until each
   is taught the region.
2. **A partial blur, carried where it is small.** The only way out along
   the tail is a run of `splits`; `pre_blur + cylinder` (a renewal) can
   reach the tail only from ten times farther in `y`, rarely, so kept at
   the top of every word it forces mass for nothing. Where the blur's
   reach is small against the region, its child is carried as the
   kernel's pull-back grown by the reach (C2's dilated regions) rather
   than kept as a renewal.
3. **Gates.** Coverage against the chaos game through the final at (0,
   0) from the saved zoom to where it can still judge; targeted against
   untargeted where the reference is dense; the per-sample offsets at
   depth.

**Built (2026-09-26), in the order the measurements asked for it.**
- **The region alone** planned real words at (0, 0) -- 300 to 1,000,
  depth 30-64 -- and changed nothing: words starting with the blurred
  `cylinder`, kept in doubt, held 0.17 of the mass at every zoom (speedup
  0.1).
- **The partial blur carried** (`Backward::renewal_here`; the cloud's
  pull-back through it gets a ring at the blur's reach about each
  preimage): efficiency 0.5-0.87, splits1's speedup 4.8 at 1e3 to 7,170
  at 1e6. But coverage fell with depth, 0.987 to 0.93 on splits1.
- **The tail, sampled by splitting** (`tail_sample`). The misses were
  one path, `[1, 2, 1^k]`: a far point the cylinder brings ten times
  closer, then `k` splits back out. The walk's child through the blur
  came back "no candidates, 400 replays land nothing": the orbit's 100k
  points never go that far, nor do replays from them. So chains of the
  chaos game start from the orbit's points past the grid's span and are
  cloned each time they cross up into a level of radius (the levels
  doubling to 1e11) -- multilevel splitting -- and run until they fall a
  sixteenth of the span in. Two things measured wrong first: chains
  stopped AT the span never took the path back out after the pull-in,
  and cloning only a chain's first crossing of a level never cloned the
  climb back (one such path in 12,798 points; 21 in the 50k now). The
  points stand for regions -- the index, the landings, the orbit's
  children -- and never for measure: the verify replays, the draw rates
  and the frame read only the orbit's own (`Backward::natural`).
- **References past the blur.** An offset cannot cross a blur, so `m` is
  never before the last blurred symbol's successor, and a carried word's
  seeds are taken past the blur by a draw of it.
- **`bipolar`'s difference, far out** (`final_map::bipolar_diff`,
  `fd_bipolar`). At (0, 0) the plain f32 replay is exact -- the final
  shrinks the tail's absolute error to nothing -- and the offset replay was
  36 px off: its two log terms' changes, each O(e/|v|), cancelled to their
  O(e/|v|^2) difference. `bipolar` is the log of `(z+1)/(z-1)`, so the
  change is `ln(1 + q)`, `q = -2e/((v+e-1)(v+1))`, every term O(e/|v|^2).
  Exact to 6e-16 against 512-bit at |v| 1e2-1e10
  (`bipolar_diff_is_exact_far_out`); the shader within 1.3e-6 there.

**Results at (0, 0).** Coverage 1.000 on both flames from the saved zoom to
1e5, 0.997 at 1e6 on splits1 (one miss in 368). Targeted against
untargeted (`an_unbounded_attractor_is_planned`): overlap 0.992-1.000,
brightness equal to two or three digits, efficiency 0.67-0.79; at 1e5
the untargeted render is scattered dots and the targeted one the whole
self-similar structure. Speedups (1280x720): splits1 4.5 at 1e3, 47 at
1e4, 505 at 1e5, 4,800 at 1e6; splits2 1.3 at 1e4, 4 at 1e5, 13 at 1e6,
near its ceiling -- its view holds 0.16% of its plot at 1e6. At the saved
zooms (42, 235) forcing is still not worth it: the view holds 13-14% of
the attractor.

**Open.**
- **Plan time.** splits1 at (0, 0): 1.8 s at 1e3, 3.9 s at 1e4, 8.9 s at
  1e5, 19 s at 1e6 (splits2 1-3 s); the tail sample's points make large
  regions. Measured on the CPU only: a plotted view is not the GPU
  planner's. By step (`what_a_plan_of_the_image_of_infinity_costs`), 8.7
  of the 18.6 s at 1e6 was deciding kept or carried, and of that nearly
  all was preparing rescues one after another -- 103,703 over three
  views, whose widened cells and gathers took 12.2 s and whose replays
  0.34 s: in the far tail nearly every node has unseen children (an
  `elliptic` preimage of a far region is farther out than any point).
  They run on every thread now, widening, gather and replays, with the
  same answers: 2.6 s at 1e4, 5.5 s at 1e5, 10.9 s at 1e6, and random1 1e8
  7.5 to 7.3 s, grand-julian 1e8 1.6 to 1.5 s, plans identical word for
  word. What is left at 1e6 is the replays (4.6 s, 40M) and the checks
  (3.1 s, 26M), already parallel: a replay of a 60-symbol word from an
  orbit point that cannot reach the far tail runs to its end to find
  so.
- **The GPU planner plans a plotted view** (2026-09-26). Its kernels take
  a word's end point through the finals the last symbol's transform has
  before the test (`ct_apply_finals` in `replay.wgsl`, the render's own
  loop; `PlanView::plotted`), with the finals and the attachment lists
  uploaded as the render lays them out. It resolves such a view at any
  depth: near infinity's image the final shrinks a far point's absolute
  error with the point, so the plot's error is ~4e-7 of the plotted
  distance from that image (`gpu_resolves`). As complete as the CPU's
  (`the_gpu_plans_the_image_of_infinity`): coverage 1.000 at every zoom
  on both flames, 0.997 at splits1's 1e6 as the CPU's.
- **Settling a node, on every thread.** A node settles on its own
  (`close`), so on the desktop all of a level's do at once; the rescues
  it asks for are found before it, also at once. Plans identical word
  for word (grand-julian, random1, the C7 views).
- Together, splits1 at (0, 0): 2.6 s at 1e4 on either planner (1.9 s on
  the GPU), 3.9 s at 1e5 on the GPU (4.7 on the CPU), 6.0 s at 1e6 on the
  GPU (9.7 on the CPU) -- from 3.7, 8.3 and 17.7 s. random1 1e8 7.5 to 6.1
  s. What is left at 1e6 on the GPU: its replays, 2.7 s over 327 batches
  (2.0 s of it waiting on the GPU), the rescues' replays on the CPU, 1.3-2
  s, and the rest under 0.4 s each.
- **The per-sample gate cannot check a word through a blur** (the GPU
  draws its own): at (0, 0) most words carrying references are such.
  Their offsets start after the blur by construction; the renders above
  are the check.
- The partial blur is carried only in plotted views. Elsewhere it is
  still a renewal, with C6's draw problem.
- **Only a sampled tail is planned through the final**
  (`Backward::holds_infinity`). A Grand JuliaN generator flame whose far
  points come from a negative-distance root -- its grid not capped, so no
  tail sample -- was planned at infinity's image from the orbit's handful
  of far points: 188k words in 12 s drawing 98.3% at zoom 40 (the view
  holding 0.05% of it), and refused as empty at 1e2. Such a view keeps
  the whole attractor as its pull-back, as before. Sampling such a tail
  too is open.
- A frame-time budget test of the simulations
  (`mccabe_meets_the_interactive_budget_at_1080p`) failed once under the
  full suite and passes alone in 0.8 s: a timing test the suite's load can
  push over.

### C8. `juliascope` -- done (2026-09-27)

The corpus census (2026-09-27) had juliascope in four refused flames. It
is the cheapest of the census's blockers, not the biggest: alone it
clears one of them. JWF-rando32-simplified sums a `julian` and a
`juliascope` in one transform, which the analysis refuses as two kernels
summed (`NotAffine::MixedSum`); JWF-rando7 and its re-export also carry
`curl`, `boarders` and `combimirror`.

**The kernel.** juliascope is julian with its odd arms mirrored: arm `k`
takes `(2πk ± arg z)/n`, `+` on even arms and `−` on odd, one draw
choosing both (JWF `JuliaScopeFunc`). An odd arm is the root of
`conj(z)`, so each arm fills the same sector as the root's, reflected on
the odd ones, and `Kernel::Root` carries it as `mirror: true`:

- **Forward**: the root's at `conj(z)` on an odd arm (`Kernel::
  mirrored_arm`, the arm counted mod `|n|`). `atan2(−y, x)` is
  `−atan2(y, x)` bit for bit, the negative real axis included.
- **Inverse**: still one branch, as the root's is: the arm is read off the
  sector `v` is in (`Kernel::root_arm`), and an odd sector's preimage is
  the root's conjugate. That is continuous, since a sector's edge is the
  image of the negative real axis from either side, and creased along
  the edges between arms of different parity. `singular_distance`
  includes the nearest crease.
- **Radius**: juliascope's is `|z|^{dist/power}`, the power's sign kept,
  where julian's is over `|power|`. So `d = dist·sgn(power)`
  (`INVERSE_JULIASCOPE`). A fractional power is refused: the body
  truncates it, the arm table (`bound::JULIASCOPE_ARMS`) takes its
  ceiling.
- **Forward differences**: the root's at `conj(v)`, `conj(ε)` on an odd
  arm, on the CPU and in `replay_delta.wgsl` (the row's third parameter
  is 1 for a mirror). The inverse difference forms are the escape
  engine's, so a mirror has none: it is the walk's alone (`walk_only`).
  The GPU planner and the render replay its arms by the variation's own
  body, which already reads the forced arm.

**Results.**

- Round trips, Jacobians against central differences across every sector
  (the creases held off by `singular_distance`), and the registry gate
  pass.
- The forward forms are exact against 512-bit (worst 3.7e-14 over all
  kernels). The shader's match the CPU's to 1.02e-6, as the root's do
  (1.3e-6).
- `juliascope_is_walked`, targeted against untargeted at 1e1-1e3:
  - juliascope-arms, a code-built flame with a power 5 beside a power −4
    summed with a `linear`: overlap 0.976-1.000, speedup to 1,138×.
  - juliascope-rays, JWF-rando32-simplified4 (a power-7 juliascope
    alone), set to 2D: overlap 1.000. Without its filter the lit counts
    are equal too (80/80, 141/142 at 1e3); with it, see C9.
  - **A `.flame` imports in 3D** (`flame_xml`: JWF and Apophysis flames
    are all treated as 3D), and Focused Rendering is 2D only, so the
    renderer does not target one. The first run of this gate compared
    the untargeted render with itself at fewer samples: its 0.975-1.000
    (commit b3201ca2's message) measured nothing. Every other flame the
    gates read is 2D. In the app, a corpus flame is not targeted until
    switched to 2D.
- The per-sample offset gate (`OFFSET_FLAMES` picks flames): mean offset
  error 0.0002-0.0006 px at 1e4-1e8 on both, worst 0.22 px (arms, 1e8).
- The census: JWF-rando32-simplified4 reads (7 symbols, 120 ms), 39 of 105
  flames with the two code-built ones. JWF-rando32-simplified stops at its
  `julian + juliascope` sum and a `spirograph3D`; JWF-rando7 at `curl` and
  `boarders`.

### C9. A spatial filter under targeting -- done (2026-09-27), a residual

Found by C8. With JWF's `filter` 0.75 (`filter_radius`), juliascope-rays
targeted lit 0.52 and 0.51 of the reference's pixels at 1e3: exactly the
pixels an unfiltered render lights (80 and 142), so the filter did
nothing. The filter is bilateral, run on each batch's histogram, and its
edge-preserving `σ_d` was the batch's samples over the frame's pixels
(`compute_kernel.rs`). An untargeted batch at depth lands `mass` of its
samples in the view and a targeted one nearly all, so the targeted `σ_d`
was `mass` times too small for the view's densities, and every
neighbour read as an edge.

Now a targeted batch is counted as the untargeted batch it stands for,
by `cylinder_iteration_scale()`, as the tonemap's `sample_density`
already counts it: the density-to-`σ_d` ratio is the untargeted
render's in expectation, and nothing changes untargeted. Overlap 1.000
at every zoom.

**The residual**: the targeted render now lights more of the fringe --
275 and 460 pixels where the reference lights 153 and 279 at 1e3, mean
brightness of lit pixels 0.49 and 0.51 against 0.56 and 0.58. An
untargeted batch at depth has a handful of samples in the view, whose
noise the bilateral weight reads as edges, so it blurs less than its
expectation; a targeted batch is dense. A reference ten times longer
barely moves (159 and 312): the noise is a batch's, not the render's.
A filter that depends on how many samples a batch holds is the filter's
design, not targeting's -- untargeted, it also moves with the batch size.
No visual test targets a filtered flame.

### C10. The per-sample gate at cylinder-turns-julian 1e6 -- done (2026-09-27)

`the_offset_replay_holds_per_sample` failed there from 7767717f on: all
850 samples in view off by ~1e46 px.

**Why.** The replay table (`pack_words`) holds its positions as f32,
exact to 2^24 floats, and a plan whose references would pass that had
them dropped whole, with a warning: the render replays it plainly, pixels
off at depth. The code said no measured plan came near (julian-disc at
1e6, ~10M floats). This one does: 210,867 words, 206,576 with
references -- 558,493 chains, 23.4M floats -- beside a 5.9M-float table.
The gate did not notice. It read each word's block offset from the
blocks array the table no longer had (the header's 0), which is word
data, and the shader ran on it: exactly `(0, 0)` for every offset. On
the CPU the same references replay to 1e-8 px.

It appeared with 7767717f because that commit made the walk faster, not
different: its up-front rescues match `close`'s one for one (compared).
The walk ran out of time either way (`TIME_BUDGET`, 20 s; this plan takes
23 s) and forced its frontier where it stood, and the faster walk stood
deeper: depth 21, 190,734 of the words forced unmeasured, where the old
one stopped at depth 19 with 87,990 words, whose references fit. Making
the new walk do only more work (each rescue twice) brought back the old
plan.

**The fix.** f32 holds every even integer to 2^25, so every position the
table stores -- its sections, each word's block, each conditional block
-- is placed at an even float (`at_even`, at most a float of padding a
block), and the table may hold `TABLE_FLOATS` = 2^25 floats: 128 MiB,
WebGPU's and wgpu's default `max_storage_buffer_binding_size`, which it
is bound whole under. No reader changes. This plan's table is 29.5M
floats (118 MB) and replays at 1e6 to 0.014 px at the 99th percentile,
none of 850 off by a pixel.

The gate now fails where a plan's references do not fit, unless the walk
ran out of time (`Cylinders::timed_out`), which it reports: C11.

### C11. cylinder-turns-julian at 1e8: the walk runs out of time -- done (2026-09-27); two accuracy tails open

At 1e8 (at the gate's point, 0.75) the walk ran 25 s and forced its whole
depth-23 frontier: 265,106 words, 264,679 of them unmeasured, whose
references would take 52M floats, past any table (C10), so the render
replayed the plan plainly.

**Why the frontier grows.** The beam ranks the nodes a replay has
measured and carries every node none could, and at 1e8 that is nearly
all of them: a depth-20 word's image is far larger than the view, and its
replays' handful of points never land. The flame's pieces overlap (the
julian's two arms, the cylinder's four branches), so each level makes
1.6 nodes of each, while their mass falls by 2.5: 1, 2, 7, 17 … 66,264
at depth 20, 265,087 at 23. Reaching words the view's size would take
some 13 levels more, ~450 times the words: enumeration does not get
there. A plan forced shallower is still worth drawing: complete, and at
an efficiency around the view's measure over the frontier's (~5e-4),
far above the untargeted render's share of the view.

**Forced before it outgrows the table** (`FRONTIER_FLOATS`). Before each
level the walk projects the frontier it would make, at the last level's
growth, and forces the one it has if that projection's words would take
over half the replay table with their references (`word_floats`, at
`MAX_CHAINS`; half for the words past a many-to-one map, which may
carry more, and for the words kept). 1e8: 4.4 s, 66,264 words at depth
20 (13.2M-float table). 1e6: 11.2 s, 87,990 words at depth 19, where it
took 23 s and 210,867. Neither runs out of time now.

The first version projected the kept words too, and julian-disc keeps
~25k words 62-84 symbols long, so it forced every frontier at once: at
1e6, 80 unmeasured words held 99.8% of the mass, efficiency 0.001 and
speedup 3.3e5 against 0.156 and 7.6e7. Fixed the next commit: the
frontier alone is projected, and julian-disc plans as before C11.

**Accuracy at depth: two tails, open.** The gate's 48 points a word land
1 at 1e8, since a forced word lands at ~5e-4. Searching the sample for up
to 8 landing points from each of 2,000 words finds what it does not:

- 1e8, 103 points from 32 words: median 0.018 px, 99th percentile 2.5,
  worst 10, 4 over a pixel. Every bad one has `m` = 1 with its nearest
  reference as close as 1e-8: the error is the first step's, run
  absolutely in f32, not the offset's. The `m` rule cannot help, since
  it never goes below 1 ("step 0 always fits").
- 1e6, 3,815 points from 699 words: median 0.0066 px, 99th percentile
  1.08, worst 13.6, 44 over a pixel -- where the gate's 353 read 0.010.
  These points are in a piece of their word's region no reference
  reaches (0.2-2.5 from the nearest, words with 1-8 seeds), so the
  offset is as large as a point and rounds like one: the limit C6
  recorded at 1e4 for a steeper cylinder. A reference is built only from
  a seed whose orbit lands within 2r, and a forced word's seeds rarely do.

**Tried and reverted: offsets from the first point.** Where step 1 does
not fit, `m` = 0, with every reference starting at an f32 point so the
shader's first base is exact. At cylinder-turns-julian 1e8 that took
the worst to 0.063 px (none over 0.1), and at 1e6 to 21 over a pixel.
But bipolar-elliptic-splits2's views of infinity's image broke: 67 of
147 samples at `FD_POLE`, where step 1 "fails" only because the tail's
points are far, the old `m` = 1 was exact (0.0006 px), and a difference
form at a far point, across a large offset, has no answer (which one --
the elliptic's or the bipolar final's -- was not traced).
Also requiring step 0 to fit gave `m` = 0 to no word at
cylinder-turns-julian and still broke splits2. What would separate them
is not yet known. What would reach the pieces is seeds found in them: a
search for landing points per forced word, as `rescue` does per child --
what the 3,815 points above cost to find.

### C12. "Not contractive": translations -- investigated, not worth targeting (2026-09-27)

The census's third blocker, in four flames. presets#3 ("Spherical3", also
`output/spherical.flame`) and presets#4 ("Square Tile") are not blocked
by an expanding map but by TRANSLATIONS: identity linear parts with an
offset, σ_max exactly 1. (presets#7's σ = 2 is an expansion, not looked
at.) Tried, measured, and reverted:

- The walk's analysis (`!holes`) accepting an affine with σ_max ≤ 1: its
  inverse keeps a region's size, and the walk needs no invariant ball
  (C6). Both then read -- but as flames without arms, the forward
  planner's.
- `flatten` beside a blur: the free-blur test counts a transform's live
  variations, and a `flatten` (nothing in the plane, `AffineRole::Nothing`)
  made Square Tile's blur "not affine". Skipping plane-inert variations
  there fixes it.
- Routing a flame the forward planner fails on (no invariant ball, not
  contractive, out of nodes, too many words) to the walk.

What the walk then planned (a view at the preset's centre + (0.3, 0.2)):

| flame | zoom | time | words | efficiency | speedup |
|---|---|---|---|---|---|
| Spherical3 | 1e1 | 1.0 s | 177 (forward planner) | 1.000 | 7.4 |
| Spherical3 | 1e2 | 13.4 s | 560,065 | 0.003 | 0.06 |
| Spherical3 | 1e3 | 22.0 s, out of time | 686,359 | 0.001 | 487 |
| Square Tile | 1e1-1e3 | 0.8-4.6 s | 49-78k | 0.000-0.072 | 0.12-0.39 |

Square Tile has no contracting map at all: four translations and a blur,
so every word's image is a unit disc moved by its net translation, and
no view deeper than the disc has structure to target -- declined at
every zoom, correctly. Spherical3 is family M (similarities and an
inversion, `inversive-targeting.md`), whose disc bounds never shrink
through the inversion; the walk plans it only past its time budget,
into a table no smaller. So routing them to the walk would cost 5-22 s
of planning a view for plans the renderer declines or cannot hold. Not
kept. If a flame with arms and a translation appears, the first two
changes are what it needs.

---

## Performance

### P0. The targeted iteration rate at depth -- done (2026-09-24)

Past zoom ~1e3 the offset replay roughly halved the targeted rate
(`what_an_iteration_costs`, which now takes `ZOOMS` and `REPS` and
reports medians). Measured, in order:

- **Cheaper steps: no gain.** Forming the reference-only terms on the
  CPU removed most of a step's transcendentals: the reference in the
  kernel's frame, the root's K(v), both angles, and the cut test, which
  was skipped when the offset cannot reach the cut. The rate did not
  move, within noise. Dropping the unused kernel forms from the shader
  did not move it either.
- **The nearest-reference search: no gain** when pinned to one chain.
- **Divergence was the cost.** A warp whose threads walk different words
  runs the longest of each loop and every branch any of them takes. With
  one word per 32 threads, the targeted rate on random1 doubled:

  | zoom | per thread | one word per 32 threads |
  |---|---|---|
  | 3e3 | 331 | 644 |
  | 1e4 | 245 | 536 |
  | 1e5 | 215 | 419 |

  (Miter/s, medians of five.) One word per 64 threads measured the same,
  and one per 16 a little less. The true Grand Julian's 5-map words gain
  1.0-1.3x.
- **The draw.** Each sample still draws its word with the plan's
  probability, independently of its own point, so the estimate is
  unchanged; 32 samples share a draw. The draw is a hash of the group
  and the iteration, so a thread that burns in after a respawn stays
  with its group.
- **Checks.** Every targeted-against-untargeted gate passes. The
  stripes gate's noise floor did not move (two plain samplings: 7.98%
  and 2.58%, against 7.76% and 2.37%).
- **Why the precompute was not kept.** It tripled the per-step table.
  On julian-disc at 1e6 (50,000 words, ~46 offset steps each) that put
  the table past 2^24 floats, where its f32 block offsets stop being
  exact. That limit is latent in the current layout too: ~10M floats
  there. Worth a guard if plans grow.

### P1. Cache each word's landings across views -- measured, not worth building

**Measured (2026-09-23, `what_could_a_cache_reuse`, the current walk):**
the share of a plan's expanded nodes that the previous plan had already
expanded.

| move | at 1e3 | at 1e6 |
|---|---|---|
| pan 1/4 view | 28% | 31% |
| pan 1 view | 10% | 17% |
| zoom in 1.5x | 20% | 11% |
| zoom in 4x | 2.6% | 2.0% |
| zoom out 2x | 25% | 11% |

A plan's work sits at the depth where words fit the view, and those words
move with the view. So a cache keyed by word saves 1.0-1.45x on typical
moves, the same answer an earlier session recorded
([inversive-targeting.md](inversive-targeting.md) §28). The expectation
that it would be the biggest win was wrong.

What already covers moves: the standby plan (twice the view's radius) is
swapped in on the frame a pan or zoom-in lands inside it. What would
make plans faster at depth: the GPU planner in offsets (C1's leftover),
which the CPU now stands in for past ~2e6 at 3-5x the time.

### P2. Patterns instead of paths -- investigate

Idea (2026-09-23): track *patterns* in how points reach each pixel,
instead of explicit paths, so a moved view need not be planned from the
root again.

- **Where the patterns are exact:** for a view V inside a cylinder
  f_u(A), the words starting with u that meet f_u(V) are u followed by
  plan(V) -- exactly, since f_u is one-to-one. Other words meet f_u(V)
  only where cylinders overlap. So a plan is a finite set of patterns
  whenever the overlaps come in finitely many shapes. In the literature
  that is the IFS *finite type* condition, and its "neighbour maps"
  f_u⁻¹∘f_u' are the patterns.
  - With finitely many, planning is a walk on a small automaton: its
    cost does not grow with depth, and a pan re-walks only the last few
    steps.
  - Exact for affine flames with suitable (e.g. algebraic) ratios.
  - For nonlinear flames, deep cylinders are close to affine, so the
    neighbour maps converge (bounded distortion). The patterns are then
    approximate, and completeness needs a margin.
- **Measure first:** along the walk for the corpus flames, count the
  distinct neighbour maps up to a tolerance as depth grows. If the count
  levels off, an automaton planner is feasible. If it keeps growing, it
  is not.
- **Cheaper, and sure to help:** P1, the per-word cache.
- **From the render itself:** under targeting, the compute shader knows
  each deposited sample's word, and PathMap already records it per pixel
  (`path_ids`). The old 4-bit history of the free orbit is gone. A census
  of those words per region of the view says which longer words land in
  a zoomed-in sub-view, and how often. It is statistical, so on its own
  it cannot rule out holes; it can seed the walk, or order it.

### P3. julian-disc plans are large and inefficient -- open

Complete, but they force many more words than the view needs. C3's
hidden pieces made them less efficient: 80k words at efficiency 0.14
for the 0.6 off 2 view, where it was 0.19 (2026-09-25). The pieces
are carried as clouds with nothing landing yet, down to the floor, and
forced there.

`cylinder` (C6, stage 2) is the same case, more so: a julian beside a
cylinder of two turns plans at efficiency 0.09-0.43 at 1e2-1e3, and one
of pre-scale 6 (stretching `x` by 3) at 0.02-0.07, with 40-70k words at
1e3 and 430k at 1e6 -- correct (overlap 1.000), and forced mostly for
nothing.

**Not reproducible (found 2026-09-26) -- explained (2026-09-27).** The
same build planned julian-disc's 1e8 view (`what_the_references_hold`)
at 41,485 words one run and 43,649 the next. It is the time budget: the
walk forces its frontier when 20 s have passed (`TIME_BUDGET`), at
whatever level it is on, and julian-disc's 1e6 and 1e8 views take 21-22
s (C10 found the same with cylinder-turns-julian). Two runs today gave
the same plans, hashed, because the level boundaries fall well clear of
the 20 s; under load or on a slower machine they would not. Plans that
finish inside the budget are identical across processes (1e4: 17.3 s,
the same hash twice). A plan that is a function of the flame and the
view would need a budget of work rather than of time -- and one set
below 20 s of this machine's work cuts plans earlier than now. Not
decided.

**Where the 1e6 plan's waste is** (2026-09-27): 114 words forced
unmeasured when time ran out hold 58% of its mass (69 words, 67%, at
1e8; 49 words, 6%, at 1e4, where the walk reaches the depth cap of 96).
They land nothing measurable, and the plan's efficiency is 0.156 (0.034
at 1e8) for it. A walk that got further in the same time would refine
them.

### P4. Keep-or-carry on the GPU -- open (optional)

GPU plans take 80-130 ms. Most of the GPU's work is checks for children
that end up kept anyway. Deciding keep-or-carry between the replay and
the check in the same submission, and returning counts rather than a
byte per point, would cut that ([gpu-cylinder-planning.md](gpu-cylinder-planning.md) §16).

### P5. The first-plan frame in the browser -- open

One frame of 38-50 ms per flame per session: the GPU process compiles
the planner's six kernels synchronously
([gpu-cylinder-planning.md](gpu-cylinder-planning.md) §17). Options:

- Async pipeline creation: the real fix. It needs a wgpu patch or upgrade.
- Merge Plan Eval and Plan Eval Gathered: less compile time, probably not
  a shorter frame.
- Compile the planner's kernels when the flame loads.

### P6. The GPU planner in offsets -- plan, for a decision (2026-09-27)

C1's leftover, and the lever P3's measurements point to. Past
`GPU_PLAN_RADIUS` the planner's absolute f32 is as displaced as the plain
replay (~4e-7 units), so a view that deep is planned on the CPU. Measured
on julian-disc at the gate's point (12 threads, GTX 1660 Super):

| zoom | CPU | GPU planner | GPU resolves |
|---|---|---|---|
| 1e2 | 15.6 s | 2.2 s | yes |
| 1e4 | 16.8 s | 2.6 s | yes |
| 1e6 | 21.8 s, out of time | 4.7 s | yes |
| 1e7 | 21.0 s, out of time | -- | no |
| 1e8 | 21.8 s, out of time | -- | no |

On the CPU, 84% is replays: 30-45M points a plan through words of 60-96
symbols, ~67 ns a map step on twelve threads, bound by the kernels'
transcendentals. Past the GPU's reach every julian-disc plan runs out of
time, so it is also where P3's irreproducible plans and forced-frontier
waste come from.

**What it would take.** The planner answers one question in batches:
does word `w` send sample point `x` into the view. In offsets, as the
render's `ct_offsets` does: `w` runs absolutely to `m`, then carries its
offset from a reference's bases, and the test is on the reference's end
plus the offset. So each job needs a reference, computed on the CPU in
f64 before the batch -- one orbit per child against a hundred GPU replays
of it, ~1 s a plan on twelve threads -- and the batch carries the words'
blocks as the replay table does.

**The question to settle first.** A reference covers one piece of a
word's region, and a replay's points come from the whole attractor. A
point in a piece no reference reaches is tested as absolutely as today:
wrong near the view's edge at that depth. For the replays that only
moves an efficiency estimate. For the exact checks (a candidate's
landing, which decides the region and so completeness) it is a hole. So
either every piece gets a reference (C11's tail, where forced words'
pieces outnumber their seeds, is the same problem), or the checks stay on
the CPU and only the replays -- 84% of the time -- move to the GPU in
offsets. The second is the smaller, safer step.

---

## Editing by words

### E1. Trim, removals and the Paths panel -- done (2026-09-24)

Plan: [word-editing.md](word-editing.md). A trim slider drops the minor
branches of a plan's word tree that flicker in during an animation; a
Words panel removes branches by hand, replacing the Path Editor.

- **Phase 1, trim: done.** At trim 0.05, two levels from the view, the
  flicker in the Grand Julian animation frames goes (all 274 of its words)
  and the other frame's render is bit-identical. Unmeasured words
  (efficiency 0, such as the renewal glow) go with their branch and are
  never ranked as slivers.
- **Phase 2, removals: done.** `FractalConfig::word_removals`. The walk
  never makes a word ending with a removed pattern, and refines a word
  that holds one. A removal holds from zoom 5.5 to 1408 on the Grand
  Julian frame, with nothing kept whole.
- **Phase 3, the Words panel: done.** The plan's tree with each
  branch's share of the view, plus remove, solo and restore, and the
  trim slider. The Path Editor and its GPU path filters are removed.
- **Named for users** ([word-editing.md](word-editing.md) §9): the UI
  says Focused Rendering, paths and the Paths panel. The code and docs
  keep the math names.
  - An Off / Auto / Always switch lets the Paths panel work at any zoom.
  - Opening a path splits it in the plan (`PlanOptions::refine`).
- **PathMap colours by path** (§10): exact per sample. Path, Path
  (distinct), Depth and Origin; a right-click shows a pixel's path, with
  Remove.
- Phase 4 (trim hysteresis across frames, hover highlight): parked. An
  animation tested on 2026-09-24 behaved as it should without it.

### E2. 3D Focused Rendering -- idea (2026-09-24)

Focused Rendering, and with it the Paths panel and PathMap, is 2D only:
the plan asks whether a word's image meets a disc in the xy plane. The
user's thought: 3D could flatten into 2D screen space, planning in
projected coordinates. Open questions include:

- A projected map is not an IFS in the plane, since what a point
  projects to depends on its z.
- The camera moves.

## Other

- **O1. Other platforms -- later.** The planner's kernel has never run on
  Metal, where fast-math has broken our shaders before (see CLAUDE.md).
  Firefox has not been run.
- **O2. Merge `ifs-distance` into `main`** -- when you decide.
- **O3. A CLAUDE.md entry** pointing here and to the design docs.
- **O4. Small fixes.**
  - The new UI strings (Paths, Focused Rendering, PathMap) are English
    only.
  - The Simulation panel's step readout uses `→`, which egui's font
    cannot draw (an empty box).
  - `a_standby_plan_covers_a_move` asserts a 100 ms swap, which fails
    when many GPU tests share the GPU.
  - `the_web_plan_job_plans_a_slice_a_frame` asserts no `sync_cylinders`
    past 16 ms, which times the whole call and not only the plan's
    slice: 13.7-20.2 ms over three runs of the same build (2026-09-25),
    so it fails about one run in three -- and on 2026-09-26 it failed
    three runs in three on the committed build (16.6-17.7 ms) and on the
    conditional draw's (18.0-18.4 ms), whose flame has no blur word.
    `a_web_plan_takes_a_slice_a_frame` times the slices alone and is
    steady.
  - `Backward::pieces` is unused.
  - `pie` and `pie3D`'s rotation is an Angle parameter, shown in degrees
    with a 0-360 slider, but the shader adds it in radians, as JWF does
    (`pie_rotation` is radians in a `.flame`). The label is wrong, not
    the render.

## Done

- 2026-09-23: phase 3 (plans on the web), phase 4 (gathers on the GPU),
  and the crash when a web plan was dropped mid-flight
  ([gpu-cylinder-planning.md](gpu-cylinder-planning.md) §16-§17).
