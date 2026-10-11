# Documentation

The docs for the Fractal Art Editor: a GPU fractal renderer with three
engines -- fractal flames (2D and 3D), escape-time fractals and
simulations -- sharing one tone mapping, effect, export, animation and
scripting tail.

Start with [ARCHITECTURE.md](ARCHITECTURE.md) for the overview and the
module map. For an AI assistant, the quick reference is
[../CLAUDE.md](../CLAUDE.md).

## The folders

| Folder | What is in it |
|---|---|
| [main/](main/) | The reference docs: how each system works now. Kept in step with the code; when one disagrees with the code, the code wins and the doc is the bug. |
| [projects/](projects/) | Work in progress: plans being built, paused projects and open lists ([README](projects/README.md)). |
| [experimental/](experimental/) | Ideas and research not being worked on ([README](experimental/README.md)). |
| [archive/](archive/) | Finished, superseded and abandoned plans, and the history of how things came to be ([README](archive/README.md)). |
| [tutorials/](tutorials/) | User tutorials ([README](tutorials/README.md)). |
| [generated/](generated/) | Files written by tools: the engine contract (a committed, test-checked artifact) and the variation probe and census reports. |

## The reference docs

| Engine or area | Docs |
|---|---|
| Flames | [RENDERER](main/RENDERER.md) - the pipeline, persistent orbits, the governor, solid rendering · [SHADERS](main/SHADERS.md) - shader assembly, sticky shaders, lints, cross-platform rules · [BUFFERS](main/BUFFERS.md) - bindings and GPU structs · [TRANSFORMS](main/TRANSFORMS.md) - the flame model · [VARIATIONS](main/VARIATIONS.md) - the variation system · [FOCUSED-RENDERING](main/FOCUSED-RENDERING.md) - flame deep zoom |
| Escape-time | [ESCAPE](main/ESCAPE.md) - formulas, colourings, deep zoom by perturbation, mode D, the camera lens |
| Simulations | [SIMULATION](main/SIMULATION.md) - the engine · [SIMULATION-CATALOG](main/SIMULATION-CATALOG.md) - the models |
| 3D views | [TERRAIN](main/TERRAIN.md) - terrains, render tiers, the path tracer, materials · [CAMERA](main/CAMERA.md) - every camera |
| Shared tail | [COLOR](main/COLOR.md) · [TONE-MAPPING](main/TONE-MAPPING.md) · [EFFECTS](main/EFFECTS.md) · [EXPORT](main/EXPORT.md) · [ANIMATION](main/ANIMATION.md) |
| App | [CONFIG](main/CONFIG.md) - config, undo, settings, storage · [UI](main/UI.md) - panels, menus, input · [FRACTAL-BROWSER](main/FRACTAL-BROWSER.md) - the Fractal Browser · [PALETTE_LIBRARY](main/PALETTE_LIBRARY.md) · [I18N](main/I18N.md) · [ONLINE](main/ONLINE.md) - the API client, catalogs, plugins |
| Scripting | [SCRIPTING](main/SCRIPTING.md) - the Rhai API reference · [SCRIPTING-GUIDE](main/SCRIPTING-GUIDE.md) - the language guide and examples |

The server API's description is [main/openapi.json](main/openapi.json).

## Procedures

- [RELEASE.md](RELEASE.md) - releasing, the gates (`python scripts/release.py check`), and changing anything the API sees.
- [TESTING-GUIDE.md](TESTING-GUIDE.md) - unit tests, the visual suite, benchmarks, the probe and census.
- [WASM.md](WASM.md) - building and running the web app and the gallery modules.
- [accepted-divergences.txt](accepted-divergences.txt) - the cross-platform differences accepted on purpose.

## Keeping the docs honest

- `python scripts/check_doc_links.py` reports broken links in the live
  docs (and, separately and without failing, in the archive); it is one
  of the release gates.
- Finished project docs move to `archive/projects/` with
  `scripts/move_docs.py`, which also rewrites the references to them in
  code comments.
- SCRIPTING.md is checked by a test (`every_script_api_name_is_documented`).
