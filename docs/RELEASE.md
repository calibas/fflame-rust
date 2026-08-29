# Release and update procedure

Every procedure that has to run, in order, and what each one gates. This
exists because the procedures were real but scattered — `build-wasm.bat`,
`run_benchmarks.py`, `run_tests.py`, three regeneration env vars — with
no single place to see them, and nothing saying which are required for a
release rather than merely available.

**Read §4 to run a release.** Everything else is why.

Three things are deliberately still open, and each says so where it
lives: code signing on both desktop platforms (§5), Linux and PyPI
packaging (§5), and how a user learns an update exists (§4b). None of
them block shipping; all of them are recorded rather than left implicit,
because the gap between "we build it" and "someone can install it" is
where a release actually stalls.

---

## 1. What ships

Four artifacts, built four different ways, from one repository.

| surface | built by | output | version source |
|---|---|---|---|
| Desktop app | `cargo build --profile dist` | `target/dist/FractalArtEditor(.exe)` | `Cargo.toml` |
| Web app | `build-wasm.bat` / `.sh` — `--profile dist` | `pkg/` + `index.html`, `css/`, `js/` | `Cargo.toml` |
| Gallery modules | `wasm-pack build` in `wasm/render`, `wasm/flame`, `wasm/escape`, `wasm/script` | `wasm/*/pkg/` | each crate's `Cargo.toml` |
| Python | `maturin build` in `python/` | a wheel | `python/pyproject.toml` |

### The version problem

There are **four independent version numbers** and nothing relates them:

```
Cargo.toml            0.4.4     ← desktop + web app, and what PNG metadata records
python/pyproject.toml 0.1.0
wasm/render           0.1.0
wasm/script           0.1.0
```

A user reporting "0.4.4 crashes" is describing the app. A user reporting
"pyfflame 0.1.0 gives a different flame" is describing a build that could
have come from any app version, because nothing records which.

**Decided: lockstep.** One version for everything, bumped together.
`release.py version` implements it — it sets all four files or none, and
says so when it runs.

The artifacts are not independently useful: a `pyfflame` wheel and the
app it matches come from one commit, and pretending otherwise creates a
compatibility matrix nobody will maintain. The cost is a version bump on
crates that did not change, which is cheap and visible.

The alternative considered was independent versions with each crate
recording the app version it was built from (the way `build.rs` records
the git hash). It buys accuracy nobody needs at the price of the matrix.
The first `version` bump makes lockstep true — until then the numbers
above are the historical drift, not a scheme.

---

## 2. Gates — what must pass

```bash
python scripts/release.py check          # ~4s: tests, wasm, contract, dumps, doc links
python scripts/release.py check --fix    # ...and regenerate what is stale
```

**Nothing runs on push, on commit, or on a timer.** That is deliberate:
the work CI would do is worth automating, the *triggering* is what gets
in the way. `release.py` is a command you type when you are releasing —
or when you want to know whether you could.

Two gates it does NOT run, because they need a GPU and minutes rather
than seconds:

```bash
cargo build --release && python tests/visual/run_tests.py
python scripts/run_benchmarks.py --quick
```

Three of the tests are **generated-artifact gates** — they fail when a
committed file no longer matches what the code produces, and each has an
env var to regenerate:

| gate | regenerate with |
|---|---|
| `contract_is_current` | `UPDATE_CONTRACT=1 cargo test --lib contract_is_current` |
| `canonical_shader_dumps` | `UPDATE_SHADER_DUMPS=1 cargo test --lib canonical_shader_dumps` |
| `every_shipped_effect_has_a_curated_label` | add the missing `locales/en.yml` entry |

**Regenerate deliberately, and read the diff.** These exist because a
stale generated file fails *silently* otherwise — the whole point is that
somebody looks.

---

## 3. Syncing with the API

The API is a separate repository. What it consumes from here:

| artifact | produced by | when it changes |
|---|---|---|
| `docs/generated/engine-contract.json` | `UPDATE_CONTRACT=1 cargo test --lib contract_is_current` | vocabularies, engine limits, reserved script stems |
| the variation corpus | `cargo run --release --bin export_variations_json` | any variation added or edited |
| the script corpus | `cargo run --release --bin export_scripts_json` | any built-in script added or edited |
| the effect corpus | `cargo run --release --bin export_effects_json` | any effect added or edited |

