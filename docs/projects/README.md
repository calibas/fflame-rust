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
| [camera-lens-everywhere.md](camera-lens-everywhere.md) | Planned 2026-10-10, not started: the camera lens in every mode. Three decisions first. |
| [hyperbolic-camera-extras.md](hyperbolic-camera-extras.md) | The hyperbolic camera's roadmap: its first half shipped in `hyperbolic_camera`; pathing and navigation through the animation system is next. |
| [v0.6.0-release.md](v0.6.0-release.md) | The v0.6.0 release's TODOs, tracked while changes still land: update the API, deploy the latest modules, cross-platform testing. |
| [slow-frame-slider-hold.md](slow-frame-slider-hold.md) | Planned 2026-10-12: hold any slider's drag once its frames turn slow, beyond the listed slow settings. |
| [escape-formula-language.md](escape-formula-language.md) | Planned 2026-10-13: Ultra Fractal and Fractint formulas and colourings, compiled to WGSL and carried in the config. Built through item 3; further UF support paused 2026-10-17. |
| [escape-kifs-and-variations.md](escape-kifs-and-variations.md) | Planned 2026-10-17: KIFS, Mandelbox and Mandelbulb solids in mode D; flame variations written in Rhai for flames and escape-time; editable solids later. |

## Paused

| Doc | Status |
|---|---|
| [deep-zoom/](deep-zoom/) | Focused Rendering (flame deep zoom): shipped as an opt-in feature, coverage work paused. Start at [deep-zoom/README.md](deep-zoom/README.md). |
| [escape-coloring-survey.md](escape-coloring-survey.md) | The escape colouring survey: items 1-5 built, 6-7 partly. |
| [pyfflame.md](pyfflame.md) | Python bindings: the tier 1-4 backlog, none of it built yet. |

## Open lists

| Doc | What it tracks |
|---|---|
| [doc-review-findings.md](doc-review-findings.md) | What the October 2026 doc refresh found wrong in the code, and what the fix pass that followed left open (its *Still open* section). |
| [escape-sim-followups.md](escape-sim-followups.md) | Escape-time and simulation decisions to make, the path tracer's speed, known gaps, and what the escape engine has not built. |
| [solid-rendering-todo.md](solid-rendering-todo.md) | Solid rendering for 3D flames: what is left. |
| [VARIATIONS_TODOS.md](VARIATIONS_TODOS.md) | The variation backlog. |
| [variation-port-blockers.md](variation-port-blockers.md) | Why particular variations are not ported yet. |
| [jwf-common-variations-port.md](jwf-common-variations-port.md) | Porting JWildfire's common variations (and the cut family). |

## Standing references

| Doc | What it is |
|---|---|
| [jwf-features.md](jwf-features.md) | JWildfire features: shipped, deferred, and why. |
| [VARIATIONS_WIRE_FORMAT.md](VARIATIONS_WIRE_FORMAT.md) | The variation wire format shared with the API repository. |
| [macos-release-checklist.md](macos-release-checklist.md) | The macOS release procedure, called from [docs/RELEASE.md](../RELEASE.md). |
