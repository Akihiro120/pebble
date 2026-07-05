pub mod app;
pub mod assets;
pub mod plugin;
pub mod prelude;
pub mod rendering;
pub mod resources;
pub mod system;

pub use app::{App, SystemStage};
pub use assets::{
    DeviceAssetPlugin,
    handle::Handle,
    storage::{AssetHandle, Assets, GPUAssets},
    upload::DeviceUpload,
    *,
};
pub use plugin::Plugin;
pub use rendering::{
    backend::*,
    graphics_plugin::GraphicsPlugin,
    render_plugin::RenderPlugin,
    web_backend::{AsyncGraphicsPlugin, AsyncInit},
    window::*,
    window_plugin::WindowPlugin,
};
pub use resources::Resources;
pub use system::{Commands, IntoSystem, Query, Res, ResMut, System};
