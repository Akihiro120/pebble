use std::marker::PhantomData;

use crate::{
    assets::{
        handle::Handle,
        storage::{Assets, ProcessedAssets, RawAssetHandle},
        upload::Asset,
    },
    ecs::system::Res,
    wgpu::{
        backend::WGPUBackend,
        binding::BindGroupTarget,
        buffer::Buffer,
        buffers::{BindGroup, BindGroupBuilder, BufferBuilder},
        flags::BufferUsages,
        samplers::{GlobalSamplers, SamplerKind},
    },
};

/// A concrete resource to bind for one named entry of a [`BindingInstance`],
/// pushed one at a time via [`BindingInstance::texture`]/[`sampler`](BindingInstance::sampler)/etc.
/// (or directly via [`BindingInstance::param`] for a dynamically-selected
/// kind). Its `name` is matched against the target's
/// [`BindingEntry::name`](super::binding::BindingEntry)s to find the right
/// `@binding(N)` — so this only needs to say *what* to bind, not *where*.
#[derive(Clone, PartialEq, Eq, Hash)]
pub enum BindingInstanceEntry {
    /// A processed [`GPUTexture`](super::textures::GPUTexture), by its
    /// source handle.
    Texture(RawAssetHandle),
    /// A processed [`GPUTextureArray`](super::texture_array::GPUTextureArray),
    /// by its source handle.
    TextureArray(RawAssetHandle),
    /// A processed [`GPUCubemap`](super::cubemap::GPUCubemap), by its
    /// source handle.
    Cubemap(RawAssetHandle),
    /// A sampler from the global sampler cache.
    Sampler(SamplerKind),
    /// Raw bytes uploaded into a uniform buffer owned by this instance —
    /// updatable later via [`GPUBindingInstance::update`].
    Uniform(Vec<u8>),
    /// Same as `Uniform` but for a storage buffer.
    Storage(Vec<u8>),
}

/// Source data for a [`GPUBindingInstance<T>`]: which `T` (a
/// [`GPUMaterial`](super::material::GPUMaterial) or
/// [`GPUCompute`](super::compute::GPUCompute)) to bind against, and the
/// concrete resource for each of its named binding entries.
///
/// `T` is a marker only — this holds no `T` value, just a
/// [`RawAssetHandle`] into whichever `ProcessedAssets<T>` store `T` lives
/// in. See the [`MaterialInstance`]/[`ComputeInstance`] aliases for the two
/// concrete instantiations. Fields are private — start from
/// [`BindingInstance::new`] and chain the per-kind binding methods below
/// (mirroring how [`BindGroupBuilder`] adds one resource per call) rather
/// than constructing one as a struct literal.
pub struct BindingInstance<T> {
    /// Handle to the target `T` (looked up in `ProcessedAssets<T>` at
    /// upload time).
    target: RawAssetHandle,
    /// `(entry name, resource)` pairs — every name must match a named
    /// binding entry on the target, or upload fails (see
    /// [`GPUBindingInstance`]'s `Asset::upload` impl).
    params: Vec<(&'static str, BindingInstanceEntry)>,
    _marker: PhantomData<fn() -> T>,
}

impl<T: Asset<WGPUBackend>> BindingInstance<T> {
    /// Start building an instance targeting `target` — the *source* handle
    /// for `T` (e.g. a `Handle<Material>` for a [`MaterialInstance`]),
    /// looked up in `ProcessedAssets<T>` once that target itself has
    /// finished uploading. Chain the binding methods below to add entries.
    pub fn new(target: Handle<T::Source>) -> Self {
        Self { target: target.id, params: Vec::new(), _marker: PhantomData }
    }

    /// Bind a processed [`GPUTexture`](super::textures::GPUTexture), by its
    /// source handle, under `name`.
    pub fn texture(mut self, name: &'static str, handle: Handle<super::textures::Texture>) -> Self {
        self.params.push((name, BindingInstanceEntry::Texture(handle.id)));
        self
    }

    /// Bind a processed [`GPUTextureArray`](super::texture_array::GPUTextureArray),
    /// by its source handle, under `name`.
    pub fn texture_array(
        mut self,
        name: &'static str,
        handle: Handle<super::texture_array::TextureArray>,
    ) -> Self {
        self.params.push((name, BindingInstanceEntry::TextureArray(handle.id)));
        self
    }

    /// Bind a processed [`GPUCubemap`](super::cubemap::GPUCubemap), by its
    /// source handle, under `name`.
    pub fn cubemap(mut self, name: &'static str, handle: Handle<super::cubemap::Cubemap>) -> Self {
        self.params.push((name, BindingInstanceEntry::Cubemap(handle.id)));
        self
    }

