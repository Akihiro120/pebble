use crate::{assets::storage::RawAssetHandle, wgpu::samplers::SamplerKind};

/// The small, closed vocabulary of value kinds a material instance
/// parameter can be. Arrays instead of a math-library type so this
/// module has no dependency beyond wgpu itself — convert to/from your
/// own vector type at the call site.
pub enum ParamValue {
    Texture(RawAssetHandle),
    Cubemap(RawAssetHandle),
    Sampler(SamplerKind),
    Float(f32),
    Vec4([f32; 4]),
    /// Escape hatch: raw bytes for anything not covered above (e.g. a
    /// custom struct of several packed values).
    Bytes(Vec<u8>),
}

/// One instance's full set of parameter values, by name. Resolve each
/// name against the base material's `MaterialDescriptor::binding_index`
/// when building the actual bind group, in your own `Asset::upload`.
pub struct MaterialInstanceDescriptor {
    pub material: RawAssetHandle,
    pub params: Vec<(&'static str, ParamValue)>,
}

impl MaterialInstanceDescriptor {
    pub fn new(material: RawAssetHandle) -> Self {
        Self {
            material,
            params: Vec::new(),
        }
    }

    pub fn with_texture(mut self, name: &'static str, handle: RawAssetHandle) -> Self {
        self.params.push((name, ParamValue::Texture(handle)));
        self
    }
    pub fn with_cubemap(mut self, name: &'static str, handle: RawAssetHandle) -> Self {
        self.params.push((name, ParamValue::Cubemap(handle)));
        self
    }
    pub fn with_sampler(mut self, name: &'static str, kind: SamplerKind) -> Self {
        self.params.push((name, ParamValue::Sampler(kind)));
        self
    }
    pub fn with_float(mut self, name: &'static str, value: f32) -> Self {
        self.params.push((name, ParamValue::Float(value)));
        self
    }
    pub fn with_vec4(mut self, name: &'static str, value: [f32; 4]) -> Self {
        self.params.push((name, ParamValue::Vec4(value)));
        self
    }

    pub fn with_bytes(mut self, name: &'static str, bytes: Vec<u8>) -> Self {
        self.params.push((name, ParamValue::Bytes(bytes)));
        self
    }
}
