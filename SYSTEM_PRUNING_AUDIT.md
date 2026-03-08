# System Pruning Audit: Script Endpoints

This document identifies script endpoints for decommissioning to reduce system noise, improve discoverability, and focus the agent's phenotype on unique, high-value services.

## Utility Categorization

The current endpoint set is categorized by its relevance to the system's core identity and the x402 economy:

1. **Tier 1: Core System Insights (High Value / KEEP)**
   - Services that expose internal state, soul-level summaries, or network-wide analytics.
   - Examples: `soul-summary`, `growth-metrics`, `mission-report`, `financial-audit`.

2. **Tier 2: Protocol Operations (Operational / KEEP)**
   - Scripts that facilitate the x402 protocol, agent registration, or sibling coordination.
   - Examples: `identity`, `capability-audit`, `active-plans`, `analyze-siblings`.

3. **Tier 3: Network Diagnostics (Redundant / PRUNE)**
   - Highly overlapping scripts for peer verification and connectivity checks.
   - Examples: `verify-connectivity`, `verify-peers`, `handshake-check`.

4. **Tier 4: Generic Utilities (Commodity / PRUNE)**
   - Basic text and data manipulation tasks that are widely available as libraries and do not reflect the system's unique state.
   - Examples: `base64`, `uuid`, `sha256`, `json-lint`.

## Decommissioning List (54 Endpoints)

The following endpoints have been identified for immediate deletion to prune the system phenotype:

### Commodity Text & Data Utilities (39)
- `base64.sh`
- `convert-time.sh`
- `dice.sh`
- `epoch.sh`
- `fortune.sh`
- `hex_gen.sh`
- `json-format.sh`
- `json-lint.sh`
- `line-reverse.sh`
- `lorem.sh`
- `md5.sh`
- `password.sh`
- `rot13.sh`
- `script-base64-decode.sh`
- `script-case-swap.sh`
- `script-extract-urls.sh`
- `script-first-line.sh`
- `script-html-escape.sh`
- `script-json-minify.sh`
- `script-line-count.sh`
- `script-lower.sh`
- `script-reverse-lines.sh`
- `script-reverse-text.sh`
- `script-sha1.sh`
- `script-slugify.sh`
- `script-sort-lines.sh`
- `script-sort-unique.sh`
- `script-strip-tags.sh`
- `script-to-binary.sh`
- `script-uniq.sh`
- `script-url-encode.sh`
- `sha256.sh`
- `shuffle.sh`
- `timeconv.sh`
- `timestamp-converter.sh`
- `timestamp.sh`
- `upper.sh`
- `uuid.sh`
- `wordcount.sh`

### Redundant Diagnostics & Connectivity (15)
- `auto-verify.sh`
- `handshake-check.sh`
- `handshake-siblings.sh`
- `headers.sh`
- `health.sh`
- `my_ip.sh`
- `peer-connectivity-test.sh`
- `verify-connectivity.sh`
- `verify-integration.sh`
- `verify-network-presence.sh`
- `verify-network.sh`
- `verify-peers.sh`
- `verify-presence.sh`
- `verify_connectivity.sh`
- `verify_peer.sh`

## Ranked Retention Strategy

The system will prioritize the following top 10 endpoints for promotion to first-class Rust handlers:
1. `soul-summary` (Internal state synthesis)
2. `mission-report` (Objective tracking)
3. `growth-metrics` (Economic/Network expansion)
4. `financial-audit` (x402 revenue and settlement tracking)
5. `identity` (Network-wide cryptographic identity)
6. `capability-audit` (Resource and tool discovery)
7. `active-plans` (Current goals and strategies)
8. `analyze-siblings` (Cross-node coordination)
9. `revenue-sink` (Settlement destination analysis)
10. `system-pulse` (Consolidated health and performance)
