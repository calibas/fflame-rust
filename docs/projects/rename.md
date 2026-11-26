# Renaming: fractals, Fractal Art Editor, `.fae`

**Status 2026-11-26: done here and in ffapi.** Steps 1-5 merged (ffapi
`fractals`, this repository's `rename`); the GitHub repository and the
local folder are renamed (step 6). Left, elsewhere:
- ffbrowser's changes (module imports, `wasmTypes.ts`, `.fae` downloads,
  DEPLOYMENT.md's paths): the browser session's, listed in ffapi's
  BROWSER_APP_0.6.md §1.2;
- the server's `APP_RELEASES_REPO=calibas/fractal-art-editor`, set with
  the 0.6 deploy (release-streamlining.md §5);
- the macOS clone: `git remote set-url origin
  https://github.com/calibas/fractal-art-editor.git`, and its folder if
  wanted.

Archive this once ffbrowser's part has landed.

**As built, beyond the plan:**
- ffapi's stored entity type follows the wire (migration 122:
  `reports`, `moderation_actions`, `curated_names`), so code, wire and
  database all say `fractal`; tables and columns keep their names.
- The OpenAPI operation ids and tag are set in the attributes
  (`get_fractal`, tag `fractals`), not by renaming Rust items.
- Every tracked `.fflame` was renamed except the six in
  `tests/visual/configs/2d/`: the visual configs, `assets/presets.fae`,
  the texture recipes, test configs and the script-parity fixtures.
- The deep-zoom diagnostics keep reading the local corpus
  `output/flame-zoom/*.fflame` under its names.
- A `FFLAME` variable some ignored diagnostics read (a config file's
  path) is unchanged.
- The presets file has no old-name fallback: it ships with the binary
  that reads it.

**Why:** the app has grown past a flame renderer. It has three engines,
and the flame is one of them. The names still say "flame" where they mean
any picture the app makes:
- the API's `/flames`;
- the repository `fflame-rust`;
- the crate `fractal_flame_wgpu`;
- the file extension `.fflame`.

---

## 1. Decisions (the user, 2026-11-25)

| What | From | To |
|---|---|---|
| The API's resource | `/flames`, `flame_id`, ... | **`/fractals`**, `fractal_id`, ... |
| The repository | `calibas/fflame-rust` | **`calibas/fractal-art-editor`** |
| The library crate | `fractal_flame_wgpu` (web build: `fractal_flame_wgpu.js`) | **`fae`** (`fae.js`, `fae_bg.wasm`) |
| The gallery modules | `fflame-render`, `fflame-script` | **`fae-render`, `fae-script`** |
| The Python package | `pyfflame` (crate `fflame-python`, module `fflame.wasm`) | **`pyfae`** (`fae-python`, `fae.wasm`) |
| The config file | `.fflame` | **`.fae`** |
| When | | **All of it before 0.6** |

The new names for the crate, modules and Python package follow §3's
recommendation. The user agreed that all three get renamed.

## 2. Compatibility: what is kept, and why

**Why it can all happen now:** `/api/v1` is new in 0.6, and the 0.6
server answers nowhere else (`build_app` nests every route under
`/api/v1`). 0.5 apps and today's live browser already break against it.
So no released client calls `/api/v1/flames`. Renamed now, the API and
the gallery modules change once. Renamed after 0.6, the server would keep
aliases for as long as 0.6 apps are in use.

