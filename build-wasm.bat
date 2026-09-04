@echo off
echo Building for WASM...

REM Build the WASM module
REM RUSTFLAGS in the environment REPLACES .cargo/config.toml's list, so
REM the getrandom cfg and simd128 from there are repeated here.
REM -zstack-size: see .cargo/config.toml (wasm-ld defaults to 1 MiB).
set RUSTFLAGS=--cfg=web_sys_unstable_apis --cfg getrandom_backend="wasm_js" -C target-feature=+simd128 -C link-arg=-zstack-size=67108864
REM --debug selects [profile.dist-debug] (Cargo.toml): same
REM optimisation level and simd/codegen shape, but symbols kept,
REM panics unwound through the console hook, and debug assertions plus
REM overflow checks ON -- so a browser trap names a function instead of
REM an address, and a wrapping subtraction fails at its source. Much
REM bigger and slower; not for shipping.
set PROFILE=dist
set BINDGEN_FLAGS=
if /I "%~1"=="--symbols" (
    REM dist codegen exactly, symbols kept: the build for a fault that
    REM only appears when optimized.
    set PROFILE=dist-symbols
    set BINDGEN_FLAGS=--keep-debug
    set SYMBOLS=1
    echo   ^(dist codegen + symbols^)
)
if /I "%~1"=="--debug" (
    set PROFILE=dist-debug
    set BINDGEN_FLAGS=--keep-debug
    echo   ^(debug profile: symbols + debug_assert + overflow checks^)
)
cargo build --lib --target wasm32-unknown-unknown --profile %PROFILE%
if %errorlevel% neq 0 exit /b %errorlevel%

REM Generate bindings with wasm-bindgen
echo Generating JavaScript bindings...
REM --symbols writes to its OWN directory. wasm-bindgen emits the JS
REM glue alongside the module and the two are a matched pair -- the
REM import object is generated from the specific module it processed --
REM so a names build landing in ./pkg replaces BOTH, and restoring only
REM the .wasm leaves glue that does not link:
REM   LinkError: import object field '__wbindgen_object_drop_ref' is
REM   not a Function
REM Keeping it out of ./pkg entirely means the served bundle is never
REM touched, in any order, and the locator gets its second module.
set OUTDIR=./pkg
if "%SYMBOLS%"=="1" set OUTDIR=./pkg-names
if not exist "pkg-names" mkdir "pkg-names"
wasm-bindgen %BINDGEN_FLAGS% --out-dir %OUTDIR% --target web ./target/wasm32-unknown-unknown/%PROFILE%/fractal_flame_wgpu.wasm
if %errorlevel% neq 0 exit /b %errorlevel%

REM Hand the names module to scripts/wasm-locate.py under the filename
REM it looks for, and leave everything else in ./pkg alone.
if "%SYMBOLS%"=="1" (
    copy /Y "pkg-names\fractal_flame_wgpu_bg.wasm" "pkg\fractal_flame_wgpu_bg.names.wasm" >nul
    echo.
    echo   names module -^> pkg\fractal_flame_wgpu_bg.names.wasm
    echo   ./pkg is untouched: the served bundle is still the shipped build.
    echo   Reproduce with it, then: python scripts/wasm-locate.py ^<offset^>
    goto :eof
)

REM Copy assets for runtime loading
echo Copying assets...
if not exist "pkg\assets\palettes\packs" mkdir "pkg\assets\palettes\packs"
xcopy /Y /Q "assets\palettes\packs\*.json" "pkg\assets\palettes\packs\" >nul 2>&1

echo.
echo Build complete! Output in ./pkg
echo.
echo To run locally:
echo   python -m http.server 8080
echo   # or
echo   npx serve
echo.
echo Then open http://localhost:8080 in your browser
