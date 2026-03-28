# Swarm Goal

Arachne Windowed Runtime -- From Headless to Interactive

Take the existing Arachne engine (14 crates, 56K LOC, 1,207 tests, all headless)
and wire it to real windows, real GPU rendering, real audio output, and real input
handling. When this goal is complete, `cargo run --example physics_playground`
opens a window where you click to spawn physics bodies that bounce around on
screen.

The engine already has: a full ECS, a sprite/shape/text/mesh renderer (logic
only), a 2D physics engine, spatial audio mixer, input state tracking, an
animation system, particles, UI layout, scene graph, networking, and WASM
bindings. What it does NOT have: anything that opens a window, creates a GPU
surface, presents frames to screen, or plays sound through speakers.

Targeting ~6,000 new/modified lines across 10 tasks with comprehensive tests.

**CRITICAL RULE: Every task MUST include its own tests. No implementation
without tests. Every benchmark MUST have a hard pass/fail threshold. If a
benchmark does not meet its threshold, the task FAILS.**

**CRITICAL RULE: No monolithic integration tasks. Large integration points
are split into 2-4 smaller tasks with independent test suites.**

**CRITICAL RULE: Existing tests MUST NOT break. After every task, `cargo test
--workspace --lib` must still pass all 1,207+ existing tests.**

---

## 0) Current State

### What Exists

| Crate | LOC | Purpose | Status |
|-------|-----|---------|--------|
| arachne-math | 5,596 | Vec/Mat/Quat/Transform/Color/PRNG | Complete |
| arachne-ecs | 4,821 | World/Entity/Archetype/Query/System/Schedule | Complete |
| arachne-input | 1,972 | Keyboard/Mouse/Touch/Gamepad/ActionMap | Logic only -- no platform events |
| arachne-render | 7,299 | wgpu context, 2D/3D renderers, batching, shaders | Logic only -- no surface/present |
| arachne-physics | 4,761 | RigidBody/Collider/Broadphase/Narrowphase/Solver | Complete |
| arachne-audio | 2,995 | Mixer/Decoder/Spatial/Effects | Logic only -- no output device |
| arachne-animation | 3,352 | Tween/Easing/Keyframe/Skeleton/StateMachine | Complete |
| arachne-ui | 4,126 | Layout/Widgets/Styling | Complete |
| arachne-particles | 2,469 | Emitter/Modules/CPU+GPU sim | Complete |
| arachne-scene | 2,381 | SceneGraph/Transform propagation/Visibility | Complete |
| arachne-app | 2,468 | App/Plugin/Runner/Time/Diagnostics | HeadlessRunner only |
| arachne-networking | 3,646 | Transport/Protocol/Sync/Client/Server/Lobby | Complete |
| arachne-wasm | 3,541 | JS API/Canvas/Events/Fetch/AudioBackend | Stubs for native |

### What's Missing

1. **No window** -- nothing calls winit or creates an OS window
2. **No GPU surface** -- `RenderContext` supports headless only; `new_with_surface()` exists but has never been connected to a real window
3. **No frame present** -- SpriteRenderer prepares batches but never executes a real render pass to a swapchain texture
4. **No input bridge** -- InputSystem tracks state but nothing feeds it real OS events
5. **No audio output** -- AudioMixer mixes to a buffer but nothing sends it to speakers
6. **No real event loop** -- NativeRunner is a simple `for` loop, not a winit event loop

### Key Existing APIs to Build On

**RenderContext** (`crates/arachne-render/src/context.rs`):
- `RenderContext::new_with_surface(target, width, height)` -- exists but needs a real `wgpu::SurfaceTarget`
- `.resize(width, height)` -- exists, handles swapchain recreation
- `.current_texture()` -- exists, returns `SurfaceTexture`
- `.device()`, `.queue()` -- accessors ready to use

**SpriteRenderer** (`crates/arachne-render/src/render2d/sprite.rs`):
- `.begin_frame()` / `.draw()` / `.prepare()` / `.render()` -- full pipeline exists
- Just needs a real `RenderPass` and real bind groups

