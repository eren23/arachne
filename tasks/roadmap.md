# Arachne Development Roadmap

**Last updated:** 2026-03-28
**Repo:** https://github.com/eren23/arachne
**Status:** 15 crates, 132K LOC, 1,281 tests, windowed rendering works, WASM compiles untested

---

## Current State — What Works

| Feature | Status | Notes |
|---------|--------|-------|
| Math library | Done | Vec2/3/4, Mat3/4, Quat, Transform, Color, PRNG, Fixed-point. 5,596 LOC |
| ECS | Done | Archetype-based, queries, systems, schedule, commands, change detection, parallel. 4,821 LOC |
| Input | Done | Keyboard, mouse, touch, gamepad, action mapping, winit bridge. 1,972 LOC |
| 2D Rendering | Done | Sprites, text (bitmap font), tilemaps, shapes. Renders to window via wgpu. 7,299 LOC |
| 3D Rendering | Exists | PBR pipeline, lights, shadows. Code exists but not wired to windowed runner |
| Physics | Done | 2D deterministic: rigid bodies, colliders, GJK/EPA, solver, spatial queries. 4,761 LOC |
| Audio | Partial | Mixer, decoder, effects exist. cpal native output exists. No actual sound tested |
| Animation | Done | Tweens, easing, keyframes, skeleton, skinning, state machines. 3,352 LOC |
| Particles | Done | Emitters, modules, CPU/GPU sim. 2,469 LOC |
| UI | Done | Flexbox layout, widgets (button, slider, checkbox, text input, etc). 4,126 LOC |
| Scene graph | Done | Hierarchy, transform propagation, visibility, prefabs, serialization. 2,381 LOC |
| Networking | Done | WebSocket transport, binary protocol, client/server, lobby. 3,646 LOC |
| WASM bindings | Partial | JS API stubs, canvas runtime stubs. Compiles for wasm32 (1.0MB). NOT tested in browser |
| Window management | Done | winit 0.30, WindowedRunner with event loop, input bridge |
| Texture loading | Partial | TextureStorage + PNG decoder exist. Not loading real PNG files in examples yet |
| Text rendering | Done | Built-in 5x7 bitmap font, ScreenTextBuffer API, renders on screen |
| Tilemap rendering | Done | TilemapRenderer with atlas, renders tile grids. Procedural tile atlas (32 tiles) |

## Current State — What Doesn't Work

1. **WASM in a browser** — compiles to 1.0MB wasm but nobody has tested it in Chrome/Safari/Firefox
2. **No real textures** — everything renders as colored squares (white fallback texture tinted by color)
3. **No public API** — no `arachne` facade crate, no prelude, can't `cargo add arachne`
4. **No docs** — no rustdoc, no tutorials, no "how to embed" guide
5. **3D not wired** — PBR pipeline exists but not connected to the windowed runner render loop
6. **Audio not audible** — mixer exists, cpal backend exists, but no example actually plays sound
7. **Not published** — not on crates.io, not on npm
8. **Examples are rough** — colored squares, janky collision, no polish

---

## Step-by-Step Roadmap

### Step 1: WASM Running in a Real Browser
**Priority:** CRITICAL — this is the entire value proposition
**Effort:** Medium

Tasks:
- [x] Install wasm-bindgen-cli: `cargo install wasm-bindgen-cli`
- [x] Install wasm-opt: `brew install binaryen`
- [x] Run `bash web/build.sh` and fix any issues
- [x] Update `web/index.html` to properly initialize the WASM module
- [x] Wire the WasmRunner in `crates/arachne-wasm/src/canvas_runtime.rs` to actually:
  - Create a WebGPU surface from a canvas element
  - Run requestAnimationFrame loop
  - Translate DOM events to InputSystem
- [x] Serve with `python3 -m http.server 8080` and open in Chrome
- [x] See pixels on screen in the browser
- [x] Measure actual WASM size after wasm-opt -Oz (target: <900KB) — **384KB**
- [ ] Take a screenshot for the README

**Key files:**
- `web/build.sh` — build script
- `web/index.html` — entry page
- `crates/arachne-wasm/src/canvas_runtime.rs` — WASM runtime
- `crates/arachne-wasm/src/events.rs` — DOM event translation
- `crates/arachne-wasm/src/api.rs` — JS-facing API

