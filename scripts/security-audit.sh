#!/usr/bin/env bash
set -euo pipefail

echo "=== tempo-x402 Security Audit ==="
echo ""

PASS=0
FAIL=0

run_check() {
    local name="$1"
    shift
    echo "[*] $name..."
    if "$@" 2>&1; then
        echo "    PASS"
        PASS=$((PASS + 1))
    else
        echo "    FAIL"
        FAIL=$((FAIL + 1))
    fi
    echo ""
}

echo "[1/5] Dependency vulnerability scan..."
if command -v cargo-audit >/dev/null 2>&1; then
    run_check "cargo audit" cargo audit
else
    echo "    SKIP (install with: cargo install cargo-audit)"
    echo ""
fi

echo "[2/5] Dependency policy check..."
if command -v cargo-deny >/dev/null 2>&1; then
    run_check "cargo deny advisories" cargo deny check advisories
    run_check "cargo deny bans" cargo deny check bans
else
    echo "    SKIP (install with: cargo install cargo-deny)"
    echo ""
fi

echo "[3/5] Security invariant tests..."
run_check "security invariants" cargo test -p tempo-x402-security-audit

echo "[4/5] Checking for unsafe code..."
UNSAFE_FOUND=$(grep -rn "unsafe " crates/*/src/*.rs crates/*/src/**/*.rs 2>/dev/null | grep -v "// SAFETY:" || true)
if [ -n "$UNSAFE_FOUND" ]; then
    echo "    WARNING: Unsafe code found without SAFETY comment:"
    echo "$UNSAFE_FOUND"
else
    echo "    No undocumented unsafe code found"
fi
echo ""

echo "[5/5] Scanning for hardcoded secrets..."
FOUND=$(grep -rn '0x[a-fA-F0-9]\{64\}' crates/*/src/*.rs 2>/dev/null \
    | grep -v DEMO_PRIVATE_KEY \
    | grep -v SECP256K1_N_DIV_2 \
    | grep -v 'mod tests' \
    | grep -v '#\[cfg(test)\]' \
    | grep -v '#\[cfg(feature' || true)
if [ -n "$FOUND" ]; then
    echo "    WARNING: Potential secrets found:"
    echo "$FOUND"
    FAIL=$((FAIL + 1))
else
    echo "    No hardcoded secrets found"
    PASS=$((PASS + 1))
fi
echo ""

echo "=== Security Audit Complete ==="
echo "Passed: $PASS  Failed: $FAIL"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
