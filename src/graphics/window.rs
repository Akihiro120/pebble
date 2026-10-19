use std::sync::{Arc, Mutex};

use winit::{
    application::ApplicationHandler,
    event::{DeviceEvent, DeviceId, ElementState, TouchPhase, WindowEvent},
    event_loop::{ActiveEventLoop, ControlFlow, EventLoop},
    window::{Fullscreen, Window as OsWindow, WindowId},
};
use winit_input_helper::WinitInputHelper;

use crate::{
    ecs::plugin::Plugin,
    graphics::types::{CursorGrabMode, CursorIcon, KeyCode, MouseButton},
};

/// How the runner decides whether to draw another frame.
#[derive(Clone, Copy, PartialEq, Eq, Default)]
pub enum RedrawPolicy {
    /// Redraw every frame, forever. What a game wants, and the historical
    /// behaviour — so it stays the default.
    #[default]
    Continuous,
    /// Redraw only when input arrives or something asks via [`Redraw`].
    ///
    /// For an app whose image is usually static — a viewer, an editor — where
    /// re-presenting an identical frame at the display's rate is pure cost.
    /// Anything that animates, polls, or is waiting on an async load has to
    /// ask for each frame it needs, or the app will simply stop.
    OnDemand,
}

/// Asks the runner for another frame under [`RedrawPolicy::OnDemand`].
/// Inserted as a resource by [`WindowPlugin`]; a no-op under `Continuous`.
///
/// The flag is consumed once the frame it asked for begins, so a system that
/// needs a continuous stream (an animation, a poll) must request on every tick
/// rather than latching it once.
#[derive(Clone)]
pub struct Redraw {
    wanted: Arc<std::sync::atomic::AtomicBool>,
    /// The window to poke, once one exists. Setting the flag alone is only
    /// enough from inside a frame, where the end-of-frame check is still to
    /// come; a request arriving while the loop is *asleep* has to actually wake
    /// it, or nothing will ever read the flag again.
    window: Arc<Mutex<Option<Arc<OsWindow>>>>,
}

impl Default for Redraw {
    fn default() -> Self {
        Self::new()
    }
}

impl Redraw {
    /// A handle to pass to [`WindowConfig::redraw`], so the caller keeps a
    /// clone of the one the runner will use. Needed to wake the loop from
    /// outside the ECS — a DOM callback, a timer, a host asking the app to
    /// shut down — which is otherwise impossible, because the runner's own
    /// handle is not created until it starts.
    pub fn new() -> Self {
        Self {
            wanted: Arc::new(std::sync::atomic::AtomicBool::new(false)),
            window: Arc::new(Mutex::new(None)),
        }
    }

    /// Ask for one more frame, waking the loop if it is asleep.
    pub fn request(&self) {
        self.wanted
            .store(true, std::sync::atomic::Ordering::Relaxed);
        if let Ok(window) = self.window.lock()
            && let Some(window) = window.as_ref()
        {
            window.request_redraw();
        }
    }

    fn attach(&self, window: Arc<OsWindow>) {
        if let Ok(mut slot) = self.window.lock() {
            *slot = Some(window);
        }
    }

    fn take(&self) -> bool {
        self.wanted
            .swap(false, std::sync::atomic::Ordering::Relaxed)
    }
}

