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
| [rhai-planners.md](rhai-planners.md) | Built 2026-11-08 (branch `rhai-planners`): Rhai variations in Focused Rendering and the Flame Attractor. Part A: forward bounds derived from a Rhai variation's own code -- every twin whose built-in derives now derives (472). Part B: a Rhai variation declares its inverse (`fn inverse`, `fn image_gap`, `fn inverse3`), checked against its forward map, and the Flame Attractor draws it alone, summed with an affine and as a solid; the planar kernels' twins carry one, and ten built-ins with none in Rust borrow their twins'. Part C (2026-11-08): Focused Rendering's inverse walk takes a Rhai inverse -- arms read off the forward map, deep zoom by a difference form derived from the body. Part D (2026-11-09): kept interpreter runs, drawing kernels summed with an affine, the Flame Attractor's deep zoom through a Rhai inverse, and a point with no preimage read as the ball's centre on the GPU. Part E (2026-11-09): hyperbolic's inverse; every twin with an inverse derives both difference forms; hyperbolic's spurious hole. Open: A3; splits' deep zoom (a per-branch image gap). |
| [terrain-speed.md](terrain-speed.md) | A rough outline (2026-10-31), not started: a deep escape terrain's section renders (single steps of 100-360 ms), which the quality governor's render scale cannot reach; the lit tier's antialiasing redrawn and thrown away while sections arrive (suspected); candidates from one lit picture a ground to a section budget the governor sets. |
| [preview-slicing.md](preview-slicing.md) | Planned 2026-11-04, not started: the Fractal Browser's previews as a job advanced a few milliseconds a frame (a cooperative `render` with yield points at its GPU waits, driven as `scene::slice` drives the planner), so no frame waits for a whole render -- the simulations' full step counts above all. The order of work starts with measuring. |
| [camera-lens-everywhere.md](camera-lens-everywhere.md) | Planned 2026-10-10, not started: the camera lens in every mode. Three decisions first. |
| [hyperbolic-camera-extras.md](hyperbolic-camera-extras.md) | The hyperbolic camera's roadmap: its first half shipped in `hyperbolic_camera`; pathing and navigation through the animation system is next. |
| [v0.6.0-release.md](v0.6.0-release.md) | The v0.6.0 release's TODOs, tracked while changes still land: update the API, deploy the latest modules, cross-platform testing. |
| [slow-frame-slider-hold.md](slow-frame-slider-hold.md) | Planned 2026-10-12: hold any slider's drag once its frames turn slow, beyond the listed slow settings. |
| [online-assets.md](online-assets.md) | Planned 2026-10-21, decided the same day, not started: skies and textures listed by providers (Poly Haven first), downloaded and kept locally; every resolution; textures on solids and terrains with valley, slope and plateau targets. |
| [wasm-modules.md](wasm-modules.md) | Researched 2026-10-23. Section 1 done 2026-11-04: the gallery modules dropped flame variations escape and simulation configs use, so one renderer replaced them and the engine features went. Open: what the web app lacks; what its 6.5 MB gzip is made of; four ways to load parts on demand, an order of work, decisions to make. |

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
| [doc-review-findings.md](doc-review-findings.md) | What the October 2026 doc refresh found wrong in the code, and what the fix pass that followed left open (its *Still open* section). |
| [escape-sim-followups.md](escape-sim-followups.md) | Escape-time and simulation decisions to make, what is left of the path tracer's speed, the 3D views' and the escape engine's unbuilt items, known gaps. |
| [solid-rendering-todo.md](solid-rendering-todo.md) | Solid rendering for 3D flames: what is left. |
| [VARIATIONS_TODOS.md](VARIATIONS_TODOS.md) | The variation backlog. |
| [variation-port-blockers.md](variation-port-blockers.md) | Why particular variations are not ported yet. |
| [jwf-common-variations-port.md](jwf-common-variations-port.md) | Porting JWildfire's common variations (and the cut family). |

## Standing references

| Doc | What it is |
|---|---|
| [rhai-plugins.md](rhai-plugins.md) | The plan (direction 2026-10-18, reviewed 2026-11-01): everything editable a Rhai script -- variations, effects, formulas, colourings, simulations, animation tracks, generators -- as plug-ins, local files and downloads. The review of today's three ways in, the user's decisions, the order of work (P0 the CPU sandbox with `import` through the library, P1 one GPU machinery in namespaces, P2 effects, P3 one library, P4 simulations, P5 tracks, P6 the API), and what is left of the KIFS and variations plan. |
| [jwf-features.md](jwf-features.md) | JWildfire features: shipped, deferred, and why. |
| [VARIATIONS_WIRE_FORMAT.md](VARIATIONS_WIRE_FORMAT.md) | The variation wire format shared with the API repository. |
| [macos-release-checklist.md](macos-release-checklist.md) | The macOS release procedure, called from [docs/RELEASE.md](../RELEASE.md). |
