# CPU Rendering — SwiftShader Desktop Fallback

**Date:** 2026-02-14 (research) / 2026-05-22 (plan update)
**Status:** Plan finalized. Partial implementation on
[`experimental/swiftshader-cpu-fallback`](https://github.com/calibas/fflame-rust/tree/experimental/swiftshader-cpu-fallback)
(Windows working end-to-end). Cross-platform binaries + cargo feature
gate are the remaining work.

## Decision

**Bundle SwiftShader as a software Vulkan ICD, feature-gated, desktop
only.** No WASM coverage (browser sandboxes can't load native ICDs).
Single renderer — no second CPU pipeline to maintain.

### What this covers

- Desktop machines with no GPU
- Machines with broken / blocked GPU drivers (corporate policy,
  Windows safe mode)
- Headless CI environments
- Server-side render farms
- Debugging / repro on developer machines

### What this doesn't cover

- **WASM.** macOS Safari users without WebGPU still have no fallback.
  Accepted — Safari 18.2+ ships WebGPU by default and the population
  shrinks naturally. The only way to fix WASM would be to write a
  second pure-Rust CPU renderer (~2-3k lines, full re-port of all
  variations + tonemap + accumulation). Decision: not worth two
  parallel renderers for a shrinking audience.

## Implementation status (on the branch)

[`experimental/swiftshader-cpu-fallback`](https://github.com/calibas/fflame-rust/tree/experimental/swiftshader-cpu-fallback)
already has:

- `--cpu-rendering` CLI flag
- `build.rs` that copies the SwiftShader ICD to the target directory
- Windows `vk_swiftshader.dll` bundled (Chrome's Subzero JIT build,
  ~5.4 MB)
- `vk_swiftshader_icd.json` manifest
- `VK_ICD_FILENAMES` env-var routing in `src/gpu/device.rs`
- Software-renderer detection (`adapter.get_info().device_type ==
  DeviceType::Cpu`) + UI indicator in menu bar
- Vulkan-backend forcing on Windows (wgpu prefers DX12 there; we
  need Vulkan for SwiftShader to be visible)
- Apache 2.0 license file alongside the binary

Two commits on the branch:

```
1d6a0  Add --cpu-rendering CLI flag with bundled SwiftShader ICD
1d6b388 Add software renderer detection, WGPU_BACKEND env var, and UI indicator
```

## Remaining work

### 1. Cargo feature flag (`cpu-fallback`)

Currently the CLI flag is unconditional — the SwiftShader binary
ships with every Windows build. That's wrong for two reasons:
distribution bloat (~5 MB on Windows, ~25 MB on Mac/Linux per LLVM
build), and users who don't need it shouldn't get it.

Move behind `#[cfg(feature = "cpu-fallback")]`:

```toml
[features]
default = []
cpu-fallback = []
```

- `build.rs`: skip the binary-copy step when feature is off
- `main.rs`: only register the `--cpu-rendering` flag when feature is
  on; print a useful error otherwise
- `device.rs`: only attempt the SwiftShader path when feature is on
- UI: only show the "CPU rendering" indicator when feature is on

Release artifacts can build with `--features cpu-fallback`; default
`cargo run` / `cargo build` from the repo is unchanged.

### 2. Cross-platform binaries

We need SwiftShader binaries for the four desktop targets we care
about. Current state and sourcing plan:

| Platform | Current | Plan | Source |
|---|---|---|---|
| Windows x86_64 | ✓ Subzero, ~5 MB | Keep | Chrome installer extraction |
| Linux x86_64 | ✗ | LLVM, ~23 MB | Build from upstream or use community Vulkan SDK bundle |
| macOS x86_64 | ✗ | LLVM, ~23 MB | Build from upstream |
| macOS aarch64 | ✗ | LLVM, ~23 MB | Build from upstream |

**Subzero is x86-only** — there's no AArch64 backend in practice
([Subzero source](https://chromium.googlesource.com/native_client/pnacl-subzero/)).
Apple Silicon and ARM Linux must use the LLVM build. Could
universally adopt LLVM for consistency (drop Windows Subzero too),
but that grows the Windows binary from 5 MB → 23 MB; keeping the
mixed approach is fine since `build.rs` selects per-target anyway.

**Build pipeline for Mac/Linux binaries**: SwiftShader's upstream
build uses CMake. Reproducible-build instructions for each platform
should land in `swiftshader/README.md` so the binaries can be
refreshed when needed. Or we publish them as a GitHub release
artifact and `build.rs` downloads on demand.

### 3. Binary distribution

Three options, each with different costs:

| Approach | Repo size impact | CI / clone speed | Reproducibility | Update flow |
|---|---|---|---|---|
| Commit binaries to repo | +75 MB (4 binaries × ~5-25 MB) | Slow clones forever | Trivial | `git add` new versions |
| Git LFS | Bytes in repo, blobs separately | Faster shallow clone | Trivial | LFS push |
| `build.rs` downloads from GitHub release | Bytes in repo, downloads on build | Slow first build only | Need cached releases | Bump version pin |

Current branch commits the binary directly. Acceptable for one
Windows DLL (5 MB), but 75 MB of binary in `master` git history is
worth avoiding. Plan: **`build.rs` downloads from a tagged GitHub
release**. Keeps git history clean, supports refresh without
rewriting history.

Sketch:

```rust
// build.rs
#[cfg(feature = "cpu-fallback")]
fn fetch_swiftshader() {
    let target_os = env::var("CARGO_CFG_TARGET_OS").unwrap();
    let target_arch = env::var("CARGO_CFG_TARGET_ARCH").unwrap();
    let url = match (target_os.as_str(), target_arch.as_str()) {
        ("windows", "x86_64") => "...subzero-x86_64.dll",
        ("linux",   "x86_64") => "...llvm-x86_64.so",
        ("macos",   "x86_64") => "...llvm-x86_64.dylib",
        ("macos",   "aarch64") => "...llvm-aarch64.dylib",
        _ => panic!("cpu-fallback not supported on this target"),
    };
    // Fetch to OUT_DIR (cached), verify SHA256, copy alongside binary
}
```

The "..." resolves to a versioned GitHub release URL on the repo
(or a fork that hosts the binaries). Cache via OUT_DIR so subsequent
builds skip the download.

### 4. Refresh policy

SwiftShader is actively maintained (Google CI runs it). Plan for
yearly version refreshes:

- Bump the pinned tag in `build.rs`
- Update SHA256 hashes
- Re-run integration tests on each platform
- Note breaking changes in commit message

Not a hot path — SwiftShader Vulkan compatibility is stable.

### 5. Documentation

- `CLAUDE.md` build section: add `cargo run --features cpu-fallback`
- README: brief blurb on when to use this
- This doc gets the "plan → done" status flip when implementation
  lands

## Performance expectations

From research and existing branch testing on Windows:

- Shader-compile: ~2-5 seconds first time per pipeline variant
  (Reactor JIT). Cached afterward.
- Per-frame rendering: ~10-100× slower than discrete GPU depending
  on resolution and variations active.
- Realistic UX: "fractal previews that resolve in seconds, not
  milliseconds." Acceptable for headless export, CI tests,
  GPU-less machines. Not "feels native."
- Subzero builds (Windows) compile faster than LLVM builds but
  produce slightly slower output (~10-30% slower runtime). For our
  use case (rare fallback path, not hot loop) the compile-time win
  is worth it.

## Open questions (most resolved)

1. ~~SwiftShader licensing~~: **Apache 2.0**, compatible.
2. ~~Binary size~~: ~5 MB (Subzero Windows) + 3 × ~23 MB (LLVM
   elsewhere) = ~75 MB. Use build.rs download to keep out of git
   history.
3. ~~Interactive performance~~: **Not interactive.** Position the
   feature as "render proceeds, just slowly" — fine for CI and
   batch export, not for live editing.
4. ~~WASM CPU priority~~: **Out of scope.** Single-renderer
   constraint wins.
5. **Refresh cadence**: yearly, opportunistically. Bump build.rs
   pin, verify, commit.
6. **CI integration**: should the GPU test suite have a
   `cpu-fallback` matrix run? Confirms the path stays working
   without anyone testing manually on a GPU-less machine.

---

## Research / alternatives evaluated (kept for context)

The plan above came out of evaluating several approaches. Summary
of why each was/wasn't chosen:

| Approach | Code Changes | Perf vs GPU | Desktop | WASM | Verdict |
|---|---|---|---|---|---|
| **SwiftShader/Lavapipe ICD** | None | 10-100× slower | Yes | No | **Chosen** |
| Pure Rust CPU renderer | New code, ~3k lines | 2-200× slower | Yes | Yes | Rejected — second renderer to maintain |
| WGSL → SPIR-V → ISPC | Full rewrite | N/A | N/A | No | Dead end — ISPC doesn't consume SPIR-V |
| Rust-GPU (rspirv) | Full rewrite | ~5-20× slower | Yes | Yes | Rejected — incompatible with our dynamic shader-builder |
| SPIRV-Cross → C++ | Significant | 50-200× slower | Yes | Maybe | Rejected — fragile, poor perf, no atomics |
| SPIR-V interpreters | Significant | 1000× slower | Yes | Maybe | Rejected — debugging only |
| WebGPU polyfill on WebGL2 | Significant | Varies | n/a | Yes | Rejected — WebGL2 has no compute shaders, no atomics |

### Why not pure-Rust CPU renderer

The hand-written Rust renderer was the only approach that could
cover WASM. It was rejected because:

- ~2-3k lines of new Rust to port the existing GPU pipeline (all
  26+ variations × the variation parameter system, tonemap, accum)
- Two parallel render paths to keep in sync as new features land
- Maintenance overhead grows linearly with every shader change
- The WASM-without-WebGPU audience shrinks as Safari + iOS adopt
  WebGPU (Safari 18.2+ has it on by default; iOS 18+ ships it
  flag-gated then default)

If the WASM gap becomes a customer-affecting problem later, the
work is still on the table. But not as a default path.

### Why not Lavapipe instead of SwiftShader

Lavapipe (Mesa's software Vulkan) is also a valid choice. It's
Vulkan 1.3 conformant vs SwiftShader's 1.1+, and on Linux it's a
single `apt install` away. We picked SwiftShader because:

- Prebuilt binaries available for Windows / macOS / Linux. Lavapipe
  requires building Mesa from source on Windows (multi-step,
  fragile).
- Chrome ships SwiftShader, so the Windows build is essentially
  pre-tested by Google's CI matrix.
- License: both Apache 2.0 (Mesa) / Apache 2.0 (SwiftShader) — wash.
- Performance: roughly equivalent. Both use LLVM-based JIT.

If we ever ship Linux packages via apt, including Lavapipe as an
alternative (system-installed) would be a nice option — let
distros provide it instead of bundling. Out of scope for the
initial feature gate; revisit when packaging is on the table.

## Key technical insight

Our `ShaderBuilder` assembles compute shaders at runtime from
templates + active-variation lists. This dynamic shader pipeline
is **perfectly compatible** with software Vulkan ICDs (Lavapipe,
SwiftShader) — they JIT-compile the same SPIR-V naga produces, no
matter how many variants the builder emits.

This dynamic model is **incompatible** with ISPC (requires AOT of
its own language) and Rust-GPU (compiles complete shader programs
at build time). Pre-compiling all 2²⁶ ≈ 67M variation combinations
isn't viable.

## Relevant crates

### Already in dependency tree
| Crate | Role | CPU relevance |
|---|---|---|
| `naga` (via wgpu) | WGSL → SPIR-V | Output goes straight to SwiftShader |
| `wgpu` | GPU abstraction | Transparently uses software Vulkan ICDs |
| `rayon` | CPU parallelism | Already used by HighResExporter; unchanged |
