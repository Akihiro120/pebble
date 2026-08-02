use std::ops::Deref;
use std::sync::{Arc, Mutex, MutexGuard};

use winit::{
    dpi::PhysicalSize,
    event::{Event, WindowEvent},
    event_loop::EventLoop,
    window::{Window, WindowBuilder},
};
use winit_input_helper::WinitInputHelper;

use crate::rendering::window::{PresentableWindow, WindowConfig, WindowProvider, WindowRunner};

/// Shared handle to the frame's input state.
///
/// Cheap to clone (an `Arc` internally). Call [`Input::get`] to read it —
/// the returned [`InputGuard`] derefs straight to [`WinitInputHelper`], so
/// there's no `.lock().unwrap()` at every call site.
#[derive(Clone)]
pub struct Input(Arc<Mutex<WinitInputHelper>>);

impl Input {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(WinitInputHelper::new())))
    }

    /// Borrow the current input state. Panics if the lock is poisoned (a
    /// prior holder panicked while holding it), matching how the rest of
    /// this codebase treats poisoning as an unrecoverable bug.
    pub fn get(&self) -> InputGuard<'_> {
        InputGuard(self.0.lock().unwrap())
    }

    fn update(&self, event: &Event<()>) -> bool {
        return self.0.lock().unwrap().update(event);
    }
}

pub struct InputGuard<'a>(MutexGuard<'a, WinitInputHelper>);

impl Deref for InputGuard<'_> {
    type Target = WinitInputHelper;
    fn deref(&self) -> &Self::Target {
        &self.0
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

        let mut window_builder = WindowBuilder::new().with_title(config.title);

        #[cfg(not(target_arch = "wasm32"))]
        {
            window_builder =
                window_builder.with_inner_size(PhysicalSize::new(config.width, config.height));
        }

        #[cfg(target_arch = "wasm32")]
        let window = {
            use wasm_bindgen::JsCast;
            use winit::platform::web::WindowBuilderExtWebSys;

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

        event_loop
            .run(move |event, elwt| {
                let stepped = input.update(&event);

                match &event {
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        ..
                    } => elwt.exit(),
                    _ => {}
                }

                if stepped {
                    on_frame();
                    window.request_redraw();
                }
            })
            .unwrap();
    }
}

impl PresentableWindow for WinitWindow {}
