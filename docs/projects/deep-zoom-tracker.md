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

### C2b. The blob's efficiency at depth -- partly done (2026-09-24)

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

**Still open: the conditional draw.** A blur word's samples are still
blobs drawn whole, and only the part that reaches the view lands. A view
inside the blob, where the glow is the whole picture, wastes what lands
elsewhere, and the draw rate cannot help there: every word is a blur
word.

To land every sample, draw the blur already inside the word's region:
- Pick a point in the region the rest of the word pulls the view back
  to.
- Solve for the blur that reaches it, through bubble's two-branch
  inverse.
- Weight the sample by the blur's density there over the draw's.

The weight is no longer an obstacle: the deposit carries one (above).
What remains is the region, the inverse, and the density. That is
research, not a transcription.

### C2c. Blurs the walk refuses -- in progress (2026-09-25)

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

**3. A nonlinear final transform -- open.** The analysis wants the
final affine, and the walk refuses a final at all.

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
    so it fails about one run in three. `a_web_plan_takes_a_slice_a_frame`
    times the slices alone and is steady.
  - `Backward::pieces` is unused.
  - `pie` and `pie3D`'s rotation is an Angle parameter, shown in degrees
    with a 0-360 slider, but the shader adds it in radians, as JWF does
    (`pie_rotation` is radians in a `.flame`). The label is wrong, not
    the render.

## Done

- 2026-09-23: phase 3 (plans on the web), phase 4 (gathers on the GPU),
  and the crash when a web plan was dropped mid-flight
  ([gpu-cylinder-planning.md](gpu-cylinder-planning.md) §16-§17).
