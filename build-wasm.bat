@echo off
echo Building for WASM...

REM Build the WASM module
set RUSTFLAGS=--cfg=web_sys_unstable_apis
cargo build --lib --target wasm32-unknown-unknown --release --features api
if %errorlevel% neq 0 exit /b %errorlevel%

REM Generate bindings with wasm-bindgen
echo Generating JavaScript bindings...
wasm-bindgen --out-dir ./pkg --target web ./target/wasm32-unknown-unknown/release/fractal_flame_wgpu.wasm
if %errorlevel% neq 0 exit /b %errorlevel%

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
