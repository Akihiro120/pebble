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
//! (`BindGroupLayoutBuilder`, `BufferBuilder`, `BindGroupBuilder`, ...) —
//! chained `.method(...)` calls that fold in bookkeeping (duplicate
//! `@binding(N)` detection, dynamic-offset alignment) a hand-written
//! descriptor won't give you for free.

pub mod backend;
pub mod binding;
pub mod buffers;
pub mod compute;
pub mod cubemap;
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
