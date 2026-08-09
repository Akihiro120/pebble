use std::marker::PhantomData;

use crate::{
    assets::{
        handle::Handle,
        storage::{Assets, RawAssetHandle},
        upload::{Asset, AssetSource},
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
/// pushed one at a time via [`BindingInstanceBuilder::texture`]/[`sampler`](BindingInstanceBuilder::sampler)/etc.
/// (or directly via [`BindingInstanceBuilder::param`] for a dynamically-selected
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
/// concrete instantiations. Fields are private — the only way to construct
/// one is [`BindingInstanceBuilder`] (see also the
/// [`MaterialInstanceBuilder`]/[`ComputeInstanceBuilder`] aliases):
/// `BindingInstanceBuilder::new(target).build()`.
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

/// Builds a [`BindingInstance<T>`]. Start from [`new`](Self::new), chain
/// the per-kind binding methods below (mirroring how [`BindGroupBuilder`]
/// adds one resource per call), then finish with
/// [`build`](Self::build)/[`build_asset`](Self::build_asset). See the
/// [`MaterialInstanceBuilder`]/[`ComputeInstanceBuilder`] aliases for the
/// two concrete instantiations.
pub struct BindingInstanceBuilder<T> {
    target: RawAssetHandle,
    params: Vec<(&'static str, BindingInstanceEntry)>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> BindingInstanceBuilder<T>
where
    T: Asset<WGPUBackend>,
    T::Processed: BindGroupTarget,
{
    /// Start building an instance targeting `target` — a `Handle<T>` for the
    /// source asset (e.g. a `Handle<Material>` for a [`MaterialInstance`]).
    pub fn new(target: Handle<T>) -> Self {
        Self { target: target.id, params: Vec::new(), _marker: PhantomData }
    }

    /// Bind a processed [`GPUTexture`](super::textures::GPUTexture), by its
    /// source handle, under `name`.
    pub fn with_texture(mut self, name: &'static str, handle: Handle<super::textures::Texture>) -> Self {
        self.params.push((name, BindingInstanceEntry::Texture(handle.id)));
        self
    }

    /// Bind a processed [`GPUTextureArray`](super::texture_array::GPUTextureArray),
    /// by its source handle, under `name`.
    pub fn with_texture_array(
        mut self,
        name: &'static str,
        handle: Handle<super::texture_array::TextureArray>,
    ) -> Self {
        self.params.push((name, BindingInstanceEntry::TextureArray(handle.id)));
        self
    }

    /// Bind a processed [`GPUCubemap`](super::cubemap::GPUCubemap), by its
    /// source handle, under `name`.
    pub fn with_cubemap(mut self, name: &'static str, handle: Handle<super::cubemap::Cubemap>) -> Self {
        self.params.push((name, BindingInstanceEntry::Cubemap(handle.id)));
        self
    }

    /// Bind a sampler from the global sampler cache under `name`.
    pub fn with_sampler(mut self, name: &'static str, kind: SamplerKind) -> Self {
        self.params.push((name, BindingInstanceEntry::Sampler(kind)));
        self
    }

    /// Bind raw bytes as a uniform buffer owned by this instance, under
    /// `name` — updatable later via [`GPUBindingInstance::update`].
    pub fn with_uniform(mut self, name: &'static str, data: Vec<u8>) -> Self {
        self.params.push((name, BindingInstanceEntry::Uniform(data)));
        self
    }

    /// Same as [`with_uniform`](Self::with_uniform) but as a storage buffer.
    pub fn with_storage(mut self, name: &'static str, data: Vec<u8>) -> Self {
        self.params.push((name, BindingInstanceEntry::Storage(data)));
        self
    }

    /// Escape hatch for a dynamically-selected entry kind that doesn't fit
    /// the typed methods above (building entries in a loop over
    /// heterogeneous data, say). Prefer [`with_texture`](Self::with_texture)/
    /// [`with_sampler`](Self::with_sampler)/etc. when the kind is known statically.
    pub fn with_param(mut self, name: &'static str, entry: BindingInstanceEntry) -> Self {
        self.params.push((name, entry));
        self
    }

    /// Logs a WARN for an instance with no bound params at all — it
    /// wouldn't set anything in its target's bind group, almost always a
    /// sign the binding calls were forgotten rather than intentional.
    fn validate(&self) {
        if self.params.is_empty() {
            tracing::warn!(
                "BindingInstanceBuilder::new(): no params — this instance won't bind anything \
                 against its target; did you forget to chain .with_texture(...)/.with_sampler(...)/etc.?"
            );
        }
    }

    /// Consume the builder and return the finished [`BindingInstance`] value.
    pub fn build(self) -> BindingInstance<T> {
        self.validate();
        BindingInstance { target: self.target, params: self.params, _marker: PhantomData }
    }

    /// Consume the builder, insert into `assets` under `name`, and return
    /// the resulting [`Handle<BindingInstance<T>>`].
    pub fn build_asset(self, name: &str, assets: &mut Assets<BindingInstance<T>>) -> Handle<BindingInstance<T>>
    where
        BindingInstance<T>: AssetSource,
    {
        let instance = self.build();
        assets.insert(name, instance)
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
    /// [`BindingInstanceBuilder::uniform`]/[`storage`](BindingInstanceBuilder::storage))
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
    /// [`BindingInstanceBuilder::uniform`]/[`storage`](BindingInstanceBuilder::storage)),
    /// e.g. to
    /// [`Buffer::read`] a compute pass's result back to the CPU. `None` if
    /// `name` doesn't match an owned buffer.
    pub fn buffer(&self, name: &str) -> Option<&Buffer> {
        self.buffers.iter().find(|(n, _)| *n == name).map(|(_, buf)| buf)
    }
}

impl<T> AssetSource for BindingInstance<T>
where
    T: Asset<WGPUBackend>,
    T::Processed: BindGroupTarget,
{
    type Processed = GPUBindingInstance<T>;
}

impl<T> Asset<WGPUBackend> for BindingInstance<T>
where
    T: Asset<WGPUBackend>,
    T::Processed: BindGroupTarget,
{
    type Deps<'a> = (
        Res<'a, Assets<T>>,
        Res<'a, Assets<super::textures::Texture>>,
        Res<'a, Assets<super::texture_array::TextureArray>>,
        Res<'a, Assets<super::cubemap::Cubemap>>,
        Res<'a, GlobalSamplers>,
    );

    fn upload<'a>(
        &self,
        backend: &WGPUBackend,
        deps: &Self::Deps<'a>,
    ) -> Option<GPUBindingInstance<T>> {
        let (targets, textures, texture_arrays, cubemaps, samplers) = deps;
        let target = targets.get(Handle::<T>::new(self.target))?;

        // Built up front, before assembling the bind group below, so that
        // pass can borrow from a Vec that's no longer growing — a
        // `BindGroupBuilder` entry borrowed from a Vec slot can't coexist
        // with later pushes into that same Vec.
        let owned_buffers: Vec<(&'static str, Buffer)> = self
            .params
            .iter()
            .filter_map(|(name, entry)| match entry {
                // `COPY_SRC` in addition to the usual `.with_uniform()`/`.with_storage()`
                // pair — not just `.with_uniform()`/`.with_storage()` shorthand — so
                // `GPUBindingInstance::buffer(name).read()`/`read_as::<T>()`
                // (documented, real capability: reading a compute result back
                // to the CPU) actually works instead of failing wgpu's
                // `COPY_SRC` validation the first time anyone calls it.
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
                _ => None,
            })
            .collect();

        let mut builder = BindGroupBuilder::new(target.bind_group_layout());
        for (name, entry) in &self.params {
            let binding = binding_index(target.binding_entries(), name)?;
            builder = match entry {
                BindingInstanceEntry::Texture(id) => builder.with_texture_2d_at(binding, textures.get(Handle::new(*id))?),
                BindingInstanceEntry::TextureArray(id) => {
                    builder.with_texture_array_at(binding, texture_arrays.get(Handle::new(*id))?)
                }
                BindingInstanceEntry::Cubemap(id) => builder.with_texture_cubemap_at(binding, cubemaps.get(Handle::new(*id))?),
                BindingInstanceEntry::Sampler(kind) => builder.with_sampler_at(binding, samplers.get(*kind)),
                BindingInstanceEntry::Uniform(_) | BindingInstanceEntry::Storage(_) => {
                    let buf = &owned_buffers.iter().find(|(n, _)| n == name)?.1;
                    builder.with_buffer_at(binding, buf)
                }
            };
        }
        let bind_group = builder.build(backend);

        Some(GPUBindingInstance {
            target: self.target,
            bind_group,
            buffers: owned_buffers,
            _marker: PhantomData,
        })
    }
}

/// A material instance uploaded to the GPU — [`GPUBindingInstance`] bound
/// against a [`GPUMaterial`](super::material::GPUMaterial).
pub type GPUMaterialInstance = GPUBindingInstance<super::material::Material>;
/// Source data for a [`GPUMaterialInstance`].
pub type MaterialInstance = BindingInstance<super::material::Material>;
/// Builds a [`MaterialInstance`].
pub type MaterialInstanceBuilder = BindingInstanceBuilder<super::material::Material>;

/// A compute instance uploaded to the GPU — [`GPUBindingInstance`] bound
/// against a [`GPUCompute`](super::compute::GPUCompute).
pub type GPUComputeInstance = GPUBindingInstance<super::compute::Compute>;
/// Source data for a [`GPUComputeInstance`].
pub type ComputeInstance = BindingInstance<super::compute::Compute>;
/// Builds a [`ComputeInstance`].
pub type ComputeInstanceBuilder = BindingInstanceBuilder<super::compute::Compute>;

crate::wgpu::plugin_macros::asset_plugin! {
    /// Registers the [`MaterialInstance`] asset pipeline. Included by
    /// [`WGPUPlugin`](super::backend::WGPUPlugin); add directly only if
    /// you're assembling the `wgpu` module's plugins by hand.
    MaterialInstancePlugin, MaterialInstance
}

crate::wgpu::plugin_macros::asset_plugin! {
    /// Registers the [`ComputeInstance`] asset pipeline. Included by
    /// [`WGPUPlugin`](super::backend::WGPUPlugin); add directly only if
    /// you're assembling the `wgpu` module's plugins by hand.
    ComputeInstancePlugin, ComputeInstance
}
