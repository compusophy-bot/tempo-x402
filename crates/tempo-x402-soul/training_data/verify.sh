#!/bin/bash
# Verify a cartridge source file compiles to wasm32-unknown-unknown.
# Usage: ./verify.sh path/to/lib.rs
# Returns 0 on success, 1 on failure.

set -e

SRC="$1"
if [ -z "$SRC" ]; then
    echo "Usage: $0 <lib.rs path>"
    exit 1
fi

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
BUILD_DIR="$SCRIPT_DIR/verify_build"
TARGET_DIR="/tmp/verify-cartridge-target"

cp "$SRC" "$BUILD_DIR/src/lib.rs"
cargo build --manifest-path "$BUILD_DIR/Cargo.toml" \
    --target wasm32-unknown-unknown \
    --release \
    --target-dir "$TARGET_DIR" 2>&1
