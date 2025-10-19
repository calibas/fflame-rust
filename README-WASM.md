# Fractal Flame Renderer - WebAssembly Build

This project can be compiled to WebAssembly and run in web browsers that support WebGPU.

## Prerequisites

1. **Rust with WASM target:**
   ```bash
   rustup target add wasm32-unknown-unknown
   ```

2. **wasm-bindgen-cli:**
   ```bash
   cargo install wasm-bindgen-cli
   ```

3. **(Optional) wasm-opt for optimization:**
   ```bash
   cargo install wasm-opt
   ```

## Building for WASM

### On Windows:
```bash
build-wasm.bat
```

### On Linux/macOS:
```bash
chmod +x build-wasm.sh
./build-wasm.sh
```

### Manual build:
```bash
# Set the required flag for WebGPU
set RUSTFLAGS=--cfg=web_sys_unstable_apis     # Windows
export RUSTFLAGS=--cfg=web_sys_unstable_apis  # Linux/macOS

# Build the WASM library
cargo build --lib --target wasm32-unknown-unknown --release

# Generate JavaScript bindings
wasm-bindgen --out-dir ./pkg --target web ./target/wasm32-unknown-unknown/release/fractal_flame_wgpu.wasm
```

## Running Locally

After building, you need a local web server to test (browsers don't allow WASM to load via `file://`):

### Option 1: Python
```bash
python3 -m http.server 8080
# or on Windows
python -m http.server 8080
```

### Option 2: npx serve
```bash
npx serve
```

### Option 3: Other web servers
Any static file server works. Then open `http://localhost:8080` (or the port your server uses).

## Browser Compatibility

### Full WebGPU Support (Recommended):
- **Chrome/Edge 113+** (stable)
- **Firefox 121+** (with `dom.webgpu.enabled` flag in about:config)
- **Safari 18+** (macOS Ventura+)

### WebGL Fallback:
If your browser doesn't support WebGPU, wgpu will try to fall back to WebGL2. However, some features may be limited.

## Troubleshooting

### "WebGPU is not supported"
- Ensure you're using a recent browser version
- Check if WebGPU is enabled in browser flags
- Try Chrome/Edge which have the best WebGPU support

### Build errors about `web_sys_unstable_apis`
- Make sure `RUSTFLAGS=--cfg=web_sys_unstable_apis` is set before building
- This flag is required for WebGPU support

### Blank screen or black canvas
- Open browser console (F12) to check for errors
- Verify your browser supports WebGPU
- Check that the web server is serving the files correctly

## Features

The WASM build includes all desktop features:
- ✅ Real-time fractal flame rendering
- ✅ Interactive pan/zoom (mouse & keyboard)
- ✅ Transform editing UI
- ✅ Color palette system
- ✅ Progressive accumulation
- ✅ Multiple coloring modes

## Performance Notes

- WASM performance is generally good but slightly slower than native
- First load may take a few seconds to compile the WASM module
- WebGPU performance varies by browser and GPU
- Chrome generally has the best WebGPU performance
- For best results, use a dedicated GPU

## Deployment

To deploy to a web server, copy these files:
- `index.html`
- `pkg/fractal_flame_wgpu.js`
- `pkg/fractal_flame_wgpu_bg.wasm`

That's it! No other assets are required - all shaders and palettes are embedded in the WASM binary.
