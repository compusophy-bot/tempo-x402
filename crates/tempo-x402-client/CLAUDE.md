# tempo-x402-client

Library name: `x402_client`. Rust SDK for making paid HTTP 402 requests.

Wraps reqwest. Handles the full flow: request → 402 → sign EIP-712 → retry with `PAYMENT-SIGNATURE` header → extract settlement from `payment-response` header.

Binary: `x402-client` — demo CLI.

## Depends On

- `x402` (core types, traits, EIP-712 utils, constants)

## Non-Obvious Patterns

- `X402Client<S: SchemeClient>` is generic over signer — pluggable signing strategies
- Timeout capped at 600s in `TempoSchemeClient` (same as wallet crate)
- Payment header format: base64-encoded JSON, optionally suffixed with `.hmac_hex`
- E2E test in `tests/e2e_gateway.rs` runs against live deployments

## If You're Changing...

- **Payment flow**: `fetch_with_body()` in `http_client.rs` is the core logic
- **Signing**: `create_payment_payload()` in `scheme_client.rs`
- **Header format**: `encode_payment()` / `decode_payment()` in `http_client.rs` — server must match
