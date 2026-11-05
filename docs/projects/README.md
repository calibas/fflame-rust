# Projects

Work in progress: plans being built, paused projects, open backlogs and
the trackers they feed. How a finished system works is in
[docs/main/](../main/); ideas not yet started are in
[docs/experimental/](../experimental/); finished plans go to
[docs/archive/projects/](../archive/projects/) under the same name, with
a banner saying what happened (move them with `scripts/move_docs.py`,
which rewrites the references in code comments too).

## Plans

| Doc | Status |
|---|---|
| [terrain-speed.md](terrain-speed.md) | A rough outline (2026-10-31), not started: a deep escape terrain's section renders (single steps of 100-360 ms), which the quality governor's render scale cannot reach; the lit tier's antialiasing redrawn and thrown away while sections arrive (suspected); candidates from one lit picture a ground to a section budget the governor sets. |
| [camera-lens-everywhere.md](camera-lens-everywhere.md) | Planned 2026-10-10, not started: the camera lens in every mode. Three decisions first. |
| [hyperbolic-camera-extras.md](hyperbolic-camera-extras.md) | The hyperbolic camera's roadmap: its first half shipped in `hyperbolic_camera`; pathing and navigation through the animation system is next. |
| [v0.6.0-release.md](v0.6.0-release.md) | The v0.6.0 release's TODOs, tracked while changes still land: update the API, deploy the latest modules, cross-platform testing. |
| [slow-frame-slider-hold.md](slow-frame-slider-hold.md) | Planned 2026-10-12: hold any slider's drag once its frames turn slow, beyond the listed slow settings. |
| [bristorbrot.md](bristorbrot.md) | Built 2026-10-23 (branch `bristorbrot`): Doug Bristor's Bristorbrot and the Quaternionbrot beside it, as Rhai solids -- 3D sections of 4D iterations, Mandelbrot and Julia, the square and both cubes -- with eight presets; why the Bristorian distance needs care (the product stretches by up to 3.3 \|z\|). |
| [quaternions.md](quaternions.md) | Decided and built 2026-10-22 (branch `quaternions`): a `quat` type in Rhai solids and 3D variations; Halayka's (2009) quaternion sets as a solid, his (2018) escape paths as a flame variation, with presets. |
| [online-assets.md](online-assets.md) | Planned 2026-10-21, decided the same day, not started: skies and textures listed by providers (Poly Haven first), downloaded and kept locally; every resolution; textures on solids and terrains with valley, slope and plateau targets. |
| [wasm-modules.md](wasm-modules.md) | Researched 2026-10-23. Section 1 done 2026-11-04: the gallery modules dropped flame variations escape and simulation configs use, so one renderer replaced them and the engine features went. Open: what the web app lacks; what its 6.5 MB gzip is made of; four ways to load parts on demand, an order of work, decisions to make. |
| [one-workspace.md](one-workspace.md) | Decided and built 2026-10-22 (branch `one-workspace`): one workspace for every mode, every panel drawn in every mode with a banner where unused, choosing enters the mode, the mode button in the menu bar; Flames, Escape Time and Simulation as layouts to pick. |

## Paused

| Doc | Status |
|---|---|
| [deep-zoom/](deep-zoom/) | Focused Rendering (flame deep zoom): shipped as an opt-in feature, coverage work paused. Start at [deep-zoom/README.md](deep-zoom/README.md). |
| [escape-formula-language.md](escape-formula-language.md) | Planned 2026-10-13: Ultra Fractal and Fractint formulas and colourings, compiled to WGSL and carried in the config. Built through item 3 and its follow-ups; further UF support paused 2026-10-17. |
| [surface-textures.md](surface-textures.md) | A terrain's surface texture (bump): step 1 built; material maps and the rest paused -- texture materials go on in [online-assets.md](online-assets.md). |
| [escape-coloring-survey.md](escape-coloring-survey.md) | The escape colouring survey: items 1-5 built, 6-7 partly. |
| [pyfflame.md](pyfflame.md) | Python: the engine as a WASI module (rendering, animation, scripts); the copied code awaiting review; the backlog. |

## Open lists

| Doc | What it tracks |
|---|---|
| [architecture-review.md](architecture-review.md) | The 2026-11-04 architecture review: the docs against the code, five parallel systems (config writes, the headless facade, the render tail, carried sources, origins), the WASM modules, every surface against animation and scripting, the governors, the plans, and a proposed order. |
| [doc-review-findings.md](doc-review-findings.md) | What the October 2026 doc refresh found wrong in the code, and what the fix pass that followed left open (its *Still open* section). |
| [escape-sim-followups.md](escape-sim-followups.md) | Escape-time and simulation decisions to make, what is left of the path tracer's speed, the 3D views' and the escape engine's unbuilt items, known gaps. |
| [solid-rendering-todo.md](solid-rendering-todo.md) | Solid rendering for 3D flames: what is left. |
| [VARIATIONS_TODOS.md](VARIATIONS_TODOS.md) | The variation backlog. |
| [variation-port-blockers.md](variation-port-blockers.md) | Why particular variations are not ported yet. |
| [jwf-common-variations-port.md](jwf-common-variations-port.md) | Porting JWildfire's common variations (and the cut family). |

## Standing references

| Doc | What it is |
|---|---|
| [one-parameter-system.md](one-parameter-system.md) | Branch `params-one-system` (2026-11-03): one parameter definition, one renderer and one compiler converter for every kind -- variations, effects, escape formulas and colourings, simulations, CPU scripts -- in place of five definition types and four widget sets; conditions and grids in every panel with it. The survey, the design, the steps. |
| [rhai-plugins.md](rhai-plugins.md) | The plan (direction 2026-10-18, reviewed 2026-11-01): everything editable a Rhai script -- variations, effects, formulas, colourings, simulations, animation tracks, generators -- as plug-ins, local files and downloads. The review of today's three ways in, the user's decisions, the order of work (P0 the CPU sandbox with `import` through the library, P1 one GPU machinery in namespaces, P2 effects, P3 one library, P4 simulations, P5 tracks, P6 the API), and what is left of the KIFS and variations plan. |
| [jwf-features.md](jwf-features.md) | JWildfire features: shipped, deferred, and why. |
| [VARIATIONS_WIRE_FORMAT.md](VARIATIONS_WIRE_FORMAT.md) | The variation wire format shared with the API repository. |
| [macos-release-checklist.md](macos-release-checklist.md) | The macOS release procedure, called from [docs/RELEASE.md](../RELEASE.md). |