The variation and script corpora carry the prose too — descriptions and
authors from `///` doc comments, script descriptions from the header
comment. Both refuse to write a file with a missing description rather
than emitting a null, because the API has no other source for it and a
null becomes a variation that reaches the browser with nothing to say.
The script corpus embeds each script's **source verbatim**, so it is
also what a re-import would have to reproduce byte for byte.

### What moves the contract's `shape`, and what does not

The API pins `shape` so a stale copy of the contract fails loudly rather
than under-checking. Measured, not assumed:

| change | `shape` moves? | API action |
|---|---|---|
| new field / limit | **yes** | re-pin deliberately |
| new variation | no | re-import the corpus; no re-pin |
| **new `Feature`, category, or param type** | **no** ← | see below |

**A new vocabulary entry does not move the fingerprint.** `key_paths`
walks structure, and a new feature is one more element in `features[]`
with the same keys. So the API's pin does not fire, and a stale contract
copy still passes its conformance check while silently missing the new
word — the exact failure the fingerprint exists to prevent, surviving in
the one dimension the contract exists to convey.

**Until that is fixed** (a second fingerprint over vocabulary *values*),
adding a `Feature`, a `VariationCategory` or a `ParamType` requires
telling the API repository directly. Do not rely on the pin.

Also update `docs/main/openapi.json` when the client's expectations of
the API change — it is the wire authority for both repositories, and it
has gone stale before.

---

## 4. Release procedure

Ordered, and every step is a command you type. Nothing fires on push or
on a timer.

### Before you start

You need **both machines**. There is no cross-compilation here: the
macOS bundle needs macOS (plistlib layout + `codesign`), and the Windows
build needs Windows. Decide which is the *authoring* machine for the
generated reports (§4.3) and do the tag there.

### 4.1 — Version and notes

```bash
python scripts/release.py version 0.5.0 --dry-run   # read it first
python scripts/release.py version 0.5.0             # all four crates, lockstep
python scripts/release.py changelog > NOTES.md      # raw material, then edit it
```

The changelog is raw material, and `NOTES.md` is the edited result you
hand to `gh` in §4.7. Grouping by `area:` prefix gets the shape;
deciding what mattered to a *user* is still a person's job — most
commits here are internal and belong in one line, not thirty. Expect to
cut most of it.

`NOTES.md` is **scratch, per release, and not committed** — it is in
`.gitignore` for that reason. The published notes live on the GitHub
release; the repository's record of what changed is the commit history
the changelog was generated from.

**On the version numbers.** Nothing has been tagged yet, so
`release.py changelog` has no "since the last tag" to work from until
the first tag exists. The first release should tag whatever it ships,
even if the number is arbitrary, so every release after it has a
boundary to diff against.

### 4.2 — Gates

```bash
python scripts/release.py check     # ~4s: tests, wasm, contract, dumps, doc links
```

Then the two it deliberately does not run, because they need a GPU and
minutes:

```bash
cargo build --release && python tests/visual/run_tests.py    # 148 tests
python scripts/run_benchmarks.py --quick
```

Regenerate any artifact a gate demands, and **read the diff** — a stale
generated file fails a test rather than failing silently, and that is
the whole point.

### 4.3 — Per-platform generated reports

These describe *the machine that ran them*, so they are per-platform
files and each machine rewrites only its own:

```bash
cargo run --release --bin variation_probe            # if variation math changed
cargo run --release --bin variation_probe -- census --corpus
```

- `docs/generated/variation-probe*.txt` — shader math, one authoring
  platform (currently Windows).
- `docs/generated/variation-census-{windows,macos}.txt` — what a corpus
  of real flames feeds each variation, **per platform**, because it is a
  measurement of that GPU.
- `variation_probe rank <old> <new>` joins them into a worklist;
  `docs/accepted-divergences.txt` records what has been examined and
  deliberately left alone. A release should not carry unexamined
  REACHABLE entries.

### 4.4 — Build

