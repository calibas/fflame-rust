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
REM --symbols must NOT destroy the shipped module. wasm-bindgen writes
REM a fixed filename into --out-dir, so a names build would overwrite
REM the very bundle a crash has to be reproduced with -- and
REM scripts/wasm-locate.py needs BOTH, from the same commit. Park the
REM shipped one here and put it back afterwards.
if "%SYMBOLS%"=="1" (
    if exist "pkg\fractal_flame_wgpu_bg.wasm" (
        move /Y "pkg\fractal_flame_wgpu_bg.wasm" "pkg\_shipped_parked.wasm" >nul
    )
)
cargo build --lib --target wasm32-unknown-unknown --profile %PROFILE%
if %errorlevel% neq 0 exit /b %errorlevel%

REM Generate bindings with wasm-bindgen
echo Generating JavaScript bindings...
wasm-bindgen %BINDGEN_FLAGS% --out-dir ./pkg --target web ./target/wasm32-unknown-unknown/%PROFILE%/fractal_flame_wgpu.wasm
if %errorlevel% neq 0 exit /b %errorlevel%

REM Name the symbols module for the locator, and give the shipped one
REM back so the served bundle is still the one that reproduces.
if "%SYMBOLS%"=="1" (
    move /Y "pkg\fractal_flame_wgpu_bg.wasm" "pkg\fractal_flame_wgpu_bg.names.wasm" >nul
    if exist "pkg\_shipped_parked.wasm" (
        move /Y "pkg\_shipped_parked.wasm" "pkg\fractal_flame_wgpu_bg.wasm" >nul
        echo   names module -^> pkg\fractal_flame_wgpu_bg.names.wasm
        echo   shipped module restored -^> the served bundle is unchanged
    ) else (
        copy /Y "pkg\fractal_flame_wgpu_bg.names.wasm" "pkg\fractal_flame_wgpu_bg.wasm" >nul
        echo   names module -^> pkg\fractal_flame_wgpu_bg.names.wasm
        echo   WARNING: no shipped module was present, so the SERVED bundle
        echo            is the names build -- it will not reproduce the crash.
        echo            Run build-wasm.bat with no flags to restore it.
    )
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
