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
MSVC builds now use the default **dynamic** CRT, which may require the Visual C++ Redistributable on target machines.
If you want to avoid that dependency, build with the GNU toolchain: `--target x86_64-pc-windows-gnu`.

## Run
- `cargo run -p mclauncher`
- `build-release.bat` (release EXE)
