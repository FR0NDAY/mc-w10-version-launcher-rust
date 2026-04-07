@echo off
setlocal

where cargo >nul 2>&1
if errorlevel 1 (
  echo Cargo not found in PATH. Install Rust and try again.
  exit /b 1
)

echo Building release...
rem Build with a static CRT to avoid VC++ Redistributable dependency.
set "RUSTFLAGS=%RUSTFLAGS% -C target-feature=+crt-static"
cargo build -p mclauncher --release
if errorlevel 1 exit /b %errorlevel%

if not exist "%CD%\\dist" (
  mkdir "%CD%\\dist"
)
copy /y "%CD%\\target\\release\\mclauncher.exe" "%CD%\\dist\\mclauncher.exe" >nul

echo.
echo Build complete.
echo Output: %CD%\dist\mclauncher.exe
