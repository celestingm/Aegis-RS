# Stage 1: Builder
FROM rust:1.75-bookworm AS builder

WORKDIR /usr/src/aegis-rs
COPY . .

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
    && rm -rf /var/lib/apt/lists/*

# Copy the binary from builder
COPY --from=builder /usr/src/aegis-rs/target/release/aegis-rs .
# Copy the default config (can be overridden by mounting a volume)
COPY --from=builder /usr/src/aegis-rs/config.toml .

# Expose the API port
EXPOSE 3000

# Set the entrypoint
CMD ["./aegis-rs"]
