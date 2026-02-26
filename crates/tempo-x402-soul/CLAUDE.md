# tempo-x402-soul

Library crate. Agentic "soul" for x402 nodes: a periodic observe-think-record loop powered by Gemini.

Observes node state via `NodeObserver` trait, reasons about it via Gemini API, records thoughts to a separate SQLite database. Decisions are logged but not executed (v1). Runs dormant (observe-only) without a Gemini API key.

## Depends On

- `x402` (core types only)

No dependency on gateway/identity/agent/node. Communicates via `NodeObserver` trait — the node crate implements it.

## Non-Obvious Patterns

- Separate SQLite DB (`soul.db`) — does NOT share the gateway DB
- On Railway, `SOUL_DB_PATH` must point to persistent volume (`/data/soul.db`)
- Dormant mode: without `GEMINI_API_KEY`, still observes and records, skips LLM calls
- Default model: `gemini-3-flash-preview` (configurable via `GEMINI_MODEL_FAST` env var)
- Fixed-interval loop (default 60s) — calls Gemini every cycle, no urgency gating
- Gemini retry: 3 attempts, exponential backoff (500ms/1s/2s) with ±25% jitter
- HTTP client: 30s timeout, `redirect(Policy::none())`
- `Soul::spawn()` consumes self, returns a `JoinHandle` — clone `soul.database()` Arc before spawning
- Personality and generation are configurable via env vars for lineage tracking

## If You're Changing...

- **Gemini API**: `gemini.rs` — model names, endpoint format, retry logic
- **Thinking loop**: `thinking.rs` — observe → think → record cycle (fixed interval, no urgency)
- **Database schema**: `db.rs` — `thoughts` + `soul_state` tables
- **Observer trait**: `observer.rs` — changing `NodeSnapshot` fields affects all implementors
- **Used by**: `x402-node` stores `Arc<SoulDatabase>` in `NodeState`, exposes via `GET /soul/status`, implements `NodeObserver` in `soul_observer.rs`
