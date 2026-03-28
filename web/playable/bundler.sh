#!/usr/bin/env bash
#
# Arachne Playable Ad Bundler
#
# Produces a single self-contained HTML file with WASM inlined as base64,
# ready to upload to ad networks (Google Ads, Facebook, Unity Ads, etc.).
#
# Usage:
#   bash bundler.sh templates/physics-toy.html
#   bash bundler.sh templates/catch-game.html
#
# Output:
#   dist/<name>.html       — single-file (WASM as base64, all JS inlined)
#   dist/<name>.zip        — multi-file ZIP (separate WASM + JS)
#   dist/size-report.txt   — size breakdown and network limit checks

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
PKG_DIR="$SCRIPT_DIR/../pkg"
DIST_DIR="$SCRIPT_DIR/dist"

WASM_FILE="$PKG_DIR/arachne_wasm_bg.wasm"
JS_FILE="$PKG_DIR/arachne_wasm.js"
MRAID_FILE="$SCRIPT_DIR/mraid-shim.js"
SDK_FILE="$SCRIPT_DIR/arachne-playable.js"

# --- Argument parsing ---
if [ $# -lt 1 ]; then
    echo "Usage: bash bundler.sh <template.html>"
    echo "Example: bash bundler.sh templates/physics-toy.html"
    exit 1
fi

TEMPLATE="$SCRIPT_DIR/$1"
NAME=$(basename "$1" .html)

if [ ! -f "$TEMPLATE" ]; then
    echo "ERROR: Template not found: $TEMPLATE"
    exit 1
fi

# --- Check prerequisites ---
for f in "$WASM_FILE" "$JS_FILE" "$MRAID_FILE" "$SDK_FILE"; do
    if [ ! -f "$f" ]; then
        echo "ERROR: Required file not found: $f"
        echo "Run 'bash web/build.sh' first to build the WASM module."
        exit 1
    fi
done

mkdir -p "$DIST_DIR"

echo "=== Arachne Playable Ad Bundler ==="
echo "Template: $NAME"
echo ""

# --- Step 1: Read source files ---
WASM_B64=$(base64 < "$WASM_FILE" | tr -d '\n')
JS_CONTENT=$(cat "$JS_FILE")
MRAID_CONTENT=$(cat "$MRAID_FILE")
SDK_CONTENT=$(cat "$SDK_FILE")

# --- Step 2: Minify JS if terser is available ---
if command -v terser &>/dev/null; then
    echo "[minify] Using terser for JS minification..."
    JS_CONTENT=$(echo "$JS_CONTENT" | terser --compress --mangle 2>/dev/null || echo "$JS_CONTENT")
    MRAID_CONTENT=$(echo "$MRAID_CONTENT" | terser --compress --mangle 2>/dev/null || echo "$MRAID_CONTENT")
    SDK_CONTENT=$(echo "$SDK_CONTENT" | terser --compress --mangle 2>/dev/null || echo "$SDK_CONTENT")
else
    echo "[minify] terser not found, skipping minification."
    echo "  Install for smaller output: npm install -g terser"
fi

# --- Step 3: Determine which run function the template uses ---
TEMPLATE_CONTENT=$(cat "$TEMPLATE")
if echo "$TEMPLATE_CONTENT" | grep -q "run_catch"; then
    RUN_FN="run_catch"
    CTA_TEXT="Beat the High Score!"
    END_DELAY=20
    MAX_INTERACT=15
else
    RUN_FN="run"
    CTA_TEXT="Play Now!"
    END_DELAY=15
    MAX_INTERACT=10
fi

echo "[config] Run function: $RUN_FN"

# --- Step 4: Build single-file HTML ---
# Write the static header portion
cat > "$DIST_DIR/$NAME.html" << 'HTMLEOF'
<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8">
<meta name="viewport" content="width=device-width,initial-scale=1,maximum-scale=1,user-scalable=no">
<style>
*{margin:0;padding:0;box-sizing:border-box}
html,body{width:100%;height:100%;overflow:hidden;background:#000}
#ad-container{width:100%;height:100%;position:relative}
#arachne-canvas{display:block;width:100%;height:100%;touch-action:none;-webkit-touch-callout:none;user-select:none}
</style>
</head>
<body>
<div id="ad-container">
<canvas id="arachne-canvas" width="320" height="480"></canvas>
</div>
HTMLEOF

# Inline MRAID shim
{
    echo "<script>"
    echo "$MRAID_CONTENT"
    echo "</script>"
} >> "$DIST_DIR/$NAME.html"

# Inline SDK
{
    echo "<script>"
    echo "$SDK_CONTENT"
    echo "</script>"
} >> "$DIST_DIR/$NAME.html"

# Inline the WASM module JS as a module script with base64 WASM loading
# The variables RUN_FN, CTA_TEXT, END_DELAY, MAX_INTERACT are shell vars expanded here.
# WASM_B64 is also expanded. The JS code inside uses $1/$2/$3 only inside check_limit
# which is written later in single-quoted context.
{
    echo '<script type="module">'
    echo "// --- Base64 WASM loader ---"
    printf 'const _WASM_B64 = "%s";\n' "$WASM_B64"
    cat << 'B64FUNC'
function _b64decode(b64) {
  const bin = atob(b64);
  const bytes = new Uint8Array(bin.length);
  for (let i = 0; i < bin.length; i++) bytes[i] = bin.charCodeAt(i);
  return bytes.buffer;
}
B64FUNC
    echo ""
    echo "// --- Inlined arachne_wasm.js ---"
    echo "$JS_CONTENT"
    echo ""
    echo "// --- Game startup ---"
    printf 'const ad = new ArachnePlayable({\n'
    printf '  container: '"'"'#ad-container'"'"',\n'
    printf '  canvasId: '"'"'arachne-canvas'"'"',\n'
    printf '  width: 320, height: 480,\n'
    printf '  storeUrl: '"'"'https://apps.apple.com/example'"'"',\n'
    printf '  ctaText: '"'"'%s'"'"',\n' "$CTA_TEXT"
    printf '  endCardDelay: %d,\n' "$END_DELAY"
    printf '  maxInteractions: %d,\n' "$MAX_INTERACT"
    printf '});\n'
    printf 'ad.start(async (canvasId) => {\n'
    printf '  const wasmBytes = _b64decode(_WASM_B64);\n'
    printf '  await __wbg_init(wasmBytes);\n'
    printf '  %s(canvasId);\n' "$RUN_FN"
    printf '});\n'
    echo "</script>"
    echo "</body>"
    echo "</html>"
} >> "$DIST_DIR/$NAME.html"

echo "[build] Single-file: $DIST_DIR/$NAME.html"

# --- Step 5: Build ZIP variant ---
ZIP_STAGE="$DIST_DIR/_zip_$NAME"
rm -rf "$ZIP_STAGE"
mkdir -p "$ZIP_STAGE/pkg"

cp "$WASM_FILE" "$ZIP_STAGE/pkg/"
cp "$JS_FILE" "$ZIP_STAGE/pkg/"
cp "$MRAID_FILE" "$ZIP_STAGE/"
cp "$SDK_FILE" "$ZIP_STAGE/"
cp "$TEMPLATE" "$ZIP_STAGE/index.html"

# Fix paths in the ZIP copy (templates reference ../../pkg, zip has ./pkg)
sed -i.bak 's|../../pkg/|./pkg/|g' "$ZIP_STAGE/index.html"
sed -i.bak 's|\.\./mraid-shim\.js|./mraid-shim.js|g' "$ZIP_STAGE/index.html"
sed -i.bak 's|\.\./arachne-playable\.js|./arachne-playable.js|g' "$ZIP_STAGE/index.html"
rm -f "$ZIP_STAGE/index.html.bak"

(cd "$ZIP_STAGE" && zip -r "$DIST_DIR/$NAME.zip" . -x '*.bak')
rm -rf "$ZIP_STAGE"

echo "[build] ZIP: $DIST_DIR/$NAME.zip"

# --- Step 6: Size report ---
SINGLE_SIZE=$(wc -c < "$DIST_DIR/$NAME.html" | tr -d ' ')
ZIP_SIZE=$(wc -c < "$DIST_DIR/$NAME.zip" | tr -d ' ')
WASM_SIZE=$(wc -c < "$WASM_FILE" | tr -d ' ')
JS_SIZE=$(wc -c < "$JS_FILE" | tr -d ' ')
MRAID_SIZE=$(wc -c < "$MRAID_FILE" | tr -d ' ')
SDK_SIZE=$(wc -c < "$SDK_FILE" | tr -d ' ')

REPORT="$DIST_DIR/size-report.txt"

{
    echo "=== Arachne Playable Ad Size Report ==="
    echo "Template: $NAME"
    printf "Date: %s\n" "$(date -u +%Y-%m-%d)"
    echo ""
    echo "Component Breakdown:"
    printf "  WASM binary:              %9d bytes\n" "$WASM_SIZE"
    printf "  arachne_wasm.js:          %9d bytes\n" "$JS_SIZE"
    printf "  mraid-shim.js:            %9d bytes\n" "$MRAID_SIZE"
    printf "  arachne-playable.js:      %9d bytes\n" "$SDK_SIZE"
    echo "──────────────────────────────────────────"
    printf "  Single HTML (base64):     %9d bytes\n" "$SINGLE_SIZE"
    printf "  ZIP (multi-file):         %9d bytes\n" "$ZIP_SIZE"
    echo ""
    echo "Network Limit Checks:"
} > "$REPORT"

check_limit() {
    local name=$1 size=$2 limit=$3
    if [ "$size" -le "$limit" ]; then
        printf "  %s: PASS (%d / %d bytes)\n" "$name" "$size" "$limit" >> "$REPORT"
    else
        printf "  %s: FAIL (%d / %d bytes) *** OVER LIMIT ***\n" "$name" "$size" "$limit" >> "$REPORT"
    fi
}

check_limit "Google Ads (600KB single)" "$SINGLE_SIZE" 614400
check_limit "Google Ads (600KB ZIP)"    "$ZIP_SIZE"    614400
check_limit "Facebook (2MB)"            "$ZIP_SIZE"    2097152
check_limit "Unity Ads (5MB)"           "$ZIP_SIZE"    5242880

cat "$REPORT"
echo ""
echo "=== Done ==="
