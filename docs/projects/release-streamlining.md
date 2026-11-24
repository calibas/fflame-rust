# Streamlining the release

**Status 2026-11-12:** plan, not built. The user's call: build it as the
last item before 0.6, and test it with the 0.6 release itself
([v0.6.0-release.md](v0.6.0-release.md)). **All six decisions made
2026-11-24 (§3).** **Steps 1-3 of §4 built 2026-11-24** on branch
`release-stages`: every stage in §2's table, plus `sync-web`, which copies
the editor page into ffbrowser. The local stages ran for real: `sync-api`
synced ffapi on its branch `app-latest-github`. The remote stages
(`deploy-api`, `deploy-web`, `publish`) have only run as dry runs and
against stand-ins; their first real run is 0.6.0 (step 4).

**The goal, in the user's words:** "smaller, more frequent updates,
different from the enormous changes we've made between 0.5.0 and 0.6.0."

The API work removed the riskiest part of a release. A config says the
lowest version that reads it (`min_reader`), and every request says what
it reads (`?reader=N`), so a release no longer has to worry about breaking
older apps. What is left is how many steps a release takes, across three
repositories and two machines, all by hand.

---

## 1. A release today

### The app (this repository; [RELEASE.md](../RELEASE.md) §4)

1. `release.py version X.Y.Z`, then `release.py changelog > NOTES.md`,
   and edit the notes.
2. `release.py check`, then the visual suite and the benchmarks (separate,
   needing a GPU).
3. The per-platform reports, when variation math changed.
4. `release.py build` on **each** machine: Windows for the zip, macOS for
   the bundle.
5. A smoke test of each artifact.
6. `git tag` and `git push` by hand.
7. `gh release create` by hand, with both zips collected onto one machine.

### The web app and museum (`ffbrowser`; its `docs/DEPLOYMENT.md`)

1. Build `pkg/` (`build-wasm.sh`) and the two gallery modules
   (`wasm-pack` in `wasm/render`, `wasm/script`).
2. Three `rsync`s into the server's `ffbrowser/public/`.
3. `ssh`, then `npm run build` on the server.
4. A hard refresh, and hope the upload was whole.

### The API (`ffapi`; its `docs/deployment-ubuntu.md`)

1. Copy the generated files and corpora into `ffapi` and regenerate its
   seed migrations ([RELEASE.md](../RELEASE.md) §3).
2. Run its tests.
3. Build on the server, copy the binary, restart (migrations run on
   start).
4. Run the built-in script import by hand when the scripts changed.
5. Set `APP_LATEST_VERSION` for the update notice.

### What goes wrong

