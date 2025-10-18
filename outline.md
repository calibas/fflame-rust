# Rust + wgpu Fractal Flame Renderer — Project Architecture

**Purpose:**
A project-scoped architecture document describing the design, file layout, data formats, pipeline, and development milestones for a GPU-accelerated fractal-flame renderer implemented in **Rust** using **wgpu** (WebGPU). The target is an interactive desktop application with optional WebAssembly (browser) demo later.

---

## 1 — High-level goals

- Real-time, interactive fractal flame exploration at 720p–1440p with progressive refinement.
- Deterministic seeds and reproducible scenes.
- Cross-platform: Windows, Linux, macOS (including Apple Silicon); optional Web via WASM.
- Modular code structure so compute kernels, UI, and export pipelines are separable and testable.
- Provide fallbacks for hardware lacking float-atomic support (per-workgroup reduce strategy).

---

## 2 — Tech stack

- Language: **Rust** (stable)
- GPU API: **wgpu** (latest stable compatible with target platforms)
- Windowing / Input: **winit**
- Immediate-mode UI: **egui** (via egui-winit + egui-wgpu integration)
- Serialization & config: **serde** + **ron** (or JSON)
- CLI / app boot: **clap** (for command-line render/export options)
- Optional: **wasm-bindgen** and **wasm-pack** for browser build
- Tooling: **cargo**, **rustfmt**, **clippy**, CI (GitHub Actions)

---

## 3 — Top-level repo layout

```
fractal-flame-wgpu/
├── Cargo.toml
├── README.md
├── docs/
│   └── design_notes.md
├── assets/
│   ├── palettes/        # color LUTs (RON/JSON or 1D texture files)
│   └── presets/         # saved transform sets
├── src/
│   ├── main.rs          # entrypoint (app init, event loop)
│   ├── app.rs           # App struct, state machine
│   ├── ui/
│   │   ├── mod.rs
│   │   └── panels.rs    # UI controls, presets, palette editor
│   ├── gpu/
│   │   ├── mod.rs
│   │   ├── device.rs    # wgpu device and queue setup, feature checks
│   │   ├── pipelines.rs # pipeline creation: compute + render pipelines
│   │   └── buffers.rs   # buffer / texture layout & helpers
│   ├── scene/
│   │   ├── mod.rs
│   │   ├── transforms.rs# transform definitions, variations
│   │   └── presets.rs
│   ├── renderer/
│   │   ├── mod.rs
│   │   ├── compute_kernel.rs # kernels invocations, dispatch control
│   │   └── postprocess.rs    # tonemapping, splatting, filters
│   ├── io/
│   │   ├── export.rs    # high-res tiled export
│   │   └── persistence.rs # save/load presets
│   └── util.rs
├── shaders/             # WGSL shader files (compute + fragment)
│   ├── trajectory.wgsl
│   ├── reduce.wgsl
│   └── tone_map.wgsl
└── examples/
    └── headless_export.rs # small binary for offline tiled rendering
```

Notes:
- Keep `gpu/` focused on wgpu resource management; `renderer/` orchestrates compute + render flow.
- `scene/transforms.rs` should implement transform + variation definitions in pure Rust so CPUside testing is trivial.

---

## 4 — Core data structures and formats

### 4.1 Transform (per-IFS transform)

Rust struct (conceptual):

```text
Transform {
    // 2x2 linear matrix
    a: f32, b: f32,
    c: f32, d: f32,
    // translation
    e: f32, f: f32,
    // weight (probability)
    weight: f32,
    // variation weights (vector of e.g., 16 variations max)
    variations: [f32; VAR_COUNT],
    // color contribution or palette index
    color: [f32; 3],
}
```

- Serialized to RON/JSON for presets.
- Packed into a `std430`-style structure for GPU buffers (align to vec4 boundaries).

### 4.2 Palette / LUT

- Provide a 1D texture (e.g., 2048×1 RGBA32F) uploaded to GPU for palette lookup.
- Also keep a small CPU-side representation for editing & saving.

### 4.3 Accumulation buffer

- Texture: `RGBA32Float` (or `Rgba32Uint` with fixed-point strategy fallback).
- Layout: same size as output resolution; each pixel stores accumulated R,G,B and sample count (or luminosity).
- Optionally: separate buffers for color moments or variance if advanced denoising is implemented.

### 4.4 Work dispatch parameters

Packed into a small UBO or storage buffer: seed, iterations per-thread, samples per-dispatch, view transform (zoom/offset), burn-in, splat size.

---

## 5 — GPU pipeline & shaders

### 5.1 Pipelines (wgpu)

- **Compute pipeline: `trajectory`**
  - Input: transforms SSBO, RNG seeds, dispatch params
  - Output: accumulation texture (via imageStore / atomic adds) or intermediate per-tile storage
  - Work: each invocation runs a small trajectory loop (e.g., 64–512 iterations), applies variations, then writes splats to accumulation.

- **Compute pipeline: `reduce`**
  - If per-workgroup shared histograms are used, a reduce pass merges tiles into final accumulation texture.

- **Render pipeline: `tonemap`**
  - Fullscreen fragment shader that reads accumulation texture, applies log mapping and palette lookup, tonemapping, and outputs to swapchain.

- **Optional compute: `denoise/upsample`** (for progressive previews)