**InputSystem** (`crates/arachne-input/src/platform.rs`):
- `PlatformInput` trait with `.process_keyboard_event()`, `.process_mouse_event()`, etc.
- `InputEvent` enum matches what winit produces

**AudioMixer** (`crates/arachne-audio/src/mixer.rs`):
- `.mix()` produces PCM sample buffers
- Backend trait exists, just needs a cpal implementation

**NativeRunner** (`crates/arachne-app/src/runner.rs`):
- Runner trait: `.run(&mut self, world, schedule)`
- Needs replacement with a winit-driven event loop

---

## 1) Architecture

```
                     ┌────────────────────────────────────────────┐
                     │              User Application              │
                     │  App::new().add_plugin(DefaultPlugins)     │
                     │         .add_system(my_system).run()       │
                     └────────────────────┬─────────────────────┘
                                          │
                     ┌────────────────────▼─────────────────────┐
                     │            NativeRunner (NEW)             │
                     │  winit EventLoop::run() integration       │
                     │  Frame timing, vsync, event dispatch      │
                     └──┬──────────┬──────────┬────────────────┘
                        │          │          │
            ┌───────────▼──┐ ┌────▼──────┐ ┌▼──────────────┐
            │  WindowBridge │ │InputBridge│ │  RenderBridge  │
            │  (winit ↔     │ │(winit →   │ │  (ECS →        │
            │   window)     │ │ InputSys) │ │   GPU present) │
            └───────────────┘ └───────────┘ └───────┬───────┘
                                                     │
                  ┌──────────────────────────────────▼──────┐
                  │          Existing Engine (unchanged)      │
                  │  ECS · Renderer · Physics · Audio · UI   │
                  └──────────────────────────────────────────┘
```

### Design Principles

1. **Additive only**: The windowed runtime adds new code. Existing headless
   functionality must continue to work unchanged. HeadlessRunner stays for CI.

2. **Feature-gated**: windowed runtime behind `windowed` Cargo feature flag.
   `cargo test` without features runs headless. `cargo run --features windowed`
   opens a window.

3. **Platform split**: winit + cpal for native. web-sys + WebAudio for WASM.
   Core engine stays platform-agnostic.

4. **Thin bridge layer**: The bridge between winit and the engine should be
   <500 LOC. Don't rewrite engine internals -- just wire them up.

---

## 2) New Dependencies

| Crate | Version | Purpose | WASM? |
|-------|---------|---------|-------|
| winit | 0.30+ | Window creation, event loop | Yes (web-sys backend) |
| raw-window-handle | 0.6+ | Bridge winit ↔ wgpu surface | Yes |
| cpal | 0.15+ | Audio output stream | No (native only) |
| pollster | 0.4+ | Block on async (wgpu init) | Yes |

**Size impact**: winit adds ~50KB to WASM. cpal is native-only so zero WASM impact.

---

## 3) Task Decomposition -- 10 Tasks

**CRITICAL RULES FOR EVERY TASK:**
1. Every task MUST write tests alongside implementation. No code without tests.
2. Every benchmark MUST include both baseline AND optimized implementation.
3. Every task MUST list its pass/fail criteria. The judge uses these to accept/reject.
4. Dependencies must be respected. A task cannot start until its dependencies complete.
5. No task exceeds ~1,000 lines. If it would, split it.
6. After every task, `cargo test --workspace --lib` must still pass.

### Dependency Graph

```
WAVE 1 (fully parallel, zero dependencies):
  Task 1:  Window Management (arachne-window crate)
  Task 2:  Audio Output (cpal backend for arachne-audio)

WAVE 2 (depends on Wave 1):
  Task 3:  wgpu Surface Wiring ── depends: 1
  Task 4:  winit → InputSystem Bridge ── depends: 1
  Task 5:  Sprite Render Pipeline (end-to-end) ── depends: 3

WAVE 3 (depends on Waves 1-2):
  Task 6:  NativeRunner (real event loop) ── depends: 1, 3, 4
  Task 7:  Shape & Text Render Pipeline ── depends: 5
  Task 8:  3D Render Pipeline ── depends: 5

WAVE 4 (integration):
  Task 9:  Interactive Examples ── depends: 5, 6, 7
  Task 10: WASM Canvas Runtime ── depends: 3, 4, 5
```

