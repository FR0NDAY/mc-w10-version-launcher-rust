@echo off
setlocal

where cargo >nul 2>&1
if errorlevel 1 (
  echo Cargo not found in PATH. Install Rust and try again.
  exit /b 1
)

echo Building release...
cargo build -p mclauncher --release
if errorlevel 1 exit /b %errorlevel%

if exist "%CD%\\mclauncher\\mclauncher.manifest" (
  copy /y "%CD%\\mclauncher\\mclauncher.manifest" "%CD%\\target\\release\\mclauncher.exe.manifest" >nul
)

echo.
echo Build complete.
echo Output: %CD%\target\release\mclauncher.exe
