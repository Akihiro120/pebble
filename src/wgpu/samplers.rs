use std::collections::HashMap;

use crate::{assets::singleton_asset::LazyResource, wgpu::backend::WGPUBackend};

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum SamplerKind {
    LinearRepeat,
    LinearClamp,
    LinearClampNoMip,
    Nearest,
    NearestClampBorder,
    CompareLess,
}

impl SamplerKind {
    pub fn descriptor(&self) -> wgpu::SamplerDescriptor<'static> {
        match self {
            SamplerKind::LinearRepeat => wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::Repeat,
                address_mode_v: wgpu::AddressMode::Repeat,
                address_mode_w: wgpu::AddressMode::Repeat,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            },
            SamplerKind::LinearClamp => wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            },
            SamplerKind::LinearClampNoMip => wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                lod_min_clamp: 0.0,
                lod_max_clamp: 0.0,
                ..Default::default()
            },
            SamplerKind::Nearest => wgpu::SamplerDescriptor {
                mag_filter: wgpu::FilterMode::Nearest,
                min_filter: wgpu::FilterMode::Nearest,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                ..Default::default()
            },
            SamplerKind::NearestClampBorder => {
                let address_mode = if cfg!(target_arch = "wasm32") {
                    wgpu::AddressMode::ClampToEdge
                } else {
                    wgpu::AddressMode::ClampToBorder
                };
                wgpu::SamplerDescriptor {
                    address_mode_u: address_mode,
                    address_mode_v: address_mode,
                    address_mode_w: address_mode,
                    mag_filter: wgpu::FilterMode::Nearest,
                    min_filter: wgpu::FilterMode::Nearest,
                    mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                    border_color: if cfg!(target_arch = "wasm32") {
                        None
                    } else {
                        Some(wgpu::SamplerBorderColor::OpaqueWhite)
                    },
                    ..Default::default()
                }
            }
            SamplerKind::CompareLess => wgpu::SamplerDescriptor {
                address_mode_u: wgpu::AddressMode::ClampToEdge,
                address_mode_v: wgpu::AddressMode::ClampToEdge,
                address_mode_w: wgpu::AddressMode::ClampToEdge,
                mag_filter: wgpu::FilterMode::Linear,
                min_filter: wgpu::FilterMode::Linear,
                mipmap_filter: wgpu::MipmapFilterMode::Linear,
                compare: Some(wgpu::CompareFunction::Less),
                ..Default::default()
            },
        }
    }
}

const ALL_SAMPLER_KINDS: [SamplerKind; 6] = [
    SamplerKind::LinearRepeat,
    SamplerKind::LinearClamp,
    SamplerKind::LinearClampNoMip,
    SamplerKind::Nearest,
    SamplerKind::NearestClampBorder,
    SamplerKind::CompareLess,
];

/// Every [`SamplerKind`] built once and shared across all materials, rather
/// than each material instance creating its own duplicate `wgpu::Sampler`.
pub struct GlobalSamplers {
    samplers: HashMap<SamplerKind, wgpu::Sampler>,
}

impl GlobalSamplers {
    /// Look up a shared sampler by kind. Panics if `kind` is somehow missing
    /// — every [`SamplerKind`] variant is built eagerly in [`LazyResource::construct`].
    pub fn get(&self, kind: SamplerKind) -> &wgpu::Sampler {
        self.samplers
            .get(&kind)
            .expect("GlobalSamplers: all SamplerKind variants are built at construction")
    }
}

impl LazyResource<WGPUBackend> for GlobalSamplers {
    type Deps<'a> = ();

    fn construct<'a>(backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let samplers = ALL_SAMPLER_KINDS
            .iter()
            .map(|&kind| (kind, backend.device.create_sampler(&kind.descriptor())))
            .collect();
        Some(Self { samplers })
    }
}