---

### Task 1: Window Management

**Crate:** `arachne-window` (NEW)

**Implement:**
- `Cargo.toml`: depends on `winit = "0.30"`, `raw-window-handle = "0.6"`.
  Part of workspace, edition 2021.
- `src/lib.rs`: module declarations, re-exports.
- `src/config.rs`:
  - `WindowConfig` struct: `title: String`, `width: u32`, `height: u32`,
    `resizable: bool`, `vsync: bool`, `fullscreen: FullscreenMode`,
    `min_size: Option<(u32, u32)>`, `max_size: Option<(u32, u32)>`,
    `transparent: bool`, `decorations: bool`.
  - `FullscreenMode` enum: `Windowed`, `Borderless`, `Exclusive`.
  - `WindowConfig::default()` → 800x600, "Arachne", resizable, vsync on.
  - Builder pattern: `.with_title()`, `.with_size()`, `.with_fullscreen()`.
- `src/window.rs`:
  - `ArachneWindow` struct: wraps `winit::window::Window`.
  - `ArachneWindow::new(event_loop, config)` → creates window from config.
  - `.request_redraw()` → triggers repaint.
  - `.inner_size()` → `(u32, u32)` physical pixels.
  - `.scale_factor()` → `f64` DPI scale.
  - `.set_title(title)`, `.set_cursor_visible(bool)`.
  - `.raw_window_handle()` → for wgpu surface creation.
  - `.raw_display_handle()` → for wgpu surface creation.
- `src/event_loop.rs`:
  - `create_event_loop()` → `EventLoop<()>`.
  - `EventLoopProxy` wrapper for cross-thread wake.

**Tests (mandatory):**
- `WindowConfig::default()` has correct values (800, 600, "Arachne", vsync true).
- Builder pattern: `.with_title("Test").with_size(1024, 768)` produces correct config.
- `FullscreenMode` variants exist and default is `Windowed`.
- DPI scale factor: `scale_factor()` returns > 0.0.
- Config with min/max size correctly constrained.
- `create_event_loop()` succeeds (may need `#[cfg(not(target_os = "..."))]` for CI).

**Pass/fail:**
- All tests pass.
- WindowConfig builder is ergonomic (chainable).
- Window can be created without panic on supported platforms.
- Existing tests unaffected (`cargo test --workspace --lib` passes).

**Dependencies:** None.

---

### Task 2: Audio Output (cpal Backend)

**Crate:** `arachne-audio` (modify existing)

**Implement:**
- `src/output.rs` (NEW file):
  - `AudioOutput` struct: wraps cpal `Stream`.
  - `AudioOutput::new(mixer: Arc<Mutex<AudioMixer>>, sample_rate: u32)` → opens
    default output device, creates stream that calls `mixer.mix()` in the audio
    callback.
  - `.pause()`, `.resume()`, `.is_playing()`.
  - Sample format handling: convert f32 mixer output to device-native format (i16, f32, u16).
  - Fallback: if no audio device, create a null output that discards samples.
- `src/lib.rs`: add `pub mod output;`, re-export `AudioOutput`.
- `Cargo.toml`: add `cpal = { version = "0.15", optional = true }`.
  Feature flag: `native-audio = ["cpal"]`.

**Tests (mandatory):**
- AudioOutput creation with mock mixer succeeds (or gracefully falls back).
- Sample format conversion: f32 → i16 roundtrip within 1 LSB.
- Mixer integration: create mixer, add source, create output, verify samples consumed.
- Pause/resume state tracking.
- Null output fallback when no device available.

**Pass/fail:**
- All tests pass.
- Audio output plays sound on native platforms with audio devices.
- Graceful fallback on headless/CI (no panic, no hang).
- Existing tests unaffected.

**Dependencies:** None.

---

### Task 3: wgpu Surface Wiring

**Crate:** `arachne-render` (modify existing)

