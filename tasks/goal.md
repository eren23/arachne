# Swarm Goal

Arachne -- Sub-1MB Embeddable Interactive Runtime (Rust, WASM-first)

Build a complete interactive runtime engine from scratch in Rust that compiles
to both WebAssembly (<500KB core, <1MB full) and native targets with wgpu
acceleration. Arachne is an ECS-based engine for interactive experiences:
product configurators, educational simulations, data visualizations, creative
coding, lightweight games, and embeddable 3D/2D content. It is NOT a AAA game
engine -- it is the runtime you embed in a web page with a single `<script>`
tag.

The engine provides: an archetype-based ECS, a wgpu-powered 2D+3D renderer
with automatic batching, a deterministic 2D physics engine, spatial audio,
unified input handling (keyboard/mouse/touch/gamepad), an asset pipeline with
async streaming, a scene graph with transform hierarchy, skeletal + tween
animation, an immediate-mode UI system, a GPU particle system, and a
WebSocket/WebRTC networking layer. All subsystems are designed to fit within
strict WASM size and frame budgets.

Targeting ~100,000 total lines across ~28 tasks with comprehensive unit tests,
integration tests, benchmark tests with hard pass/fail thresholds, and
end-to-end demo applications.

**CRITICAL RULE: Every task MUST include its own tests. No implementation
without tests. Every benchmark MUST have a hard pass/fail threshold. If a
benchmark does not meet its threshold, the task FAILS.**

**CRITICAL RULE: No monolithic integration tasks. Large integration points
are split into 2-4 smaller tasks with independent test suites.**

---

## 0) Project Overview

### Why Arachne?

The interactive content landscape has a gap:

| Engine | WASM Size | Scope | Problem |
|--------|----------|-------|---------|
| Unity WebGL | 15-30MB | Full game engine | Absurdly large for web embedding |
| Godot WASM | ~25MB | Full game engine | Same -- massive download for a widget |
| Bevy | ~5-10MB | Rust game engine | No stable API, not web-first |
| Three.js | ~150KB | JS 3D renderer | No ECS, no physics, JS-only |
| Pixi.js | ~100KB | JS 2D renderer | Rendering only, not an engine |
| macroquad | ~200KB | Rust web games | Thin wrapper, no engine features |

Arachne occupies the bottom-left: **engine-complete but tiny**. A full ECS
engine with physics, audio, animation, and rendering in <1MB WASM. Small
enough to embed in a blog post, powerful enough to build a product
configurator or an educational physics simulation.

### Design Principles

1. **WASM-first**: Every API decision is made with WASM constraints in mind.
   No file system assumptions, no threads in core (optional via
   `SharedArrayBuffer`), no unbounded allocations. If it doesn't work in
   WASM, it doesn't ship.

2. **Size budget is sacred**: <500KB core WASM, <1MB with all features.
   Every dependency is scrutinized. No serde_json (use miniserde or manual).
   No regex. No proc macros that bloat output. `wasm-opt -Oz` on every
   build. Size is tested in CI.

3. **Frame budget is sacred**: 16.6ms per frame at 60fps. Every system has
   a time budget. Physics: 2ms. Rendering: 8ms. Everything else: 4ms.
   Budgets are tested with hard thresholds.

4. **Deterministic where possible**: Physics is fixed-timestep deterministic.
   Same inputs = same outputs across WASM and native. This enables replay,
   testing, and multiplayer synchronization.

5. **Embeddable**: The WASM build exposes a clean JS API. Embedding Arachne
   in a web page is one `<script>` tag + one `<canvas>`. No build tools
   required for consumers.

6. **Feature flags**: Every subsystem is behind a Cargo feature flag.
   `arachne-core` (ECS + math + input) is ~200KB. Add `renderer-2d`,
   `renderer-3d`, `physics-2d`, `audio`, `ui`, `animation`, `particles`,
   `networking` a la carte.

### Sample Usage (Rust API)

```rust
use arachne::prelude::*;

fn main() {
    App::new()
        .add_plugin(DefaultPlugins)  // renderer, input, audio, physics
        .add_startup_system(setup)
        .add_system(move_player)
        .add_system(spawn_particles_on_click)
        .run();
}

fn setup(mut commands: Commands, assets: Res<AssetServer>) {
    // Spawn a camera
    commands.spawn(Camera2dBundle::default());

    // Spawn a sprite
    commands.spawn(SpriteBundle {
        texture: assets.load("player.png"),
        transform: Transform::from_xyz(0.0, 0.0, 0.0),
        ..default()
    });

    // Spawn a physics body
    commands.spawn((
        RigidBody::Dynamic,
        Collider::circle(16.0),
        Velocity::default(),
        SpriteBundle {
            texture: assets.load("ball.png"),
            ..default()
        },
    ));
}

fn move_player(
    input: Res<Input>,
    mut query: Query<&mut Transform, With<Player>>,
    time: Res<Time>,
) {
    for mut transform in query.iter_mut() {
        let speed = 200.0 * time.delta_seconds();
        if input.key_held(KeyCode::ArrowRight) {
            transform.translation.x += speed;
        }
        if input.key_held(KeyCode::ArrowLeft) {
            transform.translation.x -= speed;
        }
    }
}
```

### Sample Usage (JavaScript Embedding)

```html
<script type="module">
  import { Arachne } from './arachne.js';

  const app = new Arachne('#my-canvas', {
    width: 800,
    height: 600,
    features: ['renderer-2d', 'physics-2d'],
  });

  // Load a scene
  await app.loadScene('/scenes/demo.json');

  // Or build programmatically
  const ball = app.spawn({
    sprite: { color: '#ff6b6b', radius: 16 },
    rigidBody: 'dynamic',
    collider: { type: 'circle', radius: 16 },
    transform: { x: 400, y: 100 },
  });

  app.onUpdate((dt) => {
    // Custom logic each frame
  });

  app.start();
</script>
```

---

## 1) Architecture

```
                    ┌──────────────────────────────────────────────────────┐
                    │                    Arachne                           │
                    │                                                      │
   App / Runner ──► │  ┌──────────┐  ┌──────────┐  ┌───────────────────┐  │
                    │  │  Scene    │  │  Plugin  │  │  JS Binding       │  │
                    │  │  Graph    │  │  System  │  │  (wasm-bindgen)   │  │
                    │  └─────┬────┘  └─────┬────┘  └────────┬──────────┘  │
                    │        │             │                 │             │
                    │  ┌─────▼─────────────▼─────────────────▼──────────┐  │
                    │  │              Systems Layer                      │  │
                    │  │  ┌────────┐ ┌────────┐ ┌───────┐ ┌──────────┐ │  │
                    │  │  │Renderer│ │Physics │ │ Audio │ │Animation │ │  │
                    │  │  │ 2D/3D  │ │  2D    │ │       │ │Skel/Tween│ │  │
                    │  │  └───┬────┘ └───┬────┘ └───┬───┘ └────┬─────┘ │  │
                    │  │  ┌───┤     ┌────┤          │          │       │  │
                    │  │  │UI │     │Net │     ┌────▼────┐     │       │  │
                    │  │  │IMGUI│   │work│     │Particles│     │       │  │
                    │  │  └────┘    └────┘     └─────────┘     │       │  │
                    │  └───────────────────┬───────────────────┘       │  │
                    │                      │                            │  │
                    │  ┌───────────────────▼───────────────────────────┐│  │
                    │  │                  ECS Core                      ││  │
                    │  │  World · Archetypes · Queries · Systems        ││  │
                    │  │  Resources · Events · Commands · Schedule      ││  │
                    │  └───────────────────┬───────────────────────────┘│  │
                    │                      │                            │  │
                    │  ┌──────────┐ ┌──────▼──────┐ ┌────────────────┐ │  │
                    │  │  Math    │ │   Asset     │ │    Input       │ │  │
                    │  │vec/mat/q │ │  Pipeline   │ │ unified K/M/T/G│ │  │
                    │  └──────────┘ └─────────────┘ └────────────────┘ │  │
                    │                                                      │
                    │  ┌──────────────────────────────────────────────────┐│
                    │  │              Platform Layer                       ││
                    │  │  wgpu (native) │ WebGPU/Canvas2D (WASM)          ││
                    │  │  cpal (native) │ WebAudio (WASM)                 ││
                    │  │  winit (native)│ Canvas events (WASM)            ││
                    │  └──────────────────────────────────────────────────┘│
                    └──────────────────────────────────────────────────────┘
```

---

## 2) Directory Structure

