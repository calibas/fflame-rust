# CPU Rendering Research

**Date:** 2026-02-14
**Status:** Research / Feasibility Analysis

## Motivation

Currently, the fractal flame renderer requires a GPU with compute shader support (WebGPU or Vulkan/DX12/Metal). This means:
- **Desktop**: No rendering on machines without capable GPUs or drivers
- **WASM**: No fallback when WebGPU is unavailable in the browser (older Safari, some mobile browsers)

A CPU rendering backend would provide a universal fallback path.

## Original Hypothesis: WGSL → SPIR-V → ISPC

The idea: take our WGSL compute shaders, convert to SPIR-V (which naga already does internally), then use ISPC to run them on the CPU with SIMD vectorization.

### Verdict: Dead End

**ISPC does not accept SPIR-V as input.** This is the fundamental blocker.

- ISPC has its own `.ispc` language and that is its only input format
- ISPC *can emit* SPIR-V (since v1.22, 2023) for targeting Intel GPUs — this is the **reverse** direction
- Intel DPC++/SYCL can consume SPIR-V for CPU+GPU but is a full C++ compiler ecosystem, not a shader translator
- There is no "Intel SPIR-V to ISPC Compiler" product

A manual rewrite of all shaders to ISPC would be impractical because our `ShaderBuilder` dynamically assembles shaders at runtime from templates with `{{VARIATIONS_CODE}}` placeholders. ISPC requires ahead-of-time compilation, creating a fundamental model mismatch.

---

## Alternative Approaches Evaluated

### 1. Software Vulkan ICDs (Lavapipe / SwiftShader)

**Best option for desktop CPU fallback. Zero code changes required.**

wgpu's Vulkan backend doesn't care whether the underlying Vulkan driver is hardware or software. If a software Vulkan ICD (Installable Client Driver) is present, wgpu uses it transparently.

#### Lavapipe (Mesa)
- Mesa's software Vulkan implementation, uses LLVM to JIT-compile SPIR-V to native code
- **Vulkan 1.3 conformant** — production-grade
- Full compute shader support including atomics, storage textures, workgroup dispatch
- Linux: easy (install `mesa-vulkan-drivers`), Windows: requires building Mesa from source
- **Performance**: 20-100x slower than discrete GPU (LLVM JIT, no hardware parallelism)

#### SwiftShader (Google)
- Google's CPU-based Vulkan implementation (Vulkan 1.1+)
- Uses LLVM-based Reactor JIT engine
- Prebuilt binaries available for Windows, Linux, macOS — **easier to distribute**
- Widely used in Chrome's GPU testing infrastructure
- **Performance**: 10-100x slower than GPU, comparable to Lavapipe

#### Integration
```rust
// No code changes needed. wgpu adapter selection handles it:
let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
    power_preference: wgpu::PowerPreference::HighPerformance,
    ..Default::default()
}).await;
// If no hardware GPU exists but a software ICD is present,
// wgpu returns the software adapter. Pipeline runs unchanged.
```

The adapter will report `DeviceType::Cpu` which we could detect and display in the UI (e.g., "CPU rendering mode - performance will be reduced").

#### Limitations
- **WASM: Not viable.** Both depend on LLVM JIT to generate native machine code at runtime. The WASM sandbox prevents executing dynamically generated native code.
- First shader compilation is slow (multiple seconds for LLVM JIT)
- Subsequent frames use compiled native code with SIMD optimization
- Still usable for batch export, CI testing, GPU-less machines

### 2. Pure Rust CPU Implementation → WASM

**Only viable path for WASM without WebGPU.**

A hand-written Rust implementation of the flame algorithm, compiled to `wasm32`, is the only way to render fractals in browsers that lack WebGPU.

#### What Already Exists
- `src/scene/transforms.rs` — CPU-side flame algorithm with variation dispatch
- `src/export/high_res.rs` — CPU histogram accumulation using rayon

#### What Would Need to Be Built
- All 26 variation implementations in Rust (some exist in `transforms.rs`)
- Tone mapping pass in Rust
- Accumulation/blending logic
- Color mode handling (palette lookup, speed-based coloring)
- Full 3D path (camera rotation, projection)
- Function-pointer dispatch for variations (replaces dynamic shader compilation)

#### Performance Tiers in WASM

| Configuration | Est. Slowdown vs WebGPU | Browser Support |
|---|---|---|
| Single-threaded WASM | 50-200x slower | Universal |
| Multi-threaded (Web Workers + SharedArrayBuffer) | 5-30x slower | Requires COOP/COEP headers |
| Multi-threaded + WASM SIMD (128-bit) | 2-10x slower | Chrome, Firefox, recent Safari |

