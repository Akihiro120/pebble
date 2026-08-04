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
//! straight for `backend.device.create_*`/`wgpu::*Descriptor`: the
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
//! cover standalone compute dispatch the same way. `backend.device`/
//! `backend.queue` are still there for the handful of operations this
//! module doesn't wrap (submitting an encoder built some other way, a
//! resource shape [`texture_view::TextureBuilder`] doesn't cover) — but
//! everything that's a *handle* (buffer, texture, sampler, pipeline, bind
//! group, layout, pass, encoder) is opaque.

pub mod backend;
pub mod binding;
pub mod buffer;
pub mod buffers;
pub mod compute;
pub mod compute_pass;
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
pub mod render_pass;
pub mod samplers;
pub mod texture_array;
pub mod texture_view;
pub mod textures;
#[cfg(test)]
pub(crate) mod test_util;
pub mod window;
