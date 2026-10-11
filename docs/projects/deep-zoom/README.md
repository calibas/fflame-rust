# Focused Rendering: the deep-zoom family

Deep zoom for flames ("Focused Rendering" in the app, "cylinder
targeting" in the code) shipped as an opt-in feature for 2D flames; how
it works is in [docs/main/FOCUSED-RENDERING.md](../../main/FOCUSED-RENDERING.md).
Its coverage work is paused, not finished, so its design docs stay here
together.

**Start at [deep-zoom-tracker.md](deep-zoom-tracker.md)**: the open
items, their measurements and the agreed order of work.

| Doc | What it holds |
|---|---|
| [flame-deep-zoom.md](flame-deep-zoom.md) | The plan: the stages (1-3 built), and how a forced sample keeps the picture unbiased. |
| [inversive-targeting.md](inversive-targeting.md) | The inverse walk (`src/scene/backward.rs`) and its completeness. |
| [gpu-cylinder-planning.md](gpu-cylinder-planning.md) | Planning on the GPU (`src/scene/plan_gpu.rs`), and on the web. |
| [deep-zoom-precision.md](deep-zoom-precision.md) | Past f32: replay in offsets from reference orbits. |
| [forward-bounds.md](forward-bounds.md) | Forward bounds for non-affine maps (phases 1-3 built; colour and xaos not). |
| [word-editing.md](word-editing.md) | Editing words: trim, removals, solo, the Paths panel and PathMap colouring. |
| [ifs-general.md](ifs-general.md) | The general IFS machinery the planners share (`InverseDef`, `src/scene/ifs_real.rs`). |

Superseded predecessors are archived:
[ifs-measure-by-inverse-walk.md](../../archive/projects/ifs-measure-by-inverse-walk.md)
and [ifs-perturbation-delta.md](../../archive/projects/ifs-perturbation-delta.md).
