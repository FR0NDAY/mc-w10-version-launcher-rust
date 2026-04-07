# Rust Port (In Progress)

This repository now includes a Rust workspace with a native Windows GUI.

## What is implemented
- `mclauncher-core`: version list parsing (UWP/GDK), cache handling, SOAP request building, and download helpers.
- `mclauncher` GUI: native Windows UI with tabs and GDK download flow.

## Not yet ported
- Windows package registration/unregistration, launch, and dependency handling
- GDK MSIXVC staging/decryption flow
- WU token acquisition (the UI does not fetch it yet)

## VC++ Redistributable
The release build script now uses a **static** CRT, so the EXE does not require the Visual C++ Redistributable.
If you want the default dynamic CRT instead, remove `-C target-feature=+crt-static` from `build-release.bat`.
Alternatively, you can build with the GNU toolchain: `--target x86_64-pc-windows-gnu`.

## Single-file Release Output
`build-release.bat` now copies the single, self-contained EXE to `dist\mclauncher.exe`.
Resources (manifest) are embedded into the executable; this requires the Windows SDK `rc.exe` at build time.

## Run
- `cargo run -p mclauncher`
- `build-release.bat` (release EXE)
