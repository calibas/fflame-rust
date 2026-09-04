@echo off
REM Separate "module size / load timing" from "the stripped link".
REM
REM Run AFTER build-wasm.bat --symbols has populated .\pkg.
REM
REM Keeps the named module aside, then deletes the name and DWARF
REM sections from the served one by BYTE EDIT -- no relinking, so the
REM code section is bit-identical and the offset shift is zero. The
REM served module drops to roughly the shipped size while remaining
REM the same program.
REM
REM   crashes      -> it was size/timing. Send the offset; it maps
REM                   1:1 onto pkg\fractal_flame_wgpu_bg.names.wasm
REM   does not     -> the crash needs the stripped LINK itself, and no
REM                   symbol-preserving build will ever observe it
if not exist "pkg\fractal_flame_wgpu_bg.wasm" (
    echo Run build-wasm.bat --symbols first.
    exit /b 1
)
copy /Y "pkg\fractal_flame_wgpu_bg.wasm" "pkg\fractal_flame_wgpu_bg.names.wasm" >nul
python scripts\wasm-strip-names.py "pkg\fractal_flame_wgpu_bg.names.wasm" "pkg\fractal_flame_wgpu_bg.wasm"
if %errorlevel% neq 0 exit /b %errorlevel%
echo.
echo Served module is now name-free and ~shipped size.
echo Names kept in pkg\fractal_flame_wgpu_bg.names.wasm for symbolizing.