```
arachne/
├── Cargo.toml                         # Workspace root
├── arachne.h                          # C API header (optional FFI embedding)
│
├── crates/
│   ├── arachne-math/                  # ~3,000 lines
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── vec2.rs                # Vec2: f32 2D vector, all ops
│   │       ├── vec3.rs                # Vec3: f32 3D vector, all ops
│   │       ├── vec4.rs                # Vec4: f32 4D vector, SIMD hint
│   │       ├── mat3.rs                # Mat3: 3x3 matrix, 2D transforms
│   │       ├── mat4.rs                # Mat4: 4x4 matrix, projections
│   │       ├── quat.rs               # Quaternion: rotation, slerp
│   │       ├── transform.rs           # Transform: position + rotation + scale
│   │       ├── rect.rs                # Rect, AABB
│   │       ├── color.rs               # Color: RGBA f32, conversions (hex, HSL)
│   │       ├── random.rs              # Xoshiro256++ PRNG (no-std, deterministic)
│   │       └── fixed.rs               # Fixed-point math for deterministic physics
│   │
│   ├── arachne-ecs/                   # ~8,000 lines
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── world.rs               # World: entity + component storage root
│   │       ├── entity.rs              # Entity: generational ID (u32 index + u32 gen)
│   │       ├── component.rs           # Component trait + storage type registration
│   │       ├── archetype.rs           # Archetype: column-based storage of component arrays
│   │       ├── query.rs               # Query<(&A, &mut B), With<C>, Without<D>>
│   │       ├── system.rs              # System trait + IntoSystem + function systems
│   │       ├── schedule.rs            # Schedule: system ordering, dependency graph, stages
│   │       ├── resource.rs            # Res<T>, ResMut<T>: typed singleton storage
│   │       ├── event.rs               # EventWriter<T>, EventReader<T>: ring buffer events
│   │       ├── commands.rs            # Commands: deferred spawn/despawn/insert/remove
│   │       ├── bundle.rs              # Bundle: component group for ergonomic spawning
│   │       ├── change_detection.rs    # Changed<T>, Added<T> query filters
│   │       └── parallel.rs            # Optional: system parallelism (native only)
│   │
│   ├── arachne-input/                 # ~2,500 lines
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── keyboard.rs            # KeyCode enum, key state (pressed/held/released)
│   │       ├── mouse.rs               # Mouse buttons, position, delta, scroll
│   │       ├── touch.rs               # Multi-touch: touch ID, position, phase
│   │       ├── gamepad.rs             # Gamepad: axes, buttons, deadzone
│   │       ├── input_map.rs           # Action mapping: "jump" -> [Space, GamepadA, TouchTap]
│   │       └── platform.rs            # Platform-specific event translation
│   │
│   ├── arachne-asset/                 # ~4,000 lines
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── server.rs              # AssetServer: load, track, hot-reload (native)
│   │       ├── handle.rs              # Handle<T>: typed reference-counted asset handle
│   │       ├── loader.rs              # AssetLoader trait: async bytes -> typed asset
│   │       ├── cache.rs               # LRU asset cache with memory budget
│   │       ├── io.rs                  # IO backends: filesystem (native), fetch (WASM)
│   │       ├── image.rs               # Image loader: PNG decode (minipng), atlas packing
│   │       ├── mesh.rs                # Mesh loader: OBJ parser (minimal)
│   │       ├── scene.rs               # Scene loader: JSON scene format
│   │       └── bundle.rs              # Asset bundle: concatenated binary + manifest
│   │
│   ├── arachne-render/                # ~14,000 lines
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── context.rs             # RenderContext: wgpu device/queue/surface wrapper
│   │       ├── pipeline.rs            # Pipeline cache: hash pipeline desc -> cached pipeline
│   │       ├── texture.rs             # Texture atlas, dynamic texture array
│   │       ├── buffer.rs              # Vertex/index/uniform buffer management
│   │       ├── camera.rs              # Camera2d, Camera3d, viewport, projection
│   │       │
│   │       ├── render2d/
│   │       │   ├── mod.rs
│   │       │   ├── sprite.rs          # Sprite renderer: batched instanced quads
│   │       │   ├── text.rs            # Text renderer: SDF font atlas, glyph layout
│   │       │   ├── shape.rs           # Shape renderer: lines, rects, circles, polygons
│   │       │   ├── tilemap.rs         # Tilemap renderer: chunked tile layers
│   │       │   └── batch.rs           # 2D batch: sort by texture/depth, merge draw calls
│   │       │
│   │       ├── render3d/
│   │       │   ├── mod.rs
│   │       │   ├── mesh_render.rs     # Mesh renderer: vertex pulling, instancing
│   │       │   ├── material.rs        # PBR material: albedo, metallic, roughness, normal
│   │       │   ├── light.rs           # Point, directional, spot lights (max 8 forward)
│   │       │   ├── skybox.rs          # Cubemap skybox
│   │       │   └── shadow.rs          # Basic shadow map (directional light, single cascade)
│   │       │
│   │       ├── shaders/
│   │       │   ├── sprite.wgsl        # Instanced sprite shader
│   │       │   ├── text_sdf.wgsl      # SDF text rendering shader
│   │       │   ├── shape.wgsl         # Shape primitives shader
│   │       │   ├── mesh_pbr.wgsl      # PBR mesh shader
│   │       │   ├── shadow.wgsl        # Shadow map generation
│   │       │   ├── skybox.wgsl        # Skybox shader
│   │       │   ├── particle.wgsl      # GPU particle update + render
│   │       │   └── postprocess.wgsl   # Bloom, tonemap, FXAA
│   │       │
│   │       └── graph.rs               # Render graph: pass ordering, resource management
│   │
│   ├── arachne-physics/               # ~8,000 lines
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── world.rs               # PhysicsWorld: broad + narrow phase, solver
│   │       ├── rigid_body.rs          # RigidBody: mass, velocity, forces, integration
│   │       ├── collider.rs            # Collider shapes: circle, AABB, polygon, capsule
│   │       ├── broadphase.rs          # Spatial hash grid (cell size = largest collider)
│   │       ├── narrowphase.rs         # GJK+EPA for polygon pairs, circle-circle, AABB
│   │       ├── solver.rs              # Sequential impulse constraint solver
│   │       ├── constraint.rs          # Distance, revolute, prismatic joints
│   │       ├── material.rs            # PhysicsMaterial: friction, restitution
│   │       ├── spatial.rs             # Spatial queries: raycast, AABB query, point query
│   │       └── debug.rs               # Debug draw: collider outlines, contacts, AABB grid
│   │
│   ├── arachne-audio/                 # ~3,000 lines
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── mixer.rs               # Audio mixer: N channels -> stereo output
│   │       ├── source.rs              # AudioSource: PCM buffer, streaming
│   │       ├── decoder.rs             # WAV/OGG decode (stb_vorbis port or lewton)
│   │       ├── spatial.rs             # Spatial audio: distance attenuation, panning
│   │       ├── effect.rs              # Effects: reverb, low-pass, volume envelope
│   │       └── backend.rs             # Backend: cpal (native), WebAudio (WASM)
│   │
│   ├── arachne-animation/             # ~4,000 lines
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── tween.rs               # Tween: property animation with easing functions
│   │       ├── easing.rs              # 30+ easing functions (linear, ease-in/out/in-out, bounce, elastic)
│   │       ├── keyframe.rs            # Keyframe track: time -> value interpolation
│   │       ├── clip.rs                # AnimationClip: collection of keyframe tracks
│   │       ├── skeleton.rs            # Skeleton: bone hierarchy, bind pose
│   │       ├── skinning.rs            # Vertex skinning: bone weights + joint matrices
│   │       ├── animator.rs            # Animator: clip playback, blending, crossfade
│   │       └── state_machine.rs       # Animation state machine: states + transitions + conditions
│   │
│   ├── arachne-ui/                    # ~5,000 lines
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── context.rs             # UI context: input routing, focus, layout tree
│   │       ├── layout.rs              # Flexbox-like layout engine (subset)
│   │       ├── widget.rs              # Widget trait
│   │       ├── widgets/
│   │       │   ├── button.rs          # Button: text/icon, hover/press states
│   │       │   ├── label.rs           # Label: text display
│   │       │   ├── slider.rs          # Slider: horizontal/vertical, range
│   │       │   ├── checkbox.rs        # Checkbox / toggle
│   │       │   ├── textinput.rs       # Text input: cursor, selection, clipboard
│   │       │   ├── panel.rs           # Panel: scrollable container
│   │       │   ├── dropdown.rs        # Dropdown select
│   │       │   └── image.rs           # Image display widget
│   │       ├── style.rs               # Style: colors, spacing, fonts, themes
│   │       └── render.rs              # UI render: generates 2D draw commands
│   │
│   ├── arachne-particles/             # ~3,000 lines
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── emitter.rs             # ParticleEmitter: spawn rate, shape, burst
│   │       ├── particle.rs            # Particle: position, velocity, age, color, size
│   │       ├── module.rs              # Modules: gravity, noise, color-over-life, size-over-life
│   │       ├── sim_cpu.rs             # CPU simulation fallback (WASM)
│   │       ├── sim_gpu.rs             # GPU compute simulation (native, wgpu compute)
│   │       └── render.rs              # Particle rendering: billboard quads, instanced
│   │
│   ├── arachne-net/                   # ~4,000 lines
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── client.rs              # Network client: connect, send, receive
│   │       ├── server.rs              # Lightweight relay server (native only)
│   │       ├── transport.rs           # Transport trait: WebSocket, WebRTC data channel
│   │       ├── websocket.rs           # WebSocket impl (tungstenite native, web-sys WASM)
│   │       ├── webrtc.rs              # WebRTC data channel (native via webrtc-rs, web-sys WASM)
│   │       ├── protocol.rs            # Binary protocol: message framing, compression
│   │       ├── sync.rs                # State sync: snapshot + delta compression
│   │       └── lobby.rs               # Simple matchmaking: rooms, join/leave
│   │
│   ├── arachne-scene/                 # ~3,000 lines
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── graph.rs               # Scene graph: parent-child hierarchy
│   │       ├── transform_prop.rs      # Transform propagation: local -> world
│   │       ├── visibility.rs          # Visibility: frustum culling, layer masks
│   │       ├── prefab.rs              # Prefab: serialized entity template
│   │       └── serialize.rs           # Scene serialize/deserialize (JSON)
│   │
│   ├── arachne-app/                   # ~5,000 lines
│   │   ├── Cargo.toml
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── app.rs                 # App builder: plugin registration, system scheduling
│   │       ├── plugin.rs              # Plugin trait: configure app on startup
│   │       ├── runner.rs              # Runner: main loop (requestAnimationFrame WASM, winit native)
│   │       ├── time.rs                # Time resource: delta, elapsed, fixed timestep
│   │       ├── default_plugins.rs     # DefaultPlugins: bundles common plugins
│   │       └── diagnostic.rs          # FPS counter, frame time histogram, system profiling
│   │
│   └── arachne-wasm/                  # ~4,000 lines
│       ├── Cargo.toml
│       └── src/
│           ├── lib.rs
│           ├── bindings.rs            # wasm-bindgen JS API surface
│           ├── canvas.rs              # Canvas setup, resize observer, DPI handling
│           ├── events.rs              # DOM event -> Arachne input translation
│           ├── audio_backend.rs       # WebAudio API backend
│           ├── fetch.rs               # fetch() based asset loading
│           └── js_api.rs              # High-level JS API (spawn, loadScene, onUpdate)
│
├── tests/
│   ├── integration/
│   │   ├── ecs_stress.rs              # 100K entity spawn/query/despawn throughput
│   │   ├── physics_determinism.rs     # Same inputs -> same outputs (100 frame sim)
│   │   ├── renderer_correctness.rs    # Render pipeline produces expected output
│   │   ├── full_app_lifecycle.rs      # App start -> update -> shutdown, no leaks
│   │   ├── scene_roundtrip.rs         # Save scene -> load scene -> identical state
│   │   └── wasm_size.rs              # Assert WASM bundle sizes under budget
│   │
│   └── benchmarks/
│       ├── ecs_bench.rs               # Entity/component/query throughput
│       ├── render_bench.rs            # Draw call batching, sprite throughput
│       ├── physics_bench.rs           # Broadphase, narrowphase, solver throughput
│       ├── math_bench.rs              # Vec/mat/quat operation throughput
│       └── frame_budget.rs            # Full frame under 16.6ms at target load
│
├── examples/
│   ├── hello_triangle/                # Minimal: window + triangle
│   ├── sprite_demo/                   # 2D sprites, input, basic movement
│   ├── physics_playground/            # 2D physics: spawn shapes, watch them collide
│   ├── particle_fireworks/            # Particle emitters, color-over-life
│   ├── product_configurator/          # 3D mesh + material swapping + orbit camera
│   ├── platformer/                    # Simple 2D platformer game
│   ├── ui_showcase/                   # All UI widgets
│   └── multiplayer_pong/              # WebSocket-based 2-player pong
│
├── web/                               # WASM embedding demo
│   ├── index.html                     # Single-page demo
│   ├── embed_example.html             # Minimal embedding (<script> tag only)
│   └── build.sh                       # WASM build + wasm-opt + size report
│
└── tools/
    ├── size_budget.sh                 # Assert WASM sizes under budget
    ├── frame_budget.sh                # Run frame timing benchmarks
    └── asset_compiler/                # Offline asset processing (texture atlas, mesh opt)
        ├── Cargo.toml
        └── src/main.rs
```

---

## 3) Key Trait Boundaries

### arachne-ecs: World + Query + System

```rust
// --- Entity ---
#[derive(Copy, Clone, Eq, PartialEq, Hash)]
pub struct Entity {
    index: u32,
    generation: u32,
}

// --- World ---
pub struct World {
    entities: EntityAllocator,
    archetypes: Vec<Archetype>,
    resources: ResourceMap,
    event_queues: EventQueues,
}

impl World {
    pub fn spawn(&mut self) -> EntityBuilder<'_>;
    pub fn despawn(&mut self, entity: Entity) -> bool;
    pub fn get<T: Component>(&self, entity: Entity) -> Option<&T>;
    pub fn get_mut<T: Component>(&mut self, entity: Entity) -> Option<&mut T>;
    pub fn query<Q: WorldQuery>(&self) -> QueryIter<'_, Q>;
    pub fn resource<T: Resource>(&self) -> &T;
    pub fn resource_mut<T: Resource>(&mut self) -> &mut T;
    pub fn send_event<T: Event>(&mut self, event: T);
}

// --- Component + Resource + Event ---
pub trait Component: 'static + Send + Sync {}
pub trait Resource: 'static + Send + Sync {}
pub trait Event: 'static + Send + Sync + Clone {}

// --- Query ---
pub trait WorldQuery {
    type Item<'a>;
    type Fetch: for<'a> Fetch<'a, Item = Self::Item<'a>>;
}

// Queries compose: Query<(&Position, &mut Velocity), With<Player>, Without<Dead>>
pub struct Query<'w, Q: WorldQuery, F: QueryFilter = ()> { /* ... */ }

impl<'w, Q: WorldQuery, F: QueryFilter> Query<'w, Q, F> {
    pub fn iter(&self) -> impl Iterator<Item = Q::Item<'_>>;
    pub fn iter_mut(&mut self) -> impl Iterator<Item = Q::Item<'_>>;
    pub fn get(&self, entity: Entity) -> Option<Q::Item<'_>>;
    pub fn single(&self) -> Q::Item<'_>;
}

// --- System ---
pub trait System: Send + Sync {
    fn run(&mut self, world: &mut World);
    fn name(&self) -> &str;
}

// Function systems via IntoSystem:
// fn my_system(query: Query<&Position>, time: Res<Time>) { ... }
pub trait IntoSystem<Params>: Send + Sync + 'static {
    type System: System;
    fn into_system(self) -> Self::System;
}
```

### arachne-render: RenderContext + DrawCommand

