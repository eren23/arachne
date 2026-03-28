#!/usr/bin/env bash
#
# Build script for the Arachne WASM module.
#
# Produces:
#   web/pkg/arachne_wasm_bg.wasm  -- the optimized WASM binary
#   web/pkg/arachne_wasm.js       -- the JS bindings (ES module)
#   web/pkg/arachne_wasm.d.ts     -- TypeScript declarations
#
# Prerequisites:
#   rustup target add wasm32-unknown-unknown
#   cargo install wasm-bindgen-cli
#   (optional) cargo install wasm-opt   OR   brew install binaryen
#
# Usage:
#   cd web && bash build.sh
#   # or from project root:
#   bash web/build.sh

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PROJECT_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
OUT_DIR="$SCRIPT_DIR/pkg"

echo "=== Arachne WASM Build ==="
echo "Project root: $PROJECT_ROOT"
echo "Output dir:   $OUT_DIR"
echo ""

# -----------------------------------------------------------------------
# Step 1: Build the WASM target
# -----------------------------------------------------------------------
echo "[1/3] Building arachne-wasm for wasm32-unknown-unknown (release)..."
cargo build \
    --manifest-path "$PROJECT_ROOT/Cargo.toml" \
    -p arachne-wasm \
    --target wasm32-unknown-unknown \
    --release \
    --features wasm

WASM_FILE="$PROJECT_ROOT/target/wasm32-unknown-unknown/release/arachne_wasm.wasm"

if [ ! -f "$WASM_FILE" ]; then
    echo "ERROR: WASM binary not found at $WASM_FILE"
    exit 1
fi

echo "  WASM binary: $(du -h "$WASM_FILE" | cut -f1)"

# -----------------------------------------------------------------------
# Step 2: Generate JS bindings with wasm-bindgen
# -----------------------------------------------------------------------
echo "[2/3] Running wasm-bindgen..."
mkdir -p "$OUT_DIR"

wasm-bindgen \
    "$WASM_FILE" \
    --out-dir "$OUT_DIR" \
    --target web \
    --no-typescript || {
        echo "WARNING: wasm-bindgen failed. Install with: cargo install wasm-bindgen-cli"
        echo "Copying raw WASM file instead."
        cp "$WASM_FILE" "$OUT_DIR/arachne_wasm_bg.wasm"
    }

# -----------------------------------------------------------------------
# Step 3: Optimize with wasm-opt (optional)
# -----------------------------------------------------------------------
WASM_OPT_INPUT="$OUT_DIR/arachne_wasm_bg.wasm"

if [ -f "$WASM_OPT_INPUT" ]; then
    if command -v wasm-opt &>/dev/null; then
        echo "[3/3] Optimizing with wasm-opt -Oz..."
        BEFORE_SIZE=$(du -h "$WASM_OPT_INPUT" | cut -f1)
        wasm-opt -Oz "$WASM_OPT_INPUT" -o "$WASM_OPT_INPUT"
        AFTER_SIZE=$(du -h "$WASM_OPT_INPUT" | cut -f1)
        echo "  Before: $BEFORE_SIZE -> After: $AFTER_SIZE"
    else
        echo "[3/3] wasm-opt not found, skipping optimization."
        echo "  Install binaryen for smaller output: brew install binaryen"
    fi
else
    echo "[3/3] No WASM file to optimize."
fi

# -----------------------------------------------------------------------
# Summary
# -----------------------------------------------------------------------
echo ""
echo "=== Build Complete ==="
echo "Output files:"
if [ -d "$OUT_DIR" ]; then
    ls -lh "$OUT_DIR"/ 2>/dev/null || echo "  (empty)"
fi
echo ""
echo "To test locally:"
echo "  cd $SCRIPT_DIR"
echo "  python3 -m http.server 8080"
echo "  # Then open http://localhost:8080/index.html"
