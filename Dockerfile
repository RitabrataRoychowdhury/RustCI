# Multi-stage Dockerfile for RustCI
# Stage 1: Build environment with dependencies
FROM rust:1.82-slim AS dependencies

# Install system dependencies for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    curl \
    && rm -rf /var/lib/apt/lists/*

# Set working directory
WORKDIR /app

# Copy manifests and required files
COPY Cargo.toml ./
COPY Cargo.loc[k] ./
COPY benches ./benches

# Create dummy main.rs and bin files to build dependencies
RUN mkdir -p src/bin && echo "fn main() {}" > src/main.rs
RUN echo "fn main() {}" > src/bin/valkyrie-config.rs
RUN echo "fn main() {}" > src/bin/performance_validator.rs

# Build dependencies (this layer will be cached)
RUN cargo build --release && rm -rf src

# Stage 2: Build application
FROM rust:1.82-slim AS builder

# Install system dependencies for building
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    curl \
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
COPY examples ./examples
COPY benches ./benches

# Build application with optimizations
RUN cargo build --release

# Stage 3: Runtime environment
FROM debian:bookworm-slim AS runtime

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    curl \
    && rm -rf /var/lib/apt/lists/* \
    && apt-get clean

# Create non-root user
RUN useradd -r -s /bin/false -m -d /app rustci

# Set working directory
WORKDIR /app

# Copy the binary from builder stage
COPY --from=builder /app/target/release/rustci /app/rustci

# Copy configuration files if needed
COPY --from=builder /app/docs ./docs
COPY --from=builder /app/examples ./examples

# Change ownership to non-root user
RUN chown -R rustci:rustci /app

# Switch to non-root user
USER rustci

# Expose port (adjust as needed)
EXPOSE 8000

# Health check - Use RustCI API endpoints with fallback
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8000/api/healthchecker || curl -f http://localhost:8000/health || exit 1

# Set environment variables
ENV RUST_LOG=info
ENV RUST_BACKTRACE=1
ENV RUST_ENV=production

# Run the application
CMD ["./rustci"]