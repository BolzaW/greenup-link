# Compilation Environment Guide: AArch64 (64-bit) & ARMv7 (32-bit Legacy)

This guide provides complete instructions for setting up a cross-compilation environment using **WSL 2 (Ubuntu)** on Windows (or native Linux) to compile **Green'Up Link** for both supported target architectures:

1. **Modern 64-bit ARM (`aarch64-unknown-linux-gnu`)**: For Raspberry Pi 3, 4, 5, or Zero 2 W running modern 64-bit Raspberry Pi OS (Debian 11 Bullseye / Debian 12 Bookworm).
2. **Legacy 32-bit ARM (`armv7-unknown-linux-gnueabihf`)**: For the official Legrand factory image running on Raspberry Pi 3 (Debian 9 Stretch with historical **glibc 2.24**).

---

## 📋 Target Comparison Overview

| Target Architecture | Rust Triple | Toolchain / Linker | Target OS | Minimum glibc |
| :--- | :--- | :--- | :--- | :--- |
| **Modern 64-bit** | `aarch64-unknown-linux-gnu` | Standard `cargo build` + `gcc-aarch64-linux-gnu` | Raspberry Pi OS 64-bit (Lite / Desktop) | glibc 2.31+ |
| **Legacy 32-bit** | `armv7-unknown-linux-gnueabihf` | `cargo-zigbuild` (`.2.24`) | Official Legrand KitCom image (Debian 9 Stretch) | **glibc 2.24** |

---

## 1. Common Prerequisites (Host Environment)

The cross-compilation toolchains run inside **WSL 2 (Ubuntu)** on Windows 10/11 or directly on any modern Ubuntu/Debian x86_64 host.

### Step 1: Install WSL 2 (Windows only)
From an elevated PowerShell terminal on Windows:
```powershell
wsl --install -d Ubuntu
```
Once installed, open the Ubuntu terminal (`wsl -d Ubuntu`).

### Step 2: Install Essential Host Packages
Inside your Ubuntu terminal:
```bash
sudo apt-get update
sudo apt-get install -y build-essential curl pkg-config git
```

### Step 3: Install Rust & Cargo
If Rust is not already installed in your WSL environment:
```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
source "$HOME/.cargo/env"
```
Verify that Rust and Cargo are functional:
```bash
rustc --version
cargo --version
```

---

## 2. Environment 1: Modern 64-bit ARM (`aarch64-unknown-linux-gnu`)

### Why standard `cargo build` is used
Modern 64-bit Raspberry Pi OS images (Bullseye / Bookworm) ship with recent versions of the GNU C library (`glibc 2.31` or `glibc 2.36+`). A standard GNU cross-linker (`gcc-aarch64-linux-gnu`) available in Ubuntu packages compiles binaries that are natively compatible with these systems without requiring custom runtime stubs.

### Setup Instructions

1. **Install the Rust 64-bit target**:
   ```bash
   rustup target add aarch64-unknown-linux-gnu
   ```

2. **Install the GNU AArch64 cross-linker**:
   ```bash
   sudo apt-get update
   sudo apt-get install -y gcc-aarch64-linux-gnu
   ```

3. **Verify the Cargo linker configuration**:
   The repository includes a [`.cargo/config.toml`](../.cargo/config.toml) file at the root of the workspace:
   ```toml
   [target.aarch64-unknown-linux-gnu]
   linker = "aarch64-linux-gnu-gcc"
   ```
   This configuration tells Cargo to automatically invoke `aarch64-linux-gnu-gcc` when compiling for `aarch64-unknown-linux-gnu`.

### Compilation Commands

- **Debug build** (fast compilation, debug symbols included):
  ```bash
  cargo build -p greenup-link --target aarch64-unknown-linux-gnu
  # Binary output: target/aarch64-unknown-linux-gnu/debug/greenup-link
  ```

- **Release build** (fully optimized and stripped):
  ```bash
  cargo build --release -p greenup-link --target aarch64-unknown-linux-gnu
  # Binary output: target/aarch64-unknown-linux-gnu/release/greenup-link
  ```

---

## 3. Environment 2: Legacy 32-bit ARM (`armv7-unknown-linux-gnueabihf`)

### Why `cargo-zigbuild` is required

The official Legrand factory SD card runs an old **Raspbian GNU/Linux 9 (Stretch)** operating system dating back to 2017, equipped with **glibc 2.24**.

In Linux, compiled binaries dynamically link against the C runtime library (`libc.so.6`). When you compile software on a modern Linux distribution (e.g., Ubuntu 22.04 with glibc 2.35, or Ubuntu 24.04 with glibc 2.39) using a standard GCC cross-compiler (`arm-linux-gnueabihf-gcc`), the resulting binary will reference symbols from the newer glibc. When copied to the Legrand Raspberry Pi 3, running the binary fails immediately with a fatal dynamic linker error:

```text
/lib/arm-linux-gnueabihf/libc.so.6: version 'GLIBC_2.28' not found (required by ./greenup-link)
```

#### The Zig Solution
Setting up an authentic Debian 9 Stretch chroot or virtual machine is tedious and unmaintained. **`cargo-zigbuild`** solves this problem by using the **Zig compiler as the linker**. 

Zig embeds complete header definitions and symbol stubs for **every historical version of glibc** (dating all the way back to glibc 2.2). By passing `.2.24` as the version suffix in the target (`armv7-unknown-linux-gnueabihf.2.24`), `cargo-zigbuild` restricts symbol resolution to glibc 2.24, producing an ELF binary that runs seamlessly on Legrand's Raspbian 9 Stretch image.

