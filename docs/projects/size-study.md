# Size study: build time, binary size, repository

Status: **measured 2026-11-11**; decisions for the user at the end. No
code changed: every experiment was a temporary edit, reverted.

The question, in the user's words: "We're getting bloated. WASM build
takes 5 minutes, native app builds aren't exactly fast either. The
native app is getting quite big. The git repo is slowing down, and the
code base is getting huge. ... I'm thinking replacing built-ins with
Rhai, and/or having the lesser-used variations and formulas be API
downloads instead of being built into the app. Everything depends on how
much time/space is actually saved though. I want hard numbers first."

Measured on the user's machine (12 threads, Windows, rustc 1.95), main at
3a5f0b3a. Logs in `output/bloat/` (gitignored).

---

## 1. The numbers

### Builds

| Build | Time | Notes |
|---|---|---|
| Native release, incremental, one variation file edited | **16 s** | the everyday desktop loop (thin LTO, 16 CGU, incremental) |
| Native release, no change | 1 s | |
| Unit-test binary, incremental, one file edited | **25 s** | |
| `cargo check`, incremental, one file edited | 20 s | |
| `cargo check`, whole crate, non-incremental | **72 s** | the front end alone: parsing, type checking, borrow checking |
| WASM `dist` (`build-wasm.sh`) | **305-364 s** | fat LTO, 1 codegen unit; our crate (the top includes wasm-bindgen) |
| Native `dist` (the shipped desktop build) | **890-951 s** | fat LTO, 1 codegen unit; our crate |

The same crate under the two profiles, dependencies already built, each
rebuilt after a one-line edit:

| Our crate only | Fat LTO, 1 codegen unit (`dist`, shipped) | Thin LTO, 16 codegen units |
|---|---|---|
| Web (WASM lib) | 305 s; 26.6 MB raw, 7.74 MB gzip | **121 s (-60%)**; 32.4 MB raw, 8.64 MB gzip (+12%) |
| Desktop | 890 s; 36.3 MB | **403 s (-55%)**; 42.5 MB (+17%) |

### Artefacts

| Artefact | Size |
|---|---|
| Web app, `pkg/fractal_flame_wgpu_bg.wasm` | 24.4 MB raw, **7.58 MB gzip** |
| Desktop, `dist` (stripped, panic=abort) | **36.3 MB** |
| Desktop, `release` (the dev build) | 57.6 MB |
| Gallery modules (render, script) | 4.6 MB, 4.5 MB raw |

### What the binary carries as text

Counted from the source (shipped code only):

| What | Size |
|---|---|
| Variation WGSL (647 defs, raw strings in `src/variations/defs/`) | 1.72 MB |
| Variation Rhai twins (`assets/variations/twins/`, for Edit as Rhai) | 2.26 MB |
| Escape formula/colouring WGSL | 0.48 MB |
| Simulation WGSL | 0.18 MB |
| Presets, shaders, other twins, scripts | ~1.8 MB |

Every variation is in the binary **twice**: once as WGSL, once as its
Rhai twin.

### The code

| | Lines |
|---|---|
| Rust, all | 488,000 |
| Shipped | ~300,000 |
| Tests (`#[cfg(test)]` modules and test-only files) | ~185,000 (38%) |
| Largest shipped areas | escape 105k (incl. 27k of app-repro tests), variation defs 76k, scene 55k, ui 42k, lang 33k, sim 30k |

### The repository

