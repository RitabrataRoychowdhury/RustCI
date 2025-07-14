# Stage 1 – Build (with newer Rust)
FROM rust:1.85-slim as builder

# Install musl target and build deps
RUN rustup target add x86_64-unknown-linux-musl && \
    apt-get update && \
    apt-get install -y musl-tools pkg-config libssl-dev && \
    apt-get clean && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Cache dependencies
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo "fn main() {}" > src/main.rs
RUN cargo build --release --target x86_64-unknown-linux-musl
RUN rm -rf src

# Build your real code
COPY . .
RUN cargo build --release --target x86_64-unknown-linux-musl
RUN strip target/x86_64-unknown-linux-musl/release/rustautodevops

# Stage 2 – Minimal runtime
FROM scratch
COPY --from=builder /app/target/x86_64-unknown-linux-musl/release/rustautodevops /ci-server
ENTRYPOINT ["/ci-server"]
