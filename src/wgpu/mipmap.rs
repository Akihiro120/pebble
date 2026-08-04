use std::collections::HashMap;

use crate::{assets::singleton_asset::LazyResource, wgpu::{backend::WGPUBackend, buffers::BindGroupBuilder}};

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

/// Formats [`MipmapGenerator`] knows how to downsample — matches the formats
/// `GPUTexture` can decode into (see `wgpu::textures::bytes_per_pixel`).
const SUPPORTED_FORMATS: &[wgpu::TextureFormat] = &[
    wgpu::TextureFormat::Rgba8Unorm,
    wgpu::TextureFormat::Rgba8UnormSrgb,
    wgpu::TextureFormat::Rgba16Float,
    wgpu::TextureFormat::Rgba32Float,
];

/// Number of mip levels a texture with the given largest dimension needs
/// for a full chain (1 if `generate_mips` is false). Shared by every
/// texture-like upload (`GPUTexture`/`GPUCubemap`/`GPUTextureArray`) so the
/// formula — and its zero-dimension edge case (a saturating float→int cast
/// to 0, `+ 1` gives back a "chain" of 1) — is written once.
pub(crate) fn mip_count(max_dimension: u32, generate_mips: bool) -> u32 {
    if generate_mips {
        (max_dimension as f32).log2().floor() as u32 + 1
    } else {
        1
    }
}

/// Usage flags for a texture with `mip_count` levels: mips beyond level 0
/// are rendered into on the GPU (via [`MipmapGenerator::generate_mips`]),
/// so a texture with more than one level needs `RENDER_ATTACHMENT` in
/// addition to the usual sampling/upload usages.
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

/// GPU-side mipmap generator: a shared render pipeline (per texture format)
/// that downsamples one mip level into the next via a fullscreen-triangle
/// blit, plus the bind group layouts/samplers it needs.
///
/// Built once as a [`LazyResource`] and reused by every [`GPUTexture`](crate::wgpu::textures::GPUTexture)
/// upload that requests mips.
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

    fn new(device: &wgpu::Device) -> Self {
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
            // Rgba32Float has no filterable-sampling support without an extra
            // device feature, so it gets a nearest-sampled, non-filtering path.
            let filterable = format != wgpu::TextureFormat::Rgba32Float;
            let layout = if filterable {
                &filtering_layout
            } else {
                &nonfiltering_layout
            };
            let pipeline = Self::build_pipeline(device, &module, layout, format);
            pipelines.insert(format, MipPipeline { pipeline, filterable });
        }

        Self {
            filtering_layout,
            nonfiltering_layout,
            linear_sampler,
            nearest_sampler,
            pipelines,
        }
    }

    /// Downsamples `texture` (already holding valid data at mip level 0 of
    /// every layer) into its remaining `mip_count - 1` levels, entirely on
    /// the GPU. `layer_count` is 1 for a plain 2D texture, N for a texture
    /// array, or 6 for a cubemap — each array layer/face is downsampled
    /// independently via its own single-layer 2D view.
    pub fn generate_mips(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
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
        let layout = if entry.filterable {
            &self.filtering_layout
        } else {
            &self.nonfiltering_layout
        };
        let sampler = if entry.filterable {
            &self.linear_sampler
        } else {
            &self.nearest_sampler
        };

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

                // `_at`/raw-view primitives, not `.texture_2d()`/`.sampler()`:
                // `src_view` is an ad-hoc single-mip-level view, not a whole
                // `GPUTexture`, and `sampler` here is one of this
                // generator's own internal samplers, not a `Sampler` from
                // `GlobalSamplers` — both `pub(crate)`-only primitives.
                let bind_group = BindGroupBuilder::new(layout)
                    .label("mipmap-blit-bind-group")
                    .texture_view_at(0, &src_view)
                    .sampler_raw_at(1, sampler)
                    .build(device);

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

        queue.submit(std::iter::once(encoder.finish()));
    }
}

impl LazyResource<WGPUBackend> for MipmapGenerator {
    type Deps<'a> = ();

    fn construct<'a>(backend: &WGPUBackend, _deps: &()) -> Option<Self> {
        Some(Self::new(&backend.device))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mip_count_is_one_when_mips_are_not_requested() {
        assert_eq!(mip_count(2048, false), 1);
        assert_eq!(mip_count(1, false), 1);
        assert_eq!(mip_count(0, false), 1);
    }

    #[test]
    fn mip_count_matches_a_full_power_of_two_chain() {
        // A 1x1 texture needs exactly one level; each doubling adds exactly one more.
        assert_eq!(mip_count(1, true), 1);
        assert_eq!(mip_count(2, true), 2);
        assert_eq!(mip_count(4, true), 3);
        assert_eq!(mip_count(256, true), 9);
        assert_eq!(mip_count(1024, true), 11);
    }

    #[test]
    fn mip_count_floors_a_non_power_of_two_dimension() {
        // 300's largest power-of-two factor is 256 (2^8), so the chain is as
        // long as a 256-sized texture's, not rounded up to 512's.
        assert_eq!(mip_count(300, true), 9);
    }

    #[test]
    fn mip_count_of_a_zero_dimension_does_not_panic_or_underflow() {
        // log2(0) is -infinity; the `as u32` cast saturates to 0 rather than
        // panicking or wrapping, so this degenerates to a "chain" of 1
        // instead of corrupting `mip_level_count` on the eventual
        // TextureDescriptor (which must be >= 1 or wgpu panics).
        assert_eq!(mip_count(0, true), 1);
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
