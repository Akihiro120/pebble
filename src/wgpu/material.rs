use crate::{
    assets::upload::Asset,
    wgpu::{backend::WGPUBackend, binding::{BindGroupLayout, BindGroupLayoutBuilder, BindingEntry}},
};

/// A `wgpu::RenderPipeline`, opaque — built only via [`build_material`]/
/// [`GPUMaterial`]'s `Asset::upload`. Bind it against a
/// [`RenderPass`](super::render_pass::RenderPass) via
/// [`RenderPass::set_pipeline`](super::render_pass::RenderPass::set_pipeline);
/// there's no way to reach the underlying `wgpu::RenderPipeline` from
/// outside this crate.
pub struct RenderPipeline(wgpu::RenderPipeline);

impl RenderPipeline {
    pub(crate) fn raw(&self) -> &wgpu::RenderPipeline {
        &self.0
    }
}

/// Describes a render pipeline + its own bind group, the source type
/// [`GPUMaterial`] is built from via [`build_material`]. Start from
/// [`MaterialDescriptor::default()`] and override only the fields that
/// differ from a plain opaque material with no depth testing.
pub struct MaterialDescriptor<'a> {
    /// Debug label, threaded through to the shader module, pipeline, and
    /// bind group layout.
    pub label: Option<&'a str>,
    /// WGSL source for both the vertex and fragment stage.
    pub shader_source: &'a str,
    /// Vertex stage entry point. Defaults to `"vs_main"`.
    pub vertex_entry: Option<&'a str>,
    /// Fragment stage entry point. Defaults to `"fs_main"`.
    pub fragment_entry: Option<&'a str>,
    /// Vertex buffer layouts, in the order buffers will be bound at draw
    /// time (e.g. [`Vertex::layout()`](super::mesh::Vertex::layout)).
    pub vertex_layouts: Vec<wgpu::VertexBufferLayout<'static>>,
    /// This material's own bind group entries. See
    /// [`BindingKind`](super::binding::BindingKind) for what a
    /// material-appropriate entry looks like — [`build_material`] panics if
    /// any entry here is `COMPUTE`-visible.
    pub entries: Vec<BindingEntry>,
    /// Face culling mode. Defaults to `Some(Face::Back)`.
    pub cull_mode: Option<wgpu::Face>,
    /// Depth/stencil state. `None` disables depth testing.
    pub depth: Option<wgpu::DepthStencilState>,
    /// Color target states — one per fragment shader output. See
    /// [`DEFAULT_TARGET`] for a ready-made single-target default.
    pub targets: Vec<wgpu::ColorTargetState>,
    /// Rasterizer polygon mode. Defaults to `Fill`.
    pub polygon_mode: wgpu::PolygonMode,
    /// Which `@group(N)` the layout built from `entries` occupies in the pipeline, or
    /// `None` if this material has no entries of its own (e.g. it only uses `extra_layouts`).
    pub own_group: Option<u32>,
    /// Additional bind group layouts, each tagged with the `@group(N)` it occupies.
    /// Every index from 0 up to the highest one used (including `own_group`, if set) must
    /// be covered exactly once, or `build_material` panics — this makes group assignment
    /// explicit instead of inferred from field order.
    pub extra_layouts: Vec<super::layout::OwnedGroupLayout>,
}

/// A single opaque `Rgba8Unorm` color target with no blending — a
/// ready-made value for [`MaterialDescriptor::targets`] when you don't need
/// anything more specific. Not applied automatically by `Default` (which
/// leaves `targets` empty, since the right format usually depends on the
/// surface/render target), so use it explicitly: `targets:
/// DEFAULT_TARGET.to_vec()`.
pub const DEFAULT_TARGET: [wgpu::ColorTargetState; 1] = [wgpu::ColorTargetState {
    format: wgpu::TextureFormat::Rgba8Unorm,
    blend: None,
    write_mask: wgpu::ColorWrites::ALL,
}];

impl<'a> Default for MaterialDescriptor<'a> {
    fn default() -> Self {
        Self {
            label: None,
            shader_source: "",
            vertex_entry: Some("vs_main"),
            fragment_entry: Some("fs_main"),
            vertex_layouts: Vec::new(),
            entries: Vec::new(),
            cull_mode: Some(wgpu::Face::Back),
            depth: None,
            targets: Vec::new(),
            own_group: Some(0),
            extra_layouts: Vec::new(),
            polygon_mode: wgpu::PolygonMode::Fill,
        }
    }
}