| | Size |
|---|---|
| `.git` | 265 MB (7 packs, 29 MB loose) |
| Tracked files today | 67.7 MB (fonts 13 MB, visual baselines 10.7 MB, archive docs 4.7 MB, ...) |
| History, by what it holds | visual-suite PNGs **114 MB** (155 re-baseline commits; today's baselines are 10.7 MB); an old committed `pkg/` WASM build **39 MB**; stray media at the root (`animation.mp4`, test PNGs) **34 MB**; fonts and HDRIs 14 MB; SwiftShader 2 MB; **all Rust source ever 36 MB**; Markdown 9 MB |
| Everyday operations | `git status` 0.4 s, `git log -1000` 0.4 s, `git diff HEAD~20` 1.2 s |

---

## 2. What removing content saves

The experiment: keep 100 of the 647 variations registered (the classic
flam3/Apophysis set), and drop the twins. With fat LTO, an unregistered
variation is dead code and leaves the binary, so this is what "the rest
become downloads" would save.

| | Web app (gzip) | Web app (raw) | Desktop `dist` | Build time |
|---|---|---|---|---|
| Today | 7.74 MB | 26.6 MB | 36.3 MB | 319 s web, 951 s desktop |
| 547 variations and all twins gone | 6.98 MB (**-0.76, -9.8%**) | 22.2 MB (-4.4) | 31.7 MB (**-4.6, -12.7%**) | 301 s (-6%), 918 s (-3.5%) |
| ...of which the twins | -0.39 MB | -2.3 MB | | |
| ...of which 547 variations' WGSL and code | -0.37 MB | -2.1 MB | | |

(Web figures here are the library before wasm-bindgen, so they read a
little above the shipped `pkg/` file.)

So:
- **Downloads for the lesser-used variations** would take about a tenth
  off the web download and an eighth off the desktop executable, and
  almost nothing off build time.
- **Replacing the built-ins with Rhai** (the WGSL defs gone, the twins
  kept and compiled on load) saves about the WGSL's share -- 2.1 MB raw,
  0.37 MB gzip -- and costs a Rhai-to-WGSL compile per variation used, on
  every load. **Dropping the twins from the binary instead** (fetched or
  generated when Edit as Rhai asks) saves the same again, with no runtime
  cost.
- Escape and simulation built-ins carry less than a third of the
  variations' text (0.66 MB of WGSL together); moving them would save
  less still. Their engines are code, which stays either way.

Content is not where the build time goes.

## 3. Where the build time goes

Of the web build's 305 s, the front end (parsing, type and borrow
checking, single-threaded) is **72 s**. The other ~230 s is LLVM
optimising one fat-LTO module as one codegen unit -- on one thread, by
construction. Thin LTO with 16 codegen units spreads that over the cores:
121 s, of which ~50 s is codegen. The desktop is the same story at a larger
scale (890 s -> 403 s).

So:
- **The profile is the lever**, not the content: removing 547 variations
  took 3-6% off; the profile takes 55-60% off, for 12-17% more size.
- **Splitting the crate into a workspace would help less than it looks**
  for shipped builds: fat LTO merges every crate back into one module and
  optimises it on one thread anyway. It would help the front end (72 s,
  parallel across crates) and incremental and test builds -- which are
  already 16-25 s.
- The everyday desktop loop is already fast (16 s incremental, 25 s for the
  test binary). The slow builds are the shipping ones: the web app every
  time it is tried in a browser, and the desktop release.

## 4. Where the repository's weight is

The source code is a small part of it: 36 MB of the 258 MB of history is
Rust ever written. Replacing built-ins with Rhai or downloads would not
change git's size noticeably. The weight is binaries:

- **114 MB of old visual-suite baselines.** Each re-baseline adds a full
  copy of every changed PNG.
- **75 MB that should never have been committed**: a `pkg/` WASM build
  (39 MB), root-level media (34 MB), SwiftShader (2 MB). None is tracked
  today; they live on in history.

Two separate things would fix it:
- **Stop the growth**: keep the visual baselines out of git's history --
  Git LFS, or a baseline store outside the repository (a release asset,
  a separate repository) fetched by `run_tests.py`.
- **Shrink the history**: `git filter-repo` dropping `pkg/`, the stray
  media, SwiftShader and old baseline versions would take the history
  from ~258 MB to roughly 60-70 MB. This rewrites every commit hash:
  every clone must be re-made, open branches rebased, and GitHub keeps
  the old objects reachable from closed PRs until support purges them.
  Irreversible in practice; the user's call.

Everyday git operations (0.4-1.2 s) are not slow for Windows; the size
hurts clone, fetch and repack.

---

## 5. The dependencies

