# tempo-x402-soul

Library crate. Agentic "soul" for x402 nodes: a periodic observe-think-record loop powered by Gemini with full coding agent capabilities.

Observes node state via `NodeObserver` trait, reasons via Gemini API, can read/write/edit files and execute shell commands via Gemini function calling, records thoughts and mutations to SQLite. Operates on `vm/<instance-id>` branches with pre-commit validation. Runs dormant (observe-only) without a Gemini API key.

## Depends On

- `x402` (core types only)

No dependency on gateway/identity/agent/node. Communicates via `NodeObserver` trait — the node crate implements it.

## Module Overview

| Module | Purpose |
|--------|---------|
| `guard.rs` | Hardcoded protected file list — prevents self-bricking |
| `tools.rs` | Tool executor: shell, file read/write/edit, search, commit, PR + dynamic tool dispatch |
| `tool_registry.rs` | Dynamic tool registry: register/list/unregister tools at runtime, shell execution |
| `git.rs` | Branch-per-VM git workflow (ensure_branch, commit, push, PR, issues) with fork support |
| `coding.rs` | Pre-commit validation pipeline (cargo check → test → commit) |
| `mode.rs` | Agent modes (Observe, Chat, Code, Review) with per-mode tool sets |
| `prompts.rs` | System prompts per mode |
| `llm.rs` | Gemini API client with thought_signature support |
| `thinking.rs` | The main observe → think → tool loop |
| `chat.rs` | Interactive chat handler with mode detection |
| `db.rs` | SQLite: thoughts, soul_state, mutations, tools tables |
| `memory.rs` | Thought types (Observation, Reasoning, Decision, Mutation, etc.) |

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
- On Railway, `SOUL_DB_PATH` must point to persistent volume (`/data/soul.db`)
- Dormant mode: without `GEMINI_API_KEY`, still observes and records, skips LLM calls
- Default model: `gemini-3-flash-preview` (configurable via `GEMINI_MODEL_FAST` env var)
- Gemini 3+ requires `thoughtSignature` passback on function calls — handled in `llm.rs`
- Tool output truncated to 16KB per stream to stay within Gemini context limits
- Tools disabled via `SOUL_TOOLS_ENABLED=false`
- Coding disabled by default — requires `SOUL_CODING_ENABLED=true` + `INSTANCE_ID`
- Protected paths are hardcoded (not env-var) so the soul cannot bypass via shell
- Dynamic tool registry: `SOUL_DYNAMIC_TOOLS_ENABLED=false` by default, max 20 tools, meta-tools only in Code mode
- Dynamic tools execute via shell with `TOOL_ARGS` JSON + `TOOL_PARAM_{NAME}` env vars, respects existing timeouts
- `tool_registry.rs` is in PROTECTED_PREFIXES — soul cannot modify its own tool registry code
- Fork workflow: `SOUL_FORK_REPO` + `SOUL_UPSTREAM_REPO` enable push-to-fork + cross-fork PRs + issue creation
- Fork remote named "fork" is auto-configured on first push; origin stays as upstream reference
- Direct push mode (`SOUL_DIRECT_PUSH=true`): pushes to fork's main branch directly, triggering auto-deploy. Used for self-editing instances.
- Deep model: `SOUL_DIRECT_PUSH` + `SOUL_AUTONOMOUS_CODING` together use Gemini Pro (think model) instead of Flash for deeper reasoning

## Env Vars

| Var | Default | Purpose |
|-----|---------|---------|
| `SOUL_TOOLS_ENABLED` | `true` | Enable/disable tool execution |
| `SOUL_MAX_TOOL_CALLS` | `25` | Max tool calls per cycle |
| `SOUL_TOOL_TIMEOUT_SECS` | `120` | Per-command timeout |
| `SOUL_WORKSPACE_ROOT` | `/app` | Workspace root for file tools |
| `SOUL_CODING_ENABLED` | `false` | Master switch for write/edit/commit tools |
| `SOUL_AUTONOMOUS_CODING` | `false` | Allow autonomous code changes in think cycles |
| `SOUL_AUTO_PROPOSE_TO_MAIN` | `false` | Auto-create PRs from vm branch to main |
| `GITHUB_TOKEN` | — | Token for git push/PR operations |
| `INSTANCE_ID` | — | VM instance ID for branch naming |
| `SOUL_DYNAMIC_TOOLS_ENABLED` | `false` | Enable dynamic tool registry (register/list/unregister at runtime) |
| `SOUL_FORK_REPO` | — | Fork repo for push (e.g. `compusophy-bot/tempo-x402`). Pushes go to "fork" remote |
| `SOUL_UPSTREAM_REPO` | — | Upstream repo for PRs/issues (e.g. `compusophy/tempo-x402`) |
| `SOUL_DIRECT_PUSH` | `false` | Push directly to fork's main branch (self-editing mode). Safety: cargo check + test still gate every commit |

## If You're Changing...

- **LLM API**: `llm.rs` — model names, endpoint format, retry logic, thought_signature handling
- **Thinking loop**: `thinking.rs` — observe → think → tool loop → record cycle
- **Tool execution**: `tools.rs` — tool definitions, executor, all tool implementations
- **Protected files**: `guard.rs` — hardcoded list, do NOT make configurable via env
- **Git workflow**: `git.rs` — branch ops, auth, PR creation
- **Pre-commit validation**: `coding.rs` — cargo check/test pipeline
- **Agent modes**: `mode.rs` — mode enum, tool sets per mode, max_tool_calls
- **System prompts**: `prompts.rs` — per-mode prompt templates
- **Dynamic tool registry**: `tool_registry.rs` — meta-tools, dynamic tool execution, shell handlers
- **Database schema**: `db.rs` — `thoughts` + `soul_state` + `mutations` + `tools` tables
- **Observer trait**: `observer.rs` — changing `NodeSnapshot` fields affects all implementors
- **Used by**: `x402-node` stores `Arc<SoulDatabase>` in `NodeState`, exposes via `GET /soul/status`, implements `NodeObserver` in `soul_observer.rs`