/// Builds a render pipeline and its own bind group layout from `desc`.
///
/// Panics if any of `desc.entries` is visible to the compute stage —
/// [`BindingKind`](super::binding::BindingKind) is shared with
/// [`ComputeDescriptor`](super::compute::ComputeDescriptor), and this is
/// the check that catches a compute-only entry accidentally reused in a
/// material instead of letting it fail deep inside wgpu with a less
/// specific error. The bind group layout itself comes from
/// [`binding::BindGroupLayoutBuilder`](super::binding::BindGroupLayoutBuilder).
/// The pipeline layout is assembled from `desc.own_group` (this material's
/// own layout) plus `desc.extra_layouts`, keyed by explicit `@group(N)` —
/// panics on a gap or a collision across `0..=max`, turning a mismatched
/// `@group(N)` in the shader into an immediate, specific error instead of
/// an opaque wgpu validation failure at draw time.
pub fn build_material(
    device: &wgpu::Device,
    desc: &MaterialDescriptor,
) -> (RenderPipeline, BindGroupLayout) {
    for entry in &desc.entries {
        if entry.kind.visibility().intersects(wgpu::ShaderStages::COMPUTE) {
            panic!(
                "material{}: entry '{}' is visible to the compute stage ({:?}) — material bind \
                 group entries must not be COMPUTE-visible",
                desc.label.map(|l| format!(" '{l}'")).unwrap_or_default(),
                entry.name,
                entry.kind.visibility()
            );
        }
    }

    let layout = BindGroupLayoutBuilder::new()
        .label(desc.label)
        .entries(desc.entries.iter().cloned())
        .build(device);

    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: desc.label,
        source: wgpu::ShaderSource::Wgsl(desc.shader_source.into()),
    });

    let mut slots: Vec<super::layout::GroupLayout> = desc
        .extra_layouts
        .iter()
        .map(|g| super::layout::GroupLayout { group: g.group, layout: &g.layout })
        .collect();
    if let Some(own_group) = desc.own_group {
        slots.push(super::layout::GroupLayout { group: own_group, layout: &layout });
    }
    let bind_group_layouts = super::layout::assemble_bind_group_layouts(desc.label, slots);

    let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: desc.label,
        bind_group_layouts: &bind_group_layouts,
        immediate_size: 0,
    });

    let targets: Vec<Option<wgpu::ColorTargetState>> =
        desc.targets.iter().cloned().map(Some).collect();

    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: desc.label,
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: desc.vertex_entry,
            compilation_options: Default::default(),
            buffers: &desc.vertex_layouts,
        },
        primitive: wgpu::PrimitiveState {
            topology: wgpu::PrimitiveTopology::TriangleList,
            strip_index_format: None,
            front_face: wgpu::FrontFace::Ccw,
            cull_mode: desc.cull_mode,
            unclipped_depth: false,
            polygon_mode: desc.polygon_mode,
            conservative: false,
        },
        depth_stencil: desc.depth.clone(),
        multisample: wgpu::MultisampleState::default(),
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: desc.fragment_entry,
            compilation_options: Default::default(),
            targets: &targets,
        }),
        multiview_mask: None,
        cache: None,
    });

    (RenderPipeline(pipeline), layout)
}

/// A material uploaded to the GPU: a render pipeline plus the bind group
/// layout entries it expects, ready for a
/// [`GPUMaterialInstance`](super::instance::GPUMaterialInstance) to bind
/// actual resources against.
pub struct GPUMaterial {
    pub pipeline: RenderPipeline,
    layout: BindGroupLayout,
    entries: Vec<BindingEntry>,
}

impl super::binding::BindGroupTarget for GPUMaterial {
    fn bind_group_layout(&self) -> &BindGroupLayout {
        &self.layout
    }
    fn binding_entries(&self) -> &[BindingEntry] {
        &self.entries
    }
}

impl Asset<WGPUBackend> for GPUMaterial {
    type Source = MaterialDescriptor<'static>;
    type Deps<'a> = ();

    fn upload<'a>(source: &MaterialDescriptor, backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        let (pipeline, layout) = build_material(&backend.device, source);

        Some(Self {
            pipeline,
            layout,
            entries: source.entries.to_vec(),
        })
    }
}

crate::wgpu::plugin_macros::asset_plugin! {
    /// Registers the [`GPUMaterial`] asset pipeline (`Assets<MaterialDescriptor>`
    /// → `ProcessedAssets<GPUMaterial>`). Included by
    /// [`WGPUPlugin`](super::backend::WGPUPlugin); add directly only if you're
    /// assembling the `wgpu` module's plugins by hand.
    MaterialPlugin, GPUMaterial
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::wgpu::binding::{BindingEntry, BindingKind};
    use crate::wgpu::test_util::with_device;

    const MINIMAL_SHADER: &str = r#"
        @vertex
        fn vs_main() -> @builtin(position) vec4<f32> {
            return vec4<f32>(0.0, 0.0, 0.0, 1.0);
        }
        @fragment
        fn fs_main() -> @location(0) vec4<f32> {
            return vec4<f32>(1.0, 1.0, 1.0, 1.0);
        }
    "#;

    #[test]
    fn a_compute_visible_entry_panics_before_touching_the_device() {
        with_device!(device, _queue, {
            let desc = MaterialDescriptor {
                shader_source: MINIMAL_SHADER,
                entries: vec![BindingEntry {
                    name: "bad",
                    binding: 0,
                    kind: BindingKind::storage_buffer_read_write(wgpu::ShaderStages::COMPUTE),
                }],
                targets: DEFAULT_TARGET.to_vec(),
                ..Default::default()
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                build_material(&device, &desc);
            }));
            assert!(result.is_err(), "expected a panic for a COMPUTE-visible material entry");
        });
    }

    #[test]
    fn a_fragment_visible_entry_builds_without_panicking() {
        with_device!(device, _queue, {
            let desc = MaterialDescriptor {
                shader_source: MINIMAL_SHADER,
                entries: vec![],
                own_group: None,
                targets: DEFAULT_TARGET.to_vec(),
                ..Default::default()
            };
            build_material(&device, &desc);
        });
    }
}
