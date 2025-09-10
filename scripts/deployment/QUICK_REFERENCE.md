# Quick Reference: Mac-to-Windows CI/CD Setup

## 🚀 Quick Setup (5 minutes)

### 1. Windows Setup
```cmd
# Run as Administrator
Add-WindowsCapability -Online -Name OpenSSH.Server~~~~0.0.1.0
Start-Service sshd
Set-Service -Name sshd -StartupType 'Automatic'
New-NetFirewallRule -Name sshd -DisplayName 'OpenSSH Server (sshd)' -Enabled True -Direction Inbound -Protocol TCP -Action Allow -LocalPort 22
New-NetFirewallRule -DisplayName "RustCI Deploy Server" -Direction Inbound -Protocol TCP -LocalPort 3000 -Action Allow

# Start deployment server
cd C:\path\to\scripts
windows-deploy-server.bat start
```

### 2. Mac Setup
```bash
# Set environment variables
export WINDOWS_HOST="192.168.1.XXX"  # Your Windows IP
export WINDOWS_USER="your-username"   # Your Windows username

# Test setup
./scripts/deployment/test-cross-platform-setup.sh

# Start DinD runner
./scripts/runners/mac-dind-runner.sh start

# Run test pipeline
rustci run mac-to-windows-pipeline.yaml
```

## 📋 Command Cheat Sheet

### Mac Commands
```bash
# DinD Runner Management
./scripts/runners/mac-dind-runner.sh start    # Start runner
./scripts/runners/mac-dind-runner.sh stop     # Stop runner
./scripts/runners/mac-dind-runner.sh status   # Check status
./scripts/runners/mac-dind-runner.sh logs     # View logs

# Testing
./scripts/deployment/test-cross-platform-setup.sh        # Full test
./scripts/runners/test-runner-container-connection.sh    # Container connectivity test
./scripts/runners/test-rustci-runner-integration.sh      # RustCI integration test
./scripts/deployment/test-cross-platform-setup.sh ssh    # SSH test only
./scripts/deployment/test-cross-platform-setup.sh network # Network test only
```

### Windows Commands
```cmd
# Deployment Server Management
windows-deploy-server.bat start     # Start server
windows-deploy-server.bat stop      # Stop server
windows-deploy-server.bat status    # Check status
windows-deploy-server.bat test      # Test connectivity

# Manual Deployment
windows-deploy-server.bat deploy app-20241209.tar.gz
```

## 🔧 Troubleshooting Quick Fixes

### SSH Issues
```bash
# Test SSH manually
ssh your-username@your-windows-ip "echo 'test'"

# Generate SSH key if needed
ssh-keygen -t rsa -b 4096
ssh-copy-id your-username@your-windows-ip
```

### Network Issues
```bash
# Find your Windows IP
# On Windows: ipconfig | findstr "IPv4"
# On Mac: ping your-windows-hostname.local

# Test ports
telnet your-windows-ip 22    # SSH
telnet your-windows-ip 3000  # Deploy server
```

### Docker Issues
```bash
# Restart Docker Desktop
# Check Docker daemon
docker info

# Test DinD
docker run --rm --privileged docker:dind docker --version
```

## 🌐 API Endpoints

### Windows Deployment Server
- Health: `http://windows-ip:3000/health`
- Status: `http://windows-ip:3000/status`
- Deploy: `POST http://windows-ip:3000/deploy`

### Test Commands
```bash
# Test from Mac
curl http://your-windows-ip:3000/health
curl http://your-windows-ip:3000/status
```

## 📁 File Locations

### Mac
- Runner script: `scripts/runners/mac-dind-runner.sh`
- Test script: `scripts/deployment/test-cross-platform-setup.sh`
- Pipeline: `mac-to-windows-pipeline.yaml` (auto-created)
- Docker logs: `docker logs mac-dind-runner`

### Windows
- Deploy server: `scripts/deployment/windows-deploy-server.bat`
- Deploy directory: `C:\temp\rustci-deployments\`
- Artifacts: `C:\temp\rustci-deployments\artifacts\`
- Logs: `C:\temp\rustci-deployments\deploy-server.log`

## ⚡ Environment Variables

```bash
# Required
export WINDOWS_HOST="192.168.1.100"
export WINDOWS_USER="myuser"

# Optional
export RUSTCI_SERVER="http://localhost:8080"
```

## 🔍 Status Checks

### Quick Health Check
```bash
# All-in-one status check
echo "=== Mac Docker ==="
docker ps | grep mac-dind-runner

echo "=== Windows Server ==="
curl -s http://$WINDOWS_HOST:3000/health

echo "=== SSH Test ==="
ssh $WINDOWS_USER@$WINDOWS_HOST "echo 'SSH OK'"
```

### Log Monitoring
```bash
# Mac runner logs
docker logs -f mac-dind-runner

# Windows server logs (via SSH)
ssh $WINDOWS_USER@$WINDOWS_HOST "type C:\temp\rustci-deployments\deploy-server.log"
```

## 🎯 Common Workflows

### Development Cycle
1. Make code changes
2. Commit to repository
3. Run pipeline: `rustci run mac-to-windows-pipeline.yaml`
4. Check deployment: `curl http://$WINDOWS_HOST:3000/status`

### Debugging Failed Deployments
1. Check runner logs: `./scripts/runners/mac-dind-runner.sh logs`
2. Test connectivity: `./scripts/deployment/test-cross-platform-setup.sh network`
3. Check Windows server: `windows-deploy-server.bat status`
4. Manual file transfer test: `scp test.txt $WINDOWS_USER@$WINDOWS_HOST:C:/temp/`

## 🔒 Security Notes

- Use SSH key authentication (more secure than passwords)
- Consider VPN for production environments
- Regularly update SSH server and Docker
- Limit SSH access to specific IPs if possible
- Use Windows Defender or antivirus with exclusions for deployment directory

## 📞 Getting Help

1. Run comprehensive test: `./scripts/deployment/test-cross-platform-setup.sh`
2. Check setup guide: `scripts/deployment/CROSS_PLATFORM_SETUP.md`
3. Review logs for specific error messages
4. Test individual components (SSH, Docker, network)

---

**Pro Tip**: Save your environment variables in `~/.bashrc` or `~/.zshrc` for persistence:
```bash
echo 'export WINDOWS_HOST="192.168.1.100"' >> ~/.bashrc
echo 'export WINDOWS_USER="myuser"' >> ~/.bashrc
```