```rust
pub struct RenderContext {
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface: wgpu::Surface,
    pipeline_cache: PipelineCache,
    texture_atlas: TextureAtlas,
}

pub enum DrawCommand {
    Sprite {
        texture: TextureHandle,
        instances: Vec<SpriteInstance>,
    },
    Mesh {
        mesh: MeshHandle,
        material: MaterialHandle,
        transform: Mat4,
    },
    Shape {
        shape: ShapeData,
        color: Color,
        transform: Mat4,
    },
    Text {
        text: String,
        font: FontHandle,
        size: f32,
        color: Color,
        position: Vec2,
    },
}

pub trait Renderable: Send + Sync {
    fn draw_commands(&self, world: &World) -> Vec<DrawCommand>;
}
```

### arachne-physics: PhysicsWorld + Collider

```rust
pub struct PhysicsWorld {
    bodies: Vec<RigidBodyData>,
    colliders: Vec<ColliderData>,
    broadphase: SpatialHashGrid,
    contacts: Vec<ContactManifold>,
    constraints: Vec<Box<dyn Constraint>>,
}

impl PhysicsWorld {
    pub fn step(&mut self, dt: f32);
    pub fn add_body(&mut self, body: RigidBody, collider: Collider) -> BodyHandle;
    pub fn remove_body(&mut self, handle: BodyHandle);
    pub fn raycast(&self, origin: Vec2, dir: Vec2, max_dist: f32) -> Option<RayHit>;
    pub fn query_aabb(&self, aabb: Rect) -> Vec<BodyHandle>;
}

#[derive(Clone)]
pub enum ColliderShape {
    Circle { radius: f32 },
    AABB { half_extents: Vec2 },
    Polygon { vertices: Vec<Vec2> },
    Capsule { half_height: f32, radius: f32 },
}

pub struct ContactManifold {
    pub body_a: BodyHandle,
    pub body_b: BodyHandle,
    pub normal: Vec2,
    pub depth: f32,
    pub points: [ContactPoint; 2],
    pub point_count: u8,
}

pub trait Constraint: Send + Sync {
    fn solve(&mut self, bodies: &mut [RigidBodyData], dt: f32);
}
```

### arachne-app: App + Plugin

```rust
pub struct App {
    world: World,
    schedule: Schedule,
    plugins: Vec<Box<dyn Plugin>>,
    runner: Box<dyn Runner>,
}

impl App {
    pub fn new() -> Self;
    pub fn add_plugin<P: Plugin>(&mut self, plugin: P) -> &mut Self;
    pub fn add_system<S, P>(&mut self, system: S) -> &mut Self
    where
        S: IntoSystem<P>;
    pub fn add_startup_system<S, P>(&mut self, system: S) -> &mut Self
    where
        S: IntoSystem<P>;
    pub fn insert_resource<T: Resource>(&mut self, resource: T) -> &mut Self;
    pub fn run(self);
}

pub trait Plugin: Send + Sync {
    fn build(&self, app: &mut App);
    fn name(&self) -> &str;
}

pub trait Runner: Send + Sync {
    fn run(&mut self, app: App);
}
```

---

## 4) Size Budgets

**These are HARD pass/fail thresholds. If a WASM build exceeds its size
budget, the task FAILS.**

| Artifact | Budget | How to Measure |
|----------|--------|----------------|
| `arachne-core.wasm` (ECS + math + input) | **<200KB** | `wasm-opt -Oz` + `wc -c` |
| `arachne-2d.wasm` (core + 2D renderer) | **<350KB** | Same |
| `arachne-full.wasm` (all features) | **<900KB** | Same |
| JS wrapper (`arachne.js`) | **<30KB** | `terser` + `wc -c` |
| Total embedding payload (full + JS) | **<1MB** | Combined |

### Frame Budgets (at target load)

| Scenario | Budget | Target |
|----------|--------|--------|
| 10,000 2D sprites, sorted + batched | **<16.6ms** total frame | 60fps |
| 1,000 physics bodies, step + render | **<16.6ms** total frame | 60fps |
| Physics step alone, 1,000 bodies | **<4ms** | 25% frame budget |
| 2D render alone, 10,000 sprites | **<8ms** | 50% frame budget |
| ECS query, 100K entities, 3 components | **<1ms** | <6% frame budget |
| UI layout + render, 200 widgets | **<2ms** | 12% frame budget |

---

## 5) Performance Thresholds

**These are HARD pass/fail thresholds. Every task with a benchmark must
include both a baseline and the optimized implementation.**

| # | Module | Metric | Threshold | How to Measure |
|---|--------|--------|-----------|----------------|
| 1 | `arachne-math` | Vec3 ops throughput | **>=200M ops/sec** | 1M iterations of add+mul+normalize |
| 2 | `arachne-math` | Mat4 multiply | **>=50M ops/sec** | 1M mat4*mat4 |
| 3 | `arachne-ecs` | Entity spawn | **>=500K/sec** | Spawn 100K entities with 3 components |
| 4 | `arachne-ecs` | Query iteration | **>=10M entities/sec** | Iterate Query<(&A, &B)> over 1M entities |
| 5 | `arachne-ecs` | Archetype move | **>=200K/sec** | Add component to 100K entities (archetype change) |
| 6 | `arachne-render` | Sprite batch | **>=100K sprites/frame** at 60fps | Render sorted sprites, count before frame drop |
| 7 | `arachne-render` | Draw call merging | **>=90% reduction** | 1000 sprites, 4 textures -> count draw calls |
| 8 | `arachne-physics` | Broadphase | **>=1M pair checks/sec** | Spatial hash, 1000 moving bodies |
| 9 | `arachne-physics` | Narrowphase GJK | **>=500K/sec** | Polygon-polygon collision test |
| 10 | `arachne-physics` | Full step, 1000 bodies | **<4ms** | Broadphase + narrow + solve + integrate |
| 11 | `arachne-audio` | Mixer, 32 channels | **<1ms** per 1024-sample buffer | Mix 32 sources to stereo |
| 12 | `arachne-particles` | CPU sim, 10K particles | **<2ms** per frame | Update + sort + render |
| 13 | `arachne-animation` | Skeletal, 100 bones | **<0.5ms** per character | Joint matrix computation |
| 14 | `arachne-ui` | Layout, 200 widgets | **<1ms** | Full layout pass |
| 15 | `arachne-scene` | Transform propagation, 10K nodes | **<0.5ms** | Hierarchy walk + world matrix |

### Correctness Thresholds

| Module | Requirement |
|--------|-------------|
| Math | All ops match `f64` reference within **1e-5** relative error |
| Physics | 100-frame sim produces **bit-identical** results across runs (determinism) |
| Physics | Energy conservation: total energy drift **<1%** over 1000 steps (elastic) |
| ECS | Zero entity ID reuse collisions over 1M spawn/despawn cycles |
| Renderer | Depth sorting: no Z-fighting in 2D sprite benchmark |
| Scene | Save/load roundtrip produces **identical** world state |
| Animation | Blend weights sum to **1.0 +/- 1e-6** |

---

## 6) Task Decomposition -- 28 Tasks

**CRITICAL RULES FOR EVERY TASK:**
1. Every task MUST write tests alongside implementation. No code without tests.
2. Every benchmark MUST include both baseline AND optimized implementation.
3. Every task MUST list its pass/fail criteria. The judge uses these to accept/reject.
4. Dependencies must be respected. A task cannot start until its dependencies complete.
5. No task exceeds ~5,000 lines. If it would, split it.

### Dependency Graph

```
WAVE 1 (fully parallel, zero dependencies):
  Task 1:  Math library (vec2/3/4, mat3/4, quat, transform, color, rect, PRNG)
  Task 2:  ECS core -- Entity, Component, Archetype storage
  Task 3:  ECS core -- World, Query, system parameter extraction

WAVE 2 (depends on Wave 1):
  Task 4:  ECS systems -- System trait, Schedule, stages ── depends: 2, 3
  Task 5:  ECS extras -- Resources, Events, Commands, Bundles ── depends: 2, 3
  Task 6:  Input system ── depends: 1
  Task 7:  Asset pipeline -- server, handles, loaders, cache ── depends: 1

WAVE 3 (depends on Waves 1-2):
  Task 8:  Render foundation -- wgpu context, pipeline cache, buffers ── depends: 1
  Task 9:  2D renderer -- sprites, shapes, text, batching ── depends: 1, 8
  Task 10: 3D renderer -- mesh, PBR material, lights ── depends: 1, 8
  Task 11: Physics 2D -- rigid bodies, collider shapes, integration ── depends: 1
  Task 12: Physics 2D -- broadphase, narrowphase (GJK/EPA) ── depends: 1, 11
  Task 13: Physics 2D -- solver, constraints, spatial queries ── depends: 11, 12
  Task 14: Audio system -- mixer, decoder, spatial, backends ── depends: 1

WAVE 4 (depends on Waves 1-3):
  Task 15: Scene graph -- hierarchy, transform propagation, visibility ── depends: 1, 4, 5
  Task 16: Animation -- tweens, easing, keyframes, clips ── depends: 1
  Task 17: Animation -- skeleton, skinning, animator, state machine ── depends: 1, 16
  Task 18: UI system -- layout engine, context, input routing ── depends: 1, 6, 9
  Task 19: UI widgets -- button, label, slider, checkbox, panel, text input ── depends: 18
  Task 20: Particle system -- emitter, modules, CPU sim ── depends: 1, 9
  Task 21: Particle system -- GPU compute sim (native) ── depends: 8, 20

WAVE 5 (integration):
  Task 22: App framework -- App, Plugin, Runner, Time, DefaultPlugins ── depends: 4, 5, 6, 7, 8
  Task 23: App integration -- wire renderer + physics + audio into App ── depends: 9, 13, 14, 22
  Task 24: Networking -- transport, protocol, WebSocket, sync ── depends: 5
  Task 25: WASM bindings -- JS API, canvas, DOM events, fetch ── depends: 22, 23

WAVE 6 (examples + polish):
  Task 26: Examples -- hello_triangle, sprite_demo, physics_playground ── depends: 23
  Task 27: Examples -- product_configurator, platformer, multiplayer_pong ── depends: 10, 23, 24, 25
  Task 28: Size + perf audit -- WASM size optimization, frame budget tests ── depends: all
```

---

### Task 1: Math Library

**Crate:** `arachne-math`

**Implement:**
- `vec2.rs`: Vec2 -- add, sub, mul, div, dot, cross (scalar), length, normalize,
  lerp, angle, rotate, distance, min, max. Ops traits (Add, Sub, Mul, Neg).
- `vec3.rs`: Vec3 -- same + cross product (Vec3).
- `vec4.rs`: Vec4 -- same, used for homogeneous coords and color.
- `mat3.rs`: Mat3 -- multiply, inverse, determinant, from_rotation,
  from_scale, from_translation (2D transforms), transpose.
- `mat4.rs`: Mat4 -- multiply, inverse, determinant, look_at, perspective,
  orthographic, from_rotation_x/y/z, from_translation, from_scale,
  from_quat, transpose.
- `quat.rs`: Quaternion -- from_axis_angle, from_euler, to_mat3, to_mat4,
  slerp, nlerp, conjugate, inverse, normalize, multiply.
- `transform.rs`: Transform struct (Vec3 position, Quat rotation, Vec3 scale),
  local_to_world (-> Mat4), compose, inverse.
- `rect.rs`: Rect (min/max), AABB, contains, intersects, union, expand.
- `color.rs`: Color (RGBA f32), from_hex, to_hex, from_hsl, to_hsl, lerp,
  premultiply, common constants (RED, GREEN, BLUE, WHITE, BLACK, TRANSPARENT).
- `random.rs`: Xoshiro256++ PRNG. Seed, next_u64, next_f32 (0..1),
  next_range_f32, next_range_i32, next_vec2_unit_circle, next_vec3_unit_sphere.
  **Deterministic: same seed = same sequence on all platforms.**
- `fixed.rs`: Fixed-point Q16.16 type for deterministic physics.
  Add, sub, mul, div, from_f32, to_f32. Sqrt via Newton-Raphson.

**Tests (mandatory):**
- Vec2/3/4: arithmetic, dot product, cross product, normalization, lerp.
  Edge cases: zero vector normalize -> zero, very large vectors.
- Mat3/4: multiply associativity, inverse * original = identity,
  determinant of identity = 1, projection matrix correctness.
