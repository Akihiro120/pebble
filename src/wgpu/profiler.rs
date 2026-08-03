//! Opt-in CPU frame-timing and custom-section telemetry, with an
//! `egui`-rendered overlay — gated behind the `profiler` Cargo feature so
//! `egui`/`egui-wgpu` aren't pulled into a build that doesn't want them.
//!
//! Entirely inert unless you add [`ProfilerPlugin`] yourself: no systems
//! run, no resources exist, nothing is drawn. GPU-side timestamp-query
//! profiling (per-section GPU time, not just CPU wall-clock) isn't covered
//! here — this is CPU-side timing only; that's a separate, larger addition
//! for later.
//!
//! Works on native and web. On `wasm32` specifically, `egui_wgpu::Renderer`
//! stores its callback resources as `Box<dyn Any>` with no `Send + Sync`
//! bound (to allow storing non-thread-safe, JS-bound values there), which
//! doesn't structurally satisfy the `Send + Sync` every Pebble resource
//! needs — see that target's `WasmSendSync` type (only compiled on
//! `wasm32`, so not linked here) for how, and why it's sound, this module
//! asserts it anyway.

use std::collections::{HashMap, VecDeque};
use std::sync::Mutex;
use std::time::Duration;
// `web_time::Instant`, not `std::time::Instant`: the latter's `wasm32`
// support depends on how the final binary gets linked (only reliable when
// something in the chain already brings in the right JS shims), where
// `web_time` always calls `performance.now()` directly. Identical API,
// backed by `std::time::Instant` itself on every other target.
use web_time::Instant;

use crate::{
    app::App,
    ecs::{
        plugin::Plugin,
        system::{Res, ResMut, SystemOrderingExt},
    },
    prelude::{ColorTarget, CurrentFrame, LazyResourcePlugin, Pass, SystemStage},
    wgpu::backend::WGPUBackend,
};

/// How many samples of history [`Profiler`] keeps per frame-time/section —
/// enough to average out frame-to-frame jitter without the overlay's
/// numbers lagging noticeably behind reality.
const HISTORY_LEN: usize = 120;

struct Samples {
    history: VecDeque<Duration>,
}

impl Samples {
    fn new() -> Self {
        Self {
            history: VecDeque::with_capacity(HISTORY_LEN),
        }
    }

    fn push(&mut self, duration: Duration) {
        self.history.push_back(duration);
        if self.history.len() > HISTORY_LEN {
            self.history.pop_front();
        }
    }

    fn last(&self) -> Duration {
        self.history.back().copied().unwrap_or_default()
    }

    fn average(&self) -> Duration {
        if self.history.is_empty() {
            return Duration::ZERO;
        }
        self.history.iter().sum::<Duration>() / self.history.len() as u32
    }
}

struct ProfilerInner {
    frames: Samples,
    last_tick: Instant,
    sections: HashMap<&'static str, Samples>,
}

/// Frame-timing and custom-section telemetry — inserted as a resource by
/// [`ProfilerPlugin`].
///
/// Every method takes `&self`: timing state lives behind a `RefCell`
/// internally rather than requiring an exclusive borrow, specifically so
/// [`section`](Self::section) works from a plain `Res<Profiler>` and so
/// sections can nest (starting one section while another from the same
/// `Profiler` is still open would otherwise be a double-mutable-borrow).
///
/// ```ignore
/// fn physics_step(profiler: Res<Profiler>, /* ... */) {
///     let _span = profiler.section("physics");
///     // ... do physics work ...
/// } // timed automatically when _span drops here
/// ```
pub struct Profiler {
    // `Mutex`, not `RefCell`: resources must be `Send + Sync` (`hecs::Component`'s
    // bound) regardless of the fact that this scheduler happens to run
    // systems within a stage sequentially, never concurrently — so there's
    // never real contention here, just the marker trait to satisfy.
    inner: Mutex<ProfilerInner>,
}

impl Profiler {
    fn new() -> Self {
        Self {
            inner: Mutex::new(ProfilerInner {
                frames: Samples::new(),
                last_tick: Instant::now(),
                sections: HashMap::new(),
            }),
        }
    }

    /// Records one tick's elapsed wall-clock time. Called once per tick by
    /// [`ProfilerPlugin`]'s own system — not meant to be called from
    /// application code.
    fn tick(&self) {
        let mut inner = self.inner.lock().unwrap();
        let now = Instant::now();
        let elapsed = now.duration_since(inner.last_tick);
        inner.last_tick = now;
        inner.frames.push(elapsed);
    }

