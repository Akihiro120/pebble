use crate::{
    assets::{handle::Handle, storage::Assets},
    graphics::{
        pipeline::{
            binding::{BindGroupLayout, BindingEntry},
            buffers::{BindGroup, BindGroupBuilder, Buffer, BufferBuilder, DynamicBuffer},
            cubemap::Cubemap,
            samplers::{GlobalSamplers, SamplerKind},
            texture_array::TextureArray,
            texture_view::TextureView,
            textures::Texture,
        },
        render::Backend,
        types::flags::BufferUsages,
    },
};

/// One bound value in a [`BindGroupParams`] — matched to its bind group slot
/// by name at upload time.
#[derive(Clone)]
pub enum BindingValue {
    Texture(Handle<Texture>),
    TextureArray(Handle<TextureArray>),
    Cubemap(Handle<Cubemap>),
    TextureView(TextureView),
    Sampler(SamplerKind),
    Uniform(Vec<u8>),
    Storage(Vec<u8>),
    Buffer(Buffer),
    DynamicBuffer(DynamicBuffer),
}

/// A [`Material`](super::material::Material)/[`Compute`](super::compute::Compute)'s
/// named bind group values — textures/samplers/uniforms/storage buffers,
/// matched to the pipeline's own declared entries by name at upload time.
/// `Material`/`Compute` each hold one of these and expose their `with_*`
/// methods as thin delegates, so this is rarely named directly.
#[derive(Clone, Default)]
pub struct BindGroupParams {
    params: Vec<(&'static str, BindingValue)>,
}

impl BindGroupParams {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn with_texture(mut self, name: &'static str, handle: Handle<Texture>) -> Self {
        self.params.push((name, BindingValue::Texture(handle)));
        self
    }

    pub fn with_texture_array(mut self, name: &'static str, handle: Handle<TextureArray>) -> Self {
        self.params.push((name, BindingValue::TextureArray(handle)));
        self
    }

    pub fn with_cubemap(mut self, name: &'static str, handle: Handle<Cubemap>) -> Self {
        self.params.push((name, BindingValue::Cubemap(handle)));
        self
    }

    /// Binds an already-built [`TextureView`] directly — e.g. one mip level
    /// from [`GPUTexture::get_view`](crate::graphics::pipeline::textures::GPUTexture::get_view),
    /// or a standalone render target from
    /// [`Texture::empty`](crate::graphics::pipeline::textures::Texture::empty).
    /// Unlike `.with_texture`/`.with_texture_array`/`.with_cubemap`, no
    /// `Handle` lookup happens at upload time — `view` must already exist.
    pub fn with_texture_view(mut self, name: &'static str, view: TextureView) -> Self {
        self.params.push((name, BindingValue::TextureView(view)));
        self
    }

    pub fn with_sampler(mut self, name: &'static str, kind: SamplerKind) -> Self {
        self.params.push((name, BindingValue::Sampler(kind)));
        self
    }

    pub fn with_uniform(mut self, name: &'static str, data: Vec<u8>) -> Self {
        self.params.push((name, BindingValue::Uniform(data)));
        self
    }

    pub fn with_storage(mut self, name: &'static str, data: Vec<u8>) -> Self {
        self.params.push((name, BindingValue::Storage(data)));
        self
    }

