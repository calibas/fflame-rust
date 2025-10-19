#!/bin/bash
set -e

echo "Building for WASM..."

# Build the WASM module
RUSTFLAGS=--cfg=web_sys_unstable_apis cargo build --lib --target wasm32-unknown-unknown --release

# Generate bindings with wasm-bindgen
echo "Generating JavaScript bindings..."
wasm-bindgen --out-dir ./pkg --target web ./target/wasm32-unknown-unknown/release/fractal_flame_wgpu.wasm

# Optimize the WASM binary (optional, requires wasm-opt)
if command -v wasm-opt &> /dev/null; then
    echo "Optimizing WASM binary..."
    wasm-opt -Oz -o ./pkg/fractal_flame_wgpu_bg.wasm ./pkg/fractal_flame_wgpu_bg.wasm
else
    echo "wasm-opt not found, skipping optimization (install with: cargo install wasm-opt)"
fi

echo "✅ Build complete! Output in ./pkg"
echo ""
echo "To run locally:"
echo "  python3 -m http.server 8080"
echo "  # or"
echo "  npx serve"
echo ""
echo "Then open http://localhost:8080 in your browser"
