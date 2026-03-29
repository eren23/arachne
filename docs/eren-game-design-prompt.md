# Prompt: Design "Eren's World" — A Personalized Portfolio Game

Copy everything below this line into Claude Desktop:

---

I need you to design a complete, personalized tile-based adventure game called **"Eren's World"** that will be embedded in my personal website as a portfolio piece. The game runs on the **Arachne engine** — a sub-1MB Rust game engine I built that compiles to WebAssembly (389KB).

This is NOT a hypothetical game. You are designing the actual room layouts, dialogue, game flow, and asset descriptions that will be directly implemented. Be specific, creative, and make it genuinely impressive.

---

## WHO I AM

**Eren Akbulut**
- Based in Karlsruhe, Germany
- Bio: "Human, talking to machines."
- Blog: blog.akbuluteren.com
- GitHub: github.com/eren23 (144 public repos, 65 followers)

**What I do:**
- AI/ML engineer and builder — everything from training models to shipping products
- Built attocode: a "0 to hero" AI coding agent building guide (14 stars)
- Created neo-unify: toy-scale unified multimodal model experiments on Apple Silicon (20 stars)
- Built SAM+CLIP+Diffusion pipeline for text-based image editing (15 stars)
- Created open_geo_spy: open-source geolocation AI (15 stars)
- Built one_layer_image_gen: PyTorch implementation of FAE paper (13 stars)
- Created crucible: autonomous ML research on rental GPUs with LLM-driven hypothesis generation (4 stars)
- Built antelligence: ant colony simulation with emergent collective intelligence (2 stars)
- Created an AI virtual pet (Tamagotchi-style) with Electron
- Built KnowledgeGPT: personal Q&A bot for documents
- Auto cover letter generator Chrome extension
- Face recognition API, Morse code translator via blink detection, OCR tools

**Tech stack across repos:**
- Primary: Python (39 repos), JavaScript (16), Jupyter (11), Rust (5), TypeScript (3), Go (2)
- Comfortable with: ML frameworks (PyTorch, MLX), web (React, FastAPI), systems (Rust), infra (Docker, cloud)

**Personality (from working with him):**
- Wants things that are actually impressive and showable, not just "technically works"
- Prefers direct communication, dislikes surface-level work
- Values practical results over theoretical completeness
- Builder mentality — ships fast, iterates, explores new ideas constantly

---

## WHAT THE ENGINE CAN RENDER (Hard Constraints)

The Arachne engine renders in a browser via WebGPU. Here's exactly what's available:

### Tiles (32 built-in types, procedurally generated 16x16px)

| Index | Name | Visual |
|-------|------|--------|
| 0 | Empty | Transparent |
| 1 | Grass | Green with procedural dark/light spots |
| 2 | Dirt | Brown earthy texture |
| 3 | Stone Wall | Gray stone with mortar lines at edges |
| 4 | Water | Blue-green with wave pattern (semi-transparent) |
| 5 | Wood Floor | Warm brown with horizontal grain |
| 6 | Door | Dark brown frame with gold handle |
| 7 | Brick Wall | Red-brown bricks with mortar grid |
| 8 | Dark Grass | Very dark green |
| 9 | Light Dirt | Lighter tan dirt |
| 10 | Dark Stone | Darker gray stone |
| 11 | Deep Water | Deep blue water |
| 12 | Light Wood | Light tan wood |
| 13 | Metal Door | Gray/blue metallic door |
| 14 | Mossy Brick | Brick with green moss patches |
| 15 | Sand | Sandy tan/beige |
| 16 | Snow | Bright white/blue-white |
| 17 | Ice | Light cyan (semi-transparent) |
| 18 | Lava | Orange/red/yellow gradient |
| 19 | Cobblestone | Stone pavers with visible edges |
| 20 | Gravel | Gray gravel |
| 21 | Planks | Wood planks with visible seams |
| 22 | Marble | White with subtle vein patterns |
| 23 | Dark Brick | Very dark brown brick |
| 24 | Red | Solid red |
| 25 | Green | Solid green |
| 26 | Blue | Solid blue |
| 27 | Yellow | Solid yellow |
| 28 | Magenta | Solid magenta |
| 29 | Cyan | Solid cyan |
| 30 | Light Gray | Solid light gray |
| 31 | Dark Gray | Solid dark gray |