#### Multi-threading Caveats
- `SharedArrayBuffer` requires specific HTTP headers (`Cross-Origin-Embedder-Policy: require-corp`, `Cross-Origin-Opener-Policy: same-origin`)
- Not available in all deployment contexts (some CDNs, iframes)
- `wasm-bindgen-rayon` provides Rust→WASM worker bridge but is still maturing

#### WASM SIMD
- `wasm32` supports 128-bit SIMD (comparable to SSE2)
- Rust's `wide` crate (stable) or `std::simd` (nightly) can target this
- 2-4x improvement for vectorizable math (affine transforms, variations)

### 3. SPIRV-Cross to C++ → Compile

**Fragile, poor performance. Not recommended.**

SPIRV-Cross can translate SPIR-V to C++, but:
- Output is scalar (no SIMD, no vectorization)
- Intended for reflection/debugging, not production execution
- Texture operations stubbed out, atomics need manual adaptation
- Buffer binding semantics not handled
- Fragile across shader changes

Pipeline: `WGSL → naga → SPIR-V → spirv-cross → C++ → compile → run`

Could theoretically compile to WASM via Emscripten, but performance would be terrible (scalar non-vectorized code in an interpreter).

### 4. Rust-GPU

**Architecturally elegant but impractical for this project.**

rust-gpu allows writing GPU shaders in Rust that compile to SPIR-V. The same source could compile to native CPU code or wasm32.

Problems:
- Requires **complete rewrite** of all WGSL shaders in Rust-GPU's limited Rust subset
- **Fundamentally incompatible** with our dynamic shader compilation model (ShaderBuilder + templates)
- Project has been alpha/experimental since 2020 with frequent breaking changes
- Embark Studios reduced investment; community maintenance continues slowly

### 5. SPIR-V Interpreters

**Debugging only. Orders of magnitude too slow.**

- **spirv-vm** (nicbarker): Simple C interpreter, very early/experimental
- **Talvos** (University of Bristol): SPIR-V interpreter + Vulkan emulator, research project

Both are 1000x+ slower than GPU. Not viable for any rendering use case.

### 6. WebGPU Polyfills

**Do not exist.** WebGL 2.0 does not support compute shaders at all. There is no WebGL-based polyfill for WebGPU compute.

### 7. Ahead-of-Time SPIR-V → WASM

**Theoretical only.** No production-quality standalone SPIR-V-to-LLVM-IR tool exists. Mesa's shader compiler is deeply integrated into its driver stack. The execution model mismatch (GPU thousands of threads vs WASM sequential/limited workers) would need full emulation of GPU primitives.

---

## Comparative Summary

| Approach | Code Changes | Perf vs GPU | Desktop | WASM | Maturity | Effort |
|---|---|---|---|---|---|---|
| **SwiftShader/Lavapipe** | None | 10-100x slower | Yes | No | Production | Low |
| **Pure Rust CPU** | New code | 2-200x slower* | Yes | Yes | You build it | Medium-High |
| **ISPC** | Full rewrite | N/A | N/A | No | Wrong tool | N/A |
| **Rust-GPU** | Full rewrite | ~5-20x slower | Yes | Yes | Alpha | Extreme |
| **SPIRV-Cross → C++** | Significant | 50-200x slower | Yes | Maybe | Fragile | High |
| **SPIR-V interpreters** | Significant | 1000x+ slower | Yes | Maybe | Experimental | Medium |

*Pure Rust: 2-10x with WASM SIMD + workers, 50-200x single-threaded WASM

---

## Recommended Strategy

### Phase 1: Desktop CPU Fallback (Low effort, high value)

**Bundle SwiftShader as a software Vulkan ICD.**

- Zero code changes to the renderer
- Entire 3-pass pipeline runs unchanged
- SwiftShader has prebuilt binaries for Windows/Linux/macOS
- Detect `DeviceType::Cpu` adapter and show user warning about reduced performance
- Good for: CI testing, headless servers, machines with broken GPU drivers

Implementation:
1. Ship SwiftShader ICD alongside the application binary
2. Set `VK_ICD_FILENAMES` environment variable to point to it when no hardware GPU found
3. Add UI indicator: "Software rendering (CPU)" when detected
4. Optionally reduce default iterations/workgroups for acceptable interactive performance

### Phase 2: WASM — Chrome SwiftShader Backend (May resolve itself)

