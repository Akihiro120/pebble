use std::sync::{Arc, Mutex};

use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Fullscreen, Window as OsWindow, WindowId},
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
        let _ = self
            .0
            .request_inner_size(winit::dpi::PhysicalSize::new(width, height));
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
        self.0
            .set_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
    }

    pub fn is_fullscreen(&self) -> bool {
        self.0.fullscreen().is_some()
    }

    pub fn set_cursor_icon(&self, icon: CursorIcon) {
        self.0.set_cursor(winit::window::CursorIcon::from(icon));
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

    fn step(&self) {
        self.0.lock().unwrap().helper.step();
    }

    fn process_window_event(&self, event: &WindowEvent) {
        self.0.lock().unwrap().helper.process_window_event(event);
    }

    fn process_device_event(&self, event: &DeviceEvent) {
        self.0.lock().unwrap().helper.process_device_event(event);
    }

    fn end_step(&self) {
        self.0.lock().unwrap().helper.end_step();
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

/// Installs a runner that opens a window (via `winit`) and inserts
/// [`Window`]/[`Input`] as resources once the event loop resumes — see
/// [`WinitApp`]. Functional on native and `wasm32-unknown-unknown`.
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

/// Drives the `App` from `winit`'s `ApplicationHandler`, paced by
/// `RedrawRequested` rather than the poll-loop's `AboutToWait` — the latter
/// is an iteration boundary, not a frame boundary, and the two only line up
/// by coincidence on native (where a vsync-blocking `present()` inside
/// `App::update` happens to throttle it). On the web the poll loop iterates
/// independently of `requestAnimationFrame`, so stepping input there instead
/// fragments each displayed frame's input across several silent sub-steps —
/// dropping press/release edges and diluting `mouse_diff`. `RedrawRequested`
/// is rAF-aligned on every backend, so keying off it needs no platform cfg.
///
/// The window can only be created once the event loop actually resumes, so
/// `config` is consumed there rather than up front (also doubles as a
/// "window already created" guard for platforms that call `resumed` more
/// than once). Every other window/device event is buffered and replayed
/// into the input helper as one atomic step right before `RedrawRequested`
/// is handled, so edge state and diffs span exactly one displayed frame.
struct WinitApp {
    config: Option<WindowConfig>,
    app: crate::app::App,
    input: Input,
    window: Option<Arc<OsWindow>>,
    pending_window_events: Vec<WindowEvent>,
    pending_device_events: Vec<DeviceEvent>,
}

impl ApplicationHandler for WinitApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Some(config) = self.config.take() else {
            return;
        };

        #[allow(unused_mut)]
        let mut attrs = OsWindow::default_attributes()
            .with_title(config.title)
            .with_inner_size(winit::dpi::PhysicalSize::new(config.width, config.height));

        // winit doesn't insert the canvas into the page on its own — ask it
        // to, so a window actually shows up without hand-rolled web_sys/DOM
        // code
        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            attrs = attrs.with_append(true);
        }

        let os_window = Arc::new(event_loop.create_window(attrs).unwrap());
        self.window = Some(os_window.clone());
        let window = Window::new(os_window);

        self.app = std::mem::take(&mut self.app)
            .insert_resource(window)
            .insert_resource(self.input.clone());

        // Kick off the first frame — under `ControlFlow::Wait` nothing else
        // will ever request one.
        self.window.as_ref().unwrap().request_redraw();
    }

    fn window_event(&mut self, event_loop: &ActiveEventLoop, _window_id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => {
                event_loop.exit();
            }
            WindowEvent::RedrawRequested => {
                self.input.step();
                for pending in self.pending_window_events.drain(..) {
                    self.input.process_window_event(&pending);
                }
                for pending in self.pending_device_events.drain(..) {
                    self.input.process_device_event(&pending);
                }
                self.input.end_step();

                self.app.update();
                if self.app.should_exit() {
                    event_loop.exit();
                    return;
                }

                // `resumed` always runs before the first `window_event`, so
                // the window is guaranteed to exist here.
                self.window.as_ref().unwrap().request_redraw();
            }
            other => self.pending_window_events.push(other),
        }
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _device_id: DeviceId, event: DeviceEvent) {
        self.pending_device_events.push(event);
    }
}

impl Plugin for WindowPlugin {
    fn build(self, app: crate::app::App) -> crate::app::App {
        let event_loop = EventLoop::new().unwrap();
        // The loop is paced by `request_redraw` (see `WinitApp`), not by
        // spinning — `Wait` lets it actually sleep between frames instead of
        // busy-polling.
        event_loop.set_control_flow(ControlFlow::Wait);

        app.set_runner(move |app| {
            let handler = WinitApp {
                config: Some(self.config),
                app,
                input: Input::new(),
                window: None,
                pending_window_events: Vec::new(),
                pending_device_events: Vec::new(),
            };

            // `run_app` blocks forever natively; on wasm it only works via an
            // internal exception-unwinding trick and isn't always
            // available — `spawn_app` is the purpose-built non-blocking wasm
            // equivalent, same handler, just returns immediately after
            // registering it with the browser
            #[cfg(not(target_arch = "wasm32"))]
            {
                let mut handler = handler;
                event_loop.run_app(&mut handler).unwrap();
            }

            #[cfg(target_arch = "wasm32")]
            {
                use winit::platform::web::EventLoopExtWebSys;
                event_loop.spawn_app(handler);
            }
        })
    }
}
