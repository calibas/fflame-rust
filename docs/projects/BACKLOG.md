# Backlog

Everything still open, in one place (consolidated 2026-11-11). Each item
says what it is and why it is open, and links the doc that holds the
detail -- most now in [docs/archive/](../archive/), the plans not yet
started in [docs/experimental/](../experimental/). Items found already
built when this was written were left out; the archived docs may still
list them as open.

When an item becomes a project, give it a plan in `docs/projects/` and
take it off this list. When one is done or dropped, delete its line.

**Active elsewhere:** the v0.6.0 release checklist
([v0.6.0-release.md](v0.6.0-release.md)), the release streamlining plan
([release-streamlining.md](release-streamlining.md), six decisions), the
API update plan ([api-updates.md](api-updates.md); step 4 built on
branches, [rhai-scripts-online.md](rhai-scripts-online.md)) and
the size study ([size-study.md](size-study.md)).

---

## Decisions waiting on the user

Each blocks the items it names; most carry a recommendation in their doc.

1. **Texture source for flames**: where a flame's texture(s) come from.
   Blocks texture-sampling variations (colormap_wf, displacemap_wf,
   post_bumpmap_wf, text_wf, svg_wf, plane_wf's colormap) and textures
   in other kinds. [variation-port-blockers.md](../archive/projects/variation-port-blockers.md)
   §"Texture / image sampling (#2)".
2. **D7, per kind**: whether the Rhai twins replace their built-ins
   (escape formulas, colourings, variations, simulations, solids,
   fields). Input from the size study. [rhai-plugins.md](../archive/projects/rhai-plugins.md) §"Decisions".
3. **Float-threshold parameters**: keep as Float, or add a
   `legacy_import` shim (hypercrop.zero, tile_reverse.reversal,
   cpow3_wf.discrete_spread, waves2b.pwx/pwy, hexaplay3D/hexnix3D.majp).
   [VARIATIONS_TODOS.md](../archive/projects/VARIATIONS_TODOS.md) §"Float-threshold dispatch".
4. **The prepost family**: is `VariationPhase::PrePost` worth building
   for prepost_affine / prepost_circlize / prepost_mobius.
   [VARIATIONS_TODOS.md](../archive/projects/VARIATIONS_TODOS.md) §"Prepost (priority-2) variations".
5. **Rhai overloads by arity**: support, or keep refusing (`lang/rhai.rs`).
   [rhai-plugins.md](../archive/projects/rhai-plugins.md) §"Not given a branch".
6. **C3**: a built-in WGSL variation as a formula step, and the UF maps
   uf-transforms-review found missing, as Rhai variations.
   [rhai-plugins.md](../archive/projects/rhai-plugins.md) §"Left from the KIFS and variations plan".
7. **P9** (flame deep zoom), after its measurement: do the forward
   difference forms go. Also confirm the web taking twins' bounds for
   built-ins (built that way; one line in `ifs_ball::derived`).
   [rhai-one-form.md](../archive/projects/rhai-one-form.md) §"Step 3".
8. **Resuming UF support** (formula-language item 4), then UF classes
   and plug-ins (item 5). [escape-formula-language.md](../archive/projects/escape-formula-language.md)
   §"After phase 4".
9. **Simulation D1/D3**: preview vs video without a Steps track (a or b);
   stable ids for layers, colour layers and couplings so tracks can
   target them. [escape-sim-followups.md](../archive/projects/escape-sim-followups.md).
10. **Path tracer levers** left open, and adaptive sampling's default.
    [escape-sim-followups.md](../archive/projects/escape-sim-followups.md) §"TODO: the solid path tracer's speed".
11. **A light's size apart from its shadow sharpness.**
    [escape-sim-followups.md](../archive/projects/escape-sim-followups.md) §"3D views: not built".
12. **Online assets**: a solid's texture targets (orientation/concavity,
    or one texture); contacting Poly Haven.
    [online-assets.md](../experimental/online-assets.md) §"Still open".
13. **Camera lens everywhere**: three decisions before starting.
    [camera-lens-everywhere.md](../experimental/camera-lens-everywhere.md) §"5".
14. **Preview slicing**: budget, pause while dragging, renderer reuse,
    scope. [preview-slicing.md](../experimental/preview-slicing.md) §"Decisions to make".
15. **Web app loading on demand**: is splitting off the language module
    and audio worth 1.7 of 6.5 MB gzip, or are variations-as-data
    enough; serve built-in variation files from `pkg/assets/` or the
    API. Input from the size study. [wasm-modules.md](../experimental/wasm-modules.md) §"4", §"5".