| Surface | Kept | Why |
|---|---|---|
| `.fflame` files | **Read forever**, alongside `.fae` and `.flame` | Every saved config, preset and shared file in the world |
| `/api/v1/flames` | **Nothing**: no alias | No released client uses it (above) |
| `engine: "flame"` | **Unchanged** | It names the flame engine, which still exists |
| The API's database tables (`flames`, `flame_components`, `flame_tags`, `collection_flames`) and its Rust names | **Unchanged for now** | Invisible to users. A table-rename migration can come later, on its own. |
| The web app's localStorage keys (`fflame...`) | **Unchanged** | Invisible to users. Changing them would lose browser users' saved settings and palettes, or need a migration. |
| The desktop data folder | **Unchanged** | Already the product name (`Fractals for All\Fractal Art Editor`); a test keeps internal names out of it |
| PNG metadata | **Unchanged** | No `fflame` in it: the config is the `Config` chunk, and the app is `Software: Fractal Art Editor` |
| Old GitHub URLs (0.5's Help menu, links in docs) | **GitHub redirects them** | As long as no new repository takes the old name |
| `FFLAME_*` environment variables | **Read as a fallback for one release**, then dropped | Development and build only |

## 3. Order of work

**Before starting:** merge `release-stages` here and `app-latest-github`
in ffapi. `release.py`, RELEASE.md and the deploy docs are all touched
again below. The browser work in progress is written against `/flames`,
so tell that session the new paths when step 1 lands.

### Step 1: the API (ffapi, branch `fractals`)

- **Routes:**
  - `/flames` → `/fractals`, with its item routes: thumbnail, favorite,
    tags, animations;
  - `/users/{id}/flames` and `/users/me/flames` → `.../fractals`;
  - no aliases.
- **Wire names:**
  - `flame_id` → `fractal_id`;
  - a collection item's `flame` → `fractal`;
  - error codes `flame_ref_invalid`, `flame_id_invalid`,
    `exactly_one_of_item_or_flame` → `fractal_...`;
  - every other `flame` word in a request or response, found by grep over
    `models/` and `routes/`, except `engine: "flame"`.
- **OpenAPI:**
  - the schema names the browser generates its types from
    (`FlameResponse` and the rest → `Fractal...`), with `#[schema(as = ...)]`
    where renaming the Rust type would ripple into SQL;
  - the tag names;
  - `routes/meta.rs`'s list of routes, which `meta_access.rs` holds to the
    routes.
- **Tests:** every path string, including `permissions_matrix.rs`.
- **Docs:** BROWSER_APP_INTEGRATION.md, BROWSER_APP_0.6.md,
  BROWSER_APP_CATALOG_API.md, catalog-api-build.md, CLAUDE.md's route
  list.
- **On the same branch, if wanted:** the `kind:*` type filter, which the
  browser asked for. `field:*` lists every config with a field, of any
  origin: built-in, Rhai script or source. The browser's version kept
  `origin = 'builtin'` and would have missed Rhai fields. With
  `role=primary`, it lists the configs whose primary type is that kind:
  every Field config, every Formula config. Its own commit and test.

### Step 2: the client's API calls (here, branch `rename`)

- `src/api/`: every `/api/v1/flames...` and `/users/me/flames` path; the
  wire structs' renamed fields (`flame_id`, ...).
- `api::integration_tests`: run by `scripts/api_integration.py` against
  step 1's server.
- Nothing in the contract or the catalog names a route, so neither moves.

### Step 3: crate, modules, Python (same branch, one commit each)

- **The crate:**
  - `fractal_flame_wgpu` → `fae` (`[package] name`);
  - `use fractal_flame_wgpu::` in the bins, benches and tests, and in the
    gallery and Python crates' dependencies;
  - `build-wasm.sh` / `.bat` (`target/.../fae.wasm`, the output names);
  - `index.html`'s import (`./pkg/fae.js`);
  - add the new names to the internal-names test in `storage::backend`.
- **The gallery modules:**
  - `wasm/render` → package `fae-render` (`fae_render.js`), `wasm/script`
    → `fae-script`; their directories stay;
  - `release.py`'s gallery steps and `GALLERY_CRATES` stay, since they
    name directories;
  - in ffbrowser: the module imports and `src/museum/wasmTypes.ts`. Its
    `public/wasm/render|script` directories stay.
- **Python:**
  - `python/pyfflame` → `python/pyfae`, `fflame.wasm` → `fae.wasm`;
  - `wasm/python` → `fae-python`, and its exported C symbols
    (`fflame_alloc`, `fflame_call`, `fflame_free`, `fflame_output_*`,
    `fflame_info_*`) → `fae_*`;
  - `pyproject.toml`'s name, the tests' names, `PYFFLAME_WASM` →
    `PYFAE_WASM`;
  - `release.py`'s `VERSION_FILES` entry for `__init__.py`.
  - Not published anywhere, so nothing to keep.
- **Environment variables:** `FFLAME_*` → `FAE_*`, reading the old name as
  a fallback through one helper. That covers `FFLAME_API_BASE_URL` (and
  its `_CHOSEN_BY`, in `build.rs`), `FFLAME_API_URL` (also set by
  `api_integration.py`), `FFLAME_FRAME_LOG*`,
  `FFLAME_SKIP_GIT_PROVENANCE`, `FFLAME_PLAN_CPU` and
  `FFLAME_EXACT_COUNT_AFTER`.

### Step 4: `.fae` (same branch)

- **One helper**, `config::is_config_path` (`.fae` or `.fflame`), in place
  of the twelve-odd `extension() == "fflame"` checks:
  - `config/by_name.rs`, `save_dump.rs`, `scene/assets.rs`;
  - `census/run.rs`, `scene/backward.rs`, `scene/cylinder.rs`;
  - `textures`' `EXT`.
- **Writing:**
  - save dialogs default to `.fae`;
  - open dialogs list `.fae` and `.fflame` (and `.flame` where they do
    now);
  - their labels lose "Flame" ("Fractal Art Editor config");
  - `script/cli.rs`'s output;
  - the three locale strings (`empty_hint`, `save_as_fflame`,
    `load_fflame`).
- **`assets/presets.fflame` → `presets.fae`:** the embedded copy, the
  desktop's runtime read (the new name, then the old once), and the
  splice helpers' doc comments (`escape/bristor.rs`, `halayka.rs`,
  `legacy.rs`).