**Blocker:** WebGPU support in browsers (Chrome 113+, Firefox behind flag, Safari 18+)

### Step 2: Real Texture Loading (No More Colored Squares)
**Priority:** HIGH — everything looks amateur without textures
**Effort:** Medium

Tasks:
- [x] Create a simple asset directory: `assets/sprites/`, `assets/tiles/`
- [x] Create or download simple pixel art: player sprite, tile set, icons
- [x] Wire `TextureStorageResource::load_png()` into example startup systems
- [x] Make sprites reference loaded textures instead of TextureHandle(0)
- [ ] For WASM: load textures via fetch() from URLs
- [ ] Update hello_triangle to show a loaded PNG sprite
- [ ] Update eren_game to use pixel art tiles and player sprite

**Key files:**
- `crates/arachne-app/src/systems.rs` — `TextureStorageResource::load_png()`
- `crates/arachne-asset/src/image.rs` — `Image::decode_png()`
- `examples/eren_game/main.rs` — needs real assets

### Step 3: Polish One Demo Until Screenshot-Worthy
**Priority:** HIGH — need something impressive to show
**Effort:** High

Pick ONE demo and make it beautiful:

**Option A: Physics Sandbox (easier)** ← CHOSEN
- [x] Smooth 60fps (120 FPS achieved)
- [x] Colorful bodies with circle textures
- [ ] Particle effects on collision
- [x] UI overlay: body count, FPS, reset button
- [x] Click-drag to throw bodies
- [ ] Screenshot/GIF material

**Option B: Eren Game (harder, more personal)**
- Real pixel art tiles and player sprite
- Smooth player movement with animation
- 3-5 rooms with actual content (your projects, bio)
- Dialogue system with typewriter effect
- Background music (if audio works)
- "This runs in 800KB" meta-moment

### Step 4: Public API — `use arachne::prelude::*;`
**Priority:** MEDIUM — needed for other devs to use it
**Effort:** Medium

Tasks:
- [ ] Create `crates/arachne/` facade crate that re-exports everything
- [ ] Design the prelude: what types do users need?
  ```rust
  pub use arachne_app::{App, DefaultPlugins, Plugin, ...};
  pub use arachne_ecs::{World, Entity, Query, Res, ResMut, Commands, ...};
  pub use arachne_math::{Vec2, Vec3, Transform, Color, ...};
  pub use arachne_render::{Sprite, Camera2d, TextureHandle, ...};
  pub use arachne_input::{InputSystem, KeyCode, MouseButton, ...};
  pub use arachne_physics::{PhysicsWorld, RigidBody, Collider, ...};
  ```
- [ ] Write a "10-line hello world" that compiles:
  ```rust
  use arachne::prelude::*;

  fn main() {
      App::new()
          .add_plugin(DefaultPlugins)
          .add_startup_system(|mut cmd: Commands| {
              cmd.spawn((Camera::new(), Transform::IDENTITY));
              cmd.spawn((Sprite::colored(Color::RED, 50.0), Transform::IDENTITY));
          })
          .run();
  }
  ```
- [ ] Publish to crates.io as `arachne = "0.1.0-alpha.1"`
- [ ] Write a minimal README for the crate page

### Step 5: WASM Deployment — npm + CDN + One-Line Embed
**Priority:** MEDIUM — the developer experience
**Effort:** Medium-High

Tasks:
- [ ] Clean the JS API in `crates/arachne-wasm/src/api.rs`
- [ ] Generate TypeScript types with wasm-bindgen
- [ ] Create npm package: `@arachne-engine/core`
- [ ] Publish to npm
- [ ] Host on CDN (unpkg auto-serves npm packages)
- [ ] One-line embed works:
  ```html
  <script type="module">
    import { Arachne } from 'https://unpkg.com/@arachne-engine/core';
    const app = new Arachne('#canvas');
    app.start();
  </script>
  ```
- [ ] Write "How to embed Arachne in your website" guide

### Step 6: Documentation + Landing Page
**Priority:** MEDIUM — needed for adoption
**Effort:** Medium

Tasks:
- [ ] Generate rustdoc for all crates: `cargo doc --workspace --no-deps`
- [ ] Host docs on GitHub Pages
- [ ] Write quickstart tutorial (blog post format)
- [ ] Create landing page (arachne.dev or GitHub Pages):
  - Hero: "Sub-1MB embeddable engine"
  - Live WASM demo (physics sandbox)
  - Code example
  - Size comparison table (vs Unity, Godot, Bevy)
  - Link to docs + GitHub