**Implement:**
- `src/context.rs` modifications:
  - `RenderContext::new_with_window(window: &impl HasWindowHandle + HasDisplayHandle, width, height)`:
    - Create `wgpu::Instance` with backends appropriate for platform.
    - Create `wgpu::Surface` from window handle.
    - Request adapter compatible with surface.
    - Create device + queue.
    - Configure surface (format, present mode, alpha mode).
    - Store surface state.
  - `PresentMode` enum exposure: `Vsync` (Fifo), `Immediate`, `Mailbox`.
  - `.present()` → call `surface_texture.present()` after rendering.
  - `.configure_surface(width, height, present_mode)` → reconfigure on resize.
  - Ensure `new_headless()` still works unchanged.
- `src/lib.rs`: re-export new types.

**Tests (mandatory):**
- `new_headless()` still works (regression test).
- Surface format selection: prefers sRGB when available.
- Resize: configure_surface with new dimensions updates surface_size().
- current_texture() returns Some after surface creation.
- present() after render does not panic.
- All existing render tests still pass.

**Pass/fail:**
- All tests pass.
- Can create a surface from a real winit window on platforms with GPU.
- Headless path fully preserved (zero changes to existing behavior).
- Surface resize handles edge cases (0-size window → no-op or skip frame).
- Existing tests unaffected.

**Dependencies:** Task 1 (needs window handle type).

---

### Task 4: winit → InputSystem Bridge

**Crate:** `arachne-input` (modify existing) or `arachne-window` (extend)

**Implement:**
- `src/winit_bridge.rs` (NEW file, in arachne-input or arachne-window):
  - `fn translate_key(winit_key: winit::keyboard::KeyCode) -> Option<KeyCode>`:
    Full mapping of winit KeyCode → arachne KeyCode (A-Z, 0-9, arrows, space,
    enter, escape, shift, ctrl, alt, F1-F12, etc.).
  - `fn translate_mouse_button(winit_btn: winit::event::MouseButton) -> Option<MouseButton>`:
    Left, Right, Middle, Other(u16).
  - `fn process_window_event(input: &mut InputSystem, event: &WindowEvent)`:
    Match on WindowEvent variants:
    - `KeyboardInput { event, .. }` → `process_keyboard_event(key, pressed)`
    - `MouseInput { button, state, .. }` → `process_mouse_event(button, pressed)`
    - `CursorMoved { position, .. }` → `process_mouse_move(position)`
    - `MouseWheel { delta, .. }` → `process_mouse_scroll(delta)`
    - `Touch { .. }` → `process_touch_event(id, position, phase)`
    - `Resized { .. }` → update window_size
    - `Focused { .. }` → update has_focus
  - Feature-gated: `#[cfg(feature = "windowed")]`.
- Tests in the same file.

**Tests (mandatory):**
- Key mapping: winit KeyA → KeyCode::A, winit ArrowUp → KeyCode::ArrowUp.
  Test all major keys (letters, digits, arrows, modifiers, function keys).
- Mouse button mapping: Left → Left, Right → Right, Middle → Middle.
- process_window_event with KeyboardInput: key_held(KeyCode::Space) returns true.
- process_window_event with CursorMoved: mouse position updated correctly.
- process_window_event with MouseWheel: scroll delta recorded.
- process_window_event with Resized: window_size updated.
- process_window_event with Focused(false): has_focus = false.
- Unknown keys: winit keys not in mapping → gracefully ignored (no panic).

**Pass/fail:**
- All tests pass.
- Full coverage of common keys (>50 key codes mapped).
- Mouse, touch, and gamepad events translated correctly.
- No panics on unknown or unusual events.
- Existing tests unaffected.

**Dependencies:** Task 1 (needs winit types).

---

### Task 5: Sprite Render Pipeline -- End to End

**Crate:** `arachne-render` (modify existing) + `arachne-app` (modify systems)

