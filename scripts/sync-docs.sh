#!/bin/bash
# Sync version numbers across all docs and Cargo.toml files
# Run this before releases or let CI catch drift

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

# Get version from core crate (single source of truth)
VERSION=$(grep -m1 '^version = ' "$ROOT_DIR/crates/tempo-x402/Cargo.toml" | sed 's/version = "\(.*\)"/\1/')

if [ -z "$VERSION" ]; then
    echo "ERROR: Could not extract version from Cargo.toml"
    exit 1
fi

echo "Syncing docs to version: $VERSION"

# Update llms.txt version line at the bottom
if grep -q "^## Version" "$ROOT_DIR/llms.txt"; then
    # Replace the line after "## Version"
    sed -i'' -e "/^## Version$/,/^[0-9]/{s/^[0-9].*/$VERSION/}" "$ROOT_DIR/llms.txt"
    echo "✓ Updated llms.txt"
else
    echo "WARNING: No ## Version section in llms.txt"
fi

# Verify all crate versions match core version
CRATES=("tempo-x402" "tempo-x402-server" "tempo-x402-facilitator" "tempo-x402-gateway")
for crate in "${CRATES[@]}"; do
    CRATE_TOML="$ROOT_DIR/crates/$crate/Cargo.toml"
    if [ -f "$CRATE_TOML" ]; then
        CRATE_VERSION=$(grep -m1 '^version = ' "$CRATE_TOML" | sed 's/version = "\(.*\)"/\1/')
        if [ "$CRATE_VERSION" != "$VERSION" ]; then
            echo "ERROR: $crate version ($CRATE_VERSION) != core version ($VERSION)"
            exit 1
        fi
        echo "✓ $crate version matches"
    fi
done

# Check deployment URLs are mentioned in llms.txt
URLS=(
    "https://tempo-x402-demo.vercel.app"
    "https://x402-server-production.up.railway.app"
    "https://x402-facilitator-production-ec87.up.railway.app"
    "https://x402-gateway-production-5018.up.railway.app"
)
for url in "${URLS[@]}"; do
    if ! grep -q "$url" "$ROOT_DIR/llms.txt"; then
        echo "WARNING: $url not found in llms.txt"
    fi
done

echo ""
echo "Docs synced to v$VERSION"
