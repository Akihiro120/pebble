use std::collections::HashMap;

use crate::{
    ecs::{commands::Commands, resources::Read},
    graphics::render::Backend,
};

const MIP_SHADER: &str = r#"
struct VOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn vs_main(@builtin(vertex_index) idx: u32) -> VOut {
    var out: VOut;
    let x = f32((idx << 1u) & 2u);
    let y = f32(idx & 2u);
    out.pos = vec4<f32>(x * 2.0 - 1.0, 1.0 - y * 2.0, 0.0, 1.0);
    out.uv = vec2<f32>(x, y);
    return out;
}

@group(0) @binding(0) var src_texture: texture_2d<f32>;
@group(0) @binding(1) var src_sampler: sampler;

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    return textureSample(src_texture, src_sampler, in.uv);
}
"#;

const SUPPORTED_FORMATS: &[wgpu::TextureFormat] = &[
    wgpu::TextureFormat::Rgba8Unorm,
    wgpu::TextureFormat::Rgba8UnormSrgb,
    wgpu::TextureFormat::Rgba16Float,
    wgpu::TextureFormat::Rgba32Float,
];

#[derive(Default, Clone, Copy)]
pub(crate) enum MipLevels {
    #[default]
    None,
    Full,
    Fixed(u32),
}

pub(crate) fn mip_count(max_dimension: u32, requested: MipLevels) -> u32 {
    let full = (max_dimension as f32).log2().floor() as u32 + 1;
    match requested {
        MipLevels::None => 1,
        MipLevels::Full => full,
        MipLevels::Fixed(count) => count.clamp(1, full),
    }
}

pub(crate) fn texture_usage(mip_count: u32) -> wgpu::TextureUsages {
    if mip_count > 1 {
        wgpu::TextureUsages::TEXTURE_BINDING
            | wgpu::TextureUsages::COPY_DST
            | wgpu::TextureUsages::RENDER_ATTACHMENT
    } else {
        wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST
    }
}

struct MipPipeline {
    pipeline: wgpu::RenderPipeline,
    filterable: bool,
}

pub struct MipmapGenerator {
    filtering_layout: wgpu::BindGroupLayout,
    nonfiltering_layout: wgpu::BindGroupLayout,
    linear_sampler: wgpu::Sampler,
    nearest_sampler: wgpu::Sampler,
    pipelines: HashMap<wgpu::TextureFormat, MipPipeline>,
}

impl MipmapGenerator {
    fn bind_group_layout(device: &wgpu::Device, filterable: bool) -> wgpu::BindGroupLayout {
        device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("mipmap-blit-bind-group-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(if filterable {
                        wgpu::SamplerBindingType::Filtering
                    } else {
                        wgpu::SamplerBindingType::NonFiltering
                    }),
                    count: None,
                },
            ],
        })
    }

    fn build_pipeline(
        device: &wgpu::Device,
        module: &wgpu::ShaderModule,
        layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
    ) -> wgpu::RenderPipeline {
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("mipmap-blit-pipeline-layout"),
            bind_group_layouts: &[Some(layout)],
            immediate_size: 0,
        });

        device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("mipmap-blit-pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module,
                entry_point: Some("vs_main"),
                compilation_options: Default::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                unclipped_depth: false,
                polygon_mode: wgpu::PolygonMode::Fill,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module,
                entry_point: Some("fs_main"),
                compilation_options: Default::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        })
    }

    fn new(backend: &Backend) -> Self {
        let device = &backend.device;
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("mipmap-blit-shader"),
            source: wgpu::ShaderSource::Wgsl(MIP_SHADER.into()),
        });

        let filtering_layout = Self::bind_group_layout(device, true);
        let nonfiltering_layout = Self::bind_group_layout(device, false);

        let linear_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mipmap-linear-sampler"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let nearest_sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("mipmap-nearest-sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        let mut pipelines = HashMap::new();
        for &format in SUPPORTED_FORMATS {
            let filterable = format != wgpu::TextureFormat::Rgba32Float;
            let layout = if filterable { &filtering_layout } else { &nonfiltering_layout };
            let pipeline = Self::build_pipeline(device, &module, layout, format);
            pipelines.insert(format, MipPipeline { pipeline, filterable });
        }

        Self { filtering_layout, nonfiltering_layout, linear_sampler, nearest_sampler, pipelines }
    }

    pub(crate) fn generate_mips(
        &self,
        backend: &Backend,
        texture: &wgpu::Texture,
        format: wgpu::TextureFormat,
        mip_count: u32,
        layer_count: u32,
    ) {
        if mip_count <= 1 {
            return;
        }

        let Some(entry) = self.pipelines.get(&format) else {
            tracing::error!("MipmapGenerator: no blit pipeline for format {format:?}, skipping mip generation");
            return;
        };
        let layout = if entry.filterable { &self.filtering_layout } else { &self.nonfiltering_layout };
        let sampler = if entry.filterable { &self.linear_sampler } else { &self.nearest_sampler };

        let device = &backend.device;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("mipmap-blit-encoder"),
        });

        for layer in 0..layer_count {
            for level in 1..mip_count {
                let src_view = texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_mip_level: level - 1,
                    mip_level_count: Some(1),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                });
                let dst_view = texture.create_view(&wgpu::TextureViewDescriptor {
                    dimension: Some(wgpu::TextureViewDimension::D2),
                    base_mip_level: level,
                    mip_level_count: Some(1),
                    base_array_layer: layer,
                    array_layer_count: Some(1),
                    ..Default::default()
                });

                let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                    label: Some("mipmap-blit-bind-group"),
                    layout,
                    entries: &[
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&src_view) },
                        wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(sampler) },
                    ],
                });

                let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("mipmap-blit-pass"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: &dst_view,
                        depth_slice: None,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    timestamp_writes: None,
                    occlusion_query_set: None,
                    multiview_mask: None,
                });
                pass.set_pipeline(&entry.pipeline);
                pass.set_bind_group(0, &bind_group, &[]);
                pass.draw(0..3, 0..1);
                drop(pass);
            }
        }

        backend.queue.submit(std::iter::once(encoder.finish()));
    }
}

