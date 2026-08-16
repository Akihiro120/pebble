use crate::{
    assets::plugin::AssetPlugin,
    ecs::{
        plugin::Plugin,
        system::{ENGINE_READY_PRIORITY, SystemStage},
        system_param::IntoSystemConfig,
    },
    graphics::{
        pipeline::{
            compute::Compute,
            cubemap::Cubemap,
            instance::{ComputeInstance, MaterialInstance},
            layout::GlobalLayoutPool,
            material::Material,
            mesh::Mesh,
            mipmap::init_mipmap_generator,
            samplers::init_global_samplers,
            texture_array::TextureArray,
            textures::Texture,
        },
        render::{Backend, BackendPlugin},
        types::flags::DeviceFeatures,
        window::{WindowConfig, WindowPlugin},
    },
};

pub mod pipeline;
pub mod render;
pub mod types;
pub mod window;

/// Windowing + GPU backend + every built-in asset type
/// (`Mesh`/`Texture`/`TextureArray`/`Cubemap`/`Material`/`Compute`/
/// `MaterialInstance`/`ComputeInstance`), all in one plugin. The usual
/// starting point for an app that renders anything.
pub struct GraphicsPlugin {
    features: DeviceFeatures,
    window: WindowConfig,
}

impl GraphicsPlugin {
    pub fn new() -> Self {
        Self {
            features: DeviceFeatures::empty(),
            window: WindowConfig::default(),
        }
    }

    pub fn with_features(features: DeviceFeatures) -> Self {
        Self {
            features,
            window: WindowConfig::default(),
        }
    }

    /// Overrides the window title/size this plugin opens (default: 1280x720, "Pebble").
    /// Don't add a separate `WindowPlugin` alongside `GraphicsPlugin` — it already
    /// includes one, and adding both opens two windows.
    pub fn with_window(mut self, window: WindowConfig) -> Self {
        self.window = window;
        self
    }
}

impl Plugin for GraphicsPlugin {
    fn build(self, app: crate::app::App) -> crate::app::App {
        app.add_plugin(WindowPlugin::new(self.window))
            .add_plugin(BackendPlugin::with_features(self.features))
            .add_plugin(BuiltinAssetsPlugin)
    }
}

/// Just the built-in asset types, without windowing — part of what
/// [`GraphicsPlugin`] registers; add directly only if you're assembling
/// your own windowing/backend setup around it.
pub struct BuiltinAssetsPlugin;
impl Plugin for BuiltinAssetsPlugin {
    fn build(self, app: crate::app::App) -> crate::app::App {
        app.insert_resource(GlobalLayoutPool::default())
            .add_system(SystemStage::Ready, init_global_samplers.priority(ENGINE_READY_PRIORITY))
            .add_system(SystemStage::AssetSync, init_mipmap_generator)
            .add_plugin(AssetPlugin::<Backend, Mesh>::new())
            .add_plugin(AssetPlugin::<Backend, Texture>::new())
            .add_plugin(AssetPlugin::<Backend, TextureArray>::new())
            .add_plugin(AssetPlugin::<Backend, Cubemap>::new())
            .add_plugin(AssetPlugin::<Backend, Material>::new())
            .add_plugin(AssetPlugin::<Backend, Compute>::new())
            .add_plugin(AssetPlugin::<Backend, MaterialInstance>::new())
            .add_plugin(AssetPlugin::<Backend, ComputeInstance>::new())
    }
}

#[cfg(test)]
mod test {}
