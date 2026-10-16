# Build & Release Scripts

This directory centralizes all compilation and release packaging scripts for Raspberry Pi.

All batch scripts can be executed directly from Windows (by double-clicking or from a terminal): they automatically change directory to the repository root (`cd /d "%~dp0\.."`).

**Requirement:** These batch scripts require **WSL (Ubuntu)** with the cross-compilation toolchains installed.

---

## 🚀 AArch64 Target (Modern Raspberry Pi OS 64-bit)
*Uses the standard `cargo build` cross-compiler with `gcc-aarch64-linux-gnu`.*

- **`build-link-aarch64.bat`**: Compiles `greenup-link` in Debug mode to `target/aarch64-unknown-linux-gnu/debug/greenup-link`.
- **`build-everest-aarch64.bat`**: Compiles `greenup-everest` in Debug mode to `target/aarch64-unknown-linux-gnu/debug/greenup-everest`.
- **`release-aarch64.bat`**: Compiles `greenup-link` in Release mode (optimized, stripped) and packages `greenup-link-vX.Y.Z-aarch64.zip`.

---

## 🏛️ Legacy ARMv7 Target (32-bit Legrand Factory OS)
*Uses `cargo-zigbuild` targeting glibc 2.24 for binary compatibility with Legrand's historical Debian 9 Stretch image.*

- **`build-link-legacy.bat`**: Compiles `greenup-link` in Debug mode to `target/armv7-unknown-linux-gnueabihf/debug/greenup-link`.
- **`build-everest-legacy.bat`**: Compiles `greenup-everest` in Debug mode to `target/armv7-unknown-linux-gnueabihf/debug/greenup-everest`.
- **`release-legacy.bat`**: Compiles `greenup-link` in Release mode and packages `greenup-link-vX.Y.Z-armv7-legacy.zip`.

---

For full instructions on configuring the WSL compilation environment, see [`docs/ENV_COMPILATION_AARCH64_ARM7.md`](../docs/ENV_COMPILATION_AARCH64_ARM7.md).
