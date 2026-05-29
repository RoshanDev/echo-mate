@echo off
REM EchoMate Windows Build Script
REM Run this from Windows (CMD or PowerShell) in the echo-mate directory

echo === EchoMate Windows Build ===
echo.

REM Check if Rust is installed
where rustc >nul 2>&1
if %ERRORLEVEL% NEQ 0 (
    echo Rust not found. Install from https://rustup.rs and re-run this script.
    echo Or run: winget install Rustlang.Rustup
    pause
    exit /b 1
)

echo Rust found:
rustc --version
echo.

echo Building EchoMate for Windows...
cd src-tauri
cargo build --release
if %ERRORLEVEL% NEQ 0 (
    echo Build failed!
    pause
    exit /b 1
)

echo.
echo === Build Complete ===
echo Binary: src-tauri\target\release\echo-mate.exe
echo Run it directly, or use: cargo run --release
echo.
echo The global hotkey will work natively on Windows using the Win32 RegisterHotKey API.
pause
