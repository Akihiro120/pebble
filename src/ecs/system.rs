#[derive(Copy, Clone, PartialEq, PartialOrd, Eq, Ord, Hash)]
pub enum SystemStage {
    Startup,
    PreUpdate,
    Update,
    PostUpdate,
    PreRender,
    Render,
    PostRender,
}
