# 🎃 Pumpkin Minecraft Server

High-performance, multithreaded Minecraft server written in Rust, featuring comprehensive vanilla parity across Java 1.21.4.

> ## 🎮 Join the Public Server
>
> **Server address:** `pumpkinmc.dnslab.win`<br>
> **Supported versions:** Java Edition **1.7.2–26.2**<br>
> **Recommended version:** Java Edition **1.21.4**<br>
> **Location:** Singapore<br>
> **Server hardware:** 2 vCPU (Arm Neoverse-N1) and 11 GiB RAM
>
> Add the address in **Multiplayer → Add Server**. For the most thoroughly tested
> experience and best compatibility, connect with an unmodified 1.21.4 client.

> ## 💖 Support Server Hosting & Development
>
> The public test server (`pumpkinmc.dnslab.win`) is currently self-funded on a modest ARM64 VPS (2 vCPU, 11 GiB RAM).
>
> If you'd like to support the project or help upgrade the hosting infrastructure, you can buy us a coffee:
>
> [![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-Donate-orange?style=for-the-badge&logo=buy-me-a-coffee)](https://buymeacoffee.com/potatosips)

> ## 🐛 Found a Problem on the Server?
>
> If you find any bug, crash, disconnect, Vanilla behavior mismatch, or other issue
> while playing on the Pumpkin server, please create a **super-detailed GitHub issue**.
> Include what happened, what you expected, your exact Minecraft version, reproduction
> steps, relevant logs or screenshots, and whether the problem happens consistently.
>
> **If you know how to fix the problem, please also submit a pull request** containing
> the solution, focused tests, and a clear explanation of how you validated it.
>
> [Create a detailed issue](https://github.com/potatosips/Pumpkin/issues/new) ·
> [Open a pull request](https://github.com/potatosips/Pumpkin/compare)

---

## ⚡ Quick Navigation
- [🪟 Windows Executable Guide (Build & Run `pumpkin.exe`)](#-windows-executable-guide-build--run)
  - [Prerequisites](#1-prerequisites)
  - [Building `pumpkin.exe`](#2-building-the-executable)
  - [Running & Initial Setup](#3-running--initial-setup)
  - [Configuration (`pumpkin.toml`)](#4-configuration-pumpkintoml)
  - [Packaging a Standalone Release](#5-packaging-a-standalone-release)
- [🐳 Docker & Container Deployment Guide](#-docker--container-deployment-guide)
  - [Method 1: Pull from GitHub Container Registry (Recommended)](#method-1-pull-from-github-container-registry-recommended)
  - [Method 2: Docker Compose Deployment](#method-2-docker-compose-deployment)
  - [Method 3: Build Image from Source](#method-3-build-image-from-source)
  - [Persistent Data & Volume Structure](#persistent-data--volume-structure)
  - [Remote Linux VPS / Cloud VM Deployment](#remote-linux-vps--cloud-vm-deployment)

---

# 🪟 Windows Executable Guide (Build & Run)

Follow these steps to compile, run, and configure the standalone 64-bit Windows binary (`pumpkin.exe`).

### 1. Prerequisites
- **Rust Toolchain**: Install from [rustup.rs](https://rustup.rs/) (default: `stable-x86_64-pc-windows-msvc`).
- **C++ Build Tools**: Visual Studio C++ Build Tools (Desktop development with C++ workload).
- **Git**: Ensure `git` is available in PATH.

### 2. Building the Executable

Open PowerShell or Command Prompt in the repository folder:

#### ⚡ Debug Build (Fast Compilation for Testing)
```powershell
cargo build -p pumpkin
```
*Output location*: `target\debug\pumpkin.exe`

#### 🚀 Release Build (High-Performance Production Binary)
```powershell
cargo build --release -p pumpkin
```
*Output location*: `target\release\pumpkin.exe`

---

### 3. Running & Initial Setup

#### Launching the Server
```powershell
.\target\release\pumpkin.exe
```

On first launch, Pumpkin will automatically:
1. Initialize world dimensions (`minecraft:overworld`, `minecraft:the_nether`, `minecraft:the_end`).
2. Generate default configuration templates (`pumpkin.toml`, `ops.json`).
3. Bind to network address `0.0.0.0:25565`.

---

### 4. Configuration (`pumpkin.toml`)

Key configuration parameters in `pumpkin.toml`:

```toml
[server]
address = "0.0.0.0:25565"    # Server listening address & port
max_players = 100            # Maximum simultaneous players
view_distance = 10           # Chunk simulation & render distance
simulation_distance = 10     # Entity & redstone tick distance

[proxy]
enabled = false              # Set to true for BungeeCord / Velocity
online_mode = true           # Microsoft/Mojang online authentication
```

---

### 5. Packaging a Standalone Release

To bundle `pumpkin.exe` into a clean zip archive for distribution:

```powershell
# Create staging folder and copy files
New-Item -ItemType Directory -Path ".\dist" -Force | Out-Null
Copy-Item ".\target\release\pumpkin.exe" ".\dist\pumpkin.exe"
Copy-Item ".\pumpkin.toml" ".\dist\pumpkin.sample.toml"
Copy-Item ".\README.md" ".\dist\README.md"

# Package zip
Compress-Archive -Path ".\dist\*" -DestinationPath ".\pumpkin-windows-x86_64.zip" -CompressionLevel Optimal
Remove-Item -Recurse -Force ".\dist"
```

---

# 🐳 Docker & Container Deployment Guide

Deploy Pumpkin in isolated, containerized environments using Docker and Docker Compose.

---

### Method 1: Pull from GitHub Container Registry (Recommended)

Run the pre-built multi-arch image directly:

#### Interactive Run
```bash
docker run -it --rm \
  -p 25565:25565 \
  -v $(pwd)/pumpkin_data:/pumpkin \
  ghcr.io/potatosips/pumpkin:latest
```

#### Production Background Daemon
```bash
docker run -d \
  --name pumpkin-server \
  --restart unless-stopped \
  -p 25565:25565 \
  -v $(pwd)/pumpkin_data:/pumpkin \
  ghcr.io/potatosips/pumpkin:latest
```

---

### Method 2: Docker Compose Deployment

#### 1. Create `docker-compose.yml`
```yaml
services:
  pumpkin:
    image: ghcr.io/potatosips/pumpkin:latest
    container_name: pumpkin-server
    restart: unless-stopped
    ports:
      # Minecraft Java Edition Port
      - "25565:25565/tcp"
      # Minecraft Bedrock Edition (if enabled)
      - "19132:19132/udp"
    volumes:
      # Persistent world data and configuration
      - ./data:/pumpkin
    environment:
      - RUST_BACKTRACE=1
    deploy:
      resources:
        limits:
          memory: 4G
```

#### 2. Manage with Compose Commands
```bash
# Start server in background
docker compose up -d

# View live console logs
docker compose logs -f

# Stop server gracefully
docker compose down

# Update to latest version
docker compose pull && docker compose up -d
```

---

### Method 3: Build Image from Source

To build a custom container image locally:

```bash
# Build local image
docker build -t pumpkin-local:latest .

# Run local container
docker run -d \
  --name pumpkin-server \
  -p 25565:25565 \
  -v $(pwd)/data:/pumpkin \
  pumpkin-local:latest
```

---

### Persistent Data & Volume Structure

When mounting a volume (`-v ./data:/pumpkin`), the directory structure on the host machine contains:

```
./data/
├── pumpkin.toml         # Main server settings (ports, limits, motd)
├── ops.json             # Operator and admin permissions
├── world/               # World save data (chunks, player states)
└── logs/                # Server execution logs
```

---

### Remote Linux VPS / Cloud VM Deployment

To deploy on a remote cloud server (e.g., Ubuntu/Debian Oracle VM):

```bash
# 1. SSH into server
ssh root@<YOUR_SERVER_IP>

# 2. Setup server directory
mkdir -p /opt/pumpkin/data && cd /opt/pumpkin

# 3. Create docker-compose.yml
cat << 'EOF' > docker-compose.yml
services:
  pumpkin:
    image: ghcr.io/potatosips/pumpkin:latest
    container_name: pumpkin-server
    restart: always
    ports:
      - "25565:25565/tcp"
    volumes:
      - ./data:/pumpkin
    environment:
      - RUST_BACKTRACE=1
EOF

# 4. Start the container
docker compose up -d

# 5. Open firewall port 25565
iptables -I INPUT -p tcp --dport 25565 -j ACCEPT
```
