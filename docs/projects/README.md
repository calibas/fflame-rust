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
| [api-updates.md](api-updates.md) | Every API update made so older apps keep working: steps 1-3 and 5 built and merged (the reader parameter, the server catch-up, the catalog, the config cleanups); step 4 (Rhai kinds as scripts online) deferred. [api-catalog-spec.md](api-catalog-spec.md) is the catalog as built. |
| [size-study.md](size-study.md) | Measured 2026-11-11: build time, binary size, repository. The profile (fat LTO, one codegen unit) is the build-time lever, not the content; 547 variations as downloads would save ~10% of the web download; the repository's weight is binary history. Seven decisions. |
| [v0.6.0-release.md](v0.6.0-release.md) | The v0.6.0 release's checklist (rewritten 2026-11-12): the testing pass, what is left of the API, the modules, the release itself. |
| [release-streamlining.md](release-streamlining.md) | Planned 2026-11-12, to build last before 0.6 and test with it: `release.py` stages (preflight, sync-api, deploy-api, deploy-web, publish, ship) for smaller, more frequent releases. Six decisions. |

## Standing references

| Doc | What it is |
|---|---|
| [VARIATIONS_WIRE_FORMAT.md](VARIATIONS_WIRE_FORMAT.md) | The variation wire format shared with the API repository. |
| [macos-release-checklist.md](macos-release-checklist.md) | The macOS release procedure, called from [docs/RELEASE.md](../RELEASE.md). |
