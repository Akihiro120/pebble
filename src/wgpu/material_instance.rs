use crate::{
    app::App,
    assets::{
        plugin::AssetPlugin,
        storage::{ProcessedAssets, RawAssetHandle},
        upload::Asset,
    },
    ecs::{plugin::Plugin, system::Res},
    wgpu::{
        backend::WGPUBackend,
        material::{GPUMaterial, MaterialBindingEntry},
        samplers::{GlobalSamplers, SamplerKind},
    },
};

#[derive(Clone, PartialEq, Eq, Hash)]
pub enum MaterialInstanceBindingEntry {
    Texture(RawAssetHandle),
    TextureArray(RawAssetHandle),
    Cubemap(RawAssetHandle),
    Sampler(SamplerKind),
    Uniform(Vec<u8>),
    Storage(Vec<u8>),
}

pub struct MaterialInstanceDescriptor {
    pub material: RawAssetHandle,
    pub params: Vec<(&'static str, MaterialInstanceBindingEntry)>,
}

pub fn binding_index(entries: &[MaterialBindingEntry], name: &str) -> Option<u32> {
    entries
        .iter()
        .position(|e| e.name == name)
        .map(|i| i as u32)
}

pub fn build_instance_build_group(
    device: &wgpu::Device,
    layout: &wgpu::BindGroupLayout,
    material_entries: &[MaterialBindingEntry],
    resolved: &[(&'static str, wgpu::BindingResource)],
) -> Option<wgpu::BindGroup> {
    let mut entries = Vec::with_capacity(resolved.len());
    for (name, resource) in resolved {
        let binding = binding_index(material_entries, *name)?;
        entries.push(wgpu::BindGroupEntry {
            binding,
            resource: resource.clone(),
        })
    }

    Some(device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout,
        entries: &entries,
    }))
}

pub struct GPUMaterialInstance {
    pub material: RawAssetHandle,
    pub bind_group: wgpu::BindGroup,
    /// Uniform/storage buffers created for this instance's bindings — kept
    /// alive here since `bind_group` only holds GPU-side references to them.
    _owned_buffers: Vec<wgpu::Buffer>,
}

impl Asset<WGPUBackend> for GPUMaterialInstance {
    type Source = MaterialInstanceDescriptor;
    type Deps<'a> = (
        Res<'a, ProcessedAssets<GPUMaterial>>,
        Res<'a, ProcessedAssets<super::textures::GPUTexture>>,
        Res<'a, ProcessedAssets<super::texture_array::GPUTextureArray>>,
        Res<'a, ProcessedAssets<super::cubemap::GPUCubemap>>,
        Res<'a, GlobalSamplers>,
    );

    fn upload<'a>(
        source: &MaterialInstanceDescriptor,
        backend: &WGPUBackend,
        deps: &Self::Deps<'a>,
    ) -> Option<Self> {
        let (materials, textures, texture_arrays, cubemaps, samplers) = deps;
        let material = materials.get(source.material)?;

        // Two passes: first resolve every binding, deferring uniform/storage
        // buffers to an index into `owned_buffers` rather than taking a
        // reference immediately — a `BindingResource` borrowed from a Vec
        // slot can't coexist with later pushes into that same Vec.
        enum Pending<'a> {
            Direct(wgpu::BindingResource<'a>),
            OwnedBuffer(usize),
        }

        let mut owned_buffers: Vec<wgpu::Buffer> = Vec::new();
        let mut pending: Vec<(&'static str, Pending)> = Vec::new();

        for (name, entry) in &source.params {
            let resource = match entry {
                MaterialInstanceBindingEntry::Texture(id) => {
                    Pending::Direct(wgpu::BindingResource::TextureView(&textures.get(*id)?.view))
                }
                MaterialInstanceBindingEntry::TextureArray(id) => Pending::Direct(
                    wgpu::BindingResource::TextureView(&texture_arrays.get(*id)?.view),
                ),
                MaterialInstanceBindingEntry::Cubemap(id) => {
                    Pending::Direct(wgpu::BindingResource::TextureView(&cubemaps.get(*id)?.view))
                }
                MaterialInstanceBindingEntry::Sampler(kind) => {
                    Pending::Direct(wgpu::BindingResource::Sampler(samplers.get(*kind)))
                }
                MaterialInstanceBindingEntry::Uniform(bytes) => {
                    use wgpu::util::DeviceExt;
                    let buf =
                        backend
                            .device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: None,
                                contents: bytes,
                                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
                            });
                    owned_buffers.push(buf);
                    Pending::OwnedBuffer(owned_buffers.len() - 1)
                }
                MaterialInstanceBindingEntry::Storage(bytes) => {
                    use wgpu::util::DeviceExt;
                    let buf =
                        backend
                            .device
                            .create_buffer_init(&wgpu::util::BufferInitDescriptor {
                                label: None,
                                contents: bytes,
                                usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                            });
                    owned_buffers.push(buf);
                    Pending::OwnedBuffer(owned_buffers.len() - 1)
                }
            };
            pending.push((*name, resource));
        }

        let resolved: Vec<(&'static str, wgpu::BindingResource)> = pending
            .into_iter()
            .map(|(name, p)| {
                let resource = match p {
                    Pending::Direct(r) => r,
                    Pending::OwnedBuffer(i) => owned_buffers[i].as_entire_binding(),
                };
                (name, resource)
            })
            .collect();

        let bind_group = build_instance_build_group(
            &backend.device,
            &material.layout,
            &material.entries,
            &resolved,
        )?;

        Some(Self {
            material: source.material,
            bind_group,
            _owned_buffers: owned_buffers,
        })
    }
}

pub struct MaterialInstancePlugin;
impl MaterialInstancePlugin {
    pub fn new() -> Self {
        Self
    }
}
impl Plugin for MaterialInstancePlugin {
    fn build(&self, app: &mut App) {
        app.add_plugin(AssetPlugin::<WGPUBackend, GPUMaterialInstance>::new());
    }
}
