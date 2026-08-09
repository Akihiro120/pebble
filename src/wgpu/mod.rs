//! A ready-to-use `wgpu` [`Backend`](crate::rendering::backend::Backend)
//! implementation ([`backend::WGPUPlugin`]), plus a higher-level
//! descriptor-based material/mesh/texture/compute layer built on top of it
//! so apps never touch raw `wgpu` types directly.
//!
//! Building something this layer doesn't already cover (a camera, a custom
//! compute buffer, anything constructed by hand inside a
//! [`LazyResource`](crate::assets::singleton_asset::LazyResource)/
//! [`Asset`](crate::assets::upload::Asset) impl)? Start with
//! [`prelude`] — `use pebble::wgpu::prelude::*;` — rather than reaching
//! for a hand-written `wgpu::*Descriptor`: the
//! [`binding`] and [`buffers`] modules it re-exports are all builders
//! (`BindGroupLayoutBuilder`, `BufferBuilder`, `BindGroupBuilder`, ...)
//! producing opaque types ([`buffer::Buffer`], [`textures::GPUTexture`],
//! [`samplers::Sampler`], [`material::RenderPipeline`], [`buffers::BindGroup`],
//! ...) instead of raw `wgpu` ones — chained `.method(...)` calls that fold
//! in bookkeeping (duplicate `@binding(N)` detection, dynamic-offset
//! alignment) a hand-written descriptor won't give you for free.
//!
//! Pass and dispatch recording are opaque too:
//! [`ActiveFrame::begin_pass`](crate::rendering::active_frame::ActiveFrame::begin_pass)
//! hands back a [`render_pass::RenderPass`] (not a raw `wgpu::RenderPass`),
//! and [`WGPUBackend::create_command_encoder`](backend::WGPUBackend::create_command_encoder)/
//! [`CommandEncoder::compute_pass`](compute_pass::CommandEncoder::compute_pass)
//! cover standalone compute dispatch the same way. `WGPUBackend`'s
//! `device`/`queue`/`surface` fields are `pub(crate)` — every builder here
//! takes `&WGPUBackend` directly instead — and even the *value* types
//! (texture formats, shader stages, buffer/texture usage flags, blend and
//! depth/stencil state, vertex formats) are mirrored as Pebble's own types
//! ([`texture_format::TextureFormat`], [`flags::ShaderStages`], ...)
//! rather than re-exporting `wgpu`'s. Nothing in this module's public
//! surface names a raw `wgpu::*` type.

pub mod animation;
pub mod backend;
pub mod binding;
pub mod buffer;
pub mod buffers;
pub mod compute;
pub mod compute_pass;
pub mod cubemap;
pub mod cursor;
pub mod flags;
mod gpu_context;
pub mod gltf_loader;
pub mod instance;
pub mod keycode;
pub mod layout;
pub mod material;
pub mod mesh;
pub mod mipmap;
pub mod player;
mod plugin_macros;
#[cfg(feature = "profiler")]
pub mod profiler;
pub mod prelude;
pub mod render_bundle;
pub mod render_pass;
pub mod samplers;
pub mod skeleton;
pub mod skinning;
pub mod skinned_mesh;
pub mod texture_array;
pub mod texture_format;
pub mod texture_view;
pub mod textures;
#[cfg(test)]
pub(crate) mod test_util;
pub mod vertex_format;
pub mod window;
