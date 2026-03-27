# tempo-x402-soul

Library crate. Agentic "soul" for x402 nodes: a plan-driven execution loop powered by Gemini with full coding agent capabilities.

## Architecture: Plan-Driven Execution

Replaces the old "prompt and pray" loop with deterministic plan execution:

```
Every N seconds:
  observe → read nudges → check stagnation → get active plan → execute next step → advance plan → housekeeping → sleep

  Steps that DON'T call LLM: ReadFile, SearchCode, ListDir, RunShell, Commit, CheckSelf
  Steps that DO call LLM:    GenerateCode, EditCode, Think (with focused prompt)

  No plan? → Call LLM ONCE to create plan for highest-priority goal
  No goals? → Call LLM ONCE to create goals
  Plan done? → Call LLM ONCE to reflect, then create next plan
```

Observes node state via `NodeObserver` trait, can read/write/edit files and execute shell commands via Gemini function calling, records thoughts and mutations to SQLite. Operates on `vm/<instance-id>` branches with pre-commit validation. Runs dormant (observe-only) without a Gemini API key.

## Depends On

- `x402` (core types + wallet module for EIP-712 signing in `register_endpoint` tool)

No dependency on gateway/identity/node. Communicates via `NodeObserver` trait — the node crate implements it.

## Module Overview

| Module | Purpose |
|--------|---------|
| `plan.rs` | Plan types (PlanStep, Plan, PlanStatus), PlanExecutor — deterministic step execution |
| `thinking/` | Main plan-driven loop (mod.rs: ThinkingLoop + run()), split: plan_cycle, observe, goals, planning, completion, housekeeping, tool_loop |
| `prompts.rs` | 5 focused prompt builders: goal_creation, planning, code_generation, replan, reflection |
| `validation.rs` | **Plan validation**: mechanical plan checks (read-before-edit, cargo-check-before-commit, protected files, durable rules, brain gating, failure chains) — server-side enforcement, not prompt injection |
| `feedback.rs` | **Feedback loop**: structured plan outcomes, error classification, lesson extraction, experience consultation |
| `capability.rs` | **Capability tracking**: per-skill success rates, capability profiles, guidance for prompts |
| `benchmark.rs` | **HumanEval benchmark**: fetches 164 problems from HuggingFace, runs via LLM + Python tests, tracks pass@1 vs published baselines |
| `elo.rs` | **ELO rating**: maps pass@1 to ELO-style intelligence rating, tracks history, compares against reference models |
| `brain.rs` | **Neural brain**: from-scratch feedforward net (~50K params), online SGD training from experience data, distributed weight sharing via peer protocol |
| `cortex.rs` | **Predictive world model**: experience graph + causal edges + curiosity engine + dream consolidation + emotional valence + mental plan simulation. NOT a neural net — sparse associative memory learned from temporal co-occurrence. Peer-shareable via export/merge. |
| `genesis.rs` | **Evolutionary plan templates**: successful plans become "genes" in a gene pool. Crossover, mutation, selection evolve plan structures over time. Templates injected into planning prompts. Colony-wide sharing via peer sync. Memetic evolution — cultural knowledge evolving through selection. |
| `hivemind.rs` | **Stigmergic swarm intelligence**: pheromone trails on files/actions/goals that attract or repel other agents. Evaporation (decay), reinforcement (multiple agents), reputation-weighted influence. Swarm goal coordination (avoid duplication). Collective intelligence aggregation. Inspired by ant colony optimization. |
| `synthesis.rs` | **Metacognitive self-awareness**: unified predictions from all 4 systems (Brain+Cortex+Genesis+Hivemind) with auto-adapting trust weights. Cognitive conflict detection. Self-model (which system is most accurate). Cognitive state machine (Coherent/Conflicted/Exploring/Exploiting/Stuck). Imagination engine: generates plans from causal graph WITHOUT LLM. |
| `autonomy.rs` | **Autonomous planning + recursive self-improvement**: compiles executable plans from templates + causal graph WITHOUT LLM calls. Diagnoses cognitive architecture weaknesses and generates improvement goals. Cognitive peer sync protocol for sharing cortex/genesis/hivemind between peers. |
| `evaluation.rs` | **Rigorous measurement**: per-system Brier scores (calibration quality), calibration curves, Brier decomposition (reliability + resolution), adaptation gain (adapted vs uniform weights), imagination feedback tracking, colony benefit measurement (accuracy before/after peer sync), ablation config. |
| `free_energy.rs` | **Unifying theoretical framework**: single scalar F(t) measuring total cognitive surprise across all systems. F = Σ(system_surprise × weight) + λ×Complexity. Decreasing F = agent getting smarter. Drives behavioral regime (Explore/Learn/Exploit/Anomaly). Trend analysis, history tracking, prompt injection. The theoretical contribution that unifies the entire architecture under the Free Energy Principle. |
| `computer_use.rs` | **Computer use**: screenshot capture, mouse/keyboard simulation, browser control via xdotool/scrot in VM |
| `guard.rs` | Hardcoded protected file list — prevents self-bricking |
| `tools/` | Tool executor (mod.rs: dispatch + struct), split by domain: file_ops, shell, git, deployment, endpoints, social, memory, beliefs, planning |
| `tool_registry.rs` | Dynamic tool registry: register/list/unregister tools at runtime, shell execution |
| `git.rs` | Branch-per-VM git workflow (ensure_branch, commit, push, PR, issues) with fork support |
| `coding.rs` | Pre-commit validation pipeline (cargo check → test → commit) |
| `mode.rs` | Agent modes (Observe, Chat, Code, Review) with per-mode tool sets |
| `llm.rs` | Gemini API client with thought_signature support |
| `chat.rs` | Session-based interactive chat with plan context injection |
| `db/` | SQLite (mod.rs: schema + migrations + types), split by domain: thoughts, state, mutations, tools, beliefs, goals, plans, chat, feedback, events, nudges, benchmark, housekeeping |
| `memory.rs` | Thought types (Observation, Reasoning, Decision, etc.) |
| `neuroplastic.rs` | Salience scoring, tiered memory decay |
| `persistent_memory.rs` | Persistent markdown memory file — read/seed/update, 4KB cap |
| `world_model.rs` | Structured belief system: Belief, BeliefDomain, Confidence, ModelUpdate types |

