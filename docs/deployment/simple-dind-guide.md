# Simple DIND to Fake Server Deployment Guide

This guide shows you how to deploy a Node.js Hello World application using DIND runner to a fake server, working with your existing RustCI server (started with `cargo run`).

## Quick Start (3 Steps)

### 1. Start RustCI Server
```bash
# Terminal 1: Start RustCI server
cargo run
```

Keep this terminal open. The server will run on `http://localhost:8080`.

### 2. Run Simple Deployment
```bash
# Terminal 2: Run the simple deployment
./scripts/runners/simple-dind-fake-deployment.sh deploy
```

This single command will:
- ✅ Set up fake EC2-like servers
- ✅ Create a DIND runner container
- ✅ Build Node.js Hello World app in DIND
- ✅ Deploy to fake server
- ✅ Verify deployment works

### 3. Test the Deployment
```bash
# SSH into fake server and test the app
ssh root@localhost -p 2201
# Password: rustci123

# Test the application
curl http://localhost:3000/
curl http://localhost:3000/health
```

## What You'll See

The deployed Node.js app responds with:
```json
{
  "message": "Hello World from Simple DIND Runner!",
  "timestamp": "2024-01-15T10:30:00Z",
  "environment": "production",
  "hostname": "container-id"
}
```

## Alternative: Manual Step-by-Step

If you want to see each step in detail:

```bash
# Run manual step-by-step execution
./scripts/runners/simple-dind-fake-deployment.sh manual
```

This will show you:
1. **Building** the Node.js app in DIND
2. **Creating** Docker image
3. **Testing** locally
4. **Packaging** for deployment
5. **Transferring** to fake server
6. **Deploying** on fake server

## Troubleshooting

### Issue: "RustCI server is not running"
**Solution:**
```bash
# Make sure RustCI server is running
cargo run
```

### Issue: "Docker daemon is not running"
**Solution:**
```bash
# Start Docker Desktop or Docker Engine
docker info  # Should show Docker info
```

### Issue: "Application not responding"
**Solution:**
```bash
# Check deployment status
./scripts/runners/simple-dind-fake-deployment.sh status

# Check container logs on fake server
ssh root@localhost -p 2201
docker logs nodejs-hello-world
```

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                    Host Machine                            │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────────┐    ┌─────────────────────────────────┐ │
│  │   RustCI Server │    │     Simple DIND Runner          │ │
│  │   (cargo run)   │◄──►│  ┌─────────────────────────────┐ │ │
│  │   Port 8080     │    │  │      Docker Daemon         │ │ │
│  │                 │    │  │   (Builds Node.js App)     │ │ │
│  └─────────────────┘    │  └─────────────────────────────┘ │ │
│           │              └─────────────────────────────────┘ │
│           │                           │                     │
│           │                           ▼                     │
│           │              ┌─────────────────────────────────┐ │
│           └─────────────►│        Fake Server              │ │
│                          │  ┌─────────────────────────────┐ │ │
│                          │  │    Node.js Application     │ │ │
│                          │  │      (Port 3000)           │ │ │
│                          │  └─────────────────────────────┘ │ │
│                          └─────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────┘
```

## Commands Reference

```bash
# Full deployment
./scripts/runners/simple-dind-fake-deployment.sh deploy

# Manual step-by-step
./scripts/runners/simple-dind-fake-deployment.sh manual

# Check status
./scripts/runners/simple-dind-fake-deployment.sh status

# Verify deployment
./scripts/runners/simple-dind-fake-deployment.sh verify

# Clean up everything
./scripts/runners/simple-dind-fake-deployment.sh cleanup

# Show help
./scripts/runners/simple-dind-fake-deployment.sh help
```

## Access Information

- **RustCI Server**: http://localhost:8080
- **Fake Server SSH**: `ssh root@localhost -p 2201` (password: `rustci123`)
- **Application URL**: http://localhost:3000 (from within fake server)
- **Health Check**: http://localhost:3000/health (from within fake server)

## Why This Works Better

This simple approach:
- ✅ **Works with existing server**: No need to build release binaries
- ✅ **Faster setup**: Uses your running `cargo run` server
- ✅ **Easier debugging**: Direct access to server logs
- ✅ **Less complex**: Fewer moving parts
- ✅ **Same result**: Demonstrates DIND to fake server deployment

Perfect for development and testing!