```bash
python scripts/release.py build     # every surface, in order
```

Host-specific packaging runs automatically as the second step — the
`.app` bundle on macOS, the zip on Windows. Individually:

| surface | command | where |
|---|---|---|
| desktop | `cargo build --profile dist` | both |
| macOS bundle | `python3 scripts/make_macos_app.py --zip` | macOS |
| Windows zip | `python scripts/make_windows_zip.py` | Windows |
| web app | `./build-wasm.sh` / `build-wasm.bat` (they build `--profile dist`) | either |
| gallery modules | `wasm-pack build --target web --release` in `wasm/render`, `wasm/flame`, `wasm/escape`, `wasm/script` | either |
| python wheel | `maturin build --release` in `python/` | per-platform wheel |

`--profile dist`, never `--release`, for **desktop and web alike** — §4c
says why, and has the measured numbers for each. The gallery modules are
the exception: their own `[profile.release]` already sets everything
`dist` would, plus `wasm-opt`. §5 has the detail, including the two
test suites no gate runs.

### 4.5 — Smoke-test each artifact

Building is not shipping.

- **desktop (both platforms):** launch it, check the Performance panel
  says the new version and `dist`, export a PNG, confirm the same in its
  metadata. On macOS launch the **bundle**, not the loose binary —
  Finder's working directory is `/`, which is what asset loading and the
  video default path both depend on.
- **macOS additionally:** `docs/projects/macos-release-checklist.md`
  Tier 2 — the interactive surface (audio, video export, dialogs,
  shortcuts, DPI). Most of it only needs re-running when something in
  that area changed.
- **web:** serve `pkg/`, load in Chrome and Firefox. Check undo (⌘/Ctrl+Z)
  and one export.
- **python:** `import pyfflame`, run a script, compare against the app.
- **gallery modules:** `wasm/README.md` has an end-to-end example.

### 4.6 — Tag

```bash
git tag -a v0.5.0 -m "0.5.0"
git push origin v0.5.0
```

Every exported PNG records the git hash, so the tag is what makes a bug
report traceable to a source tree. Tag *after* the gates and *before*
publishing, so the artifacts and the tag agree.

### 4.7 — Publish

**GitHub release** (Windows + macOS):

```bash
gh release create v0.5.0 \
   --title "0.5.0" --notes-file NOTES.md \
   "target/windows/FractalArtEditor-0.5.0-windows.zip" \
   "target/macos/FractalArtEditor-0.5.0-macos.zip"
```

Both zips have to be built on their own machine and collected before
this runs — whoever tags uploads both. The macOS one comes out of
`target/macos/`, the Windows one out of `target/windows/`.

The release notes must carry the **macOS install instructions** —
unsigned means Gatekeeper refuses on first launch, and the steps differ
by macOS version. Copy from the checklist doc, which keeps the current
wording.

**Web:** upload `pkg/` + `index.html`, `css/`, `js/` to
`fractalsforall.com/editor/`. `pkg/` is gitignored and there is no deploy
script, so this is a manual copy (§5). Load the page afterwards and check
the version — a partial upload leaves a working page running old wasm.

---

## 4b. How upgrades reach users

Different on every surface, and none of it is automatic yet.

| surface | mechanism | friction |
|---|---|---|
| **Windows** | download the new zip, replace the folder | SmartScreen warns on an unsigned exe |
| **macOS** | download, drag the `.app` to Applications, replace | Gatekeeper **repeats on every update** until notarized |
| **Web** | user reloads | none — whatever is uploaded is what they run |

User data survives on both desktop platforms because it lives outside
the app: `%APPDATA%` / `~/Library/Application Support/…` keyed by bundle
identity (settings, downloaded plugins, scripts, effects). Replacing the
app never touches it. `.fflame` files migrate forward on load
(`CURRENT_CONFIG_VERSION`), and new fields are skip-if-default so an
older build reading a newer file degrades rather than breaks.

**Nothing tells a user an update exists.** No version check, no
auto-update. The cheapest fix by far is an in-app version check —
`APP_VERSION` plus one HTTP GET plus a link — and it is the thing to
build before Sparkle or anything else.

---

## 4c. Two build details that bite

