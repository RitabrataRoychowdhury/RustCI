# RustCI VPS Deployment Quick Start

This guide helps you quickly deploy RustCI and Valkyrie to your VPS using the new deployment system.

## 🚀 Quick Deployment Commands

### Production Deployment (Blue-Green)
```bash
# Deploy to production with blue-green strategy
./deployments/scripts/deploy.sh production blue-green

# Or use the main pipeline
rustci pipeline run --file pipeline.yaml
```

### Staging Deployment (Rolling)
```bash
# Deploy to staging with rolling strategy
./deployments/scripts/deploy.sh staging rolling

# Or use staging-specific config
rustci pipeline run --file deployments/vps/staging.yaml
```

### Development Deployment (Recreate)
```bash
# Deploy to development with recreate strategy
./deployments/scripts/deploy.sh development recreate

# Or use development-specific config
rustci pipeline run --file deployments/vps/development.yaml
```

## 🏥 Health Checks

### Check All Environments
```bash
./deployments/scripts/health-check.sh all
```

### Check Specific Environment
```bash
./deployments/scripts/health-check.sh production
./deployments/scripts/health-check.sh staging
./deployments/scripts/health-check.sh development
```

### Check Specific Service
```bash
./deployments/scripts/health-check.sh production rustci
./deployments/scripts/health-check.sh production valkyrie
```

## 🔄 Rollback Commands

### Rollback to Previous Version
```bash
./deployments/scripts/rollback.sh production
./deployments/scripts/rollback.sh staging
./deployments/scripts/rollback.sh development
```

### List Available Backups
```bash
./deployments/scripts/rollback.sh production list
```

## 🌐 Access URLs

After successful deployment, your applications will be available at:

### Production
- **RustCI**: http://46.37.122.118:8080
- **Valkyrie**: http://46.37.122.118:9090 (if separate service)

### Staging
- **RustCI**: http://46.37.122.118:8082
- **Valkyrie**: http://46.37.122.118:9092 (if separate service)

### Development
- **RustCI**: http://46.37.122.118:8083
- **Valkyrie**: http://46.37.122.118:9093 (if separate service)

## 📋 Prerequisites

Make sure you have these tools installed:
```bash
# Required tools
sudo apt-get update
sudo apt-get install -y curl sshpass jq

# Docker (if not already installed)
curl -fsSL https://get.docker.com -o get-docker.sh
sudo sh get-docker.sh
```

## 🔧 Configuration Files

### Environment Variables
- `deployments/environments/production.env` - Production settings
- `deployments/environments/staging.env` - Staging settings  
- `deployments/environments/development.env` - Development settings

### Deployment Strategies
- `deployments/strategies/blue-green.yaml` - Zero-downtime deployment
- `deployments/strategies/rolling.yaml` - Gradual replacement
- `deployments/strategies/canary.yaml` - Traffic shifting
- `deployments/strategies/recreate.yaml` - Simple restart

### VPS Configurations
- `deployments/vps/production.yaml` - Production VPS deployment
- `deployments/vps/staging.yaml` - Staging VPS deployment
- `deployments/vps/development.yaml` - Development VPS deployment

## 🚨 Troubleshooting

### Connection Issues
```bash
# Test VPS connectivity
ssh root@46.37.122.118

# Check if Docker is running on VPS
ssh root@46.37.122.118 "docker info"
```

### Deployment Issues
```bash
# Check deployment logs
./deployments/scripts/health-check.sh production

# Check container status on VPS
ssh root@46.37.122.118 "docker ps -a"

# Check container logs
ssh root@46.37.122.118 "docker logs rustci-blue"
```

### Port Issues
```bash
# Check if ports are in use
ssh root@46.37.122.118 "netstat -tlnp | grep -E '(8080|8082|8083|9090|9092|9093)'"

# Check firewall settings
ssh root@46.37.122.118 "ufw status"
```

## 📝 Deployment Flow

1. **Pre-deployment**: Validate VPS connection and install prerequisites
2. **Build**: Clone repository and build Docker images
3. **Deploy**: Use selected strategy (blue-green, rolling, canary, recreate)
4. **Health Check**: Verify deployment is healthy
5. **Traffic Switch**: Route traffic to new deployment
6. **Post-deployment**: Cleanup and verification

## 🔐 Security Notes

- SSH credentials are stored in environment files
- Consider using SSH keys instead of passwords for production
- Environment files contain sensitive information - keep them secure
- MongoDB connection string includes credentials

## 📞 Support

If you encounter issues:
1. Check the health status: `./deployments/scripts/health-check.sh all`
2. Review deployment logs
3. Test VPS connectivity
4. Check container status on VPS
5. Verify environment configuration files