# tempo-x402-server

Library name: `x402_server`. Actix-web resource server that gates endpoints behind HTTP 402 payments.

Returns 402 with pricing when client has no `PAYMENT-SIGNATURE` header. When present, forwards to facilitator for verification and settlement via HMAC-authenticated POST.

Binary: `x402-server` on port 4021.

## Depends On

- `x402` (core types, HMAC, constants)

## Non-Obvious Patterns

- HMAC auth on facilitator requests via `X-Facilitator-Auth` header (computed in `middleware.rs`)
- `require_payment()` in `middleware.rs` is the main orchestrator for the payment gate
- Metrics token uses constant-time comparison
- CORS explicitly allows `payment-signature` and `x-facilitator-auth` headers

## If You're Changing...

- **Adding a paid endpoint**: Use `PaymentConfigBuilder` in `config.rs` — call `.route(method, path, price, description)` for each endpoint. `PaymentConfig::new()` is a convenience that registers the default `GET /blockNumber` route.
- **Payment flow logic**: `middleware.rs` — `require_payment()` → `decode_payment_header()` → `call_verify_and_settle()`
- **Metrics**: Follow `x402_server_` prefix in `metrics.rs`
