//! WASM canvas runtime — connects the Arachne engine to a browser `<canvas>`
//! element with a `requestAnimationFrame` loop and DOM event listeners.
//!
//! On native targets the runner executes a fixed number of frames as a stub,
//! allowing the type to be used in cross-platform code without `cfg` gymnastics.

use std::cell::RefCell;
use std::rc::Rc;

use arachne_app::runner::Runner;
use arachne_app::time::Time;
use arachne_ecs::{Schedule, World};
use arachne_input::platform::{InputSystem, PlatformInput};

#[allow(unused_imports)]
use crate::events::{DomEvent, EventTranslator, ArachneEvent};

// ---------------------------------------------------------------------------
// Event queue resource (shared between DOM listeners and ECS systems)
// ---------------------------------------------------------------------------

/// ECS resource wrapping the shared DOM event queue.
/// DOM listeners push events into this, and a PreUpdate system drains them.
pub struct DomEventQueue(pub Rc<RefCell<Vec<DomEvent>>>);
unsafe impl Send for DomEventQueue {}
unsafe impl Sync for DomEventQueue {}

// ---------------------------------------------------------------------------
// WasmRunner
// ---------------------------------------------------------------------------

/// A [`Runner`] that drives the engine from a browser `requestAnimationFrame`
/// loop and translates DOM events into Arachne input events.
///
/// On `wasm32` targets (with the `wasm` feature) this sets up a real rAF loop
/// and attaches DOM event listeners to the canvas. On native targets it falls
/// back to running a small number of frames so tests and tooling still work.
pub struct WasmRunner {
    /// DOM element ID of the target `<canvas>` (without `#` prefix).
    canvas_id: String,
    /// Target frames-per-second hint used for frame budget calculations.
    target_fps: u32,
    /// Number of frames to execute in the native-stub fallback.
    native_stub_frames: u64,
}

impl WasmRunner {
    /// Create a runner targeting the default canvas element (`arachne-canvas`).
    pub fn new() -> Self {
        Self {
            canvas_id: "arachne-canvas".to_string(),
            target_fps: 60,
            native_stub_frames: 1,
        }
    }

    /// Create a runner targeting a specific canvas element by DOM ID.
    pub fn with_canvas_id(id: &str) -> Self {
        Self {
            canvas_id: id.to_string(),
            ..Self::new()
        }
    }

    /// Set the target FPS hint (default 60).
    pub fn with_target_fps(mut self, fps: u32) -> Self {
        self.target_fps = fps;
        self
    }

    /// Set how many frames the native stub executes (default 1).
    pub fn with_native_stub_frames(mut self, n: u64) -> Self {
        self.native_stub_frames = n;
        self
    }

    /// The canvas DOM element ID this runner targets.
    pub fn canvas_id(&self) -> &str {
        &self.canvas_id
    }

    /// The configured target FPS.
    pub fn target_fps(&self) -> u32 {
        self.target_fps
    }
}