    /// Most recent tick's duration.
    pub fn frame_time(&self) -> Duration {
        self.inner.lock().unwrap().frames.last()
    }

    /// Average tick duration over the last 120 ticks.
    pub fn average_frame_time(&self) -> Duration {
        self.inner.lock().unwrap().frames.average()
    }

    /// Frames per second, computed from [`average_frame_time`](Self::average_frame_time)
    /// (so it doesn't jump around on a single slow/fast tick the way
    /// `1.0 / frame_time` would).
    pub fn fps(&self) -> f32 {
        let seconds = self.average_frame_time().as_secs_f32();
        if seconds <= 0.0 { 0.0 } else { 1.0 / seconds }
    }

    /// Begin timing a named section, ended when the returned guard drops.
    /// Sections are created on first use — no registration step. Reusing
    /// the same `name` from multiple systems/call sites accumulates into
    /// one shared history; use distinct names if that's not what you want.
    pub fn section(&self, name: &'static str) -> SectionGuard<'_> {
        SectionGuard { profiler: self, name, start: Instant::now() }
    }

    /// Most recent duration recorded for `name`, or `None` if no
    /// [`section`](Self::section) with that name has completed yet (never
    /// called, or its guard hasn't dropped this tick) — check for a typo
    /// against the name passed to `section` if this stays `None`
    /// unexpectedly.
    pub fn section_time(&self, name: &str) -> Option<Duration> {
        self.inner.lock().unwrap().sections.get(name).map(Samples::last)
    }

    /// Average duration for `name` over its last 120 samples.
    pub fn average_section_time(&self, name: &str) -> Option<Duration> {
        self.inner.lock().unwrap().sections.get(name).map(Samples::average)
    }

    /// Every section name recorded so far — for iterating all of them
    /// (e.g. to render an overlay) without knowing their names up front.
    pub fn section_names(&self) -> Vec<&'static str> {
        self.inner.lock().unwrap().sections.keys().copied().collect()
    }

    fn record_section(&self, name: &'static str, duration: Duration) {
        self.inner
            .lock()
            .unwrap()
            .sections
            .entry(name)
            .or_insert_with(Samples::new)
            .push(duration);
    }
}

/// RAII guard returned by [`Profiler::section`] — records its elapsed time
/// into that section when dropped. Hold it for exactly the scope you want
/// timed; dropping early (or storing it and dropping late) changes what
/// gets measured.
#[must_use = "a Profiler::section guard does nothing until it drops — assign it to a named \
              binding (`let _span = ...`), not `_`, which drops it immediately"]
pub struct SectionGuard<'a> {
    profiler: &'a Profiler,
    name: &'static str,
    start: Instant,
}

impl Drop for SectionGuard<'_> {
    fn drop(&mut self) {
        self.profiler.record_section(self.name, self.start.elapsed());
    }
}

/// The `egui` context/renderer pair, built once a [`WGPUBackend`] exists —
/// internal plumbing for [`ProfilerPlugin`]'s overlay, not part of the
/// public API. A [`LazyResource`](crate::assets::singleton_asset::LazyResource)
/// for the same reason a depth texture or a camera buffer is one (see the
/// book's [Camera, Depth, and Lazy Resources](https://akihiro120.github.io/pebble/ch10-camera-and-depth.html)
/// chapter): exactly one instance, needs a device before it can exist.
struct EguiState {
    ctx: egui::Context,
    renderer: egui_wgpu::Renderer,
}

/// A zero-cost wrapper asserting `Send + Sync` for a type that doesn't
/// structurally have it — used only on `wasm32`, only to wrap [`EguiState`].
///
/// # Safety
///
/// Asserting `Send + Sync` is normally a claim about safe concurrent
/// access, which this type cannot actually verify. It's sound here anyway
/// because of what's specific to the target this is `#[cfg]`-gated to:
/// `wasm32-unknown-unknown`, without Pebble enabling the `atomics` target
/// feature anywhere, has exactly **one** thread of execution — there is no
/// `std::thread::spawn`, no shared-memory threading, no mechanism by which
/// a second thread could ever exist to violate the exclusive access these
/// traits promise. `Send`/`Sync` exist to prevent *concurrent* misuse;
/// a target that cannot run anything concurrently cannot exhibit the
/// failure mode they guard against, regardless of what's inside `T`.
///
/// The reason this wrapper is needed at all: `egui_wgpu::Renderer` type-erases
/// its internal callback resources as a bare `Box<dyn Any>` on `wasm32`
/// specifically (see the module docs), which is what actually blocks
/// [`EguiState`] from satisfying `hecs::Component`'s `Send + Sync` bound —
/// nothing about egui's own single-threaded usage pattern is unsound here,
/// only the auto-trait bookkeeping.
#[cfg(target_arch = "wasm32")]
struct WasmSendSync<T>(T);

