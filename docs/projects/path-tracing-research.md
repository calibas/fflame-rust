# Path tracing: research and plan

Researched 2026-10-19 at the user's request -- "anything to speed up or
improve the quality of Path Tracing is something we should plan on doing
... the denoiser speeds up rendering, but can also make areas look a little
odd and sometimes speckled ... I'd like to research this more before we
make any major decisions that rewrite Path Tracing." **Research and plan
only**: nothing here is built except where a line says so. The tracer as
it is: [TERRAIN.md](../main/TERRAIN.md), *The path tracer*; its speed so far:
[escape-sim-followups.md](escape-sim-followups.md), *the solid path
tracer's speed*. Weighed against the Mandelbulb as well as the Menger: the
sponge's flat faces hide what the filter and the materials do on a curve.

## What is wrong now (read from the code)

1. **The denoiser's guide is noisy where there is a highlight or a coat.**
   The share of a pixel's light the albedo does not shape (`first_white`
   in `pt_path`, `path_core.rs`) is built from the light's *sampled* cone
   direction and the shadow ray's visibility, so it varies from sample to
   sample; the filter divides the light by that guide and multiplies it
   back, so the speckle returns on glossy and highlit areas. A guide must
   be noise-free (OIDN's "clean aux").
2. **The variance is underestimated at few samples.** A pixel whose few
   samples all missed a rare bright path has near-zero variance, so the
   luminance stop closes on it and it keeps its value: speckle. The
   variance is blurred 3x3, too little; SVGF's per-pixel moments assume a
   long temporal history, which one accumulation does not have.
3. **The normal stop is cos^128** (a 10-degree difference weighs 0.14,
   20 degrees 4e-4). On the Menger's flat faces the filter runs at full
   width and the stride-8 and -16 taps leave mid-frequency residue --
   **blotches**; on the Mandelbulb the normals differ pixel to pixel, the
   filter all but stops -- **speckle**. That pair is the user's "odd areas,
   sometimes speckled", and why the Menger alone misleads.
4. **The firefly clamp is on the whole path** (10x the brightest light):
   direct light counts against it, so a bright highlight is darkened with
   the fireflies.
5. **No multiple importance sampling, and a light cannot be hit**: a
   bounce that escapes sees only the sky. Below roughness about 0.15 the
   coat's peak is narrower than a light's cone (2.4 degrees at Shadow
   sharpness 12), next-event estimation rarely lands in it -- speckle --
   and the clamp darkens what it does find. Polished surfaces never
   reflect the lights.
6. **Why gloss reads black and white:** the coat's reflectance is one
   number (white reflections); at gloss toward 1 the diffuse share goes to
   0 and the colour with it; the environment is uniform -- the background
   colour, near black on the usual backdrop -- so a reflection is near
   black; lights are not in reflections (5); and Linear with Clip clips
   the highlights. On the Menger each flat face reflects one direction of
   a uniform sky: one flat tone.

## Ranked: what to do, by expected value for this tracer

| # | Item | Effect | Cost | Main risk |
|---|---|---|---|---|
| 1 | Denoiser fixes D1-D4 (below) | less speckle and blotching at 4-64 samples | S each | re-tuning `DENOISE_SIGMA` / `DENOISE_FULL` |
| 2 | MIS, metallic, energy compensation (P4) | polished and metal looks converge; lights in reflections | S-M | keeping the lit tier in step |
| 3 | A structured environment, larger lights, AgX (P4) | the "Blender look" | S-M (procedural) | an HDRI needs a decision |
| 4 | Measure SIMD efficiency, then a fused march loop with lane refill | 1.3-2x a sample if efficiency is 50% or less | S to measure, M-L to build | registers; the invariance tests |
| 5 | Adaptive sampling (half sums) | 1.5-3x to a given quality | M | per-pixel counts change the resolve |
| 6 | Specular and diffuse denoised apart | reflections not smeared, once metals exist | M | bytes a pixel under the binding cap |
| 7 | Blue-noise error across pixels | quality to the eye; helps the filter at 1-16 samples | S-M | RMSE will not show it |
| 8 | A learned denoiser (OIDN) for stills | the best at 4-64 samples | M (desktop, CPU) / L (WGSL) | a dependency, weights, the web |
| 9 | Russian roulette on shadow rays past the first surface | 5-10% | S | some noise |
| 10 | A wavefront rewrite | about as 4, at far more cost | L | storage-buffer limits, the scheduler |
| 11 | A coarse distance-cache grid | unknown | L | measure first |

### Denoising

- **D1. Clean guides (S).** The white share from deterministic terms only:
  the light's centre direction, no visibility -- or the coat's
  pre-integrated reflectance (the split-sum DFG term, Karis 2013). Gate:
  the field-reported terrain sun-glint banding stays fixed.
- **D2. Variance (S).** Until about 32 samples, the larger of the spatial
  estimate (the existing few-sample 7x7 branch) and the per-pixel moments,
  blending to the moments as samples gather; a 5x5 prefilter.
- **D3. Outliers before the filter (S).** Split the clamp: the first
  surface's direct light high or unclamped, the indirect lower (Cycles'
  Clamp Direct / Clamp Indirect). Path regularisation: after a diffuse
  bounce, the coat at least roughness 0.3-0.5 (Kaplanyan and Dachsbacher
  2013; Cycles' Filter Glossy). Below about 16 samples, a pixel's
  demodulated light clamped to its 3x3 neighbours' maximum. (Zirr et al.
  2018's re-weighting is the robust version, at three to six more vec4 a
  pixel: stills only.)
