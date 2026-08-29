# Pumpkin Minecraft Server — Windows Executable Build Guide

This guide details how to compile, optimize, package, and run the standalone **`pumpkin.exe`** binary on Windows.

---

## 📋 Table of Contents
1. [Prerequisites](#-prerequisites)
2. [Cloning the Source Code](#-cloning-the-source-code)
3. [Building the Executable](#-building-the-executable)
   - [Debug Build (Fast Compilation)](#1-debug-build-fast-compilation)
   - [Release Build (Optimized Binary)](#2-release-build-optimized-production-binary)
4. [Output Binary Location](#-output-binary-location)
5. [Running & Configuring the Server](#-running--configuring-the-server)
6. [Packaging a Distributable Zip Release](#-packaging-a-distributable-zip-release)
7. [Troubleshooting & Build Optimization](#-troubleshooting--build-optimization)

---

## 🛠️ Prerequisites

Before compiling Pumpkin on Windows, ensure the following tools are installed:

### 1. Rust Toolchain (`rustup`)
Install Rust via [rustup.rs](https://rustup.rs/) (default toolchain: `stable-x86_64-pc-windows-msvc`):
```powershell
rustup --version
cargo --version
rustc --version
```

### 2. Visual Studio C++ Build Tools
Make sure the **Desktop development with C++** workload is installed via the Visual Studio Installer (provides MSVC `link.exe` and Windows SDK libraries).

### 3. Git
```powershell
git --version
```

---

## 📥 Cloning the Source Code

Clone the repository with submodules:
```powershell
git clone --recursive https://github.com/potatosips/Pumpkin.git
cd Pumpkin
```

---

## ⚙️ Building the Executable

Pumpkin is a multi-crate workspace. The main server binary resides in the `pumpkin` crate (`crates/pumpkin`).

### 1. Debug Build (Fast Compilation)
Use for quick iterative testing during development:
```powershell
cargo build -p pumpkin
```
*Output path*: `target\debug\pumpkin.exe`

### 2. Release Build (Optimized Production Binary)
Use for maximum performance, multi-threaded throughput, and minimal packet latency:
```powershell
cargo build --release -p pumpkin
```
*Output path*: `target\release\pumpkin.exe`

> **Note**: Building the release binary takes ~4 to 8 minutes on modern multi-core CPUs as it compiles link-time optimizations (LTO) and codegen units.

---

## 📍 Output Binary Location

Once compilation finishes, your optimized 64-bit executable is located at:
```
<RepositoryRoot>\target\release\pumpkin.exe
```

Verify binary details in PowerShell:
```powershell
Get-Item .\target\release\pumpkin.exe | Select-Object Name, Length, LastWriteTime
```

---

## 🎮 Running & Configuring the Server

### 1. Launching the Server
Open a terminal in the folder containing `pumpkin.exe` (or run it directly):
```powershell
.\target\release\pumpkin.exe
```

### 2. Initial Setup
On first startup, Pumpkin will:
- Initialize the world directories (`minecraft:overworld`, `minecraft:the_nether`, `minecraft:the_end`).
- Generate default configurations (`pumpkin.toml`, `ops.json`).
- Bind to `0.0.0.0:25565` (Java Edition default).

### 3. Configuring `pumpkin.toml`
Edit `pumpkin.toml` to customize:
- `server_address`: Default `"0.0.0.0:25565"`.
- `motd`: Server description visible in Minecraft server list.
- `view_distance`: Chunk render distance for connected clients.
- `online_mode`: Authentication mode (`true` for Mojang/Microsoft auth, `false` for offline testing).

---

## 📦 Packaging a Distributable Zip Release

To bundle `pumpkin.exe` into a clean, standalone zip file for distribution or GitHub Releases:

```powershell
# 1. Create a clean staging directory
New-Item -ItemType Directory -Path ".\dist" -Force | Out-Null

# 2. Copy binary and default files
Copy-Item ".\target\release\pumpkin.exe" ".\dist\pumpkin.exe"
Copy-Item ".\pumpkin.toml" ".\dist\pumpkin.sample.toml"
Copy-Item ".\README.md" ".\dist\README.md"
Copy-Item ".\LICENSE" ".\dist\LICENSE"

# 3. Create zip archive
Compress-Archive -Path ".\dist\*" -DestinationPath ".\pumpkin-windows-x86_64.zip" -CompressionLevel Optimal

# 4. Clean up staging folder
Remove-Item -Recurse -Force ".\dist"
```

---

## 🔧 Troubleshooting & Build Optimization

| Issue | Cause | Solution |
| :--- | :--- | :--- |
| `link.exe not found` | Missing MSVC C++ Build Tools | Install Visual Studio C++ Build Tools via Visual Studio Installer. |
| `Address 0.0.0.0:25565 is already in use` | Another server instance is running | Kill the existing process: `Stop-Process -Name pumpkin -Force` or check `Get-NetTCPConnection -LocalPort 25565`. |
| Out of Memory during LTO | Heavy Link-Time Optimization | Set `codegen-units = 16` or add `--config profile.release.lto=false` in `Cargo.toml`. |