### Use `--profile dist`, not `--release` — on **both** desktop and web

`[profile.dist]` (LTO fat, one codegen unit, stripped, `panic = "abort"`)
exists in `Cargo.toml` and, until this document, **nothing had ever
invoked it.** Measured on Windows:

```
desktop   release  32.9 MB   ~1 min       dist  22.6 MB   7m17s    31% smaller
web       release  16.3 MB                dist  13.4 MB   5m20s    18% smaller
```

The web figures are the shipped `.wasm` — after `wasm-bindgen`, which is
what a browser downloads. `build-wasm.sh` / `.bat` build `--profile dist`
for that reason; the flag is inside the scripts, so §4.4's table shows
the script rather than the profile.

**Size argues harder on web than on desktop.** 10 MB off the desktop
binary is a one-time download someone already chose to start. 2.9 MB off
the wasm is paid by every visitor on every cold load, before a single
pixel renders. `dist` also brings LTO and one codegen unit, so it is not
purely a size trade.

Do not substitute `--release` because the wait is annoying — a shipped
binary that says `release` is indistinguishable from a developer's
build, and the profile name is recorded in every exported PNG *and* in
the web Performance panel, which §4.7 tells you to read after a deploy.

**Still on the table, unmeasured:** `[profile.dist]` inherits `release`'s
`opt-level = 3`. The gallery crates (`wasm/render`, `wasm/script`) chose
`opt-level = "z"` and add `wasm-opt -Oz` through wasm-pack, which the app
build bypasses entirely. So 13.4 MB is a floor on the win, not a ceiling
— both levers are untried on the app.

`panic = "abort"` means no unwinding. Nothing in the codebase relies on
`catch_unwind`, and the GPU error path uses wgpu's uncaptured-error
handler rather than panics — but this is the profile users get, so any
new use of unwinding must be checked against it. On web it is also safe
with `console_error_panic_hook`: a panic *hook* runs before the abort, so
browser error text survives.

### Icons

`assets/branding/ffa-logo.png` is the master, 256x256. Everything else
is generated:

```bash
python scripts/make_icons.py     # after changing the logo
```

It lands in four places, and they are genuinely four:

- **The executable's resource table**, via `build.rs`. This is what
  Explorer, the taskbar and Alt-Tab show *before* the app runs, and it
  cannot be set from inside the process.
- **The running window**, via `winit`, from the 64px PNG. Windows and
  Linux only — macOS has no per-window icon, so `with_window_icon` is a
  no-op there and the `.icns` is the *only* route to a Dock icon.
- **The web favicon**, inlined into `index.html` as a data URI — the
  wasm build copies only palette packs into `pkg/`, so a file would
  need a new copy step in both `build-wasm.bat` and `.sh`, kept in step
  by hand.
- **`assets/branding/icon.icns`**, for the macOS bundle. Ten members,
  16px through 1024px, built by `iconutil`.

All four come from one source, so they cannot drift.

Two things about the macOS path are deliberate and easy to "fix" by
mistake:

- **The mark is a rounded tile there, not the full-bleed square.** macOS
  does not mask app icons the way iOS does, so the shape in the `.icns`
  is the shape the Dock draws, and a hard-cornered tile among rounded
  ones reads as foreign. The script insets the mark onto Apple's icon
  grid. Same mark, each platform's convention.
- **The `.icns` step needs `iconutil`, so it is macOS-only** and skips
  with a message elsewhere; the PNGs and `.ico` still build everywhere.
  Pillow can write `.icns` unaided, but its writer emits no 16px member
  — Finder list view would downscale the 32px one, at exactly the size
  where the tuned Lanczos matters most.

**Known limitation:** the master is 256x256, so the 512 and 1024 members
are upscaled and visibly soft at Quick Look / large-icon sizes. The
script warns when it does this. A 1024x1024 master fixes it with no code
change, since every size resizes from the master.

`make_macos_app.py` copies the `.icns` into `Contents/Resources/` and
points `CFBundleIconFile` at it, so a missing one fails the bundle build
with the command that regenerates it — not silently, and not at launch.

## 5. Packaging

