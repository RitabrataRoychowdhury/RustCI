#!/bin/bash

# RustCI Environment Setup Script
# Sets up development environment and dependencies

set -e

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[SUCCESS]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[WARNING]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

print_status "Setting up RustCI development environment..."

# Check if Rust is installed
if ! command -v cargo &> /dev/null; then
    print_error "Rust is not installed. Please install Rust first:"
    print_status "curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
    exit 1
fi

print_success "Rust is installed"

# Check Rust version
RUST_VERSION=$(rustc --version | cut -d' ' -f2)
print_status "Rust version: $RUST_VERSION"

# Install required Rust tools
print_status "Installing required Rust tools..."

# Install cargo-watch for hot reload
if ! command -v cargo-watch &> /dev/null; then
    print_status "Installing cargo-watch..."
    cargo install cargo-watch
fi

# Install cargo-tarpaulin for code coverage
if ! command -v cargo-tarpaulin &> /dev/null; then
    print_status "Installing cargo-tarpaulin..."
    cargo install cargo-tarpaulin
fi

# Install cargo-audit for security auditing
if ! command -v cargo-audit &> /dev/null; then
    print_status "Installing cargo-audit..."
    cargo install cargo-audit
fi

print_success "Rust tools installed"

# Check if Docker is installed
if ! command -v docker &> /dev/null; then
    print_warning "Docker is not installed. Some features may not work."
    print_status "Please install Docker from: https://docs.docker.com/get-docker/"
else
    print_success "Docker is installed"
    
    # Check if Docker is running
    if ! docker info &> /dev/null; then
        print_warning "Docker is not running. Please start Docker."
    else
        print_success "Docker is running"
    fi
fi

# Check if Docker Compose is installed
if ! command -v docker-compose &> /dev/null; then
    print_warning "Docker Compose is not installed. Multi-node deployment may not work."
    print_status "Please install Docker Compose from: https://docs.docker.com/compose/install/"
else
    print_success "Docker Compose is installed"
fi

# Create .env file if it doesn't exist
if [ ! -f .env ]; then
    print_status "Creating .env file..."
    cat > .env << 'EOF'
# RustCI Environment Configuration

# Environment
RUST_ENV=development
LOG_LEVEL=debug

# Server Configuration
SERVER_HOST=0.0.0.0
SERVER_PORT=8080

# Database Configuration
MONGODB_URI=mongodb://localhost:27017
MONGODB_DATABASE=rustci

# Security Configuration
JWT_SECRET=your-super-secret-jwt-key-change-in-production

# GitHub OAuth Configuration (required for authentication)
GITHUB_OAUTH_CLIENT_ID=your_github_client_id
GITHUB_OAUTH_CLIENT_SECRET=your_github_client_secret
GITHUB_OAUTH_REDIRECT_URL=http://localhost:8080/api/sessions/oauth/github/callback

# Cluster Configuration
NODE_ROLE=master
NODE_ID=dev-node-1
MAX_CONCURRENT_JOBS=4

# Feature Flags
ENABLE_METRICS=true
ENABLE_TRACING=true
ENABLE_HOT_RELOAD=true
ENABLE_AUDIT_LOGGING=true

# Monitoring
PROMETHEUS_ENDPOINT=http://localhost:9090
EOF
    print_success ".env file created"
    print_warning "Please update .env with your GitHub OAuth credentials"
else
    print_status ".env file already exists"
fi

# Create data directories
print_status "Creating data directories..."
mkdir -p data/logs
mkdir -p data/uploads
mkdir -p data/cache
mkdir -p target/coverage
print_success "Data directories created"

# Build the project
print_status "Building RustCI..."
cargo build
print_success "Build completed"

# Run basic tests
print_status "Running basic tests..."
cargo test --lib --quiet
print_success "Tests passed"

print_success "RustCI development environment setup complete!"
print_status ""
print_status "Next steps:"
print_status "1. Update .env with your GitHub OAuth credentials"
print_status "2. Start MongoDB: docker-compose up -d mongodb"
print_status "3. Run RustCI: cargo run"
print_status "4. Access at: http://localhost:8080"
print_status ""
print_status "Useful commands:"
print_status "  cargo run                    # Start development server"
print_status "  cargo watch -x run          # Start with hot reload"
print_status "  cargo test                   # Run tests"
print_status "  ./scripts/deploy.sh          # Deploy with options"
print_status "  ./scripts/test-runner.sh     # Run comprehensive tests"