- **D4. Edge stops for fractals (S).** The guide holds the summed normal;
  |sum n| / coverage below 1 means the normals vary inside the pixel --
  there the exponent drops (128 |n|^8, at least 8) and the distance stop
  carries it. Strides 16, then 8, dropped as samples pass about 8; and
  each pixel blended back to its raw value once its standard error is
  under about 1%.
- **D5. Specular and diffuse apart (M)** (SVGF; NVIDIA NRD's ReLAX): two
  sums by the first lobe, the diffuse divided by the albedo, the specular
  by its pre-integrated reflectance, the specular filtered by roughness
  (little or none when polished). Packed into the existing bindings: a
  browser's 128 MB floor allows about 60 bytes a pixel at 1080p.
- **D6. A stronger filter.** A solid's sample takes seconds and the
  filter milliseconds, so a costlier filter is affordable: first-order
  feature regression (Bitterli et al. 2016) or BMFR (Koskela et al. 2019)
  in WGSL; or OIDN -- on the desktop through Intel's library (built from
  its official release, never a third-party binary), roughly a second at
  1080p on the CPU (unmeasured); in WGSL, the MIT-licensed pmndrs/denoiser
  runs OIDN's Apache-2.0 weights, validated only on Apple GPUs. A learned
  filter smooths detail its albedo and normal buffers do not hold, and its
  weights are megabytes (the web would fetch them when asked).

### Variance reduction

- **V1. MIS (S-M; Veach and Guibas 1995).** A bounce that escapes inside a
  light's cone (and short of a point light) adds the light's radiance,
  each estimate weighted by the power heuristic. Nearly free for
  directional lights: the bounce is the shadow ray. The Blinn-Phong
  highlight stays out (it is not a sampled lobe). Part of P4. **Built
  2026-10-19** for the coat's lobe (TERRAIN.md); it also undid most of
  item 5's clamp darkening (0.068 of a true 0.42 before, 0.376 after).
- **V2. Adaptive sampling (M; as Cycles).** Odd samples also summed apart;
  the error |A - B| / sqrt(luminance) (Dammertz et al. 2010); a pixel stops
  after a minimum count and an unconverged one wakes its neighbours;
  active pixels compacted so a band does not run masked lanes; per-pixel
  sample indices keep it batch-invariant. The half sums also feed D2/D4.
- **V3. Blue noise (S-M).** Ahmed and Wonka 2020: pixels drawing one
  Owen-scrambled Sobol sequence in a scrambled Z-order -- no tables,
  progressive and adaptive. At 1-16 samples the error becomes blue noise,
  which the filter's fine levels remove.
- **V4. Shadow rays past the first surface (S).** Russian roulette on them
  rather than one light by power (measured 2026-10-19: half the error
  again, rejected); or resampled by unshadowed contribution (Talbot 2005).

### Speed (the march loop is about 90% of a sample)

- **S1. Measure first (S).** Distance calls a lane per march phase (the
  camera ray; a march a light and the bounce, per bounce), and SIMD
  efficiency -- the lanes' steps over 32 times each group's longest -- on
  the Menger, the Mandelbulb and the Mandelbox. `subgroupMax` where the
  device has subgroups, else a debug buffer summed on the CPU.