**Windows and macOS: a zip each. Web: a directory copy.** Signing is the
one thing still genuinely open, and it is open on both desktop platforms
for the same reason — it costs money and neither is blocking.

### Windows — a portable zip

```bash
cargo build --profile dist
python scripts/make_windows_zip.py          # -> target/windows/
```

```
FractalArtEditor-0.5.0-windows.zip
  FractalArtEditor-0.5.0/
    FractalArtEditor.exe      target/dist/
    assets/                   presets, palettes, fonts — loaded from disk
```

`shaders/` is **not** included: every shader is embedded in the binary,
and the on-disk tree is a developer *override* that takes precedence.
Shipping it would add half a megabyte whose only effect is to let a stray
edit change what a released app renders. The script exists mostly to hold
that decision — both halves of it fail quietly when packaged by hand, one
as an app missing half its content, the other as an app that renders
something other than what was tested.

No installer. A portable zip needs no elevation, leaves no uninstall
entry to rot, and matches how the app already stores its data (in
`%APPDATA%`, not beside the exe) — so "delete the folder" is a complete
uninstall. Revisit if file associations or Start-menu presence start
mattering.

**Open: code signing.** Unsigned means SmartScreen warns on first run.
An OV certificate is a few hundred dollars a year and the warning fades
with reputation; an EV one clears it immediately and costs more. Neither
is blocking a first release.

#### The console window, and one caveat

Release builds are GUI-subsystem, so launching from Explorer opens no
black console beside the app. The same binary is still the CLI, so it
re-attaches to the parent terminal when launched with arguments.

Verified: git bash, PowerShell pipes, `cmd` including `> file`,
`Start-Process -RedirectStandardOutput`, and Python `subprocess` capture
— which is what every test and benchmark harness here uses.

**One known gap: PowerShell 5.1's `>` operator captures nothing** from a
GUI-subsystem exe. `| Out-File`, `Start-Process -RedirectStandardOutput`
and `cmd /c ... >` all work. This is platform behaviour, not something
the attach logic can repair — a control run of `cargo --version > file`
in the same shell works, so it is specific to the subsystem.

If it proves annoying: a second console-subsystem binary (~22 MB more in
the zip), or reverting to console-subsystem and hiding the window at
startup — which trades the gap for a console flash on every GUI launch.

### macOS — a `.app` bundle, shipped as a zip

```bash
cargo build --profile dist
python3 scripts/make_macos_app.py --zip      # -> target/macos/
```

The bundle is not cosmetic. macOS has no equivalent of the Windows
resource table, so `Contents/Info.plist` → `CFBundleIconFile` is the only
way the app ever shows its icon; the bundle also gives it a menu-bar
name, a stable identity for preferences, and the `NS*UsageDescription`
keys **without which macOS silently suppresses the microphone prompt and
CoreAudio delivers nothing** — the audio-reactive stack appears dead with
no error anywhere. That one cost a full debugging session; the key is in
`make_macos_app.py` now.

`assets/` goes in `Contents/Resources/`, which is Apple's convention and
also the third place `resources::resource_path` looks. That is
load-bearing: Finder launches a bundled app with the working directory
set to `/`, and every asset path in the codebase is repo-relative.
`shaders/` is omitted, same reasoning as Windows.

Signed ad-hoc (`codesign -s -`), which needs no account — on Apple
Silicon a bundle modified after linking can fail to launch without it.
**This is not Gatekeeper approval.**

#### The cost of not notarizing, which is not the obvious one

An unsigned app downloaded through a browser carries
`com.apple.quarantine`, and Gatekeeper refuses it. On macOS 15 the
right-click → Open bypass is gone; the user must use System Settings →
Privacy & Security → "Open Anyway", or:

```bash
xattr -d com.apple.quarantine "/Applications/Fractal Art Editor.app"
```

**Every download is quarantined, so this repeats on every update**, not
just first install. That is the real price of skipping the $99/yr
Developer ID, and it is worse than "one scary dialog once". The
paste-ready user-facing instructions live in
`docs/projects/macos-release-checklist.md`.

### Web — copy four things to `fractalsforall.com/editor/`

The live app is **`https://fractalsforall.com/editor/`**. `pkg/` is
gitignored, so this is build-locally-and-upload; there is no CI step and
no deploy script.