## Plan-Driven Execution Flow

1. **Observe** — record snapshot, sync auto-beliefs from snapshot (ground truth)
2. **Read Nudges** — fetch unprocessed nudges (user messages, stagnation signals)
3. **Stagnation Check** — circuit breakers: abandon goals after 2 retries, reset all after 30 idle cycles
4. **Get/Create Plan** — check DB for active plan; if none, call LLM to create one (nudges + diagnostics in context)
5. **Execute Step** — run next step via PlanExecutor (mechanical or LLM-assisted)
6. **Handle Result** — advance plan on success, replan on failure (3 retries max), complete plan when all steps done
7. **Housekeeping** — increment cycle count, decay/promote thoughts (every 10 cycles), lifecycle pruning (every 10 cycles), consolidate memory (every 40 cycles, deletes source thoughts)

### Pacing
- Mechanical step → 30s (fast, keep progressing)
- LLM step → 120s
- Plan completed → 300s (time to create next plan)
- No goals → 600s

### PlanStep Types (12 variants)
**Mechanical (no LLM):** ReadFile, SearchCode, ListDir, RunShell, Commit, CheckSelf, CreateScriptEndpoint, TestScriptEndpoint, CargoCheck
**LLM-assisted:** GenerateCode, EditCode, Think

Each step can have `store_as` to accumulate results in plan context. LLM steps reference context via `context_keys`.

## Safety Layers (7 deep)

1. **Rust guard** — hardcoded protected file list in `guard.rs`
2. **Shell heuristic** — guard checks on write/edit tool args
3. **System prompt** — instructs LLM to use file tools, not shell for file ops
4. **Pre-commit validation** — `cargo check` + `cargo test` before any commit
5. **Branch isolation** — changes on `vm/<instance-id>`, never on `main`
6. **Rollback** — `reset_to_last_good()` on health check failure
7. **Human gate** — cross-pollination to main requires PR review

## Non-Obvious Patterns

