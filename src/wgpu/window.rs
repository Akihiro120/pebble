use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[cfg(not(target_arch = "wasm32"))]
use winit::dpi::PhysicalSize;
use winit::{
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};
use winit_input_helper::WinitInputHelper;

use crate::rendering::window::{PresentableWindow, WindowConfig, WindowProvider, WindowRunner};
use crate::wgpu::keycode::{KeyCode, MouseButton};

/// The frame's keyboard/mouse/window input state.
///
/// A self-contained ECS resource — fetch it directly with `Res<Input>`,
/// no need to go through `WindowResource<W>` or name a concrete backend
/// type. Cheap to clone (an `Arc` internally), and every accessor locks
/// internally and hands back a plain value, so there's no guard type to
/// hold onto: `input.key_held(KeyCode::KeyW)` just returns `bool`.
///
/// State is refreshed once per step, before systems run, so every accessor
/// below reflects that step's input.
#[derive(Clone)]
pub struct Input(Arc<Mutex<WinitInputHelper>>);

impl Input {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(WinitInputHelper::new())))
    }

    fn update(&self, event: &Event<()>) -> bool {
        self.0.lock().unwrap().update(event)
    }

    /// True the step a key goes from "not pressed" to "pressed". Uses
    /// physical keys (layout-independent), so this is the one to reach for
    /// game controls rather than text entry.
    pub fn key_pressed(&self, key: KeyCode) -> bool {
        self.0.lock().unwrap().key_pressed(key.into())
    }

    /// True the step a key goes from "pressed" to "not pressed".
    pub fn key_released(&self, key: KeyCode) -> bool {
        self.0.lock().unwrap().key_released(key.into())
    }

    /// True for every step the key remains pressed.
    pub fn key_held(&self, key: KeyCode) -> bool {
        self.0.lock().unwrap().key_held(key.into())
    }

    /// True while either shift key is held.
    pub fn held_shift(&self) -> bool {
        self.0.lock().unwrap().held_shift()
    }

    /// True while either control key is held.
    pub fn held_control(&self) -> bool {
        self.0.lock().unwrap().held_control()
    }

    /// True while either alt key is held.
    pub fn held_alt(&self) -> bool {
        self.0.lock().unwrap().held_alt()
    }

    /// True the step a mouse button goes from "not pressed" to "pressed".
    pub fn mouse_pressed(&self, button: MouseButton) -> bool {
        self.0.lock().unwrap().mouse_pressed(button.into())
    }

    /// True the step a mouse button goes from "pressed" to "not pressed".
    pub fn mouse_released(&self, button: MouseButton) -> bool {
        self.0.lock().unwrap().mouse_released(button.into())
    }

    /// True for every step the mouse button remains pressed.
    pub fn mouse_held(&self, button: MouseButton) -> bool {
        self.0.lock().unwrap().mouse_held(button.into())
    }

    /// Cursor position in pixels, or `None` if the window isn't focused (or
    /// the cursor is off-window and no button is held).
    pub fn cursor(&self) -> Option<(f32, f32)> {
        self.0.lock().unwrap().cursor()
    }

    /// Change in cursor position since the last step. `(0.0, 0.0)` under the
    /// same conditions [`Input::cursor`] returns `None`.
    pub fn cursor_diff(&self) -> (f32, f32) {
        self.0.lock().unwrap().cursor_diff()
    }

    /// Change in raw mouse motion since the last step — driven by device
    /// events rather than cursor position, so this is the one to reach for
    /// a captured-mouse first-person camera.
    pub fn mouse_diff(&self) -> (f32, f32) {
        self.0.lock().unwrap().mouse_diff()
    }

    /// Scroll wheel delta `(horizontal, vertical)` since the last step.
    pub fn scroll_diff(&self) -> (f32, f32) {
        self.0.lock().unwrap().scroll_diff()
    }

    /// True if the OS requested the window close this step (e.g. the title
    /// bar's close button).
    pub fn close_requested(&self) -> bool {
        self.0.lock().unwrap().close_requested()
    }

    /// Current window resolution, or `None` before the first resize event.
    pub fn resolution(&self) -> Option<(u32, u32)> {
        self.0.lock().unwrap().resolution()
    }

    /// Path of a file dropped onto the window this step, if any.
    pub fn dropped_file(&self) -> Option<PathBuf> {
        self.0.lock().unwrap().dropped_file()
    }

    /// Time elapsed since the last step, or `None` while the first step is
    /// still in progress.
    pub fn delta_time(&self) -> Option<Duration> {
        self.0.lock().unwrap().delta_time()
    }
}

