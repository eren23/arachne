# Arachne

**The engine that fits in a tweet's worth of bandwidth.**

Arachne is a sub-1MB embeddable interactive runtime engine written in Rust. It compiles to both native (via wgpu) and WebAssembly (<900KB), and includes everything you need for interactive 2D/3D content: an archetype-based ECS, sprite/shape/mesh rendering, 2D physics, spatial audio, animation, particles, UI, and networking.

Drop it into a webpage with one `<script>` tag. No 30MB Unity WebGL builds. No build tools for consumers.

## Quick Start

```bash
# Run an interactive example (opens a window)
cargo run -p arachne-examples --example hello_triangle --features windowed
cargo run -p arachne-examples --example physics_playground --features windowed
cargo run -p arachne-examples --example eren_game --features windowed

# Run all tests
cargo test --workspace

# Build for WASM
cargo build --target wasm32-unknown-unknown --release -p arachne-wasm
```

## Crate Overview

| Crate | Purpose |
|-------|---------|
| `arachne-math` | Vec2/3/4, Mat3/4, Quat, Transform, Color, PRNG, Fixed-point |
| `arachne-ecs` | Archetype-based ECS: World, Entity, Query, System, Schedule |
| `arachne-input` | Keyboard, Mouse, Touch, Gamepad, Action mapping |
| `arachne-render` | wgpu 2D/3D rendering: sprites, shapes, text, tilemaps, PBR meshes |
| `arachne-physics` | 2D deterministic physics: rigid bodies, colliders, GJK/EPA, solver |
| `arachne-audio` | Audio mixer, WAV/OGG decoding, spatial audio, effects |
| `arachne-animation` | Tweens, easing, keyframes, skeletal animation, state machines |
| `arachne-particles` | Particle emitters, CPU/GPU simulation |
| `arachne-ui` | Immediate-mode UI: flexbox layout, widgets |
| `arachne-scene` | Scene graph, transform propagation, visibility, prefabs |
| `arachne-app` | App framework, plugin system, runners (headless + windowed) |
| `arachne-networking` | WebSocket transport, binary protocol, client/server, lobby |
| `arachne-wasm` | WASM bindings, JS API, canvas, DOM events |
| `arachne-window` | Window management via winit |

## Examples

| Example | Description |
|---------|-------------|
| `hello_triangle` | Minimal: 3 colored sprites + text |
| `sprite_demo` | Arrow key movement, 100 sprites, camera follow |
| `physics_playground` | Click to spawn physics bodies, gravity, collisions |
| `particle_fireworks` | Burst particle emitters |
| `product_configurator` | 3D orbit camera, PBR material switching |
| `platformer` | Walk + jump, coins, score |
| `multiplayer_pong` | Networked pong game |
| `eren_game` | Tile-based adventure with rooms, dialogue, interactions |

## Size Budget

| Target | Budget |
|--------|--------|
| Core WASM (ECS + math + input) | <200KB |
| 2D WASM (core + renderer) | <350KB |
| Full WASM (all features) | <900KB |
| JS wrapper | <30KB |

## License

MIT