pub(crate) fn init_mipmap_generator(
    backend: Option<Read<Backend>>,
    existing: Option<Read<MipmapGenerator>>,
    mut commands: Commands,
) {
    if existing.is_some() {
        return;
    }
    let Some(backend) = backend else {
        return;
    };
    commands.insert_resource(MipmapGenerator::new(&backend));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mip_count_is_one_when_mips_are_not_requested() {
        assert_eq!(mip_count(2048, MipLevels::None), 1);
        assert_eq!(mip_count(1, MipLevels::None), 1);
        assert_eq!(mip_count(0, MipLevels::None), 1);
    }

    #[test]
    fn mip_count_matches_a_full_power_of_two_chain() {
        assert_eq!(mip_count(1, MipLevels::Full), 1);
        assert_eq!(mip_count(2, MipLevels::Full), 2);
        assert_eq!(mip_count(4, MipLevels::Full), 3);
        assert_eq!(mip_count(256, MipLevels::Full), 9);
        assert_eq!(mip_count(1024, MipLevels::Full), 11);
    }

    #[test]
    fn mip_count_floors_a_non_power_of_two_dimension() {
        assert_eq!(mip_count(300, MipLevels::Full), 9);
    }

    #[test]
    fn mip_count_of_a_zero_dimension_does_not_panic_or_underflow() {
        assert_eq!(mip_count(0, MipLevels::Full), 1);
    }

    #[test]
    fn mip_count_fixed_is_used_as_is_when_within_the_full_chain_length() {
        // 256 -> full chain is 9 levels; requesting 5 explicitly should be honored exactly.
        assert_eq!(mip_count(256, MipLevels::Fixed(5)), 5);
    }

    #[test]
    fn mip_count_fixed_is_clamped_to_the_full_chain_length() {
        // Can't have more levels than the dimension actually supports.
        assert_eq!(mip_count(256, MipLevels::Fixed(100)), 9);
    }

    #[test]
    fn mip_count_fixed_is_clamped_to_at_least_one() {
        assert_eq!(mip_count(256, MipLevels::Fixed(0)), 1);
    }

    #[test]
    fn texture_usage_adds_render_attachment_only_when_there_is_more_than_one_mip() {
        let single = texture_usage(1);
        assert!(!single.contains(wgpu::TextureUsages::RENDER_ATTACHMENT));
        assert!(single.contains(wgpu::TextureUsages::TEXTURE_BINDING));
        assert!(single.contains(wgpu::TextureUsages::COPY_DST));

        let chained = texture_usage(5);
        assert!(chained.contains(wgpu::TextureUsages::RENDER_ATTACHMENT));
        assert!(chained.contains(wgpu::TextureUsages::TEXTURE_BINDING));
        assert!(chained.contains(wgpu::TextureUsages::COPY_DST));
    }
}