16. **Effect twins in a browser**: run them by hand.
    [rhai-plugins.md](../archive/projects/rhai-plugins.md) §"P2".
17. **The volume emitters** (polyhedron_volume, sphere_volume) as Rhai
    twins: possible later work. [rhai-plugins.md](../archive/projects/rhai-plugins.md) §"P7".
18. **pyfflame's copied code**: the review the user will detail
    (six copies). [pyfflame.md](../archive/projects/pyfflame.md) §"4".

The API's own decisions (variation slot cap, `simulation` in
`render_modes`, which effect WGSL the API holds) belong to the API plan
and [v0.6.0-release.md](v0.6.0-release.md).

---

## Variations

- **85 variations without `# Authors`**; `arcsech` left of the
  hyperbolic group; 30 Apophysis Plugin Pack entries carry the pack
  placeholder. Research only, no invented names.
  [VARIATIONS_TODOS.md](../archive/projects/VARIATIONS_TODOS.md) §"Author attribution research".
- **Enum candidates**: `iconattractor_js.preset_id` (render and name
  the 17 presets); `subflame_wf.color_mode` range.
  §"Enum candidates — deferred".
- **Render comparisons never run**: the `disc3` sqrt clamp
  (`stub_recoveries2.rs`), `wave3` artifacts, and four DC checks
  (dc_linear/dc_bubble on mixed transforms, dc_bubble Java vs C++,
  colorscale_wf `reset_z`, its TC source). §"Numerical edge-case
  divergence", §"Direct-color (DC) port decisions to verify".
- **DC gradient mode 1** (nearest palette) unsupported.
  §"Direct-color (DC) infrastructure".
- **Effect parameter descriptions** mostly empty: data entry.
  §"Effects metadata".
- **Persistent-state ports** (10): brownian_js, curliecue, dc_dmodulus,
  dc_cracklep_wf, nblur, pre_stabilize, recurrenceplot, scrambly,
  sphtiling3v2, ztwister. [variation-port-blockers.md](../archive/projects/variation-port-blockers.md) §"Persistent state (#6)".
- **32 DC_BaseFunc derivatives**: dc_acrilic, apollonian, base,
  circlesblue, circuits, ducks, escher, fractaldots, gnarly, grid3d,
  hexagons, hoshi, hyperbolictile, kaleidocomplex, kaleidoscopic,
  kaleidotile, kaliset, kaliset2, layers, mandala, mandbrot, menger,
  randomoctree, rotations, squares, starsfield, tesla, tree, tritile,
  truchet, turbulence, voronoise. Tens of hours. §"DC_BaseFunc derivatives".
- **Primitives infrastructure** (17): dragon_js, gosperisland_js,
  gpattern, hexes, hilbert_js, htree_js, inversion, koch_js, lsystem_js,
  mandala, mandala2, nsudoku, rsquares_js, sunvoroni, taprats, tree_js,
  triantruchet. (`lsystem_path`/`lsystem_tree` draw the look under other
  names, so JWF flames do not import.) §"Primitives infrastructure (#7)".
