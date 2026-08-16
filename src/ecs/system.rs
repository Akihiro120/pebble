/// When a system runs, each tick. Stages run in the order listed here.
#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub enum SystemStage {
    /// Once, before anything else — no GPU backend yet, pure CPU setup.
    Startup,
    /// Once, automatically, the first tick the GPU backend is ready. For
    /// one-time setup that needs `Backend`/`Assets<T>` — a plain
    /// `Read<Backend>` here is always safe, no `Option` guard needed.
    Ready,
    /// Uploads CPU-side assets to the GPU, retrying until dependencies are met.
    AssetSync,
    /// Before main game logic (input, timers, event aging).
    PreUpdate,
    /// Main game logic.
    Update,
    /// After main game logic.
    PostUpdate,
    /// Acquire the frame.
    PreRender,
    /// Issue draw calls.
    Render,
    /// Submit and present.
    PostRender,
}

/// Priority engine-owned `Ready` systems (built-in plugins like
/// `GraphicsPlugin`) should register with, via `.priority(ENGINE_READY_PRIORITY)`
/// — guarantees they run before user `Ready` systems left at the default
/// priority (0), so engine resources (`GlobalSamplers`, etc.) are already
/// present for any user setup that runs the same tick. An explicit
/// `.after(...)`/`.before(...)` on a user system still overrides this, same
/// as any other priority tie-break.
pub const ENGINE_READY_PRIORITY: i32 = i32::MAX;