**Implement:**
- `src/pipeline_2d.rs` (NEW file in arachne-render):
  - `create_sprite_pipeline(device, surface_format)` → `wgpu::RenderPipeline`:
    - Vertex shader + fragment shader from existing `SPRITE_SHADER_SRC`.
    - Vertex buffer layout from `SpriteVertex::layout()`.
    - Instance buffer layout from `SpriteInstance`.
    - Bind group layout 0: camera uniform (mat4x4).
    - Bind group layout 1: texture + sampler.
    - Color target: surface format, alpha blend.
  - `create_camera_bind_group(device, layout, uniform_buffer)` → BindGroup.
  - `create_texture_bind_group(device, layout, texture_view, sampler)` → BindGroup.
  - `RenderFrame` struct: orchestrates a single frame render:
    - `.begin(context)` → acquire surface texture, create view.
    - `.render_sprites(renderer, camera, textures)` → execute sprite draw.
    - `.present()` → submit command buffer and present.
- `systems.rs` modifications in arachne-app:
  - `render_system`: query all (Sprite, Transform) entities, call
    `sprite_renderer.draw()` for each, then `.prepare()` + `.render()`.
  - Wire into Render stage.

**Tests (mandatory):**
- Pipeline creation succeeds on headless wgpu device.
- Camera bind group creation with identity matrix.
- RenderFrame::begin on headless returns valid encoder.
- Draw 1 sprite: prepare() produces 1 batch with 1 instance.
- Draw 1000 sprites with 4 textures: produces 4 draw calls.
- Draw 0 sprites: graceful no-op (no crash, no submit).
- **Benchmark:** Full render frame (1000 sprites) < 2ms on headless.

**Pass/fail:**
- All tests pass.
- Pipeline compiles WGSL shaders without errors.
- Sprite rendering produces correct number of draw calls.
- **1000 sprites render frame < 2ms** on native.
- Zero wgpu validation errors.
- Existing tests unaffected.

**Dependencies:** Task 3 (needs real surface for present).

---

### Task 6: NativeRunner -- Real Event Loop

**Crate:** `arachne-app` (modify existing) + uses `arachne-window`

**Implement:**
- `src/runner.rs` modifications:
  - `WindowedRunner` struct (NEW):
    - `config: WindowConfig`
    - `target_fps: u32` (default 60)
  - `WindowedRunner::new(config)` → Self.
  - `WindowedRunner::run(self, world, schedule)`:
    - `create_event_loop()`.
    - `ArachneWindow::new(event_loop, config)`.
    - `RenderContext::new_with_window(window, ...)` (async, block with pollster).
    - Insert RenderContext as resource.
    - Insert InputSystem as resource.
    - Create sprite pipeline, camera bind groups.
    - `event_loop.run(move |event, target| { ... })`:
      - `WindowEvent::CloseRequested` → exit.
      - `WindowEvent::Resized` → context.resize().
      - Input events → `process_window_event(input, event)`.
      - `AboutToWait` → schedule.run(world), render frame, present.
    - Frame timing: track delta, cap to target FPS.
  - Keep `HeadlessRunner` and `NativeRunner` unchanged.
- `src/lib.rs`: re-export `WindowedRunner`.

**Tests (mandatory):**
- WindowedRunner creation with default config succeeds.
- WindowedRunner with custom target_fps stores correct value.
- HeadlessRunner still works (regression).
- NativeRunner still works (regression).
- Frame timing calculation: 60fps target → ~16.6ms delta.
- AppExit resource causes clean exit from event loop.

**Pass/fail:**
- All tests pass.
- `cargo run --example hello_triangle --features windowed` opens a window.
- Window close (X button) exits cleanly.
- Window resize works without crash.
- Keyboard and mouse input reaches InputSystem.
- Existing tests unaffected.

**Dependencies:** Task 1, Task 3, Task 4.

---

### Task 7: Shape & Text Render Pipeline

**Crate:** `arachne-render` (modify existing)

**Implement:**
- `src/pipeline_shapes.rs` (NEW):
  - `create_shape_pipeline(device, surface_format)` → pipeline for lines/rects/circles.
  - Wire `ShapeRenderer::prepare()` output to real GPU draw calls.
  - Color-per-vertex support (no textures needed for shapes).