| upload | from | changes |
|---|---|---|
| `pkg/` | `build-wasm.sh` / `.bat` (`--profile dist`, 13.4 MB) | every release |
| `index.html` | repo root | rarely |
| `css/`, `js/` | repo root | rarely — the virtual-keyboard bridge |

Deployment is by whatever moves files onto that host. Verify by loading
the page and reading the **Performance panel** — it shows version, git
hash, branch, build time and profile. Do not trust the copy: a partial
upload leaves a working page running old wasm.

**One host, decided.** `fractalsforall.com/editor/` is production.
`calibas.github.io` is being retired — it was a second deployment of the
same artifact, last built **2026-02-11** (11.8 MB wasm) against
fractalsforall.com/editor's **2026-06-30** (15.2 MB): five months and a
product rename behind, still titled "FAR - Online Version". The README
now points at production.

Until the DNS or the GitHub Pages source is actually switched off, treat
it as live-and-wrong: anyone who has it bookmarked is running February's
build. Retiring it is a release step, not a cleanup task — a redirect to
`fractalsforall.com/editor/` is better than a 404 for anyone holding the
old link.

### Gallery modules — two `pkg/` directories, built with wasm-pack

The Endless Gallery's renderer and script engine. Separate crates, so
they are the one surface that does **not** use `build-wasm.sh`:

```bash
cd wasm/render && wasm-pack build --target web --release   # -> wasm/render/pkg/
# ...and the engine-specific pair, same source, different Cargo features:
cd wasm/flame  && wasm-pack build --target web --release   # flame only,  0.73 MB gzip
cd wasm/escape && wasm-pack build --target web --release   # escape only, 0.41 MB gzip
cd wasm/script && wasm-pack build --target web --release   # -> wasm/script/pkg/
```

Roughly 1.5 minutes each. Each `pkg/` is a self-contained ES module
(`.js` + `.wasm` + `.d.ts` + `package.json`); `wasm/README.md` has the
JS API and an end-to-end example wiring script → render.

**`--release`, not `--profile dist`** — the one exception to §4c, and
not an oversight: each crate's own `[profile.release]` already sets
`opt-level = "z"`, `lto = "fat"`, `codegen-units = 1`,
`panic = "abort"` and `strip = true`, and wasm-pack adds a `wasm-opt
-Oz` pass the app build never gets. `dist` would add nothing.

**wasm-pack, not raw cargo + wasm-bindgen**, because that `wasm-opt`
pass is where a large part of the size win is. wasm-pack downloads its
own `wasm-opt`, so binaryen need not be on PATH. The
`--enable-bulk-memory --enable-nontrapping-float-to-int` flags in each
`Cargo.toml` are load-bearing: the bundled wasm-opt 117 predates
rustc's bulk-memory default and rejects the module without them.

**Gated since 0.5.** `release.py check` runs both crates' test suites
(`gallery script parity`, `gallery renderer smoke`) — the CLI-parity
fixtures §6 names as the guard for `script + seed` reproducibility now
run in every check. The renderer smoke tests skip cleanly on a machine
with no GPU adapter. To run one by hand:

```bash
cd wasm/script && cargo test --profile gallery-test   # byte-identical flames
cd wasm/render && cargo test --profile gallery-test   # GPU smoke + device reuse
```

**Run them from the crate directory, and with that profile** — both
matter, and neither is cosmetic:

- `--profile gallery-test`, never `--release`. These crates'
  `[profile.release]` is tuned to ship a small `.wasm` (`opt-level =
  "z"`, fat LTO, one codegen unit). Testing under it fat-LTOs the whole
  parent crate to optimize a test binary for *download size*: 178s per
  crate, versus 73s under `gallery-test`, which keeps `opt-level = 2`
  and drops the rest. `wasm-pack build --release` still ships from
  `[profile.release]`, so module size is unchanged.
