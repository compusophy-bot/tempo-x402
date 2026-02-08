# Stage 1: Build the WASM SPA with Trunk
FROM rust:1.89-bookworm AS spa-builder

RUN rustup target add wasm32-unknown-unknown
RUN cargo install trunk wasm-bindgen-cli

WORKDIR /app
COPY . .

# Build the SPA
RUN cd crates/tempo-x402-app && trunk build --release

# Stage 2: Build the gateway binary
FROM rust:1.89-bookworm AS gateway-builder

WORKDIR /app
COPY . .

RUN cargo build --release --package tempo-x402-gateway

# Stage 3: Runtime
FROM debian:bookworm-slim

RUN apt-get update && apt-get install -y ca-certificates && rm -rf /var/lib/apt/lists/*

COPY --from=gateway-builder /app/target/release/x402-gateway /usr/local/bin/x402-gateway
COPY --from=spa-builder /app/crates/tempo-x402-app/dist /app/spa

ENV SPA_DIR=/app/spa
ENV PORT=4023

EXPOSE 4023

CMD ["x402-gateway"]
