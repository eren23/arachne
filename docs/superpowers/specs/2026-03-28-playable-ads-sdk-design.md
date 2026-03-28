# Arachne Playable Ads SDK — Design Spec

## Context

Arachne is a 446KB (369KB WASM + 63KB JS) 2D game engine that runs at 120 FPS in Chrome via WebGPU. The playable ads market produces 30,700+ ad creatives per day with strict file size limits (600KB Google, 2MB Facebook, 5MB Unity/IronSource). No sub-500KB game engine SDK exists for this use case. This SDK packages Arachne as a ready-to-use playable ad creation toolkit.

## Goals

1. Produce single-file HTML playable ads that fit within Google Ads' 600KB limit
2. MRAID 2.0 compliant for deployment on all major ad networks
3. Ship 2 template games developers can customize
4. Zero Rust changes — pure JS/HTML layer on top of existing WASM binary
5. Shared infrastructure reusable for Vertical 2 (education simulations)

## Non-Goals

- Canvas2D fallback (WebGPU-only for now, revisit if adoption blocked)
- Audio support (most mobile ads muted by default)
- Asset loading from URLs (all assets embedded in WASM or inlined)
- npm package (files + build script only)
- Rust-side changes to the engine

## Architecture

```
web/playable/
├── mraid-shim.js            # MRAID 2.0 bridge
├── arachne-playable.js      # SDK wrapper (lifecycle, CTA, resize)
├── bundler.sh               # Single-file HTML builder
├── templates/
│   ├── physics-toy.html     # Template 1: drag-throw physics
│   └── catch-game.html      # Template 2: tap-to-catch
└── dist/                    # Built output
```

All three JS layers (mraid-shim, sdk, template) are standalone files during development. The bundler combines them with the WASM binary into a single distributable HTML file.

## Component Design

### 1. MRAID Shim (`mraid-shim.js`)

Thin abstraction over MRAID 2.0 that works in both ad SDK environments and standalone testing.

**Detection logic:**
```
if window.mraid exists → use real MRAID
else → provide mock (logs to console, allows testing in browser)
```

**Events handled:**
- `ready` — MRAID SDK loaded, safe to initialize
- `viewableChange(isViewable)` — pause/resume engine
- `stateChange(state)` — track ad state (loading, default, expanded, hidden)

**Methods exposed:**
- `ArachneMRAID.ready(callback)` — fires when safe to start
- `ArachneMRAID.openStore(url)` — calls `mraid.open(url)` for CTA deep link
- `ArachneMRAID.close()` — dismiss the ad
- `ArachneMRAID.onViewable(callback)` — fires on visibility changes
- `ArachneMRAID.isViewable()` — current visibility state

**Mock mode behavior:**
- `ready` fires immediately on DOMContentLoaded
- `openStore` opens URL in new tab
- `close` logs "ad closed" to console
- Allows full testing without an ad network sandbox

**Size target:** <3 KB minified.

### 2. SDK Wrapper (`arachne-playable.js`)

Wraps Arachne's existing `run(canvas_id)` entry point with playable ad lifecycle management.

**Constructor:**
```javascript
const ad = new ArachnePlayable({
  container: '#ad-container',    // DOM element or selector
  width: 320,                    // logical width
  height: 480,                   // logical height
  storeUrl: 'https://...',       // CTA deep link
  ctaText: 'Play Now!',          // CTA button text
  endCardDelay: 15,              // seconds before showing end card (0 = manual)
  maxInteractions: 10,           // alt trigger: show end card after N interactions
  onStart: () => {},             // analytics hook
  onInteract: () => {},          // analytics hook
  onCTA: () => {},               // analytics hook
  onComplete: () => {},          // analytics hook
});
```

**Lifecycle:**

```
MRAID ready → create canvas → init WASM → start engine → gameplay
    ↓ (endCardDelay or maxInteractions reached)
show end card overlay (CTA button + message)
    ↓ (user taps CTA)
mraid.open(storeUrl) → ad complete
```

