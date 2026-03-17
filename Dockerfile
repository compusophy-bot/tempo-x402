# Stage 1: cargo-chef — plan dependency build from Cargo manifests only
FROM rust:1.89-bookworm AS chef
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

# Install WASM toolchain for SPA build
RUN rustup target add wasm32-unknown-unknown
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

# Copy Rust toolchain from builder for benchmark (cargo test on Exercism problems)
COPY --from=builder /usr/local/rustup /usr/local/rustup
COPY --from=builder /usr/local/cargo /usr/local/cargo
ENV RUSTUP_HOME=/usr/local/rustup
ENV CARGO_HOME=/usr/local/cargo
ENV PATH="/usr/local/cargo/bin:${PATH}"
RUN chmod -R a+rX /usr/local/rustup /usr/local/cargo

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