- `src/pipeline_text.rs` (NEW):
  - `create_text_pipeline(device, surface_format)` → pipeline for SDF text.
  - If SDF font atlas not available, use solid-color quad fallback.
  - Wire `TextRenderer::prepare()` to GPU.
- `src/pipeline_2d.rs` modifications:
  - `RenderFrame::render_shapes()` → execute shape draw.
  - `RenderFrame::render_text()` → execute text draw.
  - Ordering: shapes behind sprites behind text (z-order).

**Tests (mandatory):**
- Shape pipeline creation succeeds.
- Text pipeline creation succeeds.
- Render 100 shapes (mixed rects/circles/lines): correct draw call count.
- Render text string: produces non-zero vertices.
- Z-order: shapes rendered before text.
- **Benchmark:** 1000 shapes render < 1ms.

**Pass/fail:**
- All tests pass.
- Shapes render correctly (rects are rectangular, circles are round).
- Text is readable on screen.
- **1000 shapes < 1ms**.
- Existing tests unaffected.

**Dependencies:** Task 5.

---

### Task 8: 3D Render Pipeline

**Crate:** `arachne-render` (modify existing)

**Implement:**
- `src/pipeline_3d.rs` (NEW):
  - `create_mesh_pipeline(device, surface_format, depth_format)` → PBR mesh pipeline.
  - Depth buffer creation and management.
  - `create_light_uniform_buffer(device, lights)` → uniform buffer with light data.
  - `create_material_bind_group(device, material)` → PBR material uniforms.
  - Wire `MeshRenderer` to real GPU draw calls.
- `src/pipeline_shadow.rs` (NEW):
  - `create_shadow_pipeline(device)` → depth-only pipeline for shadow map.
  - Shadow map texture creation (1024x1024 default).
  - Light-space matrix computation.
  - Wire to render pass ordering: shadow pass → main pass.
- `src/pipeline_2d.rs` modifications:
  - `RenderFrame::render_meshes()` → execute 3D draws.
  - `RenderFrame::render_shadow_pass()` → execute shadow pass.
  - Clear depth buffer between 3D and 2D passes.

**Tests (mandatory):**
- Mesh pipeline creation succeeds.
- Shadow pipeline creation succeeds.
- Depth buffer created with correct format (Depth32Float).
- Render 1 cube: produces 1 draw call, 36 indices.
- PBR material uniforms uploaded correctly (metallic, roughness, albedo).
- Light uniform buffer layout correct for 8 lights.
- Shadow map resolution matches configured size.
- **Benchmark:** Render 100 meshes with shadows < 8ms.

**Pass/fail:**
- All tests pass.
- 3D meshes visible with correct perspective.
- PBR shading responds to material properties.
- Shadow map produces visible shadows.
- **100 meshes with shadows < 8ms**.
- Existing tests unaffected.

**Dependencies:** Task 5.

---

### Task 9: Interactive Examples

**Crate:** root workspace (modify examples)

**Implement:**
- `examples/hello_triangle/main.rs`:
  - Switch from `HeadlessRunner` to `WindowedRunner`.
  - Open 800x600 window titled "Hello Triangle".
  - Render 3 colored sprites as triangle vertices.
  - Press Escape to exit.
- `examples/sprite_demo/main.rs`:
  - Switch to `WindowedRunner`.
  - Arrow keys move player sprite on screen in real-time.
  - Camera follows player smoothly.
  - 100 background sprites visible.
- `examples/physics_playground/main.rs`:
  - Switch to `WindowedRunner`.
  - Click to spawn new physics bodies at cursor position.
  - Bodies fall under gravity, bounce off walls.
  - Bodies colored by velocity (blue=slow → red=fast).
  - Press R to reset.
- Each example: `#[cfg(feature = "windowed")]` for the windowed path,
  keep headless fallback with `#[cfg(not(feature = "windowed"))]`.
- `Cargo.toml`: add `windowed` feature flag that enables winit/cpal deps.

