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

## Cockpit

Single-page Bloomberg terminal / spaceship bridge. No page navigation — everything visible at once.

```
┌─────────────────────────────────────────────────────────────────────┐
│ tempo-x402 v6.1.0 │ borg-0 │ 0x2e1c..7830 │ 999997 pathUSD │ ↑3h │
├────────────┬───────────────────────────┬────────────────────────────┤
│ Ψ(t)      │ COGNITIVE SYSTEMS          │ PROCESSES                  │
│ Ψ=0.4218  │ BRAIN    1.2M  loss=1.07  │ > soul [code]     [ok]    │
│ trend ↑   │ XFORMER  2.2M  loss=2.45  │ > tools            on     │
│ F=0.251   │ QUALITY  1.1M  loss=0.07  │ > coding            on    │
│ EXPLOIT   │ CODEGEN  50M   loss=3.41  │                            │
│           │ CORTEX   acc=72% val=+0.3 │ CARTRIDGES                 │
│ FITNESS   │ GENESIS  gen=147 tmpl=200 │ ● snake-game       [live]  │
│ ███░░ 80% │ HIVEMND  trails=45 dep=3  │ ● tetris-core      [live]  │
│ eco  62%  │ SYNTH    coherent conf=3  │                            │
│ exec 85%  │ EVAL     delta=+0.02      │ COLONY                     │
│ evol 97%  │                           │ borg-0    ████ 80% queen   │
│ coord 97% │ BENCHMARK                 │ borg-0-2  ███░ 46% clone   │
│ intro 24% │ 65.6% pass@1  21/32      │ sync: 2 peers, Δ+0.02     │
├────────────┴───────────────────────────┴────────────────────────────┤
│ [CHAT]  [LOGS]                                                      │
│ > make a snake game                                                  │
│ soul: Building cartridge snake-game... compile_cartridge              │
│ > _                                                                  │
├─────────────────────────────────────────────────────────────────────┤
│ CPU 79% │ MEM 73% │ DISK 4% │ cycles=180 │ tempo-x402 v6.1.0      │
└─────────────────────────────────────────────────────────────────────┘
```

- **Monospace everything**: JetBrains Mono, green on black, no padding waste
- **All 9 cognitive systems** visible with real metrics
- **Ψ(t) + F(t)** with trend arrows and regime badges
- **5-component fitness** bars with color-coded levels
- **Chat**: multi-turn sessions with tool execution blocks, plan approval bar
- **Live colony**: peer fitness bars, sync delta

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

### v6.1.0 — Hacker's Cockpit + Queen Audit

**Cockpit Frontend**
- Single-page Bloomberg terminal replacing 4 separate pages (home, dashboard, studio, timeline)
- Monospace (JetBrains Mono), green/cyan/amber/red on black (#080810)
- 3-column layout: Ψ+fitness | cognitive grid+benchmark+plan | processes+cartridges+colony
- Tabbed bottom panel: CHAT | LOGS with plan approval bar
- Status bar: CPU/MEM/DISK + cycle count
- No router, no page navigation — everything visible at once

**Queen Commit Audit**
- Cherry-picked: planning prompt improvements (mental simulation rules), benchmark interval 10→5, test timeout 300→600s + --nocapture
- Rejected: benchmark solution memoization (metric gaming), dead code files, toothless safety checks
- Feedback nudge sent to queen with rules: never cache benchmarks, always wire modules, never add inactive guards

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
