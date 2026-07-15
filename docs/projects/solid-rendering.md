# Solid Rendering: occlusion, lighting, and shading for 3D flames

**Status**: Phase 1 COMPLETE — Phase 2 CORE COMPLETE: density-volume splat + shade-pass consumption (∇ρ normals, volumetric AO, shadow march, volume-authoritative occlusion repair). Remaining: translucent volumetric mode, perf tuning of the per-pixel ray march
**Motivation**: 3D flames render as additive transparent ghosts. Shapes have no
occlusion (far structure bleeds through near structure), and geometry becomes
invisible wherever palette colors are uniform, because *all* shape information
currently arrives through color and density. This adds true occlusion plus
geometry-derived shading (lighting, SSAO), independent of palette.
**Prior discussion**: `docs/projects/3d-volume-surface-planning.md` (option
families). This doc is the implementation plan for the chosen architecture.

## Decisions (agreed 2026-07-11)

1. **JWF solid mode is the reference, not the ceiling.** We adopt JWF's
   *parameter semantics* where import compat matters (per-xform
   `material`/`material_speed`, light lists, ambient/diffuse/specular
   coefficients) but not its implementation. Where we can do better (volume
   normals, volumetric AO/shadows in Phase 2), we do.
2. **Solidity is blendable, not binary** — `solid_strength: f32` (0 = today's
   additive transparency, 1 = hard surface), implemented through the existing
   per-sample `density_weight` channel, which already flows through every
   accumulation path (see "Why blendable is nearly free").
3. **No image textures yet** (no matcaps/env maps in scope; procedural
   lighting only). Architecture leaves the door open — a matcap is one
   normal-indexed texture lookup in the shade pass when we want it.
4. **WASM is budgeted from day one.** Phases 0–1 are WASM-clean by
   construction (one buffer region + core-WebGPU passes). Phase 2's voxel
   grid auto-scales to a memory budget.
5. **Hard requirement — zero cost when disabled.** All solid-rendering code
   is gated by the shader builder (a SOLID template flag, like HAS_W): a
   flame with `solid_strength == 0` compiles a **byte-identical shader** to
   today's, allocates no extra depth region reads/writes in the hot loop,
   and skips the shade pass entirely. Enforced by a unit test asserting
   byte-identity of the emitted WGSL with solid off.

## Architecture overview

Three phases sharing one architecture. Phase 0 adds *occlusion* (a per-pixel
nearest-depth channel + splat gating). Phase 1 adds *shading* (a deferred
pass between accumulate and tonemap). Phase 2 upgrades shading quality with a
world-space density volume. Nothing in an earlier phase is thrown away later.

```
compute pass (chaos game)
  ├─ histogram RGBD atomics            [existing]
  └─ depth region: atomicMin(u32(d))   [Phase 0 — same buffer, new region]
accumulate pass → accumulator texture  [existing, untouched]
shade pass                              [Phase 1 — new]
  inputs:  accumulator (albedo+density), depth region, (Phase 2: voxel grid)
  work:    reconstruct position, normals, à-trous smooth, lights, SSAO
  output:  Rgba16Float shaded texture
tonemap ← via existing tonemap_pass_with_input plumbing
density/color effects                  [existing, unchanged]
```

### Key integration facts (from the 2026-07-11 pipeline survey)

- Camera-space z is computed **four separate times** at the splat site
  (depth-density `main_template.wgsl:524`, DoF `:543`, far-fade `:742`, fog
  `:759`) and discarded. Phase 0 starts by computing `camera_space` **once**
  and reusing it — a standalone cleanup.
- The main compute pass already uses ~10 storage buffers (spec floor is 8;
  `device.rs:156-165` raises the limit). **The depth channel must not add a
  binding**: extend the histogram buffer allocation from `W*H*4` to `W*H*5`
  u32 and index the depth region at offset `W*H*4`. Same binding, +25%
  histogram memory (~8 MB extra at 1080p).
- The tiled high-res export's `Sample` struct has **two spare trailing
  floats** (`main_template.wgsl:835`) — depth rides in one of them on the
  sample-emit path.
- `tonemap_pass_with_input` (`compute_kernel.rs:914-978`) already lets a
  pre-tonemap pass substitute the accumulator as tonemap input (density
  effects use it). The shade pass plugs in there; when solid rendering is
  off, the pass is skipped entirely and the pipeline is byte-identical to
  today.