**Auto-resize:**
- Listens to container/window resize events
- Scales canvas to fill container while maintaining aspect ratio
- Handles orientation changes (portrait ↔ landscape)
- Updates Arachne's internal resolution via existing `resize()` API if using `ArachneApp`, or sets canvas dimensions before `run()`

**CTA Overlay:**
- Pure HTML/CSS overlay (not engine-rendered)
- Absolutely positioned over canvas
- Configurable text, color, animation
- Calls `ArachneMRAID.openStore(url)` on tap
- Shows on end card trigger or manual call

**End Card:**
- Semi-transparent overlay with CTA button
- Triggered by timer (endCardDelay seconds) or interaction count
- Pauses engine rendering to save CPU
- Can be triggered manually via `ad.showEndCard()`

**Pause/Resume:**
- On `viewableChange(false)` → pause engine (stop rAF loop)
- On `viewableChange(true)` → resume
- Uses existing `ArachneApp.stop()` / `ArachneApp.start()` if available, or manages rAF externally

**Size target:** <6 KB minified.

### 3. Bundler (`bundler.sh`)

Shell script that produces distributable ad files from a template.

**Inputs:**
- Template HTML file (e.g., `templates/physics-toy.html`)
- Pre-built WASM binary (`web/pkg/arachne_wasm_bg.wasm`)
- Pre-built JS wrapper (`web/pkg/arachne_wasm.js`)
- SDK files (`mraid-shim.js`, `arachne-playable.js`)

**Outputs (in `dist/`):**
1. `{name}.html` — single self-contained HTML file (WASM as base64 data URI)
2. `{name}.zip` — ZIP containing HTML + WASM + JS as separate files
3. `size-report.txt` — file sizes and network limit checks

**Base64 inlining approach:**
```javascript
// Instead of: await init();
// Inline WASM as base64, decode at runtime:
const wasmBytes = Uint8Array.from(atob("AGFzbQEAAAA..."), c => c.charCodeAt(0));
await init(wasmBytes);
```

This avoids network requests (required by some ad networks that ban fetch/XHR).

**JS minification:**
- Uses `terser` if available (npm install -g terser)
- Falls back to basic whitespace stripping if not
- Concatenates all JS into single inline `<script>` block

**Size reporting:**
```
=== Arachne Playable Ad Size Report ===
WASM binary:        369,420 bytes
JS (minified):       30,211 bytes
HTML + CSS:           2,100 bytes
Base64 overhead:    123,140 bytes (WASM only, single-file mode)
─────────────────────────────────
Single HTML total:  524,871 bytes  [OK: under 600KB Google limit]
ZIP total:          401,731 bytes  [OK: under 600KB Google limit]
```

**Warns if:**
- Single-file > 600KB (Google Ads limit)
- ZIP > 2MB (Facebook limit)
- ZIP > 5MB (Unity Ads limit)

### 4. Template 1: Physics Toy (`templates/physics-toy.html`)

Repackages the existing `physics_playground` as a playable ad.

**Gameplay:**
- Pre-spawned colorful physics bodies bouncing around
- Tap/drag to throw new bodies
- Bodies color-shift by velocity (slow=blue, fast=red)
- Boundary walls keep everything on screen

**Ad flow:**
- 0s: Physics sim starts with ~10 pre-spawned bodies
- 0-15s: User can drag-throw new bodies (interactive phase)
- 15s or 10 throws: End card appears with CTA
- CTA tap: `mraid.open(storeUrl)`

**Implementation:** Uses `run('canvas-id')` directly — the physics playground IS the game. The SDK wrapper handles the ad lifecycle around it.

**Changes from physics_playground:**
- Remove escape-to-exit behavior
- Remove R-to-reset (or repurpose as "shake to reset" for mobile)
- Scale to mobile dimensions (320x480 default)
- Add touch support (existing DOM event bridge handles this)

### 5. Template 2: Catch Game (`templates/catch-game.html`)