/// Initial window title/size, passed to [`WindowPlugin::new`].
pub struct WindowConfig {
    pub title: String,
    pub width: u32,
    pub height: u32,
    /// Whether the window free-runs or draws on demand. See [`RedrawPolicy`].
    pub redraw_policy: RedrawPolicy,
    /// The [`Redraw`] handle the runner should use, when the caller needs a
    /// clone of it before the app starts. `None` lets the runner make its own.
    pub redraw: Option<Redraw>,
    /// Web only: the canvas to render into, via [`WindowConfig::with_canvas`].
    ///
    /// `None` keeps the historical behaviour — winit creates its own canvas and
    /// appends it to `<body>`, which is fine for a page that is nothing but the
    /// app, and useless for one embedding it, since the canvas then escapes the
    /// host's layout entirely.
    #[cfg(target_arch = "wasm32")]
    pub canvas: Option<web_sys::HtmlCanvasElement>,
    /// Web only: whether winit calls `preventDefault()` on canvas events with
    /// side effects. winit's own default is `true`; an embedded app often wants
    /// `false` so the host page keeps its scrolling and focus behaviour.
    pub prevent_default: bool,
    /// Web only: whether the canvas is tab-focusable. Keyboard events on the
    /// web are canvas-scoped, so this has to stay on for them to arrive at all.
    pub focusable: bool,
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            title: "Pebble".to_string(),
            width: 1280,
            height: 720,
            redraw_policy: RedrawPolicy::default(),
            redraw: None,
            #[cfg(target_arch = "wasm32")]
            canvas: None,
            prevent_default: true,
            focusable: true,
        }
    }
}

