//! A ready-to-use `wgpu` [`Backend`](crate::rendering::backend::Backend)
//! implementation ([`backend::WGPUPlugin`]), plus a higher-level
//! descriptor-based material/mesh/texture/compute layer built on top of it
//! so most apps never touch raw `wgpu` types directly.
//!
//! Building something this layer doesn't already cover (a camera, a custom
//! compute buffer, anything constructed by hand inside a
//! [`LazyResource`](crate::assets::singleton_asset::LazyResource)/
//! [`Asset`](crate::assets::upload::Asset) impl)? Start with
//! [`prelude`] — `use pebble::wgpu::prelude::*;` — rather than reaching
//! straight for `backend.device.create_*`/`wgpu::*Descriptor`: the
//! [`binding`] and [`buffers`] modules it re-exports are all builders
//! (`BindGroupLayoutBuilder`, `BufferBuilder`, `BindGroupBuilder`, ...)
//! producing opaque types ([`buffer::Buffer`], [`textures::GPUTexture`],
//! [`samplers::Sampler`], ...) instead of raw `wgpu` ones — chained
//! `.method(...)` calls that fold in bookkeeping (duplicate `@binding(N)`
//! detection, dynamic-offset alignment) a hand-written descriptor won't give
//! you for free.
//!
//! **Not yet opaque** (raw `wgpu` types still appear in these signatures):
//! [`mesh::GPUMesh`]'s vertex/index buffers, [`material::GPUMaterial`]/
//! [`compute::GPUCompute`]'s pipelines, [`instance::GPUBindingInstance`]'s
//! bind group, and pass/dispatch recording itself
//! (`active.begin_pass(...)` still hands back a raw `wgpu::RenderPass`).
//! All of these flow directly into `wgpu::RenderPass`/`wgpu::ComputePass`
//! calls, which aren't wrapped yet — wrapping the resources that feed them
//! without also wrapping the pass-recording API itself would leave no way
//! to actually use them. Tracked as a follow-up phase.

pub mod backend;
pub mod binding;
pub mod buffer;
pub mod buffers;
pub mod compute;
pub mod cubemap;
mod gpu_context;
pub mod instance;
pub mod layout;
pub mod material;
pub mod mesh;
pub mod mipmap;
mod plugin_macros;
#[cfg(feature = "profiler")]
pub mod profiler;
pub mod prelude;
pub mod samplers;
pub mod texture_array;
pub mod textures;
#[cfg(test)]
pub(crate) mod test_util;
pub mod window;