    /// Same as [`with_uniform`](Self::with_uniform), but takes a typed
    /// value instead of pre-packed bytes — uses `encase` to lay it out with
    /// correct WGSL `uniform` (std140) alignment. `T` is usually a struct
    /// deriving `encase::ShaderType`.
    pub fn with_uniform_value<T>(self, name: &'static str, value: &T) -> Self
    where
        T: encase::ShaderType + encase::internal::WriteInto,
    {
        let mut buffer = encase::UniformBuffer::new(Vec::new());
        buffer
            .write(value)
            .expect("encase: failed to write uniform value — this shouldn't happen for a #[derive(ShaderType)] struct");
        self.with_uniform(name, buffer.into_inner())
    }

    /// Same as [`with_storage`](Self::with_storage), but takes a typed
    /// value instead of pre-packed bytes — uses `encase` to lay it out with
    /// correct WGSL `storage` (std430) alignment.
    pub fn with_storage_value<T>(self, name: &'static str, value: &T) -> Self
    where
        T: encase::ShaderType + encase::internal::WriteInto,
    {
        let mut buffer = encase::StorageBuffer::new(Vec::new());
        buffer
            .write(value)
            .expect("encase: failed to write storage value — this shouldn't happen for a #[derive(ShaderType)] struct");
        self.with_storage(name, buffer.into_inner())
    }

    /// Binds an existing [`Buffer`] instead of uploading raw bytes — for a
    /// buffer you already built yourself (e.g. one a compute pass writes
    /// to, then another pass reads from). Unlike `.with_uniform`/`.with_storage`,
    /// no buffer is created here; `buffer` must already carry the usage
    /// flags this binding needs (`BufferUsages::UNIFORM` or `::STORAGE`,
    /// matching how the target's own entry for `name` was declared).
    pub fn with_buffer(mut self, name: &'static str, buffer: Buffer) -> Self {
        self.params.push((name, BindingValue::Buffer(buffer)));
        self
    }

    /// Binds an existing [`DynamicBuffer`] — the dynamic-offset counterpart
    /// to `.with_buffer`. The target's own entry for `name` must have been
    /// declared with `BindingKind::dynamic_uniform_buffer`/`dynamic_storage_buffer`
    /// (`has_dynamic_offset: true`) to match, or bind group creation fails
    /// validation.
    pub fn with_dynamic_buffer(mut self, name: &'static str, buffer: DynamicBuffer) -> Self {
        self.params.push((name, BindingValue::DynamicBuffer(buffer)));
        self
    }

    pub fn with_param(mut self, name: &'static str, entry: BindingValue) -> Self {
        self.params.push((name, entry));
        self
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.params.is_empty()
    }
}

/// Looks up a target's bind group slot index by entry name.
pub(crate) fn binding_index(entries: &[BindingEntry], name: &str) -> Option<u32> {
    entries.iter().find(|e| e.name == name).map(|e| e.binding)
}

/// The GPU-resident buffers a [`BindGroupParams`] resolves into, alongside
/// the [`BindGroup`] itself — returned by [`build_bind_group`] for a
/// [`Material`](super::material::Material)/[`Compute`](super::compute::Compute)
/// to fold into its own processed form.
pub(crate) struct BuiltBindGroup {
    pub bind_group: BindGroup,
    pub buffers: Vec<(&'static str, Buffer)>,
    pub dynamic_buffers: Vec<(&'static str, DynamicBuffer)>,
}

/// Resolves `params` against `layout`/`entries` (a pipeline's own bind group
/// shape) into a real `BindGroup` — the shared body of `Material::upload`/
/// `Compute::upload`, since both need the exact same "named values → bind
/// group" resolution once their pipeline is in hand.
pub(crate) fn build_bind_group(
    backend: &Backend,
    params: &BindGroupParams,
    layout: &BindGroupLayout,
    entries: &[BindingEntry],
    textures: &Assets<Texture>,
    texture_arrays: &Assets<TextureArray>,
    cubemaps: &Assets<Cubemap>,
    samplers: &GlobalSamplers,
) -> Option<BuiltBindGroup> {
    // buffers backing `Uniform`/`Storage` entries are built fresh here; a
    // `Buffer` entry already exists — just cloned (cheap: it's a handle to
    // the same GPU buffer) so the result can still look it up by name later
    // via `.update()`/`.buffer()`.
    let owned_buffers: Vec<(&'static str, Buffer)> = params
        .params
        .iter()
        .filter_map(|(name, entry)| match entry {
            BindingValue::Uniform(bytes) => Some((
                *name,
                BufferBuilder::with_data(bytes)
                    .with_usage(BufferUsages::UNIFORM | BufferUsages::COPY_DST | BufferUsages::COPY_SRC)
                    .build(backend),
            )),
            BindingValue::Storage(bytes) => Some((
                *name,
                BufferBuilder::with_data(bytes)
                    .with_usage(BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC)
                    .build(backend),
            )),
            BindingValue::Buffer(buffer) => Some((*name, buffer.clone())),
            _ => None,
        })
        .collect();

    // same idea as `owned_buffers`, for `.with_dynamic_buffer` entries —
    // kept separate since binding one uses `with_dynamic_buffer_at`, not
    // `with_buffer_at`.
    let owned_dynamic_buffers: Vec<(&'static str, DynamicBuffer)> = params
        .params
        .iter()
        .filter_map(|(name, entry)| match entry {
            BindingValue::DynamicBuffer(buffer) => Some((*name, buffer.clone())),
            _ => None,
        })
        .collect();

    let mut builder = BindGroupBuilder::new(layout);
    for (name, entry) in &params.params {
        let binding = binding_index(entries, name)?;
        builder = match entry {
            BindingValue::Texture(handle) => builder.with_texture_2d_at(binding, textures.get(*handle)?),
            BindingValue::TextureArray(handle) => builder.with_texture_array_at(binding, texture_arrays.get(*handle)?),
            BindingValue::Cubemap(handle) => builder.with_texture_cubemap_at(binding, cubemaps.get(*handle)?),
            BindingValue::TextureView(view) => builder.with_texture_view_at(binding, view),
            BindingValue::Sampler(kind) => builder.with_sampler_at(binding, samplers.get(*kind)),
            BindingValue::Uniform(_) | BindingValue::Storage(_) | BindingValue::Buffer(_) => {
                let buf = &owned_buffers.iter().find(|(n, _)| n == name)?.1;
                builder.with_buffer_at(binding, buf)
            }
            BindingValue::DynamicBuffer(_) => {
                let buf = &owned_dynamic_buffers.iter().find(|(n, _)| n == name)?.1;
                builder.with_dynamic_buffer_at(binding, buf)
            }
        };
    }

    Some(BuiltBindGroup { bind_group: builder.build(backend), buffers: owned_buffers, dynamic_buffers: owned_dynamic_buffers })
}
