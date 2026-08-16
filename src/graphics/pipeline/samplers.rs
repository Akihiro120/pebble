use std::collections::HashMap;

use crate::{
    ecs::{commands::Commands, resources::Read},
    graphics::{render::Backend, types::flags::DeviceFeatures},
};

/// One of a fixed set of ready-made samplers, built once at startup and
/// looked up via [`GlobalSamplers`] — no per-texture sampler creation.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum SamplerKind {
    LinearRepeat,
    LinearClamp,
    LinearClampNoMip,
    Nearest,
    NearestClampBorder,
    LinearClampBorder,
    CompareLess,
}

impl SamplerKind {
    pub(crate) fn descriptor(&self, clamp_to_border_supported: bool) -> wgpu::SamplerDescriptor<'static> {
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
                let use_border = clamp_to_border_supported && !cfg!(target_arch = "wasm32");
                let address_mode =
                    if use_border { wgpu::AddressMode::ClampToBorder } else { wgpu::AddressMode::ClampToEdge };
                wgpu::SamplerDescriptor {
                    address_mode_u: address_mode,
                    address_mode_v: address_mode,
                    address_mode_w: address_mode,
                    mag_filter: wgpu::FilterMode::Nearest,
                    min_filter: wgpu::FilterMode::Nearest,
                    mipmap_filter: wgpu::MipmapFilterMode::Nearest,
                    border_color: if use_border { Some(wgpu::SamplerBorderColor::OpaqueWhite) } else { None },
                    ..Default::default()
                }
            }
            SamplerKind::LinearClampBorder => {
                let use_border = clamp_to_border_supported && !cfg!(target_arch = "wasm32");
                let address_mode =
                    if use_border { wgpu::AddressMode::ClampToBorder } else { wgpu::AddressMode::ClampToEdge };
                wgpu::SamplerDescriptor {
                    address_mode_u: address_mode,
                    address_mode_v: address_mode,
                    address_mode_w: address_mode,
                    mag_filter: wgpu::FilterMode::Linear,
                    min_filter: wgpu::FilterMode::Linear,
                    mipmap_filter: wgpu::MipmapFilterMode::Linear,
                    border_color: if use_border { Some(wgpu::SamplerBorderColor::OpaqueWhite) } else { None },
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

const ALL_SAMPLER_KINDS: [SamplerKind; 7] = [
    SamplerKind::LinearRepeat,
    SamplerKind::LinearClamp,
    SamplerKind::LinearClampNoMip,
    SamplerKind::Nearest,
    SamplerKind::NearestClampBorder,
    SamplerKind::LinearClampBorder,
    SamplerKind::CompareLess,
];

/// A GPU sampler, wrapping `wgpu::Sampler`.
pub struct Sampler(wgpu::Sampler);

impl Sampler {
    pub(crate) fn raw(&self) -> &wgpu::Sampler {
        &self.0
    }
}

/// Every [`SamplerKind`], pre-built — inserted as a resource by
/// [`BuiltinAssetsPlugin`](crate::graphics::BuiltinAssetsPlugin).
pub struct GlobalSamplers {
    samplers: HashMap<SamplerKind, Sampler>,
}

impl GlobalSamplers {
    pub fn get(&self, kind: SamplerKind) -> &Sampler {
        self.samplers
            .get(&kind)
            .expect("GlobalSamplers: all SamplerKind variants are built at construction")
    }
}

pub(crate) fn init_global_samplers(backend: Read<Backend>, mut commands: Commands) {
    // on wasm this kind falls back to ClampToEdge and needs no feature — see descriptor()
    let clamp_to_border_supported =
        cfg!(target_arch = "wasm32") || backend.features().contains(DeviceFeatures::ADDRESS_MODE_CLAMP_TO_BORDER);
    if !clamp_to_border_supported {
        tracing::warn!(
            "SamplerKind::NearestClampBorder/LinearClampBorder need DeviceFeatures::ADDRESS_MODE_CLAMP_TO_BORDER, \
             which this device wasn't given — falling back to ClampToEdge for them. Enable the feature via \
             GraphicsPlugin::with_features to get an actual border color."
        );
    }
    let samplers = ALL_SAMPLER_KINDS
        .iter()
        .map(|&kind| (kind, Sampler(backend.device.create_sampler(&kind.descriptor(clamp_to_border_supported)))))
        .collect();
    commands.insert_resource(GlobalSamplers { samplers });
}
