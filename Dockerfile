# Dockerfile for Synapsis Secure MCP Server Compilation
FROM rust:1.75-slim AS builder

# Install build dependencies
RUN apt-get update && apt-get install -y \
    pkg-config \
    libssl-dev \
    && rm -rf /var/lib/apt/lists/*

# Create app directory
WORKDIR /usr/src/synapsis

# Copy source
COPY . .

# Build in release mode
RUN cargo build --release --bin synapsis-mcp

# Runtime image
FROM debian:bookworm-slim

# Install runtime dependencies
RUN apt-get update && apt-get install -y \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# Copy binary from builder
COPY --from=builder /usr/src/synapsis/target/release/synapsis-mcp /usr/local/bin/synapsis-mcp

# Create non-root user
RUN useradd -m -u 1000 synapsis
USER synapsis
WORKDIR /home/synapsis

# Expose default MCP TCP port
EXPOSE 7438 7439

# Health check
HEALTHCHECK --interval=30s --timeout=3s --start-period=5s --retries=3 \
    CMD nc -z localhost 7439 || exit 1

# Entrypoint
ENTRYPOINT ["synapsis-mcp"]
CMD ["--tcp", "--secure"]