impl Default for WasmRunner {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// Runner implementation — WASM target
// ---------------------------------------------------------------------------

#[cfg(target_arch = "wasm32")]
impl Runner for WasmRunner {
    fn run(&mut self, world: &mut World, schedule: &mut Schedule) {
        // Shared event queue populated by DOM listeners, drained each frame.
        let event_queue: Rc<RefCell<Vec<DomEvent>>> = Rc::new(RefCell::new(Vec::new()));

        // Attach DOM event listeners that push into `event_queue`.
        setup_event_listeners(&self.canvas_id, Rc::clone(&event_queue));

        // Ensure core resources exist.
        if !world.has_resource::<InputSystem>() {
            world.insert_resource(InputSystem::new());
        }
        if !world.has_resource::<Time>() {
            world.insert_resource(Time::default());
        }

        // Insert event queue as an ECS resource so a system can drain it.
        world.insert_resource(DomEventQueue(Rc::clone(&event_queue)));

        // Add a PreUpdate system that drains DOM events into InputSystem.
        // This runs AFTER input_update_system (which calls begin_frame()),
        // so "just pressed" states survive until Update systems read them.
        schedule.add_system(arachne_ecs::Stage::PreUpdate, drain_dom_events_system);

        #[cfg(feature = "wasm")]
        {
            use wasm_bindgen::prelude::*;
            use wasm_bindgen::JsCast;

            let canvas_id = self.canvas_id.clone();

            // Spawn the async initialization and rAF loop.
            // SAFETY: World and Schedule are owned by the caller and live on the
            // WASM heap until page unload. We extend their lifetimes to 'static
            // for use in the rAF closure. This matches the pattern used by wgpu's
            // own WASM examples.
            let world_ptr = world as *mut World;
            let schedule_ptr = schedule as *mut Schedule;

            wasm_bindgen_futures::spawn_local(async move {
                let world: &'static mut World = unsafe { &mut *world_ptr };
                let schedule: &'static mut Schedule = unsafe { &mut *schedule_ptr };

                // Get the canvas element.
                let window = web_sys::window().expect("no global window");
                let document = window.document().expect("no document");
                let canvas = document
                    .get_element_by_id(&canvas_id)
                    .expect("canvas not found");
                let canvas: web_sys::HtmlCanvasElement =
                    canvas.dyn_into().expect("not a canvas");

                let width = canvas.width();
                let height = canvas.height();

                // Create wgpu surface from canvas and initialize GPU resources.
                let surface_target = wgpu::SurfaceTarget::Canvas(canvas);
                let context = arachne_render::RenderContext::new_with_surface(
                    surface_target, width, height,
                )
                .await
                .expect("failed to create WebGPU render context");

                // Use the shared GPU init to create all pipelines and resources.
                arachne_app::gpu_init::init_gpu_resources(world, &context);
                world.insert_resource(
                    arachne_app::gpu_init::RenderContextResource(context),
                );

                // Enter the requestAnimationFrame loop.
                let event_queue_clone = event_queue;
                let last_time: Rc<RefCell<f64>> = Rc::new(RefCell::new(0.0));

                let f: Rc<RefCell<Option<Closure<dyn FnMut(f64)>>>> =
                    Rc::new(RefCell::new(None));
                let g = Rc::clone(&f);

                *g.borrow_mut() = Some(Closure::new(move |timestamp: f64| {
                    // Compute delta time in seconds.
                    let mut last = last_time.borrow_mut();
                    let dt = if *last == 0.0 {
                        1.0 / 60.0
                    } else {
                        ((timestamp - *last) / 1000.0) as f32
                    };
                    *last = timestamp;
                    drop(last);

                    // DOM events are now drained by drain_dom_events_system
                    // in PreUpdate (after begin_frame), so we don't process
                    // them here. Just update time and run the schedule.

                    // Update time.
                    {
                        let time = world.get_resource_mut::<Time>();
                        time.update(dt);
                    }

                    // Run ECS schedule.
                    schedule.run(world);

                    // Render frame (camera upload + present).
                    render_wasm_frame(world);

                    // Schedule next frame.
                    let window = web_sys::window().unwrap();
                    let _ = window.request_animation_frame(
                        f.borrow().as_ref().unwrap().as_ref().unchecked_ref(),
                    );
                }));

                // Kick off the first frame.
                let window = web_sys::window().unwrap();
                let _ = window.request_animation_frame(
                    g.borrow().as_ref().unwrap().as_ref().unchecked_ref(),
                );
            });
        }

        #[cfg(not(feature = "wasm"))]
        {
            // Without the `wasm` feature, fall back to a single tick.
            process_queued_events(world, &event_queue);
            let delta = 1.0 / self.target_fps.max(1) as f32;
            {
                let time = world.get_resource_mut::<Time>();
                time.update(delta);
            }
            schedule.run(world);
        }
    }
}

// ---------------------------------------------------------------------------
// Runner implementation — native stub
// ---------------------------------------------------------------------------

#[cfg(not(target_arch = "wasm32"))]
impl Runner for WasmRunner {
    fn run(&mut self, world: &mut World, schedule: &mut Schedule) {
        eprintln!(
            "[arachne-wasm] WasmRunner running as native stub ({} frames, canvas_id='{}')",
            self.native_stub_frames, self.canvas_id,
        );

        if !world.has_resource::<InputSystem>() {
            world.insert_resource(InputSystem::new());
        }
        if !world.has_resource::<Time>() {
            world.insert_resource(Time::default());
        }

        let delta = 1.0 / self.target_fps.max(1) as f32;

        for _ in 0..self.native_stub_frames {
            {
                let time = world.get_resource_mut::<Time>();
                time.update(delta);
            }
            schedule.run(world);
        }
    }
}

// ---------------------------------------------------------------------------
// Event queue helpers
// ---------------------------------------------------------------------------

/// Drain queued [`DomEvent`]s, translate them via [`EventTranslator`], and feed
/// the resulting input events into the world's [`InputSystem`] resource.
fn process_queued_events(
    world: &mut World,
    event_queue: &Rc<RefCell<Vec<DomEvent>>>,
) {
    let raw_events: Vec<DomEvent> = event_queue.borrow_mut().drain(..).collect();

    if raw_events.is_empty() {
        return;
    }

    let mut translator = EventTranslator::new();
    let mut translated: Vec<ArachneEvent> = Vec::new();
    for raw in &raw_events {
        translated.extend(translator.translate(raw));
    }

    let input = world.get_resource_mut::<InputSystem>();
    // Don't call begin_frame() here — the ECS input_update_system handles
    // frame transitions. Calling it here would clear "just pressed" state
    // before game systems see it.

    for event in &translated {
        match event {
            ArachneEvent::KeyDown(code) => {
                input.process_event(arachne_input::platform::InputEvent::KeyDown(*code));
            }
            ArachneEvent::KeyUp(code) => {
                input.process_event(arachne_input::platform::InputEvent::KeyUp(*code));
            }
            ArachneEvent::MouseButtonDown(btn, _pos) => {
                input.process_event(arachne_input::platform::InputEvent::MouseDown(*btn));
            }
            ArachneEvent::MouseButtonUp(btn, _pos) => {
                input.process_event(arachne_input::platform::InputEvent::MouseUp(*btn));
            }
            ArachneEvent::MouseMove { position, .. } => {
                input.process_event(arachne_input::platform::InputEvent::MouseMove(*position));
            }
            ArachneEvent::MouseScroll(delta) => {
                input.process_event(arachne_input::platform::InputEvent::MouseScroll(*delta));
            }
            ArachneEvent::TouchStart { id, position } => {
                input.process_event(arachne_input::platform::InputEvent::TouchStart {
                    id: *id,
                    position: *position,
                });
            }
            ArachneEvent::TouchMove { id, position } => {
                input.process_event(arachne_input::platform::InputEvent::TouchMove {
                    id: *id,
                    position: *position,
                });
            }
            ArachneEvent::TouchEnd { id, position } => {
                input.process_event(arachne_input::platform::InputEvent::TouchEnd {
                    id: *id,
                    position: *position,
                });
            }
            ArachneEvent::TouchCancel { id, position } => {
                input.process_event(arachne_input::platform::InputEvent::TouchCancel {
                    id: *id,
                    position: *position,
                });
            }
            ArachneEvent::Resize { width, height, .. } => {
                input.process_event(arachne_input::platform::InputEvent::WindowResize {
                    width: *width as f32,
                    height: *height as f32,
                });
            }
            ArachneEvent::FocusChanged(focused) => {
                if *focused {
                    input.process_event(arachne_input::platform::InputEvent::FocusGained);
                } else {
                    input.process_event(arachne_input::platform::InputEvent::FocusLost);
                }
            }
            ArachneEvent::PointerLockChanged(_) => {
                // Pointer lock state is tracked by the EventTranslator; no
                // direct InputEvent equivalent.
            }
        }
    }
}

/// ECS system that drains DOM events into InputSystem each frame.
/// Runs in PreUpdate AFTER input_update_system's begin_frame(), so
/// "just pressed" states are visible to Update systems.
fn drain_dom_events_system(
    mut input: arachne_ecs::ResMut<InputSystem>,
    queue: arachne_ecs::Res<DomEventQueue>,
) {
    let raw_events: Vec<DomEvent> = queue.0.borrow_mut().drain(..).collect();
    if raw_events.is_empty() {
        return;
    }

    let mut translator = EventTranslator::new();
    for raw in &raw_events {
        for event in translator.translate(raw) {
            match event {
                ArachneEvent::KeyDown(code) => {
                    input.process_event(arachne_input::platform::InputEvent::KeyDown(code));
                }
                ArachneEvent::KeyUp(code) => {
                    input.process_event(arachne_input::platform::InputEvent::KeyUp(code));
                }
                ArachneEvent::MouseButtonDown(btn, _) => {
                    input.process_event(arachne_input::platform::InputEvent::MouseDown(btn));
                }
                ArachneEvent::MouseButtonUp(btn, _) => {
                    input.process_event(arachne_input::platform::InputEvent::MouseUp(btn));
                }
                ArachneEvent::MouseMove { position, .. } => {
                    input.process_event(arachne_input::platform::InputEvent::MouseMove(position));
                }
                ArachneEvent::MouseScroll(delta) => {
                    input.process_event(arachne_input::platform::InputEvent::MouseScroll(delta));
                }
                ArachneEvent::TouchStart { id, position } => {
                    input.process_event(arachne_input::platform::InputEvent::TouchStart { id, position });
                }
                ArachneEvent::TouchMove { id, position } => {
                    input.process_event(arachne_input::platform::InputEvent::TouchMove { id, position });
                }
                ArachneEvent::TouchEnd { id, position } => {
                    input.process_event(arachne_input::platform::InputEvent::TouchEnd { id, position });
                }
                ArachneEvent::TouchCancel { id, position } => {
                    input.process_event(arachne_input::platform::InputEvent::TouchCancel { id, position });
                }
                ArachneEvent::Resize { width, height, .. } => {
                    input.process_event(arachne_input::platform::InputEvent::WindowResize {
                        width: width as f32, height: height as f32,
                    });
                }
                ArachneEvent::FocusChanged(focused) => {
                    if focused {
                        input.process_event(arachne_input::platform::InputEvent::FocusGained);
                    } else {
                        input.process_event(arachne_input::platform::InputEvent::FocusLost);
                    }
                }
                ArachneEvent::PointerLockChanged(_) => {}
            }
        }
    }
}

/// Render a frame on WASM: upload camera, draw sprites/tilemaps/text, present.
///
/// Mirrors the `render_frame()` method from the WindowedRunner but simplified
/// for the WASM path. Called each rAF tick after schedule.run().
#[cfg(all(target_arch = "wasm32", feature = "wasm"))]
fn render_wasm_frame(world: &mut World) {
    use arachne_app::gpu_init::{
        GpuResources, RenderContextResource, SpritePipelineResource, TilemapPipelineResource,
    };
    use arachne_app::systems::{
        SpriteRendererResource, TextRendererResource, TextureStorageResource,
        TilemapRendererResource,
    };

    // Upload camera matrix to GPU.
    if world.has_resource::<GpuResources>()
        && world.has_resource::<arachne_render::Camera2d>()
        && world.has_resource::<RenderContextResource>()
    {
        let cam = world.get_resource::<arachne_render::Camera2d>();
        let vp = cam.view_projection();
        let uniform = arachne_render::CameraUniform::from_mat4(&vp);
        let gpu = world.get_resource::<GpuResources>();
        let ctx = world.get_resource::<RenderContextResource>();
        ctx.0.queue().write_buffer(&gpu.camera_buffer, 0, bytemuck::bytes_of(&uniform));
    }

    if !world.has_resource::<RenderContextResource>() {
        return;
    }

    let ctx = world.get_resource_mut::<RenderContextResource>();
    let mut frame = match arachne_render::RenderFrame::begin(&mut ctx.0) {
        Some(f) => f,
        None => return,
    };
    let queue = ctx.0.queue().clone();

    let mut surface_cleared = false;

    // Render tilemaps (background layer).
    if world.has_resource::<TilemapRendererResource>()
        && world.has_resource::<TilemapPipelineResource>()
        && world.has_resource::<GpuResources>()
        && world.has_resource::<TextureStorageResource>()
    {
        let tilemap_res = world.get_resource::<TilemapRendererResource>();
        let tilemap_pipeline = world.get_resource::<TilemapPipelineResource>();
        let gpu = world.get_resource::<GpuResources>();
        let tex_store = world.get_resource::<TextureStorageResource>();

        if tilemap_res.last_prepared.index_count > 0 {
            let atlas_bg = tex_store.0.get_bind_group(tilemap_res.atlas_texture);
            {
                let mut pass = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("tilemap_pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &frame.surface_view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color {
                                r: 0.1, g: 0.1, b: 0.15, a: 1.0,
                            }),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    ..Default::default()
                });
                tilemap_res.renderer.render(
                    &mut pass,
                    &tilemap_res.last_prepared,
                    &tilemap_pipeline.0,
                    &gpu.camera_bind_group,
                    atlas_bg,
                );
            }
            surface_cleared = true;
        }
    }

    // Render sprites on top.
    if world.has_resource::<SpriteRendererResource>()
        && world.has_resource::<SpritePipelineResource>()
        && world.has_resource::<GpuResources>()
        && world.has_resource::<TextureStorageResource>()
    {
        let srr = world.get_resource::<SpriteRendererResource>();
        let pipeline_res = world.get_resource::<SpritePipelineResource>();
        let gpu = world.get_resource::<GpuResources>();
        let tex_store = world.get_resource::<TextureStorageResource>();

        let load_op = if surface_cleared {
            wgpu::LoadOp::Load
        } else {
            wgpu::LoadOp::Clear(wgpu::Color {
                r: 0.1, g: 0.1, b: 0.15, a: 1.0,
            })
        };

        {
            let mut pass = frame.encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("sprite_pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &frame.surface_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: load_op,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                ..Default::default()
            });

            if !srr.last_batches.is_empty() {
                pass.set_pipeline(&pipeline_res.0);
                pass.set_bind_group(0, Some(&gpu.camera_bind_group), &[]);
                pass.set_vertex_buffer(0, srr.renderer.quad_vertex_buffer().slice(..));
                pass.set_vertex_buffer(1, srr.renderer.instance_buffer_slice());
                pass.set_index_buffer(
                    srr.renderer.quad_index_buffer().slice(..),
                    wgpu::IndexFormat::Uint16,
                );

                for batch in &srr.last_batches {
                    let tex_handle = batch.texture;
                    let bg = if (tex_handle.0 as usize) < tex_store.0.count() {
                        tex_store.0.get_bind_group(tex_handle)
                    } else {
                        tex_store.0.get_bind_group(arachne_render::TextureHandle(0))
                    };
                    pass.set_bind_group(1, Some(bg), &[]);
                    pass.draw_indexed(
                        0..6,
                        0,
                        batch.instance_offset..batch.instance_offset + batch.instance_count,
                    );
                }
            }
        }
    }

    // Render text on top.
    if world.has_resource::<TextRendererResource>()
        && world.has_resource::<RenderContextResource>()
    {
        let ctx = world.get_resource::<RenderContextResource>();
        let device = ctx.0.device().clone();

        let text_res = world.get_resource_mut::<TextRendererResource>();
        let prepared = text_res.renderer.prepare(&device, &queue);
        text_res.last_prepared = arachne_render::TextPrepared {
            vertex_count: prepared.vertex_count,
            index_count: prepared.index_count,
        };

        if prepared.index_count > 0 {
            frame.render_text(
                &text_res.pipeline,
                &text_res.camera_bind_group,
                &text_res.font_bind_group,
                &text_res.renderer,
                &prepared,
            );
        }
    }

    frame.present(&queue);
}

