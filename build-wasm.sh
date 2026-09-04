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
    SYMBOLS=1
    echo "  (dist codegen + symbols)"
elif [ "$1" = "--debug" ]; then
    # See [profile.dist-debug] in Cargo.toml: symbols, debug
    # assertions and overflow checks, so a browser trap names a
    # function instead of an address.
    PROFILE=dist-debug
    BINDGEN_FLAGS="--keep-debug"
    echo "  (debug profile: symbols + debug_assert + overflow checks)"
fi
# --symbols must NOT destroy the shipped module. wasm-bindgen writes a
# fixed filename into --out-dir, so a names build would overwrite the
# very bundle a crash has to be reproduced with -- and
# scripts/wasm-locate.py needs BOTH, from the same commit. Park the
# shipped one here and put it back afterwards.
if [ "$SYMBOLS" = "1" ] && [ -f pkg/fractal_flame_wgpu_bg.wasm ]; then
    mv -f pkg/fractal_flame_wgpu_bg.wasm pkg/_shipped_parked.wasm
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

# Name the symbols module for the locator, and give the shipped one
# back so the served bundle is still the one that reproduces.
if [ "$SYMBOLS" = "1" ]; then
    mv -f pkg/fractal_flame_wgpu_bg.wasm pkg/fractal_flame_wgpu_bg.names.wasm
    echo "  names module -> pkg/fractal_flame_wgpu_bg.names.wasm"
    if [ -f pkg/_shipped_parked.wasm ]; then
        mv -f pkg/_shipped_parked.wasm pkg/fractal_flame_wgpu_bg.wasm
        echo "  shipped module restored -> the served bundle is unchanged"
    else
        cp -f pkg/fractal_flame_wgpu_bg.names.wasm pkg/fractal_flame_wgpu_bg.wasm
        echo "  WARNING: no shipped module was present, so the SERVED bundle"
        echo "           is the names build -- it will not reproduce the crash."
        echo "           Run ./build-wasm.sh with no flags to restore it."
    fi
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