pub struct WinitWindow {
    window: Arc<Window>,
    event_loop: EventLoop<()>,
    input: Input,
}

impl WindowProvider for WinitWindow {
    type Handle = Arc<Window>;
    type Exposed = Input;

    fn create(config: &WindowConfig) -> Self {
        let event_loop = EventLoop::new().unwrap();
        event_loop.set_control_flow(winit::event_loop::ControlFlow::Poll);

        #[cfg_attr(target_arch = "wasm32", allow(unused_mut))]
        let mut window_builder = WindowBuilder::new().with_title(config.title.clone());

        #[cfg(not(target_arch = "wasm32"))]
        {
            window_builder =
                window_builder.with_inner_size(PhysicalSize::new(config.width, config.height));
        }

        #[cfg(target_arch = "wasm32")]
        let window = {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowBuilderExtWebSys;

            // Without this, a panic anywhere in the app — including the
            // `.expect()`s a few lines below, which are exactly the ones
            // most likely to fire on a real misconfiguration (no matching
            // canvas element) — shows up in the browser console as an
            // opaque, unhelpful trap instead of the actual message and a
            // Rust-side stack trace. Idempotent, so it's safe to call even
            // if something else already installed a hook first.
            console_error_panic_hook::set_once();

            let web_window = web_sys::window().expect("no global `window` exists");
            let document = web_window
                .document()
                .expect("should have a document on window");
            let canvas = document
                .get_element_by_id("wgpu_canvas")
                .expect("no element with id `wgpu_canvas` found — add <canvas id=\"wgpu_canvas\"></canvas> to index.html")
                .unchecked_into::<web_sys::HtmlCanvasElement>();

            let window = Arc::new(
                window_builder
                    .with_canvas(Some(canvas))
                    .build(&event_loop)
                    .unwrap(),
            );

            // winit doesn't track the browser viewport for a caller-supplied
            // canvas, so the window (and canvas) would stay stuck at its
            // initial size forever. Size it to the viewport now, then keep it
            // in sync on every `resize` event.
            let sync_size = {
                let window = window.clone();
                move || {
                    let web_window = web_sys::window().expect("no global `window` exists");
                    let width = web_window.inner_width().unwrap().as_f64().unwrap();
                    let height = web_window.inner_height().unwrap().as_f64().unwrap();
                    let _ = window.request_inner_size(winit::dpi::LogicalSize::new(width, height));
                }
            };
            sync_size();

            let closure =
                wasm_bindgen::closure::Closure::<dyn FnMut()>::new(sync_size).into_js_value();
            web_window
                .add_event_listener_with_callback("resize", closure.unchecked_ref())
                .expect("failed to add `resize` listener");

            window
        };

        #[cfg(not(target_arch = "wasm32"))]
        let window = Arc::new(window_builder.build(&event_loop).unwrap());

        Self {
            window,
            event_loop,
            input: Input::new(),
        }
    }

    fn size(handle: &Self::Handle) -> (u32, u32) {
        let s = handle.inner_size();
        (s.width, s.height)
    }

    fn exposed(&self) -> Self::Exposed {
        self.input.clone()
    }

    fn handle(&self) -> &Self::Handle {
        &self.window
    }
}

impl WindowRunner for WinitWindow {
    fn run(self, mut on_frame: impl FnMut() + 'static) {
        let Self {
            window,
            event_loop,
            input,
        } = self;

        // On web, `ControlFlow::Poll` doesn't tie the loop to vsync — winit's
        // web backend pumps `AboutToWait` (which `stepped` fires on) via an
        // unthrottled task-scheduler loop, so driving frames off it runs the
        // whole ECS tick + GPU submit hundreds of times a second, competing
        // with the browser's compositor on the same thread. `RedrawRequested`
        // is the one event winit paces via `requestAnimationFrame` on web, so
        // drive frames from that instead and keep re-requesting it each time.
        #[cfg(target_arch = "wasm32")]
        window.request_redraw();

        event_loop
            .run(move |event, elwt| {
                let stepped = input.update(&event);

                match &event {
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        ..
                    } => elwt.exit(),
                    #[cfg(target_arch = "wasm32")]
                    Event::WindowEvent {
                        event: WindowEvent::RedrawRequested,
                        ..
                    } => {
                        on_frame();
                        window.request_redraw();
                    }
                    _ => {}
                }

                #[cfg(not(target_arch = "wasm32"))]
                if stepped {
                    on_frame();
                    window.request_redraw();
                }
                #[cfg(target_arch = "wasm32")]
                let _ = stepped;
            })
            .unwrap();
    }
}

impl PresentableWindow for WinitWindow {}
