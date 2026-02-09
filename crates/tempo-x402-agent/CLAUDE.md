# tempo-x402-agent

Library crate. Railway API client and clone orchestration for self-replicating nodes.

8-step clone workflow: create service → set env vars → set Docker image → add volume → create domain → deploy. All via Railway GraphQL API.

## Depends On

- `x402-identity` (InstanceIdentity type for lineage)
- `x402` (core types)

## Non-Obvious Patterns

- Railway GraphQL at `https://backboard.railway.app/graphql/v2`, bearer token auth
- Each Railway operation is a separate GraphQL mutation string in `railway.rs`
- Child gets `AUTO_BOOTSTRAP=true` + parent URL/address for callback registration
- HTTP client: 30s timeout, no redirects

## If You're Changing...

- **Child env vars**: `clone_instance()` in `clone.rs` — the VariableCollectionUpsert step
- **Railway API calls**: GraphQL strings in `railway.rs` — follow existing mutation pattern
- **Used by**: `x402-node` creates `CloneOrchestrator` at startup if Railway creds are configured
