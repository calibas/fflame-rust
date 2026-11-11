# wasm/python — pyfflame's engine module

The engine built for WASI (`wasm32-wasip1`): the module the Python
package in [`python/`](../../python/README.md) runs with wasmtime. Every
engine is in it (flames, escape-time, simulations, terrains), with the
animation engine and Rhai.

```bash
cargo build --release      # here: .cargo/config.toml selects wasm32-wasip1
python python/build.py     # from the root: builds this and puts it in the package
```

- `src/lib.rs` — the one export, `fflame_call` (a JSON request and a
  block of bytes), and the calls it answers.
- `src/backend.rs` — a wgpu custom backend: every GPU call becomes a
  JSON command for the host (`python/pyfflame/_gpu.py`, over wgpu-py).
- `src/render.rs` — stills, Python-driven sequences, and animations as
  the video export renders them.
- `src/scripts.rs` — Rhai, as the gallery's script module runs it.

The design, the measurements and the copied code awaiting review:
[docs/archive/projects/pyfflame.md](../../docs/archive/projects/pyfflame.md).
