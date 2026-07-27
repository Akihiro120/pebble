use crate::wgpu::BindingEntry;

pub struct MaterialSpec {
    pub shader_source: &'static str,
    pub entries: Vec<BindingEntry>,
    pub cull_mode: Option<wgpu::Face>,
    pub depth: Option<wgpu::DepthStencilState>,
    pub color_format: Option<wgpu::TextureFormat>,
}

impl MaterialSpec {
    pub fn binding_index(&self, name: &str) -> Option<u32> {
        self.entries
            .iter()
            .position(|e| e.name == name)
            .map(|i| i as u32)
    }
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use crate::{assets::upload::Asset, wgpu::material::MaterialSpec};

    pub struct GPUMaterial {
        pub pipeline: wgpu::RenderPipeline,
        pub layout: wgpu::BindGroupLayout,
    }

    pub struct Material {
        pub descriptor: MaterialSpec,
    }

    impl Asset<wgpu::Device> for GPUMaterial {
        type Source = Material;
        type Deps<'a> = ();

        fn upload<'a>(
            source: &Self::Source,
            backend: &wgpu::Device,
            deps: &Self::Deps<'a>,
        ) -> Option<Self> {
            let desc = &source.descriptor;

            let entries: Vec<_> = source
                .descriptor
                .entries
                .iter()
                .enumerate()
                .map(|(i, e)| e.layout_entry(i as u32))
                .collect();

            let layout = backend.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("Bind Group Layout"),
                entries: &entries,
            });

            let pipeline_layout = backend.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Pipeline Layout"),
                bind_group_layouts: &[Some(&layout)],
                immediate_size: 0,
            });

            let module = backend.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Shader Module"),
                source: wgpu::ShaderSource::Wgsl(Cow::Borrowed(source.descriptor.shader_source)),
            });

            let pipeline = backend.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("Render Pipeline"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &module,
                    entry_point: Some("vs_main"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                primitive: wgpu::PrimitiveState {
                    topology: wgpu::PrimitiveTopology::TriangleList,
                    strip_index_format: None,
                    front_face: wgpu::FrontFace::Ccw,
                    cull_mode: source.descriptor.cull_mode,
                    unclipped_depth: false,
                    polygon_mode: wgpu::PolygonMode::Fill,
                    conservative: false,
                },
                depth_stencil: source.descriptor.depth.clone(),
                multisample: wgpu::MultisampleState::default(),
                fragment: Some(wgpu::FragmentState {
                    module: &module,
                    entry_point: Some("fs_main"),
                    targets: &[],
                    compilation_options: Default::default(),
                }),
                multiview_mask: None,
                cache: None,
            });

            Some(Self { pipeline, layout })
        }
    }
}
