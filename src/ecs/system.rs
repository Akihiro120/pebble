#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub enum SystemStage {
    /// Runs once, before anything else — including before the GPU backend
    /// exists. No Backend, no asset system guarantees. Pure CPU-only
    /// bootstrapping.
    Startup,
    /// Runs once, automatically, the first tick the GPU backend is ready.
    /// This is where one-time setup that needs Backend or Assets<T>
    /// belongs — building meshes/materials/computes, anything that isn't
    /// meant to happen every tick. Every stage after this point already
    /// only ever runs once the backend exists, so a plain Read<Backend>
    /// here is always safe — no Option<Read<Backend>> guard needed; that
    /// pattern is only for the internal systems that run *before* this
    /// point to drive the readiness transition itself.
    Ready,
    AssetSync,
    PreUpdate,
    Update,
    PostUpdate,
    PreRender,
    Render,
    PostRender,
}
