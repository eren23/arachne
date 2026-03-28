# Arachne — Current Status (2026-03-28)

## What Got Done This Session

### Step 1: WASM in Browser — COMPLETE

The entire WASM pipeline works end-to-end:

| Component | Status | Detail |
|-----------|--------|--------|
| web-sys/js-sys deps | Done | Added to arachne-wasm Cargo.toml |
| wasm_bindgen API | Done | ArachneApp, AppState exported to JS |
| DOM event listeners | Done | keyboard, mouse, wheel, touch, focus |
| WebGPU surface | Done | Canvas → wgpu::SurfaceTarget::Canvas |
| requestAnimationFrame | Done | Recursive Closure::wrap loop |
| GPU pipeline (shared) | Done | gpu_init.rs shared with WindowedRunner |
| Frame rendering | Done | render_wasm_frame() mirrors native path |
| Input pipeline | Done | DomEventQueue resource → PreUpdate system |
| TypeScript types | Done | build.sh generates .d.ts files |
| index.html | Done | WebGPU detection, real WASM loading |
| Build + optimize | Done | 384KB after wasm-opt -Oz |

**Verified:** Physics playground runs at 120 FPS in Chrome with click-to-spawn.

### Step 2: Real Textures — PARTIAL

| Component | Status | Detail |
|-----------|--------|--------|
| Asset directories | Done | assets/sprites/, tiles/, physics/ |
| Pixel art PNGs | Done | player, tileset, circle, box (~4KB total) |
| Native auto-loading | Done | gpu_init.rs loads from assets/ at startup |
| Physics playground textures | Done | Uses TextureHandle(3) circle, (4) box |
| WASM fetch loading | Not done | Placeholder handles created, no fetch |
| hello_triangle textures | Not done | Still uses fallback |
| eren_game textures | Not done | Still uses fallback |

### Step 3: Polished Demo — MOSTLY COMPLETE

| Component | Status | Detail |
|-----------|--------|--------|
| Click-drag throw | Done | Records drag start, applies velocity |
| Velocity HSV coloring | Done | Blue(240°)→Red(0°) by speed |
| UI overlay (FPS/bodies) | Done | ScreenTextBuffer bitmap font |
| Particle effects | Not done | arachne-particles exists but not wired |
| Screenshot for README | Not done | |

---

## What's Missing / Next Steps

### Immediate (finish what's started)

1. **WASM texture loading via fetch()** — assets load on native but not WASM.
   Need `AssetFetcher` or `web_sys::window().fetch()` to load PNGs from URLs,
   then call `load_png_bytes()`. The infrastructure exists, just needs wiring.

2. **Particle effects on collision** — `arachne-particles` (2,469 LOC) is complete.
   Wire it into the physics playground: trigger burst particles at contact points
   from the solver output.

3. **Screenshot for README** — Take a high-quality screenshot of the physics
   sandbox in both native windowed and browser WASM for the repo README.

4. **Update other examples** — hello_triangle and eren_game still use
   TextureHandle(0) white fallback. Update to use loaded textures.

### Step 4: Public API (`arachne` facade crate)

- Create `crates/arachne/` with `Cargo.toml` depending on all 15 crates
- Design `prelude.rs` with ergonomic re-exports
- Write 10-line hello world that compiles
- Publish `arachne = "0.1.0-alpha.1"` to crates.io

### Step 5: WASM npm Deployment

- Clean JS API in `api.rs` (current wasm_bindgen annotations work)
- Create `package.json` for `@arachne-engine/core`
- Publish to npm → auto-hosted on unpkg CDN
- One-line `<script>` embed example

### Step 6: Docs + Landing Page

- `cargo doc --workspace --no-deps` for rustdoc
- GitHub Pages hosting
- Landing page with live WASM demo
- Quickstart tutorial

### Step 7: Community

- GitHub Discussions, CONTRIBUTING.md
- Blog post: "Sub-1MB game engine in Rust"
- r/rust, HN, Twitter launch

---

## Files Changed (for commit)

### Modified (12 files, +1005/-444):
```
crates/arachne-app/src/gpu_init.rs      NEW — shared GPU init (extracted from runner.rs)
crates/arachne-app/src/lib.rs           +7 — module declaration + re-exports
crates/arachne-app/src/runner.rs        -275 — extracted to gpu_init.rs
crates/arachne-app/src/systems.rs       +12 — load_png_bytes() method
crates/arachne-wasm/Cargo.toml          +29 — web-sys, js-sys, wgpu, bytemuck, arachne-render/physics
crates/arachne-wasm/src/api.rs          +91 — wasm_bindgen annotations + JS impl block
crates/arachne-wasm/src/canvas_runtime.rs +556 — real WebGPU + rAF + render + input
crates/arachne-wasm/src/events.rs       +14 — updated register_dom_listeners
crates/arachne-wasm/src/lib.rs          +177 — run() entry point with physics playground
examples/physics_playground/main.rs     +136 — click-drag, UI overlay, textures, HSV
web/build.sh                            +2 — --typescript flag
web/index.html                          +143 — real WASM loading
Cargo.lock                              +7
```

### New untracked:
```
assets/sprites/player.png               16x16 pixel art character
assets/tiles/tileset.png                128x128 tile atlas
assets/physics/circle.png               32x32 circle
assets/physics/box.png                  32x32 box
CHANGELOG.md                            This session's changes
tasks/roadmap.md                        Development roadmap
tasks/status.md                         This file
```

### Ignore (don't commit):
```
.playwright-mcp/                        Playwright test artifacts
wasm-*.png                              Debug screenshots (9 files)
```

---

## Key Numbers

| Metric | Value |
|--------|-------|
| WASM binary (wasm-opt -Oz) | 384KB |
| WASM binary (raw) | 1.6MB |
| FPS in Chrome | 120 |
| Physics bodies (demo) | 20 initial + click-to-spawn |
| Asset size (total PNGs) | ~4KB |
| Tests passing | 1,281 / 1,281 |
| Lines changed | +1,005 / -444 |
| New files | 1 (gpu_init.rs) + 4 assets + 3 docs |
