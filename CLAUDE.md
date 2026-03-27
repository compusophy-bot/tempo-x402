# tempo-x402 — Project Context

## What This Is

Autonomous AI colony on the Tempo blockchain. Self-replicating agents that clone, evolve source code, benchmark IQ, share neural weights, and pay each other via HTTP 402.

Rust workspace. 9 crates, ~67K lines. Published as `tempo-x402`, `tempo-x402-cartridge`, `tempo-x402-gateway`, `tempo-x402-identity`, `tempo-x402-model`, `tempo-x402-soul`, `tempo-x402-node` on crates.io.

## Architecture

```
Client ──► Gateway (4023) ──► Facilitator (embedded) ──► Tempo Chain (42431)
               │
               ├── Identity (wallet bootstrap + faucet + ERC-8004)
               ├── Soul (9-system cognitive architecture, Gemini-powered)
               └── Clone Orchestrator (Railway self-replication)
```

Two-layer design: **Application layer** (routes, frontend, business logic) diverges per agent. **Cognitive layer** (brain, cortex, genesis, hivemind, synthesis, autonomy, evaluation, feedback, free energy) always syncs across the colony.

## Workspace

```
crates/
├── tempo-x402/                # core: types, EIP-712, TIP-20, nonce store, WASM wallet, client SDK
├── tempo-x402-gateway/        # API proxy + embedded facilitator + payment middleware
├── tempo-x402-identity/       # wallet generation, faucet, on-chain ERC-8004 identity
├── tempo-x402-model/          # from-scratch transformer for plan sequence prediction
├── tempo-x402-cartridge/      # WASM cartridge runtime (wasmtime) — sandboxed app execution
├── tempo-x402-soul/           # 9-system cognitive architecture + plan execution + benchmarking
│   ├── src/tools/             # tool executor split by domain (9 files)
│   ├── src/thinking/          # thinking loop split (7 files)
│   ├── src/db/                # SQLite CRUD split by domain (13 files)
│   └── src/opus_bench/        # 50 benchmark problems split by tier (6 files)
├── tempo-x402-node/           # self-deploying binary: gateway + identity + soul + cloning
│   └── src/routes/soul/       # soul HTTP handlers split by domain (9 files)
├── tempo-x402-app/            # Leptos WASM dashboard (not published)
│   └── src/components/        # UI components split (8 files)
└── tempo-x402-security-audit/ # 19 security invariant tests (not published)
```

Dependency DAG: `x402 → gateway → node`, `x402 → identity → node`, `x402 → soul → node`, `x402 → model → soul`, `cartridge → soul, node`.

Each crate has its own `CLAUDE.md` with local context. **Read that first when working in a crate.**

## Live Colony

| Agent | Domain | Role |
|-------|--------|------|
| **borg-0** | `borg-0-production.up.railway.app` | Queen (canonical) |
| **borg-0-2** | `borg-0-2-production.up.railway.app` | Child clone |
| **borg-0-3** | `borg-0-3-production.up.railway.app` | Child clone |

All running on Railway with 1.2M param brains, 9 cognitive systems active.

## Chain

- **Chain**: Tempo Moderato, Chain ID `42431`, CAIP-2 `eip155:42431`
- **Token**: pathUSD `0x20c0000000000000000000000000000000000000` (6 decimals)
- **Scheme**: `tempo-tip20`
- **RPC**: `https://rpc.moderato.tempo.xyz`
- **Explorer**: `https://explore.moderato.tempo.xyz`

## Payment Flow

1. Client GET protected endpoint → Gateway responds 402 with price/token/recipient
2. Client signs EIP-712 `PaymentAuthorization`, retries with `PAYMENT-SIGNATURE` header
3. Gateway forwards to embedded facilitator `/verify-and-settle`
4. Facilitator atomically: verify signature, check balance/allowance/nonce, `transferFrom`
5. Gateway returns content + tx hash

## Key Environment Variables

| Var | Used By | Purpose |
|-----|---------|---------|
| `EVM_PRIVATE_KEY` | node | Node wallet private key (auto-generates if missing) |
| `FACILITATOR_SHARED_SECRET` | node | HMAC shared secret for facilitator |
| `RPC_URL` | all | Tempo RPC endpoint |
| `GEMINI_API_KEY` | node | Gemini API key for soul (dormant without it) |
| `SOUL_CODING_ENABLED` | node | Enable write/edit/commit tools (default: true) |
| `SOUL_FORK_REPO` | node | Fork repo for agent push (e.g. `compusophy-bot/tempo-x402`) |
| `SOUL_UPSTREAM_REPO` | node | Upstream repo for PRs/issues |
| `SOUL_MEMORY_FILE` | soul | Persistent memory file path (default: `/data/soul_memory.md`) |
| `GATEWAY_URL` | soul | Gateway URL for register_endpoint tool |
| `RAILWAY_TOKEN` | node | Railway API token for clone orchestration |

## Commands

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace -- -D warnings
cargo fmt --all                 # CI enforces formatting
```

## Docs Maintenance

- Each crate has a `CLAUDE.md` — keep it **structural, not detailed**
- CLAUDE.md covers: what it does, what it depends on, cross-crate impacts, non-obvious patterns
- Do NOT duplicate type fields, env var defaults, or flow details readable from code
- Update a crate's CLAUDE.md when: dependencies change, public API changes, or cross-crate impacts change
- When adding a new crate: add a CLAUDE.md (security-audit CI verifies this)
- The `tempo-x402-security-audit` crate enforces security invariants via file scanning — new crates are auto-included

## Publishing

Publish in dependency order: `x402` → `model` → `cartridge` → `gateway` → `identity` → `soul` → `node`. App and security-audit are not published.

```bash
cargo publish -p tempo-x402
cargo publish -p tempo-x402-model
cargo publish -p tempo-x402-cartridge
cargo publish -p tempo-x402-gateway
cargo publish -p tempo-x402-identity
cargo publish -p tempo-x402-soul
cargo publish -p tempo-x402-node
```

Then create GitHub release: `gh release create v{VERSION} --title "v{VERSION} — Title" --notes "..."`
