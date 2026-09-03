#!/bin/bash
set -e

echo "Building for WASM..."

# Build the WASM module
echo "Building WASM module..."
# NOTE: setting RUSTFLAGS in the environment REPLACES the rustflags
# list in .cargo/config.toml (cargo's flag sources are mutually
# exclusive), so the getrandom cfg and simd128 from there must be
# repeated here.
# -zstack-size: see .cargo/config.toml. wasm-ld defaults to 1 MiB;
# an overflow there traps as a bare "index out of bounds" with no
# panic message, from whichever callback was running.
export RUSTFLAGS='--cfg=web_sys_unstable_apis --cfg getrandom_backend="wasm_js" -C target-feature=+simd128 -C link-arg=-zstack-size=67108864'
PROFILE=dist
BINDGEN_FLAGS=""
if [ "$1" = "--symbols" ]; then
    # dist codegen exactly, symbols kept: the build for a fault that
    # only appears when optimized.
    PROFILE=dist-symbols
    BINDGEN_FLAGS="--keep-debug"
    echo "  (dist codegen + symbols)"
elif [ "$1" = "--debug" ]; then
    # See [profile.dist-debug] in Cargo.toml: symbols, debug
    # assertions and overflow checks, so a browser trap names a
    # function instead of an address.
    PROFILE=dist-debug
    BINDGEN_FLAGS="--keep-debug"
    echo "  (debug profile: symbols + debug_assert + overflow checks)"
fi
cargo build --lib --target wasm32-unknown-unknown --profile "$PROFILE"

if [ $? -ne 0 ]; then
    echo "❌ Cargo build failed"
    exit 1
fi

# Generate bindings with wasm-bindgen
echo "Generating JavaScript bindings..."
wasm-bindgen $BINDGEN_FLAGS --out-dir ./pkg --target web "./target/wasm32-unknown-unknown/$PROFILE/fractal_flame_wgpu.wasm"

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