### Setup Instructions

1. **Install the Rust 32-bit ARM target**:
   ```bash
   rustup target add armv7-unknown-linux-gnueabihf
   ```

2. **Install Zig**:
   The easiest way to install a recent version of Zig in Ubuntu/WSL is via Python's `pip`:
   ```bash
   sudo apt-get install -y python3-pip
   pip3 install ziglang
   # On newer Ubuntu releases (24.04+), use: pip3 install --break-system-packages ziglang
   ```
   *Alternative manual installation:* Download the latest static Zig tarball from [ziglang.org/download](https://ziglang.org/download/), extract it, and add the directory containing the `zig` binary to your `$PATH`.

   Ensure that `~/.local/bin` is in your `PATH`:
   ```bash
   export PATH="$PATH:$HOME/.local/bin"
   echo 'export PATH="$PATH:$HOME/.local/bin"' >> ~/.bashrc
   ```

   Verify that Zig is working:
   ```bash
   zig version
   ```

3. **Install `cargo-zigbuild`**:
   Install the `cargo-zigbuild` Cargo plugin:
   ```bash
   cargo install cargo-zigbuild
   ```
   Verify the installation:
   ```bash
   cargo zigbuild --version
   ```

### Compilation Commands

- **Debug build (targeting glibc 2.24)**:
  ```bash
  cargo zigbuild -p greenup-link --target armv7-unknown-linux-gnueabihf.2.24
  # Binary output: target/armv7-unknown-linux-gnueabihf/debug/greenup-link
  ```

- **Release build (optimized, targeting glibc 2.24)**:
  ```bash
  cargo zigbuild --release -p greenup-link --target armv7-unknown-linux-gnueabihf.2.24
  # Binary output: target/armv7-unknown-linux-gnueabihf/release/greenup-link
  ```

---

## 4. Ready-to-Use Windows Batch Scripts (`builds/`)

For convenience, ready-to-use Windows batch scripts are provided in the [`builds/`](../builds/) folder. They can be launched directly from Windows (via double-click or cmd/PowerShell). Each script automatically invokes the WSL Ubuntu environment and outputs the compiled binary:

| Script | Architecture | Build Mode | Output Files |
| :--- | :--- | :--- | :--- |
| **`build-link-aarch64.bat`** | 64-bit ARM | Debug | `target/aarch64-unknown-linux-gnu/debug/greenup-link` |
| **`release-aarch64.bat`** | 64-bit ARM | Release + ZIP | `target/aarch64-unknown-linux-gnu/release/greenup-link`<br>`greenup-link-vX.Y.Z-aarch64.zip` |
| **`build-everest-aarch64.bat`** | 64-bit ARM | Debug | `target/aarch64-unknown-linux-gnu/debug/greenup-everest` |
| **`build-link-legacy.bat`** | 32-bit ARM (glibc 2.24) | Debug | `target/armv7-unknown-linux-gnueabihf/debug/greenup-link` |
| **`release-legacy.bat`** | 32-bit ARM (glibc 2.24) | Release + ZIP | `target/armv7-unknown-linux-gnueabihf/release/greenup-link`<br>`greenup-link-vX.Y.Z-armv7-legacy.zip` |
| **`build-everest-legacy.bat`** | 32-bit ARM (glibc 2.24) | Debug | `target/armv7-unknown-linux-gnueabihf/debug/greenup-everest` |

> [!TIP]
> The release scripts (`release-aarch64.bat` and `release-legacy.bat`) package the binary along with `scripts/install.sh` and `README.md` into ready-to-distribute `.zip` archives.

---

## 5. Deployment & Testing on Raspberry Pi

### Copying the binary to the Raspberry Pi
From Windows PowerShell (or WSL terminal):

```bash
# Transfer the compiled binary via SCP
scp target/armv7-unknown-linux-gnueabihf/release/greenup-link pi@<station_ip>:~/
# Or for a 64-bit Raspberry Pi OS installation:
scp target/aarch64-unknown-linux-gnu/release/greenup-link pi@<station_ip>:~/
```

### Running and Verifying on the Raspberry Pi
Connect via SSH:
```bash
ssh pi@<station_ip>

# Check the ELF architecture and dynamic linker requirements:
file ./greenup-link

# Grant execution rights and test run:
chmod +x ./greenup-link
./greenup-link
```

---

## 6. Troubleshooting

### `version 'GLIBC_2.XX' not found`
- **Cause**: The binary was compiled against a newer glibc than the one installed on the target Raspberry Pi.
- **Solution**: Make sure you use `cargo zigbuild --target armv7-unknown-linux-gnueabihf.2.24` (or `builds/release-legacy.bat`) for the official Legrand Raspbian 9 Stretch image.

### `error: linker 'aarch64-linux-gnu-gcc' not found`
- **Cause**: The GCC AArch64 cross-linker is not installed in the WSL environment.
- **Solution**: Run `sudo apt-get install -y gcc-aarch64-linux-gnu`.

### `zig: command not found` during `cargo zigbuild`
- **Cause**: The directory where `pip` installed the `zig` binary (`~/.local/bin`) is not in your current `$PATH`.
- **Solution**: Run `export PATH="$PATH:$HOME/.local/bin"` and add it to your `~/.bashrc`.