### 5.2 Shader design notes

- Use WGSL (preferred by wgpu) for portability.
- Avoid double-precision math in GPU code; prefer single precision and careful numerical handling.
- Implement RNG in shader (PCG or xorshift-based) with per-thread state.
- Implement variation functions as discrete functions (spherical, swirl, sinusoidal, etc.) — call them by an integer ID or function table.
- For float-atomic add: try atomic additions on `atomic<f32>` when supported; otherwise implement integer fixed-point accumulation or per-workgroup accumulation.

---

## 6 — CPU-side render orchestration

`renderer::compute_kernel.rs` responsibilities:

- Manage dispatch lifecycle: spawn many dispatches progressively until target sample budget reached.
- Handle accumulation buffer double-buffering if needed for UI responsiveness (ping-pong)
- Trigger reduce passes when necessary.
- Map small readbacks for histogram/statistics only (avoid reading full image until export).
- Accept UI commands to change parameters: if transforms change, optionally reset accumulation or implement temporal blending.

---

## 7 — UI and UX

Key panels:

- **Viewport**: main canvas showing progressive result; supports pan/zoom, sampling overlay, and FPS/iterations display.
- **Transforms list**: add/remove transforms, matrix editor, weight slider, variation pickers, color pickers, duplicate/clone.
- **Palette editor**: gradient stops + preview; save/load palettes.
- **Global params**: samples per dispatch, burn-in, splat radius, exposure/gamma, tonemapping.
- **Presets / randomize**: browse saved presets; quick-random button with seeded reproducibility.
- **Export**: high-res export UI with tile size, sample budget, and async progress.

UI tech: `egui` with docking panels (or simple vertical layout). Keep UI decoupled from render loop (state events buffered).

---

## 8 — Export & high-resolution rendering

Strategy:

- Implement tiled rendering: allocate a larger accumulation buffer per tile or reuse a single large floating-point buffer streamed per tile.
- For each tile, set view transform accordingly and run many more iterations to converged density.
- Optionally run a CPU-side pass for final color correction, sharpening, or apply additional anti-aliasing if needed.
- Save result as 16-bit/channel PNG or OpenEXR (for HDR fidelity) using `image` crate or `exr` crate.

---

## 9 — Fallbacks & compatibility

- Detect GPU features in `gpu/device.rs`:
  - float atomic support
  - storage texture formats
  - max workgroup sizes, max storage buffer sizes
- If float-atomic is unavailable/slow:
  - Use per-workgroup shared memory histograms + single atomic adds per tile
  - Use 32-bit integer fixed-point accumulation (e.g., multiply by 256 or 1024 and store as uint)
- Reduce shader variants built at startup based on capabilities. Keep pipeline creation centralized.

---

## 10 — Testing & validation

- Unit tests for CPU-side transform application and variation math (reproduce reference CPU implementation).
- Visual regression tests: render deterministic seeds to small resolutions and compare checksums (allow small tolerance).
- Performance tests: measure samples/sec on CI with predefined hardware if available (or local). Use profile flags.

---

## 11 — Profiling & optimization checklist

- Profile to find hotspots: compute shader time, memory bandwidth, atomic contention — use platform-specific tools (RenderDoc for frame capture, NVIDIA Nsight, Xcode GPU frame capture on mac).
- Try these optimizations in order:
  1. Switch to per-workgroup histograms.
  2. Increase iterations-per-thread to reduce dispatch overhead.
  3. Tune workgroup size for occupancy.
  4. Use smaller splat kernel for interactive preview.
  5. Add multi-resolution progressive rendering.

---

## 12 — Milestones (project-level)

1. **Repo skeleton:** wgpu + winit + egui setup, window + swapchain, basic UI panel.
2. **CPU reference:** Simple CPU flam3 engine (tiny) to verify transforms and color mapping.
3. **Minimal GPU point plot:** Compute shader that generates a few thousand points and writes as points to a texture; display via tonemap shader.
4. **Accumulation pass:** Implement accumulation buffer with per-workgroup reduce and progressive display.
5. **UI integration:** Connect transforms UI and palette editor to GPU buffers; support presets.
6. **Export:** Implement tiled high-res export and save as EXR/PNG.
7. **Performance tuning & cross-platform testing.**

---

## 13 — Security & privacy

- Sandbox WASM builds carefully: avoid allowing arbitrary file write in browser context.
- Be cautious when loading presets from untrusted sources — parse with safe deserializers and validate numeric ranges.

---

## 14 — Future expansions

- Add CUDA backend for NVIDIA for much faster accumulation if targeting desktops only.
- Integrate compute-graph scheduler for dynamic load balancing on hybrid CPU/GPU.
- Add animation tools: morphing between transforms and keyframe timelines.
- Implement layered compositing and vector field-guided splatting for artist workflows.

---

## 15 — Appendix: quick buffer & bind layout (conceptual)

**Bind group 0 — Scene / transforms**
- `0` : transforms storage buffer (array of packed Transform)
- `1` : palette texture (1D) + sampler
- `2` : params uniform buffer (seed, resolution, view transform, iteration counts)

**Bind group 1 — Accumulation target**
- `0` : accumulation storage texture (RGBA32Float) — image/texture2D write access

**Bind group N — reduce / staging**
- per-workgroup staging buffer (only bound during reduce pass)


---