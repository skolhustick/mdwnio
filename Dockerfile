# Stage 1: Build
FROM rust:1.92-slim AS builder

WORKDIR /app

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Copy manifests
COPY Cargo.toml Cargo.lock* ./

# Create dummy src to cache dependencies
RUN mkdir src && echo "fn main() {}" > src/main.rs

# Build dependencies only (cache layer)
RUN cargo build --release && rm -rf src

# Copy actual source code
COPY src ./src
COPY readme.md ./

# Build the actual binary
RUN touch src/main.rs && cargo build --release

# Stage 2: Runtime
FROM gcr.io/distroless/cc-debian12

WORKDIR /app

# Copy binary from builder
COPY --from=builder /app/target/release/mdwnio /app/mdwnio
COPY --from=builder /app/readme.md /app/readme.md

# Expose port
EXPOSE 3000

# Set environment defaults
ENV PORT=3000 \
    CACHE_TTL=3600 \
    REQUEST_TIMEOUT=10 \
    MAX_CONTENT_LENGTH=10485760 \
    MAX_REDIRECTS=5 \
    USER_AGENT="mdwn.io/1.0 (+https://mdwn.io)" \
    RUST_LOG=info

# Run as non-root (distroless default user)
USER nonroot

ENTRYPOINT ["/app/mdwnio"]
