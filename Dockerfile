# ============================================
# Build Stage
# ============================================
FROM rust:bookworm AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /app

# Copy manifests
COPY Cargo.toml ./

# Copy source code
COPY src ./src
COPY .sqlx ./.sqlx

COPY sql_models ./sql_models

# Build the application in release mode
RUN cargo build --release 
# ============================================
# Runtime Stage
# ============================================
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    libssl3 \
    && rm -rf /var/lib/apt/lists/*

# Create non-root user
RUN useradd -m -u 1000 hushnet

# Create app directory
WORKDIR /app

# Copy the binary from builder
COPY --from=builder /app/target/release/hushnet-backend /app/hushnet-backend

# Change ownership
RUN chown -R hushnet:hushnet /app

# Switch to non-root user
USER hushnet

# Expose port
EXPOSE 8080

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD curl -f http://localhost:8080/ || exit 1

# Run the binary
CMD ["/app/hushnet-backend"]