- **Other blocked**: falloff3 family (coefficient table);
  metaballs3d_wf (host-precomputed buffer, `wgsl_data_init`); subflame
  variants ringsubflame, glynns3subfl, starfractal; post_depth
  (pre-affine read in post); post_smartcrop (mid-iteration accumulator);
  pre_recip (complex inverse hyperbolics); bwrands, crob, gdoffs,
  grid3d_wf, knots3d, polylogarithm, dla_wf / dla3d_wf (CPU precompute),
  maurer_lines; glsl_randomoctree (needs a cost model);
  fract_formula_julia_wf / _mand_wf (infix DSL). §"Other abstract base
  classes (#8)" and on.
- **`log_apo` alias to `log`**: trivial (`cylinder_apo` is aliased).
  §"Already covered in registry".
- **Cut family** (18): 13 with JWF GPU code (cut_fun, wood, truchet,
  truchetweaving, mandala, kleinian, spiralcb, magfield, alientext,
  triantess, randomtile, shapes, spots); 5 to hand-port from Java
  (cut_c, triskel, btruchet, tstruchet, 2ewangtile).
  [jwf-common-variations-port.md](../archive/projects/jwf-common-variations-port.md) §"Cut family — TODO".
- **`buddhabrot_mode`** in the six fract_*_wf variations: accepted,
  not implemented. §"Group E". **isosfplot3d_wf** needs a formula
  evaluator (or hard-coded presets). §"Also unblocked by doHide".
- **Variation twins**: 7 left -- mondrianomies (refused for its
  host-baked table; retry now that parameter storage landed),
  analytic_blur / analytic_gaussian_blur / subflame_wf /
  pre_subflame_wf (out of plan), the two volume emitters (decision 17).
  [rhai-plugins.md](../archive/projects/rhai-plugins.md) §"P7".

## .flame import (JWildfire / Apophysis)

- **Zoom for non-1920 `size`**: import computes `zoom = scale/200 ×
  cam_zoom` and ignores `image_size`; which dimension drives the zoom,
  and where 200 comes from. [jwf-features.md](../archive/projects/jwf-features.md) §"`size`".
- **Variation order limits**: per flame, not per transform; a variation
  cannot appear twice in one transform; accumulate after replace does
  not match. §"Variation order preservation".
- **Dropped on import**: per-transform `color_type`, `mod_*` colour
  modulators, `wfield_*` noise fields. §"Per-transform `color_type`" and on.
- **The true 12-coefficient affine** (zx/yz): deferred until a use case.

## Rhai and the formula language

- **P6, the API** for Rhai kinds: the API plan.
- **Per-kind registries**: registry, failure memory and provenance are
  still per kind; a track naming a variation key a later Use replaced;
  subflame transforms not re-keyed by Use.
  [rhai-plugins.md](../archive/projects/rhai-plugins.md) §"P1".
- **A combined library panel**, if wanted. §"P3".
- **Layouts outside simulations**: a grid's fill and warning
  (`LayoutSpec::without_cpu`); a layout for CPU scripts
  (`param_show_if`). §"What is left, as branches".
- **Effect downloads' WGSL is not parsed before registration**;
  `ApiVariation` / `ApiEffectStage` unused. §"Review".
- **`distance_field`** stays WGSL unless the dialect gets passes. §"P2".
- **KIFS leftovers**: a formula's Live re-apply after an edit to a
  variation it calls; a hybrid member carrying its variation; a Rhai
  variation's state inside a formula; C4 transformation gaps (none in
  mode B, Orbit only in mode D, clipping, a colouring's start reading the
  Orbit map); a function parameter named like a built-in.
  §"Left from the KIFS and variations plan".
- **Unexplained slowdowns** under the gate: Brusselator2 twin 2-3%, LIC
  3-4%. §"Not given a branch".
- **rhai-one-form leftovers**: juliascope-rays' ~1% extra light in block
  66 at 1e4 (next lead: the free orbit inside a targeted render);
  optional `call_batch` chunking; a dual walker for exact Rhai Jacobians.
  [rhai-one-form.md](../archive/projects/rhai-one-form.md).
- **Script tracks**: a track's own parameters as animation targets;
  `import` in tracks; computed/pattern targets (`transform.*.weight`);
  signal-track smoothing onto fixed steps (scrub differs from play today).
  [rhai-tracks.md](../archive/projects/rhai-tracks.md).
- **Formula Editor**: compiles on the UI thread (giant UF entries freeze
  it 6-8 s); error spans not underlined.
  [escape-formula-language.md](../archive/projects/escape-formula-language.md) §"After phase 4".
- **Formula language internals**: derivative temporaries as
  pass-locals (shrinks every formula with a derivative); screen-sized
  arrays, per-pixel orbit buffers, dynamic arrays.
- **UF (paused, decision 8)**: params past 64 floats, colouring state past
  32, ~150 small mismatches, `random(seed)`, `.ugr` gradients; classes,
  `.ulb`, `.upr`; UF `#color` (refused, `check.rs`); `.uxf` transforms;
  KF `.kfr` hybrid import.
- **Retire native formulas** where twins match (D7): prerequisites --
  compress custom orbits in the store, flatten references to in-place big
  floats, Newton's `z^p sin z - 1` at a basin boundary, the scaled
  rung's f32 reference sums (Ducks), a possible fixed-point overflow at
  power 9+. §"After phase 4" item 6.
- **Held structural keyframes** (formula source and name as one target);
  a formula randomiser. §"Phase 4 … step 8".
- **Formula library in the Fractal Browser**, and an API formula corpus.
  §"Phase 5: distribution".

## Escape-time

- **Colouring pipeline candidates**: P8 separate inside colouring, P9
  phase add, P10 OKLab palette interpolation and a finer table; O1
  ordered dither on 8-bit export; R9 relief independent of resolution
  (needs a better model); a deeper itinerary that survives zoom; Sphere
  average debanding; full coloured layers with UF merge modes.
  [escape-coloring-survey.md](../archive/projects/escape-coloring-survey.md).