- Separate SQLite DB (`soul.db`) — does NOT share the gateway DB
- DB schema: PRAGMA user_version based (v1: neuroplastic, v2: beliefs, v3: goals, v4: plans, v5: nudges, v6: chat_sessions + chat_messages, v7: plan_outcomes + capability_events, **v8: benchmark_runs**)
- Plans table: id, goal_id, steps (JSON), current_step, status, context (JSON), replan_count
- Plan context accumulates step results (store_as keys) for use by later steps
- Replan limit: 3 attempts before plan is marked Failed
- LLM called only for: goal creation, plan creation, code steps (GenerateCode/EditCode), reflection, replanning
- Mechanical steps use ToolExecutor directly — no LLM overhead
- Mechanical step batching: consecutive non-LLM steps execute in a single cycle (no 30s gaps between reads)
- Compile-fix loop: after GenerateCode/EditCode, runs cargo check and feeds errors back to LLM for up to 3 fix attempts
- Code step tools: read_file, write_file, edit_file, list_directory, search_files, execute_shell, commit_changes (budget: 20 calls)
- Goal deduplication: new goals with similar descriptions to existing active goals are silently skipped
- Script endpoints: bash scripts at `/data/endpoints/{slug}.sh` served at `/x/{slug}` — instant, no compilation
- Runtime tools in scripts: bash, jq, python3, curl, bc, git, date, sed, awk, grep
- Dormant mode: without `GEMINI_API_KEY`, still observes and records, skips LLM calls
- Gemini 3+ requires `thoughtSignature` passback on function calls — handled in `llm.rs`
- Fork workflow: `SOUL_FORK_REPO` + `SOUL_UPSTREAM_REPO` enable push-to-fork + cross-fork PRs
- Direct push mode: `SOUL_DIRECT_PUSH=true` pushes to fork's main branch directly
- World model: structured beliefs (auto-synced from snapshot each cycle + LLM updates)
- Neuroplastic memory: salience scoring, tiered decay
- Nudge queue: external signals (user, system, stagnation) prioritized into goal/plan creation
- Stagnation detection: per-goal retry limit (2), global idle limit (30 cycles without commit)
- **Chat sessions**: multi-turn conversation history via `chat_sessions` + `chat_messages` tables (replaces stateless per-request chat)
- **Plan approval gate**: `PendingApproval` status — plans pause for human approval when `SOUL_REQUIRE_PLAN_APPROVAL=true`, auto-approve after timeout
- **Plan context injection**: active plan progress, pending approvals, and active goals injected into every chat conversation
- **Plan control tools**: `approve_plan`, `reject_plan`, `request_plan` — available in Chat and Code modes, LLM handles intent naturally
- **Feedback loop**: every completed/failed plan records a structured PlanOutcome (error classification, lesson extraction); lessons are fed back into goal creation + planning prompts via `consult_experience()`
- **Capability tracking**: every step success/failure records a CapabilityEvent; profiles (success rate per skill) are computed and injected into prompts so the agent knows its strengths/weaknesses
- **11 tracked capabilities**: FileRead, FileWrite, CodeCompile, TestPass, ShellExec, PeerCall, EndpointCreate, GitOps, CodeGen, CodeSearch, PlanComplete
- **HumanEval benchmark**: 164 Python coding problems from OpenAI, fetched from HuggingFace datasets API, solved by agent's LLM, validated by running Python tests, scored as pass@1
- **Benchmark schedule**: runs every 100 cycles (configurable), samples 20 problems per session, 6-hour cooldown between sessions
- **ELO rating**: derived from pass@1 scores, compared against published baselines (GPT-4: 67%, Claude 3.5 Sonnet: 92%, Gemini 1.5 Flash: 71.5%)
- **Benchmark in prompts**: pass@1 score + ELO + comparison to baselines injected into goal creation prompts so agent knows its intelligence level
- **Neural brain**: from-scratch 2-layer feedforward net (~50K params, INPUT_SIZE=32, HIDDEN_SIZE=128, OUTPUT_SIZE=23) trained via online SGD on experience data
- **Brain training**: every 10 cycles, collects plan outcomes + capability events → training examples → SGD update → save weights to soul_state
- **Brain prediction**: before each step, predicts success probability + likely error category — logged for observability
- **Brain outputs**: success probability (sigmoid), error category (11-class softmax), per-capability confidence (11 sigmoids)
- **Weight sharing**: `GET /soul/brain/weights` exports, `POST /soul/brain/merge` imports weight deltas — federated averaging across peers
- **Xavier initialization**: deterministic LCG PRNG, no external random crate needed
- **Computer use**: screenshot via scrot/import, mouse/keyboard via xdotool, browser via xdg-open — requires DISPLAY env var
- **Computer use plan steps**: Screenshot, ScreenClick, ScreenType, BrowseUrl — all mechanical (no LLM)
- **Virtual framebuffer**: `ensure_display()` starts Xvfb on :99 if no DISPLAY set
- First-boot seed: 2 concrete starter goals injected when DB has zero goals ever (avoids LLM hallucination from zero context)
- Housekeeping: thought decay, promotion, belief decay, lifecycle pruning (every 10 cycles), memory consolidation with source deletion (every 40 cycles)
- **Lifecycle pruning**: thoughts capped at 500, goals pruned after 7d (keep 10), plans after 3d (keep 10), mutations capped at 50, nudges after 24h, inactive beliefs after 3d, chat messages capped at 100/session
- **Goal deduplication**: Jaccard word similarity (40% threshold) + retread detection (50% similarity with abandoned goals)
- **Active goal cap**: 3 max — prevents goal sprawl
- **Reflection guardrails**: goal UUID included in prompt, max 1 follow-up goal, no cascading "fix" goals
- **Used by**: `x402-node` stores `Arc<SoulDatabase>` in `NodeState`, exposes via `GET /soul/status` (includes plan info + pending plan + capability_profile + plan_outcomes + brain status), `POST /soul/nudge`, `GET /soul/nudges`, `GET /soul/chat/sessions`, `GET /soul/chat/sessions/{id}`, `POST /soul/plan/approve`, `POST /soul/plan/reject`, `GET /soul/plan/pending`, `GET /soul/brain/weights`, `POST /soul/brain/merge`