- **About twenty steps, and nothing checks that any of them ran.** The
  museum's WASM modules and editor copy sat at 2026-08-12 / 08-19 while
  the app moved through config v7, escape-time and simulations
  (ffapi's `docs/BROWSER_APP_0.6.md` §1.2).
- **Order matters and is written down only in prose:** the server before
  an app that uploads simulations; the script import after the
  migrations.
- **Copies drift:**
  - `ffbrowser` keeps the editor's `index.html`, `css/` and `js/` as its
    own tracked copy (only `pkg/` is rsynced);
  - `src/museum/wasmTypes.ts` hand-mirrors the modules' `.d.ts`;
  - the API's generated files are copied by hand.
- **Every desktop release needs both machines.**
- **Was undecided** (now §3): the tag form (`0.5` was lightweight;
  RELEASE.md shows an annotated `v0.5.0`) and what a patch release is.

---

## 2. The plan: `release.py`, one subcommand a stage

Each stage is a subcommand usable on its own. `ship` runs them in order,
stops at the first failure, pauses where a person must look, and has
`--dry-run`. Nothing pushes, deploys or publishes without a confirmation
prompt that names what it is about to do.

| Subcommand | What it does | Today |
|---|---|---|
| `preflight` | All three repositories (`fflame-rust`, `ffapi`, `ffbrowser`, found beside this one) clean, on `main`, level with `origin`; versions agree; `gh` and `ssh` reachable | by eye |
| `version X.Y.Z` | as now | exists |
| `notes` | `changelog` grouped by area, written to `NOTES.md` for editing | exists as `changelog` |
| `check` | as now, plus **the visual suite** and **the API integration test** (`scripts/api_integration.py`) as gates | partly |
| `sync-api` | Copy the contract, the catalog and the corpora into `../ffapi`; run its generators (effects, variations, script import); run its conformance tests; show the diff; leave the commit to the user | by hand |
| `build` | as now | exists |
| `deploy-api` | Over SSH, per ffapi's deployment doc: pull, build, swap the binary, restart; wait for `/health`; check `/api/app/latest`; run the script import if `sync-api` regenerated it | by hand |
| `deploy-web` | Check the builds' `version.json` against HEAD; upload `pkg/` and the gallery modules into the server's `ffbrowser/public/` (tar over ssh, each swapped in whole: no rsync on Windows); build on the server; then fetch the served editor and modules and check they report the new version. The editor's `index.html`, `css/`, `js/` are ffbrowser's tracked files, so a local `sync-web` copies them there for a commit, and `deploy-web` refuses while they differ | by hand |
| `publish` | Annotated tag `vX.Y.Z`, push it, `gh release create` with whichever zips are present; the server's update notice follows the GitHub release by itself (§3, decision 4) | by hand |
| `ship X.Y.Z` | `preflight` → `version` → `notes` (pause to edit) → `check` → `sync-api` → `build` → pause for the smoke test → `deploy-api` → `deploy-web` → `publish` | n/a |

### Details that make it trustworthy

- **A version a deploy can be checked against.** `build-wasm.sh` and the
  gallery builds write `version.json` beside their output (the version,
  the git hash, the profile), and `deploy-web` reads it back from the
  live site. That catches the half-upload RELEASE.md §7 worries about.
- **Order is code:** `deploy-api` before `deploy-web`, and `publish`
  last, so the update notice never announces a build that isn't up.
- **Secrets stay out of the repository:** the host and key come from the
  environment or an untracked `release.local.toml`, never from the docs.
  The deployment docs that already name them stay where they are.
- **`ffbrowser`'s `wasmTypes.ts`:** `deploy-web` diffs the new `.d.ts`
  files against the previous ones and stops if a signature changed, so
  the mirror is updated before the deploy rather than after a broken
  page.
- **Two machines:** `publish` uploads whichever zips exist, so a release
  can go out from Windows with the macOS zip added later
  (`gh release upload`).

---

## 3. Decisions (the user, 2026-11-24)

1. **Release kinds: one.** No web-only patch version. Every release,
   minor or patch, ships the desktop and the web together, and patches
   will be frequent on both. (`publish` still uploads whichever zips are
   present, so the macOS zip can follow the Windows one.)
2. **Version policy, as proposed.**
   - `0.6.x` while configs stay readable by 0.6 (no `min_reader` bump).
   - The next minor (`0.7`) when a release raises `min_reader`, or
     changes something older apps would need to know.
3. **Tag form, as proposed:** annotated `vX.Y.Z` from 0.6 on, leaving
   `0.5` as it is.
4. **The update notice reads GitHub.** App versions are tracked as GitHub
   releases, and the server reads the newest one itself (cached), so
   nothing has to be set by hand and it can't announce a version before
   its release exists. Built 2026-11-24 on ffapi's branch
   `app-latest-github`: `APP_RELEASES_REPO` (`owner/name`) turns it on,
   and `APP_LATEST_VERSION` still overrides it when set.
5. **How far `release.py` reaches, as proposed.**
   - It copies files and runs their tools and tests in `ffapi`, and
     deploys `ffbrowser`'s already-pushed `main`.
   - It never commits in either: commits stay the user's, with a printed
     summary.
6. **CI: none yet.** Maybe later, once the release is streamlined; the
   automated tests aren't trusted enough yet to gate on their own.
   `release.py` remains the thing that runs everything on demand.

---

## 4. Order of building, and testing it with 0.6

1. `preflight`, `sync-api`, and the extra gates in `check`: no remote
   side effects, so they can be built and run any time.
2. `version.json` in the builds, then `deploy-web` and `deploy-api`,
   first tried against the live site with the 0.6 deploy itself.
3. `publish`, then `ship` gluing it together. (On the server,
   `/app/latest` reading the newest GitHub release, decision 4: built.)
4. **The 0.6 release is the test:** run `ship 0.6.0` stage by stage.
   Fix what breaks, and correct RELEASE.md to describe what actually ran.
5. Then use `ship 0.6.1` for the first patch, to see whether a small
   release is really small.
