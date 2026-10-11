# Documentation archive

The record, not the reference. These are finished, superseded and
abandoned plans, investigations and measurements: why things are the way
they are, and what was tried and did not work. For how a system works
now, read [docs/main/](../main/); for what is in progress,
[docs/projects/](../projects/); for ideas not yet started,
[docs/experimental/](../experimental/).

An archived document records what it said when it was written. Its links
and code references may point at things that have since moved or gone,
and that is expected: `scripts/check_doc_links.py` reports broken links
in here separately and does not fail on them. Docs archived from
2026-10-10 on carry a banner under their title saying what happened and
which live doc to read instead.

## Conventions

- A finished project doc moves from `docs/projects/X.md` to
  `docs/archive/projects/X.md` under the same name, with the banner.
- When several related docs retire together, they get a topic folder
  with a README saying what each one is.
- Move docs with `scripts/move_docs.py`, which also rewrites every
  reference to them -- code comments included.

## Folders

| Folder | What it holds |
|---|---|
| [projects/](projects/) | Finished project plans, grouped by area in its [README](projects/README.md) |
| [escape-time/](escape-time/) | The escape-time engine's design record, its finished plans and source conversations ([README](escape-time/README.md)) |
| [terrain/](terrain/) | The 3D height-field terrain and its height quality ([README](terrain/README.md)) |
| [3d-camera/](3d-camera/) | The 4-angle camera, fly mode, and the camera unification ([README](3d-camera/README.md)) |
| [experimental/](experimental/) | Proposals since implemented or overtaken ([README](experimental/README.md)) |
| [histogram/](histogram/) | How colour accumulation evolved from textureStore to u32, and the optimisation attempts that failed ([README](histogram/README.md)); also the obsolete COLOR_PIPELINE.md |
| [api-v1-v2/](api-v1-v2/) | The first API integration and the v2 wire format ([README](api-v1-v2/README.md)) |
| [delta-migration/](delta-migration/) | The move to delta-based state and lazy undo ([README](delta-migration/README.md)) |
| [optimization-attempt-2025-11-02/](optimization-attempt-2025-11-02/) | Shader precalculation and why the compiler's CSE made it moot ([README](optimization-attempt-2025-11-02/README.md)) |
| [apophysis-phase3/](apophysis-phase3/) | Apophysis colour, brightness and XML compatibility work |
| [xaos/](xaos/) | Xaos support |
| [transform-color/](transform-color/) | The transform colour system's redesign |
| [undo-redo-improvements/](undo-redo-improvements/) | Undo/redo snapshots and their issues |
| [state-centralization/](state-centralization/) | Centralised update logic |
| [gpu-profiling/](gpu-profiling/) | GPU usage investigations, including the idle-usage fix |
| [per-pixel-convergence/](per-pixel-convergence/) | Per-pixel adaptive scaling, tried and abandoned |

## Loose files

The files directly in this folder are older records, mostly from 2025:
status and milestone notes (STATUS, outline, MILESTONE-7-COMPLETE,
CHANGELOG), the WASM bring-up (README-WASM, QUICKSTART-WASM,
WASM-TROUBLESHOOTING, WASM-FIXED, WASM-PROFILING), investigations
(BRIGHTNESS, QUALITY, ZOOM performance, iterations-per-thread quality,
convergence diagnostics), the variation system's earlier designs
(VARIATION-SYSTEM-V2, VARIATION-PARAMETERS-PLAN, packed-variation-params,
variation-init-dispatch, intra-iteration-state-and-accum, global
registries), shader-system history (SHADER-SYSTEM, shader-builder-v3,
shader consolidation and DC gating, shader performance), config and
parameter migrations, subflames (subflame-variations-v2,
subflame-live-editing), supersampling, animation details, and the
superseded testing plan (TESTING-PLAN, VISUAL_BASELINE_TESTING).
