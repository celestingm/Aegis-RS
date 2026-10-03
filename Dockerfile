# Stage 1: Builder
FROM rust:1-bookworm AS builder

WORKDIR /usr/src/aegis-rs
COPY . .

# libfontconfig is required by the graph renderer (plotters)
RUN apt-get update && apt-get install -y --no-install-recommends libfontconfig1-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Build the release binary
RUN cargo build --release

# Stage 2: Runtime
FROM debian:bookworm-slim

WORKDIR /app

# Install necessary system tools
# docker.io: allows the container to talk to the host docker socket
# systemd: required for journalctl (though running systemd in container is complex, we install the utils)
RUN apt-get update && \
    apt-get install -y --no-install-recommends \
    docker.io \
    systemd \
    ca-certificates \
    libfontconfig1 \
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /usr/src/aegis-rs/target/release/aegis-rs .
# Ship the example config only; mount your own config.toml (with a real secret_token):
#   docker run -v $(pwd)/config.toml:/app/config.toml:ro ...
COPY --from=builder /usr/src/aegis-rs/config.example.toml ./config.example.toml

# Expose the API port
EXPOSE 3001

# Set the entrypoint
CMD ["./aegis-rs"]
