use std::marker::PhantomData;

use crate::{
    assets::{handle::Handle, storage::Assets, upload::{Asset, AssetSource}},
    ecs::resources::Read,
    graphics::{
        pipeline::{
            binding::{BindGroupTarget, BindingEntry},
            buffers::{BindGroup, BindGroupBuilder, Buffer, BufferBuilder},
            compute::Compute,
            cubemap::Cubemap,
            material::Material,
            samplers::{GlobalSamplers, SamplerKind},
            texture_array::TextureArray,
            textures::Texture,
        },
        render::Backend,
        types::flags::BufferUsages,
    },
};

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum BindingInstanceEntry {
    Texture(Handle<Texture>),
    TextureArray(Handle<TextureArray>),
    Cubemap(Handle<Cubemap>),
    Sampler(SamplerKind),
    Uniform(Vec<u8>),
    Storage(Vec<u8>),
}

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

pub fn binding_index(entries: &[BindingEntry], name: &str) -> Option<u32> {
    entries.iter().find(|e| e.name == name).map(|e| e.binding)
}

pub struct GPUBindingInstance<T> {
    pub target: Handle<T>,
    pub bind_group: BindGroup,
    buffers: Vec<(&'static str, Buffer)>,
    _marker: PhantomData<fn() -> T>,
}

impl<T> GPUBindingInstance<T> {
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
                BindingInstanceEntry::Sampler(kind) => builder.with_sampler_at(binding, samplers.get(*kind)),
                BindingInstanceEntry::Uniform(_) | BindingInstanceEntry::Storage(_) => {
                    let buf = &owned_buffers.iter().find(|(n, _)| n == name)?.1;
                    builder.with_buffer_at(binding, buf)
                }
            };
        }
        let bind_group = builder.build(backend);

        Some(GPUBindingInstance { target: self.target, bind_group, buffers: owned_buffers, _marker: PhantomData })
    }
}

pub type GPUMaterialInstance = GPUBindingInstance<Material>;
pub type MaterialInstance = BindingInstance<Material>;

pub type GPUComputeInstance = GPUBindingInstance<Compute>;
pub type ComputeInstance = BindingInstance<Compute>;
