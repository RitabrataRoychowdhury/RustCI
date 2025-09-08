# Cross-Platform CI/CD Setup Guide

This guide helps you set up a CI/CD flow between your Mac (Docker-in-Docker runner) and Windows laptop (deployment server).

## Overview

- **Mac**: Runs a Docker-in-Docker (DinD) runner that builds applications
- **Windows**: Acts as a deployment server that receives and serves built artifacts
- **Communication**: SSH for secure file transfer and command execution

## Prerequisites

### Mac Prerequisites
- Docker Desktop installed and running
- SSH client (built-in on macOS)
- Network connectivity to Windows machine

### Windows Prerequisites
- OpenSSH Server installed and running
- PowerShell 5.1+ (built-in on Windows 10/11)
- Network connectivity to Mac machine
- Administrator privileges (recommended)

## Setup Instructions

### Step 1: Configure Windows SSH Server

1. **Install OpenSSH Server** (Windows 10/11):
   ```powershell
   # Run as Administrator
   Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0
   ```

2. **Start SSH Service**:
   ```powershell
   Start-Service sshd
   Set-Service -Name sshd -StartupType 'Automatic'
   ```

3. **Configure Windows Firewall**:
   ```powershell
   New-NetFirewallRule -Name sshd -DisplayName 'OpenSSH Server (sshd)' -Enabled True -Direction Inbound -Protocol TCP -Action Allow -LocalPort 22
   ```

4. **Test SSH Access** from Mac:
   ```bash
   ssh your-windows-username@your-windows-ip
   ```

### Step 2: Set Up SSH Key Authentication (Recommended)

