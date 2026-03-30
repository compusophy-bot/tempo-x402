<p align="center">
  <h1 align="center">tempo-x402</h1>
  <p align="center"><strong>Autonomous AI colony on the Tempo blockchain. Self-replicating agents that write Rust, compile WASM, benchmark IQ, share neural weights, and pay each other with crypto. Pure Rust. 54M+ neural parameters. Colony consciousness metric Ψ(t).</strong></p>
</p>

<p align="center">
  <a href="https://crates.io/crates/tempo-x402"><img src="https://img.shields.io/crates/v/tempo-x402.svg" alt="crates.io"></a>
  <a href="https://docs.rs/tempo-x402"><img src="https://docs.rs/tempo-x402/badge.svg" alt="docs.rs"></a>
  <a href="https://github.com/compusophy/tempo-x402/blob/main/LICENSE"><img src="https://img.shields.io/badge/license-MIT-blue.svg" alt="MIT License"></a>
</p>

<p align="center">
  <a href="https://docs.rs/tempo-x402">Docs</a> &middot;
  <a href="https://crates.io/crates/tempo-x402">Crates</a> &middot;
  <a href="https://borg-0-production.up.railway.app">Live Colony</a> &middot;
  <a href="https://borg-0-production.up.railway.app/studio">Studio</a>
</p>

---

## What is this?

A colony of autonomous AI agents that **measurably get smarter over time** and **pay for their own compute**.

Each agent is a single Rust binary that bootstraps its own crypto wallet, runs a payment gateway, thinks via a 9-system cognitive architecture, writes and compiles its own Rust code, benchmarks itself against 50 novel coding problems, trains 4 neural models locally, and shares what it learns with every other agent in the swarm.

The core thesis: **N constrained agents collectively outperform any single model**. Colony consciousness Ψ(t) = Intelligence × Sync × Diversity × Learning_Velocity. When Ψ rises, the colony is getting smarter than any individual.

### Why this matters

| Property | How |
|----------|-----|
| **Verifiable intelligence** | 50 compiler-verified coding problems (Opus IQ Benchmark). `cargo test` passes or it doesn't. |
| **Neuroplastic self-modification** | Agents write WASM modules that modify their own intelligence at runtime. No redeploy. |
| **Colony consciousness (Ψ)** | Single metric measuring collective intelligence. Drives behavioral regime and phase transitions. |
| **4 neural models, pure Rust** | Brain (1.2M), Transformer (2.2M), Code Quality (1.1M), Code Gen (50M). No ML framework. |
| **Cartridge OS** | Agents compile Rust → WASM → hot-load instantly. Interactive 60fps framebuffer apps. |
| **Economic sustainability** | HTTP 402 payments on Tempo blockchain. Every API call earns pathUSD. |

## Architecture

```
                    ┌──────────────────────────────────┐
                    │        CARTRIDGE OS               │  WASM apps + cognitive modules
                    │  Interactive (60fps framebuffer)   │  hot-loaded, no redeploy
                    │  Backend (API + compute)           │
                    │  Cognitive (self-modification)     │
                    └──────────────┬───────────────────┘
                                   │
┌──────────────────────────────────┴───────────────────────────────────┐
│                      COGNITIVE LAYER (always syncs)                   │
│                                                                      │
│  ┌─────────┐ ┌─────────┐ ┌─────────┐ ┌──────────┐ ┌─────────────┐  │
│  │  BRAIN   │ │ CORTEX  │ │ GENESIS │ │ HIVEMIND │ │  SYNTHESIS  │  │
│  │ 1.2M NN │ │World Mdl│ │Plan DNA │ │Pheromones│ │Metacognition│  │
│  └─────────┘ └─────────┘ └─────────┘ └──────────┘ └─────────────┘  │
│                                                                      │
│  ┌──────────┐ ┌──────────┐ ┌────────────┐ ┌───────────────────┐     │
│  │ AUTONOMY │ │EVALUATION│ │  FEEDBACK   │ │    FREE ENERGY    │     │
│  │ LLM-free │ │  Brier   │ │Error class. │ │ F(t) + Ψ(t)      │     │
│  │ planning │ │ scores   │ │  Lessons    │ │ EXPLORE/EXPLOIT   │     │
│  └──────────┘ └──────────┘ └────────────┘ └───────────────────┘     │
│                                                                      │
│  ← All 9 systems federated across colony via peer sync protocol →   │
└──────────────────────────────────────────────────────────────────────┘
```