**Chrome is implementing SwiftShader as a WebGPU software backend.** If shipped, this means Chrome would provide CPU-based WebGPU transparently in the browser — our WASM build would "just work" without any code changes, identical to the desktop software Vulkan story.

This would cover the primary WASM use case (Chrome on machines without WebGPU-capable hardware). Safari and Firefox may follow with their own software backends, or may not.

**Impact on Phase 2 scope**: If Chrome ships this, the need for a hand-written Rust CPU renderer drops significantly. The remaining gap would only be non-Chrome browsers without WebGPU — an increasingly small audience as WebGPU adoption grows.

**Recommendation**: Wait and see on Chrome's SwiftShader-in-WASM progress before investing in a pure Rust CPU renderer. The desktop SwiftShader bundling (Phase 1) provides immediate value with minimal effort.

### Phase 2 (Contingency): Pure Rust CPU Fallback

**Only needed if browser software WebGPU backends don't materialize.**

Pure Rust CPU renderer compiled to wasm32.

Architecture:
```
┌─────────────────────────────────────────┐
│            FractalRenderer (trait)        │
├─────────────────────────────────────────┤
│  ┌───────────────┐  ┌────────────────┐  │
│  │  GpuRenderer  │  │  CpuRenderer   │  │
│  │  (existing)   │  │  (new)         │  │
│  │  wgpu-based   │  │  pure Rust     │  │
│  │  3-pass GPU   │  │  rayon/SIMD    │  │
│  └───────────────┘  └────────────────┘  │
└─────────────────────────────────────────┘
```

Key design decisions:
- **Variation dispatch**: Function pointer table (replaces dynamic shader compilation)
- **Parallelism**: `rayon` on native, Web Workers on WASM
- **SIMD**: `wide` crate for portable 128-bit SIMD (works on native + WASM)
- **Accumulation**: Direct f32 histogram (no atomic u32 encoding needed on CPU)
- **Progressive rendering**: Yield control periodically for UI responsiveness

Build on existing code:
- `src/scene/transforms.rs` — already has CPU variation dispatch
- `src/export/high_res.rs` — already has CPU histogram accumulation with rayon

#### Estimated Scope
- ~2000-3000 lines of new Rust code
- All 26 variations in Rust (partially exists)
- CPU tone mapping (port from WGSL)
- CPU accumulation with progressive blending
- Renderer trait abstraction to share code between GPU and CPU paths
- WASM worker integration for multi-threading

---

## Key Technical Insight: Dynamic Shader Compilation

Our `ShaderBuilder` assembles shaders at runtime by:
1. Inserting hard-coded constants (`NUM_TRANSFORMS`, `COLOR_MODE`, etc.)
2. Including only active variation functions
3. Processing conditional templates (`{{#if RENDER_3D}}`)
4. Optionally inlining transform data as shader constants

This dynamic model is **perfectly compatible** with:
- **Software Vulkan** (Lavapipe/SwiftShader) — they JIT-compile the same SPIR-V
- **Pure Rust CPU** — use function pointer dispatch instead

It is **incompatible** with:
- **ISPC** — requires ahead-of-time compilation of its own language
- **Rust-GPU** — compiles entire shader programs at build time
- **Pre-compiled approaches** — would need 2^26 = 67M variation combinations

---

## Relevant Crates

### Already in dependency tree
| Crate | Role | CPU Relevance |
|---|---|---|
| `naga` (via wgpu) | WGSL→SPIR-V | Produces SPIR-V for software Vulkan |
| `wgpu` | GPU abstraction | Transparently uses software Vulkan ICDs |
| `rayon` | CPU parallelism | Powers CPU fallback rendering |

### Potentially useful additions
| Crate | Purpose | WASM? |
|---|---|---|
| `wide` | Portable SIMD (stable, no nightly) | Yes (WASM SIMD) |
| `glam` | Fast math (vec2/vec3/mat4) with SIMD | Yes |
| `wasm-bindgen-rayon` | Rayon on WASM via Web Workers | Yes (purpose-built) |

---

## Open Questions

1. **SwiftShader licensing**: Apache 2.0 — compatible with bundling
2. **SwiftShader binary size**: ~10-20 MB per platform (acceptable?)
3. **Interactive performance on CPU**: Is 10-100x slower usable for interactive editing, or only for batch export?
4. **WASM CPU priority**: How important is supporting browsers without WebGPU? WebGPU adoption is growing rapidly (Chrome 113+, Firefox 121+, Safari experimental)
5. **Renderer trait design**: Should we abstract at the `FlameRenderer` level or create a separate `CpuFlameRenderer`?