- Quaternion: slerp interpolation, rotation of unit vector, from/to euler
  roundtrip, normalize idempotent.
- Transform: compose two transforms, local_to_world matrix correctness.
- Rect: intersection, containment, union.
- Color: hex roundtrip, HSL roundtrip, premultiply/unpremultiply.
- PRNG: determinism (same seed -> same 1000 values), distribution
  uniformity (chi-squared test on 100K samples).
- Fixed: arithmetic matches f32 within 1e-4 for values in [-1000, 1000].
- **Benchmark:** Vec3 ops throughput, Mat4 multiply throughput.

**Pass/fail:**
- All unit tests pass.
- **Vec3 ops: >=200M ops/sec** on native (1M iterations of add+mul+normalize).
- **Mat4 multiply: >=50M ops/sec** on native.
- PRNG determinism: **same seed produces identical sequence** on native + WASM.
- Fixed-point arithmetic within **1e-4** of f32 reference.
- `no_std` compatible (no allocations, no std dependency).

**Dependencies:** None.

---

### Task 2: ECS Core -- Entity + Component + Archetype

**Crate:** `arachne-ecs` (partial)

**Implement:**
- `entity.rs`: Entity (u32 index + u32 generation). EntityAllocator:
  allocate, deallocate, is_alive. Free list for recycled indices. Generation
  increments on deallocation to detect use-after-free.
- `component.rs`: Component trait. TypeId-based component registration.
  ComponentInfo: size, align, drop_fn. ComponentId (dense index).
- `archetype.rs`: Archetype struct: sorted component type set, column-based
  storage (Vec<u8> per component type, tightly packed). Entity-to-archetype
  mapping. Add/remove entities from archetypes. Component data access by
  column. Archetype edges: "if I add component C to this archetype, which
  archetype do I move to?" (cached transitions).

**Tests (mandatory):**
- Entity: allocate 1M entities, deallocate all, reallocate -- no ID collisions.
  Generation prevents stale handles.
- Archetype: create archetype for (Position, Velocity), add 10K entities,
  read back data correctly. Column access returns properly aligned data.
- Archetype edge: adding component triggers move to correct target archetype.
- **Benchmark:** Entity spawn throughput (100K with 3 components).

**Pass/fail:**
- All unit tests pass.
- Zero entity ID collisions over 1M spawn/despawn cycles.
- **Entity spawn: >=500K/sec** with 3 components.
- Archetype storage correctly aligned for all component types.
- No memory leaks (components with Drop are called on entity despawn).

**Dependencies:** None.

---

### Task 3: ECS Core -- World + Query

**Crate:** `arachne-ecs` (partial)

**Implement:**
- `world.rs`: World struct holding entities, archetypes, archetype graph.
  Spawn (with bundle), despawn, get/get_mut component, insert/remove component
  on existing entity.
- `query.rs`: Query<Q, F> where Q is a tuple of component references and
  F is a filter. WorldQuery trait with associated Fetch type. Implementations
  for: `&T` (immutable ref), `&mut T` (mutable ref), `Option<&T>`,
  `Entity` (yields entity ID). QueryFilter: `With<T>`, `Without<T>`.
  Query iteration: walks matching archetypes, iterates columns.

**Tests (mandatory):**
- World: spawn 5 entities with different component sets, query each set.
- Query: `Query<(&Position, &Velocity)>` returns only entities with both.
- Filter: `Query<&Position, Without<Dead>>` excludes dead entities.
- Mutation: `Query<&mut Velocity>` allows modification.
- Optional: `Query<(&Position, Option<&Name>)>` works for both named and unnamed.
- Entity query: `Query<(Entity, &Position)>` yields entity ID.
- **Benchmark:** Query iteration over 1M entities.

**Pass/fail:**
- All unit tests pass.
- **Query iteration: >=10M entities/sec** for `Query<(&A, &B)>` over 1M entities.
- Query correctly handles archetype changes mid-frame (deferred via Commands).
- Zero unsafe UB (test with Miri if feasible).

**Dependencies:** None (co-developed with Task 2, but can be tested independently
with mock archetype storage).

---

### Task 4: ECS Systems -- Schedule + Stages

**Crate:** `arachne-ecs` (partial)

**Implement:**
- `system.rs`: System trait (run + name). IntoSystem trait + implementations
  for function pointers with up to 8 parameters. SystemParam trait for
  extracting typed parameters from World (Query, Res, ResMut, EventReader,
  EventWriter, Commands). FunctionSystem wrapper that extracts params and calls
  the function.
- `schedule.rs`: Schedule struct: ordered list of systems. Stages (Startup,
  PreUpdate, Update, PostUpdate, Render). Topological sort within stages
  based on declared ordering (before/after). Cycle detection.
  apply_deferred() between stages to flush Commands.

**Tests (mandatory):**
- Function system with `Query<&Position>` correctly reads components.
- Function system with `Query<&mut Velocity>` correctly writes.
- System with `Res<Time>` reads resource.
- System ordering: system A runs before system B when declared.
- Cycle detection: error on circular dependency.
- Startup systems run once, Update systems run every frame.
- apply_deferred: commands from system A are visible to system B in next stage.
- **Benchmark:** Schedule execution overhead for 100 systems.

**Pass/fail:**
- All unit tests pass.
- System execution order respects declared dependencies.
- Cycle detection produces clear error message.
- **Schedule overhead: <0.1ms** for 100 empty systems.
- Commands properly deferred and applied between stages.

**Dependencies:** Tasks 2, 3.

---

### Task 5: ECS Extras -- Resources, Events, Commands, Bundles, Change Detection

**Crate:** `arachne-ecs` (partial)

**Implement:**
- `resource.rs`: ResourceMap: TypeId -> Box<dyn Any>. insert, get<T>, get_mut<T>,
  remove<T>. Res<T> and ResMut<T> system params.
- `event.rs`: EventQueue<T>: double-buffered ring buffer. EventWriter<T>
  pushes events. EventReader<T> iterates events from last frame. Automatic
  swap at frame boundary.
- `commands.rs`: Commands buffer: deferred Spawn, Despawn, Insert, Remove,
  InsertResource, SendEvent. EntityCommands for chaining operations on a
  single entity. Applied in apply_deferred().
- `bundle.rs`: Bundle trait: tuple of components. Derive-like manual impl
  for tuples up to 12 elements. Spawn with bundle.
- `change_detection.rs`: Changed<T> filter -- component was modified this frame.
  Added<T> filter -- component was added this frame. Tick-based tracking.

**Tests (mandatory):**
- Resources: insert, read, mutate, remove. Missing resource -> panic.
- Events: write 3 events, reader sees all 3, next frame reader sees 0.
  Multiple readers see same events.
- Commands: spawn via commands, verify entity exists after apply_deferred.
  Despawn via commands, verify entity gone after apply.
- Bundles: spawn with (Position, Velocity) bundle, query works.
- Change detection: modify Position, Changed<Position> query finds it.
  Added<Velocity> query finds newly inserted component.
- **Benchmark:** Event throughput (100K events/frame).

**Pass/fail:**
- All unit tests pass.
- Events correctly double-buffered (no lost events, no stale reads).
- Commands execute in order they were issued.
- Change detection has **zero false positives** and **zero false negatives**.
- **Event throughput: >=1M events/sec** (100K events, 10 readers).

**Dependencies:** Tasks 2, 3.

---

### Task 6: Input System

**Crate:** `arachne-input`

**Implement:**
- `keyboard.rs`: KeyCode enum (~100 keys). KeyState: pressed (this frame),
  held, released (this frame), idle. InputState<KeyCode>: HashMap<KeyCode, KeyState>.
  pressed(), held(), released(), just_pressed(), just_released().
- `mouse.rs`: MouseButton enum (Left, Right, Middle, X1, X2). Mouse position
  (Vec2), delta (Vec2), scroll (Vec2). Same pressed/held/released API.
- `touch.rs`: Touch struct (id, position, phase: Started/Moved/Ended/Cancelled).
  ActiveTouches: Vec<Touch>, indexed by ID. Multi-touch support.
- `gamepad.rs`: GamepadButton, GamepadAxis enums. Deadzone handling.
  Axis value in [-1, 1]. Connected/disconnected events.
- `input_map.rs`: ActionMap: maps string action names to input bindings.
  `action("jump").pressed()` checks Space OR GamepadA OR touch tap.
  Configurable by user at runtime.
- `platform.rs`: Trait for platform-specific event injection. WASM impl
  receives DOM events. Native impl receives winit events.

**Tests (mandatory):**
- Keyboard: press key -> just_pressed true frame 1, held true frame 2,
  release -> just_released true, then idle.
- Mouse: position update, button states, scroll delta.
- Touch: start -> move -> end lifecycle, multi-touch with 3 fingers.
- Gamepad: axis values, deadzone clipping.
- ActionMap: bind "jump" to [Space, GamepadA], verify both trigger action.
- Frame reset: all just_pressed/just_released clear after frame advance.

**Pass/fail:**
- All unit tests pass.
- Input state correctly transitions through press/hold/release lifecycle.
- ActionMap resolves to first matching binding.
- Touch IDs are stable across frames.
- Deadzone correctly clips small axis values to 0.0.

**Dependencies:** Task 1 (Vec2 for positions).

---

### Task 7: Asset Pipeline

**Crate:** `arachne-asset`

**Implement:**
- `handle.rs`: Handle<T>: typed reference to an asset. Strong handle (keeps
  asset alive) and weak handle. HandleId: u64 (path hash or UUID).
- `server.rs`: AssetServer: load(path) -> Handle<T>, get<T>(handle) -> Option<&T>,
  asset state tracking (Loading, Loaded, Failed). Load queue processes
  async IO results each frame.
- `loader.rs`: AssetLoader trait (file extensions, async load from bytes).
  Built-in loaders: ImageLoader (PNG), MeshLoader (OBJ), SceneLoader (JSON).
- `cache.rs`: LRU cache with configurable memory budget. Evicts least recently
  used assets when budget exceeded. Reference counting prevents eviction of
  in-use assets.
- `io.rs`: IO trait. NativeIO (std::fs). WasmIO (fetch API via wasm-bindgen).
  Both return `Future<Vec<u8>>`.