    /// Bind a sampler from the global sampler cache under `name`.
    pub fn sampler(mut self, name: &'static str, kind: SamplerKind) -> Self {
        self.params.push((name, BindingInstanceEntry::Sampler(kind)));
        self
    }

    /// Bind raw bytes as a uniform buffer owned by this instance, under
    /// `name` — updatable later via [`GPUBindingInstance::update`].
    pub fn uniform(mut self, name: &'static str, data: Vec<u8>) -> Self {
        self.params.push((name, BindingInstanceEntry::Uniform(data)));
        self
    }

    /// Same as [`uniform`](Self::uniform) but as a storage buffer.
    pub fn storage(mut self, name: &'static str, data: Vec<u8>) -> Self {
        self.params.push((name, BindingInstanceEntry::Storage(data)));
        self
    }

    /// Escape hatch for a dynamically-selected entry kind that doesn't fit
    /// the typed methods above (building entries in a loop over
    /// heterogeneous data, say). Prefer [`texture`](Self::texture)/
    /// [`sampler`](Self::sampler)/etc. when the kind is known statically.
    pub fn param(mut self, name: &'static str, entry: BindingInstanceEntry) -> Self {
        self.params.push((name, entry));
        self
    }

    /// Logs a WARN for an instance with no bound params at all — it
    /// wouldn't set anything in its target's bind group, almost always a
    /// sign the binding calls were forgotten rather than intentional.
    fn validate(&self) {
        if self.params.is_empty() {
            tracing::warn!(
                "BindingInstance::new(): no params — this instance won't bind anything against \
                 its target; did you forget to chain .texture(...)/.sampler(...)/etc.?"
            );
        }
    }

    /// Consume the builder and return the finished [`BindingInstance`] value.
    pub fn build(self) -> Self {
        self.validate();
        self
    }

    /// Consume the builder, insert into `assets` under `name`, and return
    /// the resulting [`Handle<BindingInstance<T>>`].
    pub fn build_asset(self, name: &str, assets: &mut Assets<Self>) -> Handle<Self> {
        self.validate();
        assets.insert(name, self)
    }
}

/// Looks up the `@binding(N)` a target declared under `name`. Returning
/// `None` for an unmatched name (rather than panicking) is what lets
/// `GPUBindingInstance::upload` turn a bad name into a `None` upload result
/// via `?` — the sync system retries next tick rather than treating it as
/// fatal (see [`Asset::upload`]).
pub fn binding_index(entries: &[super::binding::BindingEntry], name: &str) -> Option<u32> {
    entries.iter().find(|e| e.name == name).map(|e| e.binding)
}

/// An instance uploaded to the GPU: a bind group ready to set against its
/// target `T`'s pipeline, plus any owned uniform/storage buffers (from
/// [`BindingInstanceEntry::Uniform`]/`Storage`) updatable via
/// [`update`](Self::update). See the [`GPUMaterialInstance`]/
/// [`GPUComputeInstance`] aliases for the two concrete instantiations.
pub struct GPUBindingInstance<T> {
    pub target: RawAssetHandle,
    pub bind_group: BindGroup,
    /// Named buffers owned by this instance, used for updates.
    buffers: Vec<(&'static str, Buffer)>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> GPUBindingInstance<T> {
    /// Overwrite the buffer bound under `name` (the same name given to
    /// [`BindingInstance::uniform`]/[`storage`](BindingInstance::storage))
    /// with `data`. Logs a warning
    /// and does nothing if `name` doesn't match an owned buffer — most
    /// likely a typo, or `name` refers to a texture/sampler entry rather
    /// than a `Uniform`/`Storage` one.
    pub fn update(&self, name: &str, data: &[u8]) {
        match self.buffer(name) {
            Some(buf) => buf.write(data),
            None => tracing::warn!(
                "GPUBindingInstance::update: no bound buffer named '{name}' — check for a typo \
                 against the entries in this instance's BindingInstance"
            ),
        }
    }

    /// The owned buffer bound under `name` (originally passed to
    /// [`BindingInstance::uniform`]/[`storage`](BindingInstance::storage)),
    /// e.g. to
    /// [`Buffer::read`] a compute pass's result back to the CPU. `None` if
    /// `name` doesn't match an owned buffer.
    pub fn buffer(&self, name: &str) -> Option<&Buffer> {
        self.buffers.iter().find(|(n, _)| *n == name).map(|(_, buf)| buf)
    }
}

impl<T> Asset<WGPUBackend> for GPUBindingInstance<T>
where
    T: BindGroupTarget + 'static + Send + Sync,
{
    type Source = BindingInstance<T>;
    type Deps<'a> = (
        Res<'a, ProcessedAssets<T>>,
        Res<'a, ProcessedAssets<super::textures::GPUTexture>>,
        Res<'a, ProcessedAssets<super::texture_array::GPUTextureArray>>,
        Res<'a, ProcessedAssets<super::cubemap::GPUCubemap>>,
        Res<'a, GlobalSamplers>,
    );

