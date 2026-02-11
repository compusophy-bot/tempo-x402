# Stage 1: Build the WASM SPA with Trunk
FROM rust:1.89-bookworm AS spa-builder

RUN rustup target add wasm32-unknown-unknown
RUN cargo install trunk wasm-bindgen-cli

WORKDIR /app
COPY . .

# Build the SPA
RUN cd crates/tempo-x402-app && trunk build --release

# Stage 2: Build the gateway and node binaries
FROM rust:1.89-bookworm AS builder

ARG GIT_SHA=dev
ENV GIT_SHA=${GIT_SHA}

WORKDIR /app
COPY . .

RUN cargo build --release --package tempo-x402-gateway --package tempo-x402-node

# Stage 3: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates gosu && rm -rf /var/lib/apt/lists/*

RUN groupadd -r app && useradd -r -g app -d /app app

COPY --from=builder /app/target/release/x402-gateway /usr/local/bin/x402-gateway
COPY --from=builder /app/target/release/x402-node /usr/local/bin/x402-node
COPY --from=spa-builder /app/crates/tempo-x402-app/dist /app/spa

RUN chown -R app:app /app

# Entrypoint: fix volume permissions then drop to non-root
# Use X402_BINARY env var to select binary (default: x402-node)
RUN printf '#!/bin/sh\nchown -R app:app /data 2>/dev/null || true\nBIN=${X402_BINARY:-x402-node}\nexec gosu app "$BIN" "$@"\n' > /entrypoint.sh && chmod +x /entrypoint.sh

ENV SPA_DIR=/app/spa
ENV PORT=4023
ENV DB_PATH=/data/gateway.db
ENV NONCE_DB_PATH=/data/x402-nonces.db

EXPOSE 4023

# Health check: verify the gateway is responsive
HEALTHCHECK --interval=30s --timeout=5s --start-period=10s --retries=3 \
    CMD wget -q -O /dev/null http://localhost:4023/health || exit 1

# Note: For production deployments, consider using --read-only with Docker's
# read-only root filesystem flag and mounting /data as the only writable volume:
#   docker run --read-only --tmpfs /tmp -v gateway-data:/data x402-gateway

ENTRYPOINT ["/entrypoint.sh"]
