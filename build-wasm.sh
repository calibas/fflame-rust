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
# Not `export`: passed inline to cargo below, so these flags cannot
# escape into the caller if this script is ever `source`d. (The .bat
# needs `setlocal` for the same reason -- it always runs in the
# calling shell, and a leaked RUSTFLAGS makes a later native build
# warn "+simd128 is not a recognized feature" once per crate.)
WASM_RUSTFLAGS='--cfg=web_sys_unstable_apis --cfg getrandom_backend="wasm_js" -C target-feature=+simd128 -C link-arg=-zstack-size=67108864'
PROFILE=dist
BINDGEN_FLAGS=""
# Reset the flag every run: an exported SYMBOLS from the environment
# would otherwise send a plain build into pkg-names and leave ./pkg
# untouched, which fails silently and looks like a broken script.
SYMBOLS=
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
RUSTFLAGS="$WASM_RUSTFLAGS" cargo build --lib --target wasm32-unknown-unknown --profile "$PROFILE"

if [ $? -ne 0 ]; then
    echo "❌ Cargo build failed"
    exit 1
fi

# Generate bindings with wasm-bindgen
echo "Generating JavaScript bindings..."
# --symbols writes to its OWN directory. wasm-bindgen emits the JS glue
# alongside the module and the two are a matched pair -- the import
# object is generated from the specific module it processed -- so a
# names build landing in ./pkg replaces BOTH, and restoring only the
# .wasm leaves glue that does not link:
#   LinkError: import object field '__wbindgen_object_drop_ref' is not
#   a Function
# Keeping it out of ./pkg entirely means the served bundle is never
# touched, in any order.
OUTDIR=./pkg
if [ "$SYMBOLS" = "1" ]; then OUTDIR=./pkg-names; mkdir -p pkg-names; fi
# ./pkg from scratch: wasm-bindgen only adds files, and what an older
# build left there (wasm-pack's package.json and README, a 17 MiB
# names module) was copied and deployed with every new one.
if [ "$SYMBOLS" != "1" ]; then rm -rf pkg; fi
wasm-bindgen $BINDGEN_FLAGS --out-dir "$OUTDIR" --target web "./target/wasm32-unknown-unknown/$PROFILE/fae.wasm"

if [ $? -ne 0 ]; then
    echo "❌ wasm-bindgen failed"
    exit 1
fi

# A names build stops here: nothing of it goes into ./pkg. (It used to
# copy its module to pkg/*_bg.names.wasm for scripts/wasm-locate.py,
# which has read only the shipped module since it stopped comparing
# builds -- and the copy left a second 19 MiB module in the directory a
# deploy uploads.)
if [ "$SYMBOLS" = "1" ]; then
    echo ""
    echo "  names build -> ./pkg-names (module + JS glue, a matched pair)"
    echo "  ./pkg is untouched: the served bundle is still the shipped build."
    echo "  Serve ./pkg-names in place of ./pkg to reproduce; a trap names its function."
    exit 0
fi

# Copy assets for runtime loading
echo "Copying assets..."
mkdir -p pkg/assets/palettes/packs
cp assets/palettes/packs/*.json pkg/assets/palettes/packs/

# pkg/version.json: the version, commit and profile, which a deploy reads
# back from the live site (scripts/write_version_json.py). `python` first:
# on Windows `python3` can be the Store's stub, which only prints a hint.
PY=$(command -v python || command -v python3)
"$PY" scripts/write_version_json.py pkg --profile "$PROFILE"

echo ""
echo "✅ Build complete! Output in ./pkg"
echo ""
echo "To run locally:"
echo "  python3 -m http.server 8080"
echo "  # or"
echo "  npx serve"
echo ""
echo "Then open http://localhost:8080 in your browser"