#[cfg(target_arch = "wasm32")]
// SAFETY: see the type's doc comment.
unsafe impl<T> Send for WasmSendSync<T> {}
#[cfg(target_arch = "wasm32")]
// SAFETY: see the type's doc comment.
unsafe impl<T> Sync for WasmSendSync<T> {}

#[cfg(target_arch = "wasm32")]
impl<T> std::ops::Deref for WasmSendSync<T> {
    type Target = T;
    fn deref(&self) -> &T {
        &self.0
    }
}
#[cfg(target_arch = "wasm32")]
impl<T> std::ops::DerefMut for WasmSendSync<T> {
    fn deref_mut(&mut self) -> &mut T {
        &mut self.0
    }
}

/// The actual resource type stored in the ECS — [`EguiState`] directly on
/// every target where it's already `Send + Sync` on its own, wrapped in
/// [`WasmSendSync`] only where (and only because) it isn't.
#[cfg(not(target_arch = "wasm32"))]
type EguiStateResource = EguiState;
#[cfg(target_arch = "wasm32")]
type EguiStateResource = WasmSendSync<EguiState>;

impl crate::assets::singleton_asset::LazyResource<WGPUBackend> for EguiStateResource {
    type Deps<'a> = ();

    fn construct<'a>(backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let renderer = egui_wgpu::Renderer::new(
            &backend.device,
            backend.config.format,
            egui_wgpu::RendererOptions::default(),
        );
        let state = EguiState { ctx: egui::Context::default(), renderer };
        #[cfg(target_arch = "wasm32")]
        let state = WasmSendSync(state);
        Some(state)
    }
}

/// Registers CPU frame-timing/section telemetry (a [`Profiler`] resource,
/// updated once per tick) and an `egui`-rendered overlay showing it,
/// drawn in its own pass on top of whatever your own render systems already
/// drew — no changes needed to your existing rendering code.
///
/// Requires [`WGPUPlugin`](crate::wgpu::backend::WGPUPlugin) (or at least
/// [`RenderPlugin<WGPUBackend>`](crate::rendering::render_plugin::RenderPlugin)
/// registered before this) — the overlay waits quietly for the backend the
/// same way any other `Option<Res<WGPUBackend>>`-gated system would.
///
/// ```ignore
/// App::new()
///     .add_plugin(WGPUPlugin::new(config))
///     .add_plugin(ProfilerPlugin) // anywhere after WGPUPlugin
///     // ...
/// ```
///
/// Non-interactive in this version: the overlay draws unconditionally at a
/// fixed position and doesn't respond to mouse/keyboard input, since doing
/// so needs raw window events plumbed through in a way
/// [`WindowRunner`](crate::rendering::window::WindowRunner) doesn't
/// currently expose to plugins.
pub struct ProfilerPlugin;

impl Plugin for ProfilerPlugin {
    fn build(&self, app: &mut App) {
        app.add_resource(Profiler::new())
            .add_plugin(LazyResourcePlugin::<WGPUBackend, EguiStateResource>::new())
            .add_system(SystemStage::PreUpdate, tick_profiler)
            .add_system(
                SystemStage::PostRender,
                draw_overlay.before(crate::rendering::render_plugin::end_frame::<WGPUBackend>),
            );
    }
}

fn tick_profiler(profiler: Res<Profiler>) {
    profiler.tick();
}

