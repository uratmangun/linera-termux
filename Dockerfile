# Build stage
FROM docker.io/library/rust:1.83-bookworm AS builder

WORKDIR /app

# Copy manifests
COPY Cargo.toml ./

# Copy source code
COPY src ./src

# Build for release
RUN cargo build --release

# Runtime stage
FROM docker.io/library/debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/linera-rest-server /usr/local/bin/linera-rest-server

# Create directories for wallet and keystore
RUN mkdir -p /data

# Environment variables
ENV PORT=3000
ENV LINERA_BIN=/usr/local/bin/linera
ENV LINERA_WALLET=/data/wallet.json
ENV LINERA_KEYSTORE=/data/keystore.json

# Expose port
EXPOSE 3000

# Run the server
CMD ["linera-rest-server"]
