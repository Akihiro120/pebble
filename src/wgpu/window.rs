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

    fn update(&self, event: &Event<()>) {
        self.0.lock().unwrap().update(event);
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
        let window = Arc::new(
            WindowBuilder::new()
                .with_title(config.title)
                .with_inner_size(PhysicalSize::new(config.width, config.height))
                .build(&event_loop)
                .unwrap(),
        );

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
                input.update(&event);

                match &event {
                    Event::WindowEvent {
                        event: WindowEvent::CloseRequested,
                        ..
                    } => elwt.exit(),
                    Event::WindowEvent {
                        event: WindowEvent::RedrawRequested,
                        ..
                    } => on_frame(),
                    Event::AboutToWait => window.request_redraw(),
                    _ => {}
                }
            })
            .unwrap();
    }
}

impl PresentableWindow for WinitWindow {}
