#!/bin/bash
set -e

echo "Building for WASM..."

# Build the WASM module
echo "Building WASM module..."
export RUSTFLAGS=--cfg=web_sys_unstable_apis
cargo build --lib --target wasm32-unknown-unknown --release --features api

if [ $? -ne 0 ]; then
    echo "❌ Cargo build failed"
    exit 1
fi

# Generate bindings with wasm-bindgen
echo "Generating JavaScript bindings..."
wasm-bindgen --out-dir ./pkg --target web ./target/wasm32-unknown-unknown/release/fractal_flame_wgpu.wasm

if [ $? -ne 0 ]; then
    echo "❌ wasm-bindgen failed"
    exit 1
fi

# Copy assets for runtime loading
echo "Copying assets..."
mkdir -p pkg/assets/palettes/packs
cp assets/palettes/packs/*.json pkg/assets/palettes/packs/

echo ""
echo "✅ Build complete! Output in ./pkg"
echo ""
echo "To run locally:"
echo "  python3 -m http.server 8080"
echo "  # or"
echo "  npx serve"
echo ""
echo "Then open http://localhost:8080 in your browser"
