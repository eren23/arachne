# Arachne Playable Ads SDK

Package the Arachne engine (446KB) as playable ads for Google Ads, Facebook, Unity Ads, and other ad networks.

## Quick Start

1. Build the WASM module (if not already built):
   ```bash
   bash web/build.sh
   ```

2. Bundle a template into a single-file ad:
   ```bash
   cd web/playable
   bash bundler.sh templates/physics-toy.html
   bash bundler.sh templates/catch-game.html
   ```

3. Output is in `dist/`:
   - `physics-toy.html` — single self-contained HTML file
   - `physics-toy.zip` — multi-file ZIP for networks that prefer it
   - `size-report.txt` — size breakdown and network limit checks

## Templates

- **physics-toy** — Drag and throw colorful physics bodies. CTA after 15s or 10 interactions.
- **catch-game** — Tap falling objects to score. CTA after 20s or 15 catches.

## Customizing

Edit the template HTML to change:
- `storeUrl` — App Store / Play Store deep link
- `ctaText` — CTA button text
- `endCardDelay` — seconds before showing end card
- `maxInteractions` — interaction count trigger for end card

## Size Budget

| Output | Size | Google (600KB) | Facebook (2MB) | Unity (5MB) |
|--------|------|----------------|----------------|-------------|
| Single HTML | ~527KB | PASS | PASS | PASS |
| ZIP | ~404KB | PASS | PASS | PASS |

## How It Works

Three layers:
1. **mraid-shim.js** — MRAID 2.0 bridge (real SDK in ad network, mock in browser)
2. **arachne-playable.js** — Lifecycle wrapper (CTA, end card, resize, analytics)
3. **bundler.sh** — Inlines WASM + JS into single distributable HTML

## Testing Locally

```bash
cd web
python3 -m http.server 8080
# Open http://localhost:8080/playable/templates/physics-toy.html
```

## Requirements

- Rust toolchain with `wasm32-unknown-unknown` target
- `wasm-bindgen-cli`
- `wasm-opt` (optional, from binaryen)
- `terser` (optional, for JS minification)
- `zip` (for ZIP output)