- The effects chain's `Rgba16Float` ping-pong pair (`effect_chain.rs:346`)
  is reusable scratch for the à-trous normal-smoothing iterations.
- There is **no existing denoiser** suitable for depth/normals (the planning
  doc's "your à-trous denoising" was aspirational). The depth-guided à-trous
  filter is a Phase 1 work item.

## The hard problem: occlusion under progressive rendering

Opacity means *rejecting* (or down-weighting) splats behind the surface — but
the surface (per-pixel nearest depth) is only known from samples seen so far.

**Scheme**: gate every splat against depth-so-far.

- Depth encoding: `d = -camera_space.z` (positive into the screen; behind-
  camera samples are already clipped by the `zr < 1e-3` projection test).
  Positive IEEE f32 bit patterns are monotone, so `atomicMin` on
  `bitcast<u32>(d)` is a correct nearest-depth.
- Gate: a sample contributes color at full weight iff
  `d <= d_nearest + surface_thickness`; otherwise its `density_weight` is
  multiplied by `(1 - solid_strength)`.
- Races are benign: `d_nearest` only decreases, so a stale read over-accepts
  a few samples early in the frame; the gate tightens as accumulation runs.
- **Depth priming**: after any full reset, the first batch splats depth only
  (color suppressed) so the accumulator never ingests interior ghosting from
  a not-yet-converged depth buffer. One params flag, set automatically by the
  renderer on reset. Without priming the EMA/cumulative blend would still
  converge — priming just removes the visible transient.

### Why blendable solidity is nearly free

Depth-density compensation and far-density fade already multiply a per-sample
`density_weight` that is honored by **all four** accumulation paths (direct
histogram, sample-emit, tiled, CPU). `solid_strength` is one more multiplier
on the same channel: 1.0 skips the splat outright (hard surface, fastest),
0.0 is a no-op (today's translucent), anything between renders a
translucent-solid mix. No new blending machinery.

### Reset / overwrite semantics

- The depth region is part of the histogram buffer, so `clear_all` and the
  `reset_iteration_counter*` family clear it with no extra plumbing. Clear
  value must be `0xFFFFFFFF` (= +inf ordering), not zero — the histogram
  clear currently zeroes; the depth region needs a dedicated fill (small
  compute or `fill_buffer` with ones at the region offset).
- Overwrite mode (interactive parameter drag) re-clears the histogram per
  frame; the depth region follows automatically. Depth is then single-batch —
  consistent with what's displayed.
- Depth priming triggers only on full resets, not overwrite frames.

### Interactions to handle in Phase 0

- **DoF**: at-splat DoF jitters the splat *position*, which would corrupt
  nearest-depth. In solid mode (solid_strength > 0), at-splat DoF is skipped;
  post-DoF from the depth buffer is a Phase 3 item (and is better DoF).
- **Post-symmetry**: each symmetric copy has its own pixel and its own
  camera-space z — gate each copy independently (depth work sits inside the
  existing symmetry loop).
- **High-res export**: the direct path's 128 MB threshold math
  (`TARGET_BUFFER_SIZE`, `high_res.rs:194`) must account for the 5/4 buffer
  growth. The tiled path carries depth in the spare `Sample` float; the tile
  accumulate does the min + gating per tile. The CPU-histogram path does the
  same on the CPU. Export is batch-iterative like interactive rendering, so
  progressive gating behaves identically.

## Phase 0 — occlusion foundation

Deliverable: opaque (or partially opaque) fractals with correct silhouettes
and inter-shape occlusion. No lighting yet. This alone fixes "transparent
ghost".

- [x] Refactor: compute `camera_space` once at the splat site (`33514ea`) —
      `project_3d_full` in utilities.wgsl returns {pixel, camera_space}; all
      4 consumers rewritten. Verified pixel-hash-identical against
      deterministic baselines (`output/solid-baselines/`, both the
      all-four-effects scene and plain 3D).
- [x] Histogram buffer 4→5 u32/px when solid (`FlameBuffers::
      set_solid_depth_region`, zero new bindings; scratch stays 4/px).
      DESIGN CHANGE vs the plan: depth is encoded as INVERTED ordered-float
      bits with `atomicMax` (larger = nearer, 0 = "no sample"), so plain
      zero-clears initialize the region — no fill-to-ones pipeline at all.
      The per-batch `clear_histogram` is range-limited to the RGBD words so
      depth persists across batches.
- [x] Splat-site depth test + gating via `density_weight`; SOLID template
      flag; at-splat DoF compiled out in solid mode (post-DoF from depth is
      the Phase 3 replacement). SOLID additionally requires the
      direct-histogram output path.
- [x] Depth priming on reset (`needs_depth_prime`, armed by reset/
      load_config/resize/toggle; one depth-only batch).
- [x] Config: `solid_strength` + `surface_thickness` (FractalConfig +
      serde skip-default, ConfigPath::SolidStrength/SurfaceThickness →
      IterationReset, undo/history i18n keys).
- [x] UI: two sliders in the View panel ("Solid Rendering" section).
- [x] Sample-emit + tiled + CPU export paths: depth rides the Sample's
      spare slot; the tile scatter pass owns a per-tile depth region
      (appended to each tile's span, 5 words/px) with the same encoding,
      gating, and first-dispatch priming; the CPU accumulate path gates
      deterministically (sequential per row). Readback gathers RGBD spans
      per tile. Engine parity verified (flamerenderer vs highres forced on
      the same solid config: differences within the run-to-run noise
      envelope).
- [x] Tests: `solid_off_is_byte_identical` (byte-identity + 2D +
      sample-emit gating), `solid_depth_encoding_is_monotone`. Baselines
      re-verified pixel-identical with solid off. Visual regression
      category still TODO.
- [x] Size/threshold math is solid-aware end to end:
      `histogram_size_bytes(_, _, solid)`, `pick_strategy(..., solid)`
      (tile heights sized for 20 B/px spans + offset alignment), and both
      app routing sites.
- [x] Visual regression category `solid`: 3 scenes
      (tests/visual/configs/solid/ — hard occlusion, translucent blend,
      solid + depth effects incl. a deliberately nonzero DoF that must
      have no effect) with baselines. `solid-*` tests compare with a
      tolerance in run_benchmarks.py (mean delta <= 8/255, large-delta
      channel fraction <= 5%) instead of pixel hashes. Comparator
      validated both ways: re-render passes (mean 0.01), solid-off vs
      baseline fails loudly (mean 15.5, 19.8% big).

## Phase 1 — deferred shading

Deliverable: lit solids — normals, ambient + up to 4 lights
(directional/point, Blinn-Phong to match JWF's parameter vocabulary), SSAO,
specular. The milestone where same-palette shapes become legible.

- [x] Shade compute pass + pipeline (`shaders/shade.wgsl`,
      `src/renderer/shade_pass.rs`): wired through
      `tonemap_pass_with_input` in both the interactive frame and the CLI
      render path, ordered shade → density effects → tonemap. Not
      dispatched when `shading_strength == 0`. DESIGN CHANGE: lighting is
      decoupled from occlusion — the depth capture activates when
      solid_strength > 0 OR shading_strength > 0 (gating multiplies by
      solid_strength, a no-op at 0), so transparent flames can be lit.
- [x] Position reconstruction from depth (inverse Apophysis projection) +
      normals via a depth-bilateral 9x9 slope fit (edge-preserving window
      = 3x surface_thickness). LESSON: the depth sentinel must be
      out-of-band (3e38), NOT sign-based — geometry straddling the camera
      plane has legitimately negative depths.
- [x] Lighting: ambient + 4 camera-space directional lights (azimuth/
      elevation), Blinn-Phong diffuse+specular, SSAO (8-tap golden-angle
      spiral with range falloff), final = mix(emissive, lit,
      shading_strength). Config: nested `SolidShadingSettings` struct
      (serde unit, skip-if-default) — not a fan of flat fields.
- [ ] Position reconstruction from depth; screen-space normals from depth
      gradients.
- [x] À-trous normal smoothing: dedicated normals pass (normals.wgsl,
      extracting the bilateral slope fit) + 0-3 edge-aware à-trous
      iterations (atrous.wgsl: 5×5 B3 kernel at strides 1/2/4, weights =
      depth-gaussian × normal-similarity^8, σ_z tied to the surface
      shell) over a dedicated Rgba32Float (normal, depth) ping-pong —
      NOT the effect-chain textures (16F loses depth precision).
      `normal_smoothing` (0-3, default 1) on SolidShadingSettings, UI
      slider, ShadingOnly (live, no reset). Interactive + single-shot
      export paths use it; the strip-tiled export keeps the inline
      estimator (apron machinery not worth it at those sizes).
      Verified: the noisy sphere case at 3 iterations renders a coherent
      lit hemisphere with a clean terminator.
- [ ] Lighting model: ambient + N≤4 lights, diffuse + specular; final color
      = `mix(emissive_flame_color, lit_color, shading_strength)` — the
      emissive term preserves today's look as a blendable component.
- [ ] SSAO: depth-buffer horizon sampling, radius/strength params.
- [ ] Per-transform `material` index + material table — MOVED to Phase 2
      (materials pair naturally with the volume work; global material
      params shipped in Phase 1 cover the single-material case).
- [x] Lighting UI: View panel "Lighting" section (shading strength,
      ambient/diffuse/specular/shininess, SSAO, 4 lights with enable/
      color/azimuth/elevation/intensity). ConfigPaths: 7 flat params +
      SolidLightEnabled{index} + SolidLightParam{index, param} (mirroring
      the DensityEffectParam pattern), all -> IterationReset (smooth
      overwrite-mode transitions; a dedicated lighter UpdateType is a
      possible optimization). Undo/history + string roundtrip covered. A
      dedicated dockable Lighting panel is deferred until the control
      count grows (materials, Phase 2).
- [ ] WASM verification pass (all core WebGPU; no FLOAT32_FILTERABLE
      dependency — shade pass uses `textureLoad`).
- [ ] Visual regression scenes: single light, multi-light, SSAO-only,
      solid_strength sweep.

### Brightness renormalization (landed with Phase 1)

Hard occlusion culls most dispatched samples, but the tonemap normalizes
by dispatched iterations — solids rendered dark by exactly the culled
fraction (the Phase 0 "dark cloud" finding). Fixed by measurement: a
one-workgroup GPU reduction (`shaders/density_stats.wgsl`) sums the
accumulator's alpha (accepted density); `survival_fraction =
sum / samples_in_buffer` scales the tonemap's `sample_density`.
Interactive path: async readback every 16 frames, EMA-smoothed (a couple
frames of lag is invisible for a brightness scalar). CLI path: one exact
blocking measurement before the final tonemap. Fraction clamped to
[0.005, 1] (≤200× boost); forced to 1.0 whenever occlusion isn't culling
(transparent renders verified bit-identical). Self-calibrating for any
solid_strength — partial occlusion measures its own partial fraction.

## Phase 2 — density volume

Deliverable: world-space ∇ρ normals (stable under camera motion, no
screen-space edge artifacts), volumetric AO, shadow rays toward lights, and
an optional emission/absorption translucent mode. This is where quality
passes JWF.

- [x] Flat `array<atomic<u32>>` density grid in a world-space cube
      (user-set half-extent, `volume_extent`, default 2.5). Fixed dim:
      desktop 192³ (28 MB), WASM 128³ (8 MB) — `FlameRenderer::VOLUME_DIM`.
      Rides the main compute pass at binding 6 (the old iteration_counts
      gap); 4-byte dummy bound when off, real buffer created/dropped by
      `FlameBuffers::set_density_volume` (update_flame / load_config /
      resize rewire bind groups on change). Lifecycle mirrors the depth
      region: persists across progressive batches, cleared on full reset
      and per-frame in overwrite mode.
- [x] Splat: one extra `atomicAdd` per plotted sample (VOLUME template
      flag = volume_enabled && 3D && direct-histogram; sample-emit export
      path never compiles it). `volume_off_is_byte_identical` enforces
      zero-cost-off. Config: `solid_shading.volume_enabled/volume_extent`,
      ConfigPaths VolumeEnabled/VolumeExtent (IterationReset), View-panel
      toggle + extent slider.
- [x] Shade pass consumption (all gated on `sp.volume_dim > 0`; the
      camera→world inverse comes from `effective_camera_rows` in
      shade_pass.rs, which replicates utilities.wgsl's build_camera_matrix
      + transposed application EXACTLY — verify against it before touching
      either):
      - ∇ρ gradient normals (trilinear central differences), blended over
        the screen-space normal by edge confidence (|∇ρ| per voxel
        relative to local density) — world-space, camera-stable.
      - Volumetric AO: 5 hemisphere density taps at the SSAO radius,
        multiplied into the SSAO term under the same strength control.
      - Shadow march: 32 × 2-voxel steps toward each light; transmittance
        exp(-∫ρ) attenuates diffuse + specular. New `shadow_strength`
        config/ConfigPath (SolidShadowStrength, ShadingOnly) + UI slider
        + animation target.
      - OCCLUSION REPAIR (the "high density shows through low density"
        fix): per-pixel volume ray march (`vol_ray_depth`, 160 × 1.25-voxel
        steps, integrated-density first-hit) gives an authoritative
        surface depth. Pixels whose nearest sample sits > margin behind it
        are LEAKS — rebuilt from front-surface ring neighbors anchored to
        the volume depth (relaxed consensus, runs even at gap_fill 0);
        no-ring fallback keeps the pixel's color but moves its geometry to
        the volume surface with the gradient normal. Holes get the same
        authority (volume-backed fill instead of blind ring consensus).
      - Density normalization: `vol_density_scale` = dim³/(splats·8), so
        ρ_norm ≈ 1 ⇔ 8× the uniform-spread mean — iteration-invariant.
      - VIEW-FIT placement (default, `volume_auto_fit`): the cube centers
        on the world point at screen center and sizes to 2× the visible
        half-width (`FlameRenderer::volume_placement`), so voxels stay
        ~1/100th of the visible width at any zoom. A fixed world cube
        left a zoomed-in object spanning ~3 voxels — giant rectangular
        AO/normal facets and dead occlusion repair (field-reported on
        the grand-julian sphere). Camera/zoom/pan changes reset
        accumulation → the volume re-splats anyway, so tracking the view
        is free. Manual mode (auto-fit off) keeps the origin-centered
        `volume_extent` cube. Shadow march samples trilinearly (a
        nearest march casts voxel-shaped shadow blocks).
      - DERIVED FIELDS (volume_mip.wgsl): the shade pass never samples
        the raw splat grid — at view-fit resolution it carries per-voxel
        Poisson noise ("patchy" lighting) and cell-faceted gradients
        ("rectangular" shading blocks). Each shade derives two half-res
        fields: a 4³-window MEAN (gradient normals, AO, shadow march —
        with per-pixel jittered march phase) and a morphologically
        CLOSED max field (dilate→erode, radius = `volume_closing`, 0-2,
        ShadingOnly) for the occlusion/repair ray march — holes in
        genuinely sparse shells (julian-on-bubble) read as sealed.
        Closing INVENTS surface where the IFS measure has none; it's an
        artistic dial, documented as such. `ShadePass::ensure_vol_derived`
        allocates; `run_region` stays `&self` for the exporters and
        silently disables volume shading if unprepared.
      - REPAIR STABILITY (hardened through four field-feedback rounds;
        every rule below exists because its absence produced a specific
        visible artifact — treat them as load-bearing):
        · The march returns continuous INTEGRALS (occlusion in front of
          the pixel; opacity-clipped mean depth of the first surface
          unit), never a first-hit depth — first-hit is bistable when a
          sparse shell hovers at the trip threshold.
        · The repair weight is a SATURATING function of the occlusion
          integral (o²/(o²+0.25)) — windowed smoothsteps paint a seam
          along the iso-occlusion contour.
        · March steps are OPACITY-CLAMPED at the solid level — fractal
          density spans orders of magnitude, and one filament voxel
          (1000× "solid") otherwise casts a saturated voxel-shaped
          shadow block; clamped voxels also converge early.
        · Ring statistics are DENSITY-WEIGHTED, so a neighbor whose
          depth just flipped front grows into the ring smoothly instead
          of jumping every pixel that gathers it.
        · There is deliberately NO no-ring fallback: an occluder with no
          sampled pixels (a dense plane seen edge-on = 1-px image line)
          would get relit into bright streaks. Repair only paints what
          the ring has image evidence for.
        · `vol_trust` (host): geometric (derived voxels across the
          visible width, 1 at ~36+, 0 at ≤12) × statistical (ramps over
          the first ~12 batches; overwrite frames ≈ 0) scales every
          volume feature — coarse or thin volumes fade to the Phase 1
          look instead of stamping voxel artifacts.
        · ACCUMULATOR DEPTH-TIGHTENING RESET (accumulate.wgsl +
          accum_depth buffer): when a pixel's nearest depth moves closer
          by > 1.5× thickness, its accumulated history (provably
          occluded surface) is discarded instead of blended.
        · TEMPORAL EMA (0.85, interactive only) on the shade output —
          repaired pixels track the front shell's relative density,
          which genuinely drifts during the coverage-transition window
          (user-visible at ~0.5-1G iters on large viewports); the blend
          turns per-frame stepping into a calm drift. History resets on
          lighting edits, accumulation resets, and per video frame.
        · Volumetric AO taps are lifted 2 voxels off the shell, jittered
          per pixel, and half-weighted — surface-scale features
          otherwise darken voxel-shaped footprints.
        Verify banding fixes with CROPS at the coverage-transition
        window (~150M iters at 800×600 for the grand-julian scenes), not
        full-frame thumbnails, and check temporal stability by diffing
        deterministic renders at N vs N+10% iterations.
      - Verified on solid-lit: the inter-tile see-through gaps close,
        facets shade coherently, shadows add real depth.
- [ ] Translucent volumetric mode: short emission/absorption march composited
      with the solid term — makes solid↔volumetric a continuous artistic dial.
- [ ] High-res export: the exporter currently passes `volume: None` (the
      tiled path is sample-emit and never builds a grid; the single-shot
      direct path could — wire it when Phase 2 stabilizes). Interactive,
      CLI, and video-export paths all consume the volume.
- [ ] Perf: `vol_ray_depth` is ~160 nearest-voxel fetches per pixel per
      shade. Fine at preview sizes; consider early-out via pixel depth,
      coarse mip pre-pass, or half-res repair mask if it shows up.

### Field feedback (2026-07-12, first real-scene session)

- Sparse-coverage GAPS (e.g. julian projected on a sphere via bubble):
  see-through pinholes wherever the IFS measure is thin; worst in
  animation (temporal shimmer). First-line fix shipped: `gap_fill`
  (surface closing in the shade pass). Fundamental fixes: splat
  footprints > 1px (surface splatting), surface-sampler variations
  (quaternion_julia_set-style density generators for common shells),
  and the Phase 2 volume. Surface samplers v1 SHIPPED: `solid_sphere`
  + `bubble_solid` (defs/solid_samplers.rs) - radial thickness + a
  uniform `fill` probability give shells guaranteed baseline density,
  attacking the holes at the source.
- Surface thickness sweet spot is 0.001-0.01; above ~0.01 "ripples"
  appear — the acceptance shell is measured along the VIEW ray, so on
  slanted surfaces a thick shell cuts iso-depth contour bands whose
  accepted density varies with slope. Candidate fix: slope-adaptive
  thickness (scale the shell by the surface slant once normals exist);
  the volume makes it moot.
- Residual bright-pixel noise in rough areas after à-trous: sparse
  single-sample albedo amplified by lighting. Candidates: supersampled
  rendering (render at 2x + downsample — no user-facing supersampling
  exists today), firefly clamp against the local neighborhood in the
  shade pass.

## Phase 3 — volume-primary solid mode (IN PROGRESS)

Decision (2026-07-12, after four field-feedback rounds on Phase 2): the
depth-buffer renderer consulting the volume by rules was the inverted
architecture — every hole/repair-seam/banding artifact came from
reconciling two geometry sources with thresholds. Phase 3 makes the
volume THE renderer, per the original 3d-volume-surface-planning.md
recommendation ("voxel volume as the destination architecture").

Stage A (DONE):
- RGBA volume: the splat carries base_final_color (histogram
  fixed-point scheme, 4 u32/voxel). volume_mip's reduce also emits a
  half-res base-coat color field (density-weighted window mean).
- volume_march (shade.wgsl): front-to-back emission/absorption through
  the CLOSED field; solid_strength is the σ dial (1 = hard surface,
  lower = translucent); density opacity-clamped; per-pixel jittered
  start; returns weighted base color, coverage, surface depth, and
  mean unclamped density.
- Compositing: base coat from the march (voxels fill ~1000× faster
  than pixels — complete surface long before pixel convergence),
  per-pixel chaos-game data layered on top by a continuous detail
  confidence (accum.a vs scene-mean base_alpha), gated to samples at
  or in front of the base surface (occluded back structure can never
  show through). Lighting: ∇ρ normals + emissive fallback where the
  gradient is weak (dust), volumetric AO, opacity-clamped shadow
  march. Base-coat tonemap alpha scales with the ray's mean density
  (keeps the flame's log-density dynamic range).
- The Phase 2 repair machinery (integral leak march, ring anchoring,
  sparse hand-over) is REPLACED by this path and removed; Phase 1
  (volume off) keeps plain gap_fill + depth-buffer shading.
- Consequence users will notice: ALL density in the view volume now
  renders as solid geometry — structures the old path showed as
  near-black dust become lit volumetric surfaces. That is the real
  field made solid; artistic control via solid_strength (σ) and
  the flame itself.

Stage A follow-ups (DONE, field feedback):
- Volume splat gated on should_plot: opacity-0 / solo'd-out transforms
  no longer leave ghost geometry in the volume.
- MEASURED-BOUNDS auto-fit: the splat maintains a running world AABB of
  plotted samples (subsampled ordered-float atomicMax, 8-word tail
  after the depth region); async readback (BoundsTracker) feeds
  volume_placement, which now fits the cube to the FLAME — shrinking
  well below the view fit when the attractor is small (every halving
  doubles base-coat resolution). Placement is FROZEN per accumulation
  run (splat coords depend on it); interactive auto-refit fires once
  when the first measurement lands (young runs only), exports run a
  6-batch warmup + blocking read + reset.
- Optically-thin dust: march opacity scales by smoothstep(0.05, 0.8)
  of raw density — sparse structure renders as translucent veil
  instead of solid voxel blocks ("big ugly blocks" report). Solid
  shells unaffected.

ARCHITECTURE SETTLED (2026-07-13, after base-coat field feedback): the
volume is SCAFFOLDING, never visible geometry. A practical grid cannot
represent the sub-voxel filaments flames are made of without inflating
them into voxel-thick tubes; rendering voxel color at all produces
lumpy blobs and halos wherever a filament is dense. The compositor
therefore uses the march ONLY for: surface depth + coverage (occlusion
authority), gradient normals, volumetric AO, shadow marches. Every
visible color is PIXEL data — the pixel's own accumulation when at or
in front of the volume surface, or a ring fill of neighboring pixels
anchored at the march depth (holes + occluded leaks). No pixel
evidence → passthrough; nothing is invented. This also retired the
base-coat brightness-alpha calibration problem.

Perf: run_shade_pass skips entirely when inputs are unchanged and the
temporal blend has settled (shade_dirty + 16-frame settle) — lighting
used to burn the full march/normals/AO/shadow chain every frame even
after rendering completed.

Volume-only fill (Feature::VolumeFill, SHIPPED): a variation can mark
an iteration's sample volume-only via the builder-emitted per-thread
`volume_fill_flag` — it splats into the density volume (sealing
geometry for occlusion / lighting / shadows) but never into the image,
so fill can't dilute colors. Dropped entirely when no volume exists.
`bubble_solid.fill` uses it; measured: mean image brightness identical
at fill 0.3 vs 0 (the old plotting fill dimmed the texture).

Shadow fine detail: the shadow march reads the RAW grid (trilinear,
opacity-clamped, 1.5-voxel steps) — the smoothed field cast
voxel-blurred shadows. Remaining limit: shadows can't be finer than the
raw grid; the queued answer for pixel-fine shadow detail is
screen-space contact shadows (short depth-buffer march toward the
light) composited with the volume shadows.

SPLAT-NATIVE PIVOT (2026-07-13, stage 1 SHIPPED): theory review
concluded the density volume's load-bearing roles all have
pixel-resolution splat-native replacements — see the discussion in this
session's log. sphere_volume's side emission now writes DEPTH-ONLY
splats into the camera depth buffer (encoding identical to the SOLID
block), so the existing splat-time occlusion culls everything behind
the emitted surface with NO volume required. Sealed-but-unsampled
pixels render dark — an honest "solid object, not yet textured" — and
fill in as real samples arrive. Verified: the sealed-pixel map is a
crisp disk matching the sphere silhouette. The volume deposit remains
when the volume is enabled (optional shading polish).

Stage 2 of the pivot (SHIPPED): LIGHT-SPACE SHADOW MAPS. 4 ortho
depth maps (SHADOW_MAP_RES=1024², 16 MB) ride the solid histogram's
tail after the bounds words; the main pass splats every plotted sample
and every side-emitted point toward each enabled light (ordered-float
atomicMax; shadow_map_splat in core/header.wgsl; runtime-gated by
params.shadow_count so Shadow Strength stays ShadingOnly-cheap, with a
gpu_updates escalation when the capture requirement flips). The shadow
fit (center + bounding-sphere radius from measured bounds) freezes per
run alongside the volume placement. shade.wgsl's shadow_map_factor
does a 4-tap PCF lookup with a 2.5-texel bias — shadows at SPLAT
resolution in BOTH shade paths (the volume-off path gains shadows for
the first time; the volume branch's raw-grid march is retired). The
basis derivation in splat and lookup must match exactly. Tiled
sample-emit exports: no maps (v1, documented).
LESSON (cost one debug cycle): the shade pass's camera rows/pos were
plumbed through VolumeShadeInput and were ZERO with the volume off —
camera data is now a first-class run_region argument; nothing
world-space may depend on the volume being enabled.
The density volume is now OPTIONAL everywhere: occlusion = splat-time
depth culling (+ sphere_volume sealing), shadows = maps, AO/normals =
screen-space; the volume adds gradient normals, volumetric AO and the
march compositor when enabled.

Stage B (NEXT):
- Light-space transmittance grids (deep-shadow style: one sweep per
  light per shade, one lookup per sample) replacing the per-pixel
  32-step shadow march.
- Brightness calibration of the base coat vs detail layer against
  engine-mean-intensity references; base_alpha currently =
  samples×survival/pixels.
- March quality: step-count/early-out tuning, possible half-res march
  + bilateral upsample for interactive, higher-dim volume as a
  quality setting.
- Retire/repurpose gap_fill & closing UI copy for the new pipeline.

## Phase 3 — backlog (unscheduled)

- JWF solid-rendering `.flame` XML import (`sld_render_*`, materials, lights).
- Post-DoF from the depth buffer (replaces at-splat DoF in solid mode).
- Supersampling / AA (render at 2x + downsample) — also the answer to
  residual lit-surface speckle beyond what à-trous removes.
- Firefly clamp in the shade pass (limit lit output vs neighborhood).
- Slope-adaptive surface thickness (fixes thick-shell ripples).
- Surface-sampler variation family (solid-friendly shells: sphere, box,
  torus projections with uniform coverage — the quaternion_julia_set
  sampler generalized).
- Matcap / environment-map materials ("image textures, later").
- Marching-cubes isosurface export (mesh for Blender) from the Phase 2 grid.
- à-trous as a general user-facing denoise effect.

## Known limitation: solid renders are not bit-reproducible

Measured (pentatope, same binary, deterministic_rng, two runs): ~49% of
covered pixels differ at mean channel delta ~4.8/255 (sub-visible shell
jitter; isolated thin-feature pixels can flip harder). Cause: within a
batch, a sample's occlusion test races the same batch's depth writes
(`atomicMax` return order is GPU-scheduling dependent). Depth persisted
from PREVIOUS batches is deterministic — only in-batch self-gating races.
(Correction for the record: commit 156067e's message claims a bit-identical
export verification; that comparison was actually measuring this
nondeterminism. The overwrite fix itself is sound.)

Consequences and options:
- Visual regression for solid scenes needs tolerance-based comparison, not
  pixel hashes (transparent renders remain bit-exact — verified).
- If bit-exact solid renders become necessary (regression tests, render
  farms): ping-pong the depth region (2 words/pixel, gate against the
  PREVIOUS batch's completed depth, swap each batch). Removes the in-batch
  race entirely at +1 word/pixel; depth converges one batch delayed, which
  priming already covers. Candidate for a "deterministic solid" option —
  not default (memory + a batch of convergence lag for no visual gain).

## Risks / open questions

- **Nearest-depth noise**: a single stray sample claims a pixel's depth.
  Mitigations: surface_thickness shell absorbs small noise; à-trous normal
  smoothing; if insufficient, switch the depth region to a small fixed-point
  histogram-of-depth (median-ish) at the cost of memory — decide empirically
  in Phase 0.
- **Perf**: +1 atomicMin per splat (~free next to 4 existing atomicAdds);
  shade pass is one fullscreen dispatch per frame (only when enabled).
- **Depth + heavy density effects**: density effects currently feed tonemap
  directly; with shading enabled the order is accumulate → shade → density
  effects → tonemap — confirm the chain composes (shade output is
  Rgba16Float like the density chain's textures).
- **fly-mode responsiveness**: solid mode re-primes depth on every camera
  move (camera changes reset accumulation already) — check that priming
  doesn't add perceptible lag to fly mode.
