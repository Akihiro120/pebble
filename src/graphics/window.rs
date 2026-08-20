use std::sync::{Arc, Mutex};

use winit::{
    event::{Event, StartCause, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::{Fullscreen, Window as OsWindow, WindowBuilder},
};
use winit_input_helper::WinitInputHelper;

use crate::{
    ecs::plugin::Plugin,
    graphics::types::{CursorGrabMode, CursorIcon, KeyCode, MouseButton},
};

/// Initial window title/size, passed to [`WindowPlugin::new`].
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Pebble".to_string(),
            width: 1280,
            height: 720,
        }
    }
}

/// Runtime control over the OS window — inserted as a resource by
/// [`WindowPlugin`]. No raw `winit` type appears in its public API.
#[derive(Clone)]
pub struct Window(Arc<OsWindow>);

impl Window {
    fn new(handle: Arc<OsWindow>) -> Self {
        Self(handle)
    }

    pub(crate) fn raw(&self) -> Arc<OsWindow> {
        self.0.clone()
    }

    pub fn set_title(&self, title: &str) {
        self.0.set_title(title);
    }

    pub fn inner_size(&self) -> (u32, u32) {
        let size = self.0.inner_size();
        (size.width, size.height)
    }

    pub fn set_inner_size(&self, width: u32, height: u32) {
        let _ = self.0.request_inner_size(winit::dpi::PhysicalSize::new(width, height));
    }

    pub fn set_resizable(&self, resizable: bool) {
        self.0.set_resizable(resizable);
    }

    pub fn set_visible(&self, visible: bool) {
        self.0.set_visible(visible);
    }

    pub fn set_minimized(&self, minimized: bool) {
        self.0.set_minimized(minimized);
    }

    pub fn set_maximized(&self, maximized: bool) {
        self.0.set_maximized(maximized);
    }

    pub fn set_decorations(&self, decorations: bool) {
        self.0.set_decorations(decorations);
    }

    pub fn focus(&self) {
        self.0.focus_window();
    }

    pub fn set_fullscreen(&self, fullscreen: bool) {
        self.0.set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
    }

    pub fn is_fullscreen(&self) -> bool {
        self.0.fullscreen().is_some()
    }

    pub fn set_cursor_icon(&self, icon: CursorIcon) {
        self.0.set_cursor_icon(icon.into());
    }

    pub fn set_cursor_visible(&self, visible: bool) {
        self.0.set_cursor_visible(visible);
    }

    pub fn set_cursor_grab(&self, mode: CursorGrabMode) -> bool {
        self.0.set_cursor_grab(mode.into()).is_ok()
    }

    pub fn request_redraw(&self) {
        self.0.request_redraw();
    }
}

struct InputState {
    helper: WinitInputHelper,
}

/// Keyboard/mouse state for this tick — inserted as a resource by
/// [`WindowPlugin`]. `key_pressed`/`mouse_pressed` are edge-triggered (true
/// only the tick a key/button went down); `key_held`/`mouse_held` are
/// level-triggered (true for as long as it's down).
#[derive(Clone)]
pub struct Input(Arc<Mutex<InputState>>);

