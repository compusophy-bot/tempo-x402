# Stage 1: cargo-chef — plan dependency build from Cargo manifests only
FROM rust:1.94-bookworm AS chef
RUN cargo install cargo-chef
WORKDIR /app

# Stage 2: Prepare recipe (captures dependency graph without source code)
FROM chef AS planner
COPY . .
RUN cargo chef prepare --recipe-path recipe.json

# Stage 3: Cook dependencies (cached unless Cargo.toml/Cargo.lock change)
FROM chef AS builder

ARG GIT_SHA=dev
ENV GIT_SHA=${GIT_SHA}

# Install WASM toolchains: unknown-unknown for SPA, wasip1 for cartridges
RUN rustup target add wasm32-unknown-unknown wasm32-wasip1
RUN cargo install trunk

# Pre-download wasm-bindgen binary (trunk needs the musl static build;
# downloading it here with retries avoids transient failures during trunk build)
ARG WASM_BINDGEN_VERSION=0.2.108
RUN curl -sSL --retry 5 --retry-delay 5 \
    "https://github.com/rustwasm/wasm-bindgen/releases/download/${WASM_BINDGEN_VERSION}/wasm-bindgen-${WASM_BINDGEN_VERSION}-x86_64-unknown-linux-musl.tar.gz" \
    -o /tmp/wb.tar.gz && \
    mkdir -p /root/.trunk/tools/wasm-bindgen-${WASM_BINDGEN_VERSION} && \
    tar xzf /tmp/wb.tar.gz --strip-components=1 \
        -C /root/.trunk/tools/wasm-bindgen-${WASM_BINDGEN_VERSION}/ && \
    rm /tmp/wb.tar.gz

# Cook: build native dependencies (this layer is cached when only source changes)
# WASM deps (leptos, wasm-bindgen) are lightweight and built by trunk below
COPY --from=planner /app/recipe.json recipe.json
RUN cargo chef cook --release --recipe-path recipe.json

# Now copy actual source and build
COPY . .

# Build the SPA (Trunk compiles Leptos to WASM)
RUN cd crates/tempo-x402-app && trunk build --release

# Build the gateway and node binaries
RUN cargo build --release --package tempo-x402-gateway --package tempo-x402-node

# Stage 4: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y \
    ca-certificates gosu git curl \
    jq python3 bc \
    xvfb xdotool scrot imagemagick \
    gcc libc6-dev \
    libssl-dev pkg-config \
    && rm -rf /var/lib/apt/lists/*

# Install GitHub CLI for soul PR/issue creation
RUN curl -fsSL https://cli.github.com/packages/githubcli-archive-keyring.gpg \
    -o /usr/share/keyrings/githubcli-archive-keyring.gpg && \
    echo "deb [arch=$(dpkg --print-architecture) signed-by=/usr/share/keyrings/githubcli-archive-keyring.gpg] https://cli.github.com/packages stable main" \
    > /etc/apt/sources.list.d/github-cli.list && \
    apt-get update && apt-get install -y gh && \
    rm -rf /var/lib/apt/lists/*

RUN groupadd -r app && useradd -r -g app -d /app app

COPY --from=builder /app/target/release/x402-gateway /usr/local/bin/x402-gateway
COPY --from=builder /app/target/release/x402-node /usr/local/bin/x402-node
COPY --from=builder /app/crates/tempo-x402-app/dist /app/spa

# Copy wasm-bindgen CLI for runtime frontend cartridge compilation
COPY --from=builder /root/.trunk/tools/wasm-bindgen-0.2.108/wasm-bindgen /usr/local/bin/wasm-bindgen

# Copy Rust toolchain from builder for benchmark (cargo test on Exercism problems)
COPY --from=builder /usr/local/rustup /usr/local/rustup
COPY --from=builder /usr/local/cargo /usr/local/cargo
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH="/usr/local/cargo/bin:${PATH}"
# Make cargo writable so cargo check can update the index at runtime.
# Only chmod specific dirs cargo writes to — not the entire tree (too slow).
RUN chmod -R a+rwX /usr/local/cargo/registry 2>/dev/null; \
    chmod a+rw /usr/local/cargo/.package-cache 2>/dev/null; \
    chmod -R a+rX /usr/local/rustup; \
    true

RUN chown -R app:app /app

# Entrypoint: fix volume permissions then drop to non-root
# NOTE: Do NOT chown cargo/registry here — too slow (60s+), blocks healthcheck.
# Cargo registry is world-writable from the chmod in the build stage.
RUN printf '#!/bin/sh\n\
# Fix volume permissions\n\
chown -R app:app /data 2>/dev/null || true\n\
\n\
# Remove old workspace from volume (moved to /tmp in v8)\n\
rm -rf /data/workspace 2>/dev/null || true\n\
# Remove legacy SQLite files from pre-v8\n\
rm -f /data/soul.db /data/soul.db-wal /data/soul.db-shm 2>/dev/null || true\n\
# Remove brain checkpoints (stored in sled now)\n\
rm -rf /data/brain_checkpoints 2>/dev/null || true\n\
# Remove cargo registry cache from volume (save disk space)\n\
rm -rf /data/.cargo 2>/dev/null || true\n\
\n\
# Disk pressure relief: if volume is >80%% full, nuke the sled DB.\n\
# Agent will rebuild from scratch — better than being stuck in a crash loop.\n\
USAGE=$(df /data 2>/dev/null | tail -1 | awk \047{gsub(\"%%\",\"\",$5); print $5}\047)\n\
if [ -n "$USAGE" ] && [ "$USAGE" -gt 80 ] 2>/dev/null; then\n\
  echo "DISK PRESSURE: ${USAGE}%% used — purging sled DB for fresh start"\n\
  rm -rf /data/soul.sled 2>/dev/null || true\n\
fi\n\
\n\
BIN=${X402_BINARY:-x402-node}\n\
exec gosu app "$BIN" "$@"\n\
' > /entrypoint.sh && chmod +x /entrypoint.sh

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