    fn upload<'a>(
        source: &Self::Source,
        backend: &WGPUBackend,
        deps: &Self::Deps<'a>,
    ) -> Option<Self> {
        let (targets, textures, texture_arrays, cubemaps, samplers) = deps;
        let target = targets.get(source.target)?;

        // Built up front, before assembling the bind group below, so that
        // pass can borrow from a Vec that's no longer growing — a
        // `BindGroupBuilder` entry borrowed from a Vec slot can't coexist
        // with later pushes into that same Vec.
        let owned_buffers: Vec<(&'static str, Buffer)> = source
            .params
            .iter()
            .filter_map(|(name, entry)| match entry {
                // `COPY_SRC` in addition to the usual `.uniform()`/`.storage()`
                // pair — not just `.uniform()`/`.storage()` shorthand — so
                // `GPUBindingInstance::buffer(name).read()`/`read_as::<T>()`
                // (documented, real capability: reading a compute result back
                // to the CPU) actually works instead of failing wgpu's
                // `COPY_SRC` validation the first time anyone calls it.
                BindingInstanceEntry::Uniform(bytes) => Some((
                    *name,
                    BufferBuilder::new()
                        .usage(BufferUsages::UNIFORM | BufferUsages::COPY_DST | BufferUsages::COPY_SRC)
                        .data(bytes)
                        .build(backend),
                )),
                BindingInstanceEntry::Storage(bytes) => Some((
                    *name,
                    BufferBuilder::new()
                        .usage(BufferUsages::STORAGE | BufferUsages::COPY_DST | BufferUsages::COPY_SRC)
                        .data(bytes)
                        .build(backend),
                )),
                _ => None,
            })
            .collect();

        let mut builder = BindGroupBuilder::new(target.bind_group_layout());
        for (name, entry) in &source.params {
            let binding = binding_index(target.binding_entries(), name)?;
            builder = match entry {
                BindingInstanceEntry::Texture(id) => builder.texture_2d_at(binding, textures.get(*id)?),
                BindingInstanceEntry::TextureArray(id) => {
                    builder.texture_array_at(binding, texture_arrays.get(*id)?)
                }
                BindingInstanceEntry::Cubemap(id) => builder.texture_cubemap_at(binding, cubemaps.get(*id)?),
                BindingInstanceEntry::Sampler(kind) => builder.sampler_at(binding, samplers.get(*kind)),
                BindingInstanceEntry::Uniform(_) | BindingInstanceEntry::Storage(_) => {
                    let buf = &owned_buffers.iter().find(|(n, _)| n == name)?.1;
                    builder.buffer_at(binding, buf)
                }
            };
        }
        let bind_group = builder.build(backend);

        Some(Self {
            target: source.target,
            bind_group,
            buffers: owned_buffers,
            _marker: PhantomData,
        })
    }
}

/// A material instance uploaded to the GPU — [`GPUBindingInstance`] bound
/// against a [`GPUMaterial`](super::material::GPUMaterial).
pub type GPUMaterialInstance = GPUBindingInstance<super::material::GPUMaterial>;
/// Source data for a [`GPUMaterialInstance`].
pub type MaterialInstance = BindingInstance<super::material::GPUMaterial>;

/// A compute instance uploaded to the GPU — [`GPUBindingInstance`] bound
/// against a [`GPUCompute`](super::compute::GPUCompute).
pub type GPUComputeInstance = GPUBindingInstance<super::compute::GPUCompute>;
/// Source data for a [`GPUComputeInstance`].
pub type ComputeInstance = BindingInstance<super::compute::GPUCompute>;

crate::wgpu::plugin_macros::asset_plugin! {
    /// Registers the [`GPUMaterialInstance`] asset pipeline
    /// (`Assets<MaterialInstance>` → `ProcessedAssets<GPUMaterialInstance>`).
    /// Included by [`WGPUPlugin`](super::backend::WGPUPlugin); add directly
    /// only if you're assembling the `wgpu` module's plugins by hand.
    MaterialInstancePlugin, GPUMaterialInstance
}

crate::wgpu::plugin_macros::asset_plugin! {
    /// Registers the [`GPUComputeInstance`] asset pipeline
    /// (`Assets<ComputeInstance>` → `ProcessedAssets<GPUComputeInstance>`).
    /// Included by [`WGPUPlugin`](super::backend::WGPUPlugin); add directly
    /// only if you're assembling the `wgpu` module's plugins by hand.
    ComputeInstancePlugin, GPUComputeInstance
}