1. **Generate SSH key on Mac** (if you don't have one):
   ```bash
   ssh-keygen -t rsa -b 4096 -C "your-email@example.com"
   ```

2. **Copy public key to Windows**:
   ```bash
   ssh-copy-id your-windows-username@your-windows-ip
   ```

   Or manually copy `~/.ssh/id_rsa.pub` content to `C:\Users\your-username\.ssh\authorized_keys` on Windows.

### Step 3: Start Windows Deployment Server

1. **Copy the batch file** to your Windows machine:
   ```
   scripts/deployment/windows-deploy-server.bat
   ```

2. **Run the deployment server**:
   ```cmd
   # Open Command Prompt as Administrator
   cd C:\path\to\scripts
   windows-deploy-server.bat start
   ```

3. **Verify server is running**:
   ```cmd
   windows-deploy-server.bat status
   ```

   Or test from Mac:
   ```bash
   curl http://your-windows-ip:3000/health
   ```

### Step 4: Configure and Start Mac DinD Runner

1. **Set environment variables**:
   ```bash
   export WINDOWS_HOST="your-windows-ip"
   export WINDOWS_USER="your-windows-username"
   export RUSTCI_SERVER="http://localhost:8080"  # Adjust if different
   ```

2. **Start the DinD runner**:
   ```bash
   cd /path/to/RustCI
   ./scripts/runners/mac-dind-runner.sh start
   ```

3. **Verify runner is running**:
   ```bash
   ./scripts/runners/mac-dind-runner.sh status
   ```

### Step 5: Test the CI/CD Pipeline

1. **Run the test pipeline**:
   ```bash
   # The script creates a test pipeline automatically
   rustci run mac-to-windows-pipeline.yaml
   ```

2. **Monitor the deployment**:
   - Mac: `docker logs -f mac-dind-runner`
   - Windows: Check `C:\temp\rustci-deployments\deploy-server.log`

## Network Configuration

### Find Your IP Addresses

**Mac**:
```bash
ifconfig | grep "inet " | grep -v 127.0.0.1
```

**Windows**:
```cmd
ipconfig | findstr "IPv4"
```

### Firewall Configuration

**Mac**: Usually no additional configuration needed for outbound connections.

**Windows**: Ensure these ports are open:
- Port 22 (SSH)
- Port 3000 (Deployment server)

```powershell
# Allow deployment server port
New-NetFirewallRule -DisplayName "RustCI Deploy Server" -Direction Inbound -Protocol TCP -LocalPort 3000 -Action Allow
```

## Usage Commands

### Mac DinD Runner Commands

```bash
# Start runner
./scripts/runners/mac-dind-runner.sh start

# Stop runner
./scripts/runners/mac-dind-runner.sh stop

# Check status
./scripts/runners/mac-dind-runner.sh status

# View logs
./scripts/runners/mac-dind-runner.sh logs

# Help
./scripts/runners/mac-dind-runner.sh help
```

### Windows Deployment Server Commands

```cmd
# Start server
windows-deploy-server.bat start

# Stop server
windows-deploy-server.bat stop

# Check status
windows-deploy-server.bat status

# Deploy specific artifact
windows-deploy-server.bat deploy app-20241209.tar.gz

# Test connectivity
windows-deploy-server.bat test

# Help
windows-deploy-server.bat help
```

## API Endpoints

### Windows Deployment Server

- **Health Check**: `GET http://windows-ip:3000/health`
- **Server Status**: `GET http://windows-ip:3000/status`
- **Deploy Endpoint**: `POST http://windows-ip:3000/deploy`

Example health check response:
```json
{
  "status": "healthy",
  "server": "windows-deploy-server",
  "timestamp": "2024-12-09T10:30:00.000Z"
}
```

## Troubleshooting

### Common Issues

1. **SSH Connection Refused**:
   - Ensure OpenSSH Server is running on Windows
   - Check Windows Firewall settings
   - Verify correct IP address and username

2. **Docker Permission Denied**:
   - Ensure Docker Desktop is running on Mac
   - Check Docker daemon permissions

3. **Deployment Server Not Starting**:
   - Run Command Prompt as Administrator
   - Check if port 3000 is already in use
   - Verify PowerShell execution policy

4. **Pipeline Fails to Deploy**:
   - Test SSH connectivity manually
   - Check network connectivity between machines
   - Verify file permissions on Windows

### Debug Commands

**Test SSH from Mac**:
```bash
ssh -v your-windows-username@your-windows-ip "echo 'SSH test successful'"
```

**Test Network Connectivity**:
```bash
# From Mac
ping your-windows-ip
telnet your-windows-ip 22
telnet your-windows-ip 3000
```

**Check Windows Services**:
```powershell
Get-Service sshd
Get-Process -Name powershell | Where-Object {$_.CommandLine -like "*server.ps1*"}
```

### Log Locations

- **Mac DinD Runner**: `docker logs mac-dind-runner`
- **Windows Deploy Server**: `C:\temp\rustci-deployments\deploy-server.log`
- **Windows SSH**: `C:\ProgramData\ssh\logs\sshd.log`

## Security Considerations

1. **Use SSH Key Authentication**: More secure than password authentication
2. **Firewall Rules**: Only open necessary ports
3. **Network Isolation**: Consider using VPN for production environments
4. **Regular Updates**: Keep SSH server and Docker updated
5. **Access Control**: Limit SSH access to specific users/IPs

## Advanced Configuration

### Custom Deployment Scripts

Create `deploy.bat` in your artifact to run custom deployment logic:

```bat
@echo off
echo Running custom deployment...
# Your deployment commands here
echo Deployment completed
```

### Environment-Specific Configuration

Set different configurations for different environments:

```bash
# Development
export WINDOWS_HOST="192.168.1.100"
export RUSTCI_SERVER="http://localhost:8080"

# Staging
export WINDOWS_HOST="192.168.1.200"
export RUSTCI_SERVER="http://staging.rustci.local:8080"
```

### Multiple Windows Targets

Configure multiple Windows deployment targets:

```bash
# In your pipeline YAML
deploy_targets:
  - host: "192.168.1.100"
    user: "dev-user"
    environment: "development"
  - host: "192.168.1.200"
    user: "staging-user"
    environment: "staging"
```

## Support

For issues or questions:
1. Check the troubleshooting section above
2. Review log files for error messages
3. Test individual components (SSH, Docker, network connectivity)
4. Refer to the RustCI documentation for pipeline configuration

## Example Pipeline

The setup creates a test pipeline (`mac-to-windows-pipeline.yaml`) that demonstrates:
- Building an application on Mac DinD runner
- Creating deployment artifacts
- Testing connectivity to Windows
- Transferring artifacts via SSH
- Triggering deployment on Windows
- Verifying deployment success

This pipeline serves as a template for your own CI/CD workflows.