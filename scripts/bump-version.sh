#!/bin/bash
# Bump version across ALL files at once
# Usage: ./scripts/bump-version.sh 0.2.1

set -e

if [ -z "$1" ]; then
    echo "Usage: $0 <new-version>"
    echo "Example: $0 0.2.1"
    exit 1
fi

NEW_VERSION="$1"
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(dirname "$SCRIPT_DIR")"

# Validate version format
if ! echo "$NEW_VERSION" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
    echo "ERROR: Invalid version format. Use semantic versioning (e.g., 0.2.1)"
    exit 1
fi

echo "Bumping version to: $NEW_VERSION"
echo ""

# Get current version
OLD_VERSION=$(grep -m1 '^version = ' "$ROOT_DIR/Cargo.toml" | sed 's/version = "\(.*\)"/\1/')
echo "Current version: $OLD_VERSION"
echo ""

# Update workspace Cargo.toml
sed -i'' -e "s/^version = \"$OLD_VERSION\"/version = \"$NEW_VERSION\"/" "$ROOT_DIR/Cargo.toml"
echo "✓ Updated Cargo.toml"

# Update each crate's Cargo.toml
CRATES=("tempo-x402" "tempo-x402-server" "tempo-x402-facilitator" "tempo-x402-gateway")
for crate in "${CRATES[@]}"; do
    CRATE_TOML="$ROOT_DIR/crates/$crate/Cargo.toml"
    if [ -f "$CRATE_TOML" ]; then
        sed -i'' -e "s/^version = \"$OLD_VERSION\"/version = \"$NEW_VERSION\"/" "$CRATE_TOML"
        echo "✓ Updated crates/$crate/Cargo.toml"
    fi
done

# Update inter-crate dependencies (workspace deps use workspace = true, but just in case)
# This handles any hardcoded version references
for crate in "${CRATES[@]}"; do
    CRATE_TOML="$ROOT_DIR/crates/$crate/Cargo.toml"
    if [ -f "$CRATE_TOML" ]; then
        # Update tempo-x402 = "X.Y" style dependencies
        sed -i'' -e "s/tempo-x402 = \"$OLD_VERSION\"/tempo-x402 = \"$NEW_VERSION\"/g" "$CRATE_TOML"
        sed -i'' -e "s/tempo-x402-server = \"$OLD_VERSION\"/tempo-x402-server = \"$NEW_VERSION\"/g" "$CRATE_TOML"
        sed -i'' -e "s/tempo-x402-facilitator = \"$OLD_VERSION\"/tempo-x402-facilitator = \"$NEW_VERSION\"/g" "$CRATE_TOML"
        sed -i'' -e "s/tempo-x402-gateway = \"$OLD_VERSION\"/tempo-x402-gateway = \"$NEW_VERSION\"/g" "$CRATE_TOML"
    fi
done

# Update llms.txt version at the bottom
if [ -f "$ROOT_DIR/llms.txt" ]; then
    sed -i'' -e "s/^$OLD_VERSION$/$NEW_VERSION/" "$ROOT_DIR/llms.txt"
    echo "✓ Updated llms.txt"
fi

echo ""
echo "================================"
echo "Version bumped: $OLD_VERSION -> $NEW_VERSION"
echo "================================"
echo ""
echo "Next steps:"
echo "  1. Review changes: git diff"
echo "  2. Commit: git add -A && git commit -m 'Bump version to $NEW_VERSION'"
echo "  3. Tag: git tag v$NEW_VERSION"
echo "  4. Push: git push && git push --tags"
echo "  5. Publish: cargo publish -p tempo-x402 && cargo publish -p tempo-x402-server && ..."
