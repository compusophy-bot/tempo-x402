#!/bin/bash
# Verify all cartridge training examples compile.
# Usage: ./verify_all.sh cartridges/
# Reads each .rs file, copies to verify_build/src/lib.rs, compiles.

set -e

DIR="${1:-.}"
SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="$SCRIPT_DIR/verify_build"
TARGET_DIR="/tmp/verify-cartridge-target"

PASS=0
FAIL=0
TOTAL=0

for rs_file in "$DIR"/*.rs; do
    [ -f "$rs_file" ] || continue
    TOTAL=$((TOTAL + 1))
    name=$(basename "$rs_file" .rs)

    cp "$rs_file" "$BUILD_DIR/src/lib.rs"
    if cargo build --manifest-path "$BUILD_DIR/Cargo.toml" \
        --target wasm32-unknown-unknown \
        --release \
        --target-dir "$TARGET_DIR" 2>/dev/null; then
        PASS=$((PASS + 1))
    else
        echo "FAIL: $name"
        FAIL=$((FAIL + 1))
    fi
done

echo ""
echo "Results: $PASS/$TOTAL passed, $FAIL failed"
