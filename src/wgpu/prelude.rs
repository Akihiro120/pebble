//! `use pebble::wgpu::prelude::*;` for everything needed to construct
//! custom GPU resources against a [`WGPUBackend`] — a camera's uniform
//! buffer, a compute pass's storage buffers, anything built by hand inside
//! a [`LazyResource`](crate::assets::singleton_asset::LazyResource) or
//! [`Asset`](crate::assets::upload::Asset) impl that isn't already covered
//! by [`MaterialDescriptor`](super::material::MaterialDescriptor)/
//! [`ComputeDescriptor`](super::compute::ComputeDescriptor).
//!
//! Everything that constructs a GPU resource here is a builder —
//! [`BindGroupLayoutBuilder`], [`BufferBuilder`], [`DynamicBufferBuilder`],
//! [`BindGroupBuilder`] — chained `.method(...)` calls instead of a
//! hand-written `wgpu::BufferDescriptor`/`BindGroupLayoutDescriptor`/
//! `BindGroupDescriptor`, producing opaque [`Buffer`]/[`DynamicBuffer`]
//! values rather than raw `wgpu::Buffer` — there's no `wgpu::*` type
//! anywhere in this module's public surface. `backend.device`/`backend.queue`
//! are still there for the few operations this layer doesn't cover yet
//! (submitting command encoders, dispatching a compute pass — see the
//! `wgpu` module's own docs for what's still unwrapped and why), but for
//! buffers, textures, bind group layouts, and bind groups, reach for a
//! builder first: typing `SomeBuilder::new().` and letting autocomplete show
//! you what's available beats remembering a specific free function's name,
//! and each builder folds in bookkeeping a hand-written descriptor won't
//! (duplicate-`@binding(N)` detection in [`BindGroupLayoutBuilder::build`],
//! correct dynamic-offset alignment in [`DynamicBufferBuilder::build`]) that's
//! easy to get subtly wrong by hand and would otherwise show up as an opaque
//! wgpu validation panic instead of a clear one.
//!
//! See the book's [Camera, Depth, and Lazy Resources](https://akihiro120.github.io/pebble/ch10-camera-and-depth.html)
//! chapter for a worked example.

pub use super::backend::WGPUBackend;
pub use super::binding::{BindGroupLayoutBuilder, BindGroupTarget, BindingEntry, BindingKind};
pub use super::buffer::{Buffer, DynamicBuffer};
pub use super::buffers::{
    BindGroupBuilder, BufferBuilder, DynamicBufferBuilder, dynamic_storage_offset_stride,
    dynamic_uniform_offset_stride,
};
pub use super::cubemap::GPUCubemap;
pub use super::layout::{GroupLayout, OwnedGroupLayout, assemble_bind_group_layouts};
pub use super::samplers::{GlobalSamplers, Sampler, SamplerKind};
pub use super::texture_array::GPUTextureArray;
pub use super::textures::GPUTexture;
