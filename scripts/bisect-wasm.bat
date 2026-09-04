@echo off
REM One bisect step: build the SHIPPED dist profile and refresh ./pkg.
REM
REM Deliberately NOT build-wasm.bat. That script sets RUSTFLAGS from
REM whatever commit is checked out, and older commits carry a 1 MiB
REM shadow stack -- which caused a DIFFERENT wasm crash. Pinning the
REM stack here holds that variable still, so a bisect converges on the
REM load-time freeze rather than rediscovering the stack bug.
set RUSTFLAGS=--cfg=web_sys_unstable_apis --cfg getrandom_backend="wasm_js" -C target-feature=+simd128 -C link-arg=-zstack-size=67108864
cargo build --lib --target wasm32-unknown-unknown --profile dist
if %errorlevel% neq 0 exit /b %errorlevel%
wasm-bindgen --out-dir ./pkg --target web ./target/wasm32-unknown-unknown/dist/fractal_flame_wgpu.wasm
if %errorlevel% neq 0 exit /b %errorlevel%
if not exist "pkg\assets\palettes\packs" mkdir "pkg\assets\palettes\packs"
xcopy /Y /Q "assets\palettes\packs\*.json" "pkg\assets\palettes\packs\" >nul 2>&1
echo.
echo Built. Hard-refresh, then load several files.
echo   crash          -^> git bisect bad
echo   ~10 clean loads-^> git bisect good
