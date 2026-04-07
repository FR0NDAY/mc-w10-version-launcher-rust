# MCLauncher (Rust Port)

This repository now contains a Rust workspace that is a port-in-progress of the original MCLauncher.
The current Rust implementation provides a native Windows GUI and download flow.

## Disclaimer
This tool will **not** help you to pirate the game; it requires that you have a Microsoft account which can be used to download Minecraft from the Store.

## Status
- Native Windows GUI implemented (tabs for Release/Beta/Preview/Imported).
- Download flow implemented for GDK packages.
- Package registration/launch and UWP token acquisition are not yet ported.

## Prerequisites
- Rust toolchain (stable)
- A Microsoft account that owns Minecraft for Windows 10 is still required for UWP downloads.

## Build and Run
- `cargo run -p mclauncher`
- `build-release.bat` (release EXE)

See `RUST_PORT.md` for details and limitations.

## Frequently Asked Questions
**Does this allow running multiple instances of Minecraft: Bedrock at the same time?**

Not yet. The Rust port currently focuses on listing and downloading packages.