impl Input {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(InputState {
            helper: WinitInputHelper::new(),
        })))
    }

    /// Advances one input step from the events gathered since the last frame,
    /// so edge state (`pressed`/`released`, diffs) spans exactly one frame.
    fn step(&self, events: &[Event<()>]) {
        let state = &mut *self.0.lock().unwrap();
        state.helper.update(&Event::<()>::NewEvents(StartCause::Poll));
        for event in events {
            state.helper.update(event);
        }
        state.helper.update(&Event::<()>::AboutToWait);
    }

    pub fn key_pressed(&self, key: KeyCode) -> bool {
        self.0.lock().unwrap().helper.key_pressed(key.into())
    }

    pub fn key_released(&self, key: KeyCode) -> bool {
        self.0.lock().unwrap().helper.key_released(key.into())
    }

    pub fn key_held(&self, key: KeyCode) -> bool {
        self.0.lock().unwrap().helper.key_held(key.into())
    }

    pub fn mouse_pressed(&self, button: MouseButton) -> bool {
        self.0.lock().unwrap().helper.mouse_pressed(button.into())
    }

    pub fn mouse_released(&self, button: MouseButton) -> bool {
        self.0.lock().unwrap().helper.mouse_released(button.into())
    }

    pub fn mouse_held(&self, button: MouseButton) -> bool {
        self.0.lock().unwrap().helper.mouse_held(button.into())
    }

    /// Current cursor position in window coordinates, if it's inside the window.
    pub fn cursor(&self) -> Option<(f32, f32)> {
        self.0.lock().unwrap().helper.cursor()
    }

    /// Cursor movement since last tick.
    pub fn cursor_diff(&self) -> (f32, f32) {
        self.0.lock().unwrap().helper.cursor_diff()
    }

    /// Raw mouse motion since last tick — unlike [`cursor_diff`](Self::cursor_diff),
    /// not clamped to the window (useful for a look/orbit camera).
    pub fn mouse_diff(&self) -> (f32, f32) {
        self.0.lock().unwrap().helper.mouse_diff()
    }

    pub fn scroll_diff(&self) -> (f32, f32) {
        self.0.lock().unwrap().helper.scroll_diff()
    }

    /// True the tick the window's close button was pressed — you decide
    /// whether/how to actually exit.
    pub fn close_requested(&self) -> bool {
        self.0.lock().unwrap().helper.close_requested()
    }

    /// The window's resolution, once known.
    pub fn resolution(&self) -> Option<(u32, u32)> {
        self.0.lock().unwrap().helper.resolution()
    }
}

/// Opens a window (via `winit`) and inserts [`Window`]/[`Input`] as
/// resources. Installs a runner that drives the app from `winit`'s own
/// event loop, functional on native and `wasm32-unknown-unknown`.
pub struct WindowPlugin {
    config: WindowConfig,
}

impl WindowPlugin {
    pub fn new(config: WindowConfig) -> Self {
        Self { config }
    }
}

impl Default for WindowPlugin {
    fn default() -> Self {
        Self::new(WindowConfig::default())
    }
}

impl Plugin for WindowPlugin {
    fn build(self, app: crate::app::App) -> crate::app::App {
        let event_loop = EventLoop::new().unwrap();
        // The frame is driven by request_redraw below, so the loop can sleep
        // between events instead of spinning.
        event_loop.set_control_flow(ControlFlow::Wait);

        #[allow(unused_mut)]
        let mut builder = WindowBuilder::new()
            .with_title(self.config.title)
            .with_inner_size(winit::dpi::PhysicalSize::new(self.config.width, self.config.height));

        // winit doesn't insert the canvas into the page on its own — ask it
        // to, so a window actually shows up without hand-rolled web_sys/DOM code
        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowBuilderExtWebSys;
            builder = builder.with_append(true);
        }

        let os_window = Arc::new(builder.build(&event_loop).unwrap());
        let frame_window = os_window.clone();

        let window = Window::new(os_window);
        let input = Input::new();

        app.insert_resource(window)
            .insert_resource(input.clone())
            .set_runner(move |mut app| {
                // winit's model on every platform: RedrawRequested is the frame
                // tick (rAF-aligned on the web), everything else is input to
                // gather and step once per frame.
                // Driving the frame from the event batch instead renders once per loop iteration
                // on the web, where nothing blocks the loop, that is many full frames
                // per displayed frame.
                let mut pending = Vec::new();
                frame_window.request_redraw();

                let handler = move |event, elwt: &winit::event_loop::EventLoopWindowTarget<()>| {
                    match event {
                        Event::WindowEvent { event: WindowEvent::CloseRequested, .. } => {
                            elwt.exit();
                        }
                        Event::WindowEvent { event: WindowEvent::RedrawRequested, .. } => {
                            input.step(&pending);
                            pending.clear();
                            app.update();
                            if app.should_exit() {
                                elwt.exit();
                            }
                            frame_window.request_redraw();
                        }
                        event @ (Event::WindowEvent { .. } | Event::DeviceEvent { .. }) => {
                            pending.push(event);
                        }
                        _ => {}
                    }
                };

                // `run` blocks forever natively; on wasm it only works via an
                // internal exception-unwinding trick and isn't always available.
                // `spawn` is the purpose-built and non-blocking wasm equivalent.
                #[cfg(not(target_arch = "wasm32"))]
                event_loop.run(handler).unwrap();

                #[cfg(target_arch = "wasm32")]
                {
                    use winit::platform::web::EventLoopExtWebSys;
                    event_loop.spawn(handler);
                }
            })
    }
}
