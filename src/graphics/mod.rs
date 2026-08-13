use crate::{
    assets::plugin::AssetPlugin,
    ecs::{plugin::Plugin, system::SystemStage},
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
        window::WindowPlugin,
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
}

impl GraphicsPlugin {
    pub fn new() -> Self {
        Self {
            features: DeviceFeatures::empty(),
        }
    }

    pub fn with_features(features: DeviceFeatures) -> Self {
        Self { features }
    }
}

impl Plugin for GraphicsPlugin {
    fn build(self, app: crate::app::App) -> crate::app::App {
        app.add_plugin(WindowPlugin::default())
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
            .add_system(SystemStage::AssetSync, init_global_samplers)
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
