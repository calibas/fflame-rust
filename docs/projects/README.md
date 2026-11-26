# Projects

Work in progress: plans being built, paused projects, open backlogs and
the trackers they feed. How a finished system works is in
[docs/main/](../main/); ideas not yet started are in
[docs/experimental/](../experimental/); finished plans go to
[docs/archive/projects/](../archive/projects/) under the same name, with
a banner saying what happened (move them with `scripts/move_docs.py`,
which rewrites the references in code comments too).

## The list

[BACKLOG.md](BACKLOG.md) -- everything still open, in one place
(consolidated 2026-11-11): decisions waiting on the user, then open items
by area, each linking the doc with the detail (finished and paused docs in
[docs/archive/](../archive/), plans not started in
[docs/experimental/](../experimental/)).

## Active

| Doc | Status |
|---|---|
| [size-study.md](size-study.md) | Measured 2026-11-11: build time, binary size, repository. The profile (fat LTO, one codegen unit) is the build-time lever, not the content; 547 variations as downloads would save ~10% of the web download; the repository's weight is binary history. Seven decisions. |
| [v0.6.0-release.md](v0.6.0-release.md) | The v0.6.0 release's checklist (rewritten 2026-11-12): the testing pass, what is left of the API, the modules, the release itself. |
| [curated-content.md](curated-content.md) | Built 2026-11-18 on both repositories and merged (PR #172 here): curated items as frozen, canonically named copies for every kind; browsing is Curated or the account's own, anything else by id; curated code trusted by hash; configs name curated items by canonical name; every kind synced by hash through one API import. Left: the production dry run, with the API's deploy in the 0.6 release. |
| [rename.md](rename.md) | Planned 2026-11-25, all before 0.6: the API's `/flames` becomes `/fractals`, the repository `fractal-art-editor`, the crate, gallery modules and Python package `fae` / `fae-render` / `fae-script` / `pyfae`, and configs `.fae` (`.fflame` still read). Six steps across the three repositories. |
| [release-streamlining.md](release-streamlining.md) | Planned 2026-11-12, to build last before 0.6 and test with it: `release.py` stages (preflight, sync-api, deploy-api, deploy-web, publish, ship) for smaller, more frequent releases. Decisions made 2026-11-24. |

## Standing references

| Doc | What it is |
|---|---|
| [VARIATIONS_WIRE_FORMAT.md](VARIATIONS_WIRE_FORMAT.md) | The variation wire format shared with the API repository. |
| [api-catalog-spec.md](api-catalog-spec.md) | The engine catalog as built: what the API needs to summarise a saved config as the client does (`docs/generated/engine-catalog.json`, `src/catalog.rs`). |
| [macos-release-checklist.md](macos-release-checklist.md) | The macOS release procedure, called from [docs/RELEASE.md](../RELEASE.md). |