- **From the crate directory**, because cargo discovers
  `.cargo/config.toml` by current directory. `--manifest-path` from the
  repo root silently misses `wasm/.cargo/config.toml`, and with it both
  wins it carries: one shared `wasm/target-gallery` for the pair (the
  parent crate compiled once instead of once each, and ~10 GB less on
  disk) and `FFLAME_SKIP_GIT_PROVENANCE`, which stops `build.rs`
  re-emitting `GIT_HASH`/`BUILD_TIME` into these builds on every commit.
  That last one was the real cost: a commit touches no gallery code, but
  it changed the hash, which invalidated the parent crate in each
  private target directory and bought a full rebuild of both.

Nothing in the gallery modules reads version, hash or build time, so
they record explicit `not-recorded` placeholders rather than a frozen
real hash — a wrong label is worse than none. The app and the dist
build never set that variable and are unaffected.

`pkg/` is gitignored, so each machine builds its own and nothing about
these directories appears in a diff. Where the built modules are
*deployed* is not decided here — see §7.

### Linux — nothing decided

The build works; packaging does not exist. AppImage is the usual answer
for a GPU app that has to carry its own assets.

### Python — nothing decided

A wheel is per-platform and per-Python-version. Publishing to PyPI means
either a build matrix or shipping only what can be built by hand.

---

## 6. What a release must not break

The properties this project has spent effort earning, each with the
thing that guards it:

- **`script + seed` reproduces a flame** across desktop, web and Python.
  Guarded by the CLI-parity fixtures; the pinned PCG64-MCG stream and
  `rand` version are load-bearing (see the `Cargo.toml` comment).
- **A `.fflame` from an older version still opens.** Guarded by
  `CURRENT_CONFIG_VERSION` and the `migrate_value` arms. Adding a field
  is free (`serde(default)`); changing a *default* needs an explicit arm.
- **Apophysis/JWildfire `.flame` round-trips.** Guarded by unit tests.
- **A shipped flame renders the same after upgrade.** Guarded by the
  visual regression suite and the canonical shader dumps.
- **Exported PNGs identify their build.** version, git hash, profile,
  platform, rustc version, and the complete config.

That last one is what makes a bug report actionable — the reporter sends
an image and it carries the build and the flame that produced it.

---

## 7. Missing infrastructure

Honest list, so nobody assumes these exist:

- **No CI, deliberately.** `scripts/release.py` does what CI would do,
  on demand. It exits non-zero, so it *could* become a hook later
  without being one now. What is genuinely given up: nothing enforces
  that anyone ran it.
- **No hand-written changelog, deliberately.**
  `release.py changelog` generates one from commit subjects. It gets the
  raw material; deciding what mattered to a user is still a person's
  job.
- **No signing or notarization** for any platform (§5).
- **No release tags.** `git tag` returns nothing, so `release.py
  changelog` has no boundary to diff from until the first one exists
  (§4.1). Every exported PNG records a git *hash*, so builds are already
  traceable — tags are what make them nameable.
- **No web deploy step.** `pkg/` is gitignored and the upload to
  `fractalsforall.com/editor/` is manual (§5). A manual upload is also
  how the retired `calibas.github.io` mirror drifted five months behind
  before anyone noticed — one host now, but nothing stops the one host
  from being half-uploaded, which is why §4.7 says to load the page and
  read the version afterwards.
- **`build-wasm.bat` and `build-wasm.sh` are parallel implementations**
  of one procedure. They agree today; nothing enforces that. The move to
  `--profile dist` is what that costs in practice: two lines in each
  file, in two languages, and a build that still succeeds if you edit
  only one — it would just quietly ship the unoptimized wasm from
  whichever platform ran the stale script.

---

## 8. Adding things that affect the API

Quick reference — the full reasoning is in §3.

| you changed | run |
|---|---|
| a variation | shader dumps if WGSL changed; contract; variation corpus |
| a variation *feature* | `Feature::ALL` + its count test; contract; **tell the API** (§3) |
| an effect | contract; effect corpus; check `locales/en.yml` has its label |
| a wire struct | contract (shape moves, API re-pins); `VARIATIONS_WIRE_FORMAT.md` §11 |
| `FractalConfig` structure | bump `CURRENT_CONFIG_VERSION`, add a `migrate_value` arm |
| a shipped script | its stem is reserved — it lands in `builtin_scripts`; regenerate the contract |
