# 🚀 Quick Start - WASM Build

## TL;DR

```bash
# 1. Setup (once)
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli

# 2. Build
build-wasm.bat              # Windows
# or
./build-wasm.sh             # Linux/macOS

# 3. Serve
python -m http.server 8080

# 4. Open
# http://localhost:8080
```

## That's It!

Open your browser to see the fractal flame renderer running with:
- Full GPU acceleration (WebGPU)
- Interactive pan/zoom
- Live transform editing
- 5 color palettes
- All 16 variation functions

**Note**: Use Chrome 113+ for best results.

---

See [BUILD-SUCCESS.md](BUILD-SUCCESS.md) for detailed info.
