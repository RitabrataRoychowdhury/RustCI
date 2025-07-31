# Multi-stage Dockerfile for RustAutoDevOps
# Stage 1: Build environment with dependencies
FROM rust:1.82-slim AS dependencies

# Install system dependencies for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml ./

# Create dummy main.rs to build dependencies (without Cargo.lock to avoid version conflicts)
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release && rm -rf src

# Stage 2: Build application
FROM rust:1.82-slim AS builder

# Install system dependencies for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy pre-built dependencies from previous stage
COPY --from=dependencies /app/target target
COPY --from=dependencies /usr/local/cargo /usr/local/cargo

# Copy source code
COPY Cargo.toml ./
COPY Cargo.loc[k] ./
COPY src ./src
COPY docs ./docs

# Build application with optimizations
RUN cargo build --release

# Stage 3: Runtime environment
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create non-root user
RUN useradd -r -s /bin/false -m -d /app rustapp

# Set working directory
WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/RustAutoDevOps /app/rustapp

# Copy configuration files if needed
COPY --from=builder /app/docs ./docs
COPY --from=builder /app/examples ./examples

# Change ownership to non-root user
RUN chown -R rustapp:rustapp /app

# Switch to non-root user
USER rustapp

# Expose port (adjust as needed)
EXPOSE 8000

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8000/health || exit 1

# Set environment variables
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1

# Run the application
CMD ["./rustapp"]