- **Splits' deep zoom** (`fn image_gap`); series approximation
  (unmeasured against BLA); row banding of the perturbed path; NTT GPU
  reference orbits (go/no-go never run); spectral rendering.
  [escape-sim-followups.md](../archive/projects/escape-sim-followups.md) §"The escape engine: not built".
- **Mode D ignores opacity**: a bug, fix and test planned.
- **`timeline_of_a_cached_revisit`** fails, since before 2026-09-02.
- **Deep rung BLA** costs a little in chaotic views.
- **Motion-blur leftovers**: the viewport's path-traced solid, per-thread
  time for flames, a terrain's lit-pass timing, the API field.

## Simulations

- **Script access**: `escape.formula` takes only `FORMULAS` (field and
  IFS refused, `script/api.rs`); no `escape.preset`; curated per-group
  functions later. [escape-sim-followups.md](../archive/projects/escape-sim-followups.md) §"D2".
- **A simulation terrain has no surface texture.**
  [surface-textures.md](../archive/projects/surface-textures.md).

## 3D: solids, terrains, lighting, path tracer

- **Flame solids**: reflection/environment maps (the coat reflects only
  an ambient sky); bump mapping; per-transform materials; positional
  point lights (skipped by choice); per-light shadow strength (import
  takes the max); JWF AO mapping (needs A/B renders).
  [solid-rendering-todo.md](../archive/projects/solid-rendering-todo.md).
- **DoF**: on the tiled export (skipped with a warning, `high_res.rs`);
  reset-free DoF sliders in solid mode; CoC cap scaled for supersampling.
- **AA and shadows**: 2x AA in WASM; 4x (with a use case); shadow map
  resolution setting; shadows on the CPU export path.
- **Solid performance**: half-resolution DoF gather and interactive
  shading; early shadow-splat subsampling; shade-chain bind-group caching
  (made every frame, `shade_pass.rs`); hoist the shadow basis.
- **Escape 3D views not built**: iteration transformations for every
  solid (needs a step factor); a point light on a terrain; more than 4
  lights; placing a light by click or fly-mode key; emissive geometry; a
  Paper preset; presets showing the skies; Halayka's principled surface.
  [escape-sim-followups.md](../archive/projects/escape-sim-followups.md) §"3D views: not built".
- **Path tracer levers**: S2 fused rewrite or S3 wavefront; D6 learned
  denoiser; V3 blue noise; V4 Russian roulette from the first bounce; S4
  compile in/out; S5 distance cache; Zirr outlier re-weighting (decision 10).
- **Surface textures**: material maps; 2D relief anchored to the picture;
  canvas/sand/hammered-metal kinds; displacement at section ingest.
  [surface-textures.md](../archive/projects/surface-textures.md) §"Steps".

## Flame deep zoom (Focused Rendering)

Opt-in, coverage work paused. Agreed order: **P9, then C14, C13, C15**;
C16 unplaced. Detail: [deep-zoom-tracker.md](../archive/deep-zoom/deep-zoom-tracker.md).

- **P9**: offsets by Jacobian chains (`d = J·d`) in place of the
  per-kernel difference forms; table size (~45M floats) and seams open;
  decide by measurement first (decision 7).
- **C14**: one refinement mechanism for RNG-bearing maps (fixes C7's holes).
- **C13**: every final through the render's own code (blocks
  hypertile1/julia3D finals; low urgency).
- **C15**: kernels as registry entries (`InverseDef`); when the next
  kernel is added.
- **C16**: the GPU planner's f32 drops landing children at 1e5-1e6
  (0.1-3.7%); visibility unknown; likely fix: lower `GPU_PLAN_RADIUS`.
