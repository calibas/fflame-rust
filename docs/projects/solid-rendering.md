# Solid Rendering: occlusion, lighting, and shading for 3D flames

**Status**: Phase 0 in progress (branch `solid-rendering`)
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
- [ ] Sample-emit + tiled + CPU export paths (>128 MB exports render
      WITHOUT solid for now — the SOLID flag self-gates off the sample-emit
      shader; wire the tiled paths before closing Phase 0).
- [x] Tests: `solid_off_is_byte_identical` (byte-identity + 2D +
      sample-emit gating), `solid_depth_encoding_is_monotone`. Baselines
      re-verified pixel-identical with solid off. Visual regression
      category still TODO.
- [ ] `TARGET_BUFFER_SIZE` math in high_res.rs for the 5/4 growth (only
      matters once the tiled path carries depth).

## Phase 1 — deferred shading

Deliverable: lit solids — normals, ambient + up to 4 lights
(directional/point, Blinn-Phong to match JWF's parameter vocabulary), SSAO,
specular. The milestone where same-palette shapes become legible.

- [ ] Shade compute pass + pipeline; wire through `tonemap_pass_with_input`;
      skipped entirely when `solid_strength == 0` and lighting off.
- [ ] Position reconstruction from depth; screen-space normals from depth
      gradients.
- [ ] Depth-guided à-trous smoothing for normals (2–3 iterations, reusing
      the effect-chain `Rgba16Float` ping-pong textures as scratch).
- [ ] Lighting model: ambient + N≤4 lights, diffuse + specular; final color
      = `mix(emissive_flame_color, lit_color, shading_strength)` — the
      emissive term preserves today's look as a blendable component.
- [ ] SSAO: depth-buffer horizon sampling, radius/strength params.
- [ ] Per-transform `material` index + material table (diffuse/specular/
      shininess per slot) — the JWF import hook, even before XML import
      exists.
- [ ] Lighting panel (lights, material editor, SSAO, strengths); config +
      undo + animation-track paths for all lighting params.
- [ ] WASM verification pass (all core WebGPU; no FLOAT32_FILTERABLE
      dependency — shade pass uses `textureLoad`).
- [ ] Visual regression scenes: single light, multi-light, SSAO-only,
      solid_strength sweep.

## Phase 2 — density volume

Deliverable: world-space ∇ρ normals (stable under camera motion, no
screen-space edge artifacts), volumetric AO, shadow rays toward lights, and
an optional emission/absorption translucent mode. This is where quality
passes JWF.

- [ ] Flat `array<atomic<u32>>` density grid in a world-space AABB
      (auto-fit from accumulated bounds or user-set). Size auto-scales to a
      memory budget: desktop default 256³ (67 MB), WASM/mobile 128³ (8 MB).
      Separate bind group (shade pass, not main pass) to respect the
      binding budget; the *splat* into the grid does ride the main pass —
      one added storage buffer there, acceptable because it's optional
      (template-gated) and Phase-2 hardware targets report ≥12.
- [ ] Splat: one extra `atomicAdd` per plotted sample (optionally
      stochastically subsampled for perf).
- [ ] Shade pass: trilinear ∇ρ normals replacing screen-space normals
      (screen-space kept as fallback / low-VRAM mode); density-sphere AO;
      fixed-step shadow march toward each light.
- [ ] Translucent volumetric mode: short emission/absorption march composited
      with the solid term — makes solid↔volumetric a continuous artistic dial.
- [ ] High-res export: the grid is resolution-independent — no tiling work.

## Phase 3 — backlog (unscheduled)

- JWF solid-rendering `.flame` XML import (`sld_render_*`, materials, lights).
- Post-DoF from the depth buffer (replaces at-splat DoF in solid mode).
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
