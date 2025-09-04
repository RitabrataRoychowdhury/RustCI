# Installation Guide

This guide covers all installation methods for RustCI across different platforms and environments.

## System Requirements

### Minimum Requirements
- **CPU**: 1 core (2+ cores recommended)
- **Memory**: 2GB RAM (4GB+ recommended)
- **Storage**: 10GB free disk space
- **Network**: Internet connection for downloads

### Supported Platforms
- **Linux**: Ubuntu 20.04+, CentOS 8+, RHEL 8+, Debian 11+
- **macOS**: macOS 11.0+ (Big Sur and later)
- **Windows**: Windows 10/11, Windows Server 2019+

### Optional Dependencies
- **Docker**: For Docker-based pipeline execution
- **Kubernetes**: For Kubernetes runner support
- **Git**: For source code integration

## Installation Methods

### Method 1: Quick Install Script (Recommended)

The quickest way to get started:

```bash
# Download and install RustCI
curl -sSL https://install.rustci.dev | bash

# Reload your shell or source your profile
source ~/.bashrc  # or ~/.zshrc on macOS

# Verify installation
rustci --version
```

This script will:
- Detect your platform automatically
- Download the appropriate binary
- Install to `/usr/local/bin/rustci`
- Set up basic configuration

### Method 2: Package Managers

#### Ubuntu/Debian (APT)
```bash
# Add RustCI repository
curl -fsSL https://packages.rustci.dev/gpg | sudo gpg --dearmor -o /usr/share/keyrings/rustci.gpg
echo "deb [signed-by=/usr/share/keyrings/rustci.gpg] https://packages.rustci.dev/apt stable main" | sudo tee /etc/apt/sources.list.d/rustci.list

# Update package list and install
sudo apt update
sudo apt install rustci

# Verify installation
rustci --version
```

#### CentOS/RHEL/Fedora (YUM/DNF)
```bash
# Add RustCI repository
sudo tee /etc/yum.repos.d/rustci.repo << EOF
[rustci]
name=RustCI Repository
baseurl=https://packages.rustci.dev/rpm
enabled=1
gpgcheck=1
gpgkey=https://packages.rustci.dev/gpg
EOF

# Install RustCI
sudo yum install rustci  # or sudo dnf install rustci

# Verify installation
rustci --version
```

#### macOS (Homebrew)
```bash
# Add RustCI tap
brew tap rustci/tap

# Install RustCI
brew install rustci

# Verify installation
rustci --version
```

#### Windows (Chocolatey)
```powershell
# Install Chocolatey if not already installed
# See https://chocolatey.org/install

# Install RustCI
choco install rustci

# Verify installation
rustci --version
```

#### Windows (Scoop)
```powershell
# Add RustCI bucket
scoop bucket add rustci https://github.com/rustci/scoop-bucket

# Install RustCI
scoop install rustci

# Verify installation
rustci --version
```

### Method 3: Direct Binary Download

Download pre-compiled binaries directly:

#### Linux (x86_64)
```bash
# Download latest release
curl -LO https://github.com/rustci/rustci/releases/latest/download/rustci-linux-x86_64.tar.gz

# Extract and install
tar -xzf rustci-linux-x86_64.tar.gz
sudo mv rustci /usr/local/bin/
sudo chmod +x /usr/local/bin/rustci

# Verify installation
rustci --version
```

#### macOS (x86_64)
```bash
# Download latest release
curl -LO https://github.com/rustci/rustci/releases/latest/download/rustci-macos-x86_64.tar.gz

# Extract and install
tar -xzf rustci-macos-x86_64.tar.gz
sudo mv rustci /usr/local/bin/
sudo chmod +x /usr/local/bin/rustci

# Verify installation
rustci --version
```

#### macOS (Apple Silicon)
```bash
# Download latest release
curl -LO https://github.com/rustci/rustci/releases/latest/download/rustci-macos-aarch64.tar.gz

# Extract and install
tar -xzf rustci-macos-aarch64.tar.gz
sudo mv rustci /usr/local/bin/
sudo chmod +x /usr/local/bin/rustci

# Verify installation
rustci --version
```

#### Windows
```powershell
# Download from GitHub releases
# Visit: https://github.com/rustci/rustci/releases/latest
# Download: rustci-windows-x86_64.zip

# Extract to a directory in your PATH
# For example: C:\Program Files\RustCI\

# Add to PATH if needed
$env:PATH += ";C:\Program Files\RustCI"

# Verify installation
rustci --version
```

### Method 4: Build from Source

For developers or custom builds:

#### Prerequisites
- **Rust**: 1.70.0 or later
- **Git**: For cloning the repository
- **Build tools**: Platform-specific build tools