New minimal game built with `ArachneApp` programmatic API.

**Gameplay:**
- Objects fall from random positions at top of screen
- Tap/click to "catch" them (removes object, increments score)
- Missed objects hit bottom and disappear
- Speed increases over time
- Score displayed via `ScreenTextBuffer`

**Ad flow:**
- 0s: Game starts, objects begin falling
- 0-20s: Gameplay phase, score accumulates
- 20s or score reaches 15: End card with "Beat the high score!"
- CTA tap: `mraid.open(storeUrl)`

**Implementation:** New game logic, but uses existing engine systems:
- Sprites for falling objects (use built-in colored shapes if no textures)
- Physics gravity for falling (existing)
- Click/touch hit detection (existing input system)
- Text rendering for score (existing ScreenTextBuffer)

This template will be defined as a self-contained `<script>` block that configures `ArachneApp` or calls `run()` after modifying the world setup. Exact approach depends on how much of the `run()` setup we can reuse vs needing the `ArachneApp` API.

**Fallback approach:** If `ArachneApp` proves too complex to wire for a custom game, we build this as a second Rust entry point in `arachne-wasm/src/lib.rs` (e.g., `run_catch(canvas_id)`) and compile both templates into the same WASM binary. This adds no size if the code shares engine systems.

## Size Budget

| Component | Single-file (base64) | ZIP mode |
|-----------|---------------------|----------|
| WASM binary | 369 KB | 369 KB |
| Base64 overhead (+33%) | +123 KB | 0 |
| arachne_wasm.js (minified) | ~25 KB | ~25 KB |
| mraid-shim.js (minified) | ~2 KB | ~2 KB |
| arachne-playable.js (minified) | ~5 KB | ~5 KB |
| Template + HTML + CSS | ~3 KB | ~3 KB |
| **Total** | **~527 KB** | **~404 KB** |

Both fit within Google's 600KB limit. ZIP mode has 196KB of headroom for enhanced templates on larger networks.

## Shared Infrastructure for Education Sims (Vertical 2)

The following components are designed for reuse:

| Playable Ads | Education Sims | Shared Pattern |
|-------------|----------------|----------------|
| `mraid-shim.js` | `lms-shim.js` (xAPI/LTI) | Environment detection + mock fallback |
| `arachne-playable.js` | `arachne-sim.js` | Lifecycle wrapper over `run()` / `ArachneApp` |
| `bundler.sh` | Same bundler | Inline WASM + JS into single HTML |
| CTA overlay | Parameter controls (sliders) | HTML overlay pattern over canvas |
| End card | Reset/step/pause controls | Post-gameplay UI |
| `onInteract` hooks | `onMeasurement` hooks | Analytics callback pattern |

The abstraction boundary is: "wrapper that manages an Arachne canvas with domain-specific chrome around it."

## Testing & Validation

1. **Local testing:** Open template HTML files directly in Chrome (mraid-shim mock mode)
2. **Size validation:** bundler.sh reports sizes and checks against network limits
3. **Facebook validation:** Upload to [Facebook Ad Tester](https://developers.facebook.com/tools/playable-preview/)
4. **Google validation:** Test via Google Ads preview tool
5. **Mobile testing:** Test on Android Chrome (WebGPU) and iOS Safari 18+ (WebGPU)
6. **Regression:** Existing `cargo test --workspace` still passes (no Rust changes)

## Files to Create

1. `web/playable/mraid-shim.js`
2. `web/playable/arachne-playable.js`
3. `web/playable/bundler.sh`
4. `web/playable/templates/physics-toy.html`
5. `web/playable/templates/catch-game.html`
6. `web/playable/README.md` (usage docs for SDK)

## Files to Modify

None. Zero Rust changes. The existing WASM binary and JS wrapper are used as-is.

Exception: If Template 2 (catch game) needs a custom entry point, we add a `run_catch(canvas_id)` function to `crates/arachne-wasm/src/lib.rs`. This is additive only.
