# tempo-x402-security-audit

Test-only crate. No runtime code. Not published. Runs on every `cargo test --workspace`.

14 tests that scan production `.rs` files via regex to enforce security invariants. New crates are auto-included (walks `crates/*/src/**/*.rs`).

## Depends On

Dev-only: `walkdir`, `regex`. No workspace crate deps.

## The Tests

1. No hardcoded private keys outside demo/test
2. HMAC verify reaches constant-time comparison on all paths
3. All reqwest builders disable redirects
4. Constant-time comparison uses `subtle` crate
5. Webhook validation returns hard error for non-HTTPS
6. HTTP error responses don't leak internals
7. Facilitator uses SqliteNonceStore in production
8. No SQL string formatting (must use parameterized queries)
9. Node registration validates UUID, EVM address, HTTPS URL
10. Private key never in tracing macros
11. Parent URL requires HTTPS
12. Clone route uses atomic limit check
13. Clone errors don't leak internal details
14. HMAC secret is mandatory (not Optional)

## If You're Changing...

- **Adding a crate**: Auto-scanned. No action needed.
- **Getting a false positive**: Add to the allowlist in the specific test
- **Adding a security invariant**: Add test to `tests/security_invariants.rs`
- **These are regex heuristics, not AST**: Code formatting can affect matching