/// Register DOM event listeners that push [`DomEvent`]s into the shared queue.
///
/// On WASM with the `wasm` feature this attaches real `addEventListener` calls
/// for keyboard, mouse, wheel, touch, and resize events. On native this is a
/// no-op.
pub fn setup_event_listeners(
    _canvas_id: &str,
    _event_queue: Rc<RefCell<Vec<DomEvent>>>,
) {
    #[cfg(all(target_arch = "wasm32", feature = "wasm"))]
    {
        use crate::events::DomEventKind;
        use wasm_bindgen::prelude::*;
        use wasm_bindgen::JsCast;

        let window = web_sys::window().expect("no global window");
        let document = window.document().expect("no document");
        let canvas = document
            .get_element_by_id(_canvas_id)
            .expect("canvas element not found");
        let canvas: web_sys::HtmlCanvasElement = canvas
            .dyn_into()
            .expect("element is not a canvas");

        // --- Keyboard events (on window, not canvas) ---
        {
            let eq = Rc::clone(&_event_queue);
            let closure = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
                eq.borrow_mut().push(DomEvent {
                    kind: DomEventKind::Key {
                        code: e.code(),
                        pressed: true,
                        repeat: e.repeat(),
                    },
                    prevent_default: true,
                });
                e.prevent_default();
            });
            window.add_event_listener_with_callback("keydown", closure.as_ref().unchecked_ref()).ok();
            closure.forget();
        }
        {
            let eq = Rc::clone(&_event_queue);
            let closure = Closure::<dyn FnMut(web_sys::KeyboardEvent)>::new(move |e: web_sys::KeyboardEvent| {
                eq.borrow_mut().push(DomEvent {
                    kind: DomEventKind::Key {
                        code: e.code(),
                        pressed: false,
                        repeat: false,
                    },
                    prevent_default: true,
                });
            });
            window.add_event_listener_with_callback("keyup", closure.as_ref().unchecked_ref()).ok();
            closure.forget();
        }

        // --- Mouse events (on canvas) ---
        {
            let eq = Rc::clone(&_event_queue);
            let closure = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                eq.borrow_mut().push(DomEvent {
                    kind: DomEventKind::MouseButton {
                        button: e.button() as u16,
                        pressed: true,
                        client_x: e.client_x() as f64,
                        client_y: e.client_y() as f64,
                    },
                    prevent_default: false,
                });
            });
            canvas.add_event_listener_with_callback("mousedown", closure.as_ref().unchecked_ref()).ok();
            closure.forget();
        }
        {
            let eq = Rc::clone(&_event_queue);
            let closure = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                eq.borrow_mut().push(DomEvent {
                    kind: DomEventKind::MouseButton {
                        button: e.button() as u16,
                        pressed: false,
                        client_x: e.client_x() as f64,
                        client_y: e.client_y() as f64,
                    },
                    prevent_default: false,
                });
            });
            canvas.add_event_listener_with_callback("mouseup", closure.as_ref().unchecked_ref()).ok();
            closure.forget();
        }
        {
            let eq = Rc::clone(&_event_queue);
            let closure = Closure::<dyn FnMut(web_sys::MouseEvent)>::new(move |e: web_sys::MouseEvent| {
                eq.borrow_mut().push(DomEvent {
                    kind: DomEventKind::MouseMove {
                        client_x: e.client_x() as f64,
                        client_y: e.client_y() as f64,
                        movement_x: e.movement_x() as f64,
                        movement_y: e.movement_y() as f64,
                    },
                    prevent_default: false,
                });
            });
            canvas.add_event_listener_with_callback("mousemove", closure.as_ref().unchecked_ref()).ok();
            closure.forget();
        }

        // --- Wheel event (on canvas) ---
        {
            let eq = Rc::clone(&_event_queue);
            let closure = Closure::<dyn FnMut(web_sys::WheelEvent)>::new(move |e: web_sys::WheelEvent| {
                eq.borrow_mut().push(DomEvent {
                    kind: DomEventKind::MouseWheel {
                        delta_x: e.delta_x(),
                        delta_y: e.delta_y(),
                    },
                    prevent_default: true,
                });
                e.prevent_default();
            });
            canvas.add_event_listener_with_callback("wheel", closure.as_ref().unchecked_ref()).ok();
            closure.forget();
        }

        // --- Touch events (on canvas) ---
        let touch_handler = |phase: &'static str, eq: Rc<RefCell<Vec<DomEvent>>>| {
            let dom_phase = match phase {
                "touchstart" => crate::events::DomTouchPhase::Start,
                "touchmove" => crate::events::DomTouchPhase::Move,
                "touchend" => crate::events::DomTouchPhase::End,
                _ => crate::events::DomTouchPhase::Cancel,
            };
            Closure::<dyn FnMut(web_sys::TouchEvent)>::new(move |e: web_sys::TouchEvent| {
                e.prevent_default();
                let touches = e.changed_touches();
                for i in 0..touches.length() {
                    if let Some(touch) = touches.get(i) {
                        eq.borrow_mut().push(DomEvent {
                            kind: DomEventKind::Touch {
                                id: touch.identifier() as u64,
                                phase: dom_phase,
                                client_x: touch.client_x() as f64,
                                client_y: touch.client_y() as f64,
                            },
                            prevent_default: true,
                        });
                    }
                }
            })
        };

        for phase in &["touchstart", "touchmove", "touchend", "touchcancel"] {
            let closure = touch_handler(phase, Rc::clone(&_event_queue));
            canvas.add_event_listener_with_callback(phase, closure.as_ref().unchecked_ref()).ok();
            closure.forget();
        }

        // --- Focus events (on window) ---
        {
            let eq = Rc::clone(&_event_queue);
            let closure = Closure::<dyn FnMut()>::new(move || {
                eq.borrow_mut().push(DomEvent {
                    kind: DomEventKind::Focus { focused: true },
                    prevent_default: false,
                });
            });
            window.add_event_listener_with_callback("focus", closure.as_ref().unchecked_ref()).ok();
            closure.forget();
        }
        {
            let eq = Rc::clone(&_event_queue);
            let closure = Closure::<dyn FnMut()>::new(move || {
                eq.borrow_mut().push(DomEvent {
                    kind: DomEventKind::Focus { focused: false },
                    prevent_default: false,
                });
            });
            window.add_event_listener_with_callback("blur", closure.as_ref().unchecked_ref()).ok();
            closure.forget();
        }
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use arachne_ecs::{Schedule, World};

    #[test]
    fn default_canvas_id() {
        let runner = WasmRunner::new();
        assert_eq!(runner.canvas_id(), "arachne-canvas");
    }

    #[test]
    fn custom_canvas_id() {
        let runner = WasmRunner::with_canvas_id("my-game");
        assert_eq!(runner.canvas_id(), "my-game");
    }

    #[test]
    fn default_target_fps() {
        let runner = WasmRunner::new();
        assert_eq!(runner.target_fps(), 60);
    }

    #[test]
    fn custom_target_fps() {
        let runner = WasmRunner::new().with_target_fps(30);
        assert_eq!(runner.target_fps(), 30);
    }

    #[test]
    fn builder_chain() {
        let runner = WasmRunner::with_canvas_id("canvas2")
            .with_target_fps(120)
            .with_native_stub_frames(5);

        assert_eq!(runner.canvas_id(), "canvas2");
        assert_eq!(runner.target_fps(), 120);
        assert_eq!(runner.native_stub_frames, 5);
    }

    #[test]
    fn default_impl_matches_new() {
        let a = WasmRunner::new();
        let b = WasmRunner::default();
        assert_eq!(a.canvas_id(), b.canvas_id());
        assert_eq!(a.target_fps(), b.target_fps());
        assert_eq!(a.native_stub_frames, b.native_stub_frames);
    }

    #[test]
    fn native_stub_run_does_not_panic() {
        let mut runner = WasmRunner::new().with_native_stub_frames(3);
        let mut world = World::new();
        world.insert_resource(Time::default());
        world.insert_resource(InputSystem::new());
        let mut schedule = Schedule::new();

        runner.run(&mut world, &mut schedule);

        // Time should have been updated 3 times.
        let time = world.get_resource::<Time>();
        assert_eq!(time.frame_count(), 3);
    }

    #[test]
    fn native_stub_inserts_missing_resources() {
        let mut runner = WasmRunner::new();
        let mut world = World::new();
        let mut schedule = Schedule::new();

        // World has no Time or InputSystem — runner should insert them.
        runner.run(&mut world, &mut schedule);

        assert!(world.has_resource::<Time>());
        assert!(world.has_resource::<InputSystem>());
    }

    #[test]
    fn native_stub_zero_frames() {
        let mut runner = WasmRunner::new().with_native_stub_frames(0);
        let mut world = World::new();
        world.insert_resource(Time::default());
        let mut schedule = Schedule::new();

        runner.run(&mut world, &mut schedule);

        let time = world.get_resource::<Time>();
        assert_eq!(time.frame_count(), 0);
    }

    #[test]
    fn process_queued_events_empty_is_noop() {
        let mut world = World::new();
        world.insert_resource(InputSystem::new());
        let queue: Rc<RefCell<Vec<DomEvent>>> = Rc::new(RefCell::new(Vec::new()));

        // Should not panic.
        process_queued_events(&mut world, &queue);
    }

    #[test]
    fn process_queued_events_translates_key() {
        use crate::events::DomEventKind;

        let mut world = World::new();
        world.insert_resource(InputSystem::new());

        let queue: Rc<RefCell<Vec<DomEvent>>> = Rc::new(RefCell::new(vec![
            DomEvent {
                kind: DomEventKind::Key {
                    code: "KeyW".to_string(),
                    pressed: true,
                    repeat: false,
                },
                prevent_default: true,
            },
        ]));

        process_queued_events(&mut world, &queue);

        let input = world.get_resource::<InputSystem>();
        assert!(input.keyboard.just_pressed(arachne_input::KeyCode::W));
        // Queue should be drained.
        assert!(queue.borrow().is_empty());
    }

    #[test]
    fn process_queued_events_translates_mouse() {
        use crate::events::DomEventKind;

        let mut world = World::new();
        world.insert_resource(InputSystem::new());

        let queue: Rc<RefCell<Vec<DomEvent>>> = Rc::new(RefCell::new(vec![
            DomEvent {
                kind: DomEventKind::MouseButton {
                    button: 0,
                    pressed: true,
                    client_x: 100.0,
                    client_y: 200.0,
                },
                prevent_default: false,
            },
        ]));

        process_queued_events(&mut world, &queue);

        let input = world.get_resource::<InputSystem>();
        assert!(input.mouse.pressed(arachne_input::MouseButton::Left));
    }

    #[test]
    fn process_queued_events_translates_resize() {
        use crate::events::DomEventKind;

        let mut world = World::new();
        world.insert_resource(InputSystem::new());

        let queue: Rc<RefCell<Vec<DomEvent>>> = Rc::new(RefCell::new(vec![
            DomEvent {
                kind: DomEventKind::Resize {
                    width: 1920,
                    height: 1080,
                    device_pixel_ratio: 2.0,
                },
                prevent_default: false,
            },
        ]));

        process_queued_events(&mut world, &queue);

        let input = world.get_resource::<InputSystem>();
        assert_eq!(input.window_size, arachne_math::Vec2::new(1920.0, 1080.0));
    }

    #[test]
    fn process_queued_events_translates_focus() {
        use crate::events::DomEventKind;

        let mut world = World::new();
        world.insert_resource(InputSystem::new());

        let queue: Rc<RefCell<Vec<DomEvent>>> = Rc::new(RefCell::new(vec![
            DomEvent {
                kind: DomEventKind::Focus { focused: false },
                prevent_default: false,
            },
        ]));

        process_queued_events(&mut world, &queue);

        let input = world.get_resource::<InputSystem>();
        assert!(!input.has_focus);
    }

    #[test]
    fn setup_event_listeners_noop_on_native() {
        let queue = Rc::new(RefCell::new(Vec::new()));
        // Should not panic on native.
        setup_event_listeners("test-canvas", queue);
    }
}