Measured 2026-11-11 with `cargo bloat --release --bin FractalArtEditor
--crates` (the release build, thin LTO; the shipped `dist` differs in
size, not in proportion; full list in `output/bloat/crates.txt`). The
app's code (`.text`) is 32.4 MiB of a 54.9 MiB release executable; 396
crates are in the build, 41 of them in two versions.

| Crate | Code | |
|---|---|---|
| this crate | 10.6 MiB | |
| `std` | 5.5 MiB | mostly generics our code instantiates |
| `rhai` | 2.7 MiB | the scripting engine and the formula language's parser |
| `egui` | 1.7 MiB | |
| `naga` | 1.6 MiB | wgpu's shader translator |
| `serde_json` | 1.1 MiB | a deserializer per config type |
| `wgpu_core` + `wgpu_hal` | 1.6 MiB | |
| `rustfft` | 678 KiB | |
| TLS: `rustls`, `ring`, `webpki`, `ureq`, `url` | ~0.9 MiB | desktop online |
| regex: `regex_automata`, `regex_syntax`, `aho_corasick` | ~760 KiB | not ours: pulled in by `rust-i18n` |
| `image` (+ `png`, `zune_jpeg`) | 571 KiB | |
| `rayon` | 360 KiB | |
| `clap` | 349 KiB | |
| `symphonia` (mp3, flac, ogg, vorbis, wav) | ~380 KiB | |
| `vello_cpu`, `skrifa`, `read_fonts` | ~810 KiB | not ours: egui's text rendering |

### Redundant: remove

- **`ron`**: never used (no reference in any Rust file). It also pulls in
  a second `base64`.
- **`ringbuf`**: never used (one comment: "reserved for future").
- **`png` 0.17**: our direct dependency, while `image` uses `png` 0.18; two
  copies compile. Bump ours to 0.18 (one file uses it).
- **`once_cell`**: `std::sync::OnceLock` / `LazyLock` do the same (10
  files); the crate stays in the tree through others, but not as ours.

### Worth replacing in-app

- **`rust-i18n`** -> our own `t!` macro and a table built by `build.rs`.
  It brings **74 crates into the app** -- regex, a deprecated YAML parser,
  TOML, file globbing, itertools -- linked at runtime, to look up strings
  from four YAML files: ~760 KiB of regex code alone. Its use is uniform:
  10,620 `t!(key)` / `t!(key, name = value)` calls in 273 files, which keep
  their syntax. What is needed: `build.rs` reads `locales/*.yml` (a
  two-level map of strings; a small parser, or `serde_yaml` as a
  build-only dependency) into a static table; `t!` looks up the current
  locale, then English, then the key, and fills `%{name}`; `set_locale` /
  `locale()` as today. A day's work; the web app saves the same.
