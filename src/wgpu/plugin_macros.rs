//! Crate-internal codegen for the `wgpu` module's per-asset-type `Plugin`s.
//! Not a public module — the generated types (`MeshPlugin`, `MaterialPlugin`,
//! `TexturePlugin`, ...) are exported from the modules that invoke these
//! macros, same as if they'd been hand-written there.

/// Declares a zero-config `Plugin` that registers a single
/// `AssetPlugin::<WGPUBackend, $gpu_ty>` — the shape shared by most of this
/// module's per-asset-type plugins (`MeshPlugin`, `MaterialPlugin`,
/// `ComputePlugin`, `MaterialInstancePlugin`, `ComputeInstancePlugin`),
/// which otherwise only differ in this one type parameter. Doc comments
/// written inside the invocation (before the name) become the generated
/// struct's own docs, same as if written by hand.
macro_rules! asset_plugin {
    ($(#[$doc:meta])* $name:ident, $gpu_ty:ty) => {
        $(#[$doc])*
        #[derive(Default)]
        pub struct $name;
        impl $name {
            pub fn new() -> Self {
                Self
            }
        }
        impl crate::ecs::plugin::Plugin for $name {
            fn build(&self, app: &mut crate::app::App) {
                app.add_plugin(crate::assets::plugin::AssetPlugin::<
                    crate::wgpu::backend::WGPUBackend,
                    $gpu_ty,
                >::new());
            }
        }
    };
}

/// Same as [`asset_plugin!`] but also registers the startup system for
/// [`MipmapGenerator`](crate::wgpu::mipmap::MipmapGenerator) — the shape
/// used by the texture-like plugins (`TexturePlugin`, `CubemapPlugin`,
/// `TextureArrayPlugin`) whose descriptors support `generate_mips`.
macro_rules! mipmap_asset_plugin {
    ($(#[$doc:meta])* $name:ident, $gpu_ty:ty) => {
        $(#[$doc])*
        #[derive(Default)]
        pub struct $name;
        impl $name {
            pub fn new() -> Self {
                Self
            }
        }
        impl crate::ecs::plugin::Plugin for $name {
            fn build(&self, app: &mut crate::app::App) {
                app.add_system(
                    crate::app::SystemStage::Startup,
                    crate::wgpu::mipmap::init_mipmap_generator,
                );
                app.add_plugin(crate::assets::plugin::AssetPlugin::<
                    crate::wgpu::backend::WGPUBackend,
                    $gpu_ty,
                >::new());
            }
        }
    };
}

pub(crate) use asset_plugin;
pub(crate) use mipmap_asset_plugin;