fn draw_overlay(
    backend: Option<Res<WGPUBackend>>,
    egui_state: Option<ResMut<EguiStateResource>>,
    mut frame: ResMut<CurrentFrame<WGPUBackend>>,
    profiler: Res<Profiler>,
) {
    let (Some(backend), Some(mut egui_state)) = (backend, egui_state) else {
        return; // backend/EguiState not constructed yet — same tick this settles, nothing to draw.
    };
    let Some(mut active) = frame.active() else {
        return; // no frame acquired this tick (see Chapter 7 of the book) — nothing to draw onto.
    };

    let screen_size = egui::vec2(backend.config.width as f32, backend.config.height as f32);
    let raw_input = egui::RawInput {
        screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, screen_size)),
        ..Default::default()
    };

    let full_output = egui_state.ctx.run_ui(raw_input, |ui| {
        let ctx = ui.ctx().clone();
        egui::Window::new("Pebble Profiler").show(&ctx, |ui| {
            ui.label(format!("FPS: {:.1}", profiler.fps()));
            ui.label(format!(
                "Frame time: {:.2} ms",
                profiler.average_frame_time().as_secs_f64() * 1000.0
            ));
            let mut names = profiler.section_names();
            names.sort_unstable();
            if !names.is_empty() {
                ui.separator();
                for name in names {
                    if let Some(d) = profiler.average_section_time(name) {
                        ui.label(format!("{name}: {:.2} ms", d.as_secs_f64() * 1000.0));
                    }
                }
            }
        });
    });

    let clipped_primitives = egui_state
        .ctx
        .tessellate(full_output.shapes, full_output.pixels_per_point);
    let screen_descriptor = egui_wgpu::ScreenDescriptor {
        size_in_pixels: [backend.config.width, backend.config.height],
        pixels_per_point: full_output.pixels_per_point,
    };

    for (id, delta) in &full_output.textures_delta.set {
        egui_state.renderer.update_texture(&backend.device, &backend.queue, *id, delta);
    }

    let mut encoder = backend.device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("profiler-overlay-buffer-upload"),
    });
    egui_state
        .renderer
        .update_buffers(&backend.device, &backend.queue, &mut encoder, &clipped_primitives, &screen_descriptor);
    // Submitted before the frame's own encoder (queued by `begin_pass`
    // below, finished later in `end_frame`) — same-queue submissions
    // execute in submission order, so these buffer/texture writes are
    // guaranteed visible to the render pass that samples them.
    backend.queue.submit(std::iter::once(encoder.finish()));

    {
        // `default_load()`: no clear — draws on top of whatever the app's
        // own Render-stage systems already produced this frame.
        let pass = active.begin_pass(Pass {
            colors: &[ColorTarget::default_load()],
            depth: None,
        });
        // `egui_wgpu::Renderer::render` requires `RenderPass<'static>` —
        // `forget_lifetime` drops the borrow-checker tie to `active`'s
        // encoder rather than actually extending its lifetime; sound here
        // because `pass` is dropped (ending the GPU pass) before `active`
        // itself goes out of scope at the end of this function.
        let mut pass = pass.forget_lifetime();
        egui_state.renderer.render(&mut pass, &clipped_primitives, &screen_descriptor);
    }

    for id in &full_output.textures_delta.free {
        egui_state.renderer.free_texture(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_fresh_profiler_reports_zero_rather_than_dividing_by_zero() {
        let profiler = Profiler::new();
        assert_eq!(profiler.frame_time(), Duration::ZERO);
        assert_eq!(profiler.average_frame_time(), Duration::ZERO);
        assert_eq!(profiler.fps(), 0.0);
        assert!(profiler.section_time("never_ran").is_none());
        assert!(profiler.average_section_time("never_ran").is_none());
        assert!(profiler.section_names().is_empty());
    }

    #[test]
    fn tick_records_frame_time_and_fps_follows_from_it() {
        let profiler = Profiler::new();
        std::thread::sleep(Duration::from_millis(10));
        profiler.tick();

        assert!(profiler.frame_time() >= Duration::from_millis(10));
        assert!(profiler.fps() > 0.0 && profiler.fps() < 100.0);
    }

    #[test]
    fn a_section_guard_records_on_drop_not_on_creation() {
        let profiler = Profiler::new();
        assert!(profiler.section_time("work").is_none());

        {
            let _span = profiler.section("work");
            std::thread::sleep(Duration::from_millis(5));
            // Guard still alive — nothing recorded yet.
            assert!(profiler.section_time("work").is_none());
        } // _span drops here

        let recorded = profiler.section_time("work").expect("section should be recorded after the guard drops");
        assert!(recorded >= Duration::from_millis(5));
    }

    #[test]
    fn sections_nest_without_a_borrow_conflict() {
        // The whole point of Mutex-per-field interior mutability instead of
        // requiring &mut Profiler: starting an inner section while an outer
        // one from the same Profiler is still open must not deadlock or
        // fail to borrow-check.
        let profiler = Profiler::new();
        {
            let _outer = profiler.section("outer");
            {
                let _inner = profiler.section("inner");
            }
        }
        assert!(profiler.section_time("outer").is_some());
        assert!(profiler.section_time("inner").is_some());
    }

    #[test]
    fn reusing_a_section_name_accumulates_history_for_averaging() {
        let profiler = Profiler::new();
        for _ in 0..3 {
            let _span = profiler.section("repeated");
            std::thread::sleep(Duration::from_millis(1));
        }
        assert!(profiler.average_section_time("repeated").is_some());
        assert!(profiler.section_names().contains(&"repeated"));
    }
}