## Env Vars

| Var | Default | Purpose |
|-----|---------|---------|
| `SOUL_MAX_PLAN_STEPS` | `20` | Max steps in a plan |
| `SOUL_TOOLS_ENABLED` | `true` | Enable/disable tool execution |
| `SOUL_MAX_TOOL_CALLS` | `15` | Max tool calls per LLM step |
| `SOUL_TOOL_TIMEOUT_SECS` | `120` | Per-command timeout |
| `SOUL_WORKSPACE_ROOT` | `/data/workspace` | Workspace root for file tools |
| `SOUL_CODING_ENABLED` | `false` | Master switch for write/edit/commit tools |
| `SOUL_AUTONOMOUS_CODING` | `false` | Allow autonomous code changes |
| `SOUL_FORK_REPO` | — | Fork repo for push |
| `SOUL_UPSTREAM_REPO` | — | Upstream repo for PRs/issues |
| `SOUL_DIRECT_PUSH` | `false` | Push directly to fork's main branch |
| `SOUL_MEMORY_FILE` | `/data/soul_memory.md` | Path to persistent memory file |
| `GATEWAY_URL` | — | Gateway URL for `check_self`/`register_endpoint` tools |
| `SOUL_REQUIRE_PLAN_APPROVAL` | `false` | Pause plans for human approval before execution |
| `SOUL_PLAN_APPROVAL_TIMEOUT` | `30` | Minutes before auto-approving a pending plan |
| `SOUL_NEUROPLASTIC` | `true` | Enable neuroplastic memory |
| `SOUL_PRUNE_THRESHOLD` | `0.01` | Strength threshold for thought pruning |

## If You're Changing...

- **Plan execution**: `plan.rs` — step types, PlanExecutor, step dispatch
- **Thinking loop**: `thinking/` — mod.rs has run(), submodules: plan_cycle, observe, goals, planning, completion, housekeeping, tool_loop
- **Prompts**: `prompts.rs` — 5 focused builders (goal_creation, planning, code_generation, replan, reflection); accepts experience + capability data
- **Feedback loop**: `feedback.rs` — PlanOutcome recording, error classification, lesson extraction, experience consultation
- **Capability tracking**: `capability.rs` — per-step event recording, profile computation, prompt guidance
- **HumanEval benchmark**: `benchmark.rs` — fetch from HuggingFace, LLM solve, Python test validation, pass@1 scoring, reference comparison
- **ELO rating**: `elo.rs` — pass@1 → ELO mapping, history tracking, reference model comparison
- **Neural brain**: `brain.rs` — feedforward net, SGD training, weight serialization, feature encoding, distributed weight deltas
- **Computer use**: `computer_use.rs` — screenshot capture, mouse/keyboard simulation, display management
- **Tool execution**: `tools.rs` — tool definitions, executor, all tool implementations (includes computer use + brain tools)
- **Protected files**: `guard.rs` — hardcoded list, do NOT make configurable via env
- **Git workflow**: `git.rs` — branch ops, auth, PR creation
- **Pre-commit validation**: `coding.rs` — cargo check/test pipeline
- **Database schema**: `db.rs` — plans table (v4), nudges table (v5), chat sessions/messages (v6), CRUD methods
- **Chat sessions**: `chat.rs` — session-based conversation, plan context builder
- **Plan approval**: `thinking/plan_cycle.rs` — approval gate; `plan.rs` — PendingApproval status; `tools/planning.rs` — approve/reject/request tools
- **World model**: `world_model.rs` — belief types + formatters
- **Observer trait**: `observer.rs` — changing `NodeSnapshot` fields affects all implementors
