# Release and update procedure

Every procedure that has to run, in order, and what each one gates. This
exists because the procedures were real but scattered — `build-wasm.bat`,
`run_benchmarks.py`, `run_tests.py`, three regeneration env vars — with
no single place to see them, and nothing saying which are required for a
release rather than merely available.

**Status: partly decided.** Sections marked **UNDECIDED** are open
questions, not steps. They are recorded here rather than left implicit,
because the gap between "we build it" and "someone can install it" is
where a release actually stalls.

---

## 1. What ships

Four artifacts, built four different ways, from one repository.

| surface | built by | output | version source |
|---|---|---|---|
| Desktop app | `cargo build --profile dist` | `target/dist/fractal_flame_wgpu(.exe)` | `Cargo.toml` |
| Web app | `build-wasm.bat` / `.sh` | `pkg/` + `index.html`, `css/`, `js/` | `Cargo.toml` |
| Gallery modules | `wasm-pack build` in `wasm/render`, `wasm/script` | `wasm/*/pkg/` | each crate's `Cargo.toml` |
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

**Decide before the first release.** Two workable answers:

- **Lockstep** — one version for everything, bumped together. Simple to
  reason about, and honest given they are built from one commit. Costs a
  version bump on crates that did not change.
- **Independent, with a recorded floor** — each crate versions itself,
  and each records the app version it was built from (the same way
  `build.rs` already records the git hash).

Lockstep is the recommendation. The artifacts are not independently
useful — a `pyfflame` wheel and the app it matches come from one commit,
and pretending otherwise creates a compatibility matrix nobody will
maintain.

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
| the effect corpus | `cargo run --release --bin export_effects_json` | any effect added or edited |

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

1. **Bump the version:** `python scripts/release.py version 0.5.0`
   — every crate, in lockstep (§1). `--dry-run` first.
2. **Generate the changelog:** `python scripts/release.py changelog`
   — grouped by the `area:` prefix in commit subjects, which 141 of the
   last 150 commits use. Edit the result; a generator gets the raw
   material, not the judgement about what mattered.
3. **Run every gate** (§2). Regenerate artifacts if a gate demands it,
   and read the diffs.
4. **Sync the API** (§3) if any generated artifact changed. Do this
   *before* tagging, so the tag matches what the server serves.
5. **Build each surface:**
   ```bash
   python scripts/release.py build         # all four, in order
   ```
   or individually — note `--profile dist`, not `--release`, see below:
   ```bash
   cargo build --profile dist
   ./build-wasm.sh                         # or .bat
   cd wasm/render && wasm-pack build --target web --release
   cd wasm/script && wasm-pack build --target web --release
   cd python && maturin build --release
   ```
6. **Smoke-test each artifact** — not just that it built:
   - desktop: launch it, and export a PNG; check `BuildProfile` in the
     metadata says `dist`
   - web: serve `pkg/` and load it in Chrome and Firefox
   - python: `import pyfflame`, run a script, compare against the app
   - gallery modules: `wasm/README.md` has an end-to-end example
7. **Tag the commit.** Every exported PNG records the git hash, so a tag
   is what makes a bug report traceable to a source tree.
8. **Package and publish** — **UNDECIDED**, see §5.

### Use `--profile dist`, not `--release`

`[profile.dist]` (LTO fat, one codegen unit, stripped, `panic = "abort"`)
exists in `Cargo.toml` and, until this document, **nothing had ever
invoked it.** Verified:

```
release   32.7 MB    ~1 min
dist      22.5 MB    ~7-10 min      31% smaller
```

It builds clean and the binary runs. Budget the ten minutes; do not
substitute `--release` because the wait is annoying — a shipped binary
that says `release` is indistinguishable from a developer's build.

`panic = "abort"` means no unwinding. Nothing in the codebase relies on
`catch_unwind`, and the GPU error path uses wgpu's uncaptured-error
handler rather than panics — but this is the profile users get, so any
new use of unwinding must be checked against it.

---

## 5. Packaging — **UNDECIDED**

Nothing here is settled. What has to be answered:

**Windows.** Leaning toward a plain `.zip`. It needs the executable plus
`assets/` (presets and palettes are loaded from disk at startup) and
`shaders/` is *no longer required* — every shader is embedded, and the
on-disk copy is only a developer override. Verify by unzipping somewhere
with no `shaders/` and rendering an effect; that path is now tested but
should be confirmed per release.

Open: signing (unsigned binaries get SmartScreen warnings), and whether
to ship a portable zip, an installer, or both.

**macOS.** Nothing decided. A `.app` bundle needs a plist, an icon, and
— to avoid Gatekeeper refusing it outright — notarization, which needs a
paid developer account. An unsigned zip is possible but users must
right-click-open past a scary dialog.

**Linux.** Nothing decided. AppImage is the usual answer for a GPU app.

**Web.** `pkg/` is gitignored, so deployment is currently "build locally,
upload somewhere" — undocumented, and the README already points at
`calibas.github.io`. Write down what that upload actually is.

**Python.** A wheel is per-platform and per-Python-version. Publishing to
PyPI means either a build matrix or shipping only what can be built by
hand.

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
- **Repository hygiene**: `debug_shader_3d.wgsl` and `random-notes.txt`
  are tracked at the root and probably should not be.
- **`build-wasm.bat` and `build-wasm.sh` are parallel implementations**
  of one procedure. They agree today; nothing enforces that.

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