- **The visual suite's 393 configs:**
  - `git mv` to `.fae`, keeping the `2d/` six as `.fflame` so the legacy
    path stays tested;
  - `run_tests.py` globs both;
  - baselines are named by stem, so none change.
- **Python:** load and save take either extension and write `.fae`.
- **ffbrowser:** its three downloads write `.fae` (`WalkActions.tsx`,
  `FlameDetail.tsx`, `lib/download.ts`) and the `downloadTitle` string.
  That's the browser session's change.
- **No OS file association exists**, so nothing to register.

### Step 5: the name in text (all three repositories)

- `fflame-rust` → `fractal-art-editor`:
  - the Help menu's three GitHub links (`ui/help.rs`, `ui/menu_bar.rs`);
  - `storage/backend.rs`'s comment;
  - `release.py`;
  - README, both CLAUDE.md files, and the docs (16 files across the
    three repositories at last count);
  - ffbrowser's DEPLOYMENT.md (its `/cygdrive/c/projects/fflame-rust`
    paths).
- `.fflame` → `.fae` in the docs (CONFIG.md, EXPORT.md and the rest),
  keeping "(and `.fflame`)" where reading is described.
- ffapi: the server's `APP_RELEASES_REPO=calibas/fractal-art-editor`.

### Step 6: the repository itself (the user, then a fresh session)

1. Everything above merged and pushed, in all three repositories.
2. **GitHub:** Settings → General → Repository name →
   `fractal-art-editor`. GitHub redirects the old URLs.
3. Locally: `git remote set-url origin https://github.com/calibas/fractal-art-editor.git`.
4. **Before closing this session:** copy Claude's project memory to the
   folder's new key. It is stored by path:
   `.claude/projects/c--projects-fflame-rust/memory` →
   `c--projects-fractal-art-editor/memory`.
5. Close VS Code and every session that has the folder open, then rename
   `c:\projects\fflame-rust` → `c:\projects\fractal-art-editor`.
6. The first build there is a full one: build scripts' outputs record
   absolute paths.
7. The macOS clone: the same `set-url`, and its folder if wanted.

## 4. Checks

- Step 1: ffapi's full suite, and the browser told.
- Step 2: `scripts/api_integration.py` against step 1.
- Steps 3-4:
  - `release.py check --full`: every fast gate, the visual suite on the
    renamed fixtures, and the API test;
  - `release.py gallery`;
  - `build-wasm.sh`, then the editor page loaded;
  - open and save a `.fflame` and a `.fae` in the app.
- Step 6: `release.py preflight` from the renamed folder.

## 5. Not in this plan

- **UI wording that says "flame" for any config** (menu items, dialog
  titles, the browser's labels): a wording pass of its own, after this.
- **The API's table names:** later, on their own (§2).