#### Build Steps
```bash
# Install Rust if not already installed
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env

# Clone the repository
git clone https://github.com/rustci/rustci.git
cd rustci

# Build in release mode
cargo build --release

# Install the binary
sudo cp target/release/rustci /usr/local/bin/
sudo chmod +x /usr/local/bin/rustci

# Verify installation
rustci --version
```

#### Custom Build Options
```bash
# Build with specific features
cargo build --release --features "kubernetes,observability"

# Build for different target
cargo build --release --target x86_64-unknown-linux-musl

# Build with optimizations
RUSTFLAGS="-C target-cpu=native" cargo build --release
```

## Post-Installation Setup

### 1. Initialize Configuration
```bash
# Create default configuration
rustci init

# This creates ~/.rustci/config.yaml with default settings
```

### 2. Verify Installation
```bash
# Check version
rustci --version

# Check available commands
rustci --help

# Test basic functionality
rustci doctor
```

### 3. Configure Runners (Optional)

#### Docker Runner Setup
```bash
# Install Docker if not already installed
# Ubuntu/Debian:
sudo apt install docker.io
sudo usermod -aG docker $USER

# macOS:
brew install --cask docker

# Windows:
# Install Docker Desktop from docker.com

# Test Docker integration
rustci test-runner docker
```

#### Kubernetes Runner Setup
```bash
# Install kubectl
# Ubuntu/Debian:
sudo apt install kubectl

# macOS:
brew install kubectl

# Windows:
choco install kubernetes-cli

# Configure kubectl with your cluster
kubectl config current-context

# Test Kubernetes integration
rustci test-runner kubernetes
```

## Configuration

### Default Configuration Location
- **Linux/macOS**: `~/.rustci/config.yaml`
- **Windows**: `%USERPROFILE%\.rustci\config.yaml`

### Basic Configuration
```yaml
# ~/.rustci/config.yaml
server:
  host: "0.0.0.0"
  port: 8080
  
runners:
  native:
    enabled: true
    max_concurrent: 4
  
  docker:
    enabled: true
    max_concurrent: 2
    
logging:
  level: "info"
  format: "json"
```

For complete configuration options, see the [Configuration Guide](../configuration.md).

## Troubleshooting Installation

### Common Issues

#### Permission Denied
```bash
# If you get permission denied errors:
sudo chmod +x /usr/local/bin/rustci

# Or install to user directory:
mkdir -p ~/.local/bin
mv rustci ~/.local/bin/
export PATH="$HOME/.local/bin:$PATH"
```

#### Command Not Found
```bash
# Check if rustci is in PATH
which rustci

# Add to PATH if needed (add to ~/.bashrc or ~/.zshrc):
export PATH="/usr/local/bin:$PATH"

# Reload shell
source ~/.bashrc
```

#### Docker Permission Issues
```bash
# Add user to docker group
sudo usermod -aG docker $USER

# Log out and back in, or run:
newgrp docker
```

### Verification Commands
```bash
# Check installation
rustci --version
rustci doctor

# Test basic functionality
echo 'name: test
stages:
  - name: hello
    jobs:
      - name: greet
        runner: native
        steps:
          - run: echo "Hello World"' > test-pipeline.yaml

rustci run test-pipeline.yaml
```

## Updating RustCI

### Package Manager Updates
```bash
# Ubuntu/Debian
sudo apt update && sudo apt upgrade rustci

# CentOS/RHEL/Fedora
sudo yum update rustci  # or sudo dnf update rustci

# macOS
brew upgrade rustci

# Windows
choco upgrade rustci
```

### Manual Updates
```bash
# Download latest version using the same method as installation
# Replace existing binary with new version

# Verify update
rustci --version
```

## Uninstallation

### Package Manager Removal
```bash
# Ubuntu/Debian
sudo apt remove rustci

# CentOS/RHEL/Fedora
sudo yum remove rustci  # or sudo dnf remove rustci

# macOS
brew uninstall rustci

# Windows
choco uninstall rustci
```

### Manual Removal
```bash
# Remove binary
sudo rm /usr/local/bin/rustci

# Remove configuration (optional)
rm -rf ~/.rustci

# Remove from PATH if manually added
# Edit ~/.bashrc or ~/.zshrc and remove RustCI PATH entries
```

## Next Steps

After successful installation:

1. **[Quick Start Guide](README.md)** - Run your first pipeline
2. **[Configuration Guide](../configuration.md)** - Customize RustCI settings
3. **[Pipeline Examples](../pipeline-examples/)** - Learn from working examples
4. **[API Documentation](../../api/README.md)** - Integrate with your tools

---

**Need help?** Check the [Troubleshooting Guide](../troubleshooting.md) or visit our [GitHub Discussions](https://github.com/rustci/rustci/discussions).