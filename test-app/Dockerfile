# Multi-stage build for efficient image
FROM rust:1.70-alpine AS builder

# Install build dependencies
RUN apk add --no-cache musl-dev

# Create app directory
WORKDIR /app

# Copy manifest files
COPY Cargo.toml Cargo.lock ./

# Copy source code
COPY src ./src

# Build the application
RUN cargo build --release

# Runtime stage
FROM alpine:3.18

# Install runtime dependencies
RUN apk add --no-cache ca-certificates

# Create non-root user
RUN addgroup -g 1000 rustci && \
    adduser -D -s /bin/sh -u 1000 -G rustci rustci

# Create app directory
WORKDIR /app

# Copy binary from builder stage
COPY --from=builder /app/target/release/test-app /app/test-app

# Change ownership
RUN chown -R rustci:rustci /app

# Switch to non-root user
USER rustci

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD wget --no-verbose --tries=1 --spider http://localhost:8080/health || exit 1

# Run the application
CMD ["./test-app"]