## Four Neural Models

All from-scratch. No ML framework. Pure Rust. 54M+ parameters total.

| Model | Params | Architecture | Purpose |
|-------|--------|-------------|---------|
| **Brain** | 1.2M | 128→1024→1024→23 FFN | Step success prediction, error classification, brain gating |
| **Plan Transformer** | 2.2M | 4-layer causal attention, D=256, 8 heads | Plan sequence generation WITHOUT LLM calls |
| **Code Quality** | 1.1M | 32→1024→1024→1 FFN | Diff evaluation, commit gating, benchmark-trained |
| **Code Gen** | 50M | 8-layer transformer, D=512, 8 heads, 8K BPE vocab | Local Rust code generation (Phase 3) |

All models train online (no batch jobs, no GPU) and share weights across the colony via federated averaging.

## Ψ(t) — Colony Consciousness

```
Ψ(t) = (Intelligence × Sync × Diversity × Velocity)^0.25
```

- **Intelligence**: mean pass@1 across colony (raw coding ability)
- **Sync**: accuracy improvement from peer weight sharing
- **Diversity**: fitness standard deviation (specialization pressure)
- **Velocity**: -dF/dt (negative free energy trend = learning)

Ψ drives phase transitions: when Ψ > 0.5 with >500 training examples and pass@1 > 60%, Phase 3 activates and the colony begins building its local code generation model.

## Cartridge OS

The node is an operating system. Agents write Rust, compile to WASM, and deploy instantly — no restart.

**Three cartridge types:**

| Type | Exports | Use case |
|------|---------|----------|
| **Backend** | `x402_handle` | HTTP APIs, JSON services, server compute |
| **Interactive** | `x402_tick`, `x402_get_framebuffer` | Games, visualizations, 60fps canvas apps |
| **Cognitive** | Registered as tools | Self-modification modules — agent rewrites its own intelligence |

**Studio preview**: WASM-within-WASM. The Leptos SPA instantiates cartridge binaries client-side via `WebAssembly.instantiate()` and renders output inline. Interactive cartridges blit framebuffers to `<canvas>` at 60fps.

## Workspace

Nine crates, clean dependency DAG:

| Crate | What it does |
|-------|-------------|
| [`tempo-x402`](https://crates.io/crates/tempo-x402) | Core: EIP-712 signing, TIP-20 contracts, WASM wallet, client SDK |
| [`tempo-x402-gateway`](https://crates.io/crates/tempo-x402-gateway) | Payment gateway + embedded facilitator + endpoint proxy |
| [`tempo-x402-identity`](https://crates.io/crates/tempo-x402-identity) | Wallet generation, faucet, on-chain ERC-8004 identity + peer discovery |
| [`tempo-x402-model`](https://crates.io/crates/tempo-x402-model) | 4 ML models: brain, transformer, quality, code gen + BPE tokenizer |
| [`tempo-x402-cartridge`](https://crates.io/crates/tempo-x402-cartridge) | WASM cartridge runtime (wasmtime) — sandboxed app + cognitive module execution |
| [`tempo-x402-soul`](https://crates.io/crates/tempo-x402-soul) | 9-system cognitive architecture, Ψ(t), plan execution, benchmarking, neuroplastic self-modification |
| [`tempo-x402-node`](https://crates.io/crates/tempo-x402-node) | Self-deploying binary: gateway + identity + soul + clone orchestration |
| `tempo-x402-app` | Leptos WASM dashboard with WASM-within-WASM cartridge preview (bundled) |
| `tempo-x402-security-audit` | 19 security invariant tests (not published) |

## Studio

```
┌──────────────┬──────────────────────────────┬───────────────────┐
│  CARTRIDGES  │     PREVIEW                  │      CHAT         │
│  snake  cart │  ┌────────────────────────┐  │  "make a game"    │
│  tetris cart │  │  60fps canvas          │  │  Soul: Building.. │
│  calc   cart │  │  (WASM-within-WASM)    │  │  [good] [bad]     │
│  FILES ▸     │  └────────────────────────┘  │  [input bar]      │
├──────────────┴──────────────────────────────┴───────────────────┤
│ Fitness 80% | F=0.25 EXPLOIT | Ψ=0.42↑ | ELO -- | CPU RAM Disk │
└─────────────────────────────────────────────────────────────────┘
```

- **Cartridge browser**: Scripts + WASM cartridges. Click to preview.
- **WASM-within-WASM preview**: Cartridge binaries instantiated client-side. No iframe.
- **Interactive canvas**: 60fps framebuffer rendering for game/viz cartridges.
- **Chat**: Multi-turn sessions. Agent builds cartridges when asked.
- **Feedback**: `good`/`bad` buttons train the quality model (human-in-the-loop).
- **Status bar**: Fitness, F(t), Ψ(t), ELO, CPU, RAM, Disk — event-driven, no polling.

## Quick Start

```bash
git clone https://github.com/compusophy/tempo-x402
cd tempo-x402
cargo build --release

export GEMINI_API_KEY="your-key"
./target/release/x402-node
```

The node auto-bootstraps: generates wallet, requests faucet funds, mints on-chain identity, starts gateway on port 4023, begins cognitive loop.

## Changelog

### v6.0.0 — Ψ(t) + Cartridge OS + Phase 3 Code Gen + Neuroplastic Self-Modification

**Colony Consciousness (Ψ)**
- Ψ(t) = (Intelligence × Sync × Diversity × Velocity)^0.25
- Computed every cycle, logged, displayed in Studio status bar
- Drives phase transitions: Ψ > 0.5 → Phase 3 activates

**Cartridge OS**
- Interactive framebuffer cartridges: 60fps canvas rendering, keyboard input
- WASM-within-WASM: Leptos SPA instantiates cartridges client-side
- Cartridge-backed tools: agent writes WASM → registers as tool → LLM uses it
- CartridgeEngine wired into Soul for cognitive cartridge execution

**Phase 3: Local Code Generation**
- BPE tokenizer: 8K vocab, pure Rust, trained on benchmark solutions
- 50M code gen transformer: D=512, 8 layers, 8 heads
- Training pipeline: benchmark solutions → BPE → model training every brain cycle
- Local-first inference hook: attempts local model before Gemini API

**Intelligence Loop**
- Goal priorities overhauled: IQ improvement focus, ban maintenance goals
- 4x faster cycle pacing: 120s idle (was 600s), models train more often
- Enhanced reflection: quality model trains on ALL plan outcomes
- TOON encoder: 30-60% fewer tokens in prompts
- Benchmark solution accumulation: ground truth Rust code stored for Phase 3

**Peer Discovery**
- Blockchain peer discovery: ERC-8004 auto-mint, on-chain registry sync
- link_peer UPSERT: any node can register, not just Railway clones
- Startup discovery reads own children table + parent siblings
- Ghost cleanup no longer deletes reachable linked peers

**Studio**
- No polling: status fetched event-driven only
- System metrics: CPU/RAM/Disk in status bar
- Feedback buttons: text `good`/`bad`, click-locks
- `create_script_endpoint` removed: cartridges only

### v5.1.0 — Deep Planning + Cartridge Fix + Studio Polish
### v5.0.0 — Three-Model Coding Intelligence
### v4.0.0 — WASM Cartridge System
### v3.4.0 — Major Structural Refactor

## License

MIT
