# Changelog

## [Unreleased] — 2026-03-28

### WASM Browser Runtime (Step 1 of Roadmap — COMPLETE)

The Arachne engine now runs in the browser via WebGPU. Full interactive physics
playground running at 120 FPS in 384KB of WASM.

**Added:**
- `web-sys`, `js-sys`, `wasm-bindgen-futures` dependencies to `arachne-wasm`
- `#[wasm_bindgen]` annotations on `ArachneApp`, `AppState`, and JS-facing methods
- Real DOM event listeners for keyboard, mouse, wheel, touch, and focus events
- WebGPU surface creation from `<canvas>` element via `wgpu::SurfaceTarget::Canvas`
- `requestAnimationFrame` loop with proper frame timing
- `render_wasm_frame()` — full sprite/tilemap/text render pipeline for WASM
- `DomEventQueue` ECS resource + `drain_dom_events_system` for correct input frame timing
- `run(canvas_id)` — exported WASM entry point that boots the full engine with physics
- `gpu_init.rs` — shared GPU resource initialization (pipelines, textures, camera, font atlas)
- `init_gpu_resources(world, context)` — reusable function called by both WindowedRunner and WasmRunner
- `load_png_bytes()` on `TextureStorageResource` for WASM-compatible texture loading
- TypeScript declaration generation in `web/build.sh`
- WebGPU feature detection in `web/index.html`

**Changed:**
- `WindowedRunner::resumed()` now calls shared `init_gpu_resources()` (was 250 lines inline)
- `RenderContextResource`, `SpritePipelineResource`, `TilemapPipelineResource`, `GpuResources`
  moved from `windowed` module to `gpu_init` module (always available, not feature-gated)
- `web/index.html` — replaced Canvas 2D placeholder with real WASM WebGPU initialization
- `web/build.sh` — enabled TypeScript declarations (`--typescript` instead of `--no-typescript`)

**Fixed:**
- Doc comment containing `*/` inside JS code block broke wasm-bindgen generated JS
- Double `begin_frame()` in WASM input pipeline was clearing "just pressed" state before
  game systems could read it; fixed by draining DOM events as a PreUpdate ECS system

### Real Textures (Step 2 of Roadmap — PARTIAL)

**Added:**
- `assets/sprites/player.png` (16x16 pixel art character)
- `assets/tiles/tileset.png` (128x128, 8x8 grid of 16x16 tiles)
- `assets/physics/circle.png` (32x32 circle with alpha)
- `assets/physics/box.png` (32x32 beveled box)
- Auto-loading of asset PNGs during GPU init (handles 1-4) with fallback
- WASM placeholder handles for async texture loading path

**Changed:**
- Physics playground uses `TextureHandle(3)` (circle) and `TextureHandle(4)` (box)
  instead of white fallback for physics bodies

### Polished Physics Sandbox (Step 3 of Roadmap — COMPLETE)

**Added:**
- Click-drag to throw bodies (mouse down records position, mouse up spawns with velocity)
- UI overlay: FPS counter, body count, instructions via `ScreenTextBuffer`
- HSV-based velocity coloring (blue 240° = slow → red 0° = fast)

**Changed:**
- `spawn_on_click` replaced with `spawn_on_click_drag` in physics playground
- Gravity scaled to 800 px/s² for pixel-coordinate physics (was 9.81 m/s²)

### Stats

- **WASM binary size:** 384KB (wasm-opt -Oz), well under 900KB budget
- **FPS:** 120 in Chrome (WebGPU)
- **All 1,281 existing tests pass** — zero regressions
- **12 files changed**, +1,005 / -444 lines, 1 new file (`gpu_init.rs`)