- **S2. One fused march loop with lane refill (M-L).** `pt_path` as a state
  machine, an iteration one distance step of the lane's current ray,
  shading on the transitions (Aila and Laine 2009's persistent threads;
  path regeneration, van Antwerpen 2011, Novák et al. 2010): a warp's time
  is its lanes' longest total rather than the sum of each phase's longest;
  threads pulling a band's pixels from an atomic counter cut the
  dispatch's tail. One kernel, no new storage. 1.3-2x if S1 finds
  efficiency at or under 50%, little if over 75%. Risks: registers (unused
  code alone cost a fifth, `4a8671ae`), the random draws' order a lane.
- **S3. Wavefront (L; Laine, Karras and Aila 2013).** Its gains came from
  material divergence and register pressure; ours is divergence in march
  lengths, which a wavefront march only fixes by refilling lanes -- S2 plus
  queues, some 200 MB of path state at 1080p, indirect dispatch, and the
  browsers' storage-buffer count. Only if S2's registers prove fatal.
- **S4. More compiled in or out (S):** the coat or metal, MIS, the shadow
  strength's mix, the bounce and light counts -- as `placed_lights` is.
- **S5. A distance cache (L, speculative):** a coarse 3D texture of
  conservative distances; only if a step histogram shows many far-field
  steps (the sponge's tunnels and grazing shadow rays are near-field).

### Not recommended

- **Path guiding, ReSTIR:** they pay with many bounces and many lights;
  here two bounces, at most four lights, a sky.
- **Over-relaxed marching:** measured slower (2026-10-19), and unsafe on
  the Mandelbulb, whose estimate is not a strict bound.
- **Segment tracing:** needs Lipschitz bounds the fractals do not give.
- **Cone or bundled shadow rays:** jittered light directions and scattered
  bounce origins leave no coherence.
- **Temporal reprojection for solids:** a sample takes seconds (worth it
  for terrains only).

## Gates for any of it

`path_quality_against_a_reference` (a seed of its own for the reference)
with `PT_PRESET` the Mandelbulb and the Mandelbox as well as the Menger
Sponge, and gloss/metal variants (roughness 0.1 and 0.4, metallic 0 and 1):
RMSE and bias at **equal time** -- the honest axis for MIS and adaptive
sampling; the denoised error at 4, 16 and 64 samples, and FLIP (Andersson
et al. 2020), since RMSE under-weights blotches and speckle; flat and
curved pixels measured apart. Gloss 0 and metallic 0 bit-identical;
`solid_samples_are_band_invariant` and the batch-invariance tests hold.

## Proposed order, for the user's decision

1. Within P4 (materials, [3d-look-and-tools.md](3d-look-and-tools.md)):
   V1 MIS, metallic, energy compensation, the environment, AgX -- needed
   for the polished look, small, and gated as above.
2. **Decide:** the denoiser fixes D1-D4 (small, each measured), then D5
   once metals exist.
3. **Decide:** S1's measurement (cheap), and S2 only if it finds the
   lanes idle half the time or more.
4. **Decide:** V2 adaptive sampling.
5. **Decide:** a learned denoiser for stills (D6), and an HDRI environment
   (reopens 2026-10-02's decision to skip image files).

## Agreed 2026-10-20: order of work

The user: "Yes" to 2, 3 and 4 above (item 1 was built within P4). In
this order, each change measured before it lands:

0. [ ] **The gate:** `denoise_quality` (`solid_path.rs`, ignored): the
   Menger Sponge, the Mandelbulb and the Mandelbox, each matte, glossy
   (gloss 0.04, roughness 0.15) and metal (metallic 1, roughness 0.4), at
   4, 16 and 64 samples, raw and denoised, against 1024 undenoised samples
   from a stream of their own (cached in `PT_REF_DIR`). The luminance
   error over covered pixels as a share of the mean, split by scale:
   **fine** (the error less its 3x3 mean: speckle) and **coarse** (its 9x9
   mean: blotches) -- in place of FLIP, the two failures the user named,
   which RMSE weighs alike.
   **What the gate found first (2026-10-20).** Every solid denoiser test
   so far -- and the tuning of `DENOISE_SIGMA` and `DENOISE_FULL` on
   solids -- ran with an all-white palette (the tests' `white_palette`),
   so the albedo guide never had a colour to separate; the gate's first
   run did too, and was redone with each preset's palette. With colour:
   - **The denoiser removes little on a fractal solid**: on average it
     left 98% of the raw error, and from 16 samples it often made the
     picture *worse* (the Mandelbox at 64 samples: 1.09% raw, 1.61%
     denoised) -- the blotches, as coarse error.
   - **Most of a coloured solid's error is the colouring's own sub-pixel
     variation**: the Mandelbulb at 4 samples is 17.8% from its reference
     white, 46% with its palette. A filter that divides out the pixel's
     mean albedo and multiplies it back leaves that untouched; filtering
     the whole colour instead (the guide white) was no better.
   - **Gloss and metal are fireflies**: 30-200% at 4 samples, 6-43% at 64.
1. [~] D1 clean guides: built; within 1% everywhere -- the guide's noise
   is not what limits the filter here. Kept (correct, and free).
2. [x] D2 the variance: built, measured, **dropped** -- a wider stop
   blurred more detail than noise (mean denoised/raw 0.982 -> 1.014; the
   Mandelbox at 16 samples 3.84% -> 4.66%).
   **In its place, a confidence bound on the filter** (`finish`): a pixel
   may move from its own mean by no more than its standard error times
   `max(1, sqrt(16 / n))`, measured on the largest channel, shrinking
   past it as (bound / gap)^2 -- so a disagreement within the noise is
   noise removed, and a larger one, the picture's detail, goes back to
   the samples' own; from 8 samples on. Mean denoised/raw 0.979 -> 0.934;
   from 16 samples the denoised picture is no worse than raw anywhere
   (the Mandelbox at 64: 1.61% -> 1.11%, its coarse error halved), and it
   converges to the samples' mean. (A hard clamp at 2 and 1 errors: 0.967,
   0.948; by luminance alone the colours still bled.)
3. [x] D3 outliers, measured against references of 4096 samples with no
   firefly clamp (`PT_REF_TRUTH`), so a change to the estimator shows its
   bias:
   - **The clamp split** (direct light at 10x the bound): **dropped** --
     less bias, more noise from the highlights at these counts (glossy
     Mandelbulb at 16 samples 62.6% -> 74.0% raw); MIS had already undone
     most of the darkening the whole-path clamp was blamed for.
   - **Path regularisation** (a coat past a diffuse bounce at least
     roughness 0.4, `PT_REGULAR_ROUGH`): kept -- glossy 1-4% better, nothing
     worse (a metal never bounces diffusely).
   - **The neighbourhood clamp** (`prepare`): a pixel's light no brighter
     than twice its brightest neighbour's on the same surface (normals
     within 37 degrees), loosening in proportion past 16 samples so it
     fades rather than switching off. Kept: glossy at 4 samples 76% (the
     Mandelbox), 32% (the Menger) and 21% (the Mandelbulb) less error,
     metal 13-17%; at its neighbours' maximum itself it darkened a metal's
     real glints (the Mandelbox metal +33%), and held at 2x to every count
     it biased one at 64 samples (+16%).
4. [x] D4 edge stops:
   - **The normals' stop eased where a pixel holds several facets**
     (their mean's length; a fourth texture): **dropped** -- neutral (mean
     0.866 -> 0.865), a little less speckle for a little more blotch.
   - **The widest strides dropped as samples gather** (16 from 8 samples,
     8 from 32; `denoise_steps`): kept -- neutral in error, two passes of
     five fewer.
   - **Back to the raw value once converged**: superseded by the
     confidence bound (2. above).

   **D1-D4 together, 2026-10-20** (both against the 4096-sample truth):
   the denoised picture's error as a share of the raw's, averaged over
   the 27 cases, **0.986 -> 0.866**; the denoised picture now worse than
   raw nowhere from 16 samples (it was in 9 of 18 cases). Kept: D1, the
   confidence bound, path regularisation, the neighbourhood clamp, the
   stride schedule.
5. [x] **S1 measured 2026-10-20** (`march_lane_efficiency`: one sample
   at 320x180, the shader patched to count each lane's distance steps by
   phase; a warp an 8x4 block). The share of a warp's lanes busy, and what
   S2 could gain at best:

   | Solid | busy now | fused alone | fused, with refill |
   |---|---|---|---|
   | Menger Sponge | 25% | 1.53x | 3.97x |
   | Mandelbulb | 20% | 1.33x | 4.88x |
   | Mandelbox | 50% | 1.09x | 1.99x |

   The camera ray keeps 58-80% of the lanes busy; a bounce's march
   15-29%, the first light's shadow 25-41% -- only some lanes have one
   to trace, and the rest wait. Past the agreed bar (idle half the time or
   more) on two of three, so **S2 goes ahead**, with refill: fusing alone
   is worth little. Steps are distance calls, not their cost (a fractal's
   iteration count varies within each), so the gains are ceilings.
   [x] **S2's design** (built, measured slower, parked -- below). Mode D's solids only (a terrain's trace is a
   traversal of sections, not a distance march, and keeps `pt_path`):
   - **One loop, one distance step an iteration**, of whichever ray the
     lane holds -- the camera ray, a light's shadow ray, a bounce -- with
     the shading on the transitions: a hit's surface, each light's
     contribution (seen and unseen, added when its shadow ray ends), the
     bounce's lobe. The draws in the order `pt_path` makes them, so a
     sample's random numbers, and so its value, do not change.
   - **Refill within the workgroup**: a workgroup takes a tile of several
     pixels a lane, and its lanes pull pixels from a `var<workgroup>`
     atomic counter as they finish -- no new storage binding (the solid's
     path tracer is near a browser's eight), the bands and their pricing
     as they are. A lane owns a pixel until its samples are in, so the sum
     is written without atomics, and in sample order.
   - **Gates**: the picture to the last bit or within f32 reordering
     against the current tracer (`solid_samples_are_band_invariant`, the
     tier tests); the time a sample on the three solids at 320x180 and
     1080p (`path_quality_against_a_reference`).

   **S2 built and measured 2026-10-20 -- slower; parked.** The fused
   march with refill was built as designed (`main_fused`, beside `main`)
   and gave the same picture: identical to the bit on a flame cube, a
   metal Mandelbox and a band split in tiles, and to the last bits on a
   glossy Mandelbulb. But a sample took **2.6-3.5x longer at 1080p**
   (the Mandelbulb 46 -> 162 ms, the Mandelbox 21 -> 55) and up to 25x at
   320x180, and holding less state (the shading recomputed, the sums in
   memory) made it slower still (184 ms). Why, as far as measured:
   - **Tiles cost parallelism**: 512 pixels a workgroup gave a 320x180
     frame 113 workgroups, a third of what the GPU holds; the Menger's
     8-row bands got 5 where `main` dispatched 40 -- the 25x.
   - **The surfaces serialise**: a hit costs a normal (six distance calls)
     and a colouring's walk. In `main` the lanes that hit in a phase
     evaluate theirs together; in the fused loop lanes hit on different
     iterations, and the warp evaluates one lane's surface at a time.
     S1 counted only march steps (6-8 a pixel); the surfaces are about as
     much work again, so its ceilings (4-5x) were too high.
   A version that could win would take the normal's six calls as steps of
   the one loop and batch the colourings' walks, and size its tiles to the
   band -- a deeper rewrite of the tracer, with the gain now uncertain:
   **for the user's decision**, not built. Kept from S2: `pt_path` on shared
   pieces (`pt_shade`, `pt_light`, `pt_light_add`, `pt_bounce`,
   `pt_escape`, `pt_guide_white`), the same pictures to the bit (a solid,
   a metal, a terrain, a flame's solid), the same speed within noise.
6. [ ] V2 adaptive sampling.
7. [ ] D5 specular and diffuse denoised apart (metals exist now).

Sources: Blender 4.0 shading release notes; Veach and Guibas 1995; Dammertz
et al. 2010; Kaplanyan and Dachsbacher 2013; Karis 2013, 2014; Kulla and
Conty 2017; Turquin 2019; Zirr et al. 2018; Bitterli et al. 2016; Koskela
et al. 2019; Ahmed and Wonka 2020; Talbot 2005; Aila and Laine 2009; van
Antwerpen 2011; Novák et al. 2010; Laine, Karras and Aila 2013; Andersson et
al. 2020 (FLIP); Tokuyoshi and Kaplanyan 2019; pmndrs/denoiser; Intel Open
Image Denoise.
