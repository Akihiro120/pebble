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