- `image.rs`: PNG decoder (use `png` crate, it's tiny). Image struct:
  width, height, RGBA8 pixel data. Atlas packer: greedy shelf algorithm,
  outputs packed atlas + UV rects.
- `mesh.rs`: Minimal OBJ parser: positions, normals, texcoords, faces.
  Index buffer generation.
- `scene.rs`: JSON scene format: array of entity descriptors with component
  data. Serialize + deserialize.
- `bundle.rs`: Asset bundle: concatenated binary blobs + manifest (offsets,
  sizes, types). Single-file distribution for WASM.

**Tests (mandatory):**
- Handle: create, clone, drop -> reference count tracks correctly.
- AssetServer: load PNG, verify it reaches Loaded state.
- LRU cache: fill to budget, load one more -> LRU evicted.
- PNG decode: load test image, verify dimensions and pixel values.
- OBJ parse: load cube, verify 8 vertices, 12 triangles.
- Scene roundtrip: save -> load -> identical entity layout.
- Atlas packer: pack 10 images, all UVs valid, no overlap.
- Bundle: create bundle from 3 assets, load back, verify all present.
- **Benchmark:** PNG decode throughput, atlas packing speed.

**Pass/fail:**
- All unit tests pass.
- Scene roundtrip preserves all component data.
- Atlas packer fits 100 sprites into a 2048x2048 atlas with **<5% wasted space**.
- LRU eviction is correct (verified by tracking eviction order).
- **PNG decode: >=100 images/sec** for 256x256 RGBA.

**Dependencies:** Task 1 (Vec2, Color).

---

### Task 8: Render Foundation -- wgpu Context + Pipeline Cache + Buffers

**Crate:** `arachne-render` (partial)

**Implement:**
- `context.rs`: RenderContext: initialize wgpu (instance, adapter, device, queue,
  surface). Surface configuration (format, present mode). Resize handling.
  Abstract over native (winit) and WASM (canvas element) surface creation.
- `pipeline.rs`: PipelineCache: hash RenderPipelineDescriptor -> cached
  wgpu::RenderPipeline. Shader module loading from embedded WGSL strings.
  Pipeline key: shader source hash + vertex layout + blend state + depth state.
- `buffer.rs`: BufferPool: reusable GPU buffers. DynamicBuffer: grows as
  needed, maps and writes data. UniformBuffer: typed uniform binding.
  VertexBuffer, IndexBuffer wrappers with proper usage flags.
- `texture.rs`: TextureHandle: index into texture array. Texture creation
  from Image (RGBA8). Dynamic texture atlas: add textures at runtime,
  get UV rects back. Texture bind group management.
- `camera.rs`: Camera2d (orthographic), Camera3d (perspective). ViewProjection
  matrix computation. Viewport rect. Screen-to-world / world-to-screen.

**Tests (mandatory):**
- Context: create context with headless adapter (wgpu supports this).
  Verify device capabilities.
- PipelineCache: create pipeline, request same again -> cache hit.
  Different desc -> cache miss -> new pipeline.
- BufferPool: allocate buffer, return to pool, allocate again -> reused.
  DynamicBuffer: write data, verify GPU contents (via map_read).
- Camera2d: screen corners map to expected world positions.
- Camera3d: perspective projection correctness (frustum corners).
- Texture: create from image, bind group valid.

**Pass/fail:**
- All unit tests pass.
- Pipeline cache **hit rate >= 95%** on a typical frame (measure in integration test).
- DynamicBuffer writes are correct (readback matches input).
- Camera projection matrices match glam/cgmath reference.
- No GPU validation errors (wgpu validation layer enabled in tests).

**Dependencies:** Task 1 (math).

---

### Task 9: 2D Renderer -- Sprites, Shapes, Text, Batching

**Crate:** `arachne-render` (partial)

**Implement:**
- `render2d/sprite.rs`: Sprite component: texture handle, color tint, flip_x/y,
  anchor point. SpriteRenderer: collects all sprites, sorts by Z + texture,
  batches into instanced draw calls. Single vertex buffer for a unit quad,
  per-instance data (transform, UV rect, color) in instance buffer.
- `render2d/shape.rs`: ShapeRenderer: lines, rectangles, circles (as triangle fans),
  filled polygons. Immediate-mode API: `shapes.rect(pos, size, color)`.
  Batched by shape type.
- `render2d/text.rs`: SDF font rendering. BMFont-compatible font atlas loader.
  Glyph layout: character advance, kerning (from font metrics), line wrapping.
  TextRenderer: generates quads for each glyph, batches by font texture.
- `render2d/batch.rs`: Batcher: sort draw commands by texture/shader/depth,
  merge consecutive commands with same state into single draw call.
  Draw call statistics.
- `shaders/sprite.wgsl`: Instanced sprite shader. Vertex: unit quad * instance
  transform. Fragment: sample texture atlas at instance UV, multiply tint color.
- `shaders/text_sdf.wgsl`: SDF text shader with adjustable edge softness.
- `shaders/shape.wgsl`: Solid color shapes, optional outline.

**Tests (mandatory):**
- Sprite: spawn 100 sprites, verify correct number of draw calls after batching.
  Sprites with same texture merge into one draw call.
- Sorting: sprites with Z=1 render behind Z=0 (depth correct).
- Shapes: draw rect, verify vertex positions. Draw circle, verify triangle count.
- Text: layout "Hello" with known font metrics, verify glyph positions.
- Batching: 1000 sprites, 4 textures -> exactly 4 draw calls (or fewer with atlas).
- **Benchmark:** Sprite throughput (render N sprites, measure frame time).

**Pass/fail:**
- All unit tests pass.
- **10,000 sprites render in <8ms** (50% frame budget).
- **Draw call reduction >=90%**: 1000 sprites with 4 textures -> <=10 draw calls.
- Text renders correctly: "HELLO WORLD" at multiple sizes, no glyph overlap.
- Depth sorting: no visual artifacts in layered sprite benchmark.

**Dependencies:** Tasks 1, 8.

---

### Task 10: 3D Renderer -- Mesh, PBR Material, Lights

**Crate:** `arachne-render` (partial)

**Implement:**
- `render3d/mesh_render.rs`: MeshRenderer: vertex pulling from vertex buffer.
  Vertex format: position(Vec3), normal(Vec3), texcoord(Vec2), tangent(Vec4).
  Index buffer support. Instanced rendering for multiple instances of same mesh.
- `render3d/material.rs`: PBR material: albedo (color or texture), metallic (f32),
  roughness (f32), normal map (optional), emissive (optional). MaterialHandle.
  Uniform buffer per material.
- `render3d/light.rs`: PointLight (position, color, intensity, range).
  DirectionalLight (direction, color, intensity). SpotLight (position, direction,
  angle, range). Max 8 lights (forward rendering). Light uniform buffer.
- `render3d/skybox.rs`: Cubemap texture loading. Skybox rendering (inverted cube,
  rendered first with depth disabled).
- `render3d/shadow.rs`: Basic directional shadow map. Single cascade, 2048x2048
  depth texture. Light-space projection. PCF sampling in main shader.
- `shaders/mesh_pbr.wgsl`: PBR shader: Cook-Torrance BRDF, Fresnel, GGX
  distribution, Smith geometry term. 8 lights forward. Normal mapping.
  Shadow sampling.
- `shaders/shadow.wgsl`: Depth-only pass for shadow map generation.
- `shaders/skybox.wgsl`: Sample cubemap, no lighting.

**Tests (mandatory):**
- Mesh: load cube, verify 36 indices (12 triangles * 3).
  Render cube, verify no wgpu validation errors.
- Material: create PBR material, set properties, verify uniform buffer.
- Lights: 1 directional + 2 point lights, verify uniform data packed correctly.
- Shadow: render shadow map, verify depth texture has reasonable values.
- Skybox: render skybox, verify it appears behind all geometry.
- **Benchmark:** Mesh rendering throughput (N instances of same mesh).

**Pass/fail:**
- All unit tests pass.
- PBR shader compiles and runs without validation errors.
- Shadow map: directional shadow produces visible shadow on floor plane.
- **1,000 mesh instances render in <8ms** (50% frame budget).
- No wgpu validation errors in any test.

**Dependencies:** Tasks 1, 8.

---

### Task 11: Physics 2D -- Rigid Bodies, Colliders, Integration

**Crate:** `arachne-physics` (partial)

**Implement:**
- `rigid_body.rs`: RigidBody types: Static, Dynamic, Kinematic. Properties:
  mass, inverse_mass, inertia, inverse_inertia, linear_velocity, angular_velocity,
  linear_damping, angular_damping, gravity_scale. Force/torque accumulator.
  Semi-implicit Euler integration. CCD option (sweep test for fast bodies).
- `collider.rs`: ColliderShape enum: Circle, AABB, Polygon (convex, max 8 vertices),
  Capsule. Collider: shape + offset from body + material (friction, restitution).
  AABB computation for each shape type (bounding box for broadphase).
- `material.rs`: PhysicsMaterial: friction (f32), restitution (f32).
  combine_friction (geometric mean), combine_restitution (max).
- `world.rs`: PhysicsWorld struct: body storage, collider storage, config
  (gravity, iterations, timestep). Fixed timestep accumulator with interpolation.

**Tests (mandatory):**
- RigidBody: apply force, integrate, verify position change matches F=ma.
  Angular: apply torque, verify rotation.
- Collider AABB: circle at (5,5) radius 2 -> AABB (3,3)-(7,7). Polygon AABB.
- Integration: free-fall under gravity, verify y(t) = 0.5*g*t^2 within 1e-3.
- Damping: body with damping slows to near-zero velocity.
- Static body: forces applied have no effect.
- Kinematic body: velocity set externally, not affected by forces.
- **Benchmark:** Integration step for 10K bodies.

**Pass/fail:**
- All unit tests pass.
- Free-fall position matches analytical solution within **1e-3** after 100 steps.
- Static bodies have **exactly zero** velocity after any number of steps.
- **Integration (10K bodies): <1ms**.
- Mass/inertia computation correct for all shape types (verified against formulas).

**Dependencies:** Task 1 (Vec2, math).

---

### Task 12: Physics 2D -- Broadphase + Narrowphase

**Crate:** `arachne-physics` (partial)

**Implement:**
- `broadphase.rs`: SpatialHashGrid: cell_size configurable (default: 2x largest
  collider). Insert AABB, query overlapping pairs. Update on body movement
  (remove + reinsert, or mark dirty). Grid uses HashMap<(i32,i32), Vec<BodyHandle>>.
  Returns candidate pairs (no duplicate pairs, no self-pairs).
- `narrowphase.rs`: Collision detection for all shape pair types:
  - Circle-Circle: distance < sum of radii.
  - Circle-AABB: closest point on AABB to circle center.
  - Circle-Polygon: GJK with circle support function.
  - AABB-AABB: overlap test.
  - AABB-Polygon: GJK.
  - Polygon-Polygon: GJK for intersection test, EPA for penetration depth + normal.
  - Capsule pairs: Minkowski sum with circle.
  ContactManifold: contact points (1-2), normal, penetration depth.
- GJK implementation: support function, simplex evolution (1D, 2D, 3D simplex),
  closest point on simplex. EPA: expand polytope to find penetration vector.

**Tests (mandatory):**
- Broadphase: 100 random bodies, brute-force check all pairs that spatial hash
  returns match actual AABB overlaps (no false negatives). Allow false positives.
- Circle-circle: overlapping, touching, separated. Verify normal direction.
- AABB-AABB: overlap, corner touch, separated.
- Polygon-polygon: triangle vs square, overlapping at known depth.
  GJK + EPA: verify penetration depth within 1e-3 of analytical.
- Capsule-circle: verify contact point correctness.
- Edge cases: zero-area polygon, coincident circles, perfectly aligned AABBs.
- **Benchmark:** Broadphase query, narrowphase GJK throughput.

**Pass/fail:**
- All unit tests pass.
- Broadphase: **zero false negatives** (every actual overlap is detected).
- GJK/EPA penetration depth within **1e-3** of analytical reference.
- **Broadphase: >=1M pair checks/sec** (spatial hash, 1000 bodies).
- **GJK: >=500K tests/sec** for polygon-polygon pairs.
- Contact normal always points from body A to body B.

**Dependencies:** Tasks 1, 11.

---

### Task 13: Physics 2D -- Solver, Constraints, Spatial Queries

**Crate:** `arachne-physics` (partial)

**Implement:**
- `solver.rs`: Sequential impulse solver. For each contact: compute relative
  velocity at contact point, compute impulse magnitude (Baumgarte stabilization
  for penetration correction), clamp impulse (non-negative for normal, friction
  cone for tangent). Iteration: default 8 solver iterations.
  Warm starting: cache impulse from previous frame, apply at start.
- `constraint.rs`: DistanceConstraint (fixed distance between two bodies).
  RevoluteConstraint (pin joint, shared point). PrismaticConstraint (slide along
  axis). All as impulse-based constraints in the solver loop.
- `spatial.rs`: Raycast: march through spatial hash cells along ray, test
  narrowphase for each candidate. Returns closest hit (point, normal, distance,
  body handle). AABB query: all bodies overlapping a rect. Point query: which
  body contains a point.
- `debug.rs`: Debug draw data generation: collider outlines as line segments,
  contact points + normals as lines, broadphase grid as rect outlines.
  Outputs Vec<DebugLine> for the renderer to consume.

**Tests (mandatory):**
- Solver: two circles collide, separate, verify no overlap after solve.
  Stack of 5 boxes: stable within 10 frames (no jitter > 0.01).
  Ball bouncing: restitution=1.0 -> height restored within 5%.
- Friction: box on slope, friction > slope_angle -> static. Less -> slides.
- DistanceConstraint: two bodies connected, verify distance maintained within 1e-3.
- RevoluteConstraint: pendulum swings correctly.
- Raycast: ray hits circle at expected point. Ray misses -> None.
  Multiple bodies along ray -> closest hit returned.
- AABB query: returns all bodies in region, none outside.
- Determinism: run 100-frame simulation twice with same inputs -> identical states.
- **Benchmark:** Full physics step (broad + narrow + solve) for 1000 bodies.

**Pass/fail:**
- All unit tests pass.
- **Full step, 1000 bodies: <4ms** (25% frame budget).
- Determinism: two runs produce **bit-identical** results.
- Energy conservation: elastic collision (restitution=1.0) loses **<1%** energy
  over 1000 steps.
- Stack of 5 boxes: stable (max body displacement <0.01 per frame after settling).
- Raycasts return correct closest hit in all test cases.

**Dependencies:** Tasks 11, 12.

---

### Task 14: Audio System

**Crate:** `arachne-audio`

**Implement:**
- `source.rs`: AudioSource: PCM f32 stereo buffer. Channels, sample_rate.
  Streaming: read chunks from decoder on demand.
- `decoder.rs`: WAV decoder: parse RIFF header, read PCM data (16-bit int to f32).
  OGG Vorbis decoder: use `lewton` crate (small, no-std friendly) or
  port stb_vorbis if too large for WASM.
- `mixer.rs`: AudioMixer: N active channels (max 32). Each channel: source,
  volume, pan, loop flag, playback position. Mix to stereo output buffer
  (1024 samples at 48kHz = ~21ms of audio). Volume per channel. Master volume.
  Fade in/out.
- `spatial.rs`: Spatial audio: listener position + orientation. Sound source
  position. Distance attenuation (linear, inverse, exponential).
  Stereo panning based on angle to listener.
- `effect.rs`: Low-pass filter (simple IIR). Volume envelope (ADSR).
  Optional reverb (Schroeder).
- `backend.rs`: Backend trait: submit audio buffer, configure sample rate/channels.
  NativeBackend: cpal output stream. WasmBackend: AudioWorklet or
  ScriptProcessorNode via web-sys.

**Tests (mandatory):**
- WAV decode: load test WAV, verify sample count and first 100 samples match.
- Mixer: mix two sources at 0.5 volume each, verify output = 0.5*a + 0.5*b.
- Panning: hard left -> right channel is silent. Center -> equal both channels.
- Spatial: source 10 units right -> pans right. Source far away -> quiet.
- Low-pass: white noise through low-pass at 1kHz, verify high frequencies attenuated.
- ADSR: verify envelope shape matches expected curve.
- **Benchmark:** Mix 32 channels to stereo, 1024 samples.

**Pass/fail:**
- All unit tests pass.
- WAV decode: sample values within **1e-6** of reference.
- **Mixer: <1ms** for 32 channels, 1024 samples.
- Spatial panning angles correct within **1 degree**.
- No audio glitches: output buffer always full (no underruns in 10-second test).
- ADSR envelope timing within **1ms** of specified durations.

**Dependencies:** Task 1 (Vec2/Vec3 for spatial).

---

### Task 15: Scene Graph -- Hierarchy, Transform Propagation, Visibility

**Crate:** `arachne-scene`

**Implement:**
- `graph.rs`: Parent and Children components. Set_parent, add_child, remove_child
  operations via Commands. Orphan handling (remove parent -> entity becomes root).
  DFS iteration over hierarchy.
- `transform_prop.rs`: Transform propagation system: walks hierarchy top-down,
  computes GlobalTransform = parent.global * local_transform. Dirty flag
  optimization: only recompute subtrees with changed local transforms.
- `visibility.rs`: Visibility component (Visible, Hidden, Inherited).
  ComputedVisibility: resolved visibility considering parent chain.
  Frustum culling for Camera2d (AABB test) and Camera3d (frustum planes).
  Layer masks: entity.layer & camera.layer_mask != 0 -> visible.
- `prefab.rs`: Prefab: serialized entity hierarchy template. Instantiate
  prefab -> spawns entity tree with correct parent-child relationships.
  Prefab can reference assets by path.
- `serialize.rs`: Scene serialization: traverse world, serialize entities +
  components + hierarchy to JSON. Deserialize: recreate entities with same
  relationships. Component registry for serialization (TypeId -> name + serialize fn).

**Tests (mandatory):**
- Hierarchy: parent with 3 children, reparent one child, verify structure.
- Transform propagation: parent at (10,0), child at local (5,0) -> global (15,0).
  Rotate parent 90 degrees -> child global position rotated.
- Dirty flags: modify leaf -> only that subtree recomputed, not entire tree.
- Visibility: hide parent -> all children hidden (Inherited).
  Show child explicitly -> still hidden if parent hidden.
- Frustum culling: 1000 entities, camera sees 200 -> 800 culled.
- Prefab: instantiate, verify all entities and relationships correct.
- Scene roundtrip: serialize 100-entity scene -> deserialize -> identical state.
- **Benchmark:** Transform propagation for 10K node hierarchy.

**Pass/fail:**
- All unit tests pass.
- Transform propagation correct: child global positions match hand-computed values.
- **Transform propagation (10K nodes): <0.5ms**.
- Frustum culling reduces draw count by expected amount.
- Scene roundtrip: **all components preserved** (verified by equality check).
- Dirty flags: modifying 1 leaf in 10K tree triggers **<10 recomputations**.

**Dependencies:** Tasks 1, 4, 5.

---

### Task 16: Animation -- Tweens, Easing, Keyframes, Clips

**Crate:** `arachne-animation` (partial)

**Implement:**
- `easing.rs`: 30+ easing functions: Linear, EaseInQuad, EaseOutQuad,
  EaseInOutQuad, EaseInCubic, EaseOutCubic, EaseInOutCubic, EaseInQuart,
  EaseOutQuart, EaseInOutQuart, EaseInSine, EaseOutSine, EaseInOutSine,
  EaseInExpo, EaseOutExpo, EaseInOutExpo, EaseInCirc, EaseOutCirc,
  EaseInOutCirc, EaseInElastic, EaseOutElastic, EaseInOutElastic,
  EaseInBounce, EaseOutBounce, EaseInOutBounce, EaseInBack, EaseOutBack,
  EaseInOutBack. All: fn(t: f32) -> f32 where t in [0, 1].
- `tween.rs`: Tween<T>: animates a value from start to end over duration
  with easing. Tween state: Playing, Paused, Completed. Loop modes:
  Once, Loop, PingPong. Tween chaining (sequence, parallel).
  TweenSystem: updates all active tweens each frame.
- `keyframe.rs`: Keyframe<T>: time + value + interpolation mode (Linear, Step,
  CubicBezier). KeyframeTrack<T>: sorted Vec<Keyframe<T>>, sample(time) -> T.
  Interpolation between keyframes using mode.
- `clip.rs`: AnimationClip: named collection of KeyframeTracks targeting
  different properties. Duration. Clip playback: time tracking, looping.

**Tests (mandatory):**
- Easing: all functions return 0.0 at t=0 and 1.0 at t=1. Monotonic for
  non-elastic/bounce. EaseInOutQuad(0.5) = 0.5.
- Tween: animate f32 from 0 to 100 over 1 second, verify value at t=0.5.
  Loop mode: value resets after duration. PingPong: reverses.
- Keyframe: 3 keyframes at t=0, t=1, t=2 with values 0, 10, 5.
  Sample at t=0.5 -> 5 (linear interp). Sample at t=1.5 -> 7.5.
  Step interpolation: sample at t=0.9 -> 0 (not 9).
- Clip: clip with position + rotation tracks, sample at time, verify both.
- Tween chaining: sequence of 3 tweens, verify each plays in order.
- **Benchmark:** Sample 1000 keyframe tracks.

**Pass/fail:**
- All unit tests pass.
- All easing functions **f(0) = 0** and **f(1) = 1** (within 1e-6).
- Keyframe interpolation within **1e-5** of hand-computed values.
- Tween completion fires exactly once.
- **1000 keyframe track samples: <0.1ms**.

**Dependencies:** Task 1 (math for interpolation).

---

### Task 17: Animation -- Skeleton, Skinning, Animator, State Machine

**Crate:** `arachne-animation` (partial)

**Implement:**
- `skeleton.rs`: Skeleton: bone hierarchy (parent indices), bind pose
  (local transforms per bone), inverse bind matrices. Bone names for
  targeting. Max 128 bones per skeleton.
- `skinning.rs`: SkinningData: per-vertex bone indices (max 4) + bone weights.
  Compute joint matrices: joint_matrix[i] = global_transform[i] * inv_bind[i].
  Vertex skinning: output_pos = sum(weight[j] * joint_matrix[bone[j]] * pos).
  CPU skinning (WASM) and GPU skinning (uniform buffer of joint matrices).
- `animator.rs`: Animator component: current clip, playback time, speed,
  blend weight. Animation blending: lerp between two poses by weight.
  Crossfade: transition between clips over duration.
- `state_machine.rs`: AnimationStateMachine: states (each with clip + speed),
  transitions (condition + duration + target state). Conditions: bool parameters,
  float comparisons, trigger (auto-reset). Evaluate transitions each frame,
  crossfade between states.

**Tests (mandatory):**
- Skeleton: 3-bone chain, compute global transforms from local.
  Verify bone 2 global = bone0 * bone1 * bone2 local.
- Skinning: vertex at (0,0,0), 100% weight to bone 1, bone 1 translated (5,0,0)
  -> vertex at (5,0,0).
- Blend: pose A has bone at 0 degrees, pose B at 90 degrees, blend 0.5 -> 45 degrees.
- Crossfade: start clip A, trigger transition to clip B, verify smooth blend.
- State machine: Idle -> Walk transition when "moving" parameter is true.
  Walk -> Run when "speed" > 5.0.
- **Benchmark:** Joint matrix computation for 100-bone skeleton.

**Pass/fail:**
- All unit tests pass.
- Skinned vertex positions within **1e-4** of reference.
- Blend weights always sum to **1.0 +/- 1e-6**.
- **100-bone joint matrices: <0.5ms**.
- State machine transitions fire exactly when conditions met.
- Crossfade produces smooth interpolation (no snapping).

**Dependencies:** Tasks 1, 16.

---

### Task 18: UI System -- Layout Engine + Context + Input Routing

**Crate:** `arachne-ui` (partial)

**Implement:**
- `context.rs`: UIContext: manages UI tree, input routing (hit testing),
  focus tracking (tab order), hover state, active (pressed) state.
  Processes input events -> UI events (Click, Hover, FocusIn, FocusOut).
- `layout.rs`: Flexbox-subset layout engine. Node: width, height, min/max size,
  padding, margin, flex_direction (Row, Column), flex_grow, flex_shrink,
  align_items, justify_content, gap. Layout algorithm: measure pass (compute
  intrinsic sizes) + arrange pass (assign positions). Absolute positioning option.
- `style.rs`: Style struct: background_color, border_color, border_width,
  border_radius, text_color, font_size, padding, margin, opacity. Theme:
  collection of named styles (default, hover, active, disabled). Theme switching.
- `render.rs`: UI render: traverse layout tree, generate 2D draw commands
  (rects, text, images). Clip rects for scrollable containers. Z-ordering
  (UI always on top of scene).

**Tests (mandatory):**
- Layout: row with 3 children (100px each), parent 300px -> no wrapping, each at
  correct x position. Column layout equivalent.
- Flex grow: 2 children with flex_grow 1 and 2 in 300px parent -> 100px and 200px.
- Padding/margin: verify spacing between elements.
- Hit testing: click at (50, 50), verify correct node receives event.
  Overlapping nodes: topmost receives event.
- Focus: Tab key cycles focus between focusable widgets.
- Clip: child extends beyond parent with overflow:hidden -> clipped.
- **Benchmark:** Layout pass for 200 widgets.

**Pass/fail:**
- All unit tests pass.
- Layout matches expected positions within **1px** for all test cases.
- Hit testing: **zero false positives** (click outside -> no event).
- Focus order: matches document order (DFS of UI tree).
- **Layout, 200 widgets: <1ms**.
- Clip rects correctly prevent rendering outside bounds.

**Dependencies:** Tasks 1, 6, 9.

---

### Task 19: UI Widgets

**Crate:** `arachne-ui` (partial)

**Implement:**
- `widgets/button.rs`: Button: text label, optional icon. States: normal,
  hover, pressed, disabled. on_click callback. Style per state.
- `widgets/label.rs`: Label: text display, alignment (left/center/right),
  wrapping, truncation with ellipsis.
- `widgets/slider.rs`: Slider: horizontal/vertical, min/max/step, current value.
  Drag handle. on_change callback. Visual: track + knob.
- `widgets/checkbox.rs`: Checkbox: checked/unchecked/indeterminate. on_change.
  Toggle variant (switch appearance).
- `widgets/textinput.rs`: Text input: single-line text field. Cursor (blinking),
  selection (shift+arrow or click-drag), copy/paste (clipboard API).
  on_change, on_submit (Enter key). Placeholder text.
- `widgets/panel.rs`: Panel: scrollable container. Vertical/horizontal scroll.
  Scroll bar (thin, auto-hide). Content masking.
- `widgets/dropdown.rs`: Dropdown: click to open option list, select one.
  Search/filter within options (for long lists).
- `widgets/image.rs`: Image widget: display texture with sizing modes
  (contain, cover, fill, none).

**Tests (mandatory):**
- Button: click fires on_click once. Disabled button ignores clicks.
  Hover state activates on mouse enter, deactivates on leave.
- Slider: drag from 0 to 100, verify value at midpoint = 50. Step snapping.
- Checkbox: click toggles. Indeterminate state renders differently.
- Text input: type "hello", verify value. Select all + delete -> empty.
  Cursor movement with arrow keys.
- Panel: scroll content larger than panel, verify visible region.
- Dropdown: open, select option, verify value changed, dropdown closes.
- All widgets respect disabled state.

**Pass/fail:**
- All unit tests pass.
- All widgets render correctly in all states (normal, hover, pressed, disabled).
- Text input cursor position is correct after all edit operations.
- Scroll panel: scrollbar reflects content ratio correctly.
- Dropdown: option list position doesn't overflow screen bounds.

**Dependencies:** Task 18.

---

### Task 20: Particle System -- Emitter, Modules, CPU Simulation

**Crate:** `arachne-particles` (partial)

**Implement:**
- `particle.rs`: Particle struct: position, velocity, age, lifetime, color,
  size, rotation. Pool: pre-allocated Vec<Particle> with free list.
- `emitter.rs`: ParticleEmitter: spawn_rate (particles/sec), burst (spawn N at
  once), shape (point, circle, rect, cone). Initial velocity: direction +
  speed range + spread angle. Lifetime range.
- `module.rs`: Particle modules (update particle properties over lifetime):
  - GravityModule: constant acceleration.
  - ColorOverLifeModule: color gradient from birth to death.
  - SizeOverLifeModule: size curve (linear, bezier).
  - VelocityOverLifeModule: speed multiplier curve.
  - NoiseModule: Perlin/Simplex noise displacement.
  - RotationModule: angular velocity.
- `sim_cpu.rs`: CPU particle simulation: for each particle, apply all modules,
  integrate position, kill expired particles. Sort by depth for rendering.
- `render.rs`: Particle rendering: generate billboard quad per particle,
  instance buffer with per-particle data (position, size, color, rotation).
  Additive and alpha blend modes.

**Tests (mandatory):**
- Emitter: spawn_rate = 100/sec, after 1 second, ~100 alive particles (+/- 5).
  Burst: emit 50 at once, verify 50 created.
- Gravity: particle falls, position.y matches 0.5*g*t^2 within 1e-2.
- ColorOverLife: at t=0 -> start color, at t=lifetime -> end color.
  Midpoint -> interpolated color.
- NoiseModule: particles diverge (verify variance increases over time).
- Pool: spawn 1000, kill 500, spawn 300 -> 800 alive, pool reused.
- Render: verify instance count matches alive particle count.
- **Benchmark:** CPU sim for 10K particles.

**Pass/fail:**
- All unit tests pass.
- Spawn rate within **+/- 5%** of target over 10 seconds.
- **CPU sim, 10K particles: <2ms** per frame.
- No memory allocation during steady-state (pool pre-allocated).
- Color/size gradients sample correctly at all t values.

**Dependencies:** Tasks 1, 9.

---

### Task 21: Particle System -- GPU Compute Simulation

**Crate:** `arachne-particles` (partial)

**Implement:**
- `sim_gpu.rs`: GPU particle simulation using wgpu compute shaders.
  Storage buffer: particle data (position, velocity, age, lifetime, color, size).
  Compute shader: one thread per particle, apply gravity + modules + integration.
  Dead particle compaction: prefix sum to remove dead particles.
  Double buffering: read from buffer A, write to buffer B, swap.
  Emit new particles: append to buffer via atomic counter.
- `shaders/particle.wgsl`: Compute shader for particle update.
  Vertex/fragment shader for particle rendering (billboard quads from
  compute buffer, no CPU readback).
- Fallback: if compute shaders unavailable (old WebGPU), fall back to CPU sim.
  Feature detection at init time.

**Tests (mandatory):**
- GPU sim matches CPU sim within 1e-3 for same initial conditions (100 particles,
  10 frames).
- Emit: spawn 1000 particles via GPU, verify count.
- Dead removal: kill 50% of particles, verify compacted buffer has correct count.
- Rendering: GPU-simulated particles produce correct instance count.
- Fallback: when compute disabled, CPU path activates automatically.
- **Benchmark:** GPU sim for 100K particles.

**Pass/fail:**
- All unit tests pass.
- GPU and CPU sim agree within **1e-3** (floating point order differences allowed).
- **100K particles (GPU): <1ms** compute dispatch.
- No GPU validation errors.
- Fallback activates correctly on capability check.

**Dependencies:** Tasks 8, 20.

---

### Task 22: App Framework -- App, Plugin, Runner, Time, DefaultPlugins

**Crate:** `arachne-app`

**Implement:**
- `app.rs`: App struct: World, Schedule, plugin list, runner. Builder pattern:
  add_plugin, add_system, add_startup_system, insert_resource, run.
  Plugin ordering: plugins register systems and resources.
- `plugin.rs`: Plugin trait: build(app). DefaultPlugins: registers Input, Time,
  AssetServer, RenderContext, AudioMixer. Feature-gated plugins: Physics2dPlugin,
  AudioPlugin, UIPlugin, AnimationPlugin, ParticlePlugin, NetworkPlugin.
- `runner.rs`: Runner trait. NativeRunner: winit event loop, polls events,
  runs schedule, presents frame. WasmRunner: requestAnimationFrame loop,
  processes DOM events, runs schedule, presents frame. Both share same
  Schedule execution logic.
- `time.rs`: Time resource: delta_seconds, elapsed_seconds, frame_count,
  fixed_timestep (for physics). Stopwatch, Timer utilities.
- `default_plugins.rs`: DefaultPlugins struct: adds core systems in correct order.
  Stage ordering: Input -> PreUpdate -> FixedUpdate (physics) -> Update ->
  PostUpdate (transform propagation) -> Render.
- `diagnostic.rs`: FPS counter, frame time histogram (last 120 frames),
  system execution time profiling. DiagnosticPlugin.

**Tests (mandatory):**
- App: create app, add plugin, add system, run for 1 frame -> system executed.
- Plugin: custom plugin registers resource, verify resource available.
- Time: after 5 frames at 60fps, elapsed ~= 0.083sec, delta ~= 0.0167.
- Runner: native runner processes 10 frames, verify frame_count = 10.
- Stage ordering: PreUpdate system runs before Update system.
- Diagnostics: FPS counter reports ~60fps when running at 60fps.
- **Benchmark:** Full frame overhead (app + schedule + empty systems).

**Pass/fail:**
- All unit tests pass.
- Plugin registration order is deterministic.
- Stage execution order matches specification.
- **Full frame overhead (empty app, no render): <0.5ms**.
- Time accumulates correctly over 1000 frames (drift <1ms).
- DefaultPlugins initializes all core resources.

**Dependencies:** Tasks 4, 5, 6, 7, 8.

---

### Task 23: App Integration -- Wire Renderer + Physics + Audio into App

**Crate:** `arachne-app` (integration)

**Implement:**
- Wire SpriteRenderer system into Render stage: queries all entities with
  Sprite + Transform, generates draw commands, submits to RenderContext.
- Wire Physics2d into FixedUpdate stage: step physics world, sync Transform
  components from physics body positions.
- Wire Audio into PostUpdate stage: update listener position, spatial sources.
- Wire Transform propagation into PostUpdate stage (before render).
- Wire Input update into PreUpdate stage.
- Wire AssetServer poll into PreUpdate stage.
- Spawn/despawn synchronization: ECS entity with RigidBody component auto-creates
  physics body. Despawn auto-removes. Same for audio sources.
- Camera system: update Camera ViewProjection uniform each frame.

**Tests (mandatory):**
- Spawn entity with (Sprite, Transform) -> renders on screen (verify draw call count > 0).
- Spawn entity with (RigidBody, Collider, Transform) -> physics simulates,
  Transform updates.
- Move Camera -> sprites render at different positions.
- Despawn physics entity -> body removed from physics world (no crash, no leak).
- Spawn AudioSource -> mixer has active channel.
- Full lifecycle: spawn 100 entities with mixed components, run 60 frames,
  despawn all, verify no leaks (entity count = 0, physics body count = 0).
- **Benchmark:** Full frame with 1000 sprites + 100 physics bodies.

**Pass/fail:**
- All unit tests pass.
- **Full frame (1000 sprites + 100 physics bodies): <16.6ms** (60fps).
- Zero leaked entities/bodies/sources after full lifecycle test.
- Transform sync: physics body position matches ECS Transform within 1e-6.
- Camera update: ViewProjection matches expected matrix.

**Dependencies:** Tasks 9, 13, 14, 22.

---

### Task 24: Networking -- Transport, Protocol, WebSocket, Sync

**Crate:** `arachne-net`

**Implement:**
- `transport.rs`: Transport trait: connect(url), send(bytes), recv() -> Option<bytes>,
  disconnect(). Async-friendly (poll-based for WASM compatibility).
- `websocket.rs`: WebSocket transport. Native: tungstenite (or tokio-tungstenite).
  WASM: web-sys WebSocket API. Auto-reconnect with exponential backoff.
- `webrtc.rs`: WebRTC data channel transport. Native: webrtc-rs. WASM: web-sys
  RTCPeerConnection. Signaling via WebSocket relay. Unreliable + reliable channels.
- `protocol.rs`: Binary protocol: message type (u8) + length (u16) + payload.
  Message types: Connect, Disconnect, StateUpdate, Input, Chat, Ping/Pong.
  Simple LZ4-like compression for state updates.
- `sync.rs`: State synchronization: snapshot (full state) + delta (changed fields
  only). Server authority: server sends authoritative state, client predicts +
  reconciles. Entity ID mapping (server IDs != client IDs).
- `client.rs`: NetworkClient resource: connect, send_message, poll_messages.
  Connection state: Connecting, Connected, Disconnected.
- `lobby.rs`: Simple room system: create_room, join_room, leave_room.
  Room state: player list, ready status.

**Tests (mandatory):**
- WebSocket: connect to echo server, send message, receive echo.
  Auto-reconnect: drop connection, verify reconnect attempt.
- Protocol: encode message, decode, verify identical payload.
  Compression: compress 1KB state, decompress, verify identical.
- Delta sync: state A, modify 2 fields, compute delta, apply to state A
  copy -> identical to state B.
- Snapshot: full state roundtrip (serialize, deserialize, verify).
- Lobby: create room, 2 clients join, verify player list.
- **Benchmark:** Protocol encode/decode throughput.

**Pass/fail:**
- All unit tests pass.
- Protocol roundtrip is **bit-exact** for all message types.
- Delta compression reduces typical update by **>=50%** vs full snapshot.
- **Protocol encode/decode: >=100K messages/sec**.
- WebSocket reconnect fires within 2 seconds of disconnection.
- No data corruption: 10K messages roundtrip with zero errors.

**Dependencies:** Task 5 (events for network events).

---

### Task 25: WASM Bindings -- JS API, Canvas, DOM Events

**Crate:** `arachne-wasm`

**Implement:**
- `bindings.rs`: wasm-bindgen entry point. Arachne class exposed to JS:
  new(canvas_selector, config), start(), stop(), resize(w, h).
- `canvas.rs`: Canvas setup: get canvas element, create wgpu surface from canvas.
  ResizeObserver: auto-resize on container change. DPI handling: devicePixelRatio.
- `events.rs`: DOM event listeners: keydown/keyup -> KeyCode mapping,
  mousemove/mousedown/mouseup -> mouse events, touchstart/touchmove/touchend ->
  touch events, gamepadconnected. Prevent defaults where appropriate.
  Pointer lock support (optional).
- `audio_backend.rs`: WebAudio backend: AudioContext, AudioWorkletNode (or
  ScriptProcessorNode fallback). Buffer: fill from mixer output.
  Handle browser autoplay restrictions (resume on first user gesture).
- `fetch.rs`: Fetch-based asset loading: fetch(url) -> ArrayBuffer -> Vec<u8>.
  Progress tracking for large assets. Error handling (404, network failure).
- `js_api.rs`: High-level JS API for non-Rust users:
  `app.spawn({ sprite: {...}, transform: {...} })` -> creates entity from JS object.
  `app.loadScene(url)` -> async scene loading.
  `app.onUpdate(callback)` -> registers per-frame JS callback.
  `app.query("Position", "Velocity")` -> returns entity data as JS arrays.
  Event listeners: `app.on("collision", callback)`.

**Tests (mandatory):**
- Canvas: create surface from canvas element (use wasm-bindgen-test + headless browser).
- Events: simulate keydown, verify Input resource updated.
- Fetch: load test asset via fetch, verify bytes match.
- JS API: spawn entity from JS, query it back, verify component data.
- Audio: create AudioContext, verify sample rate and channel count.
- DPI: devicePixelRatio = 2 -> canvas internal size is 2x display size.
- **WASM size:** measure final bundle.

**Pass/fail:**
- All wasm-bindgen-test tests pass.
- **Core WASM: <200KB** (wasm-opt -Oz).
- **Full WASM: <900KB** (all features enabled).
- **JS wrapper: <30KB** (terser minified).
- Canvas resize responds within 1 frame of container size change.
- Zero console errors during 60-second run.
- Browser autoplay restriction handled (no audio error on first load).

**Dependencies:** Tasks 22, 23.

---

### Task 26: Examples -- Core Demos

**Location:** `examples/`

**Implement:**
- `hello_triangle/`: Minimal: create app, spawn camera + colored triangle.
  Demonstrates: App, Renderer, basic shapes. <50 lines of user code.
- `sprite_demo/`: 2D sprites with input-driven movement. Load PNG sprites,
  animate position with keyboard input. Spawn 100 sprites at random positions.
  Demonstrates: sprites, input, asset loading, 2D camera.
- `physics_playground/`: Interactive 2D physics. Click to spawn circles/boxes
  that fall under gravity and collide. Walls at screen edges. Color by velocity.
  Demonstrates: physics, input, sprites, camera. Reset button (UI widget).
- `particle_fireworks/`: Click to spawn firework emitters. Particles explode
  outward, fade color over life, shrink and die. Multiple simultaneous fireworks.
  Demonstrates: particles, input, color-over-life, burst emission.

**Tests (mandatory):**
- Each example compiles for both native and WASM targets.
- Each example runs for 120 frames without panic or wgpu error.
- hello_triangle: produces at least 1 draw call per frame.
- physics_playground: 50 bodies, runs 300 frames, no physics explosion (max
  velocity < 10000).
- particle_fireworks: emit 5 bursts, verify particle count rises then falls.

**Pass/fail:**
- All examples compile and run on native.
- All examples compile for wasm32-unknown-unknown.
- **Zero panics** in 120-frame run for each example.
- Physics playground: stable simulation (no NaN, no explosion).
- User code for hello_triangle is **<50 lines**.

**Dependencies:** Task 23.

---

### Task 27: Examples -- Advanced Demos

**Location:** `examples/`

**Implement:**
- `product_configurator/`: 3D mesh (e.g., a chair or shoe) with orbit camera.
  UI panel with color swatches, material presets (matte, glossy, metallic).
  Click swatch -> material changes. Mouse drag -> orbit. Scroll -> zoom.
  Demonstrates: 3D renderer, PBR materials, UI, camera control, input.
- `platformer/`: Simple 2D platformer: player character with walk + jump.
  Tilemap level. Gravity, ground detection (raycast down from feet).
  Collectible coins (spawn, overlap detection, despawn). Score display (UI).
  Demonstrates: physics, tilemaps, sprites, animation (walk cycle), UI, input.
- `multiplayer_pong/`: 2-player pong over WebSocket. Server: relay input.
  Client: paddle movement (up/down), ball physics (simple, custom, no full
  physics engine needed). Score display. Win condition.
  Demonstrates: networking, input, UI, shapes.

**Tests (mandatory):**
- product_configurator: compiles native + WASM. Runs 120 frames. Material
  change updates uniform buffer.
- platformer: player jumps, lands on ground (raycast confirms ground).
  Coin collection: overlap -> coin despawned -> score incremented.
- multiplayer_pong: two clients connect (loopback), exchange input, ball
  moves consistently on both. (Integration test with local WebSocket server.)

**Pass/fail:**
- All examples compile native + WASM.
- **Zero panics** in 300-frame run for each example.
- product_configurator: orbit camera produces correct View matrix.
- platformer: player cannot fall through floor (ground detection works).
- multiplayer_pong: both clients see same ball position within 1 frame.

**Dependencies:** Tasks 10, 23, 24, 25.

---

### Task 28: Size + Performance Audit

**Location:** `tools/`, `tests/`

**Implement:**
- `tools/size_budget.sh`: Build WASM targets with `--release` + `wasm-opt -Oz`,
  measure and report sizes. Assert under budget.
- `tools/frame_budget.sh`: Run each benchmark, collect timing, assert under budget.
- `tests/integration/wasm_size.rs`: Programmatic WASM size check (parse .wasm
  file, assert byte count).
- `tests/benchmarks/frame_budget.rs`: Full-frame benchmark: 1000 sprites +
  100 physics bodies + 50 particles + 20 UI widgets. Assert <16.6ms.
- Profile and optimize hot paths identified in benchmarks:
  - ECS query iteration: ensure archetype iteration is cache-friendly.
  - Sprite batching: minimize sort overhead.
  - Physics broadphase: tune spatial hash cell size.
  - WASM size: identify and eliminate large dependencies.
- `tests/integration/full_app_lifecycle.rs`: Start app, run 600 frames
  (10 seconds at 60fps), spawn and despawn 1000 entities, verify zero leaks.
- Write a size breakdown report: which crates contribute how many KB to WASM.

**Tests (mandatory):**
- All WASM size budgets met (see Section 4).
- All frame budgets met (see Section 4).
- All performance thresholds met (see Section 5).
- Full lifecycle: zero leaks after 600 frames of spawn/despawn.
- Size breakdown report generated.

**Pass/fail:**
- **arachne-core.wasm: <200KB**.
- **arachne-2d.wasm: <350KB**.
- **arachne-full.wasm: <900KB**.
- **JS wrapper: <30KB**.
- **Full frame (1000 sprites + 100 physics + 50 particles + 20 UI): <16.6ms**.
- All 15 performance thresholds from Section 5 met.
- Zero memory leaks in lifecycle test.

**Dependencies:** All previous tasks.

---

## 7) Success Metrics

| Metric | Target |
|--------|--------|
| Tasks completed | 28/28 |
| Unit tests | All pass, 100% pass rate |
| Performance thresholds | All 15 hard thresholds met |
| WASM size budgets | All 4 size targets met |
| Frame budgets | All 6 scenarios under budget |
| Physics determinism | Bit-identical across runs |
| Memory safety | Zero leaks in lifecycle test |
| Rendering correctness | Zero wgpu validation errors |
| Total lines | ~100,000 (+/- 10K) |
| Test coverage | Every module has unit tests |
| Examples | 7 examples, all run on native + WASM |
| Embeddability | Single `<script>` tag embedding works |

---

## 8) Key Architectural Decisions

1. **Archetype-based ECS over sparse set.** Archetypes give cache-friendly
   iteration (all components of the same type are contiguous in memory).
   This matters for WASM where cache misses are expensive and we can't
   rely on hardware prefetching. Tradeoff: adding/removing components
   requires moving entities between archetypes (slower than sparse set
   for dynamic composition). Mitigation: cache archetype edges.

2. **wgpu as sole rendering backend.** wgpu compiles to Vulkan/Metal/DX12
   on native and WebGPU in the browser. This gives us one rendering
   codebase for all targets. Tradeoff: WebGPU browser support is still
   rolling out. Mitigation: fallback to Canvas2D for 2D-only mode
   (not in scope for v1, but API allows it).

3. **Fixed-timestep physics with interpolation.** Physics runs at fixed
   60Hz (or configurable). Rendering interpolates between physics states.
   This ensures determinism regardless of frame rate. Tradeoff: slight
   visual latency (1 physics step). Mitigation: interpolation smooths it.

4. **Immediate-mode UI.** IMGUI avoids the complexity of a retained DOM.
   Every frame, UI code declares what's visible. Layout is recomputed
   each frame. Tradeoff: can't diff/skip unchanged subtrees. Mitigation:
   200-widget layout in <1ms is our target, which is sufficient.

5. **No proc macros.** Proc macros bloat compile times and WASM output.
   Component/Bundle/Plugin derivation is done via manual trait impls or
   simple declarative macros. Tradeoff: more boilerplate. Mitigation:
   macro_rules! macros for common patterns.

6. **miniserde or manual serialization instead of serde.** serde adds
   ~50-100KB to WASM output. Scene serialization uses a custom JSON
   parser/writer or miniserde. Tradeoff: fewer format options. Mitigation:
   JSON is sufficient for scene files.

7. **Pre-allocated particle pool.** Particles are never individually
   allocated. A fixed pool (e.g., 10K particles) is allocated at emitter
   creation. Dead particles are recycled via free list. This prevents
   GC-like pauses and keeps WASM memory stable.

8. **Feature flags for everything.** Physics, audio, 3D rendering, UI,
   animation, particles, networking are all behind feature flags. The
   core (ECS + math + input) is ~200KB WASM. Users opt into only what
   they need. This is how we hit <500KB core and <1MB full.

9. **No runtime reflection.** Rust's TypeId is used for component
   registration, but there's no dynamic type inspection beyond that.
   This keeps the binary small and avoids RTTI overhead.

10. **Compile-time system parameter extraction.** IntoSystem uses Rust's
    type system to extract system parameters at compile time. No runtime
    type checking per frame. The Schedule knows each system's access
    pattern (which components read/written) at build time for future
    parallelization.

---

## 9) Build and Run Instructions

### Prerequisites
- Rust 1.75+ (edition 2021)
- wasm-pack (for WASM builds)
- wasm-opt (from binaryen, for WASM optimization)
- A WebGPU-capable browser (Chrome 113+, Firefox 121+)

### Build (Native)
```bash
# Build entire workspace
cargo build --release

# Run all tests
cargo test --all

# Run benchmarks
cargo bench

# Run specific example
cargo run --example sprite_demo --release
cargo run --example physics_playground --release
```

### Build (WASM)
```bash
# Build WASM bundle
cd crates/arachne-wasm
wasm-pack build --release --target web

# Optimize for size
wasm-opt -Oz -o arachne_bg_opt.wasm pkg/arachne_bg.wasm

# Check size budgets
./tools/size_budget.sh

# Serve locally
python3 -m http.server 8080 --directory web/
# Open http://localhost:8080
```

### Run Examples (WASM)
```bash
# Build all examples for WASM
./tools/build_wasm_examples.sh

# Serve
python3 -m http.server 8080 --directory web/
# Open http://localhost:8080/examples/sprite_demo/
```

---

## 10) WASM Size Reduction Strategies

These are enforced throughout development, not just at the end:

1. **No serde** -- use miniserde or manual JSON. Saves ~50-100KB.
2. **No regex** -- manual parsing. Saves ~50KB.
3. **No proc macros** -- use macro_rules! instead. Reduces codegen bloat.
4. **`#[cfg(target_arch = "wasm32")]`** -- strip native-only code paths.
5. **LTO = true** in release profile. Dead code elimination across crates.
6. **`opt-level = "z"`** for WASM (size over speed).
7. **`wasm-opt -Oz`** post-processing. Typically 10-30% reduction.
8. **Feature flags** -- only compile what's used.
9. **No format strings in hot paths** -- `core::fmt` adds significant WASM size.
10. **Panic = abort** in WASM -- no unwinding machinery.
11. **Minimal allocator** -- dlmalloc (default for WASM) is fine, no wee_alloc (unmaintained).
12. **Strip debug info** in release.

Each crate has a `wasm_size_test` that asserts its individual contribution
to the final WASM binary stays under its budget.