- **`rustfft`** -> a radix-2 FFT. 678 KiB for the audio analyser and live
  capture (`audio/analyzer.rs`, `audio/capture_common.rs`), which transform
  one power-of-two window a frame. An iterative radix-2 complex FFT (or a
  real FFT by the half-size trick) is ~70 lines and fast enough by orders of
  magnitude at that size; held to `rustfft` by a test, kept as a
  dev-dependency (the GPU FFT's test already uses it).

### Possible, smaller gains

- **`rayon`** (360 KiB): CPU export paths in 11 files; `std::thread::scope`
  with chunking covers parallel loops. Medium effort.
- **`chrono`** (43 KiB, plus `iana-time-zone` and Windows bindings to
  compile): three timestamps (`build.rs`, the GPU log, a CLI log line); a
  UTC formatter is a few lines, local time needs the OS.
- **`futures`**: only `futures::channel::oneshot` is used; depend on
  `futures-channel` alone, or a 40-line oneshot.
- **`half`** (8 KiB): f16 conversions for 16-bit textures; ~30 lines by
  hand. **`anyhow`** (13 KiB): five files. Hygiene, not size.
- **`cpal` 0.15 -> 0.16** and **`ureq` 2 -> 3**: each removes a duplicated
  family (`windows` 0.54, `webpki-roots` 0.26) from the build; API changes
  to check.

### Keep

- **Crypto** (`aes-gcm`, `sha2`, `ring` via TLS): never by hand.
- **TLS** (`rustls`): cannot be written in-app; the alternative is the
  OS's TLS (`native-tls`), which on Linux means OpenSSL.
- **`symphonia`**: audio codecs. Ogg/Vorbis (144 KiB) or FLAC (53 KiB)
  could be dropped if those formats are not wanted.
- **`clap`** (349 KiB): a hand parser for four subcommands and their flags
  would lose generated help for a third of a megabyte.
- **`image`**: 24 files decode and encode PNG, JPEG and HDR through it;
  replacing it means a JPEG encoder and an RGBE reader of our own.
- **`rhai`, `egui`, `egui_dock`, `naga`, `wgpu`, `serde_json`, `quick-xml`,
  `rfd`, `winit`, `cpal`, `web-sys`**: the app's foundations.

**Together**, the removals and the two replacements take ~1.5 MiB of code
out of the desktop app (~5% of its code) and ~75 crates out of the build
(clean builds and CI compile less; fat LTO optimises less). They do not
change the everyday build: our own crate is what compiles on every edit.

## 6. Options, by what they buy

| Option | Saves | Costs |
|---|---|---|
| A. Drop the variation twins from the binary; fetch or generate on Edit as Rhai | web -0.39 MB gzip (-5%), desktop ~-2.3 MB | Edit as Rhai needs the file (bundled as a separate asset on desktop, fetched on the web) |
| B. Lesser-used variations as downloads (547 of 647) | web -0.37 MB gzip more; desktop ~-2.3 MB more | a missing-variation fetch path for every config that uses one (the API path exists for old clients); offline use loses them unless cached; which 547 is a guess without usage data |
| C. Built-ins replaced by their Rhai twins (D7) | about B's WGSL share, but every variation stays available | a compile per variation on load; the built-in WGSL's speed is matched by most twins, not all |
| D1. A fast profile for everyday web and desktop testing (thin LTO, 16 CGU; `build-wasm.sh --fast`), fat LTO kept for releases | web 305 s -> 121 s; desktop 890 s -> 403 s; shipped artefacts unchanged | the test build is 12-17% bigger than what ships (a build to try, not to measure size or ship) |
| D2. Ship thin LTO too | the same times for releases | users download 0.9 MB more (web, +12% gzip); desktop +6 MB |
| E. Split the crate into a workspace (engine core, variations, escape, simulation, language, UI/app) | the front end (72 s) in parallel; an edit rebuilds one crate; little for fat-LTO release builds (§3) | a large refactor: module boundaries, `pub` surfaces, the registries' globals |
| F. Visual baselines out of git (LFS or an external store) | stops ~1 MB of history per re-baseline | `run_tests.py` fetches baselines |
| G. Rewrite history (`git filter-repo`) | ~258 MB -> ~60-70 MB | every hash changes; irreversible in practice |

## 7. Decisions for the user

1. **A fast build profile (D1)** for trying the web app and the desktop
   between releases: recommended -- it is the one change that takes most
   of the build time away, at no cost to what ships.
2. **Ship thin LTO (D2)?** Recommended no: 12% more download for every
   user, against build time only the developer pays (and D1 removes most
   of that).
3. **The variation twins out of the binary (A)**: the cheapest real size
   cut (web -5%, desktop ~-2.3 MB), Edit as Rhai fetching or loading the
   twin file. Recommended if size matters on its own.
4. **Variations as downloads (B) / built-ins replaced by Rhai (C, D7)**:
   about another 5% off the web download and ~2 MB off the desktop for
   547 variations; little for escape and simulation built-ins. Worth it
   for the code base's size and for D7's own reasons, not for build time
   or download size.
5. **The repository**: baselines out of git's history going forward (F),
   and whether to rewrite history (G) -- irreversible, the user's call.
6. **A crate split (E)**: not for build time (§3); possibly for the code
   base's organisation, as its own decision.
7. **Dependencies (§5)**: remove `ron`, `ringbuf`, bump `png`, `once_cell`
   to std (recommended: trivial, no risk); replace `rust-i18n` and
   `rustfft` in-app (recommended: ~1.4 MiB and ~75 crates, a day or two);
   the smaller ones as wanted.