**Tests (mandatory):**
- Each example compiles in both windowed and headless mode.
- Headless mode still works (runs N frames, prints summary).
- hello_triangle spawns exactly 4 entities (3 sprites + 1 camera).
- sprite_demo: player position changes when arrow keys pressed.
- physics_playground: spawned bodies have non-zero velocity after 10 frames.

**Pass/fail:**
- All tests pass.
- `cargo run --example hello_triangle --features windowed` opens window with triangle.
- `cargo run --example sprite_demo --features windowed` shows movable sprite.
- `cargo run --example physics_playground --features windowed` shows interactive physics.
- Escape key exits cleanly in all examples.
- Headless mode (`cargo run --example hello_triangle`) still works.
- Existing tests unaffected.

**Dependencies:** Task 5, Task 6, Task 7.

---

### Task 10: WASM Canvas Runtime

**Crate:** `arachne-wasm` (modify existing)

**Implement:**
- `src/canvas_runtime.rs` (NEW):
  - `WasmRunner` struct implementing Runner trait.
  - `WasmRunner::run(self, world, schedule)`:
    - Get canvas element via web-sys.
    - Create wgpu surface from canvas (WebGPU backend).
    - Create RenderContext with surface.
    - `requestAnimationFrame` loop via `web_sys::window().request_animation_frame()`.
    - Frame callback: process events, run schedule, render, present.
  - `setup_event_listeners(canvas, input_system)`:
    - Keyboard: `addEventListener("keydown"/"keyup")` → InputSystem.
    - Mouse: `addEventListener("mousedown"/"mouseup"/"mousemove"/"wheel")`.
    - Touch: `addEventListener("touchstart"/"touchmove"/"touchend")`.
    - Resize: `addEventListener("resize")` on window.
    - Pointer lock support.
  - `#[wasm_bindgen(start)]` entry point or explicit `init()` function.
- `src/lib.rs`: add `pub mod canvas_runtime;`, re-export `WasmRunner`.

**Tests (mandatory):**
- WasmRunner struct creation succeeds on native (with stubs).
- Event listener setup function compiles for wasm32 target.
- `cargo check --target wasm32-unknown-unknown -p arachne-wasm` succeeds.
- Canvas element ID resolution logic correct.
- requestAnimationFrame callback structure correct (no infinite recursion).

**Pass/fail:**
- All tests pass.
- `cargo check --target wasm32-unknown-unknown -p arachne-wasm` compiles cleanly.
- On WASM: canvas renders frames via requestAnimationFrame.
- Input events from DOM reach InputSystem.
- Existing tests unaffected.

**Dependencies:** Task 3, Task 4, Task 5.

---

## 4) Performance Thresholds

| # | Metric | Threshold |
|---|--------|-----------|
| 1 | Sprite render frame (1000 sprites) | < 2ms |
| 2 | Shape render frame (1000 shapes) | < 1ms |
| 3 | 3D mesh render (100 meshes + shadows) | < 8ms |
| 4 | Full frame (render + physics + input) | < 16.6ms @ 60fps |
| 5 | Window creation + GPU init | < 2s |
| 6 | Input event translation | < 0.01ms per event |

---

## 5) Success Criteria

| Metric | Target |
|--------|--------|
| Tasks completed | 10/10 |
| Existing tests | All 1,207+ still pass |
| New tests | All pass |
| Performance thresholds | All 6 met |
| Interactive examples | 3 working with real window |
| WASM target | Compiles for wasm32-unknown-unknown |
| Escape key | Exits cleanly in all examples |
| Window resize | No crash, re-renders at new size |
| Frame timing | Stable 60fps on modern hardware |

---

## 6) Build & Run

```bash
# Build with windowed runtime
cargo build --features windowed

# Run interactive example
cargo run --example hello_triangle --features windowed
cargo run --example sprite_demo --features windowed
cargo run --example physics_playground --features windowed

# Run all tests (headless, always works)
cargo test --workspace

# Run only new windowed tests
cargo test --features windowed

# Check WASM compilation
cargo check --target wasm32-unknown-unknown -p arachne-wasm

# Verify existing tests still pass
cargo test --workspace --lib
```
