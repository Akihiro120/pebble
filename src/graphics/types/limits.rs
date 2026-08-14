/// GPU limits granted to the device — see [`Backend::limits`](super::render::Backend::limits).
///
/// Like [`DeviceFeatures`](super::flags::DeviceFeatures), this mirrors a curated
/// subset of `wgpu::Limits` rather than every field wgpu exposes — the omitted
/// fields are mostly mesh-shader/ray-tracing-specific limits that pair with
/// `DeviceFeatures::MESH_SHADER`/`RAY_QUERY`, which aren't commonly needed
/// outside those features.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct DeviceLimits {
    pub max_texture_dimension_1d: u32,
    pub max_texture_dimension_2d: u32,
    pub max_texture_dimension_3d: u32,
    pub max_texture_array_layers: u32,
    pub max_bind_groups: u32,
    pub max_bindings_per_bind_group: u32,
    pub max_dynamic_uniform_buffers_per_pipeline_layout: u32,
    pub max_dynamic_storage_buffers_per_pipeline_layout: u32,
    pub max_sampled_textures_per_shader_stage: u32,
    pub max_samplers_per_shader_stage: u32,
    pub max_storage_buffers_per_shader_stage: u32,
    pub max_storage_textures_per_shader_stage: u32,
    pub max_uniform_buffers_per_shader_stage: u32,
    pub max_uniform_buffer_binding_size: u64,
    pub max_storage_buffer_binding_size: u64,
    pub max_vertex_buffers: u32,
    pub max_buffer_size: u64,
    pub max_vertex_attributes: u32,
    pub max_vertex_buffer_array_stride: u32,
    pub min_uniform_buffer_offset_alignment: u32,
    pub min_storage_buffer_offset_alignment: u32,
    pub max_color_attachments: u32,
    pub max_color_attachment_bytes_per_sample: u32,
    pub max_compute_workgroup_storage_size: u32,
    pub max_compute_invocations_per_workgroup: u32,
    pub max_compute_workgroup_size_x: u32,
    pub max_compute_workgroup_size_y: u32,
    pub max_compute_workgroup_size_z: u32,
    pub max_compute_workgroups_per_dimension: u32,
}

impl From<wgpu::Limits> for DeviceLimits {
    fn from(value: wgpu::Limits) -> Self {
        Self {
            max_texture_dimension_1d: value.max_texture_dimension_1d,
            max_texture_dimension_2d: value.max_texture_dimension_2d,
            max_texture_dimension_3d: value.max_texture_dimension_3d,
            max_texture_array_layers: value.max_texture_array_layers,
            max_bind_groups: value.max_bind_groups,
            max_bindings_per_bind_group: value.max_bindings_per_bind_group,
            max_dynamic_uniform_buffers_per_pipeline_layout: value
                .max_dynamic_uniform_buffers_per_pipeline_layout,
            max_dynamic_storage_buffers_per_pipeline_layout: value
                .max_dynamic_storage_buffers_per_pipeline_layout,
            max_sampled_textures_per_shader_stage: value.max_sampled_textures_per_shader_stage,
            max_samplers_per_shader_stage: value.max_samplers_per_shader_stage,
            max_storage_buffers_per_shader_stage: value.max_storage_buffers_per_shader_stage,
            max_storage_textures_per_shader_stage: value.max_storage_textures_per_shader_stage,
            max_uniform_buffers_per_shader_stage: value.max_uniform_buffers_per_shader_stage,
            max_uniform_buffer_binding_size: value.max_uniform_buffer_binding_size,
            max_storage_buffer_binding_size: value.max_storage_buffer_binding_size,
            max_vertex_buffers: value.max_vertex_buffers,
            max_buffer_size: value.max_buffer_size,
            max_vertex_attributes: value.max_vertex_attributes,
            max_vertex_buffer_array_stride: value.max_vertex_buffer_array_stride,
            min_uniform_buffer_offset_alignment: value.min_uniform_buffer_offset_alignment,
            min_storage_buffer_offset_alignment: value.min_storage_buffer_offset_alignment,
            max_color_attachments: value.max_color_attachments,
            max_color_attachment_bytes_per_sample: value.max_color_attachment_bytes_per_sample,
            max_compute_workgroup_storage_size: value.max_compute_workgroup_storage_size,
            max_compute_invocations_per_workgroup: value.max_compute_invocations_per_workgroup,
            max_compute_workgroup_size_x: value.max_compute_workgroup_size_x,
            max_compute_workgroup_size_y: value.max_compute_workgroup_size_y,
            max_compute_workgroup_size_z: value.max_compute_workgroup_size_z,
            max_compute_workgroups_per_dimension: value.max_compute_workgroups_per_dimension,
        }
    }
}