- [ ] "Built with Arachne" showcase section

### Step 7: Community + Growth
**Priority:** LOW (do after v0.1)
**Effort:** Ongoing

- [ ] GitHub Discussions enabled
- [ ] CONTRIBUTING.md
- [ ] Issue templates
- [ ] Example contribution guidelines
- [ ] Blog post: "How I built a sub-1MB game engine in Rust"
- [ ] Post to r/rust, Hacker News, Twitter
- [ ] Target: 500 GitHub stars in 3 months

---

## Architecture Decisions (for reference)

| Decision | Choice | Why |
|----------|--------|-----|
| ECS storage | Archetype-based | Better cache locality for WASM |
| GPU backend | wgpu | Compiles to WebGPU (WASM) + Vulkan/Metal/DX12 (native) |
| Physics timestep | Fixed (deterministic) | Same inputs = same outputs, enables replay + multiplayer |
| UI paradigm | Immediate-mode | Simpler, <1ms for 200 widgets |
| Serialization | Manual (no serde) | Saves ~50-100KB in WASM binary |
| Font rendering | Built-in bitmap 5x7 | Zero external deps, works everywhere |
| Proc macros | None | Reduces compile time + WASM bloat |

## Size Budgets (Hard Caps)

| Artifact | Budget | Current |
|----------|--------|---------|
| Core WASM (ECS + math + input) | <200KB | Untested |
| 2D WASM (core + renderer) | <350KB | Untested |
| Full WASM (all features) | <900KB | 1.0MB (needs wasm-opt) |
| JS wrapper | <30KB | Untested |

## Performance Thresholds (All Passing)

| Metric | Threshold | Actual |
|--------|-----------|--------|
| Vec3 ops | >=200M ops/sec | 875M |
| Mat4 multiply | >=50M ops/sec | 207M |
| Entity spawn (3 comp) | >=500K/sec | 7.7M |
| Query iterate 1M | >=10M/sec | 575M |
| Broadphase | >=1M checks/sec | 12M |
| Physics step (1000 bodies) | <4ms | 0.24ms |
| Batch 100K sprites | <100ms | 0.23ms |

## Key Commands

```bash
# Run examples (windowed)
cargo run -p arachne-examples --example hello_triangle --features windowed
cargo run -p arachne-examples --example physics_playground --features windowed
cargo run -p arachne-examples --example eren_game --features windowed

# Run all tests
cargo test --workspace

# Build WASM
cargo build --target wasm32-unknown-unknown --release -p arachne-wasm

# Build + optimize WASM
bash web/build.sh

# Check WASM size
ls -lh target/wasm32-unknown-unknown/release/arachne_wasm.wasm

# Generate docs
cargo doc --workspace --no-deps --open
```

## Repo Structure

```
arachne/
├── Cargo.toml              # Virtual workspace
├── README.md               # Project README
├── crates/                  # 15 engine crates
│   ├── arachne-math/
│   ├── arachne-ecs/
│   ├── arachne-input/
│   ├── arachne-render/      # 2D/3D rendering, shaders, pipelines
│   ├── arachne-physics/
│   ├── arachne-audio/
│   ├── arachne-asset/
│   ├── arachne-animation/
│   ├── arachne-particles/
│   ├── arachne-ui/
│   ├── arachne-scene/
│   ├── arachne-app/         # App framework, runners, plugins
│   ├── arachne-networking/
│   ├── arachne-wasm/        # WASM bindings, JS API
│   └── arachne-window/      # winit window management
├── examples/                # Demo applications (workspace member)
│   ├── Cargo.toml
│   ├── hello_triangle/
│   ├── sprite_demo/
│   ├── physics_playground/
│   ├── particle_fireworks/
│   ├── product_configurator/
│   ├── platformer/
│   ├── multiplayer_pong/
│   └── eren_game/           # Tile-based adventure (portfolio demo)
├── tests/
│   ├── integration/         # App lifecycle, ECS stress, physics determinism
│   └── benchmarks/          # Math, ECS, physics, render throughput
├── web/                     # WASM build output + HTML
├── tools/                   # Size/frame budget scripts
└── tasks/                   # Goals + roadmap docs
```
