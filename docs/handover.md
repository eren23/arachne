# Arachne Playable Ads SDK — Handover

## What Was Done

### Strategic Analysis
- Deep audit of all 15 crates: identified what's production-ready vs stubs
- Competitive landscape research (Phaser, Bevy, Godot, WASM-4, playable ad tools)
- Identified 3 verticals: **Playable Ads** (started), **Education Sims** (next), **Browser Games** (later)

### Playable Ads SDK (MVP Complete)

Built `web/playable/` — packages Arachne as playable ads for ad networks.

**Files created:**
- `web/playable/mraid-shim.js` — MRAID 2.0 bridge (real SDK or mock for testing)
- `web/playable/arachne-playable.js` — Lifecycle wrapper (CTA, end card, resize, analytics hooks)
- `web/playable/bundler.sh` — Inlines WASM+JS into single self-contained HTML
- `web/playable/templates/physics-toy.html` — Drag-throw physics ad template
- `web/playable/templates/catch-game.html` — Tap-to-catch ad template
- `web/playable/README.md`, `.gitignore`

**Rust change:**
- Added `run_catch()` entry point in `crates/arachne-wasm/src/lib.rs` — tap-to-catch falling objects game

**Build outputs (`web/playable/dist/`):**
- `physics-toy.html` / `catch-game.html` — 578KB single-file (PASS Google 600KB limit)
- `.zip` variants — 165KB (PASS all networks)

### Size Budget

| Output | Size | Google (600KB) | Facebook (2MB) | Unity (5MB) |
|--------|------|----------------|----------------|-------------|
| Single HTML | 578KB | PASS (35KB headroom) | PASS | PASS |
| ZIP | 165KB | PASS | PASS | PASS |

## What's NOT Done

### Playable Ads — Remaining
- **Test on actual ad network testers** (Facebook Ad Tester, Google Ads preview) — validates WASM works in ad sandboxes
- **Bundled dist/ templates need rebuilding** after the canvas resolution fix (320→480)
- Templates use 480x720 canvas with max-width cap — works on desktop but untested on actual mobile ad viewports
- No terser installed — JS not minified (would save ~15-20KB)

### Vertical 2: Education Sims — Not Started
- Design exists in conversation context (JS config API, pre-built simulations)
- Shared infra from playable ads SDK is reusable (bundler, lifecycle wrapper pattern)
- Needs: wire physics constraints in solver, `ArachneSim` JS API, 4 simulation templates, CDN deploy

### Honest Feature Gaps (from audit)
- **3D rendering**: shader exists, not integrated into any example
- **UI rendering**: layout works, nothing draws to screen
- **Networking**: MockTransport only, WebSocket is stubs
- **Audio in WASM**: entirely comments, zero sound plays in browser
- **Animation**: data structures exist, nothing actually animates
- **Asset loading from URLs**: not wired in WASM

## How to Build & Test

```bash
# Rebuild WASM
bash web/build.sh

# Bundle playable ads
cd web/playable
bash bundler.sh templates/physics-toy.html
bash bundler.sh templates/catch-game.html

# Test locally
cd web && python3 -m http.server 8090
# http://localhost:8090/playable/templates/physics-toy.html
# http://localhost:8090/playable/templates/catch-game.html

# Run tests
cargo test --workspace
```

## Key Documents
- Strategic plan: `docs/superpowers/plans/2026-03-28-playable-ads-sdk.md`
- Memory files: `~/.claude/projects/.../memory/` (project overview, user profile, repo reference)

## Nothing Was Committed
All changes are unstaged. The git revert from the accidental commit is in history.