- **C17**: opacity and Speed colour under targeting use the free orbit's
  transform (quick fix: decline; right fix: the word's last symbol).
- **C4** Schottky flames (Möbius analysis); **C5** over the forward word
  cap; **C6** blur-beside-kernel draw conditioned on the target; **C7**
  leftovers (pole tail, geometric candidate test); **C11** accuracy
  tails at 1e6/1e8.
- **P2** patterns instead of paths (measure first); **P3** step 2, a
  deeper word cap (dense replay table), work budget vs time budget;
  **P4** keep-or-carry in one GPU submission; **P5** the first plan in
  the browser (38-50 ms of compiles); **E2** 3D Focused Rendering
  ([planner-experiments.md](../experimental/planner-experiments.md)).
- **Precision and bounds**: zoom past f64's pan (~1e12-1e13); the
  references' cost on the web; colour (DC/RGB) for forward bounds; 9
  `Unbounded` flames need hand bounds; julian-disc's cover leaks 34% at
  256 discs; branch hover highlight in the word editor.
- **Small**: Metal and Firefox never run; new strings English only;
  a `→` glyph in the coupling row (`sim_panel.rs`); flaky timing tests
  `a_standby_plan_covers_a_move`, `the_web_plan_job_plans_a_slice_a_frame`;
  pie/pie3D rotation labelled degrees, shader uses radians (`shapes.rs`).

## Web app and builds

- **Web gaps against the desktop**: video (WebCodecs on `FrameSource`);
  signal file load/save (silent no-op); HDRI skies (none on the web,
  `environment.rs`); CJK fonts; deep zoom's orbit worker and store;
  Detect Period; the planner budget at 8M; naga validation and derived
  bounds; 2x supersample and terrain background export; persistent
  caches; VSync and device-lost rebuild.
  [wasm-modules.md](../experimental/wasm-modules.md) §"2".
- **Web-only warnings** (deprecated web-sys, desktop-only code).
  [doc-review-findings.md](../archive/projects/doc-review-findings.md) §"Still open".
- **Browser export gaps**: texture layers not generated; the custom-size
  export never run in a page ([v0.6.0-release.md](v0.6.0-release.md)).
- **Load on demand** (A variations as data, B a language module, C
  wasm-split): see the size study and decision 15.

## Python (pyfflame)

[pyfflame.md](../archive/projects/pyfflame.md) §"5", §"6".

- **Backend gaps**: no depth/stencil, vertex/index buffers, bundles,
  timestamps; error scopes always empty (OOM checks see nothing);
  audio-file signal tracks not loaded; a panic traps the module.
- **Backlog**: a transform proxy, palette access, bulk field views;
  linked/final, post-affine, xaos, subflames, solo, post-symmetry,
  effects; type stubs, PyPI, an API reference; animations from Python,
  seed batches, `_repr_png_`, numpy, `python -m pyfflame convert`.
- **The copied-code review** (decision 18).

## App, UI and code health

- **Deep-zoom gates read gitignored files** (`output/flame-zoom/*.fflame`):
  they cannot run from a fresh clone.
  [doc-review-findings.md](../archive/projects/doc-review-findings.md) §"Still open".
- **Move the flame dispatch governor** (`Batch`, `Knee`,
  `adjust_iter_scale`) out of `app/mod.rs` beside `app/governor.rs`.
- **A Fractal Browser thumbnail of an escape config with a long
  uncached texture** generates it, blocking.
- **~60 stale comments** and possibly dead public functions, never
  re-audited. §"Stale comments", §"Dead code".
- **Known gaps**: no PNG input for `export`; flame-only metadata written
  for escape and simulation pictures; es/ja/zh mostly English.
  [escape-sim-followups.md](../archive/projects/escape-sim-followups.md) §"Known gaps".

## Plans not started

Each is a plan, kept in [docs/experimental/](../experimental/); move it
back to `docs/projects/` when work starts.

- [terrain-speed.md](../experimental/terrain-speed.md) -- a deep escape
  terrain's section renders (100-360 ms steps) out of the governor's
  reach; measure first.
- [preview-slicing.md](../experimental/preview-slicing.md) -- the
  Fractal Browser's previews as a job advanced a few ms a frame
  (decision 14).
- [camera-lens-everywhere.md](../experimental/camera-lens-everywhere.md)
  -- the camera lens in every mode (decision 13).
- [slow-frame-slider-hold.md](../experimental/slow-frame-slider-hold.md)
  -- hold any slider's drag once its frames turn slow; audit ~150 raw
  sliders.
- [online-assets.md](../experimental/online-assets.md) -- skies and
  textures from providers (Poly Haven first), web skies, texture
  materials (decision 12).
- [hyperbolic-camera-extras.md](../experimental/hyperbolic-camera-extras.md)
  -- pathing and navigation through the animation system (parabolic
  isometries first), then the deferred extras.
- [wasm-modules.md](../experimental/wasm-modules.md) -- the web app's
  gaps and ways to load parts on demand (decision 15; the size study
  measures it).

## Decided: will not do

- **Java-JIT variations**: custom_wf, custom_wf_full, pre_custom_wf,
  post_custom_wf, dc_code, colordomain, ducks, glsl_code -- refused
  (arbitrary code). [variation-port-blockers.md](../archive/projects/variation-port-blockers.md) §"JIT-compiled custom code (#1)".
- **JWF `material`**. [jwf-features.md](../archive/projects/jwf-features.md).
- **Escape**: Chebyshev, mode C (recorded declines).
- **Edges-only AA** (not recommended).
