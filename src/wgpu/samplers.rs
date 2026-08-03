use std::collections::HashMap;

use crate::{assets::singleton_asset::LazyResource, wgpu::backend::WGPUBackend};

/// A named, pre-built sampler configuration. Every variant is built once at
/// startup and shared via [`GlobalSamplers`] — look one up with
/// `GlobalSamplers::get(kind)` rather than creating your own `wgpu::Sampler`.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub enum SamplerKind {
    /// Linear filtering, tiling (`Repeat`) address mode, mipmapped.
    LinearRepeat,
    /// Linear filtering, edge-clamped, mipmapped.
    LinearClamp,
    /// Linear filtering, edge-clamped, no mipmapping (`lod_min/max_clamp = 0`).
    LinearClampNoMip,
    /// Nearest-neighbor filtering, `wgpu`'s default (repeat) address mode.
    Nearest,
    /// Nearest-neighbor filtering, clamped to a border color. On web this
    /// falls back to edge-clamping instead (see [`descriptor`](Self::descriptor)'s
    /// docs) since WebGPU has no border-color support.
    NearestClampBorder,
    /// Linear filtering, edge-clamped, with `CompareFunction::Less` — for
    /// shadow-map `textureSampleCompare`.
    CompareLess,
}

impl SamplerKind {
    /// The `wgpu::SamplerDescriptor` this kind builds.
    ///
    /// `NearestClampBorder` is the one variant that isn't identical across
    /// platforms: WebGPU doesn't support `ClampToBorder`/a border color at
    /// all, so on `wasm32` this silently downgrades to `ClampToEdge` (no
    /// border color) instead — sampling outside `[0, 1]` UV will read edge
    /// texels on web instead of the border color you'd get natively. If
    /// your material depends on the border being visually distinct from the
    /// edge, this is a real cross-platform behavior difference to account
    /// for, not just an implementation detail.
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