Each tile can be flipped horizontally or vertically.

### Sprites
- Colored rectangles (any RGBA color, any size)
- Can be rotated (via quaternion)
- Can be flipped (X/Y)
- 9 anchor points (Center, TopLeft, etc.)
- Depth sorting via Z position
- NO sprite sheets / animation frames — sprites are solid-colored rectangles

### Text (ScreenTextBuffer)
- Built-in 5x7 pixel bitmap font (ASCII 32-127)
- Any font size (scales the base), any RGBA color
- Positioned at (x, y) screen coordinates
- Word wrapping via max_width
- Always axis-aligned (no rotation)
- Supports newlines

### Camera
- 2D camera with smooth lerp following
- Screen-to-world coordinate conversion

### Input
- Keyboard: pressed/just_pressed for all keys (WASD, arrows, E, Space, etc.)
- Mouse: position, click (left/right), just_pressed/released
- Touch: start/move/end with multi-touch IDs

### What CANNOT be done
- No real textures from files (WASM fetch not wired yet — all visuals are colored tiles/sprites)
- No audio (zero sound in WASM)
- No animation system (would need manual frame logic in update systems)
- No particle effects (exists but not wired to render pipeline)
- No text rotation
- No UI widgets rendering (layout exists but doesn't draw)
- No networking in WASM

---

## CURRENT GAME STRUCTURE

The game currently has:
- 20x15 tile grid per room (32px tiles = 640x480 viewport)
- Player: green 24x24 sprite, WASD/arrow movement, tile collision
- 3 rooms connected by Door tiles (walk into door = transition)
- Smooth camera following
- HUD text (room name, controls hint)
- Interaction system (E key near interactable tiles shows dialogue)
- Transition cooldown (prevents rapid room toggling)

Room transitions work by tile position: doors on room edges teleport to the connected room's entry point.

---

## WHAT I NEED FROM YOU

Design the complete game. Be specific and creative. Output:

### 1. Game Concept
What's the theme and story? How does visiting rooms feel like exploring Eren's world? What makes a visitor say "this is cool" and remember it?

### 2. Room Designs (minimum 5 rooms, up to 8)
For EACH room, provide:
- **Room name and theme** (e.g., "The Workshop" — Eren's project showcase)
- **Complete 20x15 tile grid** as a 2D array using tile indices from the table above. Be creative with tile choices — use lava, ice, water, marble, cobblestone, etc. to create varied environments.
- **Door placements** and which rooms they connect to (draw a room graph)
- **Interactable tile locations** and what dialogue they trigger
- **Decorative details** using creative tile combinations (e.g., a "desk" made of dark wood tiles, a "screen" made of cyan/blue tiles, a "garden" with grass/water)

### 3. Dialogue and Content
- Welcome text for each room
- Dialogue for each interactable object (actual text about Eren's projects, skills, personality)
- Easter eggs or hidden messages
- Make the dialogue voice match Eren's personality: direct, slightly playful, builder-mentality

### 4. Player and Visual Design
- What color should the player sprite be? Size?
- Any NPC sprites? (colored rectangles with different sizes/colors can represent things)
- Suggest additional colored sprites that should be placed in rooms to represent objects (computers, trophies, artifacts, etc.) — specify color, size, and position

### 5. Room Connection Graph
Draw the map layout — how rooms connect. Think about flow: a visitor should naturally explore the whole space.

### 6. Special Mechanics (within engine constraints)
- Any creative uses of the existing systems? (e.g., timed text sequences, interactive objects that change state, movement puzzles, speed zones using tile detection)
- Hidden rooms?
- Score or collection mechanic?

### 7. HUD and Polish
- What should always be on screen?
- How should room transitions feel?
- Loading/welcome screen text?

Be bold and creative. This is a portfolio piece that should show personality and technical skill. Use the full tile palette — don't just stick to wood/stone/grass. Make each room visually distinct and memorable.
