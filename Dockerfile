# Build stage
FROM docker.io/library/rust:1.83-bookworm AS builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml ./

# Copy source code
COPY src ./src

# Build for release
RUN cargo build --release

# Runtime stage - Use Ubuntu 24.04 for newer glibc (2.39)
FROM docker.io/library/ubuntu:24.04

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/linera-rest-server /usr/local/bin/linera-rest-server

# Copy web UI
COPY web ./web

# Create directories for wallet and keystore
RUN mkdir -p /data

# Environment variables
ENV PORT=3000
ENV WEB_DIR=/app/web
ENV LINERA_BIN=/usr/local/bin/linera
ENV LINERA_WALLET=/data/wallet.json
ENV LINERA_KEYSTORE=/data/keystore.json

# Expose port
EXPOSE 3000

# Run the server
CMD ["linera-rest-server"]
