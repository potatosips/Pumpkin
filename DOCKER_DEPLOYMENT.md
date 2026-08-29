# Pumpkin Minecraft Server — Docker Deployment Guide

This guide covers deploying the high-performance **Pumpkin** Minecraft server using Docker, Docker Compose, and GitHub Container Registry (`ghcr.io`).

---

## 📋 Table of Contents
1. [Prerequisites](#-prerequisites)
2. [Method 1: Pull from GitHub Container Registry (Recommended)](#-method-1-pull-from-github-container-registry-recommended)
3. [Method 2: Docker Compose Deployment](#-method-2-docker-compose-deployment)
4. [Method 3: Build & Run from Source](#-method-3-build--run-from-source)
5. [Configuration & Data Persistence](#-configuration--data-persistence)
6. [Server Management & Operations](#-server-management--operations)
7. [Deploying to Remote Linux VPS / Oracle VM Host](#-deploying-to-remote-linux-vps--oracle-vm-host)

---

## 🛠️ Prerequisites

Ensure Docker and Docker Compose are installed:
```bash
docker --version
docker compose version
```

---

## 🚀 Method 1: Pull from GitHub Container Registry (Recommended)

You can run the official pre-built image published under `potatosips/Pumpkin`:

### Quick Start (Interactive)
```bash
docker run -it --rm \
  -p 25565:25565 \
  -v $(pwd)/pumpkin_data:/pumpkin \
  ghcr.io/potatosips/pumpkin:latest
```

### Production Daemon Mode (Background)
```bash
docker run -d \
  --name pumpkin-server \
  --restart unless-stopped \
  -p 25565:25565 \
  -v $(pwd)/pumpkin_data:/pumpkin \
  ghcr.io/potatosips/pumpkin:latest
```

---

## 🐳 Method 2: Docker Compose Deployment

Docker Compose simplifies server lifecycle management, volume bindings, and port mapping.

### 1. Create `docker-compose.yml`
```yaml
services:
  pumpkin:
    image: ghcr.io/potatosips/pumpkin:latest
    container_name: pumpkin-server
    restart: unless-stopped
    ports:
      # Minecraft Java Edition default port
      - "25565:25565/tcp"
      # Minecraft Bedrock Edition (if enabled in pumpkin.toml)
      - "19132:19132/udp"
    volumes:
      # Persistent server files (configs, worlds, logs, ops)
      - ./data:/pumpkin
    environment:
      - RUST_BACKTRACE=1
    deploy:
      resources:
        limits:
          memory: 4G
```

### 2. Start the Server
```bash
# Start container in detached mode
docker compose up -d

# View live console logs
docker compose logs -f

# Stop the server
docker compose down
```

---

## 🔨 Method 3: Build & Run from Source

To build a local Docker image directly from the repository source:

```bash
# Clone the repository
git clone https://github.com/potatosips/Pumpkin.git
cd Pumpkin

# Build the Docker image
docker build -t pumpkin-local:latest .

# Run the newly built local image
docker run -d \
  --name pumpkin-server \
  -p 25565:25565 \
  -v $(pwd)/data:/pumpkin \
  pumpkin-local:latest
```

---

## ⚙️ Configuration & Data Persistence

All server configuration and world state reside inside `/pumpkin` in the container. When bind-mounted (`-v ./data:/pumpkin`), files are stored on the host filesystem:

```
./data/
├── pumpkin.toml         # Main server configuration (ports, motd, view distance)
├── ops.json             # Operator and admin permissions
├── world/               # Dimension data (overworld, the_nether, the_end)
└── logs/                # Server console logs
```

### Generating Default `pumpkin.toml`
On the first run, Pumpkin will automatically create default configuration files inside your data directory. You can edit `pumpkin.toml` directly and restart the container:

```bash
docker restart pumpkin-server
```

---

## 📊 Server Management & Operations

### View Server Logs
```bash
docker logs -f --tail 100 pumpkin-server
```

### Restart Server
```bash
docker restart pumpkin-server
```

### Updating to the Latest Release
```bash
# Pull new image
docker pull ghcr.io/potatosips/pumpkin:latest

# Recreate container with existing persistent volume
docker compose down
docker compose up -d
```

### Backup World & Configurations
```bash
tar -czvf pumpkin-backup-$(date +%Y%m%d).tar.gz ./data/
```

---

## 🌐 Deploying to Remote Linux VPS / Oracle VM Host

To deploy on a remote cloud host (e.g. `root@129.225.101.146`):

```bash
# 1. SSH into the VM host
ssh root@129.225.101.146

# 2. Create server directory
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

# 4. Launch the server
docker compose up -d

# 5. Open firewall port 25565 (Ubuntu/Debian UFW or iptables)
iptables -I INPUT -p tcp --dport 25565 -j ACCEPT
```
