use std::marker::PhantomData;

use crate::{
    assets::{handle::Handle, storage::Assets, upload::{Asset, AssetSource}},
    ecs::resources::Read,
    graphics::{
        pipeline::{
            binding::{BindGroupTarget, BindingEntry},
            buffers::{BindGroup, BindGroupBuilder, Buffer, BufferBuilder, DynamicBuffer},
            compute::Compute,
            cubemap::Cubemap,
            material::Material,
            samplers::{GlobalSamplers, SamplerKind},
            texture_array::TextureArray,
            texture_view::TextureView,
            textures::Texture,
        },
        render::Backend,
        types::flags::BufferUsages,
    },
};

/// One bound value in a [`BindingInstance`] — matched to its bind group slot
/// by name at upload time.
#[derive(Clone)]
pub enum BindingInstanceEntry {
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

/// A bind group asset for a [`Material`]/[`Compute`] target — named
/// textures/samplers/uniforms/storage buffers, matched to the target's
/// declared entries by name. Usually used via its aliases
/// [`MaterialInstance`]/[`ComputeInstance`].
pub struct BindingInstance<T> {
    target: Handle<T>,
    params: Vec<(&'static str, BindingInstanceEntry)>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> BindingInstance<T>
where
    T: Asset<Backend>,
    T::Processed: BindGroupTarget,
{
    pub fn new(target: Handle<T>) -> Self {
        Self { target, params: Vec::new(), _marker: PhantomData }
    }

    pub fn with_texture(mut self, name: &'static str, handle: Handle<Texture>) -> Self {
        self.params.push((name, BindingInstanceEntry::Texture(handle)));
        self
    }

    pub fn with_texture_array(mut self, name: &'static str, handle: Handle<TextureArray>) -> Self {
        self.params.push((name, BindingInstanceEntry::TextureArray(handle)));
        self
    }

    pub fn with_cubemap(mut self, name: &'static str, handle: Handle<Cubemap>) -> Self {
        self.params.push((name, BindingInstanceEntry::Cubemap(handle)));
        self
    }

    /// Binds an already-built [`TextureView`] directly — e.g. one mip level
    /// from [`GPUTexture::get_view`](super::textures::GPUTexture::get_view),
    /// or a standalone render target from [`RenderTargetTextureBuilder`](super::texture_view::RenderTargetTextureBuilder).
    /// Unlike `.with_texture`/`.with_texture_array`/`.with_cubemap`, no
    /// `Handle` lookup happens at upload time — `view` must already exist.
    pub fn with_texture_view(mut self, name: &'static str, view: TextureView) -> Self {
        self.params.push((name, BindingInstanceEntry::TextureView(view)));
        self
    }

    pub fn with_sampler(mut self, name: &'static str, kind: SamplerKind) -> Self {
        self.params.push((name, BindingInstanceEntry::Sampler(kind)));
        self
    }

    pub fn with_uniform(mut self, name: &'static str, data: Vec<u8>) -> Self {
        self.params.push((name, BindingInstanceEntry::Uniform(data)));
        self
    }

    pub fn with_storage(mut self, name: &'static str, data: Vec<u8>) -> Self {
        self.params.push((name, BindingInstanceEntry::Storage(data)));
        self
    }

    /// Binds an existing [`Buffer`] instead of uploading raw bytes — for a
    /// buffer you already built yourself (e.g. one a compute pass writes
    /// to, then another pass reads from). Unlike `.with_uniform`/`.with_storage`,
    /// no buffer is created here; `buffer` must already carry the usage
    /// flags this binding needs (`BufferUsages::UNIFORM` or `::STORAGE`,
    /// matching how the target's own entry for `name` was declared).
    pub fn with_buffer(mut self, name: &'static str, buffer: Buffer) -> Self {
        self.params.push((name, BindingInstanceEntry::Buffer(buffer)));
        self
    }

    /// Binds an existing [`DynamicBuffer`] — the dynamic-offset counterpart
    /// to `.with_buffer`. The target's own entry for `name` must have been
    /// declared with `BindingKind::dynamic_uniform_buffer`/`dynamic_storage_buffer`
    /// (`has_dynamic_offset: true`) to match, or bind group creation fails
    /// validation.
    pub fn with_dynamic_buffer(mut self, name: &'static str, buffer: DynamicBuffer) -> Self {
        self.params.push((name, BindingInstanceEntry::DynamicBuffer(buffer)));
        self
    }

    pub fn with_param(mut self, name: &'static str, entry: BindingInstanceEntry) -> Self {
        self.params.push((name, entry));
        self
    }

    fn validate(&self) {
        if self.params.is_empty() {
            tracing::warn!(
                "BindingInstance::new(): no params — this instance won't bind anything \
                 against its target; did you forget to chain .with_texture(...)/.with_sampler(...)/etc.?"
            );
        }
    }

    pub fn build_asset(self, name: &str, assets: &mut Assets<BindingInstance<T>>) -> Handle<BindingInstance<T>>
    where
        BindingInstance<T>: AssetSource,
    {
        self.validate();
        assets.insert(name, self)
    }
}

/// Looks up a target's bind group slot index by entry name.
pub fn binding_index(entries: &[BindingEntry], name: &str) -> Option<u32> {
    entries.iter().find(|e| e.name == name).map(|e| e.binding)
}

/// The GPU-resident bind group an uploaded [`BindingInstance`] produces.
pub struct GPUBindingInstance<T> {
    pub target: Handle<T>,
    pub bind_group: BindGroup,
    buffers: Vec<(&'static str, Buffer)>,
    dynamic_buffers: Vec<(&'static str, DynamicBuffer)>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> GPUBindingInstance<T> {
    /// Overwrites a named uniform/storage buffer's contents in place —
    /// avoids rebuilding the whole bind group for a per-frame update.
    pub fn update(&self, name: &str, data: &[u8]) {
        match self.buffer(name) {
            Some(buf) => buf.write(data),
            None => tracing::warn!(
                "GPUBindingInstance::update: no bound buffer named '{name}' — check for a typo \
                 against the entries in this instance's BindingInstance"
            ),
        }
    }

    pub fn buffer(&self, name: &str) -> Option<&Buffer> {
        self.buffers.iter().find(|(n, _)| *n == name).map(|(_, buf)| buf)
    }

    /// Same as `.buffer`, for a binding made via `.with_dynamic_buffer` —
    /// use `DynamicBuffer::write_element` on the result to update one
    /// element in place.
    pub fn dynamic_buffer(&self, name: &str) -> Option<&DynamicBuffer> {
        self.dynamic_buffers.iter().find(|(n, _)| *n == name).map(|(_, buf)| buf)
    }
}

impl<T> AssetSource for BindingInstance<T>
where
    T: Asset<Backend>,
    T::Processed: BindGroupTarget,
{
    type Processed = GPUBindingInstance<T>;
}

impl<T> Asset<Backend> for BindingInstance<T>
where
    T: Asset<Backend>,
    T::Processed: BindGroupTarget,
{
    type Deps<'a> = (
        Read<'a, Assets<T>>,
        Read<'a, Assets<Texture>>,
        Read<'a, Assets<TextureArray>>,
        Read<'a, Assets<Cubemap>>,
        Read<'a, GlobalSamplers>,
    );

    fn upload<'a>(&self, backend: &Backend, deps: &Self::Deps<'a>) -> Option<GPUBindingInstance<T>> {
        let (targets, textures, texture_arrays, cubemaps, samplers) = deps;
        let target = targets.get(self.target)?;

        // buffers backing `Uniform`/`Storage` entries are built fresh here;
        // a `Buffer` entry already exists — just cloned (cheap: it's a
        // handle to the same GPU buffer) so `GPUBindingInstance` can still
        // look it up by name later via `.update()`/`.buffer()`.
        let owned_buffers: Vec<(&'static str, Buffer)> = self
            .params
            .iter()
            .filter_map(|(name, entry)| match entry {
                BindingInstanceEntry::Uniform(bytes) => Some((
                    *name,
                    BufferBuilder::with_data(bytes)
                        .with_usage(BufferUsages::UNIFORM | BufferUsages::COPY_DST | BufferUsages::COPY_SRC)
                        .build(backend),
                )),
                BindingInstanceEntry::Storage(bytes) => Some((
                    *name,
                    BufferBuilder::with_data(bytes)
                        .with_usage(BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC)
                        .build(backend),
                )),
                BindingInstanceEntry::Buffer(buffer) => Some((*name, buffer.clone())),
                _ => None,
            })
            .collect();

        // same idea as `owned_buffers`, for `.with_dynamic_buffer` entries —
        // kept separate since binding one uses `with_dynamic_buffer_at`,
        // not `with_buffer_at`.
        let owned_dynamic_buffers: Vec<(&'static str, DynamicBuffer)> = self
            .params
            .iter()
            .filter_map(|(name, entry)| match entry {
                BindingInstanceEntry::DynamicBuffer(buffer) => Some((*name, buffer.clone())),
                _ => None,
            })
            .collect();

        let mut builder = BindGroupBuilder::new(target.bind_group_layout());
        for (name, entry) in &self.params {
            let binding = binding_index(target.binding_entries(), name)?;
            builder = match entry {
                BindingInstanceEntry::Texture(handle) => builder.with_texture_2d_at(binding, textures.get(*handle)?),
                BindingInstanceEntry::TextureArray(handle) => {
                    builder.with_texture_array_at(binding, texture_arrays.get(*handle)?)
                }
                BindingInstanceEntry::Cubemap(handle) => builder.with_texture_cubemap_at(binding, cubemaps.get(*handle)?),
                BindingInstanceEntry::TextureView(view) => builder.with_texture_view_at(binding, view),
                BindingInstanceEntry::Sampler(kind) => builder.with_sampler_at(binding, samplers.get(*kind)),
                BindingInstanceEntry::Uniform(_) | BindingInstanceEntry::Storage(_) | BindingInstanceEntry::Buffer(_) => {
                    let buf = &owned_buffers.iter().find(|(n, _)| n == name)?.1;
                    builder.with_buffer_at(binding, buf)
                }
                BindingInstanceEntry::DynamicBuffer(_) => {
                    let buf = &owned_dynamic_buffers.iter().find(|(n, _)| n == name)?.1;
                    builder.with_dynamic_buffer_at(binding, buf)
                }
            };
        }
        let bind_group = builder.build(backend);

        Some(GPUBindingInstance {
            target: self.target,
            bind_group,
            buffers: owned_buffers,
            dynamic_buffers: owned_dynamic_buffers,
            _marker: PhantomData,
        })
    }
}

pub type GPUMaterialInstance = GPUBindingInstance<Material>;
/// A [`Material`]'s bind group — the values a shader actually reads from
/// (textures, samplers, uniforms) for one draw.
pub type MaterialInstance = BindingInstance<Material>;

pub type GPUComputeInstance = GPUBindingInstance<Compute>;
/// A [`Compute`] pipeline's bind group — the buffers/textures it reads and
/// writes for one dispatch.
pub type ComputeInstance = BindingInstance<Compute>;
