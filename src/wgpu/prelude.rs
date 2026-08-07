//! `use pebble::wgpu::prelude::*;` for everything needed to construct and
//! render custom GPU resources against a [`WGPUBackend`] — a camera's
//! uniform buffer, a compute pass's storage buffers, anything built by hand
//! inside a [`LazyResource`](crate::assets::singleton_asset::LazyResource)
//! or [`Asset`](crate::assets::upload::Asset) impl that isn't already
//! covered by [`Material`](super::material::Material)/
//! [`Compute`](super::compute::Compute).
//!
//! Everything that constructs a GPU resource here is a builder —
//! [`BindGroupLayoutBuilder`], [`BufferBuilder`], [`DynamicBufferBuilder`],
//! [`BindGroupBuilder`], [`TextureBuilder`], [`RenderBundleEncoderBuilder`] —
//! chained `.method(...)` calls instead of a hand-written
//! `wgpu::BufferDescriptor`/`BindGroupLayoutDescriptor`/`BindGroupDescriptor`,
//! producing opaque types ([`Buffer`], [`DynamicBuffer`], [`TextureView`],
//! [`BindGroupLayout`], [`BindGroup`], [`RenderBundleEncoder`], ...) rather
//! than raw `wgpu` ones — there's no `wgpu::*` type anywhere in this
//! module's public surface. Recording draw/dispatch calls is opaque too:
//! [`RenderPass`] (from `ActiveFrame::begin_pass`) and [`ComputePass`] (from
//! [`WGPUBackend::create_command_encoder`]/[`CommandEncoder::compute_pass`])
//! take these types directly.
//!
//! `backend.device`/`backend.queue` aren't reachable at all — every builder
//! here takes `&WGPUBackend` directly instead. For buffers, textures, bind
//! groups, and pass recording, reach for a builder: typing
//! `SomeBuilder::new().` and letting autocomplete show you what's available
//! beats remembering a specific free function's name, and each builder folds
//! in bookkeeping a hand-written descriptor won't (duplicate-`@binding(N)`
//! detection in [`BindGroupLayoutBuilder::build`], correct dynamic-offset
//! alignment in [`DynamicBufferBuilder::build`]) that's easy to get subtly
//! wrong by hand and would otherwise show up as an opaque wgpu validation
//! panic instead of a clear one.
//!
//! See the book's [Custom GPU Resources](https://akihiro120.github.io/pebble/custom-gpu-resources.html)
//! page for a worked example.

pub use super::backend::WGPUBackend;
pub use super::binding::{
    BindGroupLayout, BindGroupLayoutBuilder, BindGroupTarget, BindingEntry, BindingKind, StorageTextureAccess,
    TextureSampleType, TextureViewDimension,
};
pub use super::buffer::{Buffer, DynamicBuffer};
pub use super::buffers::{
    BindGroup, BindGroupBuilder, BufferBuilder, DynamicBufferBuilder, dynamic_storage_offset_stride,
    dynamic_uniform_offset_stride,
};
pub use super::compute::ComputePipeline;
pub use super::compute_pass::{CommandEncoder, ComputePass, DispatchIndirectArgs};
pub use super::cubemap::GPUCubemap;
pub use super::flags::{BufferUsages, ColorWrites, ShaderStages, TextureUsages};
pub use super::keycode::{KeyCode, MouseButton};
pub use super::layout::{GlobalLayoutPool, GroupEntry};
pub use super::material::{
    BlendComponent, BlendFactor, BlendOperation, BlendState, ColorTargetState, CompareFunction, DepthBiasState,
    DepthStencilState, Face, PolygonMode, RenderPipeline, StencilFaceState, StencilOperation, StencilState,
};
pub use super::render_bundle::{RenderBundle, RenderBundleEncoder, RenderBundleEncoderBuilder};
pub use super::render_pass::{DrawIndexedIndirectArgs, DrawIndirectArgs, IndexFormat, RenderPass};
pub use super::samplers::{GlobalSamplers, Sampler, SamplerKind};
pub use super::texture_array::GPUTextureArray;
pub use super::texture_format::{AstcBlock, AstcChannel, TextureFormat};
pub use super::texture_view::{TextureBuilder, TextureView};
pub use super::textures::GPUTexture;
pub use super::vertex_format::{VertexAttribute, VertexBufferLayout, VertexFormat, VertexStepMode};
pub use super::window::Input;