impl WindowConfig {
    /// Render into a canvas the host already owns, instead of one winit creates
    /// and appends to `<body>`. Web only.
    ///
    /// This is what makes the app embeddable: the canvas stays where the host
    /// put it, obeys the host's CSS, and is removed with the host's own element.
    #[cfg(target_arch = "wasm32")]
    pub fn with_canvas(mut self, canvas: web_sys::HtmlCanvasElement) -> Self {
        self.canvas = Some(canvas);
        self
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

    /// The canvas this window renders into. Web only.
    ///
    /// Exposed so an app can attach its own DOM listeners — a `ResizeObserver`,
    /// or pointer events winit does not surface, such as stylus pressure and
    /// tilt — to the element winit is actually using, rather than guessing at
    /// it with a `querySelector`.
    #[cfg(target_arch = "wasm32")]
    pub fn canvas(&self) -> Option<web_sys::HtmlCanvasElement> {
        use winit::platform::web::WindowExtWebSys;
        self.0.canvas()
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
        let helper = &mut self.0.lock().unwrap().helper;

        // winit's web backend reports touches only as `Touch` events, which
        // the input helper doesn't track — map them onto the primary-button
        // pointer model so `cursor`/`cursor_diff`/`mouse_held` cover both.
        if let WindowEvent::Touch(touch) = event {
            helper.process_window_event(&WindowEvent::CursorMoved {
                device_id: touch.device_id,
                position: touch.location,
            });
            let state = match touch.phase {
                TouchPhase::Started => ElementState::Pressed,
                TouchPhase::Ended | TouchPhase::Cancelled => ElementState::Released,
                TouchPhase::Moved => return,
            };
            helper.process_window_event(&WindowEvent::MouseInput {
                device_id: touch.device_id,
                state,
                button: winit::event::MouseButton::Left,
            });
            return;
        }

        helper.process_window_event(event);
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
    redraw: Redraw,
    redraw_policy: RedrawPolicy,
}

impl WinitApp {
    /// Wake the loop for an event that arrived while it was idle. Free under
    /// `Continuous`, where a redraw is always already pending.
    fn wake(&self) {
        if self.redraw_policy == RedrawPolicy::OnDemand
            && let Some(window) = self.window.as_ref()
        {
            window.request_redraw();
        }
    }
}

impl ApplicationHandler for WinitApp {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        let Some(config) = self.config.take() else {
            return;
        };

        // Adopt the caller's handle before the window exists, so that the
        // `attach` below reaches the clone they are holding and not one we are
        // about to throw away.
        self.redraw_policy = config.redraw_policy;
        if let Some(redraw) = config.redraw.clone() {
            self.redraw = redraw;
        }

        #[allow(unused_mut)]
        let mut attrs = OsWindow::default_attributes().with_title(config.title);

        // A host canvas is already laid out by the page's CSS, so the requested
        // size is not ours to impose: winit writes an inner size onto the canvas
        // as inline pixel styles, and inline styles beat the host's stylesheet.
        // Measured before this guard — a 640x380 container held a canvas that
        // stayed stubbornly 1280x720 and overflowed it. Everywhere else the
        // requested size is the only size there is, so it still applies.
        #[cfg(target_arch = "wasm32")]
        let size_is_ours = config.canvas.is_none();
        #[cfg(not(target_arch = "wasm32"))]
        let size_is_ours = true;

        if size_is_ours {
            attrs = attrs.with_inner_size(winit::dpi::PhysicalSize::new(config.width, config.height));
        }

        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            // With a host canvas, take it and append nothing: the element is
            // already in the page, where the host's layout wants it. Without
            // one, winit creates its own and we ask it to insert that, so a
            // window still shows up without hand-rolled web_sys/DOM code.
            let host_canvas = config.canvas.clone();
            attrs = attrs
                .with_append(host_canvas.is_none())
                .with_canvas(host_canvas)
                .with_prevent_default(config.prevent_default)
                .with_focusable(config.focusable);
        }

        let os_window = Arc::new(event_loop.create_window(attrs).unwrap());
        self.window = Some(os_window.clone());
        // From here a `Redraw::request` can wake the loop rather than only
        // setting a flag for a frame that may never come.
        self.redraw.attach(os_window.clone());
        let window = Window::new(os_window);


        self.app = std::mem::take(&mut self.app)
            .insert_resource(window)
            .insert_resource(self.input.clone())
            .insert_resource(self.redraw.clone());

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

                // Sampled before the tick, not after: the frame that brings the
                // backend up runs only the internal GPU schedules, so no system
                // of ours has had a chance to ask for the next one. Testing
                // readiness afterwards would see `true` on exactly that frame
                // and let the loop sleep one tick before anything had run.
                let was_ready = self.app.backend_ready();

                self.app.update();
                if self.app.should_exit() {
                    event_loop.exit();
                    return;
                }

                // `resumed` always runs before the first `window_event`, so
                // the window is guaranteed to exist here.
                //
                // Under `OnDemand` this is where the loop actually stops: with
                // nothing asking for a frame, no redraw is queued, `Wait` puts
                // the loop to sleep, and the last presented image stays on
                // screen at no cost. Input and `Redraw::request` wake it again.
                let wanted = self.redraw.take();
                if self.redraw_policy == RedrawPolicy::Continuous || !was_ready || wanted {
                    self.window.as_ref().unwrap().request_redraw();
                }
            }
            other => {
                self.pending_window_events.push(other);
                // Buffered events are only drained by a redraw, so on demand
                // one has to be asked for or the input would sit there unseen.
                self.wake();
            }
        }
    }

    fn device_event(&mut self, _event_loop: &ActiveEventLoop, _device_id: DeviceId, event: DeviceEvent) {
        self.pending_device_events.push(event);
        self.wake();
    }
}

impl Plugin for WindowPlugin {
    fn build(self, app: crate::app::App) -> crate::app::App {
        app.set_runner(move |app| {
            // Constructed here, not in `build`. winit allows one event loop per
            // process and returns `RecreationAttempt` for a second, so building
            // it while merely *assembling* an App made construction itself the
            // scarce operation: a host that builds an App it then decides not to
            // run — React StrictMode double-invoking an effect in development is
            // the everyday case — would panic on the next attempt. Creating it
            // at run time means only actually running twice is an error.
            let event_loop = EventLoop::new().unwrap();
            // The loop is paced by `request_redraw` (see `WinitApp`), not by
            // spinning — `Wait` lets it actually sleep between frames instead of
            // busy-polling.
            event_loop.set_control_flow(ControlFlow::Wait);

            let handler = WinitApp {
                config: Some(self.config),
                app,
                input: Input::new(),
                window: None,
                pending_window_events: Vec::new(),
                pending_device_events: Vec::new(),
                redraw: Redraw::new(),
                redraw_policy: RedrawPolicy::default(),
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
